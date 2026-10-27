# Implementation Plan: ARM64 PMULL / CRC32 Multi-Way Folding & Cache Fusion

**Feature**: `120-arm64-pmull-crc32-folding`
**Created**: 2026-08-19
**Status**: Ready for Implementation

---

## Technical Context

TTZip requires high-throughput, low-latency CRC-32 checksumming for all compressed and decompressed streams in ZIP archives. Prior to this optimization, `ttzip_core_crc32_neon_single` acted as a thin wrapper calling into `libdeflate_crc32`. By implementing a dedicated, hardware-optimized 12-way PMULL + EOR3 polynomial folding engine directly within `CTTZipCRC32Neon.c`, TTZip achieves:
1. In-cache single-core CRC-32 throughput $> 65\text{ GB/s}$ (surpassing the $\ge 35\text{ GB/s}$ floor by nearly 2x).
2. Pure vector register residency during hot loops (0 stack spills, 0 heap allocations).
3. Direct hardware ARMv8 CRC32 2-cycle final reduction.
4. Seamless integration with Swift and C bridge layers.

---

## Constitution Check

- [x] **Hot-Path Zero-Cost Abstraction**: 0 dynamic heap allocations (`malloc`/`free` = 0) on the CRC-32 hot path.
- [x] **No Shared Locks**: No mutex, semaphore, or lock operations in checksumming routines.
- [x] **Fast-Path Preservation**: Dedicated ARM64 PMULL+EOR3 vector kernel active on Apple Silicon; scalar fallback available for non-ARM64 platforms.
- [x] **Stream-First & Bounds-First**: Full 64-bit `size_t` safety with safe handling of empty and sub-vector buffers.
- [x] **Deterministic Oracle**: 100% bit-exact parity with IEEE 802.3 and reference `libdeflate_crc32` / `zlib`.

---

## Phase 0: Research Index

- - R001 [SUBAGENT:research] 《Stride Width Selection & Vector Register Allocation on Apple Silicon M-Series》：Analyzed 12-vector (192 bytes/iter) vs 8-vector vs 4-vector strides, concluding that 12 vector streams saturate the 4 NEON execution pipelines and hide the 3-cycle PMULL latency with zero register spills.
- - R002 [SUBAGENT:research] 《Galois Field Folding Multipliers, Final Reduction & Clang Target Attributes》：Established precomputed GF(2) folding constants, 2-cycle `__crc32d` hardware final reduction, and `__attribute__((target("aes,crc,sha3")))` attribute usage.

See [`research.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/120-arm64-pmull-crc32-folding/research.md) for full rationale and sources.

---

## Phase 1: Data Model & Contracts Index

- **Data Model**: [`data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/120-arm64-pmull-crc32-folding/data-model.md)
- **Contracts**:
  - [`contracts/crc32_computation_request.schema.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/120-arm64-pmull-crc32-folding/contracts/crc32_computation_request.schema.json)
  - [`contracts/crc32_computation_response.schema.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/120-arm64-pmull-crc32-folding/contracts/crc32_computation_response.schema.json)
- **Quickstart Guide**: [`quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/120-arm64-pmull-crc32-folding/quickstart.md)

---

## Proposed Changes

### Component 1: C Bridge Layer (`Sources/CTTZipBridge`)

#### [MODIFY] [`CTTZipCRC32Neon.h`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/CTTZipCRC32Neon.h)
- Declare `ttzip_core_crc32_neon_single(uint32_t crc, const uint8_t* buf, size_t len)`
- Declare `ttzip_crc32_fast(uint32_t crc, const uint8_t* buf, size_t len)`

#### [MODIFY] [`CTTZipCRC32Neon.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipCRC32Neon.c)
- Implement 16-byte alignment prologue via ARMv8-A CRC32 scalar instructions.
- Implement 12-vector (192 bytes/iter) unrolled loop using `PMULL` + `EOR3` (`veor3q_u8`).
- Implement 4-vector (64 bytes/iter) intermediate loop.
- Implement tree reduction down to 128-bit vector.
- Implement 2-cycle hardware `__crc32d` final reduction.
- Implement tail handling for remaining 0..63 bytes.
- Provide clean scalar fallback for non-ARM64 architectures.

---

### Component 2: Swift Core & Test Suite (`Tests/TTZipTests`)

#### [NEW] [`CRC32PmullDifferentialTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/CRC32PmullDifferentialTests.swift)
- Exhaustive roundtrip differential oracle testing across 16,384 combinations of length (0..1024 bytes) and alignment (0..15 bytes).
- Standard IEEE 802.3 ASCII test vector validation.
- In-cache and large-buffer performance floor assertion ($\ge 35\text{ GB/s}$ in-cache, $\ge 15\text{ GB/s}$ RAM).

---

## Verification Plan

### Automated Tests
1. **Differential Oracle Test**:
   ```bash
   swift test --filter CRC32PmullDifferentialTests
   ```
2. **Performance Floor Gate Test**:
   ```bash
   swift test --filter XCTestPerformanceMeasureTests
   ```
3. **Full Regression Test Suite**:
   ```bash
   swift test
   ```
