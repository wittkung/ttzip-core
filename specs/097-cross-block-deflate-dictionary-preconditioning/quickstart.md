# Quickstart: 097-cross-block-deflate-dictionary-preconditioning

## Scenario 1: Verify Cross-Block Dictionary Correctness & Ratio Gain
- **Command**: `swift test --filter CrossBlockDeflateDictionaryTests`
- **Expected Output**: 100% tests pass with verified ratio improvement.

## Scenario 2: Verify Hard Performance Floors
- **Command**: `swift test --filter XCTestPerformanceMeasureTests`
- **Expected Output**: All 13 throughput floors pass.
