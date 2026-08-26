# Quickstart Validation Guide: Full Codebase Remediation & Safety Verification

**Feature**: `specs/054-codebase-codereview`
**Created**: 2026-08-17
**Status**: Ready

---

## Scenario 1: CI Codebase Invariant Static Linting Gate

Validate that the codebase is 100% free of hardcoded `/Users/` paths, bare logging, hot-path `Data(count:)`, and `NSLock` inside `concurrentPerform`.

### Command
```bash
./scripts/lint_codebase_invariants.sh
```

### Expected Output
```text
[LINT] Scanning for hardcoded absolute /Users/ paths...
[LINT] OK: No hardcoded developer paths found.
[LINT] Scanning for unescaped bare logging (print, printf, NSLog)...
[LINT] OK: All production logging routes through TTLogger / ttzip_log.
[LINT] Scanning for hot-path Data(count:) kernel zero-fills...
[LINT] OK: Zero Data(count:) found in hot paths.
[LINT] Scanning for NSLock / blocking synchronization inside concurrentPerform...
[LINT] OK: All parallel loops use lock-free atomics.
[LINT] ✅ All codebase invariant checks passed (0 violations).
```

### Failure Diagnostic
If the script exits with non-zero code:
1. Examine printed file and line number violations.
2. For `/Users/` paths: replace with `Bundle.main.path` or `FileManager`.
3. For `print(...)`: replace with `TTLogger.debug(...)` or `TTLogger.info(...)`.
4. For `Data(count:)`: use `UnsafeMutablePointer<UInt8>.allocate` with `Data(bytesNoCopy:)`.
5. For `NSLock`: use `OSAtomicCompareAndSwap32Barrier` or `ManagedAtomic`.

---

## Scenario 2: Two-Way System Differential Oracle Verification

Verify bit-level interoperability between TTZip and native macOS utilities (`/usr/bin/unzip`, `/usr/bin/tar`).

### Command
```bash
swift test --filter SystemDifferentialTests
```

### Expected Output
```text
Test Suite 'SystemDifferentialTests' passed at ...
	 Executed 4 tests, with 0 failures (0 unexpected) in ... seconds
```

### Failure Diagnostic
If `SystemDifferentialTests` fails:
1. Check whether `/usr/bin/unzip -t` or `/usr/bin/tar -tf` reported corrupt headers.
2. Verify that `ArchiveWriter` emitted valid standard PKZIP central directories and POSIX ustar tar blocks.
3. Compare `payloadSha256Original` with `payloadSha256Extracted` to locate byte corruption offsets.

---

## Scenario 3: Golden Defect Corpus End-to-End Extraction

Verify that in-memory decoded `.uu` golden fixtures extract safely without crashing or throwing unexpected errors.

### Command
```bash
swift test --filter ArchiveGoldenCorpusTests
```

### Expected Output
```text
Test Suite 'ArchiveGoldenCorpusTests' passed at ...
	 Executed 2 tests, with 0 failures (0 unexpected) in ... seconds
```

### Failure Diagnostic
If `ArchiveGoldenCorpusTests` fails:
1. Ensure `Fixtures/GoldenCorpus/` contains valid `.uu` files.
2. Confirm `UUDecoder.decode(uuText:)` produced valid non-empty archive binary payloads.
3. Inspect `ArchiveExtractor` logs to check if a security gate rejected a valid test fixture.

---

## Scenario 4: Historical Peak Performance Gate & Zero-Regression Floor

Verify that all throughput baselines strictly achieve $\ge 90\%$ of the historical peak (`604d44d`).

### Command
```bash
swift test --filter XCTestPerformanceMeasureTests
swift test --filter PerformanceRegressionGuardTests
```

### Expected Output
```text
Test Suite 'XCTestPerformanceMeasureTests' passed at ...
	 Executed 13 tests, with 0 failures (0 unexpected) in ... seconds
Test Suite 'PerformanceRegressionGuardTests' passed at ...
	 Executed 3 tests, with 0 failures (0 unexpected) in ... seconds
```

### Failure Diagnostic
If performance gate tests fail:
1. Check if `floorRatio` dropped below 0.90 in `PerformanceRegressionGuardTests.swift`.
2. Inspect whether hot paths reintroduced intermediate heap allocations or lock contention.
3. Run `swift run ttzip-cli bench -f zip` and `swift run ttzip-cli bench -f 7z` to benchmark individual formats against historical floors.
