# Tasks: All-Formats Zero-Loss Performance Domination (Phase 2)

- [x] **TASK-1**: In `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c`, configure `ZSTD_c_nbWorkers = p_cores` and `ZSTD_c_jobSize = 4MB` with high-entropy fast detection.
- [x] **TASK-2**: In `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c`, optimize Level 1 AES-256 small-file batching and micro-chunking.
- [x] **TASK-3**: Run `swift test --filter XCTestPerformanceMeasureTests` and verify all 9 performance floors pass.
- [x] **TASK-4**: Run `TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests` and verify 0 losses against competitors.
- [x] **TASK-5**: Run full 561-test suite with `swift test`, commit and push to `origin/main`.
