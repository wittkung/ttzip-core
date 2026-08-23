# Quickstart: 190-domain-responsibility-realignment-and-core-lightweighting

## Validation Scenarios

### Scenario 1: Clean Core Compilation
- **Command**: `swift build`
- **Expected Output**: Compiles swiftly with zero warnings.

### Scenario 2: Swift E2E Tests
- **Command**: `swift test`
- **Expected Output**: 100% PASS in $<1.0\text{s}$.

### Scenario 3: Local CI/CD Gate
- **Command**: `./scripts/run_local_ci_gate.sh`
- **Expected Output**: 3/3 stages PASS in $<5\text{s}$.
