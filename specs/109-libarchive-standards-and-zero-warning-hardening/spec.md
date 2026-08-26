# Feature Specification: Libarchive-Inspired In-Process Native Test Log & Harness Architecture

**Feature ID**: `109-native-test-log-and-harness-architecture`  
**Status**: In-Progress  
**Target**: TTZip Core Test Infrastructure  

---

## 1. Problem Statement & Motivation

1. **Legacy Test Log Flaws**:
   - Default `swift test` prints 3 verbose lines per test case, resulting in 3,300+ lines of console noise across 1,100+ tests.
   - Using external Python scripts (`pretty_test.py`) to pipe and filter logs is an anti-pattern: it introduces external runtime dependencies, risks stdout buffer deadlock, and masks sub-process exit codes.
2. **Libarchive Inspiration**:
   - `libarchive` uses a 100% native C in-process test harness (`test_utils/test_main.c`):
     - **Silent on success, loud on failure**: Successful tests produce compact single-line indicators; failures immediately unfold with exact file, line, and assertion diffs.
     - **In-memory logging**: Debug/trace logs are buffered in memory and only dumped when an assertion fails.
     - **Structured Totals report**: Clear end-of-run executive summary.

---

## 2. Core Functional Requirements

### FR-01: Pure In-Process Test Observation (`TTZipTestObserver`)
- Implement Apple's native `XCTestObservation` protocol to intercept test lifecycle events within the running test process.
- Zero external Python / Shell filtering scripts required.

### FR-02: Silent-on-Success & Ring-Buffered Diagnostics
- During test execution, all engine debug/info logs must be captured into a 2,000-entry in-memory ring buffer (`TTLogger.startTestCapture()`).
- If a test passes, discard the buffer silently.
- If a test fails (`XCTIssue`), immediately dump the buffer to stderr with formatted `[DIAGNOSTIC TRACE]` headers and exact source file:line hyperlinks.

### FR-03: High-Density Suite-Level Console Reporting
- Print exactly **1 compact line per Test Suite** showing suite name, passed test count, skipped count, and millisecond execution time.
- Colorize timings: `< 100ms` dim gray, `>= 1.0s` yellow warning badge (`⚠️`).

### FR-04: Libarchive-Style End-of-Run Totals Dashboard
- Upon test bundle completion (`testBundleDidFinish`), print a structured summary table:
  ```text
  ================================================================================
  Totals:
    Test Suites:            86 passed, 0 failed, 86 total
    Tests:                1086 passed, 23 skipped, 1109 total
    Duration:             36.38s
    Status:               ALL TESTS PASSED (100% GREEN)
  ================================================================================
  ```

### FR-05: Fast-Tier Decoupling & Zero External Process Calls
- Unit tests must be 100% self-contained in-process. All multi-tool competitor PK benchmarks (`*PkTests.swift`) must require `TTZIP_RUN_BENCHMARKS=1`.
- Full `swift test` must execute in $\le 40$ seconds.

---

## 3. Success Criteria

1. **Zero External Script Execution**: `swift test` alone produces clean, high-density, formatted logs without piping into any Python script.
2. **Complete Failure Diagnostics**: Any intentional failure immediately prints the failing assertion, file path, line number, and in-memory trace.
3. **Execution Speed**: 1,100+ tests execute in $\le 40.0\text{s}$ wall time.
4. **Zero Warnings Gate**: Passes `swift build --build-tests -Xswiftc -warnings-as-errors` with 0 errors and 0 warnings.
