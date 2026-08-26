# Quickstart & Verification Guide: Streamlining Redundant Swift Tests

**Feature**: `155-155-streamline-redundant`  
**Date**: 2026-08-20  
**Status**: Completed  

---

## 1. Verify Clean Swift Build and Test Target Compilation

### Validation Scenario 1: Zero Compiler Warnings on Test Target

- **Command**:
  ```bash
  swift build --build-tests
  ```
- **Expected Output**:
  - Exit code `0`
  - Zero compiler warnings or errors
  - `Build complete!`
- **Failure Diagnostic**:
  If a test file fails to compile due to missing symbols, check if any retained test file had an internal dependency on a pruned helper.

---

## 2. Verify CTest & Swift Test Suite Dual Execution

### Validation Scenario 2: Full Dual-Engine Local CI Run

- **Command**:
  ```bash
  ./scripts/local-ci.sh
  ```
- **Expected Output**:
  - `[1/5]` CMake build passed: `libttzip.a`, `ttzip-cli` & `ttzip_c_test_runner` ready.
  - `[2/5]` All C11 microkernel test suites passed 100% green (< 50ms).
  - `[3/5]` Standalone C CLI & C SDK quickstart verification passed.
  - `[4/5]` Zero-GCD audit passed: 0 Apple GCD calls in TTZipCore.
  - `[5/5]` Swift test suites passed 100% green.
  - `🎉 ALL LOCAL CI CHECKS PASSED SUCCESSFULLY (0 Quota)`
- **Failure Diagnostic**:
  Review the failed stage in `scripts/local-ci.sh`. If Stage 2 fails, run `./build/ttzip_c_test_runner all`. If Stage 5 fails, run `swift test --filter <failed_suite>`.
