# Tasks: 174-sink-swift-core-into-rust-engine

## Phase 1: Standards Compliance & 16-Format Magic Sniffing in Rust (US1)
- [x] T001 [P] [US1] Implement `rust/ttzip-glue/src/standards/anchors.rs`, `signatures.rs`, and `sniffer.rs` for zero-allocation 16-format magic sniffing across 4 anchor types (Head, Tail, Sector 16, TarOffset).
- [x] T002 [P] [US1] Implement `rust/ttzip-glue/src/standards/extra_fields.rs` for zero-copy ZIP TLV Extra Field parsing (Zip64, UT, up, ux, AE).
- [x] T003 [P] [US1] Implement `rust/ttzip-glue/src/standards/checkers/` (ZIP, TAR, 7z, GZ, ZSTD, BZ2, XZ, DiskImages, Modern) for strict specification compliance assertions.
- [x] T004 [P] [US1] Implement `rust/ttzip-glue/src/standards/ffi.rs` exporting C-ABI functions (`ttzip_rust_detect_format_buffer`, `ttzip_rust_check_compliance_buffer`, `ttzip_rust_free_compliance_report`).

## Phase 2: High-Performance Crypto, Zeroize & Reed-Solomon FEC in Rust (US2)
- [x] T005 [P] [US2] Implement `rust/ttzip-glue/src/crypto/zipcrypto.rs` (PKZIP 3-Key stream cipher with ARM64 CRC32 instruction acceleration, multi-stream SIMD batching, and `zeroize::ZeroizeOnDrop`).
- [x] T006 [P] [US2] Implement `rust/ttzip-glue/src/crypto/rs_fec/gf8.rs` (Galois Field GF(2^8) arithmetic with ARM NEON `vqtbl1q_u8` 4-bit nibble table SIMD acceleration >25 GB/s).
- [x] T007 [P] [US2] Implement `rust/ttzip-glue/src/crypto/rs_fec/cauchy.rs` and `recovery_record.rs` (Cauchy matrix generation, Gaussian elimination inversion, and TTZR/TTRC self-healing layout).
- [x] T008 [P] [US2] Export crypto and FEC C-ABI symbols in `rust/ttzip-glue/src/ffi/crypto_ffi.rs` and update `ttzip_rust_glue.h`.

## Phase 3: Pure Rust TAR (GNU/PAX) & ZIP Zero-Copy Parser/Packer (US3)
- [x] T009 [P] [US3] Implement `rust/ttzip-glue/src/archive/tar/` (`header.rs`, `scanner.rs`, `reader.rs`, `writer.rs`) supporting POSIX.1 ustar, GNU LongName/LongLink (Type 'L'/'K'), PAX Extended Headers (Type 'x'/'g'), and dual octal checksum verification.
- [x] T010 [P] [US3] Implement `rust/ttzip-glue/src/zip/parser.rs` and `reader.rs` for zero-copy Central Directory parsing with lifetime-bounded `&'a [u8]` slices and Rayon parallel extraction.
- [x] T011 [P] [US3] Implement `rust/ttzip-glue/src/zip/writer/store_stream.rs` for direct I/O + APFS extent preallocation + `pwrite` concurrent store-mode packing.
- [x] T012 [P] [US3] Export TAR and ZIP C-ABI symbols in `rust/ttzip-glue/src/ffi/archive_ffi/` and update headers.

## Phase 4: Swift Thinning & C-ABI Forwarding (US4)
- [x] T013 [P] [US4] Thin `Sources/TTZipCore/Standards/ArchiveMagicSignatureScanner.swift` and `StandardsComplianceChecker.swift` to directly forward to `ttzip_rust_detect_format_*` and `ttzip_rust_check_compliance_*`.
- [x] T014 [P] [US4] Thin `Sources/TTZipCore/Zip/ZipCryptoEngine.swift` and `Security/ReedSolomonFEC.swift` to forward to `ttzip_rust_zipcrypto_*` and `ttzip_rust_rs_*`.
- [x] T015 [P] [US4] Thin `Sources/TTZipCore/Tar/TarLz4SeekScanner.swift` and `Zip/ZipCentralDirectoryReader.swift` to forward to Rust native parsers.
- [x] T016 [P] [US4] Clean up unneeded private helper functions in Swift core that were fully sunk to Rust.

## Phase 5: Verification, CI Gates & Standalone CLI Validation (US5)
- [x] T017 [US5] Run `cargo test --workspace` on all Rust crates (`ttzip-glue`, `ttzip-tui`).
- [x] T018 [US5] Run `./scripts/build_rust.sh --release` and `./scripts/build_tui.sh` and test standalone `bin/ttzip`.
- [x] T019 [US5] Run `swift test` ensuring all 860+ tests pass with 0 failures and 0 warnings.
- [x] T020 [US5] Run `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
