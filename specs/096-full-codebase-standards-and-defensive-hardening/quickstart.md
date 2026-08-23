# Quickstart & Verification: 096-full-codebase-standards-and-defensive-hardening

## Scenario 1: Zero-Warning Compiler Verification
- **Command**: `swift build`
- **Expected Output**: `Build complete!` with 0 errors, 0 warnings.
- **Failure Diagnostic**: Inspect any compiler warning output and eliminate root cause.

## Scenario 2: SPDX Header Completeness Scan
- **Command**: `git grep -L "SPDX-License-Identifier" -- "Sources/**/*.swift" "Sources/**/*.c" "Sources/**/*.h"`
- **Expected Output**: Empty output (0 non-compliant files).
- **Failure Diagnostic**: Add standard SPDX header block to any reported file.

## Scenario 3: Hard Performance Floors
- **Command**: `swift test --filter XCTestPerformanceMeasureTests`
- **Expected Output**: All 13 performance throughput floors pass.
- **Failure Diagnostic**: Check CPU frequency or cacheline padding.

## Scenario 4: Full Test Suite Regression
- **Command**: `swift test`
- **Expected Output**: 525+ tests pass with 0 failures.
