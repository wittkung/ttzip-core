# Tasks Breakdown: Historical Peak Gap Closure (Feature 019)

**Feature**: Historical Peak Gap Closure & Unified Fast-Path Alignment  
**Directory**: `specs/019-historical-peak-gap-closure-and-unified-fast-path/`  
**Status**: Ready for Implementation

---

## Phase 1: Fast-Path Routing & Entropy Bypass Upgrades

- [x] T001 [P] [US1] Remove hasDirectoryInput roadblock in Sources/TTZipCore/ArchiveWriter+Dispatch.swift to enable direct C engine for directory inputs
- [x] T002 [P] [US2] Implement fast entropy probe in Sources/TTZipCore/ArchiveWriter+Dispatch.swift to bypass CPU search on incompressible payloads
- [x] T003 [P] [US1] Optimize write_reg_file_data buffer streaming in Sources/CTTZipBridge/ttzip_tar_native.c

---

## Phase 2: Benchmarking & Peak Gap Closure

- [x] T004 [US1] Run TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests
- [x] T005 [US1] Run audit_performance_regression.py against peak matrix to evaluate closed gaps
- [x] T006 [US3] Verify XCTestPerformanceMeasureTests in Release mode

---

## Phase 3: Quality, Verification & Convergence

- [x] T007 [US3] Run full test suite swift test (591+ tests pass, 0 warnings)
- [x] T008 [US3] Finalize Feature 019 and commit to origin/main
