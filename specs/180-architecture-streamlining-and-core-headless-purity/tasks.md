# Tasks: 180-architecture-streamlining-and-core-headless-purity

## Phase 1: 7z Engine Onion Cleanup & Authentic Header Descriptors (US1)
- [x] T001 [P] [US1] Streamline `Sources/TTZipCore/SevenZip/NativeSevenZipEngine.swift` to directly invoke `SevenZipCAdapter` / `ttzip_rust_create_archive` / `ttzip_rust_extract_archive`.
- [x] T002 [P] [US1] Refactor `Sources/TTZipCore/SevenZip/SevenZipHeaderReader.swift` to fetch authentic entry descriptors via `ttzip_rust_scan_entries`.
- [x] T003 [P] [US1] Streamline `Sources/TTZipCore/SevenZip/SevenZipParallelExtractor.swift` and `SevenZipParallelWriter.swift` maintaining LOC < 100.
- [x] T004 [P] [US1] Add unit tests for authentic 7z header descriptor inspection in `Tests/TTZipTests/`.

## Phase 2: Standards & Magic Signature Delegation to Rust (US2)
- [x] T005 [P] [US2] Refactor `Sources/TTZipCore/Standards/ArchiveMagicSignatureScanner.swift` to delegate to Rust `ttzip_rust_detect_format_buffer` / `ttzip_rust_detect_format_file`.
- [x] T006 [P] [US2] Refactor `Sources/TTZipCore/Standards/StandardsComplianceChecker.swift` and its extension files to delegate to Rust `ttzip_rust_check_compliance_file` / `ttzip_rust_check_compliance_buffer`.
- [x] T007 [P] [US2] Verify standards and signature scanning compliance across all 17 supported formats.
- [x] T008 [P] [US2] Add unit tests for standards compliance in `Tests/TTZipTests/ArchiveStandardsComplianceTests.swift`.

## Phase 3: Piped Tar/Brotli/Zstd Stream & Headless Purity (US3)
- [x] T009 [P] [US3] Move `Sources/TTZipCore/Services/FileClipboardStore.swift` to `Sources/TTZipApp/Services/FileClipboardStore.swift` and remove AppKit/SwiftUI imports from TTZipCore.
- [x] T010 [P] [US3] Refactor `Sources/TTZipCore/Brotli/NativeBrotliEngine.swift` composite TAR handling to eliminate intermediate uncompressed `.tar` disk writes.
- [x] T011 [P] [US3] Streamline `Sources/TTZipCore/TemplateMethod/` and `Sources/TTZipCore/StatePattern/` boilerplate.
- [x] T012 [P] [US3] Add unit tests for memory streaming and headless core compilation.

## Phase 4: Verification, CI Gates & Standalone Validation (US4)
- [x] T013 [US4] Run `cargo test` across all Rust crates (`ttzip-glue`, `ttzip-tui`).
- [x] T014 [US4] Run `./scripts/build_rust.sh --release && ./scripts/build_tui.sh` and verify universal libraries and `bin/ttzip`.
- [x] T015 [US4] Run `swift test` ensuring all 880+ tests pass with 0 failures and 0 warnings.
- [x] T016 [US4] Run `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
