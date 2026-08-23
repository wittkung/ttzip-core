# Tasks: 094 Entropy-Aware Tiered Chunking Engine

- [x] T001 [P] [US1] Add `ttzip_calculate_adaptive_block_size` into `Sources/CTTZipBridge/include/CTTZipStreamCoder.h` and `Sources/CTTZipBridge/CTTZipStreamCoder.c`.
- [x] T002 [US1] Update `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift` with `effectiveBlockSize` calculation based on probed entropy and file size.
- [x] T003 [US1] Create `Tests/TTZipTests/EntropyTieredChunkingEngineTests.swift` to test all 4 entropy tiers with ratio and throughput assertions.
- [x] T004 [US1] Run full local CI/CD gate and push.
