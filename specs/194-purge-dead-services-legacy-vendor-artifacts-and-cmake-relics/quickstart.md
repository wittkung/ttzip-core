# Quickstart: 194-purge-dead-services-legacy-vendor-artifacts-and-cmake-relics

## Validation Scenarios

### Scenario 1: Clean SwiftPM Build
- **Command**: `swift build --build-tests`
- **Expected Output**: 100% PASS with 0 warnings.

### Scenario 2: Swift E2E Tests
- **Command**: `swift test`
- **Expected Output**: 100% PASS in $<5.0\text{s}$.

### Scenario 3: Local CI/CD Gate
- **Command**: `./scripts/run_local_ci_gate.sh`
- **Expected Output**: 4/4 stages PASS in $<10\text{s}$.
