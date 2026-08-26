# Feature Specification: zlib-ng NEON LCP Acceleration & Dual-Platform Integration

**Feature Branch**: `058-zlib-ng-neon-integration`

**Created**: 2026-08-17

**Status**: Draft

**Input**: User description: "zlib-ng/zlib-ng 架构与 NEON LCP 对标、源码实现机制分析、性能对比与双平台集成策略"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Dual-Platform High-Performance Streaming Deflate Pipeline (Priority: P1)

As a user compressing or extracting streaming Deflate/GZIP archives (large archives or continuous stream I/O where whole-buffer flat memory is unavailable), I want the system to utilize modern hardware-accelerated SIMD (ARM64 NEON / x86_64 AVX2/AVX-512) for sliding window compression and expansion, so that streaming compression throughput is drastically improved compared to legacy scalar zlib.

**Why this priority**: While TTZip uses `libdeflate` for in-memory block compression (achieving peak throughput >1,500 MB/s), streaming and pipeline-based operations currently fall back to scalar zlib/libarchive. Integrating modern SIMD-accelerated Deflate streaming bridges the last major gap in TTZip's Deflate stack across macOS and Windows.

**Independent Test**: Can be tested by streaming a 100MB+ dataset through the Deflate streaming compressor and verifying that throughput exceeds 350 MB/s (a 2.5x–3x improvement over legacy scalar zlib) while maintaining bit-identical RFC 1951 Deflate compatibility.

**Acceptance Scenarios**:

1. **Given** a streaming input buffer, **When** Deflate compression is executed in streaming mode, **Then** the compressed output matches RFC 1951 specifications and decompression recovers original data with 100% fidelity.
2. **Given** macOS Apple Silicon hardware, **When** executing sliding window match finding, **Then** ARM NEON / SWAR vector comparisons are activated transparently without performance regressions.

---

### User Story 2 - Micro-Architecture Match-Length Comparison Optimization (Priority: P2)

As a compression engine developer, I want the sliding-window match length calculation (`compare256` / `match_len`) to avoid cross-register domain latency between NEON 128-bit vector units and general-purpose integer registers, so that short matches (3–8 bytes) and long matches (up to 258 bytes) achieve maximum instruction throughput on superscalar Apple Silicon cores.

**Why this priority**: In LZ77 parsing, >80% of candidate comparisons terminate within the first 8 bytes. A hybrid match finder (64-bit SWAR for short candidates + 128-bit NEON unrolling for long candidates) eliminates pipeline stalls and provides optimal execution time.

**Independent Test**: Execute benchmark micro-tests for candidate matching with various prefix lengths (0–258 bytes) and assert zero CPU pipeline stall cycles and monotonic performance improvement.

**Acceptance Scenarios**:

1. **Given** two memory pointers within sliding window, **When** comparing matching bytes, **Then** the algorithm accurately returns length in range `[0, 258]`.
2. **Given** candidate matches differing in the first 8 bytes, **When** evaluated, **Then** evaluation completes in <= 3 CPU cycles using unaligned 64-bit load and `__builtin_ctzll`.

---

### User Story 3 - Windows x86_64 Legacy zlib Replacement & Upstream Contribution (Priority: P3)

As a Windows cross-platform user, I want the archive engine on Windows to replace legacy `zlib1.dll` with an AVX2/AVX-512/NEON-accelerated backend, and contribute verified ARM64 match optimizations back to upstream `zlib-ng`.

**Why this priority**: Enables consistent high-speed Deflate processing on Windows while establishing an open-source collaboration cycle with the `zlib-ng` community.

**Independent Test**: Verify Windows binary loads hardware-optimized zlib-ng symbols without dependency on legacy `zlib1.dll`.

**Acceptance Scenarios**:

1. **Given** Windows x86_64 or ARM64 execution, **When** running Deflate routines, **Then** AVX2/NEON paths are automatically dispatched via CPU feature detection.

---

### Edge Cases

- **Unaligned Memory Pointers**: Sliding window candidates may not be 16-byte aligned. The engine must handle arbitrary unaligned loads without generating bus faults or alignment penalties on ARM64.
- **Window Boundary Overrun**: Match length search near the end of the sliding buffer (`max_len < 16`) must safely fall back without reading past valid buffer boundaries.
- **Identical Long Runs (258 Bytes)**: Full 258-byte Deflate maximum match lengths must clamp cleanly at exactly 258 bytes without byte over-consumption.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide dual-tier Deflate processing: Tier 1 Whole-Buffer `libdeflate` for in-memory block compression, and Tier 2 SIMD-accelerated `zlib-ng` streaming for non-in-memory pipeline streams.
- **FR-002**: System MUST implement hybrid match finding on ARM64 combining 64-bit SWAR immediate check with 128-bit NEON vector unrolling (`compare256_neon`).
- **FR-003**: System MUST guarantee 100% RFC 1951 Deflate and RFC 1952 GZIP format compliance and standard CRC32/Adler32 validation.
- **FR-004**: System MUST support dynamic CPU feature dispatch (NEON/CRC32 on ARM64, AVX2/AVX-512 on x86_64) on macOS and Windows.
- **FR-005**: System MUST ensure zero memory allocations inside hot-path matching loops and use thread-local or static context structures.

### Key Entities

- **DeflateStreamContext**: Manages streaming sliding window state (32KB window, pending buffer, hash chain, and tree tables).
- **MatchFinderBackend**: Interface encapsulating hardware-accelerated longest-match and match-length calculation strategies.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Streaming Deflate compression throughput on Apple Silicon exceeds 350 MB/s for Level 1 and 80 MB/s for Level 6.
- **SC-002**: Micro-benchmark match length calculation latency for short matches (< 8 bytes) reduced by >= 25% compared to pure vector extract implementations.
- **SC-003**: 100% roundtrip test pass rate across standard compression test corpora (Silesia, Enwik8/9, Caltech).
- **SC-004**: Windows x86_64 Deflate streaming throughput improved by >= 250% compared to legacy scalar `zlib1.dll`.

## Assumptions

- Target architectures are ARM64 (macOS Apple Silicon, Windows on ARM) and x86_64 (Intel macOS, Windows x64).
- In-memory block compression will continue to prioritize `libdeflate` as the fast path for maximum single-core throughput.
- All hardware optimizations must adhere to Zlib / MIT permissive licensing.
