# Quickstart & Verification: 095-comprehensive-systems-engineering-evolution

## Scenario 1: Compiler Flags & Warning Cleanliness
- **Command**: `swift build -Xswiftc -warnings-as-errors`
- **Expected Output**: `Build complete!` with 0 warnings, 0 missing prototypes.
- **Failure Diagnostic**: If warning occurs, check `cSettings` in `Package.swift` or missing forward declarations in `.h`.

## Scenario 2: Multi-Way Differential Consensus Oracle
- **Command**: `swift test --filter DifferentialOracleTests`
- **Expected Output**: Executed all tests with 0 failures, validating consensus against `/usr/bin/tar`, `/usr/bin/unzip`, `/usr/bin/ditto`.
- **Failure Diagnostic**: Inspect divergent byte offsets in `DifferentialOracleTestHarness.swift` logs.

## Scenario 3: Defensive Overflow & Struct Magic Verification
- **Command**: `swift test --filter AlgorithmicOptimizationBenchmarkTests,CoreEngineExhaustiveCoverageTests`
- **Expected Output**: All magic checks and arithmetic overflow bounds pass with 0 errors.
- **Failure Diagnostic**: Verify `0xDEADBEEF` poison sentinel in `CTTZipCommon.h`.

## Scenario 4: Hard Performance Floors
- **Command**: `swift test --filter XCTestPerformanceMeasureTests`
- **Expected Output**: All 13 performance gates pass with $\ge$ baseline throughput.
- **Failure Diagnostic**: Check CPU frequency scaling or background processes.
