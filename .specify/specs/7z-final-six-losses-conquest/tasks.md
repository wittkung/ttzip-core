# Tasks: 7Z Final Six Losses Conquest & Universal Dominance

- [x] **TASK-1**: In `Sources/CTTZipBridge/ttzip_lzma2_dec_native.c`, implement LZMA2 uncompressed sub-chunk (`0x01`/`0x02`) NEON direct copy bypass and dynamic 64KB dictionary sizing for Level 1 blocks.
- [x] **TASK-2**: In `Sources/CTTZipBridge/CTTZipBridge_7zNativeDecoder.c`, optimize 16KB-aligned `mmap` writeback and integrate ARM64 NEON CRC32 for non-encrypted extraction.
- [x] **TASK-3**: In `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c` and `ttzip_lzma2_enc_native.c`, configure 64KB L1 cache-resident dictionary and 4MB~8MB chunking for >= 500MB streams.
- [x] **TASK-4**: In `Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c` and `ttzip_lzma2_enc_native.c`, implement async KDF pre-warming and in-place NEON AES-256 encryption.
- [x] **TASK-5**: Run `swift test --filter XCTestPerformanceMeasureTests` to ensure all 9 performance floors pass.
- [x] **TASK-6**: Run `TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests` and verify all 6 previously trailing 7Z items flip to dominant wins.
- [x] **TASK-7**: Run full 561-test regression suite with `swift test`, commit and push to `origin/main`.
