# Quickstart: Strict Dual-Axis Pareto Superiority Verification

## Scenario 1: Verify 1v1 Pareto Duels Across All 4 Corpora
- **Command**: `TTZIP_FORCE_RERUN=1 TTZIP_RUN_BENCHMARKS=1 swift test --filter ZipSingleCoreParetoFrontierPkTests`
- **Expected Output**:
  - `Structured Logs & JSON: 100MB`: 100% strictly superior
  - `Binary & Machine Code: 100MB`: 100% strictly superior
  - `Mixed Modality Workspace: 100MB`: 100% strictly superior
  - `Text & Web: enwik8 100MB`: 100% strictly superior
  - 4 Retina PNG Pareto plots exported
- **Failure Diagnostic**: Check if any level experienced clock frequency downscaling or timer noise.

## Scenario 2: Verify Full 1,138 Test Suite
- **Command**: `swift test`
- **Expected Output**: `Executed 1138 tests, with 0 failures in < 30.0s`.
- **Failure Diagnostic**: Check buffer bounds or unaligned memory access in C bridge.

## Scenario 3: Verify 13 Hard Performance Gates
- **Command**: `swift test --filter XCTestPerformanceMeasureTests`
- **Expected Output**: `Executed 13 tests, with 0 failures in < 2.0s`.
- **Failure Diagnostic**: Ensure background load is clean.
