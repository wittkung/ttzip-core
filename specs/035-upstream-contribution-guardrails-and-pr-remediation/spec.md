# Feature Specification: 上游开源贡献质量规范体系与 3 个 PR 严谨重构交付

**Feature Directory**: `specs/035-upstream-contribution-guardrails-and-pr-remediation`  
**Feature Branch**: `035-upstream-contribution-guardrails-and-pr-remediation`  
**Created**: 2026-08-16  
**Status**: Revised (Incorporating Upstream Architectural & Safety Invariants)  
**Input**: 用户请求建立完备的 Code Review 与上游贡献质量规范体系，并对 PR #3388、PR #3391、PR #3393 进行彻底反思、代码修复、Git Worktree 物理隔离、原子 Commit 拆分、流式前向推进与安全内存边界防御。

---

## 1. User Scenarios & Testing *(mandatory)*

### User Story 1 - 建立全局防御性 C 审查与上游贡献硬门禁 (Priority: P1)

作为开源项目贡献者与架构师，我需要一套覆盖“系统级 C/POSIX 跨平台防御性编程”、“Git 纯净分支物理隔离”与“原子 Commit 编排”的标准规范与审查 Checklist，以确保每一次推向上游的代码具备最高级别的稳健性与可移植性，杜绝任何低级工程事故。

**Why this priority**: 规范先行。这是解决当前问题并防止未来所有上游贡献再次发生质量事故的根本前提。

**Independent Test**:
- 检查 `/Users/kevintung/.agents/skills/code-review/SKILL.md` 包含系统级 C 防御性编程审查章节（32/64 位溢出、流式 I/O NULL 防御、返回值捕获、前向推进死循环防护）。
- 检查 `/Users/kevintung/.agents/skills/upstream-contribution/SKILL.md` 完整落地（涵盖分支隔离断言、Git Worktree 物理隔离 SOP、原子 Commit 拆分、原生 Oracle 对齐、社区沟通纪律）。
- 检查 `GEMINI.md` 注入了上游贡献审查的硬门禁。

**Acceptance Scenarios**:
1. **Given** 任何 C/POSIX 代码修改，**When** 涉及 `int64_t`/`off_t` 转 `size_t`，**Then** Checklist 强制断言存在上限 Clamp 保护。
2. **Given** 准备向上游提交 PR，**When** 执行提交前检查，**Then** 必须通过物理断言验证 `git diff upstream/master..HEAD` 零无关文件污染，并通过 `git rebase --exec` 验证每个原子 Commit 独立可编译。

---

### User Story 2 - PR #3388 (7z AES-256 解密) 深度防御修复与原子 Commit 重构 (Priority: P1)

作为 libarchive 的贡献者，我需要对 PR #3388 进行彻底的代码加固与结构重构，消除 @stoeckmann 提出的 5 大技术缺陷，严格遵守流式 I/O 状态机前向推进与零内存泄漏原则，并将原有单一大 commit 拆分为符合上游标准的原子 commit 序列。

**Why this priority**: 7z AES 解密是闭环 7z 核心能力的关键基础设施，必须率先达到工业级可合并标准。

**Independent Test**:
- 运行 32 位与 64 位编译检查，验证 `to_read` 截断防护无警告。
- 验证 `__archive_read_ahead` 在请求 16 字节时对 `buff_in == NULL` 及 `bytes_avail < 16` 具备完全防御。
- 验证 `__archive_read_consume` 错误被完整捕获并向上传播。
- 验证 `archive_read_format_7zip_cleanup` 彻底释放 `decrypted_buffer` 并重置 `crypto_ctx` 与密钥内存（零内存泄漏与内存擦除安全）。
- 分支历史呈现清晰的 3 个独立原子 Commit（1. 加密接口基础设施 -> 2. 7z 解密流水线 -> 3. 测试与构建系统注册）。

**Acceptance Scenarios**:
1. **Given** 加密 7z 流读取，**When** 可用字节少于请求的 16 字节或流损坏，**Then** 代码正确报错并安全返回 `ARCHIVE_FATAL`，不发生空指针解引用。
2. **Given** 超过 4GB 的加密数据流，**When** 在 32 位平台上运行，**Then** 分块读取被严格限制在 `UBUFF_SIZE` 内，无整型溢出。

---

### User Story 3 - PR #3391 (CRC32 硬件加速) 纯净分支重建与测试预言机对齐 (Priority: P1)

作为 libarchive 的贡献者，我需要从 clean upstream `master` 完全重建 `armv8-crc32-acceleration` 分支，100% 剥离任何无关代码；并将测试套件全面改造为使用 libarchive 官方黄金预言机 `bitcrc32()`，同时准备针对 kientzle 顾虑的建设性价值与无维护负担阐述。

**Why this priority**: 纠正被创始人抓包的分支污染事故，重建项目信任，并对齐官方测试哲学。

**Independent Test**:
- `git diff origin/master..armv8-crc32-acceleration --stat` 仅包含 4 个 CRC32 相关文件（`archive_crc32.h`、`test_archive_crc32.c`、`Makefile.am`、`CMakeLists.txt`）。
- `test_archive_crc32.c` 中零硬编码常量，全部调用 `bitcrc32()` 进行位级交叉校验。
- `cmake -DENABLE_ZLIB=OFF -DENABLE_TEST=ON` 构建下所有测试 100% 通过。

**Acceptance Scenarios**:
1. **Given** 干净检出的 PR #3391 分支，**When** 检查 Git 变更树，**Then** 零 7z 代码泄露。
2. **Given** 5 组 CRC32 测试用例，**When** 运行测试，**Then** 每一组输出均与 `bitcrc32()` 黄金预言机逐字节一致。

---

### User Story 4 - PR #3393 (磁盘预分配) 技术答辩与设计演进 (Priority: P2)

作为 libarchive 的贡献者，我需要深入分析 kientzle 提出的关于“稀疏文件检测”和“小文件自动跳过 vs Opt-in Flag”的技术疑问，在 Issue #3392 中提供详尽的技术事实分析和代码依据，并在 PR #3393 保持 Draft 状态下等待 Maintainer 达成设计共识。

**Why this priority**: 遵循开源社区决策流程，尊重 Maintainer 架构意见，以技术事实推动方案共识。

**Independent Test**:
- 形成完整的技术答辩文档，清晰解答 `archive_write_disk_posix.c` 中 `ARCHIVE_EXTRACT_SPARSE` 动态写扫描打洞机制与预分配的互斥原理。
- 给出小文件自动跳过阈值（如 `< 64KB`）结合 `ARCHIVE_EXTRACT_PREALLOCATE` 的设计方案。

**Acceptance Scenarios**:
1. **Given** Maintainer 对稀疏文件与小文件开销的疑问，**When** 提交技术分析回复，**Then** 给出基于内核行为与 libarchive 内部状态机的精确解释。

---

### User Story 5 - Git Worktree 物理隔离与多 PR 并行工作流 (Priority: P1)

作为多 PR 贡献者，我需要将所有 PR 分支检出为磁盘上相互独立的物理工作区（`Vendor/worktrees/pr-*`），使每个 PR 的 CMake 构建缓存、临时测试对象文件和 Git 索引彻底物理隔离，杜绝交叉编译污染与分支链污染。

**Why this priority**: 根治多分支频繁切换带来的缓存污染与派生链污染。

**Independent Test**:
- 验证 `Vendor/worktrees/pr-3388-7z-aes`、`Vendor/worktrees/pr-3391-crc32`、`Vendor/worktrees/pr-3393-preallocate` 为独立物理目录。
- 验证 `Vendor/worktrees/` 在主仓库 `.gitignore` 中被忽略，零脏文件追踪。
- 验证在各自 worktree 下独立执行 `cmake` + `make libarchive_test` 均能独立成功。

---

## 2. Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 必须在全局 `/Users/kevintung/.agents/skills/code-review/SKILL.md` 中新增《系统级 C / 跨平台防御性编程审查规范》，强制约束跨架构整型转换、流式 I/O 指针安全、前向推进与死循环熔断、副作用函数返回值检查。
- **FR-002**: 必须创建专用 Skill `/Users/kevintung/.agents/skills/upstream-contribution/SKILL.md`，规范化开源上游 PR 贡献全生命周期 SOP 与 Git Worktree 物理隔离操作指南。
- **FR-003**: 必须在 `TTZip/GEMINI.md` 中引用上游贡献审查门禁。
- **FR-004**: PR #3388 必须修复 32 位整型截断风险（将 `to_read` 限制在 `UBUFF_SIZE`）。
- **FR-005**: PR #3388 必须在 `__archive_read_ahead` 返回 `NULL` 或可用字节不足时进行显式判空与错误返回。
- **FR-006**: PR #3388 必须对 `__archive_read_consume` 的返回值进行显式错误检查，保证每次推进字节数有效。
- **FR-007**: PR #3388 必须将 `struct _7z_crypto_properties` 移动到文件顶部结构体声明区域。
- **FR-008**: PR #3388 必须拆分为 3 个逻辑独立的原子 Commit（基础设施、解密流水线、测试用例），且每个 Commit 均可独立编译通过。
- **FR-009**: PR #3388 必须在清理函数 `archive_read_format_7zip_cleanup` 中完整释放 `decrypted_buffer` 并重置 `crypto_ctx`，杜绝任何错误路径或正常退出时的内存泄漏。
- **FR-010**: PR #3391 必须基于 upstream `master`（`22e3e20`）彻底重建干净分支，严禁包含任何无关文件的修改。
- **FR-011**: PR #3391 测试套件必须使用 `test_utils.h` 中的 `bitcrc32()` 黄金预言机，严禁使用外部硬编码常量。
- **FR-012**: 必须建立 `Vendor/worktrees/` 物理工作区隔离机制，每个 PR 分支独占一个物理目录，且在根 `.gitignore` 中注册。
- **FR-013**: 所有 PR 在推送前必须通过本地 C89 严格编译、CMake 构建以及全量单测验证。

---

## 3. Success Criteria *(mandatory)*

- **SC-001**: 全局 `code-review` 与 `upstream-contribution` 规范落地并在本地磁盘物理存在。
- **SC-002**: PR #3388 分支 `feat/7z-aes256-decryption` 包含清晰的 3 个原子 commit 序列，5 项 Review 问题 100% 彻底修复，并在独立 Worktree 下通过所有加密自动化测试。
- **SC-003**: PR #3391 分支 `armv8-crc32-acceleration` 仅修改 4 个相关文件，`git diff origin/master..HEAD` 零污染，并在独立 Worktree 下通过 `bitcrc32()` 100% 验证。
- **SC-004**: 针对 Reviewer 的回复文案专业、谦逊、详实，严格对齐开源社区协作礼仪。
- **SC-005**: 物理 Worktree 隔离机制搭建完毕，所有 PR 拥有独立 build 缓存与目录，根代码库零脏文件追踪。

---

## Clarifications

### Session 2026-08-16 (Clarification Round 1)

- **Q1 (Git 物理隔离策略)**: 多 PR 并行开发时如何彻底杜绝 CMake 缓存与中间对象文件泄漏？
  - **A1**: 采用 `git worktree` 为每个 PR 分支分配独立物理目录（`Vendor/worktrees/pr-*`），根仓库忽略该目录，提交前必须执行 `git diff origin/master..HEAD --stat` 物理断言。
- **Q2 (32 位整型溢出防御)**: `int64_t` 的 `pack_stream_inbytes_remaining` 转换为 `size_t` 时如何保证 32 位平台安全？
  - **A2**: 首先在 64 位无符号空间与分块上限 `UBUFF_SIZE` 比较 clamp，再转换为 `size_t`，杜绝整型截断。
- **Q3 (测试预言机对齐)**: CRC32 测试如何避免硬编码常量的脆弱性？
  - **A3**: 统一使用 `test_utils.h` 中的 `bitcrc32()` 慢速但绝对正确的位级黄金预言机，并在 64KB+ 大缓冲区与单字节、非对齐多维度交叉验证。
- **Q4 (稀疏文件与预分配冲突)**: 磁盘预分配是否应与动态稀疏化共存？
  - **A4**: 互斥。当启用 `ARCHIVE_EXTRACT_SPARSE` 或文件小于 64KB 时，自动旁路预分配，防止占满物理扇区破坏打洞并避免小文件系统调用风暴。

