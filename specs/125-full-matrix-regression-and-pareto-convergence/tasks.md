# Implementation Tasks: Full-Matrix Regression & Pareto Convergence

**Feature**: `125-full-matrix-regression-and-pareto-convergence`
**Created**: 2026-08-19

---

## Task Matrix

### Phase 1: Benchmark & Pareto Chart Generation

- [x] T001 [US1] Run `ZipSingleCoreParetoFrontierPkTests` to generate updated 1v1 Pareto charts on 100MB corpus.

### Phase 2: Full Regression & Convergence

- [x] T002 [US1] Run full regression tests (`swift test --filter XCTestPerformanceMeasureTests`) to verify 0 regressions.
- [x] T003 [US1] Run full test suite (`swift test`) across all modules.

---

## Dependencies

- T001 -> T002 -> T003
