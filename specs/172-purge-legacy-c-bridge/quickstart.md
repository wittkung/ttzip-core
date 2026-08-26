# Quickstart & Verification Guide: Feature 172

## Scenario 1: Clean Build Verification
- **Command**: `swift build`
- **Expected Output**: Builds successfully with 0 warnings in < 3 seconds.
- **Failure Diagnostic**: Check if any deleted C symbols are still referenced by Swift files.

## Scenario 2: Swift 6 Full Test Suite
- **Command**: `swift test`
- **Expected Output**: 859 tests passed, 0 failures, 2 skipped.
- **Failure Diagnostic**: Check test logs for missing symbol errors or assertions.

## Scenario 3: 7-Stage Local CI/CD Gate
- **Command**: `./scripts/run_local_ci_gate.sh`
- **Expected Output**: All 7 stages PASS (Total ~10s).
- **Failure Diagnostic**: Check `scripts/run_local_ci_gate.sh` output for stage failure details.
