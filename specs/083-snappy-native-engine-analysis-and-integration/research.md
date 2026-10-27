# Phase 0 Research: Google Snappy 原生引擎深度调研与架构设计 (083-snappy-native-engine-analysis-and-integration)

**Feature Branch**: `083-snappy-native-engine-analysis-and-integration`  
**Created**: 2026-08-18  
**Status**: Completed  
**Feature Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/083-snappy-native-engine-analysis-and-integration/spec.md)

---

## 1. R001: Google Snappy 源码架构与 C 桥接中枢设计

### Decision (选定方案)
以源码级子目录静态嵌入（In-Process Static Embedding）方式，将 Google Snappy C++17 核心源文件嵌入 `Sources/CTTZipBridge/snappy/`，并通过 `include/CTTZipBridge_Snappy.h` 对外导出纯 C11 线程安全无锁桥接接口（`ttzip_snappy_compress`、`ttzip_snappy_decompress`、`ttzip_snappy_max_compressed_length`、`ttzip_snappy_uncompressed_length`、`ttzip_snappy_validate`）。

### Rationale (选择理由)
1. **架构一致性与 100% 自包含**：与 TTZip 既有的 `Sources/CTTZipBridge/fast-lzma2/` 和 `Sources/CTTZipBridge/lzfse/` 嵌入模式 100% 一致。彻底摆脱外部 CMake / Homebrew 动态库依赖，确保在 Mac App Store（`-DMAS_BUILD`）沙盒与 Direct 独立分发环境下构建与运行的一致性。
2. **L1 缓存亲和与硬件加速**：针对 64KB 块大小，Snappy 哈希表仅需 $2^{14} \times 2\text{B} = \mathbf{32\text{ KB}}$，可 100% 驻留于 Apple Silicon / ARM64 / x86_64 CPU 的 L1 Data Cache（64KB~128KB），实现热循环零 L2/L3 未命中。SWAR 宽字异或（`UNALIGNED_LOAD64` ^ `__builtin_ctzll`）在 Apple Silicon 上映射为 `rbit` + `clz` 硬件单周期指令，单指令结算 8 字节等值长度。
3. **零堆分配与纯无状态**：解压过程 0 堆分配，编解码函数均为纯函数（Pure Functions），无全局锁与共享可变状态，天然支持 GCD（`DispatchQueue.concurrentPerform`）与 Swift Task 多核并发。

### Alternatives Considered (被否决方案)
- **被否决方案：通过外部动态库（`brew install snappy` / `-lsnappy`）或外部 CMake 预编译 `.a` 静态库引入**
  - **否决理由**：违反 TTZip “100% 独立自包含零外部依赖” 构建原则；预编译库在 MAS 沙盒打包时易出现 Universal 架构切片缺失与符号冲突；外部构建无法与 Swift PM 的 `-O3` 与 LTO 优化协同。

### Source (查阅来源)
- `google/snappy` 官方开源仓库源码：`https://github.com/google/snappy` (`snappy.h`, `snappy-c.h`, `snappy.cc`, `snappy-internal.h`, `snappy-stubs-internal.h`, `format_description.txt`)
- TTZip 现有工程配置：`Package.swift`（第 32–50 行 `CTTZipBridge` 目标配置与 `libc++` 链接设置）
- TTZip 桥接规范：`Sources/CTTZipBridge/include/CTTZipBridge_LZFSE.h`、`Sources/CTTZipBridge/fast-lzma2/`

---

## 2. R002: Snappy 官方 Framing Format 规范与 Apple Silicon ARM64 CRC32C 硬件加速

### Decision (选定方案)
构建完全遵循 Google Snappy 官方 Framing Format 规范（`framing_format.txt` / `.sz` / `.tar.sz`）的流式分块状态机，结合 Apple Silicon ARM64 ACLE 原生硬件指令（`__builtin_arm_crc32cd` 4 路展开）加速 Castagnoli CRC32C 计算，并配套 Slice-by-8（8KB 查表）软件降级回退。

1. **Framing 帧结构定义**：
   - Stream Identifier：起始 10 字节 `0xff 0x06 0x00 0x00 "sNaPpY"`。
   - Chunk Header：4 字节（1 字节类型 + 3 字节小端长度）。
   - Compressed Chunk (`0x00`)：4 字节 Masked CRC32C + 原始 Snappy 块（解压后数据硬约束 $\le 64\text{ KB}$）。
   - Uncompressed Chunk (`0x01`)：4 字节 Masked CRC32C + 原始字节。
   - Padding (`0xfe`) 与 Skippable (`0x80`~`0xfd`)：安全跳过载荷。
   - Masked CRC32C 计算与逆变换：
     $$\text{Masked}(C) = ((C \gg 15) \mid (C \ll 17)) + \text{0xa282ead8U}$$
     $$\text{Unmasked}(M) = ((M - \text{0xa282ead8U}) \gg 17) \mid ((M - \text{0xa282ead8U}) \ll 15)$$
2. **三级自适应 CRC32C 体系**：
   - 硬件直通：`<arm_acle.h>` 的 `__builtin_arm_crc32cd`（4-way unrolled），单核吞吐 $> 25\text{ GB/s}$。
   - 动态探测：`sysctlbyname("hw.optional.arm.FEAT_CRC32", ...)` 严格布尔绑定系统调用。
   - 软件降级：Slice-by-8 查表法（8 张 256 表，8KB 缓存占用），吞吐 $\ge 2.5\sim 3.5\text{ GB/s}$。

### Rationale (选择理由)
1. **标准化互操作性**：Snappy Framing Format 是业界标准，支持流式分块、校验与文件拼接。
2. **消除校验瓶颈**：ARM64 硬件指令在 64KB 块上仅耗时 $< 2\ \mu\text{s}$，使 CRC32C 计算开销趋近于零，确保全速流式吞吐。

### Alternatives Considered (被否决方案)
- **被否决方案 1：裸 Snappy Block 格式 (Raw Bitstream)**
  - **否决理由**：缺乏流魔数、分块边界与 CRC 校验，网络流或大文件解压时无法断点检验，单比特翻转可能导致不可逆越界。
- **被否决方案 2：误用 IEEE 802.3 CRC32 多项式 (`0xEDB88320`)**
  - **否决理由**：Snappy 官方规范要求必须使用 Castagnoli 多项式（$0x1EDC6F41$），多项式不匹配会导致与所有标准生态工具（`snzip` / `libarchive` / Go `snappy`）无法互通。

### Source (查阅来源)
- Google Snappy 官方规范：`https://github.com/google/snappy/blob/main/framing_format.txt`
- ARM Architecture Reference Manual: ARMv8-A CRC Extension (`__builtin_arm_crc32c*`)
- 本地魔数与探测实现：`Sources/TTZipCore/ChainOfResponsibility/ArchiveHeaderMagicHandler.swift` (199–202 行)、`Sources/CTTZipBridge/ttzip_platform_detect.c`

---

## 3. R003: 100% 进程内 TAR.SZ 流式管道与 Libarchive 自定义回调

### Decision (选定方案)
构建 100% 进程内的 TAR.SZ 流式压缩与解压引擎（`ttzip_create_tar_snappy_native_c` 与 `ttzip_extract_tar_snappy_native_c`）：
1. **Libarchive 自定义回调桥接**：
   - 压缩端：调用 `archive_write_open(a, client_data, NULL, ttzip_snappy_write_cb, ttzip_snappy_close_cb)` 接收 libarchive 生成的 PAX TAR 字节流，在 C 回调中通过 64KB 环形缓冲流式封装 Snappy 帧并写入目标文件。
   - 解压端：调用 `archive_read_open2(a, client_data, NULL, ttzip_snappy_read_cb, ttzip_snappy_skip_cb, ttzip_snappy_close_cb)`，流式解析 `.sz` 帧还原为 TAR 字节流并送入 libarchive 解包状态机。
2. **零拷贝直通架构 (`ttzip_tar_snappy_direct.c`)**：
   - 借鉴 `ttzip_tar_zstd_direct.c` 的成熟架构，支持单文件/大批量文件 USTAR 直通，消除中间临时文件。
3. **64 位整型安全截断保护**：
   - 严格使用 `ttzip_clamp_size` 与 `ttzip_clamp_ssize` 进行 `SSIZE_MAX` clamp 保护，防范超大文件跨语言数值截断溢出。

### Rationale (选择理由)
1. **彻底根除子进程派生**：消除原本有缺陷的 `archive_write_add_filter_program(a, "snappy")`，100% 兼容 Mac App Store 沙盒（`-DMAS_BUILD`）与无外部 CLI 的纯净系统。
2. **符合流式第一性铁律**：以 64KB 微块流式传输，内存开销 $< 2\text{ MB}$，零全量内存膨胀。

### Alternatives Considered (被否决方案)
- **被否决方案：向 libarchive upstream 打 Patch 强行注入全局 filter**
  - **否决理由**：增加了维护分支与同步开销；通过 libarchive 原生支持的自定义 I/O 回调，可以在 CTTZipBridge 自身无缝实现且不污染基础静态库。

### Source (查阅来源)
- `Sources/CTTZipBridge/ttzip_tar_native.c`: 211–214 行（原有外部进程缺陷）、231–235 行（`archive_write_open` 回调模式）、308–351 行（`mkdir_cache`）
- `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c`: 31–73 行、492–745 行（直通解包状态机与 mmap 回写）
- `Sources/CTTZipBridge/include/CTTZipCommon.h`: 85–102 行（`ttzip_clamp_size` 与 `ttzip_clamp_ssize`）
- `Vendor/libarchive-upstream/libarchive/archive_write_add_filter_by_name.c`: 44–76 行（查证 libarchive 无内置 snappy filter）

---

## 4. R004: 不可信输入/恶意损坏流内存安全防御与 13 维 Fuzzing 矩阵

### Decision (选定方案)
采用“双层确界防御与快速短路 (Dual-Layer Bounded Defense & Fast Short-Circuit)”内存安全架构，并在 Swift / C 层面配套 13 维逆向变异测试矩阵：
1. **C 热路径边界确界**：
   - 指针单调递增：每次循环强制 $ip$ 前移至少 1 字节，防止死循环。
   - 历史窗口下界防御：解引用前强制断言 $0 < \text{offset} \le (op - op_{\text{base}})$，非法 offset 立即短路返回 `TTZIP_SNAPPY_ERR_OFFSET_OUT_OF_BOUNDS`。
   - Wild Copy 安全边界：向量化 16 字节拷贝仅在 $op + 16 \le op_{\text{limit}} \land \text{offset} \ge 16$ 时执行；末端严格回退到单字节步进拷贝，杜绝堆越界写（Heap OOB Write）与 ASan 报警。
2. **Swift 强类型错误传播**：
   - 建立高颗粒度的 `SnappyError` 枚举（`.invalidMagicHeader`, `.corruptVarint`, `.corruptChunkHeader`, `.offsetOutOfBounds`, `.literalLengthExceeded`, `.crc32cMismatch`, `.unsupportedChunkType`, `.unexpectedEOF`），100% 捕获底层异常。
3. **13 维 Fuzzing 矩阵**：
   - 覆盖 Varint 溢出、Varint 虚标、Literal 欠读/过读、Copy 历史越界、自重叠边界踩踏、Magic 损坏、Chunk 长度超限、CRC32C 篡改、Reserved Chunk 注入、单字节级联截断等全场景。

### Rationale (选择理由)
1. **零信任输入安全原则**：防御来自不可信来源的恶意构造压缩包导致的 RCE / DoS / SIGSEGV。
2. **热路径零开销安全**：一次性 Token 边界判定结合尾部精确 Clamping，既保持 $> 4,500\text{ MB/s}$ 极速解压，又彻底消除越界漏洞。

### Alternatives Considered (被否决方案)
- **被否决方案：解压前执行全量预先两遍扫描 (Two-pass Validation)**
  - **否决理由**：对大文件执行预扫描会导致解压吞吐减半，严重违背性能铁律。单遍流式解压 + 实时边界断言为最优解。

### Source (查阅来源)
- Google Snappy 核心解压实现与测试：`snappy.cc` (`SnappyDecompressor::DecompressAllTags`, `Varint::Parse32WithLimit`)、`snappy_unittest.cc` (`VerifyCorrupted`, `CorruptTest`)
- 本地跳过测试代码：`Tests/TTZipTests/AllFormatsAndAdvancedParametersMatrixTests.swift:464`
