# Research Report: zlib-ng Streaming Fallback Engine & Cross-Platform Hardware Acceleration

**Feature**: `076-zlib-ng-streaming-fallback`  
**Created**: 2026-08-18  
**Status**: Completed  

---

## 1. R001: zlib-ng 构建配置、ZLIB_COMPAT 模式与跨平台 (macOS/Windows) 静态集成边界

- **Decision (选定方案)**:
  采用 **`-DZLIB_COMPAT=ON` 模式静态打包构建 Universal 2 (macOS) 与 MSVC x64/ARM64 (Windows) 库**。
  - **macOS 集成**：通过 `scripts/build_zlib_ng.sh` 编译 Universal 2 (arm64 + x86_64) `libz.a`，与 `libarchive.a`、`libdeflate.a`、`liblzma.a` 等统一打包进 `Vendor/TTZipVendor.xcframework` 与 `Vendor/libTTZipVendor.a`。在 `Package.swift` 中**彻底移除**系统 `.linkedLibrary("z")`，由 `CTTZipBridge` 静态绑定 `TTZipVendor`。
  - **Windows 集成**：在 `CMakeLists.txt` 中引入 `Vendor/zlib-ng`，开启 `DYNAMIC_CPU_DISPATCH=ON`、`WITH_AVX512=ON`、`WITH_AVX2=ON` 与 `WITH_PCLMULQDQ=ON`，使 `CTTZipBridge` 直接链接 `zlibstatic`。

- **Rationale (选择理由)**:
  1. **符号隔离与上游库零侵入**：`ZLIB_COMPAT=ON` 暴露标准 `zlib.h` 头文件与函数符号名，使得 `Vendor/libarchive-upstream` 内部 80+ 处 `deflateInit2` / `inflate` / `crc32` 调用无需修改任何源码即可直接受益于硬件 SIMD 加速。
  2. **静态打包消除动态依赖与版本漂移**：MAS 沙盒与 Direct 分发要求应用自包含。将 `libz.a` 静态合并入 `libTTZipVendor.a` 避免了 macOS 系统旧版标量 `libz.dylib` (1.2.12) 的符号抢占问题，确保编译与运行时行为 100% 确定。
  3. **动态 CPU 调度避免指令集崩溃**：`DYNAMIC_CPU_DISPATCH=ON` 确保生成的单一 x86_64 二进制文件能在老旧 CPU 上安全降级运行，而在支持 AVX2/AVX-512 的 CPU 上自动激活高性能路径。

- **Alternatives Considered (被否决方案及理由)**:
  1. **被否决方案 1：直接链接 macOS 系统 `/usr/lib/libz.dylib`（即在 `Package.swift` 中使用 `.linkedLibrary("z")`）**。
     - *否决理由*：macOS 系统自带的 `libz.dylib` 为标准 zlib 1.2.x，缺乏 ARM NEON、PMULL 及 x86 AVX2/AVX-512 硬件加速指令；且系统动态库在不同 macOS 版本之间存在版本漂移，无法满足 TTZip 吞吐底线要求。
  2. **被否决方案 2：使用 zlib-ng 原生 API 模式（`-DZLIB_COMPAT=OFF`，即 `zng_deflate` / `zlib-ng.h`）**。
     - *否决理由*：该模式会生成带有 `zng_` 前缀的符号，导致 `Vendor/libarchive-upstream` 等无法直接静态链接解析标准 `deflate` / `inflate` 符号，需要对上游代码进行大规模侵入式 Patch，违背了维护上游纯净度的原则。

- **Source (查阅来源)**:
  1. `scripts/build_zlib_ng.sh` (L70–L143, L173–L200)
  2. `CMakeLists.txt` (L35–L68, L116–L121)
  3. `Package.swift` (L28–L49)
  4. `Vendor/lib/` 与 `Vendor/TTZipVendor.xcframework`

---

## 2. R002: Tier 1 (libdeflate) 与 Tier 2 (zlib-ng) 双轨分流架构及吞吐性能门禁边界

- **Decision (选定方案)**:
  采用 **严格物理隔离的 Dual-Tier 双轨分流架构**：
  - **Tier 1 (Whole-Buffer Fast-Path: libdeflate)**：针对内存中已知完整大小的数据块（如 ZIP 标准条目压缩/解压、`ArchivePipelineProducerConsumerEngine` 中的分块多线程并行任务），必须强制直通 `libdeflate`。保留并使用 `Thread-Local`（TLS）对象池（`g_tls_compressors[14]` 与 `g_tls_decompressor`），实现热路径零 `malloc`/`free` 分配。
  - **Tier 2 (Streaming Fallback: zlib-ng)**：针对无界流（Network Stream）、管道输入（POSIX Pipe）、增量异步流（`AsyncThrowingStream` / `AsyncSequence`）以及需要精细控制 `Z_SYNC_FLUSH` / `Z_FULL_FLUSH` / `windowBits`（RFC 1950 Zlib / RFC 1951 Raw / RFC 1952 Gzip）的场景，分流至 `zlib-ng` 流式状态机。

- **Rationale (选择理由)**:
  1. **Whole-Buffer 场景下 libdeflate 的微架构优势**：
     - **零状态机中断开销**：libdeflate 假定全部输入与输出均在连续内存中，其内层循环不需要频繁维护 `avail_in` / `avail_out` 边界检查，也不需要保存/恢复 32KB 环形窗口指针。
     - **宽字长非对齐访问与专用哈希**：libdeflate 采用多级哈希表与 64 位宽字非对齐匹配查找，并使用基于 64 位寄存器字长直接写入的 Huffman 位流生成器，无需逐字节同步位缓冲区。
     - **解压向量优化**：libdeflate 解压器采用专用预计算快速查找表和 16 字节宽块复制（Apple Silicon 上可达 10,000+ MB/s），而 zlib-ng 在流式状态机约束下解压吞吐通常在 2,000–3,500 MB/s。
  2. **Streaming 场景下 zlib-ng 的不可替代性**：
     - libdeflate 官方设计原则为“全缓冲批处理”，完全不支持在数据未就绪时暂停并在后续块传入时恢复状态（没有流式上下文状态机）。
     - zlib-ng 拥有完整的 `z_stream` 状态机，能维护 32KB 历史字典滑动窗口，原生支持 `Z_SYNC_FLUSH`、`Z_FULL_FLUSH`、`Z_FINISH`，可满足 `DeflateStreamEngine` 异步流式管道的需求。

- **Alternatives Considered (被否决方案及理由)**:
  1. **被否决方案 1：废弃 libdeflate，全部场景统一使用 zlib-ng**。
     - *否决理由*：这将导致 ZIP / 7Z 等格式的全量内存压缩与解压吞吐发生严重性能倒退（压缩吞吐从 2,000+ MB/s 跌至 600–900 MB/s，解压吞吐从 10,000+ MB/s 跌至 3,000 MB/s），直接击穿 TTZip 性能铁律硬门禁。
  2. **被否决方案 2：在 libdeflate 之上通过应用层切片拼凑流式解压**。
     - *否决理由*：Deflate 的 LZ77 算法依赖跨 32KB 边界的历史引用。若无连续字典状态机，应用层人工切片无法解压跨块引用的标准 Deflate 流，会引发数据损坏。

- **Source (查阅来源)**:
  1. `Sources/CTTZipBridge/CTTZipStreamCoder.c` (L10–L44, L132–L268)
  2. `Sources/TTZipCore/Adapters/LibdeflateCAdapter.swift` (L12–L87)
  3. `Sources/TTZipCore/Pipeline/DeflateStreamEngine.swift` (L7–L22, L74–L115, L531–L643)
  4. `ARCHITECTURE.md` (L78–L82)
  5. `GEMINI.md` (§四.1 性能铁律)

---

## 3. R003: 硬件加速校验和 (CRC32/Adler32) 与无锁状态机设计

- **Decision (选定方案)**:
  - **硬件加速校验和路径**：
    - **ARM64 / Apple Silicon**：CRC32 优先调用 ARMv8 专用指令（`__crc32b` / `__crc32w` / `__crc32d`）及 Crypto Extension PMULL 128 位无进位乘法多路折叠算法；Adler-32 采用 ARM NEON 矢量化宽加指令（`vaddw_u8`、`vmlal_u16`）分块累加，在 5552 字节周期执行模 65521 归约。
    - **x86_64**：CRC32 采用 PCLMULQDQ (`vpclmulqdq`) 进行 128/256 位并行折叠；Adler-32 采用 AVX2 256 位矢量累加。
    - 在 C 桥接层中，CRC32 计算统一桥接至 `libdeflate_crc32` / `ttzip_compute_buffer_crc32_neon` 硬件路径。
  - **无锁状态机设计**：
    - `ttzip_deflate_stream_state_t` 采用“单实例独占 + 显式生命周期”模型。每一个流式会话（`DeflateStreamCompressor` / `DeflateStreamDecompressor`）拥有独立的堆分配 `state` 与 `internal_state (z_stream*)`。
    - 多线程并发（如 `withTaskGroup` / `DispatchQueue.concurrentPerform`）在各自的 Task 上独立持有实例，数据平面**零互斥锁 (Zero-Mutex)、零自旋锁 (Zero-Spinlock)、零跨线程原子争用**。
    - **内存确界与失效防御 (Invariant-First)**：
      - 状态机头部嵌入 Magic 幻数 `TTZIP_DEFLATE_STREAM_MAGIC = 0x545A4453U`。
      - 销毁时严格执行减法防御：先将 `state->magic = 0` 使状态机失效，再调用 `deflateEnd()` / `free()`，最后以 `memset(state, 0, sizeof(*state))` 清零，彻底杜绝 UAF 与 Double-Free。

- **Rationale (选择理由)**:
  1. **吞吐瓶颈转移**：在 SIMD 优化下，Deflate 编解码的主瓶颈往往落在校验和（CRC32/Adler32）计算上。传统查表法吞吐仅约 1–2 GB/s，而 ARMv8 PMULL 与 x86 PCLMULQDQ 折叠算法吞吐可达 20–30+ GB/s，保证校验和计算不会成为数据管道的瓶颈。
  2. **无锁隔离保证横向扩展**：锁竞争是多核流式压缩的最大衰减源。通过实例级隔离与 Thread-Local 缓存，使得 $N$ 个并发流式压缩任务能够线性利用 Apple Silicon 的所有性能核心与能效核心。

- **Alternatives Considered (被否决方案及理由)**:
  1. **被否决方案 1：全局维护一个共享的 `z_stream` 对象池并加锁借还**。
     - *否决理由*：在 `TaskGroup` 20+ 高并发流式传输下，互斥锁会导致线程上下文切换和内核系统调用激增，严重破坏性能铁律；且 `z_stream` 在流式过程中需要持久持有滑动窗口状态，无法在流未结束前跨线程安全复用。
  2. **被否决方案 2：使用标准 zlib 的软件查表法 CRC-32 (`crc32()` table-lookup)**。
     - *否决理由*：查表法存在 L1 D-Cache 频繁访存与分支预测惩罚，吞吐比硬件 PMULL / CRC32 指令低一个数量级。

- **Source (查阅来源)**:
  1. `Sources/CTTZipBridge/CTTZipStreamCoder.c` (L134–L201, L203–L268)
  2. `Sources/CTTZipBridge/ttzip_platform_detect.c` (L13–L74)
  3. `Sources/CTTZipBridge/CTTZipCRC32Neon.c` (L1–L15)
  4. `Sources/TTZipCore/Pipeline/DeflateStreamEngine.swift` (L128–L322, L326–L529)
  5. `Tests/TTZipTests/DeflateStreamingPipelineTests.swift` (L230–L257)
