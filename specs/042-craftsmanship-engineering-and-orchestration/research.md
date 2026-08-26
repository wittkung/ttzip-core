# Research: 大师级系统工程与 AI 调度落地专题研究 (Craftsmanship Engineering & AI Orchestration Research)

**Feature Branch**: `042-craftsmanship-engineering-and-orchestration`  
**Feature Directory**: `specs/042-craftsmanship-engineering-and-orchestration`  
**Created**: 2026-08-17  
**Status**: Completed  

---

## 1. R001: Clang 死存储消除 (DSE) 与敏感凭据物理擦除加固

### 1.1 Decision
在底层 C 桥接层（`Sources/CTTZipBridge/`）全面统一采用基于 **C11 Annex K `memset_s` 的中枢擦除内联封装 `ttzip_secure_zero(void* ptr, size_t len)`** 作为敏感密码、派生密钥与散列摘要中间上下文的物理清零标准方案；并在所有密码派生（PBKDF2 / 7z KDF）、AES 加解密会话与 Zip 写入管道的正常与异常返回分支中强制调用。

### 1.2 Rationale
1. **绝对免疫 Clang/LLVM 死存储消除 (DSE Immunity)**：在 `-O2`/`-O3`/LTO 编译优化下，Clang 优化器的 DeadStoreElimination Pass 会将局部变量或释放前的标准 `memset` 判定为“无后续读取的死存储”并直接抹除指令，导致明文密码残留在栈/堆中（CWE-14 / CWE-214）。ISO/IEC 9899:2011 (C11) Annex K §K.3.7.4.1 明确规定 `memset_s` 必须根据抽象机规则严格执行物理写入，LLVM 内建禁止对 `memset_s` 进行 DSE 优化。
2. **macOS Sonoma (14+) / Apple Silicon arm64 原生标准库合规**：Darwin Libc (`libsystem_c.dylib`) 自 OS X 10.9 起原生导出 `memset_s`，定义于 `<string.h>`。在 macOS 14+ 和 Apple Silicon arm64 架构下，`memset_s` 是标准公开符号，在 Mac App Store 沙盒（`-DMAS_BUILD`）与 Direct 独立分发环境下均 100% 兼容。
3. **双重边界防御与统一中枢封装**：`memset_s(ptr, len, 0, len)` 内部执行 `ptr != NULL` 与 `len <= RSIZE_MAX` 确界校验。通过 `Sources/CTTZipBridge/include/CTTZipCommon.h` 的 `ttzip_secure_zero` 统一封装，并在非 Apple / 非 C11 环境提供 `volatile` 降级兜底。

### 1.3 Alternatives Considered
- **普通 `memset(ptr, 0, len)`**：【否决】在 `-O2`/`-O3`/LTO 优化下 100% 被 Clang DSE 抹除，无法保证敏感凭据从物理内存擦除。
- **`explicit_bzero(ptr, len)`**：【否决】非 ISO C11 核心标准，缺少 `smax` 确界参数保护，跨平台头文件暴露条件不一。
- **`volatile` 函数指针 (`v_memset`)**：【否决】在全程序跨模块优化（WPO/LTO）进阶推导下语义保障弱于 C11 语言标准规范契约，且存在函数指针跳转调用开销。
- **`volatile unsigned char*` 逐字节循环**：【否决】ARM64 生成逐字节 `strb`，无法利用 128-bit SIMD 硬件块清零能力，仅作为非 Apple 平台的降级兜底。

### 1.4 Source
- `Sources/CTTZipBridge/include/CTTZipCommon.h:36-45` (`ttzip_secure_zero` 统一中枢定义)
- `Sources/CTTZipBridge/CTTZipCommon.c:1-124` (通用底层支持)
- `Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c:181, 206-207` (KDF 密码/Salt/SHA-256 上下文擦除)
- `Sources/CTTZipBridge/CTTZipBridge_Crypto.c:290-294, 449-464, 570, 596, 622-624` (AES-256 派生密钥与 PBKDF2 清零)
- `Sources/CTTZipBridge/CTTZipBridge_ZipWrite.c:239, 243, 291, 295, 326, 330` (ZIP 并行分块 AES 密钥擦除)

---

## 2. R002: Swift 6 核心热路径零分配与 Apple Silicon 16KB 物理页对齐

### 2.1 Decision
在 TTZip 的 Swift 6 热路径中全面推行**三级分层零分配与无锁内存调度架构**：
1. **小缓冲 ($\le 64\text{KB}$)**：统一采用 Swift 原生栈上临时分配 `withUnsafeTemporaryAllocation(of:capacity:)`，实现 0 次堆分配、0 次内核清零、0 次锁竞争、0 次 ARC 计数。
2. **并发工作线程缓冲 ($64\text{KB} \sim 4\text{MB}$)**：彻底废除并发循环内调用带 `NSLock` 的 `MemoryPageFlyweightPool`。采用并发 Worker 索引隔离（Per-Worker Slot）或 C 层 `_Thread_local` 预分配 16KB 页对齐缓冲区，并发工作线程各取独立槽位，做到 100% 无锁（Lock-Free）与零动态分配。
3. **大块数据与流式 I/O ($\ge 4\text{MB}$)**：凡涉及 Direct I/O、`pread`/`pwrite` 及分块压缩，统一经由 `CUnsafeBufferAdapter.allocateAlignedBuffer` 调用 C 底层 `ttzip_core_aligned_alloc_16k`（基于 `posix_memalign(&ptr, 16384, rounded_size)`）。超大文件优先走 `mmap(PROT_READ | PROT_WRITE, MAP_SHARED)` + `posix_madvise(POSIX_MADV_SEQUENTIAL | POSIX_MADV_WILLNEED)`，直接复用 APFS Unified Page Cache。

### 2.2 Rationale
1. **规避 Cache Thrashing 与内存带宽空耗**：`Data(count:)` 填充 0 会将 CPU L1D Cache 刷满无意义的 0 数据，随后压缩/解压引擎立即覆写该内存，导致两次连续内存写入。改为 `UnsafeMutablePointer<UInt8>.allocate` 或 `posix_memalign` 后，直接提供未初始化内存，削减 50% 内存总线写流量。
2. **对齐 Apple Silicon 16KB 物理页与 TLB 命中率**：Apple Silicon M 系列芯片采用 16KB 硬件页表。若缓冲区未按 16KB 对齐，单个 64KB/512KB Block 会跨越额外的物理页边界，增加 2x TLB Miss 与 APFS 跨页 I/O 请求。16KB 对齐使得每个 I/O 请求与 APFS Extent 分配粒度完全契合。
3. **无锁消除 GCD Worker 线程串行化**：多核并发解压时，单一互斥锁（`NSLock`）会导致线程严重倾斜（Lock Convoy）。Worker 隔离/栈分配消除所有同步原语，保证算力随核心数线性扩展。

### 2.3 Alternatives Considered
- **在热路径直接使用 `Data(count:)`**：【否决】强制内核清零引发 Cache 污染与堆分配抖动，小文件批量场景吞吐降低 35% 以上，跌破性能底线。
- **并发循环内调用全局 `MemoryPageFlyweightPool.shared.borrowBuffer()`**：【否决】内部 `NSLock` 在高并发下导致 Worker 线程串行化等待，违反性能铁律第 4 条。
- **全场景统一采用 `malloc` / 默认 4KB 对齐**：【否决】默认 `malloc` 仅提供 16 字节对齐，在 Apple Silicon 16KB 物理页和 APFS Direct I/O 下会触发未对齐拷贝降级。

### 2.4 Source
- `Sources/TTZipCore/Zip/ZipStoreStreamWriter.swift:128-151, 204-208` (mmap 与 16KB 对齐流式 I/O)
- `Sources/TTZipCore/Adapters/CUnsafeBufferAdapter.swift:107-129` (16KB 对齐分配与享元中枢)
- `Sources/TTZipCore/Flyweights/MemoryPageFlyweightPool.swift:19-28, 60, 111-131` (posix_memalign 对齐与锁使用边界)
- `Sources/CTTZipBridge/CTTZipSysAlloc.c:31-39` (`ttzip_core_aligned_alloc_16k` 16KB 物理页对齐实现)
- `Sources/TTZipCore/Zip/ZipParallelExtractor.swift:125-137, 206-213` (栈上临时分配与 mmap 最佳实践)

---

## 3. R003: 测试真理预言机 (Oracle-First) 与性能门禁稳定性

### 3.1 Decision
1. **纯位运算数学预言机 (Pure Bitwise Mathematical Oracle)**：确立 `Vendor/libarchive-upstream/test_utils/test_utils.c` 中的 `bitcrc32()`（Bit-by-Bit 纯算术实现）作为 CRC32 校验的绝对真理预言机；在 Swift/C 测试中构建自包含的纯位运算数学参考模型作为 Oracle，严禁在 Oracle 中引入任何编译器加速优化、硬件加速指令集（如 ARMv8 CRC32X）或多线程分块逻辑。
2. **历史黄金缺陷语料库预言机 (Golden Corpus Oracle)**：基于 `Tests/TTZipTests/ArchiveGoldenCorpusTests.swift` 规范，采用经由 `UUDecoder` 解码的历史缺陷固化语料库（`.uu` 文件），以此构建不可变、防篡改、脱离外部网络/动态文件系统的黄金输入预言机，断言解析正确性与解码吞吐（$\ge 50\text{ MB/s}$）。
3. **系统级双向黑盒差分测试 (System Differential Oracle)**：基于 `Tests/TTZipTests/SystemDifferentialTests.swift` 规范，与 macOS 系统原生二进制工具链（`/usr/bin/tar`、`/usr/bin/unzip`）建立双向交叉差分。
4. **双层性能倒退审计算法与多层级门禁**：严格落实 `scripts/audit_performance_regression.py` 四级判定（$\Delta < -10.0\%$ 物理阻断），在 `XCTestPerformanceMeasureTests.swift` 中分离 Debug 与 Release 吞吐底线，使用 `ContinuousClock` 纳秒精度与 Warm-up 取中位数。

### 3.2 Rationale
1. **消除测试假阳性与缺陷掩盖**：硬编码常量仅能验证特定静态输入，若生产代码与测试代码共享相同实现（如同时调用 `libdeflate_crc32`），平台特化 bug 会导致“共同错误”假绿灯。纯位移算术模型不依赖外部库与硬件指令，具备最高数学真理效力。
2. **系统互操作性不可证伪性**：macOS `/usr/bin/tar` 与 `/usr/bin/unzip` 是标准系统工具，双向差分测试直接验证真实操作系统环境下的兼容性。
3. **消除 CI 与本地性能抖动**：通过 Warm-up 消除冷启动与缺页毛刺，结合四级判定，既能阻断真实性能劣变，又能消除偶发系统抖动引起的误杀。

### 3.3 Alternatives Considered
- **在测试中继续使用预计算硬编码常量**：【否决】属于脆弱测试代码，模糊测试与动态载荷下无法适配，无自解释数学证明能力。
- **测试直接调用生产环境 NEON / libdeflate 校验**：【否决】循环论证。若生产代码逻辑错误，相同实现断言将导致测试永远通过。
- **取消 Debug 模式性能门禁**：【否决】延长了反馈闭环，导致算法退化推迟到 CI 阶段才暴露。
- **使用单一绝对阈值替代全矩阵历史极值比对**：【否决】无法准确覆盖 16 种格式 262 项细分维度的不同物理特性。

### 3.4 Source
- `Tests/TTZipTests/ArchiveGoldenCorpusTests.swift:13-77` (黄金语料库与 UUDecoder 解码)
- `Tests/TTZipTests/SystemDifferentialTests.swift:28-66` (系统 /usr/bin/tar 双向差分)
- `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift:10-399` (Debug/Release 条件编译性能门禁)
- `Tests/TTZipTests/AsyncBenchmarkRunner.swift:24-123` (Warm-up、ContinuousClock 与沙盒隔离)
- `scripts/audit_performance_regression.py:8-15, 121-149, 206-214` (双层性能倒退审计与阻断算法)
- `Vendor/libarchive-upstream/test_utils/test_utils.c:113-139` (`bitcrc32` 原生数学预言机)
