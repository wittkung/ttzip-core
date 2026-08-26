# Quickstart: 191-full-benchmark-engine-sinking-to-rust

## Validation Scenarios

### Scenario 1: Rust Benchmark Suite
- **Command**: `cargo test --manifest-path rust/ttzip-glue/Cargo.toml -- benchmark`
- **Expected Output**: 100% tests PASS for matrix runner, spline, and SVG/HTML plotters.

### Scenario 2: Swift TTZipBench CLI Execution
- **Command**: `swift run ttzip-bench gate`
- **Expected Output**: 50-point matrix gate passes in $<3.0\text{s}$ using Rust micro-kernel.

### Scenario 3: Local CI/CD Gate
- **Command**: `./scripts/run_local_ci_gate.sh`
- **Expected Output**: 3/3 stages PASS in $<12\text{s}$.
