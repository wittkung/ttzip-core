# Quickstart & Verification Guide: Expanded C Test Harness

**Feature**: `156-156-test-harness-expansion`  
**Date**: 2026-08-20  
**Status**: Completed  

---

## 1. Execute All 13 C Test Suites

### Validation Scenario 1: Master C Test Runner Execution

- **Command**:
  ```bash
  cmake -B build -DBUILD_TESTING=ON && cmake --build build --target ttzip_c_test_runner && ./build/ttzip_c_test_runner all
  ```
- **Expected Output**:
  - `[ PASS ] [Blosc & BitGroom]`
  - `[ PASS ] [Huffman In-Place]`
  - `[ PASS ] [Snappy Engine   ]`
  - `[ PASS ] [DMG & LZFSE     ]`
  - `[ PASS ] [Radix Tree      ]`
  - `🎉 ALL C TEST SUITES PASSED SUCCESSFULLY (< 15.00 ms total)`
- **Failure Diagnostic**:
  Run individual test target via `./build/ttzip_c_test_runner <suite>` (e.g. `blosc_engine`, `huffman_inplace`, `snappy_engine`, `dmg_lzfse`, `archive_tree`) to inspect failing line numbers.

---

## 2. Granular CTest Validation

### Validation Scenario 2: CTest Discovery & Execution

- **Command**:
  ```bash
  ctest --test-dir build --output-on-failure
  ```
- **Expected Output**:
  - `100% tests passed, 0 tests failed out of 14`
  - `Total Test time (real) = < 0.20 sec`
- **Failure Diagnostic**:
  Check `build/Testing/Temporary/LastTest.log` for output.

---

## 3. Local CI Pipeline Verification

### Validation Scenario 3: 5-Stage Local CI Pipeline

- **Command**:
  ```bash
  ./scripts/local-ci.sh
  ```
- **Expected Output**:
  - All 5 stages execute cleanly with exit code `0` and `🎉 ALL LOCAL CI CHECKS PASSED SUCCESSFULLY (0 Quota)`.
- **Failure Diagnostic**:
  Inspect the failing stage and ensure no compiler warnings exist.
