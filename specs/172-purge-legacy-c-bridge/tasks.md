# Tasks: 172-purge-legacy-c-bridge

## Phase 1: Dead & Orphan C Files Purge (Batch 1)
- [x] T001 [P] [US1] Remove 32 orphan experimental C files in `Sources/CTTZipBridge/` (BitGroom, Blosc, Quantum, SuperChunk, Tensor, etc.) and `native_inflate/`.
- [x] T002 [P] [US1] Clean up dead Swift adapter files (`AdaptiveBlockSplitAdapter.swift`, `InPlaceHuffmanAdapter.swift`, `QuantumPipelineAccelerator.swift`, `NDimTensorLayout.swift`, `ThreadLocalContextPoolAdapter.swift`, `ArchiveSearchIndex.swift`).

## Phase 2: Redirect Swift Callers to Rust C-ABI & Delete Superseded C Files (Batch 2)
- [x] T003 [P] [US2] Redirect `HardwareChecksumAdapter.swift` (CRC32, Adler32) to `ttzip_rust_crc32` / `ttzip_rust_adler32`.
- [x] T004 [P] [US2] Redirect `SevenZipCAdapter.swift` (7z create/extract, FL2 compress) to `ttzip_rust_create_archive`, `ttzip_rust_extract_archive`, `ttzip_rust_fl2_compress`.
- [x] T005 [P] [US2] Redirect `ZstdCAdapter.swift` and `LzfseCAdapter.swift` to `ttzip_rust_zstd_*` and `ttzip_rust_lzfse_*`.
- [x] T006 [P] [US2] Redirect `LibdeflateCAdapter.swift` and `FastContainerEngine.swift` to `ttzip_rust_deflate_*`, `ttzip_rust_gzip_*`, `ttzip_rust_zlib_*`.
- [x] T007 [P] [US2] Redirect `ZipCryptoEngine.swift` to `ttzip_rust_aes256_*`.
- [x] T008 [P] [US2] Remove 33 superseded C files in `Sources/CTTZipBridge/` (`CTTZipCRC32Neon.c`, `CTTZipAdler32Neon.c`, `CTTZipBridge_7z*.c`, `CTTZipBridge_Zip*.c`, `CTTZipBridge_Crypto.c`, `CTTZipBridge_Zstd.c`, `CTTZipBridge_LZFSE.c`, etc.).

## Phase 3: Swift 6 Native Utility Replacements & Single-File C Bridge Convergence (Batch 3)
- [x] T009 [P] [US3] Replace `ttzip_strnatcmp` with Swift `String.localizedStandardCompare` in `NativeMicrokernelBridge.swift` / `DiskItemSorter.swift`.
- [x] T010 [P] [US3] Replace `ttzip_core_aligned_alloc_16k` with `UnsafeMutableRawPointer.allocate(alignment: 16384)` in `CUnsafeBufferAdapter.swift`.
- [x] T011 [P] [US3] Replace `ttzip_mem_budget_*` / `ttzip_thread_budget_*` with `ProcessInfo` in `ConcurrencyBridge.swift`.
- [x] T012 [P] [US3] Replace `ttzip_platform_monotonic_nanos` with `ContinuousClock` in `PlatformMonotonicTimer.swift`.
- [x] T013 [P] [US3] Replace `ttzip_generate_corpus` with pure Swift in `BenchmarkCorpusGenerator.swift`.
- [x] T014 [P] [US1] Converge necessary remaining C symbols (`ttzip_core_posix_spawn_fast`, `ttzip_rs_*`, `ttzip_zopfli_*`, `ttzip_crc64`, `ttzip_magic_sniff_buffer`) into single `Sources/CTTZipBridge/CTTZipBridge.c`.
- [x] T015 [US1] Remove all other `.c` files and dead headers from `Sources/CTTZipBridge/include/`.

## Phase 4: Build, Clean, and Full CI Validation
- [x] T016 [US1] Update `Package.swift` `CTTZipBridge` target configuration.
- [x] T017 [US1] Run `./scripts/build_rust.sh --release` and `./scripts/build_tui.sh`.
- [x] T018 [US1] Run `swift test` and `./scripts/run_local_ci_gate.sh` to ensure 100% PASS with 0 Warnings.
