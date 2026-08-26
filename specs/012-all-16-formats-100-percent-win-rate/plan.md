# Implementation Plan: 100% Win Rate & Zero Regression (012)

## Phase 1: Pure TAR and ZSTD Direct Pipeline Refinements
- In `ttzip_tar_native.c`, add memory prefetching (`madvise(MADV_WILLNEED | MADV_SEQUENTIAL)`) for input files during TAR packaging.
- In `ttzip_tar_zstd_direct.c`, optimize the decompression loop buffer from 4MB to 16MB aligned page buffer.

## Phase 2: LZ4, LZIP, XZ Compression Level Mapping
- Ensure LZ4 compression level for Level 6 uses fast multi-threading and does not trigger single-threaded LZ4HC stall.
- Ensure LZIP and XZ block sizes are tuned to 16MB for multicore scaling.

## Phase 3: Benchmark Execution & Regression Audit
- Run `AllFormatsPkSuiteTests` with 2 passes for stable peak throughput.
- Audit performance report using `audit_performance_regression.py`.
- Run full 560+ regression suite and 11 performance gates.
