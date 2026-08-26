# Quickstart & Verification Guide: Native C11 Benchmark Suites

**Feature**: `159-159-benchmark-suite-migration`  
**Date**: 2026-08-20  
**Status**: Completed  

---

## 1. Execute Full Native C Benchmark Matrix

### Validation Scenario 1: Standalone Benchmark Runner

- **Command**:
  ```bash
  cmake -B build -DBUILD_TESTING=ON && cmake --build build --target ttzip_benchmark_runner && ./build/ttzip_benchmark_runner --all
  ```
- **Expected Output**:
  - `[ Codec Throughput ] Deflate, Zstd, Fast-LZMA2, LZFSE, Snappy`
  - `[ Checksums & SIMD ] CRC32, CRC64, Adler-32, Entropy`
  - `[ Pareto Frontier  ] Non-dominated efficient frontier`
  - `[ Stress & VFS     ] Radix Tree 50k search & 2GB stress`
  - `🎉 ALL NATIVE C BENCHMARKS EXECUTED SUCCESSFULLY (< 1.50 s total)`
- **Failure Diagnostic**:
  Run specific benchmark target via `./build/ttzip_benchmark_runner --codecs` or `--checksums` to inspect specific results.

---

## 2. Granular CTest Benchmark Validation

### Validation Scenario 2: CTest Execution

- **Command**:
  ```bash
  ctest --test-dir build -L benchmark --output-on-failure
  ```
- **Expected Output**:
  - `100% tests passed, 0 tests failed`
  - `Total Test time (real) = < 1.50 sec`
- **Failure Diagnostic**:
  Check `build/Testing/Temporary/LastTest.log` for logs.

---

## 3. Local CI Pipeline Verification

### Validation Scenario 3: 5-Stage Local CI Pipeline

- **Command**:
  ```bash
  ./scripts/local-ci.sh
  ```
- **Expected Output**:
  - All 5 stages pass cleanly with exit code `0` and `0 Quota`.
- **Failure Diagnostic**:
  Inspect failing stage for any remaining Swift imports or compile errors.
