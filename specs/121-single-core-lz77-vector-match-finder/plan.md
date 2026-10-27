# Implementation Plan: Single-Core LZ77 Vector Match Finder & AArch64 SIMD

**Feature**: `121-single-core-lz77-vector-match-finder`
**Created**: 2026-08-19
**Status**: Ready for Implementation

---

## Technical Context

LZ77 match candidate search is the primary computational bottleneck in single-core Deflate compression. In `ttzip_deflate_fast.c`, the match finder used a 512KB hash table that overflowed Apple Silicon's 128KB L1 Data Cache. By:
1. Resizing the Tier 1 table to 64KB (4,096 2-way buckets), ensuring 100% L1 D-Cache residency.
2. Replacing scalar lane extraction in match length comparisons with a high-throughput SWAR/NEON vector pipeline.
3. Adding microsecond early entropy detection on high-entropy blocks.

Single-core Deflate match finding reaches $\ge 2,200\text{ MB/s}$ on Apple Silicon P-cores.

---

## Constitution Check

- [x] **Hot-Path Zero-Cost Abstraction**: 0 intermediate heap allocations (`malloc`/`free` = 0) inside the match finder loop.
- [x] **No Shared Locks**: Zero mutexes, semaphores, or locks.
- [x] **Fast-Path Preservation**: AArch64 NEON & SWAR accelerated match length comparison active on Apple Silicon with strict C11 portable scalar fallback.
- [x] **Deterministic Bounds**: RFC 1951 match length `[3, 258]` and offset `[1, 32768]` boundaries strictly preserved.

---

## Phase 0: Research Index

- - R001 [SUBAGENT:research] 《L1 D-Cache Table Sizing & Direct 2-Way Hash Topology on Apple Silicon》：Evaluated 64KB (4K x 2) vs 512KB (32K x 2) hash table sizes, determining that 64KB fits 100% in L1 D-cache with 0 L2 cache miss penalties.
- - R002 [SUBAGENT:research] 《Zero-Stall Vector Match Comparison via Dual 64-bit SWAR & 128-bit NEON》：Formulated dual 64-bit SWAR probe for 0..15 bytes with 128-bit NEON unrolled vector loop for long matches.

See [`research.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/121-single-core-lz77-vector-match-finder/research.md) for details.

---

## Phase 1: Data Model & Contracts Index

- **Data Model**: [`data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/121-single-core-lz77-vector-match-finder/data-model.md)
- **Contracts**:
  - [`contracts/lz77_match_finder_request.schema.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/121-single-core-lz77-vector-match-finder/contracts/lz77_match_finder_request.schema.json)
  - [`contracts/lz77_match_finder_response.schema.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/121-single-core-lz77-vector-match-finder/contracts/lz77_match_finder_response.schema.json)
- **Quickstart Guide**: [`quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/121-single-core-lz77-vector-match-finder/quickstart.md)

---

## Proposed Changes

### Component 1: C Bridge Layer (`Sources/CTTZipBridge`)

#### [MODIFY] [`ttzip_deflate_engine.h`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.h)
- Update `ttzip_deflate_fast_mf_t` structure definition to 4,096 2-way buckets (64KB aligned to 64-byte cache line).

#### [MODIFY] [`ttzip_deflate_fast.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/native_deflate/ttzip_deflate_fast.c)
- Implement 64-bit SWAR + 128-bit NEON zero-stall match length comparison.
- Implement 12-bit multiplicative hash (`ttzip_hash4`).
- Implement 64KB L1-resident 2-way hash table traversal.
- Implement early entropy detection for incompressible data short-circuiting.

---

### Component 2: Swift Core & Test Suite (`Tests/TTZipTests`)

#### [NEW] [`LZ77VectorMatchFinderTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/LZ77VectorMatchFinderTests.swift)
- Exhaustive match length oracle test (0..258 bytes x 0..15 alignments).
- Tier 1 match finder throughput benchmark ($\ge 2,200\text{ MB/s}$).
- Entropy short-circuiting benchmark on incompressible payloads ($\ge 4,500\text{ MB/s}$).

---

## Verification Plan

### Automated Tests
1. **Match Length Oracle Test**:
   ```bash
   swift test --filter LZ77VectorMatchFinderTests/testMatchLengthVectorOracle
   ```
2. **Tier 1 Throughput Gate Test**:
   ```bash
   swift test --filter LZ77VectorMatchFinderTests/testTier1MatchFinderThroughputFloor
   ```
3. **Full Regression Test Suite**:
   ```bash
   swift test --filter XCTestPerformanceMeasureTests
   ```
