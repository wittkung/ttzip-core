# Tasks: Universal Zero-Loss Grand Slam Across All Formats

- [x] **TASK-1**: In `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c`, optimize Level 1 block divisor to `p_cores * 2` (~20MB blocks) for large files >= 64MB.
- [x] **TASK-2**: In `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c`, fine-tune HC3 match finder and dictionary sizing for Level 1 blocks.
- [x] **TASK-3**: In `Sources/CTTZipBridge/ttzip_lzma2_dec_native.c` and `CTTZipBridge_7zNativeDecoder.c`, optimize NEON direct copy and zero-allocation block decoding.
- [x] **TASK-4**: In `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c`, optimize 8MB read chunking and direct USTAR header writeback for TAR.ZST decompression.
- [x] **TASK-5**: Run `swift test --filter XCTestPerformanceMeasureTests` to verify all 9 performance floors pass.
- [x] **TASK-6**: Run `TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests` and verify benchmark numbers.
- [x] **TASK-7**: Run full 561-test regression suite with `swift test`, commit and push to `origin/main`.
