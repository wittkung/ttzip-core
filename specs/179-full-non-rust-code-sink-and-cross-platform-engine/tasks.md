# Tasks: 179-full-non-rust-code-sink-and-cross-platform-engine

## Phase 1: Zero-Allocation Path Sanitizer & ZipSlip Defense in Rust (US1)
- [x] T001 [P] [US1] Implement `rust/ttzip-glue/src/security/path_sanitizer.rs` with single-pass traversal check, Win32 device filtering, and NFC normalization.
- [x] T002 [P] [US1] Export C-ABI in `rust/ttzip-glue/src/ffi/security_ffi.rs` and update `Sources/CTTZipBridge/include/ttzip_rust_glue.h`.
- [x] T003 [P] [US1] Refactor `Sources/TTZipCore/Platform/PlatformPathSanitizer.swift` and `SecurityScanner.swift` to delegate to Rust C-ABI.
- [x] T004 [P] [US1] Add unit tests for path sanitization and ZipSlip detection in `rust/ttzip-glue/src/security/tests.rs`.

## Phase 2: CJK Bigram Statistical Charset Sniffing & encoding_rs Transcoding (US2)
- [x] T005 [P] [US2] Implement `rust/ttzip-glue/src/charset/` with 2-byte Bigram frequency state machines and `encoding_rs` zero-allocation transcoder.
- [x] T006 [P] [US2] Export C-ABI `ttzip_rust_sanitize_filename` in `rust/ttzip-glue/src/charset/ffi.rs` and `Sources/CTTZipBridge/include/ttzip_rust_glue.h`.
- [x] T007 [P] [US2] Refactor `Sources/TTZipCore/Strategies/CharsetDetectionStrategyProtocol.swift` and `CharsetDetector.swift` to use Rust C-ABI, removing CoreFoundation dependency.
- [x] T008 [P] [US2] Add unit tests for CJK charset detection in `rust/ttzip-glue/src/charset/tests.rs`.

## Phase 3: Streaming Cauchy RS-FEC, 32B SHA-256 & UAF Elimination (US3)
- [x] T009 [P] [US3] Implement streaming chunk-by-chunk Cauchy accumulator and 32B binary SHA-256 digest in `rust/ttzip-glue/src/crypto/rs_fec/recovery_record.rs`.
- [x] T010 [P] [US3] Export file-level and streaming C-ABI in `rust/ttzip-glue/src/ffi/crypto_ffi/fec.rs` and `Sources/CTTZipBridge/include/ttzip_rust_glue.h`.
- [x] T011 [P] [US3] Refactor `Sources/TTZipCore/Security/ReedSolomonFEC.swift` and `ArchiveRecoveryRecordEngine.swift`, eliminating `withUnsafeBytes` pointer escape.
- [x] T012 [P] [US3] Add unit tests for streaming recovery records in `rust/ttzip-glue/src/crypto/rs_fec/tests.rs`.

## Phase 4: Parallel FS Scanner, SIMD Diff, Fuzzing & Platform Zeroize (US4)
- [x] T013 [P] [US4] Implement `rust/ttzip-glue/src/fs/scanner.rs` with Rayon multi-threaded directory walking and 64-way sharded Inode loop tracker.
- [x] T014 [P] [US4] Implement SIMD 16B fast hex diff and SplitMix64 mutation fuzzers in `rust/ttzip-glue/src/testing/`.
- [x] T015 [P] [US4] Implement `SecureBuffer` with compiler barrier `zeroize` and dynamic CPUID capability detector in `rust/ttzip-glue/src/platform/`.
- [x] T016 [P] [US4] Refactor `Sources/TTZipCore/Zip/ZipDirectoryScanner.swift`, `FastHexDiffEngine.swift`, `MalformedStreamFuzzEngine.swift`, `PlatformMemory.swift`, and `PlatformHardware.swift`.

## Phase 5: Verification, CI Gates & Standalone Packaging (US5)
- [x] T017 [US5] Run `cargo test --workspace` on all Rust crates.
- [x] T018 [US5] Run `./scripts/build_rust.sh --release` and `./scripts/build_tui.sh` to update universal static libraries and `bin/ttzip`.
- [x] T019 [US5] Run `swift test` ensuring all 872+ tests pass with 0 failures and 0 warnings.
- [x] T020 [US5] Run `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
