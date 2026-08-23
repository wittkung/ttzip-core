# Phase 0 Research: libarchive 工业级工程卓越性深度解构与全方位体系演进报告

**Feature Directory**: `specs/036-libarchive-standards-architecture-workflow-synthesis`  
**Date**: 2026-08-16  
**Status**: Completed  
**Sources Baseline**: `Vendor/libarchive-upstream` (libarchive v3.7.x+ 源码树、测试套件、构建与治理体系)

---

## 目录
1. [R001: libarchive 流式架构、面向对象多态与内存模型解构](#r001)
2. [R002: libarchive 防御性编程与安全漏洞免疫模型](#r002)
3. [R003: libarchive 黄金预言机测试与质量验证哲学](#r003)
4. [R004: Upstream 治理标准与 Prompt/Skills/Workflows 升级映射](#r004)
5. [综合决策矩阵 (Consolidated Decision Matrix)](#decision-matrix)

---

<a id="r001"></a>
## R001: libarchive 流式架构、面向对象多态与内存模型解构

### 1. 核心研究结论

#### A. 纯 C 语言下的单根继承与双层虚表派发 (Single-Root OOP Layout)
- **基类与派生结构**：libarchive 顶层对象 `struct archive` 作为单根基类，派生类（`struct archive_read`、`struct archive_write`、`struct archive_write_disk`）将 `struct archive archive` 作为自身结构体的第一个成员。利用 C 语言内存布局标准，派生类指针与基类指针物理地址完全一致，实现零开销向上转型 `(struct archive *)`。
- **双层动态派发体系**：
  1. **顶层引擎统一虚表 (`struct archive_vtable`)**：统一定义 `archive_close`、`archive_free`、`archive_read_next_header`、`archive_read_data_block`、`archive_write_header`、`archive_write_data` 等，由 `archive_virtual.c` 实现中继查找。
  2. **格式与过滤器策略虚表**：`archive_read_filter_bidder_vtable`（`bid`, `init`, `free`）、`archive_read_filter_vtable`（`read`, `close`, `read_header`）、`archive_format_descriptor`（`bid`, `options`, `read_header`, `read_data`, `read_data_skip`, `seek_data`, `cleanup`）。
- **静态槽位数组注册 (Zero Heap Registration)**：`struct archive_read` 内部固定分配 16 个槽位的静态数组（`struct archive_read_filter_bidder bidders[16]` 与 `struct archive_format_descriptor formats[16]`），消除注册与遍历时的动态堆分配。

#### B. 双向流式过滤器流水线与自动竞标 (Bidding Protocol Pipeline)
- **自底向上竞标协商**：
  1. 基础过滤器 `ARCHIVE_FILTER_NONE` 将底层 I/O 回调包装为 `none_reader_vtable`。
  2. `choose_filters` 循环（硬上限 `MAX_NUMBER_FILTERS = 25`）：遍历 `bidders` 调用 `bidder->bid()`，通过 `__archive_read_filter_ahead()` 窥探数据流前部特征字节，返回置信度分数（匹配比特数）。胜出者分配 `struct archive_read_filter` 并建立双向链 `f->upstream = a->filter; a->filter = f;`。
  3. `choose_format` 循环：在已完全解压的平坦数据流前端遍历 `formats[16]` 调用 `format->bid()` 识别容器格式（如 ZIP 魔数 `PK\x03\x04` 得 29 分，TAR 得 48~106 分），最终锁定 `a->format`。
- **正向写流水线**：格式层将编码后的字节块注入 `filter_first`，沿 `f->next_filter` 链式下压压缩并最终由 `client_writer` 写出。

#### C. 微缓冲 (Micro-Buffering) 与零拷贝指针直通
- **Lookahead 与 Consume 正交解耦**：
  - `__archive_read_ahead(a, min, &avail)`：保证返回至少 `min` 字节的连续指针，在 `*avail` 中返回实际连续可用字节（通常远大于 `min`），**绝不移动流指针**。
  - `__archive_read_consume(a, request)`：显式推进流指针，支持按需部分消费。
- **Fast-Path 与 Slow-Path 动态切换**：
  - **Fast-Path (零拷贝直通)**：当 Client/Filter 缓冲内剩余数据足以满足 `min` 时，直接返回 `f->client_next` 指针，指针推进仅修改偏移，零 `malloc`、零 `memcpy`。
  - **Slow-Path (跨块微缓冲拼接)**：仅在请求跨越 I/O 块边界时，启用 `f->buffer`，使用 `memmove` 迁移碎片并通过 `archive_ckd_mul_size(&s, s, 2)` 翻倍安全扩容。
- **上层零拷贝数据透传 (`archive_read_data_block`)**：公开 API 直接将解码缓冲区的地址 `const void **buff` 传递给调用者，避免二次内存拷贝。

#### D. 位掩码状态机与单调错误传播
- **位掩码状态 (`a->state`)**：`ARCHIVE_STATE_NEW` (0x01) ➔ `OPEN` (0x02) ➔ `HEADER` (0x04) ➔ `DATA` (0x08) / `DATA_RECOVERY` (0x10) ➔ `EOF` (0x20) ➔ `CLOSED` (0x40) ➔ `FATAL` (0x8000)。
- **入口哨兵 (`archive_check_magic`)**：所有 API 入口处执行 1 次位运算断言；若指针非法或魔数不匹配，立即以底层 `write(2, ...)` 打印诊断并调用 `abort()`；若状态不匹配，硬迁移至 `FATAL`。
- **分级错误码与合并原则**：`ARCHIVE_EOF (1)`, `ARCHIVE_OK (0)`, `ARCHIVE_RETRY (-10)`, `ARCHIVE_WARN (-20)`, `ARCHIVE_FAILED (-25)`, `ARCHIVE_FATAL (-30)`。宏 `#define err_combine(a, b) ((a) < (b) ? (a) : (b))` 确保高严重度错误绝不被轻微错误覆盖。

### 2. 方案决策、论证与替代方案

- **Decision**: 在底层引擎架构设计中，采用“结构体首成员单根继承 + 统一虚表 + 竞标过滤器流水线 + 微缓冲 Lookahead/Consume 解耦”模型。
- **Rationale**: 零运行时开销，ABI 极其稳定；动态竞标天然支持任意多层嵌套过滤器与自动格式识别；微缓冲在热路径下提供 100% 零拷贝直通。
- **Alternatives Considered**: 
  - *GObject 动态类型系统或哈希分派表*：否决。带来繁重的堆分配、引用计数和动态查找开销，严重损害解压缩吞吐。
  - *固定 Ring-Buffer 循环缓冲区*：否决。跨环绕点时无法提供平坦连续内存指针，迫使所有读取均退化为内存拷贝。
- **Source**: 
  - `Vendor/libarchive-upstream/libarchive/archive_private.h:100-140`
  - `Vendor/libarchive-upstream/libarchive/archive_virtual.c:32-150`
  - `Vendor/libarchive-upstream/libarchive/archive_read.c:575-627, 743-790, 1330-1555`
  - `Vendor/libarchive-upstream/libarchive/archive_read_private.h:88-116`

---

<a id="r002"></a>
## R002: libarchive 防御性编程与安全漏洞免疫模型

### 1. 核心研究结论

#### A. 路径清洗与逐级符号链接穿透防御 (Zip Slip & Symlink Traversal)
- **无分配路径规范化 (`cleanup_pathname_fsobj`)**：原地单遍扫描消除连续斜杠（`//` ➔ `/`）与当前段（`./` ➔ 清除）；在 `ARCHIVE_EXTRACT_SECURE_NODOTDOT` 模式下直接阻断 `..` 段；严禁静默抹除 `..`。
- **AT-API 逐级符号链接校验 (`check_symlinks_fsobj`)**：
  - 基于 `chdir_fd = la_opendirat(AT_FDCWD, ".")` 与 `fstatat(..., AT_SYMLINK_NOFOLLOW)` 进行逐级路径探测。
  - 若路径中间段存在符号链接，立即抛出 `ELOOP`（"Cannot extract through symlink"）并返回 `ARCHIVE_FAILED`，彻底免疫通过软链接跳出解压根目录的攻击。
  - 若末尾为现有符号链接且被覆写，先执行 `unlinkat` 删除软链接，防止写入目标文件。
- **延后 Fixup 倒序回写与 TOCTOU 防御**：
  - 目录创建时先赋予临时最小权限（`0700`），将最终只读权限、ACL、mtime 存入 `fixup_entry` 链表。
  - 解压结束前调用 `sort_dir_list`（归并排序）将目录按**深度从深到浅倒序回写**。
  - 回写前使用 `open(..., O_NOFOLLOW | O_DIRECTORY)` 打开句柄并执行 `la_verify_filetype` 校验，防止在解压并发过程中目录被攻击者替换为符号链接。

#### B. 跨架构整型安全与算术溢出防护 (`archive_integer.h`)
- **硬件与编译器加速防溢出算术**：
  - 封装 `archive_ckd_add_size`、`archive_ckd_mul_size`、`archive_ckd_add_i64` 等。
  - 优先调用 C23 `<stdckdint.h>` 或 Clang/GCC `__builtin_add_overflow` / `__builtin_mul_overflow`，由 CPU 溢出标志位硬件级捕获，零分支预测惩罚。
- **64 位偏移向 `size_t` Clamp 截断保护**：
  - 所有 `int64_t`/`off_t` 向 `size_t` 转换前，显式执行 Clamp（如 `requested = buf_len > SSIZE_MAX ? SSIZE_MAX : buf_len;`），并在流式消费中通过 `minimum(request, (int64_t)f->avail)` 防御负数转无符号产生的超大回绕。

#### C. 解压炸弹 (Zip Bomb) 与畸形流熔断机制
- **过滤器深度硬熔断**：`choose_filters` 遍历层数超过 `MAX_NUMBER_FILTERS = 25` 时直接判 `ARCHIVE_FATAL`，阻断 quine 自解压死循环与深度嵌套炸弹。
- **RAR5 窗口上限约束**：解压缩窗口严格限制在 $\le 64\text{MB}$（`window_size > 64 * 1024 * 1024` 即拒收），防止畸形 Header 声明数十 GB 窗口导致 OOM。
- **7-Zip 元数据内存与流大小一致性校验 (`files_info_numfiles_is_sane`)**：在为条目分配数组前，断言 `zip->numFiles <= SIZE_MAX / sizeof(*zip->entries)` 且其所需位图大小不超过归档头部剩余字节数，防止几 KB 小文件声明上亿条目耗尽宿主内存。
- **密码死循环熔断**：解密重试超过 10,000 次强制中断；流消费遇负数或非预期截断立即硬错误返回。

#### D. 魔数校验、双重释放与凭据擦除
- **句柄魔数物理隔离**：`ARCHIVE_READ_MAGIC (0xdeb0c5U)`, `ARCHIVE_WRITE_MAGIC (0xb0c5c0deU)` 等。
- **魔数主动清零 (Magic Invalidation)**：析构函数在调用 `free(a)` 之前，强制执行 `a->archive.magic = 0; __archive_clean(&a->archive);`，使得任何 Use-After-Free 或二次释放重入立即被 `archive_check_magic` 捕获。
- **密码安全擦除**：密码释放前必须 `memset(p->passphrase, 0, strlen(p->passphrase))` 清零。

### 2. 方案决策、论证与替代方案

- **Decision**: 确立“AT-API 逐级符号链接防御 + 延后 Fixup 倒序应用 + `__builtin_*_overflow` 安全算术 + 内存分配前置一致性校验 + 析构魔数清零”五维安全矩阵。
- **Rationale**: 纯字符串正则防御无法抵御 TOCTOU 和符号链接劫持；事后内存检测无法挽回 OOM 崩溃；必须在入口、计算、解压和析构全生命周期建立不可穿透的防御屏障。
- **Alternatives Considered**: 
  - *依赖 Swift 高层 `URL.standardizingPath` 进行路径防御*：否决。仅做词法分析，无法感知文件系统已存在的软链接与动态竞争劫持。
  - *使用 `assert(a + b >= a)` 检查有符号整型溢出*：否决。在 C 标准中有符号溢出为 UB，现代优化器会自动移除该断言，导致防御失效。
- **Source**: 
  - `Vendor/libarchive-upstream/libarchive/archive_write_disk_posix.c:2560-2614, 2704-2750, 2822-3340`
  - `Vendor/libarchive-upstream/libarchive/archive_integer.h:1-268`
  - `Vendor/libarchive-upstream/libarchive/archive_read_support_format_7zip.c:2778-2790`
  - `Vendor/libarchive-upstream/libarchive/archive_check_magic.c:50-183`

---

<a id="r003"></a>
## R003: libarchive 黄金预言机测试与质量验证哲学

### 1. 核心研究结论

#### A. 零依赖轻量级测试框架 (Standalone Harness)
- **两阶段宏元编程注册 (Two-Phase Metaprogramming)**：
  通过 `test_main.c` 两次包含 `list.h`：第一阶段生成测试函数声明 `void test_name(void);`，第二阶段生成函数指针数组 `struct test_list_t tests[]`，零外部测试框架依赖（不依赖 GTest、Catch2）。
- **物理沙盒隔离与纯净化**：
  每个用例在独立目录 `tmpdir/test_name` 执行；强制重置 `LC_ALL="C"` 与 `LANG="C"`；捕获并恢复 `umask`；支持通过 `seteuid` 降权为 `nobody` 测试非特权边界。
- **上下文级联断言**：`failure("Processing entry %d: %s", i, name)` 暂存上下文；断言失败时自动合并输出；`assertEqualIntA` 自动提取底层 `archive_error_string(a)` 与 `archive_errno(a)`。

#### B. 真实历史缺陷黄金语料库与 UUEncode 机制 (Golden Oracle Corpus)
- **ASCII 安全持久化**：将 20 余年积累的历史 CVE、Crash 样本与跨工具兼容用例（GNU tar base256, Zip length-at-end, RAR5 指针泄漏, Zip64 4GiB 边界等）编码为纯 ASCII `.uu` 文本文件入库，由 `test_main.c` 内置微型解码器动态还原。既避免 Git 二进制膨胀与 CRLF 损坏，又构建了绝对客观的回归黄金预言机。
- **双向跨生态差分测试**：建立针对 GNU tar, Info-ZIP, BSD pax, xorriso 的差分校验；压缩输出通过系统原生工具（如 `gzip -d`）反向解压并校验 Header 字节与压缩等级单调性。

#### C. 变异模糊测试与崩溃现场优先转储 (`test_fuzz.c`)
- **轻量级变异算法**：对有效归档注入 ~1% 的伪随机单字节变异。
- **崩溃优先落盘 (Crash-First Disk Persistence)**：在调用解压解析器前，**先将变异后二进制数据落盘为明确命名的调试文件**（如 `after.test.failure.send.this.file...`）。一旦 C 引擎触发 SIGSEGV 或 ASan 报错，该文件即为现成的最小复现用例（Reproducer）。
- **双模式消费验证**：Pass 1 遍历 Header + 全解压 Body；Pass 2 遍历 Header + Skip Body，测试状态机容错与快进跳跃能力。

### 2. 方案决策、论证与替代方案

- **Decision**: 引入“UUEncoded 黄金缺陷语料库 + In-Process 变异模糊测试 + 双向跨工具差分校验 + 崩溃优先落盘”测试体系。
- **Rationale**: 真实历史缺陷是防范回归的最强屏障；变异测试与崩溃预转储能在 CI 中秒级捕获野指针与死循环；差分校验确保与生态工具 100% 互操作。
- **Alternatives Considered**: 
  - *仅依赖 Swift XCTest 封装业务测试*：否决。无法深入底层 C 桥接层的数据结构与内存边界进行白盒验证。
  - *纯随机内存模糊测试 (Raw Fuzz)*：否决。绝大多数输入在 Magic 校验即被拒绝，无法触达深度解析逻辑。
- **Source**: 
  - `Vendor/libarchive-upstream/test_utils/test_main.c:468-483, 3228-3288, 3548-3565, 3653-3798`
  - `Vendor/libarchive-upstream/test_utils/test_common.h:155-270`
  - `Vendor/libarchive-upstream/libarchive/test/test_fuzz.c:27-44, 151-217`
  - `Vendor/libarchive-upstream/libarchive/test/test_compat_zip.c:28-144`

---

<a id="r004"></a>
## R004: Upstream 治理标准与 Prompt/Skills/Workflows 升级映射

### 1. 核心研究结论

#### A. Upstream 社区工程纪律与贡献标准
- **原子提交与 Bisectability 保证**：严禁混杂的大 Commit；每个 Commit 必须全平台独立可编译；严格执行 `[infra]` ➔ `[feat]` ➔ `[test]` 三段式提交。
- **BSD KNF 规范与零无关污染**：使用 8 字符 Hard Tab，K&R 括号，C89 变量置顶声明；严禁夹带无关格式化 diff。
- **mdoc Man Page 与多格式派生**：任何 API/CLI 变更必须同步修改 `libarchive/*.3`, `*.5` 或 `tar/*.1`，并通过 `sh doc/update.sh` 派生 HTML/TXT/PDF/Wiki。
- **双构建系统严格同步**：`Makefile.am` 与 `CMakeLists.txt` 必须按**严格 ASCII 字母序**注册新测试 `.c` 与 `.uu` 样本。

#### B. 多层物理架构与依赖隔离蓝图 (Multi-Tier Layout)
- **Layer 0 (Pristine Upstream)**: `Vendor/libarchive-upstream/` 保持 100% 官方 upstream 纯净结构，零 TTZip/Swift 代码污染。所有 upstream patch 通过 `Vendor/worktrees/` 隔离开发。
- **Layer 1 (C Bridge & SIMD Layer)**: `Sources/CTTZipBridge/` 仅暴露 `CTTZipBridge.h` 门面头文件，封装 NEON SIMD、无锁 Ring Buffer 与 POSIX 适配器，禁止泄露 Vendor 内部私有头文件。
- **Layer 2 (Swift Core Engine)**: `Sources/TTZipCore/` 封装并发流式管道、设计模式、密码库与安全校验，向外暴露统一高层 Facade。
- **Layer 3 (App & CLI Layer)**: `Sources/TTZipApp/` (@MainActor MVVM) 与 `Sources/TTZipCLI/`，严禁越级直接调用 C 桥接层。

#### C. Prompts、Rules 与 Agent Skills 体系全面升级方案
1. **`GEMINI.md` 与全局 Rules 升级**：新增《系统级防御性编程与流式架构铁律》（整型 Narrowing Clamp、流式非满读断言、状态单调不可逆、双构建系统与文档同步）。
2. **`code-review` Skill 升级**：增加 Apple Silicon 16KB 页对齐与 SIMD 尾部越界防护、热路径零中间分配断言、状态机 Cleanup 幂等性、Swift 6 跨语言生命周期 4 大硬性审查维度。
3. **`upstream-contribution` Skill 强化**：注入 mdoc man page 同步 SOP、`Makefile.am` 字母序合规检查、Bidding Protocol 竞标契约审查与 BSD KNF 格式检查。
4. **`design-patterns-guide` Skill 扩充**：新增“流式管道竞标模式 (Streaming Bidder-Filter-Format Pipeline)”、“无锁环形缓冲区适配器模式 (Lock-Free SPSC Ring Buffer Adapter)”与“硬件向量动态分发模式 (Dynamic Capability Dispatch)”。

### 2. 方案决策、论证与替代方案

- **Decision**: 实施 4 层代码组织物理隔离，并将 libarchive 的工业级标准全面转化为 Prompt 铁律、硬性 Code Review 清单、Upstream 贡献 SOP 及底层流式设计模式。
- **Rationale**: 使工业级标准从“文档知识”转化为 AI 代理与工程流水线的“反射性执行约束”与“物理隔离防线”。
- **Alternatives Considered**: 
  - *直接将 libarchive 源码混入 CTTZipBridge 编译*：否决。导致 upstream 版本升级和向官方提交 PR 成为不可维护的灾难。
- **Source**: 
  - `Vendor/libarchive-upstream/CONTRIBUTING.md:83-99`
  - `Vendor/libarchive-upstream/doc/WRITING_TESTS.md:108-125, 238-243`
  - `Vendor/libarchive-upstream/doc/update.sh:1-122`
  - `Package.swift:34-124`
  - `ARCHITECTURE.md:5-57`
  - `.agents/skills/upstream-contribution/SKILL.md:1-97`
  - `.agents/skills/code-review/SKILL.md:90-114`
  - `.agents/skills/design-patterns-guide/SKILL.md:22-56`

---

<a id="decision-matrix"></a>
## 综合决策矩阵 (Consolidated Decision Matrix)

| 编号 | 核心决策 (Decision) | 选择理由 (Rationale) | 否决方案及理由 (Alternatives Considered) | 查阅源码依据 (Source) |
| :--- | :--- | :--- | :--- | :--- |
| **D01** | **纯 C 面向对象：首成员继承 + 统一虚表 + 竞标流水线** | 零开销多态，ABI 稳定，支持任意多层嵌套过滤器自动探测与平权容器识别。 | **GObject 动态类型系统**：堆开销大、运行时性能损耗严重。 | `archive_private.h:100-140`<br>`archive_read.c:575-627` |
| **D02** | **微缓冲模型：Lookahead 与 Consume 正交解耦** | 块边界内 100% 零拷贝直通，跨块动态微缓冲拼接，支持 GB 级大文件流式解压。 | **固定 Ring-Buffer**：跨回绕点无法提供平坦连续指针。 | `archive_read.c:1330-1555`<br>`archive_read_private.h:88-116` |
| **D03** | **AT-API 逐级符号链接防御与延后 Fixup 倒序回写** | 彻底免疫 Zip Slip、符号链接沙盒逃逸与 TOCTOU 权限竞争劫持。 | **高层 URL.standardizingPath**：纯词法分析，无法感知文件系统动态符号链接。 | `archive_write_disk_posix.c:2560-2614, 2822-3340` |
| **D04** | **硬件防溢出算术与 64-bit/`size_t` Clamp 截断保护** | 硬件级零分支预测开销捕获整型溢出，杜绝跨架构缓冲区回绕与堆破坏。 | **`assert(a+b>=a)` 事后断言**：C 标准中属 UB，会被现代优化器直接移除。 | `archive_integer.h:1-268` |
| **D05** | **格式元数据前置一致性校验与炸弹熔断** | 限制过滤器深度 $\le 25$、限制解压窗口 $\le 64\text{MB}$、条目数与剩余流字节交叉校验，消除 OOM 崩溃。 | **仅依赖系统 OOM Killer**：导致宿主进程崩溃退出，损坏用户数据。 | `archive_read.c:572-627`<br>`archive_read_support_format_7zip.c:2778-2790` |
| **D06** | **魔数校验哨兵与析构魔数清零 (Magic Invalidation)** | 入口单次位运算拦截乱序调用与野指针，析构清零彻底防御 Use-After-Free。 | **隐式布尔标记**：野指针和悬垂指针无法被有效拦截。 | `archive_check_magic.c:50-183`<br>`archive_read.c:1180-1189` |
| **D07** | **UUEncode 黄金缺陷语料库与崩溃优先落盘 Fuzzer** | 20 年真实 CVE/Crash 样本 ASCII 持久化回归，变异测试在崩溃前预存用例。 | **纯随机内存 Fuzzer**：极浅测试，无法触达内部深度状态机。 | `test_main.c:3228-3288`<br>`test_fuzz.c:27-44, 151-217` |
| **D08** | **4 层物理架构隔离与 Upstream 纯净分支机制** | 确保 Upstream 开源贡献零污染，同时使 Swift 6 严格并发与 MAS 构建拓扑清晰。 | **源码直接混入 CTTZipBridge**：升级与向上游提 PR 演变为维护灾难。 | `Package.swift:34-124`<br>`ARCHITECTURE.md:5-57` |
