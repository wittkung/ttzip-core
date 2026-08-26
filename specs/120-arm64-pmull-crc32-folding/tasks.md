# Implementation Tasks: ARM64 PMULL / CRC32 Multi-Way Folding & Cache Fusion

**Feature**: `120-arm64-pmull-crc32-folding`
**Created**: 2026-08-19

---

## Task Matrix

### Phase 1: Foundation & Header Interfaces

- [x] T001 [P] [US1] Expose clean C interfaces and function declarations in [`Sources/CTTZipBridge/include/CTTZipCRC32Neon.h`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/CTTZipCRC32Neon.h)

### Phase 2: C SIMD Kernel Implementation

- [x] T002 [US1] Implement Galois field folding constants and inline assembly helpers for `PMULL2` and `EOR3` in [`Sources/CTTZipBridge/CTTZipCRC32Neon.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipCRC32Neon.c)
- [x] T003 [US1] Implement 12-way (192-byte) primary vector folding loop and tree reduction in [`Sources/CTTZipBridge/CTTZipCRC32Neon.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipCRC32Neon.c)
- [x] T004 [US1] Implement 4-way intermediate loop, alignment prologue, and tail residue handling in [`Sources/CTTZipBridge/CTTZipCRC32Neon.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipCRC32Neon.c)
- [x] T005 [P] [US3] Implement scalar fallback path with zero compiler warnings in [`Sources/CTTZipBridge/CTTZipCRC32Neon.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipCRC32Neon.c)

### Phase 3: Comprehensive Test Suite & Floor Gate

- [x] T006 [P] [US3] Implement exhaustive differential oracle tests (0..1024 bytes x 0..15 alignments) in [`Tests/TTZipTests/CRC32PmullDifferentialTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/CRC32PmullDifferentialTests.swift)
- [x] T007 [P] [US1] Implement in-cache and large-buffer performance floor tests in [`Tests/TTZipTests/CRC32PmullDifferentialTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/CRC32PmullDifferentialTests.swift)

### Phase 4: Verification & Convergence

- [x] T008 [US1] Execute full test suite (`swift test`) and assert zero regression in [`Tests/TTZipTests/XCTestPerformanceMeasureTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/XCTestPerformanceMeasureTests.swift)

---

## Dependencies

- T001 -> T002 -> T003 -> T004 -> T005 -> T006 -> T007 -> T008
- T006 and T007 can execute in parallel once T005 is complete.
