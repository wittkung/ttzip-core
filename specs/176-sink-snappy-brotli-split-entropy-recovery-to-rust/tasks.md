# Tasks: 176-sink-snappy-brotli-split-entropy-recovery-to-rust

## Phase 1: Pure Rust Snappy Framing & Brotli Streaming Codecs (US1)
- [x] T001 [P] [US1] Add `snap = "1.1"` and `brotli = "7.0"` to `rust/ttzip-glue/Cargo.toml` and implement raw block & framing stream in `rust/ttzip-glue/src/codecs/snappy/`.
- [x] T002 [P] [US1] Implement 100% pure Rust Brotli streaming engine in `rust/ttzip-glue/src/codecs/brotli/`.
- [x] T003 [P] [US1] Export `ttzip_rust_snappy_frame_*` and `ttzip_rust_brotli_*` C-ABIs in `rust/ttzip-glue/src/ffi/codecs_ffi/`.
- [x] T004 [P] [US1] Thin `Sources/TTZipCore/Snappy/SnappyFramingStream.swift`, `SnappyBlockEngine.swift`, and `Brotli/NativeBrotliEngine.swift` (delete `import Compression`).

## Phase 2: Multi-Volume Split Container & Virtual Continuous Reader (US2)
- [x] T005 [P] [US2] Implement `SplitVolumeWriter` (`std::io::Write`) with exact byte counting and PKZIP normalization in `rust/ttzip-glue/src/archive/split/writer.rs`.
- [x] T006 [P] [US2] Implement `VirtualMultiVolumeReader` (`std::io::Read + Seek`) with multi-volume topology discovery in `rust/ttzip-glue/src/archive/split/reader.rs`.
- [x] T007 [P] [US2] Export multi-volume C-ABIs in `rust/ttzip-glue/src/ffi/archive_ffi/split.rs`.
- [x] T008 [P] [US2] Thin `Sources/TTZipCore/Split/MultiVolumeStreamSink.swift` and `Decorators/SplitVolumeDecorator.swift`.

## Phase 3: SIMD Shannon Entropy & Cascaded Codec Selector (US3)
- [x] T009 [P] [US3] Implement 4-way unrolled 256-bucket histogram with NEON/AVX2 vector reduction and table-driven log2 in `rust/ttzip-glue/src/analytics/entropy.rs`.
- [x] T010 [P] [US3] Implement 3-stage cascaded recommendation engine in `rust/ttzip-glue/src/analytics/codec_selector.rs`.
- [x] T011 [P] [US3] Export analytics C-ABIs in `rust/ttzip-glue/src/ffi/analytics_ffi.rs`.
- [x] T012 [P] [US3] Thin `Sources/TTZipCore/Services/ArchiveEntropyEvaluator.swift` and `SmartCodecSelector.swift`.

## Phase 4: VFS O(1) Lock-Free LZ4 Cache Pool & In-Memory Password Recovery / Repair (US4, US5)
- [x] T013 [P] [US4] Implement index-based Arena doubly-linked list with `hashbrown::HashMap` and 16-way sharded locks in `rust/ttzip-glue/src/vfs/cache_pool.rs`.
- [x] T014 [P] [US5] Implement in-memory Rayon multi-core password verification in `rust/ttzip-glue/src/crypto/recovery.rs` and SIMD corrupted stream reconstruction in `rust/ttzip-glue/src/archive/repair.rs`.
- [x] T015 [P] [US4, US5] Export VFS cache, password recovery, and repair C-ABIs in `rust/ttzip-glue/src/ffi/`.
- [x] T016 [P] [US4, US5] Thin `Sources/TTZipCore/VFS/VFSLz4CachePool.swift`, `PasswordRecoveryEngine.swift`, and `ArchiveRepairEngine.swift`.

## Phase 5: Verification, CI Gates & Standalone CLI Validation (US6)
- [x] T017 [US6] Run `cargo test --workspace` on all Rust crates (`ttzip-glue`, `ttzip-tui`).
- [x] T018 [US6] Run `./scripts/build_rust.sh --release` and `./scripts/build_tui.sh` and test standalone `bin/ttzip`.
- [x] T019 [US6] Run `swift test` ensuring all 863+ tests pass with 0 failures and 0 warnings.
- [x] T020 [US6] Run `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
