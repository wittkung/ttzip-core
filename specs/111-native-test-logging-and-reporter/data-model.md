# Data Model: Native High-Aesthetic Test Logging & Reporter (111-native-test-logging-and-reporter)

## Entities & Type Definitions

### 1. `TestLogLevel` (Enum)
Defines the verbosity filtering threshold for test logging:
- `silent` (rawValue: 0): Only emitted when test fails or gate fails.
- `normal` (rawValue: 1): Progress badges, suite headers, and failure diagnostics.
- `verbose` (rawValue: 2): Detailed step-by-step assertions and subsystem timings.
- `debug` (rawValue: 3): Low-level hex dumps, raw pointer traces, SIMD register diagnostics.

### 2. `TestBadgeType` (Enum)
Defines the standardized visual category badges:
- `pass`: Passed test / suite (Emerald bold).
- `fail`: Failed test / suite (Crimson bold).
- `skip`: Skipped benchmark / test (Amber bold).
- `standards`: Standards compliance suite (Cyan bold).
- `oracle`: Differential oracle test against system tools (Magenta bold).
- `fuzz`: Stream mutation fuzzing test (Lemon yellow).
- `perf`: Hardware & throughput performance gate (Kintsugi Gold bold).
- `run`: Ongoing execution indicator (Blue bold).
- `info`: General informational badge (White bold).

### 3. `TestExecutionRecord` (Struct)
Represents the discrete outcome of an executed test method or diagnostic block:
- `sessionID` (String): Unique UUID identifying the test execution batch.
- `sequenceNumber` (Int): 1-indexed sequence position in the overall test run.
- `suiteName` (String): Fully qualified name of the test suite (e.g., `ArchiveMutationFuzzTests`).
- `caseName` (String): Method or diagnostic case identifier (e.g., `testCorruptCRCMutationStability`).
- `badge` (TestBadgeType): Category badge assigned to this execution.
- `status` (String): Exact status code: `"passed" | "failed" | "skipped"`.
- `durationMs` (Double): Execution duration in milliseconds with floating-point microsecond precision.
- `deferredLogs` (Array<String>): Buffered log entries emitted during execution.
- `failureContext` (Optional<TestFailureContext>): Populated only when `status == "failed"`.

### 4. `TestFailureContext` (Struct)
Detailed structured failure diagnostic card:
- `sourceFile` (String): Absolute path or relative path to the test source file.
- `lineNumber` (Int): 1-indexed line number of the failing assertion.
- `assertionMessage` (String): Failure reason provided to assertion.
- `expectedValue` (Optional<String>): Stringified expected outcome.
- `actualValue` (Optional<String>): Stringified actual outcome.
- `hexDiffSnippet` (Optional<String>): Visual 16-byte side-by-side hex diff if binary comparison failed.
- `unicodeDiagnostic` (Optional<String>): NFD/NFC scalar decomposition if string comparison failed.

### 5. `TestRunSummary` (Struct)
Aggregated execution summary report:
- `sessionID` (String): Unique test run session identifier.
- `totalSuites` (Int): Number of executed test suites.
- `totalCases` (Int): Total number of discrete test cases.
- `passedCases` (Int): Number of successfully passed cases.
- `failedCases` (Int): Number of failed test cases.
- `skippedCases` (Int): Number of skipped test cases.
- `passRatePercent` (Double): Computed pass percentage (e.g., `100.0`).
- `wallClockDurationMs` (Double): Total wall clock elapsed time in milliseconds.
- `architecture` (String): Target CPU architecture (e.g., `arm64e`, `x86_64`).
- `osVersion` (String): Host macOS release (e.g., `macOS 14.7.1`).
- `timestamp` (String): ISO 8601 formatted UTC timestamp.
