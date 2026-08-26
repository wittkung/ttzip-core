# Feature Specification: Unified SOTA Single-Core Codec Engine & Multi-Core Scheduler Architecture

**Feature Branch**: `142-unified-sota-codec-architecture`  
**Created**: 2026-08-20  
**Status**: Draft  
**Input**: User description: "好好思考完善方案：将底层 SOTA 单核开源算法（libdeflate, fast-lzma2, zstd, lz4, lbzip2, blosc2, PMULL/NEON）全面整合作为物理基石，在其上构建通用多核分块与字典预热调度层，顶层容器格式（ZIP, 7Z, TAR, DMG, WIM）彻底解耦调用；并深度审视与分析可能存在的所有工程问题与潜在风险。"

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - SOTA Single-Core Codec Integration & Universal Multi-Core Acceleration (Priority: P1) 🎯 MVP

As a system architect or high-throughput application user, I want the archiving engine to execute compression and decompression across all formats using the world's most optimized single-core algorithmic kernels (`libdeflate` for Deflate, `fast-lzma2` for LZMA2, `zstd` for Zstandard, `lz4` for LZ4, `lbzip2` for Bzip2, ARM64 PMULL for CRC), combined with a universal lock-free multi-core parallel scheduler, so that total throughput scales multiplicatively ($Speedup = \text{SingleCoreSOTA} \times \text{MultiCoreScaling}$) without being constrained by legacy single-threaded codebases.

**Why this priority**:
Constitutes the fundamental performance thesis of TTZip: standard multi-core archivers (e.g. `pigz`, `pbzip2`) suffer from slow single-core cores, while top single-core engines lack integrated multi-container framing. Unifying them delivers a 3x~5x throughput leap across all archive types.

**Independent Test**:
Can be tested independently by compressing standard Silesia and Enwik8 benchmarks using the unified engine and verifying that single-core throughput matches or exceeds upstream `libdeflate`/`fast-lzma2` while multi-core throughput scales linearly across all physical CPU cores.

**Acceptance Scenarios**:
1. **Given** a Deflate compression task (ZIP or GZIP), **When** processed by the unified engine, **Then** the engine executes via `libdeflate` + SWAR + NEON with 32 KiB sliding dictionary priming across worker threads, achieving $\ge 300\text{ MB/s}$ per core and $>4\text{ GB/s}$ aggregate multi-core throughput.
2. **Given** an LZMA2 compression task (7Z or XZ), **When** processed by the unified engine, **Then** the engine routes chunks through `fast-lzma2` Radix Match Finders, achieving $\ge 3.0\times$ faster compression than standard `7zz` CLI at bit-for-bit identical decompression compatibility.
3. **Given** any format decompression stream, **When** decoded, **Then** the single-core decompression pipeline saturates memory bus bandwidth without dynamic heap allocations in hot loops.

---

### User Story 2 - Dual-Track Adaptive Scheduling & Invariant Memory Bounds (Priority: P2)

As a DevOps engineer managing large-scale, heterogeneous datasets (mixing hundreds of thousands of small files with massive multi-gigabyte disk images), I want the scheduler to dynamically balance file-level parallelism against chunk-level parallelism while strictly enforcing bounded memory envelopes ($\le 64\text{MB} \sim 128\text{MB}$ per streaming task), so that systems with limited physical RAM (e.g. 8GB/16GB Unified Memory Macs) never encounter Out-Of-Memory (OOM) aborts, thread starvation, or kernel swapping.

**Why this priority**:
Guarantees rock-solid stability under pathological real-world workloads, preventing memory explosion when processing multi-gigabyte files with large dictionary algorithms (e.g. 1GB LZMA2 or 2GB Zstd LDM).

**Independent Test**:
Can be tested by feeding a synthetic pathological workload (100,000 $\times$ 1KB files + 1 $\times$ 50GB file) under constrained memory configurations and verifying that peak memory remains bounded below the configured threshold while all CPU cores remain fully utilized.

**Acceptance Scenarios**:
1. **Given** a directory containing $\ge 10,000$ small files ($< 1\text{MB}$), **When** scheduled, **Then** the engine routes tasks through the *Small-File Worker Pool* (file-level concurrency, zero dictionary overlap overhead).
2. **Given** a massive continuous file ($\ge 1\text{GB}$), **When** scheduled, **Then** the engine routes tasks through the *Chunk-Parallel Worker Pool* with dynamic chunk sizing and memory clamping based on `AppleSiliconTuner.shared.topology`.
3. **Given** an asymmetric core topology (P-cores and E-cores), **When** chunks are dispatched, **Then** work-stealing and asymmetric chunk sizing prevent slow E-cores from causing tail-latency bottlenecks (Straggler Problem).

---

### User Story 3 - Decoupled Container Framing & Format Standard Invariant Compliance (Priority: P3)

As a software developer integrating TTZip into cross-platform pipelines, I want all archive container formats (ZIP/Zip64, 7Z Solid, TAR PAX, Apple UDIF DMG, Microsoft WIM, ISO 9660) to be completely decoupled from underlying compression algorithms, while strictly adhering to official format specifications (APPNOTE.TXT, RFC 1951/1952, RFC 8878, Pax, ECMA-119), so that all generated archives can be extracted flawlessly by standard operating system utilities (`/usr/bin/unzip`, `/usr/bin/tar`, Windows Explorer, Finder).

**Why this priority**:
Ensures strict standards compliance, zero format corruption, and seamless cross-platform interoperability without vendor lock-in.

**Independent Test**:
Can be tested by compressing test corpuses across all 16 formats and verifying byte-accurate extraction using external standard oracles (`/usr/bin/unzip -t`, `/usr/bin/tar -tvf`, `7zz t`).

**Acceptance Scenarios**:
1. **Given** a multi-threaded parallel Deflate stream within a ZIP or GZIP archive, **When** emitted, **Then** the format-aware sequencer properly manages Deflate block boundaries (BFINAL=0 on intermediate chunks, BFINAL=1 on the terminal chunk), ensuring `/usr/bin/unzip` unpacks the archive without stream truncation errors.
2. **Given** a new compression algorithm added to Layer 0, **When** exposed, **Then** all supported containers capable of encapsulating that algorithm immediately inherit both single-core SOTA speed and multi-core scalability without rewriting container framing logic.

---

## 潜在问题与失效模式深度分析 (Deep Risk Analysis & Countermeasures)

在实施该架构时，必须直面并解决以下 6 大核心工程问题：

### 1. 字典连续性与跨线程内存拷贝开销 (Dictionary Overlap Memory Traffic)
* **问题机理**：多核分块压缩必须将前一个 Chunk 尾部的 $W$ 字节（如 Deflate 32KB，Zstd 128KB~2MB）传递给下一个 Chunk 作为预热字典。若采用跨线程内存拷贝，在高并发、小分块下会导致高频 `memcpy` 和 L1/L2 缓存行颠簸（Cache Invalidation）。
* **应对方案**：采用**环形零拷贝只读视图（Zero-Copy Sliding Ring Buffer View）**，Worker 线程直接通过只读指针偏移引用前序块末尾数据；页面内存统一由 `MemoryPageFlyweightPool` 分配，实现零多余内存搬运。

### 2. 容器标准格式对分块流的合法性约束冲突 (Bitstream Standard Invariants)
* **问题机理**：
  * *ZIP/GZIP (RFC 1951)*：标准只允许单一连续 Deflate 流。若多个线程各自生成包含 BFINAL=1 的独立流拼接，标准解压工具会在第 1 个块结束处报错。
  * *7Z/LZMA2*：原生支持带 Reset 标记的独立 chunk，但需正确配置固实（Solid）元数据。
* **应对方案**：构建**格式感知型位流汇聚器 (Format-Aware Bitstream Sequencer)**：
  * 对 ZIP/GZIP：Worker 仅输出未封口的 Raw Deflate 块（BFINAL=0），由 Sequencer 统一管理尾块 BFINAL=1 闭合；
  * 对小文件集合：直接采用文件级并行，完全绕过单文件内分块的协议开销。

### 3. 内存爆炸与不对称负载饥饿 (OOM vs Asymmetrical Starvation)
* **问题机理**：超大文件开启高压缩等级（如 LZMA2 1GB 字典）若盲目全核并发，瞬间引发数十 GB 内存索取导致 OOM；若仅按文件并行，处理单一大文件时仅单核运转，其余核心饥饿。
* **应对方案**：**双轨自适应调度器 (Dual-Track Scheduler)**：小文件走文件并行池，大文件走动态限额分块池，常驻内存强制钳制在 $\le 64\text{MB} \sim 128\text{MB}$。

### 4. 静态 C 符号污染与全局命名空间冲突 (C Namespace Collisions)
* **问题机理**：同时静态链接 `libdeflate`, `zlib-ng`, `liblzma`, `fast-lzma2`, `libzstd`, `liblz4`, `brotli`, `libarchive` 时，各库内部存在同名的全局宏或辅助函数（如 `crc32`, `adler32`, `custom_alloc`），引发链接期 Duplicate Symbol 或交叉污染。
* **应对方案**：统一开启 `-fvisibility=hidden` 编译隔离，所有自研微内核强制加上 `ttzip_` 前缀，并通过 `module.modulemap` 与纯 C11 头文件向 Swift 导出强类型接口。

### 5. Apple Silicon P-Core 与 E-Core 的非对称负载长尾阻塞 (Straggler Problem)
* **问题机理**：M 系列芯片大核与能效核算力相差 3~4 倍。若均分相同大小的重负载 Chunk，E 核迟迟无法完成，导致 Sequencer 严重阻塞在等待 E 核任务上。
* **应对方案**：**非均匀分块与工作窃取 (Asymmetric Chunking & Work-Stealing)**：分配给 E 核的 Chunk 尺寸按比例缩小（P 核 2MB，E 核 512KB），快核完成后主动从慢核窃取未开始的任务。

### 6. 密码学敏感内存泄露与编译器死代码消除 (Dead-Store Elimination Risks)
* **问题机理**：多线程并发加解密时，密码与派生密钥若缓存在临时堆中可能发生泄漏；释放前的普通 `memset` 极易被编译器优化掉。
* **应对方案**：严格执行 `ttzip_secure_zero`（C11 `memset_s` + 内存屏障），所有密钥派生限制在确定性栈帧中，杜绝在并发堆对象中散落口令。

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST implement a 4-layer decoupled architecture: Layer 0 (SOTA Single-Core Kernels), Layer 1 (Universal Multi-Core Parallel Scheduler), Layer 2 (Decoupled Container Framing), and Layer 3 (Swift 6 Strict Concurrency Domain & Presentation).
- **FR-002**: Layer 0 MUST integrate the absolute SOTA single-core engines: `libdeflate` (Deflate/Gzip), `fast-lzma2` (LZMA2/7z), `facebook/zstd` (Zstandard), `liblz4` (LZ4), `kjn/lbzip2` (Bzip2), `google/brotli` (Brotli), `lzfse/lzfse` (LZFSE), `google/snappy` (Snappy), `Blosc/c-blosc2` (Shuffle/Bit-Grooming), and ARM64 PMULL CRC64/CRC32.
- **FR-003**: Layer 1 MUST implement a universal lock-free multi-core parallel scheduler supporting dynamic chunking, dictionary priming (sliding history overlap across chunks), and memory-page flyweight pooling.
- **FR-004**: Layer 1 MUST implement Dual-Track Adaptive Scheduling: routing small files ($< 1\text{MB}$) to the *File-Level Pool* and large files ($\ge 1\text{MB}$) to the *Chunk-Level Pool* with P/E-core asymmetric sizing.
- **FR-005**: Layer 2 MUST decouple container framing (ZIP/Zip64, 7Z, TAR PAX, Apple UDIF DMG, WIM, ISO) from underlying codecs, interacting strictly via unified C ABI stream interfaces (`ttzip_codec_ops_t` and `ttzip_parallel_compressor_t`).
- **FR-006**: The engine MUST enforce strict RFC 1951/1952 bitstream validity in multi-threaded Deflate streams, emitting non-terminal chunks with BFINAL=0 and the terminal chunk with BFINAL=1.
- **FR-007**: The engine MUST guarantee zero dynamic heap allocation in inner compression loops, capping resident streaming memory to $\le 64\text{MB} \sim 128\text{MB}$ per task.
- **FR-008**: All cryptographic credentials, key derivations, and intermediate states MUST be cleared using `ttzip_secure_zero` (`memset_s` + assembly memory barrier).

---

### Key Entities

- **`ttzip_codec_ops_t`**: Pure C11 VTable defining single-core creation, block compression with history dictionary, and block decompression operations.
- **`ttzip_parallel_compressor_t`**: Multi-core scheduler instance managing thread pool dispatch, chunk ring buffers, and bitstream sequencing.
- **`ArchiveContainerWriter`**: Swift protocol representing container format writers responsible solely for entry headers, directory records, and archive finalization.
- **`DictionaryOverlapWindow`**: Represents the historical sliding dictionary buffer ($32\text{KB} \sim 2\text{MB}$) passed across adjacent chunk boundaries.
- **`HardwareTopologyContext`**: Encapsulates physical CPU core topology (P-cores vs E-cores), page alignment, and unified memory bandwidth limits.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Single-core Deflate compression throughput reaches $\ge 300\text{ MB/s}$ on Apple Silicon; multi-core Deflate throughput reaches $\ge 4,000\text{ MB/s}$ (16-core).
- **SC-002**: 7Z / LZMA2 compression speed achieves $\ge 3.0\times$ speedup compared to standard 7-Zip CLI (`7zz`) at identical compression ratio.
- **SC-003**: Multi-core parallel scaling efficiency achieves $\ge 85\%$ across 8 to 32 physical cores.
- **SC-004**: 100% of generated ZIP, 7Z, TAR.GZ, TAR.ZST, DMG, and WIM archives pass validation by external standard system oracles (`/usr/bin/unzip`, `/usr/bin/tar`, `7zz t`).
- **SC-005**: Peak resident memory consumption remains strictly $\le 128\text{MB}$ during 50GB+ streaming compression tasks.

---

## Assumptions

- **Target OS & Hardware**: macOS 14.0+ running on Apple Silicon (M1/M2/M3/M4) with x86-64 backward compatibility.
- **Language & Runtime**: Swift 6.0 (`swift-tools-version: 6.0`) with strict concurrency checking + C11 / POSIX APIs.
- **Zero Subprocess Policy**: All codecs operate in-process via static C library bindings; zero `posix_spawn`/`NSTask` invocations.
