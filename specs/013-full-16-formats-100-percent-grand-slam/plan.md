# Implementation Plan: Feature 013 (100% Grand Slam)

## Phase 1: Pure TAR & Direct I/O Acceleration
- Apply APFS zero-copy file copying for uncompressed TAR creation in `ttzip_tar_native.c`.
- Increase TAR stream write block size to 16MB.

## Phase 2: ZSTD & LZ4 Dynamic Strategy Refinements
- Set `ZSTD_fast` for high-entropy payloads in `ttzip_tar_zstd_direct.c`.
- Refine LZ4 compression parameters to maintain multi-threaded high throughput across all payload types.

## Phase 3: Benchmark & Regression Assertions
- Run `AllFormatsPkSuiteTests`.
- Run `audit_performance_regression.py`.
- Run `XCTestPerformanceMeasureTests` and full test suite.
