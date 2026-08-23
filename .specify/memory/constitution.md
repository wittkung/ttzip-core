# TTZip Engineering Constitution & Architectural Invariants

> **Status**: Living Document | **Enforcement**: Mandatory across all AI & Human Contributions

---

## 1. Core Architecture & Tech Stack Boundaries
- **Language & Runtime**: Swift 6.0 (`swift-tools-version: 6.0`) + C11 / POSIX APIs.
- **Platform Compatibility**: macOS 14.0+ (Apple Silicon NEON prioritized, Intel x86_64 compatible).
- **Core Engine Strategy**: 100% In-process static C library bindings (`CTTZipBridge` -> `Vendor/*.a`). Zero ad-hoc CLI subprocess execution.
- **Distribution Channels**:
  - Mac App Store (MAS Sandbox): Activated via `-DMAS_BUILD`.
  - Direct Independent Distribution: Sparkle v2.6.0 auto-update, wrapped in `#if !MAS_BUILD`.

---

## 2. Inviolable Performance Invariants (Hot-Path Floors)

### A. Zero-Cost Abstraction on Hot Paths
The following hot paths must maintain **zero intermediate heap allocations, zero redundant syscalls, and zero dynamic object tree instantiations**:
- `Sources/TTZipCore/Zip/` (All parallel compress / decompress / stream writers)
- `Sources/CTTZipBridge/CTTZipExtract.c`
- `Sources/CTTZipBridge/CTTZipBridge_Crypto.c` (AES-256 SIMD / NEON)
- `Sources/CTTZipBridge/CTTZipBridge_ZipWrite*.c`
- `Sources/CTTZipBridge/ttzip_lzma2_*.c`
- `Sources/TTZipCore/Zip/ZipDirectoryScanner.swift`

### B. Prohibited Hot-Path Anti-Patterns
- **No Shared Locks**: Never call `NSLock`, `DispatchSemaphore`, or `pthread_mutex` inside `DispatchQueue.concurrentPerform` or GCD parallel closures.
- **No Kernel Zeroing**: Never initialize per-file buffers with `Data(count:)` in hot loops. Use uninitialized raw pointers (`UnsafeMutablePointer<UInt8>.allocate`) or stack memory.
- **No Dynamic Trees**: Composite / Visitor / Decorator patterns are strictly prohibited inside compression/decompression inner loops.

### C. Fast-Path Bypass Preservation
- ZIP parallel AES decompression must directly bypass to native C engine `ttzip_extract_zip_c_parallel`.
- Apple Silicon ARM NEON SIMD routines must never be hidden behind generic fallbacks.

### D. Hard Throughput Floors (Verified via `XCTestPerformanceMeasureTests`)
| Scenario | Hard Minimum Floor |
| :--- | :--- |
| ZIP Level 1 Compression | >= 1500 MB/s |
| ZIP Level 6 Compression | >= 800 MB/s |
| ZIP Decompression | >= 4500 MB/s |
| ZIP AES-256 Decompression | >= 1800 MB/s |
| Small File Directory Scan | >= 2000 MB/s |

---

## 3. Subsystem Freeze & Safety Discipline

### A. Frozen Files
The following files are fully frozen (refer to `.agents/rules/zip-engine-freeze.md`). Modifications require explicit user token `FORCE UNFREEZE ZIP`:
- `ZipParallelExtractor.swift`, `ZipParallelWriter.swift`, `ZipCryptoEngine.swift`
- `ZipBlockParallelCompressor.swift`, `ZipBlockParallelDecompressor.swift`
- `ZipCentralDirectoryReader.swift`, `ZipStoreStreamWriter.swift`
- `CTTZipBridge_Crypto.c`, `CTTZipBridge_Crypto.h`, `CTTZipExtract.c`

### B. C Bridge Pointer Safety
- All pointer manipulations must transit through `CUnsafeBufferAdapter.withBufferPointer(data)`.
- Page buffer allocation `allocateAlignedPageBuffer` and `deallocateAlignedPageBuffer` in `NativeCoreArchitecture` must pair identically.

## 4. The Four Systemic Engineering Invariants (四大系统工程铁律)

> **最高效力级别 (Constitution Level 0)**：以下四大心法统领全库架构与编码，任何违反直接构成 CI 阻断。

### I. 流式第一性 (Stream-First)
- **零内存假设 (Zero-Memory Assumption)**：严禁在热路径或编解码引擎中出现假设内存无限的“一次性全量分配”（如读取整个几十 GB 文件到单一 `malloc`/`posix_memalign` 缓冲区）。
- **微缓冲拉取模型 (Micro-buffering Pull Pipeline)**：一切数据流动采用 `__archive_read_ahead` 与 `__archive_read_consume` 正交模型，单任务内存常驻必须稳定在 $\le 64\text{MB} \sim 128\text{MB}$。
- **零内核页清零 (Zero Zero-Fill Faults)**：热路径严禁使用 `Data(count:)` 产生内核清零页中断，必须使用未初始化裸指针与 `Data(bytesNoCopy:)`。

### II. 纵深防御 (Invariant-First)
- **POSIX 原语级防御**：严禁仅在上层使用字符串正则或黑名单进行路径安全防护。解压落盘必须开启 `ARCHIVE_EXTRACT_SECURE_SYMLINKS`、`ARCHIVE_EXTRACT_SECURE_NODOTDOT` 与 `ARCHIVE_EXTRACT_SECURE_NOABSOLUTEPATHS`。
- **延后 Fixup 倒序回写**：目录先以 `0700` 临时创建，最后按深度从深到浅倒序回写权限与 mtime，回写前使用 `O_NOFOLLOW` 验证 inode 类型，物理免疫 TOCTOU 符号链接劫持。
- **硬件级防溢出算术**：所有缓冲区与条目大小计算优先调用 `__builtin_add_overflow` / `__builtin_mul_overflow`。

### III. 确定性确界 (Bounds-First)
- **Magic 结构体生命周期**：所有 C 句柄结构体首字段必为 `magic`，构造写入，`free()` 释放前强制 `magic = 0`，使 Use-After-Free 成为可确界捕获的致命错误。
- **敏感凭据防篡改擦除**：密码、派生密钥与解密中间上下文在释放前必须使用 `memset_s` / `explicit_bzero` 写入物理内存，严禁依赖可能被编译器死代码消除的普通 `memset`。
- **跨语言 Narrowing Clamp**：所有 64 位整数向 `size_t`/`Int` 窄化前必须经过 `SSIZE_MAX` Clamp。

### IV. 真实预言机 (Oracle-First)
- **历史缺陷黄金语料库 (Golden Corpus)**：拒绝仅用合法 Mock 数据的自欺欺人测试；将真实 CVE 漏洞与边界用例持久化为 ASCII `.uu` 文本文件，由 `UUDecoder` 在内存中秒级还原。
- **跨生态双向差分测试 (Differential Oracle)**：自研引擎生成的归档必须能被系统原生 `/usr/bin/tar` 与 `/usr/bin/unzip` 完美解压；系统工具生成的归档必须能被自研引擎正确解析。
- **崩溃现场优先模糊测试 (Crash-First Fuzzing)**：变异测试在将 1% 伪随机扰动数据传入解析器前，**必须先将样本落盘至沙盒调试文件**，确保段错误发生的第一时间留存最小复现用例。

### C. Logging Discipline
- Zero bare `print(...)`, `printf(...)`, `fprintf(...)`, `puts(...)`, `fputs(...)`, or `NSLog(...)` in production, test code, and C bridge layers.
- All logging must use `TTLogger` (`TTLogger.debug`, `info`, `warning`, `error`).
- C bridge layers must never write bare diagnostic output directly to `stdout`/`stderr`.

---

## 5. Verification & Quality Gates
- **Unit & Regression Suite**: `swift test` (All 525+ tests must pass).
- **Performance Gate**: `swift test --filter XCTestPerformanceMeasureTests`.
- **Benchmark PK**: `TTZIP_RUN_BENCHMARKS=1 swift test --filter ZipBenchPkTests` (throughput degradation > 10% blocks merge).
- **CI / Hook Respect**: Bypassing Git hooks or CI verification with `--no-verify` is strictly forbidden.

---

## 6. Upstream Open-Source Contribution & Hardware Grounding Protocol

> **Applicability**: Mandatory whenever modifying or proposing patches to upstream vendor libraries (`Vendor/worktrees/*`, `zlib-ng`, `libdeflate`, `libarchive`, `zstd`, `lz4`, `bzip2`).

### Invariant 1: Hardware Grounding & Microarchitectural Proof (硬件机理确界律)
- **Zero Blind AI Submissions**: It is strictly forbidden to propose upstream PRs based purely on automated AI generation without verifying the physical instruction pipeline, register file domain crossings (FPR vs GPR), and load-to-use latencies.
- **Line-by-Line Disassembly Mandate**: Every proposed inner-loop or SIMD optimization must have an extracted machine assembly audit (`otool -tv` or `llvm-objdump -d`) proving zero unwanted stack spilling and bounded instruction count.

### Invariant 2: Multi-Workload Zero Regression (多维全负载零倒退律)
- **The Single-Length Trap Prohibition**: Optimizations targeting a specific microbenchmark length (e.g. 256-byte matches) must never be merged if they introduce regressions on high-frequency real-world workloads (short matches, compound text, level 9 deflate).
- **Hard Single-Point Regression Floor**: The patch must be evaluated across all 8 standard workloads (`text`, `striped_rgb`, `dna`, `mixed`, `short_match`, `random`, `literals`, `realistic_rgb`) across 128KB and 1MB payloads. Any statistically significant degradation exceeding +2.0% execution time immediately fails the pre-flight gate.

### Invariant 3: Single-Variable Ablation Testing (单变量消融确证律)
- **Orthogonal Verification**: When multiple optimization techniques are combined (e.g. scalar early-exit + 2x unrolling + branch hints), each technique must be benchmarked and validated in isolation before composite integration.

### Invariant 4: Maintainer Attention Reverence (Maintainer 关注度敬畏律)
- **Authentic Human Communication**: Communications with upstream maintainers must be direct, technically grounded, humble, and devoid of repetitive AI boilerplate.
- **Prompt Technical Remediation**: If a maintainer identifies an architectural regression, the contributor must immediately acknowledge the finding, isolate the root cause via single-variable testing, and provide verifiable data rather than rhetorical argument.

### Invariant 5: Atomic Commit Hygiene (原子提交整洁律)
- Upstream PR branches must be structured with clean, bisectable, and standalone atomic commits (e.g. `Refactor/Macro Infra` -> `Feat/Optimization` -> `Test/Docs`).
