# Quickstart & Verification Guide: C Test Harness

**Feature**: `154-c-test-harness-migration`  
**Date**: 2026-08-20  
**Status**: Completed  

---

## 1. Build and Run Full C Test Suite

### Validation Scenario 1: All-in-One C Test Runner

- **Command**:
  ```bash
  cmake -B build -DBUILD_TESTING=ON && cmake --build build --target ttzip_c_test_runner && ./build/ttzip_c_test_runner all
  ```
- **Expected Output**:
  ```text
  [ PASS ] [crc_neon       ] test_crc32_ieee8023_standard_vector      (0.12 µs)
  [ PASS ] [crc_neon       ] test_crc64_xz_standard_vector            (0.15 µs)
  [ PASS ] [magic_sniff    ] test_all_16_formats_magic_detection      (0.45 µs)
  [ PASS ] [strnatcmp      ] test_natural_sort_ordering               (0.20 µs)
  [ PASS ] [deflate_zopfli ] test_deflate_roundtrip_lossless          (1.82 ms)
  [ PASS ] [7z_lzma2       ] test_7z_varint_and_lzma2_block           (2.40 ms)
  [ PASS ] [tar_container  ] test_tar_swar_octal_and_tree             (0.35 ms)
  [ PASS ] [security       ] test_zip_slip_traversal_blocked          (0.10 µs)
  [ PASS ] [concurrency    ] test_threadpool_parallel_for             (0.90 ms)
  --------------------------------------------------------------------------------
   Master Suite Summary: All Suites
     Total: 25+ | Passed: 25+ | Failed: 0 | Skipped: 0 | Rate: 100.0%
     Assertions: 120+ | Total Duration: < 15.00 ms
  --------------------------------------------------------------------------------
  ```
- **Failure Diagnostic**:
  If a test fails, `ttzip_test_harness.h` prints the exact source file path, line number, failing expression, and a byte/value diff. Check the relevant subsystem under `Sources/CTTZipBridge/`.

---

## 2. Granular CTest Suite Execution

### Validation Scenario 2: CTest Individual Suite Run

- **Command**:
  ```bash
  ctest --test-dir build --output-on-failure
  ```
- **Expected Output**:
  ```text
  1/9 Test #1: c_test_crc_neon ................. Passed   0.01 sec
  2/9 Test #2: c_test_magic_sniff .............. Passed   0.01 sec
  3/9 Test #3: c_test_strnatcmp ................ Passed   0.01 sec
  4/9 Test #4: c_test_deflate_zopfli ........... Passed   0.02 sec
  5/9 Test #5: c_test_7z_lzma2 ................. Passed   0.02 sec
  6/9 Test #6: c_test_tar_container ............ Passed   0.01 sec
  7/9 Test #7: c_test_security_zipslip ......... Passed   0.01 sec
  8/9 Test #8: c_test_concurrency_threadpool ... Passed   0.01 sec
  9/9 Test #9: c_test_all ...................... Passed   0.02 sec
  
  100% tests passed, 0 tests failed out of 9
  Total Test time (real) =   0.08 sec
  ```
- **Failure Diagnostic**:
  Run individual test target directly with `./build/ttzip_c_test_runner <suite_name>` to isolate and debug failed assertions.

---

## 3. Memory Safety & Sanitizer Audit

### Validation Scenario 3: AddressSanitizer & UBSan Zero-Leak Gate

- **Command**:
  ```bash
  cmake -B build_asan -DBUILD_TESTING=ON -DENABLE_SANITIZERS=ON && cmake --build build_asan && ./build_asan/ttzip_c_test_runner all
  ```
- **Expected Output**:
  - Exit code `0`
  - Zero `ERROR: AddressSanitizer:` messages
  - Zero `SUMMARY: UndefinedBehaviorSanitizer:` warnings
  - Zero memory leaks detected by LeakSanitizer
- **Failure Diagnostic**:
  ASan will print a stack trace pointing to the offending pointer or heap allocation. Check pointer bounds in the corresponding `Sources/CTTZipBridge/*.c` source.
