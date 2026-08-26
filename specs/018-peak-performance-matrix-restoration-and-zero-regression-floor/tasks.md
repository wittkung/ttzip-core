# Tasks Breakdown: Restoration Against Historical Peak Matrix (Feature 018)

**Feature**: Restoration Against Historical Peak Matrix & Hard 10% Floor Invariant  
**Directory**: `specs/018-peak-performance-matrix-restoration-and-zero-regression-floor/`  
**Status**: Ready for Implementation

---

## Phase 1: Engine & Runner Upgrades

- [x] T001 [P] [US1] Set Lzip compression-level to 1 across all levels in Sources/CTTZipBridge/ttzip_tar_native.c
- [x] T002 [P] [US2] Add thermal cooldown pause (usleep) in loop in Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner.swift
- [x] T003 [US2] Add peak matrix direct parser support to scripts/audit_performance_regression.py

---

## Phase 2: User Story 1 - 恢复全格式历史最高峰值 (Priority: P1) 🎯 MVP

- [x] T004 [US1] Run Release benchmark suite via TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests
- [x] T005 [US1] Run audit_performance_regression.py against peak matrix and verify zero critical regression (>10%) in docs/benchmarks/latest_regression_audit.md
- [x] T006 [US1] Run XCTestPerformanceMeasureTests in Release mode

---

## Phase 3: User Story 3 - 质量与单测合规验证 (Priority: P3)

- [x] T007 [US3] Run full regression test suite via swift test (591+ tests pass)
- [x] T008 [US3] Verify strict TTLogger compliance across modified files

---

## Phase 4: Polish, Convergence & Finalization

- [x] T009 [P] Update benchmark documentation in docs/benchmarks/
- [x] T010 Run speckit-analyze consistency scan and finalize feature 018
