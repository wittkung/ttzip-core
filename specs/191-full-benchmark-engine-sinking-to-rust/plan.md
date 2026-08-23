# Implementation Plan: 191-full-benchmark-engine-sinking-to-rust

## Technical Context
- **Objective**: Sink the entire Benchmark Suite (50-point matrix, corpus generator, Pareto calculator, Fritsch-Carlson spline, SVG/HTML plotters, binary delta inspector) into Safe Rust and reduce `Sources/TTZipBench/` to a single $< 150\text{ LOC}$ runner.

---

## Constitution Check
- [x] **Rust Single Source of Truth**: 100% of benchmark computation in Rust.
- [x] **Zero Cloud Actions Quota**: 100% local validation.
- [x] **Cross-Platform Portability**: Linux/Windows headless benchmark support.

---

## Phase 0: Research Items
- R001 [SUBAGENT:research] 《Rust 原生多 Codec 基准与语料生成方案》: Completed.
- R002 [SUBAGENT:research] 《Rust 原生 Fritsch-Carlson 三次样条与 SVG/HTML 渲染》: Completed.

---

## Phase 1: Rust Benchmark Core Modules
- Write `rust/ttzip-glue/src/benchmark/runner.rs` (50-point matrix, synthetic corpus generator).
- Write `rust/ttzip-glue/src/benchmark/plotter.rs` (Fritsch-Carlson spline, SVG/HTML Pareto dashboards).
- Write `rust/ttzip-glue/src/benchmark/delta.rs` (Binary delta inspector).
- Export C-ABI in `rust/ttzip-glue/src/ffi/benchmark_ffi.rs`.

## Phase 2: Purge Swift Benchmark Code & Thin TTZipBench
- Delete `Sources/TTZipBench/Audit/`, `Sources/TTZipBench/Corpus/`, `Sources/TTZipBench/Engines/`, `Sources/TTZipBench/Models/`, `Sources/TTZipBench/Pareto/`, and helper files.
- Refactor `Sources/TTZipBench/main.swift` into a single, clean $< 150\text{ LOC}$ executable that calls Rust C-ABI.
- Refactor `Sources/TTZipApp/Views/Benchmark/BenchmarkEngine.swift` to directly delegate to Rust.

## Phase 3: Verification Plan
1. `swift run ttzip-bench gate` executes and passes cleanly.
2. `swift build` and `swift test` pass with 0 warnings.
3. `cargo test --workspace` passes cleanly.
4. `./scripts/run_local_ci_gate.sh` passes 100%.
