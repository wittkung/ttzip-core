# Tasks: LZ4 Deep Integration and Performance Verification

**Feature**: `064-lz4-deep-integration`
**Created**: 2026-08-17
**Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/064-lz4-deep-integration/spec.md) | **Plan**: [plan.md](file:///Users/kevintung/Documents/dev/TTZip/specs/064-lz4-deep-integration/plan.md)

---

## Phase 1: Engine Migration & Acceleration Alignment
- [x] T001 [P] [US1] Eliminate `<compression.h>` in `Sources/CTTZipBridge/CTTZipStreamCoder.c` and bind native `LZ4_compress_fast` / `LZ4_decompress_safe`
- [x] T002 [US1] Verify `LZ4LzoEngine` dynamic `acceleration` passthrough in `Sources/TTZipCore/ProfessionalAlgorithmsSuite.swift`

## Phase 2: Performance Audit & Regression Verification
- [x] T003 [P] [US2] Execute LZ4 hard throughput floor benchmark in `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift`
- [x] T004 [P] [US2] Execute TAR.LZ4 matrix regression in `Tests/TTZipTests/AllFormatsAndAdvancedParametersMatrixTests.swift`
- [x] T005 [US2] Execute InMemory benchmark suite in `Tests/TTZipTests/InMemoryBenchmarkSuiteTests.swift`
- [x] T006 [US2] Output 4-step performance optimization differential audit table in response
