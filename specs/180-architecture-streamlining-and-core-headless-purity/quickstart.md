# Quickstart: 180-architecture-streamlining-and-core-headless-purity

## Validation Scenarios

### Scenario 1: Rust Workspace Tests
- **Command**: `cargo test --manifest-path rust/ttzip-glue/Cargo.toml && cargo test --manifest-path rust/ttzip-tui/Cargo.toml`
- **Expected Output**: 175+ tests pass with 0 failures.

### Scenario 2: Swift Full Suite
- **Command**: `swift test`
- **Expected Output**: 880+ tests pass with 0 failures.

### Scenario 3: Local CI/CD Gate
- **Command**: `./scripts/run_local_ci_gate.sh`
- **Expected Output**: 7/7 stages PASS.
