# Tasks: Tied Scenarios Domination (Phase 3)

- [x] **TASK-1**: In `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c`, allocate dynamic 64MB buffer for large payloads and refine high-entropy tuning.
- [x] **TASK-2**: In `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c` and `ttzip_lzma2_enc_native.c`, configure 16KB micro-dictionary and 128KB micro-chunking for Level 1 small files.
- [x] **TASK-3**: Run `swift test --filter XCTestPerformanceMeasureTests` to ensure 9 performance floors pass.
- [x] **TASK-4**: Run `TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests` and verify all tied scenarios flip to dominant wins.
- [x] **TASK-5**: Run full 561-test suite with `swift test`, commit and push to `origin/main`.
