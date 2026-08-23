# Quickstart: Pointwise Pareto Dominance Verification

## Scenario 1: Verify Single-Core 1v1 Duels Against libdeflate
- **Command**: `TTZIP_RUN_BENCHMARKS=1 swift test --filter ZipSingleCoreParetoFrontierPkTests`
- **Expected Output**:
  - `testTTZipVsLibdeflate1v1Duel_Structured_JSON` passed with 100% pointwise dominance
  - `testTTZipVsLibdeflate1v1Duel_Binary_Executables` passed with 100% pointwise dominance
  - `testTTZipVsLibdeflate1v1Duel_Mixed_Compound100MB` passed with 100% pointwise dominance
  - `testTTZipVsLibdeflate1v1Duel` (enwik8) passed with 100% pointwise dominance
  - 4 high-resolution Retina PNG Pareto charts generated in brain artifact directory
- **Failure Diagnostic**: Check individual tier speed and size printouts to isolate which tier dropped below the competitor envelope.

## Scenario 2: Verify Full 1138 Regression Suite
- **Command**: `swift test`
- **Expected Output**: `Executed 1138 tests, with 0 failures in < 30.0s`.
- **Failure Diagnostic**: If any unit test fails, run with `--filter <FailingSuite>` to diagnose buffer bounds or CRC checksums.

## Scenario 3: Verify 13 Hard Performance Measure Floors
- **Command**: `swift test --filter XCTestPerformanceMeasureTests`
- **Expected Output**: `Executed 13 tests, with 0 failures in < 2.0s`.
- **Failure Diagnostic**: Verify hardware CPU frequency scaling or background process throttling.
