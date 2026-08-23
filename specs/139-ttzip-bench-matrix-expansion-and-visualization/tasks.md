# Tasks: ttzip-bench Multi-Format Matrix Expansion & Interactive Visualizations

**Feature Directory**: `specs/139-ttzip-bench-matrix-expansion-and-visualization`  
**Status**: Completed  

---

## Phase 1: Codec Bridge & Unified Matrix Expansion (Priority: P1)

- [x] T001 [P] [US1] Expose Brotli and Bzip2 buffer memory compression wrappers in `Sources/CTTZipBridge/include/CTTZipStreamCoder.h` and `Sources/CTTZipBridge/CTTZipStreamCoder.c`
- [x] T002 [P] [US1] Integrate LZFSE, Snappy, Brotli, and Bzip2 execution points into `Sources/TTZipCore/Benchmark/TTZipCoreCodecBenchmarks.swift`
- [x] T003 [US1] Benchmark and verify full 74-point matrix execution in $< 1.8\text{ s}$ via `swift run ttzip-bench matrix`

---

## Phase 2: Standalone Zen UI HTML & Interactive SVG Pareto Visualizations (Priority: P1)

- [x] T004 [P] [US2] Implement standalone zero-dependency `HTMLParetoDashboardGenerator` in `Sources/TTZipBench/Plotters/HTMLParetoDashboardGenerator.swift`
- [x] T005 [P] [US2] Enhance `SVGParetoPlotter.swift` with dynamic tooltip CSS and Pareto rank convex hull styling in `Sources/TTZipBench/Plotters/SVGParetoPlotter.swift`
- [x] T006 [US2] Wire `--svg-out` and `--html-out` options into `Sources/TTZipBench/main.swift` under `plot` and `matrix` subcommands

---

## Phase 3: Benchmark Differential Tooling & CI Regression Gate (Priority: P2)

- [x] T007 [P] [US3] Implement `BenchmarkDiffCalculator` and regression status machine in `Sources/TTZipBench/main.swift`
- [x] T008 [US3] Add `ttzip-bench diff <baseline.json> <candidate.json>` subcommand with colorized table output and exit codes

---

## Phase 4: Full Suite Verification & CI Gate (Priority: P2)

- [x] T009 [US1] Update `Tests/TTZipTests/TTZipCoreCodecBenchmarkTests.swift` for 74-point coverage and schema assertion
- [x] T010 Execute `./scripts/run_local_ci_gate.sh` and verify all 6 stages pass
- [x] T011 Run `@speckit-converge` and `@speckit-analyze` consistency analysis
