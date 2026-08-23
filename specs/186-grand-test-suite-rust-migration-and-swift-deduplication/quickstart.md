# Quickstart: 186-grand-test-suite-rust-migration-and-swift-deduplication

## Validation Scenarios

### Scenario 1: Rust Workspace Tests
- **Command**: `cargo test --workspace`
- **Expected Output**: All unit and integration test suites PASS in $<2\text{s}$.

### Scenario 2: Streamlined Swift Tests
- **Command**: `swift test`
- **Expected Output**: 100% PASS with reduced runtime.

### Scenario 3: Local CI/CD Gate
- **Command**: `./scripts/run_local_ci_gate.sh`
- **Expected Output**: 7/7 stages PASS.
