# Quickstart & Verification Guide: Feature 137

**Feature Branch**: `137-libdeflate-single-core-consolidation`

**Date**: 2026-08-20

## Verification Scenarios

### Scenario 1: Libdeflate Unit Tests & Roundtrip Parity
Validates that `LibdeflateCAdapter` and `CTTZipStreamCoder` compress and decompress data accurately across all compression levels.

- **Command**:
  ```bash
  swift test --filter LibdeflateCAdapterTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'LibdeflateCAdapterTests' passed.
  Executed X tests, with 0 failures (0 unexpected) in X.XXX seconds.
  ```
- **Failure Diagnostic**:
  - If tests fail with assertion mismatch, inspect `LibdeflateCAdapter.swift` for memory allocation boundary errors or `ttzip_libdeflate_decompress` return value handling.
  - Check whether `MemoryPageFlyweightPool` buffer reuse capacity is exceeded.

---

### Scenario 2: Zip Parallel Block Deflate Tests
Validates that `ZipBlockParallelCompressor` and `ZipBlockParallelDecompressor` execute parallel multi-threaded Deflate operations through `libdeflate` instances.

- **Command**:
  ```bash
  swift test --filter ZipBlockParallelTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'ZipBlockParallelTests' passed.
  Executed X tests, with 0 failures (0 unexpected) in X.XXX seconds.
  ```
- **Failure Diagnostic**:
  - If tests fail, verify that `g_tls_compressors` array in `CTTZipStreamCoder.c` is properly thread-local and not causing data races across GCD threads.

---

### Scenario 3: Full Test Suite Regression & Zero-Warning Gate
Validates that the entire codebase builds cleanly with 0 compiler warnings and passes all unit, integration, and architecture tests.

- **Command**:
  ```bash
  swift test
  ```
- **Expected Output**:
  ```text
  Test Suite 'All tests' passed.
  Executed 520+ tests, with 0 failures (0 unexpected).
  ```
- **Failure Diagnostic**:
  - If any test fails, run the specific test class with `swift test --filter <FailedTestClassName>` to isolate and diagnose the failure.
