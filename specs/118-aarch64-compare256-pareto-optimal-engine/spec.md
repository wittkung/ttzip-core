# Feature Specification: AArch64 Pareto-Optimal Zero-Regression compare256 Engine

**Feature Branch**: `118-aarch64-compare256-pareto-optimal-engine`

**Created**: 2026-08-19

**Status**: Draft

**Input**: User description: "我觉得有办法去结合的，好好去想，一定有的，可以全面不倒退，又保持这个提速效果 /speckit-specify"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Zero-Regression Short-Length Fast Path (0..15 Bytes) (Priority: P1)

As a compression engine consumer processing high-entropy or short-match data streams (such as uncompressed literals or random binary payloads), I need the memory comparison function (`compare256`) to terminate with minimal instruction latency (< 1 ns) on short-length mismatches (0..15 bytes) so that no CPU cycle regression occurs compared to the baseline implementation.

**Why this priority**: Short-length mismatches represent the majority of failed match candidate evaluations in LZ77 compressors (e.g. Deflate). Any micro-latency regression in this range can degrade compression speed on incompressible or literal-heavy files.

**Independent Test**: Can be validated via standalone microbenchmarks on match lengths 0, 1, 2, 4, 8, 10, 12, 14, 15 bytes under 0..63 byte rolling misalignment, asserting 0.0% regression against the upstream `develop` baseline.

**Acceptance Scenarios**:
1. **Given** two buffers differing at byte offset $0 \le k < 8$, **When** `compare256` is invoked, **Then** it returns index $k$ in $\le 0.75\text{ ns}$ without invoking horizontal vector reduction.
2. **Given** two buffers differing at byte offset $8 \le k < 16$, **When** `compare256` is invoked, **Then** it returns index $k$ in $\le 0.95\text{ ns}$ with exact parity or speedup against baseline.

---

### User Story 2 - High-Throughput Long-Match Vector Pipeline (32..256 Bytes) (Priority: P2)

As an archive creator compressing structured text, RGB image data, or repetitive binary streams, I need `compare256` to sustain maximum memory bandwidth throughput on long matches (32..256 bytes) so that compression throughput is improved by +20% to +50%.

**Why this priority**: Long match finding dominates CPU time on highly compressible data (e.g. text level 1/3, tarballs, RGB rasters).

**Independent Test**: Can be validated via microbenchmarks on match lengths 32, 48, 64, 80, 96, 128, 256 bytes and Deflate benchmarks on `text` and `striped_rgb`.

**Acceptance Scenarios**:
1. **Given** two buffers matching for $\ge 32$ bytes, **When** `compare256` is invoked, **Then** it advances through 32-byte chunks using dual-vector comparisons and a single vector reduction per iteration.
2. **Given** 64-byte or 128-byte matches, **When** benchmarked against baseline, **Then** latency is reduced by at least 25% (>= 1.3 ns saved per call).

---

### User Story 3 - Intermediate Match Transition (16..31 Bytes) (Priority: P3)

As a compression engine running mid-level compression (levels 3..6), I need seamless transition between the initial short probe and the long vector loop without transition penalties on 16..31 byte matches.

**Why this priority**: Ensures continuous Pareto dominance across all input lengths with zero boundary dips.

**Independent Test**: Can be validated by testing lengths 16, 20, 24, 28, 31 bytes under rolling misalignment.

**Acceptance Scenarios**:
1. **Given** two buffers matching for $16 \le k < 32$ bytes, **When** `compare256` is evaluated, **Then** execution completes in $\le 1.25\text{ ns}$, maintaining parity with baseline.

---

### Edge Cases

- **Unaligned Source Pointers**: `src0` and `src1` having independent, arbitrary byte misalignments (0..63 bytes modulo 64). Must execute safely without unaligned fault or performance cliff.
- **Identical 256-Byte Buffers**: Full 256-byte exact match must return exactly 256 without out-of-bounds reads.
- **Single Byte Difference at Boundary**: Mismatches exactly at index 0, 7, 8, 15, 16, 31, 32, 255 must return the exact mathematical index.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST compute the exact number of identical contiguous bytes between `src0` and `src1` up to a maximum limit of 256 bytes.
- **FR-002**: System MUST return identical bit-exact results to the reference linear byte-by-byte comparison oracle across all 257 possible match lengths (0..256) and all 16 memory misalignment combinations.
- **FR-003**: System MUST execute the 0..15 byte mismatch path with zero horizontal reduction vector instructions (`UMAXV`), avoiding multi-cycle subregister reduction stalls on early mismatch.
- **FR-004**: System MUST execute the 32..256 byte matching loop using 32-byte dual-vector loads with unified branch consolidation.
- **FR-005**: System MUST maintain 100% portable C fallback implementation for non-AArch64 ARM architectures (ARMv7 NEON).
- **FR-006**: System MUST pass all 71/71 zlib-ng standard CTest test suites with 0 compilation warnings under `-Wall -Wextra -Werror -Wshadow`.

### Key Entities

- **Match Offset**: An unsigned 32-bit integer in the range `[0, 256]` indicating the first mismatched byte index or 256 if identical.
- **Misalignment Delta**: The address difference `(intptr_t)src0 - (intptr_t)src1` used for single-base offset-addressed hardware loads.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: **Zero Microbenchmark Regression**: In the 0..128 byte microbenchmark (18 test lengths), 0 test lengths exhibit statistically significant latency regression ($> 3.0\%$) compared to the upstream `develop` baseline.
- **SC-002**: **Long-Match Speedup**: Microbenchmark latency for match lengths $\ge 64$ bytes is reduced by $\ge 30.0\%$ compared to baseline.
- **SC-003**: **Macro Deflate Speedup**: Full-matrix Deflate benchmark across 8 data types achieves $\ge +20.0\%$ compression throughput improvement on `text` Level 1/3 with 0 regressions across all 25 test points.
- **SC-004**: **Bit-Exact Correctness**: 100.0% pass rate across all 8,224 exhaustive dual-architecture test combinations.

## Assumptions

- Target hardware is AArch64 (ARMv8-A / ARMv9-A) with NEON SIMD support.
- Buffers `src0` and `src1` have at least 256 readable bytes allocated, or are safely padded according to zlib-ng window buffer invariants.

## Clarifications

### Session 2026-08-19
- **Q: How should the intermediate 16..31 byte range transition into the long vector loop?**
  - **Decision**: Inline a single 16B vector evaluation (Stage 3) before entering the unrolled loop, avoiding loop initialization overhead for 16..31 byte matches.
- **Q: Does ARMv7 NEON 32-bit require architecture-specific assembly?**
  - **Decision**: No, ARMv7 NEON continues to use portable intrinsics (`vld1q_u8`, `veorq_u8`), while AArch64 uses dedicated offset-addressed inline assembly for zero-cost pointer arithmetic.

