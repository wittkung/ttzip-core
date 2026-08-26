# Quickstart & Verification Guide: 080-test-suite-acceleration-and-optimization

## Scenario 1: ArchiveMutationFuzzTests Acceleration & Integrity Verification

### Command
```bash
swift test --filter ArchiveMutationFuzzTests
```

### Expected Output
```text
Test Suite 'ArchiveMutationFuzzTests' passed at ...
	 Executed 7 tests, with 0 failures (0 unexpected) in 0.xxx seconds
```
- Execution duration: **<= 8.0 seconds** (target < 1.0s).
- 0 failures, 0 errors, 0 memory corruption.

### Failure Diagnostic
If the test takes > 8.0 seconds or throws failures:
1. Verify that `ArchiveMutationFuzzTests.swift` uses `withThrowingTaskGroup` for concurrent format iterations.
2. Check if temporary disk writes are active instead of in-memory buffer validation.

---

## Scenario 2: Concurrency & Synchronization Tests Speedup Verification

### Command
```bash
swift test --filter StrategyPatternTests/testRound3MultiCoreBruteForce100PlusTasksGroupCancellationSafety
swift test --filter RepositoryPatternTests/testHighConcurrency100ThreadsPasswordRepositoryReadWrite
```

### Expected Output
```text
Test Case '-[TTZipTests.StrategyPatternTests testRound3MultiCoreBruteForce100PlusTasksGroupCancellationSafety]' passed (0.020 seconds).
Test Case '-[TTZipTests.RepositoryPatternTests testHighConcurrency100ThreadsPasswordRepositoryReadWrite]' passed (0.100 seconds).
```
- Total execution time for both tests: **<= 0.3 seconds** (previously ~11.0s).

### Failure Diagnostic
If `testRound3MultiCoreBruteForce100PlusTasksGroupCancellationSafety` hangs or takes > 1.0s:
1. Check for leftover `Task.sleep` calls in the password verification closure.
2. Ensure child task cancellation signal propagates immediately via `Task.isCancelled`.

---

## Scenario 3: Full Test Suite Regression (10x Speedup)

### Command
```bash
swift test
```

### Expected Output
```text
Test Suite 'All tests' passed at ...
	 Executed 883+ tests, with 0 failures (0 unexpected) in <= 20.0 seconds
```
- Full suite runtime: **<= 20.0 seconds** (down from 116.76s baseline).
- All 883+ test cases green.

### Failure Diagnostic
If any test fails or runtime exceeds 20.0 seconds:
1. Parse test durations using `python3 -c "import re; ..."` to locate any lingering unoptimized suite.
2. Assert zero dropped assertions.
