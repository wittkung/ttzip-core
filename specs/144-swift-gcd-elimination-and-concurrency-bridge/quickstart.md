# Quickstart & Verification Guide: 144-swift-gcd-elimination-and-concurrency-bridge

## Scenario 1: Verify Zero Apple GCD Invocations in Swift Layer

### Command
```bash
grep -rn "DispatchQueue\|DispatchSemaphore\|DispatchGroup" Sources/TTZipCore/ --include="*.swift" | grep -v "FileWatcherEngine.swift"
```

### Expected Output
```text
(Empty output / 0 matches)
```

### Failure Diagnostic
If matches are returned, inspect the matched lines and replace `DispatchQueue.concurrentPerform` with `ConcurrencyBridge.parallelFor`, `DispatchSemaphore` with direct synchronous execution, and `DispatchQueue.main.async` with `MainActor.run`.

---

## Scenario 2: Verify Parallel For and Hardware Budget Integrity

### Command
```bash
swift test --filter ConcurrencyBridgeTests
```

### Expected Output
```text
Test Suite 'ConcurrencyBridgeTests' passed.
Executed X tests, with 0 failures (0 unexpected).
```

### Failure Diagnostic
If tests fail, verify that `ttzip_threadpool_shared()` is initialized and `ParallelForBox` is correctly passed through `Unmanaged.passUnretained`.

---

## Scenario 3: Verify Full Regression & Matrix Suite

### Command
```bash
swift test --filter "AllFormatsAndAdvancedParametersMatrixTests|AllFormatDiagnosticSuiteTests"
```

### Expected Output
```text
Test Suite 'AllFormatDiagnosticSuiteTests' passed.
Test Suite 'AllFormatsAndAdvancedParametersMatrixTests' passed.
Executed 36 tests, with 0 failures (0 unexpected).
```

### Failure Diagnostic
If any archive format fails, verify buffer boundary offsets and ensure no race condition exists across chunk worker closures.
