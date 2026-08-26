# Implementation Tasks: Single-Core LZ77 Vector Match Finder & AArch64 SIMD

**Feature**: `121-single-core-lz77-vector-match-finder`
**Created**: 2026-08-19

---

## Task Matrix

### Phase 1: Header Definitions

- [x] T001 [P] [US1] Update `ttzip_deflate_fast_mf_t` structure definition to 4,096 2-way entries in [`Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.h`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.h)

### Phase 2: C Kernel Implementation

- [x] T002 [US2] Implement optimized SWAR + NEON `ttzip_fast_match_len_arm64` in [`Sources/CTTZipBridge/native_deflate/ttzip_deflate_fast.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/native_deflate/ttzip_deflate_fast.c)
- [x] T003 [US1] Implement 64KB L1-cache hash table traversal and 12-bit `ttzip_hash4` in [`Sources/CTTZipBridge/native_deflate/ttzip_deflate_fast.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/native_deflate/ttzip_deflate_fast.c)
- [x] T004 [P] [US3] Implement early entropy sampling and incompressible short-circuiting in [`Sources/CTTZipBridge/native_deflate/ttzip_deflate_fast.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/native_deflate/ttzip_deflate_fast.c)

### Phase 3: Comprehensive Test Suite & Benchmark

- [x] T005 [P] [US2] Implement match length oracle differential test in [`Tests/TTZipTests/LZ77VectorMatchFinderTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/LZ77VectorMatchFinderTests.swift)
- [x] T006 [P] [US1] Implement Tier 1 throughput benchmark test in [`Tests/TTZipTests/LZ77VectorMatchFinderTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/LZ77VectorMatchFinderTests.swift)

### Phase 4: Verification & Convergence

- [x] T007 [US1] Run full test suite (`swift test --filter XCTestPerformanceMeasureTests`) to assert zero regression across all formats.

---

## Dependencies

- T001 -> T002 -> T003 -> T004 -> T005 -> T006 -> T007
- T005 and T006 can execute concurrently once T004 is completed.
