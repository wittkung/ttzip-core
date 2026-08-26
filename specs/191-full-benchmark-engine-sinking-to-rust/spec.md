# Feature Specification: 191-full-benchmark-engine-sinking-to-rust

## 1. Executive Summary & Strategic Motivation
The user correctly identified that benchmarking logic (matrix runners, Pareto convex hull, Fritsch-Carlson cubic spline curve interpolation, SVG/HTML plotters, binary delta auditing, and corpus generators) was still implemented in Swift across `Sources/TTZipBench/` (16 files) and `Sources/TTZipApp/Views/Benchmark/`.

To ensure 100% cross-platform parity (macOS, Linux, Windows), nanosecond-precision monotonic timing, and zero Swift runtime overhead, **all benchmark computation and report plotting must sink completely into Safe Rust (`rust/ttzip-glue/src/benchmark/`)**. Swift `TTZipBench` will be reduced to a $< 150\text{ LOC}$ CLI runner that simply invokes Rust C-ABI.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Native High-Performance Benchmark Execution
- **Given** running `ttzip-bench gate` or `ttzip-bench matrix`
- **When** the benchmark executes
- **Then** all 50-point matrix measurements, synthetic corpus generation, and throughput calculations are executed directly in multi-threaded Safe Rust (Rayon).

### User Scenario 2: Cross-Platform SVG & HTML Pareto Plotting
- **Given** generating Pareto visualization dashboards
- **When** `--svg` or `--html` options are passed
- **Then** SVG vectors, Bézier trajectory curves, and HTML tables are generated 100% in Rust without Swift string concatenation overhead.

---

## 3. Success Metrics
1. **Full Rust Engine**: Implement runner, plotter, delta, and corpus generator in `rust/ttzip-glue/src/benchmark/`.
2. **Swift Thinning**: Reduce `Sources/TTZipBench/` from 16 files to a single, ultra-thin `main.swift` ($< 150\text{ LOC}$).
3. **Zero Regression**: 100% pass rate on `cargo test`, `swift test`, and `./scripts/run_local_ci_gate.sh`.
