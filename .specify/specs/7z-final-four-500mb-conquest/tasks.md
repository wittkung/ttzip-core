# Tasks: 7Z Final Four 500MB Level 1 Losses Conquest

- [x] **TASK-1**: In `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c`, optimize Level 1 block sizing for large files (>= 64MB) to $4\text{MB}$ and fast zero-chunk path.
- [x] **TASK-2**: In `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c`, configure 64KB micro-dictionary, `opts.mode = LZMA_MODE_FAST`, `opts.mf = LZMA_MF_HC3`, `opts.nice_len = 8` for Level 1 blocks.
- [x] **TASK-3**: In `Sources/CTTZipBridge/CTTZipBridge_7zNativeDecoder.c`, implement zero-copy direct writeback decoding for single large files, eliminating intermediate 500MB `unpack_buf` and sync munmap flush.
- [x] **TASK-4**: In `Sources/CTTZipBridge/ttzip_lzma2_dec_native.c`, optimize REP0 `dist == 1` with 128-bit NEON `vdupq_n_u8` broadcast.
- [x] **TASK-5**: In `Sources/CTTZipBridge/ttzip_7z_crypto_neon.c` & `ttzip_lzma2_enc_native.c`, optimize in-place ARMv8 AES-256 pipeline.
- [x] **TASK-6**: Run `swift test --filter XCTestPerformanceMeasureTests` to verify all 9 upgraded performance floors pass.
- [x] **TASK-7**: Run `TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests` and verify 7Z dominance reaching 31/32 (96.9% win rate).
- [x] **TASK-8**: Run `python3 scripts/audit_performance_regression.py` to confirm zero regression across all 46 benchmarks, run full 561 unit tests (`swift test`), commit and push to `origin/main`.
