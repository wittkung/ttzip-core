# Implementation Plan: Full-Matrix Regression & Pareto Convergence

**Feature**: `125-full-matrix-regression-and-pareto-convergence`
**Created**: 2026-08-19
**Status**: Ready for Implementation

---

## Technical Context

PR 6 brings the 6-PR single-core optimization sequence to final convergence:
1. Re-run `ZipSingleCoreParetoFrontierPkTests` on 100MB Silesia / Enwik8 corpus and mixed datasets.
2. Render updated 2x Retina Pareto PNG charts into `docs/benchmarks/` and conversation artifacts.
3. Execute the full test suite (`swift test`) across 525+ tests to assert 0 regressions.
4. Update benchmark summary documents.

---

## Constitution Check

- [x] **Zero-Regression Floor**: All 13 performance gates and format suites green.
- [x] **Reproducibility Standard**: 100% deterministic physical monotonic clock data.

---

## Proposed Changes

### Component 1: Benchmark Runner & Suite Execution

#### [MODIFY] [`Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift)
- Execute Pareto frontier shootout.

---

## Verification Plan

### Automated Tests
```bash
TTZIP_RUN_BENCHMARKS=1 swift test --filter ZipSingleCoreParetoFrontierPkTests
swift test --filter XCTestPerformanceMeasureTests
```
