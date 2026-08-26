# Feature Specification: zlib-ng Streaming Fallback Engine & Cross-Platform Hardware Acceleration

**Feature Branch**: `076-zlib-ng-streaming-fallback`  
**Created**: 2026-08-18  
**Status**: Draft  
**Input**: User description: "[zlib-ng/zlib-ng](https://github.com/zlib-ng/zlib-ng) Zlib Mac: ARMv8 CRC32 Win: AVX-512/AVX2/PCLMUL 作为流式 Deflate 的高性能回退引擎，替换 Windows 原生旧版 zlib。P1 (流式回退) 详细看看相关内容我们是怎么实现的，这个库又是怎么实现的，比我们真的更快更好吗，我们可以怎么利用 /speckit-specify"

## Clarifications

### Session 2026-08-18
- Q: 如何在保持 libarchive 上游兼容的同时利用 zlib-ng 硬件加速？ → A: 采用 `ZLIB_COMPAT=ON` 编译模式，生成符号与标准 `zlib.h` 100% 兼容的 `libz.a`，零侵入替换底层依赖。
- Q: zlib-ng 是否可以替代 libdeflate 全内存块 Fast-Path？ → A: 绝对禁止。libdeflate 在全内存块模式下具备更高的压倒性单核吞吐优势（>2000 MB/s 压缩，>10000 MB/s 解压），必须坚决保留在 Tier 1。zlib-ng 专职服务于 Tier 2 有状态增量流式状态机与 libarchive 全局底座。
- Q: Windows 与 macOS 硬件加速指令集如何分配？ → A: macOS 开启 ARMv8 CRC32/PMULL 与 NEON 向量优化；Windows 开启 AVX-512 / AVX2 / PCLMUL 动态 CPU 派发 (`DYNAMIC_CPU_DISPATCH=ON`)。

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - State-Machine Streaming Deflate Acceleration (Priority: P1)

As a user compressing or extracting unbounded/streaming data (such as CLI pipes, network streams, or multi-chunk streams with unknown uncompressed length), I want the Deflate stream engine to utilize hardware vector instructions (ARMv8 NEON/CRC32 on macOS, AVX-512/AVX2/PCLMUL on Windows) so that streaming compression and decompression speeds are 2.5x to 4x faster than legacy scalar zlib, without requiring all input data to be buffered in memory at once.

**Why this priority**: Streaming Deflate (Tier 2) is the primary fallback for all dynamic workflows where whole-buffer `libdeflate` (Tier 1) cannot be applied. On Windows and certain streaming pipelines, legacy scalar zlib has been a major throughput bottleneck (40–90 MB/s compression vs 380–520 MB/s with vectorization).

**Independent Test**:
Can be verified by streaming 100MB data through `DeflateStreamCompressor` and `DeflateStreamDecompressor` via 64KB incremental chunks and asserting bit-exact roundtrip correctness and streaming throughput >= 350 MB/s (compression) and >= 1500 MB/s (decompression).

**Acceptance Scenarios**:
1. **Given** an unbounded or chunked raw input stream, **When** processed incrementally through `DeflateStreamEngine.compressStream`, **Then** the compressed output chunks are generated with valid RFC 1951/1950/1952 bitstreams and hardware-accelerated Adler32/CRC32 checksums.
2. **Given** a valid compressed stream chunk, **When** processed through `DeflateStreamEngine.decompressStream`, **Then** the decompressed payload matches the original data byte-for-byte and halts cleanly upon encountering `Z_STREAM_END`.

---

### User Story 2 - Libarchive & Global Stream Filter Modernization (Priority: P2)

As a user extracting GZIP, CAB, TAR.GZ, or ZIP archives parsed by `libarchive`, I want the underlying `zlib.h` calls inside `libarchive` to automatically route to `zlib-ng` with hardware-accelerated CRC32, Adler32, and match finding, so that archive format decoding throughput is maximized across macOS and Windows.

**Why this priority**: `libarchive` relies on standard `zlib.h` for its internal stream read/write filters. Upgrading the underlying zlib implementation to `zlib-ng` (`ZLIB_COMPAT=ON`) immediately benefits all 80+ zlib call sites across the C codebase with zero intrusive modifications.

**Independent Test**:
Can be verified by extracting multi-file GZIP and TAR.GZ archives via `TTZipCore` and asserting full format compliance, zero memory leaks, and throughput improvements across both Apple Silicon and x86_64/Windows environments.

**Acceptance Scenarios**:
1. **Given** a GZIP archive containing 50MB of data, **When** extracted through `ArchiveExtractor`, **Then** the archive filter utilizes hardware-accelerated streaming inflate and verifies CRC32 in single-pass SIMD.
2. **Given** an invalid or truncated Deflate payload inside an archive, **When** decompressed, **Then** error states are safely mapped without unhandled crashes or undefined behavior.

---

### User Story 3 - Strict Tier Isolation & Zero Regression for Fast-Path (Priority: P3)

As a performance engineer and end user, I want full-buffer operations (known file sizes in memory or direct I/O) to strictly remain on `libdeflate` (Tier 1 Fast-Path) and never be downgraded to `zlib-ng`, so that all existing historical peak performance gates (>2000 MB/s compression, >10000 MB/s decompression) are 100% preserved with zero performance regressions.

**Why this priority**: `libdeflate` is fundamentally faster than `zlib-ng` for Whole-Buffer batch workloads. Ensuring strict architectural tier separation prevents fast-path degradation.

**Independent Test**:
Can be verified by executing `swift test --filter XCTestPerformanceMeasureTests` and `AllFormatsPkSuiteTests`, confirming that all 16 format peak throughput floors remain intact.

**Acceptance Scenarios**:
1. **Given** a 50MB known-size file in memory, **When** compressed with ZIP Level 1/6, **Then** the execution route selects `LibdeflateAccelerator` (Tier 1) and achieves >= 1500 MB/s (Debug) / >= 2000 MB/s (Release).
2. **Given** a continuous streaming input without known total length, **When** compressed via `DeflateStreamCompressor`, **Then** the execution route seamlessly selects `zlib-ng` (Tier 2) and operates with zero dynamic memory allocation per chunk.

---

## Edge Cases

- **Zero-Byte Chunks**: Passing empty buffers or zero-length chunks with `DeflateFlushMode.noFlush` must return 0 bytes without advancing stream state.
- **Premature Stream Termination**: Input stream truncation before `Z_STREAM_END` must return a clear `DeflateStreamError.corruptedData` error and avoid infinite loops.
- **Cross-Platform Instruction Fallback**: On older x86 CPUs without AVX2 or older ARM hardware without CRC32 instructions, dynamic CPU dispatch must safely fall back to standard scalar routines without throwing `SIGILL`.
- **Thread Safety**: Concurrent compression/decompression tasks on multiple threads must maintain isolated `z_stream` state blocks without global lock contention.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST provide a dual-tier Deflate architecture where Tier 1 (Whole-Buffer) is powered by `libdeflate` and Tier 2 (State-Machine Streaming) is powered by `zlib-ng`.
- **FR-002**: The Tier 2 streaming Deflate engine MUST support raw Deflate (RFC 1951, windowBits = -15), Zlib wrapped (RFC 1950, windowBits = 15), and GZIP wrapped (RFC 1952, windowBits = 31).
- **FR-003**: The Tier 2 streaming engine MUST calculate CRC32 and Adler32 checksums incrementally using hardware instructions (ARMv8 CRC32 / NEON PMULL on macOS, AVX2 / PCLMUL on Windows).
- **FR-004**: The system MUST replace legacy scalar zlib in `Vendor/` and build configurations with `zlib-ng` in `ZLIB_COMPAT` mode for unified C/C++ upstream compatibility (including `libarchive`).
- **FR-005**: The Swift interface `DeflateStreamCompressor` and `DeflateStreamDecompressor` MUST provide incremental chunk-by-chunk processing with explicit flush modes (`noFlush`, `syncFlush`, `fullFlush`, `finish`).
- **FR-006**: The streaming coder MUST guarantee clean memory deallocation upon stream completion or error, zeroing internal state handles and preventing memory leaks.

---

### Key Entities

- **DeflateTierMode**: Enum distinguishing between Tier 1 (`tier1Block` / libdeflate) and Tier 2 (`tier2Stream` / zlib-ng).
- **DeflateStreamConfig**: Configuration entity defining compression level (1–9), window bits (-15, 15, 31), memory level (1–9), and compression strategy.
- **DeflateStreamMetrics**: Runtime performance metric snapshot containing `totalIn`, `totalOut`, `adler32`, `crc32`, and `isFinished`.
- **ttzip_deflate_stream_state_t**: C-level opaque stream state container binding `z_stream` with thread-safe lifecycle and memory tracking.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Streaming Deflate compression throughput for 100MB chunked payloads increases by >= 250% compared to legacy scalar zlib (achieving >= 350 MB/s on Apple Silicon and modern x86_64).
- **SC-002**: Streaming Deflate decompression throughput for chunked payloads increases by >= 200% compared to legacy scalar zlib (achieving >= 1500 MB/s on Apple Silicon and modern x86_64).
- **SC-003**: Zero regression ($\Delta \ge 0.0\%$) across all Tier 1 Whole-Buffer benchmark tests (`XCTestPerformanceMeasureTests` and `AllFormatsPkSuiteTests`).
- **SC-004**: 100% roundtrip bitstream fidelity across all window configurations (Raw Deflate, Zlib, GZIP) and flush combinations.

---

## Assumptions

- Target build environments include macOS 14+ (Apple Silicon arm64 and Intel x86_64) and Windows 10/11 (MSVC x64 and ARM64).
- `zlib-ng` is compiled in `ZLIB_COMPAT=ON` mode to ensure 100% ABI/API drop-in compatibility for `libarchive` and existing C interfaces.
- Whole-buffer operations with known input/output lengths will strictly continue using `libdeflate` as their Tier 1 execution engine.
