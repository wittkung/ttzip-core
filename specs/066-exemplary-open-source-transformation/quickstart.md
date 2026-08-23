# Quickstart & Verification Guide: Exemplary Open-Source Project

## Scenario 1: Clean Clone & Full Build Verification
- **Command**:
  ```bash
  git clone https://github.com/wittkung/TTZip.git && cd TTZip && swift build
  ```
- **Expected Output**:
  ```text
  Building for debugging...
  [x/x] Compiling Swift modules...
  Build complete!
  ```
- **Failure Diagnostic**:
  Verify Xcode Command Line Tools are set to version 16.0+ via `xcode-select -p`.

---

## Scenario 2: Comprehensive Test Suite & Concurrency Verification
- **Command**:
  ```bash
  swift test
  ```
- **Expected Output**:
  ```text
  Test Suite 'All tests' passed at [timestamp].
  Executed 520+ tests, with 0 failures (0 unexpected).
  ```
- **Failure Diagnostic**:
  Ensure macOS Sonoma (14.0+) SDK is active.

---

## Scenario 3: Community Health & Policy File Assertion
- **Command**:
  ```bash
  test -f README.md && test -f LICENSE && test -f CONTRIBUTING.md && test -f SECURITY.md && test -f CODE_OF_CONDUCT.md && echo "HEALTH_CHECK_PASSED"
  ```
- **Expected Output**:
  ```text
  HEALTH_CHECK_PASSED
  ```
- **Failure Diagnostic**:
  Ensure all root community documents exist and are tracked by Git.
