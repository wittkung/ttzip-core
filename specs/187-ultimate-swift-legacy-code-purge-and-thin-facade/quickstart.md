# Quickstart: 187-ultimate-swift-legacy-code-purge-and-thin-facade

## Validation Scenarios

### Scenario 1: Clean Lean Swift Build
- **Command**: `swift build`
- **Expected Output**: Compiles swiftly in $<1.0\text{s}$ with $< 35$ files.

### Scenario 2: Rust Full Workspace Tests
- **Command**: `cargo test --workspace`
- **Expected Output**: 100% PASS across all crates and integration tests.

### Scenario 3: Local CI/CD Gate
- **Command**: `./scripts/run_local_ci_gate.sh`
- **Expected Output**: 7/7 stages PASS.
