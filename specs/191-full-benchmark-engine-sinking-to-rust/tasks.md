# Tasks: 191-full-benchmark-engine-sinking-to-rust

## Phase 1: Rust Native Benchmark Suite (US1)
- [x] T001 [P] [US1] Write `rust/ttzip-glue/src/benchmark/runner.rs` (50-point matrix gate, multi-codec runner, synthetic corpus generator).
- [x] T002 [P] [US1] Write `rust/ttzip-glue/src/benchmark/plotter.rs` (Fritsch-Carlson cubic spline interpolation, SVG/HTML Pareto dashboard rendering).
- [x] T003 [P] [US1] Write `rust/ttzip-glue/src/benchmark/delta.rs` (Binary delta and section inspector).
- [x] T004 [P] [US1] Export C-ABI functions in `rust/ttzip-glue/src/ffi/benchmark_ffi.rs` and update `Sources/CTTZipBridge/include/ttzip_rust_glue.h`.

## Phase 2: Purge Swift Benchmark Files & Thin TTZipBench (US2)
- [x] T005 [P] [US2] Delete 15 redundant benchmark files in `Sources/TTZipBench/` and consolidate `Sources/TTZipBench/main.swift` into a single, clean $< 150\text{ LOC}$ runner.
- [x] T006 [P] [US2] Refactor `Sources/TTZipApp/Views/Benchmark/BenchmarkEngine.swift` to directly delegate to Rust C-ABI.

## Phase 3: CI Alignment & Final Verification (US3)
- [x] T007 [US3] Run `swift run ttzip-bench gate` and confirm $< 3\text{s}$ execution.
- [x] T008 [US3] Verify `swift build` and `swift test` 100% PASS with zero warnings.
- [x] T009 [US3] Run `cargo test --workspace` on all Rust crates.
- [x] T010 [US3] Run `./scripts/run_local_ci_gate.sh` full CI validation.
