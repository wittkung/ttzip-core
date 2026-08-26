# Quickstart: 185-total-rust-microkernel-migration-and-c-swift-pruning

## Validation Scenarios

### Scenario 1: SPM Compilation Without Legacy C Sources
- **Command**: `swift build`
- **Expected Output**: Compiles swiftly without compiling zopfli, fast-lzma2, lzfse C source files.

### Scenario 2: Rust Workspace Tests
- **Command**: `cargo test --manifest-path rust/ttzip-glue/Cargo.toml && cargo test --manifest-path rust/ttzip-tui/Cargo.toml`
- **Expected Output**: All unit tests pass with 0 failures.

### Scenario 3: Local CI/CD Gate
- **Command**: `./scripts/run_local_ci_gate.sh`
- **Expected Output**: 7/7 stages PASS.
