# Feature Specification: Blosc2 Deep Architectural Study and Meta-Compression Pipeline Integration

**Feature Branch**: `088-blosc2-deep-architectural-study-and-integration`
**Created**: 2026-08-18
**Status**: Ready for Planning
**Input**: User description: "[Blosc/c-blosc2](https://github.com/Blosc/c-blosc2) BSD 3-Clause 好好研究这个仓库还有什么值得我们学习，可以让我们做到更好的 /speckit-specify"

---

## Clarifications

### Session 2026-08-18
- Q: What specific architectural mechanisms from `c-blosc2` should TTZip prioritize for learning and integration? → A: Four core mechanisms: (1) SIMD BitShuffle & ByteDelta multi-filter chaining pipeline; (2) Special-Value (all-zero, uninitialized, constant pattern) zero-overhead bypass engine; (3) Super-Chunk (schunk) and two-tier cache-aware chunk/block partitioning with shared frame dictionary training; (4) Dynamic small-block compressibility probing (BTune-inspired heuristic filter/codec selection).
- Q: How should these innovations integrate with standard archive ecosystems (ZIP, 7Z, TAR.ZST) versus standalone high-throughput frame containers? → A: Dual-mode integration: Standard container paths transparently adopt L1/L2 cache-blocking, special-value bypass, and pre-filtering; high-performance binary/tensor datasets and QuickLook previews support Blosc2/B2ND frame container decoding and direct-to-memory streaming.
- Q: What platform-specific optimizations should be enforced on Apple Silicon? → A: Apple Silicon M-series CPUs feature 128KB L1 Data Caches and 128-byte cache lines (compared to 32KB/64-byte on x86); chunk and block sizing must dynamically calibrate to 128KB/256KB boundaries, with ARM NEON SIMD vector transposition and PMULL hardware acceleration.

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - SIMD Multi-Filter Chaining for Numerical & Structured Data (Priority: P1)

As a user archiving scientific datasets, telemetry logs, machine learning weights (Safetensors/GGUF), or structured tables, I want TTZip to offer SIMD-accelerated pre-filtering (BitShuffle and ByteDelta) chained before LZ4/Zstd/Deflate compression, so that numerical datasets achieve 5x to 30x higher compression ratios without sacrificing compression speed.

**Why this priority**: Traditional byte-oriented LZ algorithms struggle with floating-point mantissas, timestamps, and multi-byte integers because identical bits are scattered across byte boundaries. Bit-level shuffling and delta differencing cluster identical bits into long runs, transforming low-entropy numerical data into ideal inputs for entropy coders.

**Independent Test**: Can be tested independently by compressing a 100MB synthetic floating-point array and time-series log corpus using (a) raw Zstd vs (b) BitShuffle + ByteDelta + Zstd, verifying that the chained filter achieves >= 5x compression ratio improvement and >= 3 GB/s filter throughput on Apple Silicon.

**Acceptance Scenarios**:
1. **Given** a 32-bit or 64-bit floating-point / integer dataset, **When** applying the BitShuffle filter with NEON SIMD acceleration, **Then** bits are transposed into contiguous bit-planes at >= 4,000 MB/s, and subsequent compression achieves at least 3x higher ratio than raw compression.
2. **Given** a monotonic or correlated time-series stream, **When** chaining ByteDelta after Shuffle, **Then** consecutive differences collapse into zero bytes, elevating compression throughput and ratio.
3. **Given** a compressed stream with chained filters, **When** decompressing, **Then** inverse filters restore the original data bit-for-bit with 100% data integrity.

---

### User Story 2 - Special-Value Zero-Overhead Chunk & Block Bypass (Priority: P2)

As a user compressing large disk images, sparse sparse-matrix files, database dumps, or memory snapshots containing massive regions of zeros, NaNs, or repeating constant patterns, I want TTZip to detect uniform blocks branchlessly and bypass the heavy compression engine entirely, so that uniform regions decompress at memory bus line-rate (> 25 GB/s) with zero storage overhead.

**Why this priority**: Disk images, container layers, and sparse scientific arrays often contain gigabytes of pure zeros or uninitialized padding. Running standard DEFLATE or LZMA on these wastes massive CPU cycles and cache bandwidth.

**Independent Test**: Can be tested by compressing a 1GB sparse file (containing 90% zeros / uninitialized pages), verifying that compression completes in < 50 ms and decompression executes via SIMD memory broadcast at >= 25,000 MB/s.

**Acceptance Scenarios**:
1. **Given** an uncompressed chunk consisting entirely of zeros, **When** analyzed by the vectorized uniform scanner, **Then** the block is flagged as `SPECIAL_ZERO` with 0 payload bytes stored, requiring zero compression cycles.
2. **Given** a chunk flagged as `SPECIAL_ZERO` or `SPECIAL_VALUE`, **When** decompressed, **Then** the buffer is populated via vectorized `memset` or SIMD broadcast at memory bus saturation speed without allocating decompressor context.
3. **Given** a mixed file containing both sparse and high-entropy data, **When** packaged into an archive, **Then** sparse blocks are bypassed while entropy blocks are normally compressed, producing an intact, valid archive.

---

### User Story 3 - Two-Tier Cache-Aware Partitioning & Shared Dictionary Training (Priority: P3)

As a software developer compressing collections of structured logs, JSON records, or small configuration files, I want TTZip to partition data into L1/L2 cache-sized blocks (128KB–256KB) and share a frame-level pre-trained dictionary across chunks, so that small and repetitive files achieve maximum compression density without cache thrashing.

**Why this priority**: Independent compression of small blocks suffers from dictionary cold-start penalties. Training and sharing a dictionary across a Super-Chunk eliminates dictionary ramp-up overhead and maximizes compression ratios.

**Independent Test**: Can be tested by compressing 10,000 small JSON / log files with a shared Super-Chunk dictionary versus independent ZIP entries, verifying a >= 40% size reduction and sustained multi-threaded throughput.

**Acceptance Scenarios**:
1. **Given** a Super-Chunk containing heterogeneous small records, **When** dictionary training is enabled, **Then** a shared dictionary (up to 112KB) is embedded once in the container header, and all chunks compress against this shared dictionary.
2. **Given** a multi-core compression job, **When** processing blocks within a chunk, **Then** working buffers stay within private L1/L2 caches (128KB–256KB), avoiding L3 cache pollution and memory bus contention.

---

### User Story 4 - Heuristic Small-Block Auto-Tuning (BTune Optimization) (Priority: P4)

As a user archiving heterogeneous directories with mixed media, binaries, and text, I want TTZip to dynamically probe compressibility on initial sample blocks to automatically select the optimal filter and codec configuration, preventing catastrophic expansion on incompressible data and maximizing speed on compressible data.

**Why this priority**: Static compression settings either waste time running useless filters on JPEG/PNG files or miss massive ratio gains on numerical data. Adaptive probing finds the Pareto-optimal configuration in microseconds.

**Independent Test**: Can be tested by running an automated benchmark on a mixed corpus (compressed JPEGs + raw CSV + float models), verifying that incompressible files bypass filters in $O(1)$ time while numerical data automatically triggers BitShuffle.

**Acceptance Scenarios**:
1. **Given** an incompressible data stream (e.g. encrypted or pre-compressed data), **When** small-block heuristic probing runs, **Then** filters are automatically disabled, preventing throughput degradation and negative compression.
2. **Given** a structured array stream, **When** probing detects high shuffle compressibility, **Then** the pipeline automatically engages Shuffle + ByteDelta.

---

### Edge Cases

- **Unaligned Byte Sizes in BitShuffle**: When the input length is not an exact multiple of `8 * sizeof(type)`, the leftover tail bytes must be safely copied without buffer overrun or bit corruption.
- **Mixed Endianness & Type Sizes**: BitShuffle and ByteDelta must strictly validate `typesize` (1, 2, 4, 8, 16 bytes) and ensure endianness consistency across ARM64 and x86_64 architectures.
- **Corrupted Frame Headers or Special Value Flags**: Corrupted special-value flags must be caught by CRC32 verification before buffer population to prevent memory safety violations.
- **Zero-Length Chunks & Empty Super-Chunks**: Empty datasets must produce valid frame headers with zero chunk count without triggering division-by-zero in partition geometry.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST implement SIMD-accelerated `BitShuffle` (transposing bits into 8 bit-planes across 1/2/4/8/16-byte elements) using ARM NEON vector instructions on Apple Silicon and SSE2/AVX2 on Intel.
- **FR-002**: The system MUST implement SIMD-accelerated `ByteDelta` (first-order byte differencing and cumulative prefix sum reconstruction) with NEON and SSE4.1 vectorization.
- **FR-003**: The system MUST support dynamic multi-filter chaining up to 4 sequential stages (e.g., `TruncPrec` -> `Shuffle`/`BitShuffle` -> `ByteDelta` -> `Zstd`/`LZ4`/`Deflate`).
- **FR-004**: The system MUST implement branchless SIMD detection for uniform chunks, identifying `SPECIAL_ZERO`, `SPECIAL_NAN`, `SPECIAL_UNINIT`, and `SPECIAL_VALUE` in $O(N)$ with memory bandwidth saturation.
- **FR-005**: The system MUST bypass the compression and decompression codec for special-value chunks, storing a 1-byte descriptor and executing decompression via SIMD `memset` / broadcast.
- **FR-006**: The system MUST support two-tier hierarchical partitioning (Super-Chunk -> Chunks -> Blocks) where block size is dynamically calibrated to CPU L1/L2 cache boundaries (128KB for Apple Silicon).
- **FR-007**: The system MUST support frame-level pre-trained Zstd dictionary sharing across chunks in a Super-Chunk.
- **FR-008**: The system MUST provide an adaptive small-block heuristic tuner (probing compressibility of 16KB–64KB sample blocks) to select optimal filter configurations dynamically.
- **FR-009**: The system MUST guarantee bit-for-bit lossless data roundtrip for all filter combinations, verified against differential golden vectors.
- **FR-010**: The system MUST adhere to Zero-Memory Assumption and Zero Kernel Zero-Fill Faults, utilizing cache-aligned pre-allocated page buffer pools.

### Key Entities

- **BloscFilterPipeline**: An ordered chain of pre-compression transformations (Shuffle, BitShuffle, ByteDelta, TruncPrec) applied prior to entropy coding and reversed in exact inverse order during decompression.
- **SpecialValueDescriptor**: A compact 1-byte metadata flag indicating whether a chunk is uniform (All Zero, All NaN, All Uninitialized, or Repeated 8-byte Pattern) with zero payload bytes stored.
- **SuperChunkContainer**: A multi-chunk logical container managing 64-bit sparse chunk offsets, frame-level shared dictionaries, and variable-length metalayer trailers.
- **HeuristicTuner**: A lightweight micro-probing engine that tests small sample blocks against candidate filter/codec pipelines to determine the Pareto-optimal configuration.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: SIMD BitShuffle on Apple Silicon NEON MUST achieve >= 4,000 MB/s throughput for 32-bit and 64-bit numerical data.
- **SC-002**: SIMD ByteDelta on Apple Silicon NEON MUST achieve >= 8,000 MB/s throughput for forward differencing and inverse reconstruction.
- **SC-003**: Chained BitShuffle + ByteDelta + Zstd on synthetic float and time-series datasets MUST achieve >= 5x compression ratio improvement over un-filtered compression.
- **SC-004**: Special-value chunk detection and decompression MUST achieve >= 25,000 MB/s on Apple Silicon via SIMD memory operations.
- **SC-005**: Heuristic tuner probing overhead MUST NOT exceed 2% of total compression time for files larger than 1MB.
- **SC-006**: 100% of generated bitstreams and frames pass automated roundtrip verification with zero checksum mismatches and zero memory leaks.
- **SC-007**: Full regression test suite (525+ tests) and 13 performance floors maintain 100% green pass.

---

## Assumptions

- Target operating systems are macOS 14.0+ (Sonoma, Sequoia) on Apple Silicon (M1/M2/M3/M4) and Intel x86_64.
- Apple Silicon M-series performance cores have 128KB L1 Data Cache and 128-byte cache lines, providing optimal performance when block boundaries are aligned to 128KB multiples.
- Standard archive formats (ZIP, 7Z, TAR.ZST) remain standard-compliant; filter extensions operate on stream payloads or within dedicated high-throughput frame containers.
- Numerical data includes IEEE-754 single/double precision floats, integers, and fixed-stride structured records.
