# Quickstart & Verification Guide: Codebase Copyright & English Translation

## Scenario 1: Execute Automated Header Injection & Translation
- **Command**:
  ```bash
  python3 scripts/internationalize_codebase.py
  ```
- **Expected Output**:
  ```text
  [SUCCESS] Processed 470+ files. Injected SPDX headers and translated all Chinese comments.
  ```

---

## Scenario 2: Verify Zero Chinese in Comments
- **Command**:
  ```bash
  python3 scripts/assert_zero_chinese.py
  ```
- **Expected Output**:
  ```text
  [PASS] Zero Chinese characters detected in codebase (outside whitelisted test fixtures).
  ```

---

## Scenario 3: Full Compilation & Unit Test Regression Gate
- **Command**:
  ```bash
  swift test
  ```
- **Expected Output**:
  ```text
  Test Suite 'All tests' passed at [timestamp].
  Executed 520+ tests, with 0 failures (0 unexpected).
  ```
