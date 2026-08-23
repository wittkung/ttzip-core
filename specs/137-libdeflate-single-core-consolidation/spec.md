# Feature Specification: Consolidate Single-Core Deflate Engine on libdeflate and Modernize Architecture

**Feature Branch**: `137-libdeflate-single-core-consolidation`

**Created**: 2026-08-20

**Status**: Specified

**Input**: User decision to halt low-ROI single-core Deflate custom micro-optimization against `libdeflate`, fully consolidate single-core and chunk-based Deflate/Inflate operations onto `libdeflate` (with `zlib-ng` for stateful streaming), isolate experimental native deflate code as benchmarking oracle/research baseline, and focus TTZip's core advantages on multi-core scheduling, I/O zero-copy, and container performance.

## Clarifications

### Session 2026-08-20
- **Q1: Should experimental native deflate code in `native_deflate/` be deleted or isolated?**
  - **Decision**: Retain and isolate as an internal research oracle / educational baseline (`ttzip_native_deflate_*`). Ensure all production paths in `TTZipCore` (Zip parallel compressor, memory engine, chunked stream writer, archive extractor) strictly route single-core and chunked Deflate operations to `libdeflate` (`ttzip_libdeflate_compress` / `ttzip_libdeflate_decompress`) and stateful streaming to `zlib-ng`.
- **Q2: How should compression levels be mapped to libdeflate?**
  - **Decision**: Direct 1:1 mapping for levels 1-12 supported natively by `libdeflate`. Level 0 routes to Store (uncompressed), Level 1-12 map to `libdeflate_alloc_compressor(level)`.
- **Q3: What are the performance and safety acceptance thresholds?**
  - **Decision**: Single-core memory throughput must match native `libdeflate` (>1500 MB/s for Level 1, >800 MB/s for Level 6, >4500 MB/s for Decompression). All 525+ test cases must pass with 0 regressions.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Unified High-Throughput Single-Core & Chunk Compression via libdeflate (Priority: P1)

As a macOS user or developer using TTZip, when I compress data using Deflate (via ZIP container, memory engine, or chunked stream), TTZip must execute the compression via `libdeflate` with zero unnecessary wrapper overhead, achieving optimal single-core throughput.

**Why this priority**: Deflate is the most widely used compression format in ZIP and GZ archives. Ensuring all single-core paths utilize `libdeflate` guarantees rock-solid stability and maximum throughput without maintenance debt.

**Independent Test**: Can be tested via unit tests (`LibdeflateCAdapterTests`, `ZipMemoryEngineTests`) asserting throughput >= 1500 MB/s (Level 1) and valid RFC 1951 bitstream verifiable by `/usr/bin/unzip` and `zlib`.

**Acceptance Scenarios**:
1. **Given** an uncompressed input buffer, **When** compressed with level 1-12, **Then** `libdeflate_deflate_compress` is invoked with thread-local cached compressors and returns valid RFC 1951 Deflate payload.
2. **Given** a 0-byte input, **When** compressed, **Then** the engine safely returns empty payload without memory allocation or segfault.

---

### User Story 2 - High-Speed Zero-Memory Decompression via libdeflate (Priority: P1)

As a user extracting ZIP or GZ files containing Deflate streams, TTZip must decompress every chunk and entry through `libdeflate`'s high-speed SIMD decompressor.

**Why this priority**: Decompression speed directly impacts user waiting time when opening or extracting archives.

**Independent Test**: Verified via roundtrip decompression tests comparing against expected original byte buffers with 100% bit-exact equality.

**Acceptance Scenarios**:
1. **Given** any valid RFC 1951 Deflate compressed stream, **When** decompressed via `LibdeflateCAdapter` or `ttzip_libdeflate_decompress`, **Then** the decompressed data matches the original data with exact byte parity.
2. **Given** a corrupted Deflate payload, **When** decompression is attempted, **Then** `libdeflate` safely returns error code 0 without crash or out-of-bounds memory read.

---

### User Story 3 - Stateful Streaming with zlib-ng & Architecture Decoupling (Priority: P2)

As a system component handling unbounded streaming pipelines or dictionary-injected cross-block deflate, TTZip must seamlessly use `zlib-ng` for stateful streaming while preserving `libdeflate` for whole-buffer fast paths.

**Why this priority**: Deflate streams spanning multiple incremental chunks with sliding window dependencies require stateful streaming, which `zlib-ng` provides with dynamic CPU SIMD detection.

**Independent Test**: Verified through `DeflateStreamEngine` streaming pipeline tests with chunked feed and multi-part flush.

**Acceptance Scenarios**:
1. **Given** a streaming input fed in non-uniform chunk sizes, **When** processed through `DeflateStreamEngine`, **Then** `zlib-ng` processes each chunk incrementally and emits valid stream data.

---

### User Story 4 - Clear Architecture Documentation and Benchmark Transparency (Priority: P3)

As a contributor or auditor of TTZip, the architecture documentation must clearly reflect the dual-tier Deflate strategy (libdeflate for whole-buffer/chunk plane, zlib-ng for stateful streaming plane) and document why custom single-core Deflate micro-optimization was consolidated.

**Why this priority**: Ensures long-term engineering clarity and prevents future redundant efforts on single-core Deflate.

**Independent Test**: Validated by inspecting `ARCHITECTURE.md` and performance guides.

**Acceptance Scenarios**:
1. **Given** `ARCHITECTURE.md`, **When** reviewed, **Then** it clearly defines Layer 0 upstream libraries, Tier 1/2 Deflate dispatch, and the rationale for using upstream standards.

## Edge Cases

- **0-Byte Inputs**: Handled immediately with 0 allocations, returning empty data or 0 bytes written.
- **Buffer Overflow Protection**: Destination capacity checks prevent buffer overflow before calling C bridge.
- **Thread-Local State Cleanup**: Thread-local compressor/decompressor pointers are managed safely across threads without memory leaks or race conditions.
- **Corrupted Payloads**: Negative verification with corrupted headers/payloads cleanly returns error status instead of trapping or hanging.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: `LibdeflateCAdapter` and `CTTZipStreamCoder.c` MUST be the single canonical entry point for all buffer-level and chunk-level Deflate compression and decompression.
- **FR-002**: Thread-local `libdeflate_compressor` instances MUST support levels 1 through 12 dynamically allocated on first use and cached per thread.
- **FR-003**: Thread-local `libdeflate_decompressor` MUST be shared across all decompression requests on the same thread without re-allocation.
- **FR-004**: `ZipBlockParallelCompressor` and `ZipMemoryEngine` MUST route block compression directly to `LibdeflateCAdapter` / `ttzip_libdeflate_compress`.
- **FR-005**: `ZipBlockParallelDecompressor` MUST route block decompression directly to `LibdeflateCAdapter` / `ttzip_libdeflate_decompress`.
- **FR-006**: Stateful streaming requiring arbitrary step-by-step buffer sliding MUST continue to use `zlib-ng` with dynamic SIMD dispatch.
- **FR-007**: `ARCHITECTURE.md` and related documentation MUST document the consolidated Deflate engine topology, Layer 0 upstream boundaries, and performance rationale.
- **FR-008**: All existing tests in `Tests/TTZipTests/` MUST compile and pass with zero warnings and zero regressions.

### Success Criteria

- **SC-001**: Zero compile warnings under strict Swift 6 and C11 compilation (`-warnings-as-errors`).
- **SC-002**: 100% test pass rate across all TTZip test suites (`swift test`).
- **SC-003**: Single-core Deflate compression throughput >= 1500 MB/s (Level 1) and decompression >= 4500 MB/s on Apple Silicon.
- **SC-004**: Architectural documentation updated with crystal clear tiering invariants.
