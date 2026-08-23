# Tasks: 184-ultimate-rust-microkernel-unification

## Phase 1: Rust Unified Archive Orchestrator Engine (US1)
- [x] T001 [P] [US1] Implement `rust/ttzip-glue/src/archive/unified.rs` with unified format dispatch, multi-volume handling, and streaming pipelines.
- [x] T002 [P] [US1] Export C-ABI functions `ttzip_rust_archive_create_unified`, `ttzip_rust_archive_extract_unified`, `ttzip_rust_archive_inspect_unified`, `ttzip_rust_archive_repair_unified` in `rust/ttzip-glue/src/ffi/archive_ffi/unified.rs` and `Sources/CTTZipBridge/include/ttzip_rust_glue.h`.
- [x] T003 [P] [US1] Refactor `Sources/TTZipCore/ArchiveWriter.swift` and `Sources/TTZipCore/ArchiveExtractor.swift` to delegate directly to Rust unified C-ABI, maintaining LOC < 350.
- [x] T004 [P] [US1] Add unit tests for unified archive lifecycle in `rust/ttzip-glue/src/archive/unified.rs` and `Tests/TTZipTests/`.

## Phase 2: Rust Unified VFS Tree & Fuzzy Search (US2)
- [x] T005 [P] [US2] Implement `rust/ttzip-glue/src/fs/vfs.rs` with fast hierarchical tree construction, ASCII/Unicode rendering, and fuzzy query matching.
- [x] T006 [P] [US2] Export C-ABI functions `ttzip_rust_vfs_tree_build`, `ttzip_rust_vfs_fuzzy_search`, `ttzip_rust_vfs_tree_render` in `rust/ttzip-glue/src/ffi/fs_ffi.rs` and `Sources/CTTZipBridge/include/ttzip_rust_glue.h`.
- [x] T007 [P] [US2] Refactor `Sources/TTZipCore/ArchiveComponentTraversals.swift` and `Sources/TTZipCore/ArchiveReader.swift` to delegate to Rust VFS engine, maintaining LOC < 350.
- [x] T008 [P] [US2] Add unit tests for VFS tree building and search in `Tests/TTZipTests/`.

## Phase 3: Zero-Fat Swift Skin Consolidation & Integrity Unification (US3)
- [x] T009 [P] [US3] Refactor `Sources/TTZipCore/ArchiveIntegrityChecker.swift` and `Sources/TTZipCore/ArchiveRepairEngine.swift` to delegate directly to Rust unified C-ABI, maintaining LOC < 350.
- [x] T010 [P] [US3] Re-verify all first-party Swift files in `Sources/TTZipCore/` and `Sources/TTZipBench/` strictly adhere to `< 350 LOC`.
- [x] T011 [P] [US3] Run full `swift test` suite.
- [x] T012 [P] [US3] Run full `cargo test` suite across all crates.

## Phase 4: Verification, CI Gates & Standalone Validation (US4)
- [x] T013 [US4] Run `./scripts/build_rust.sh --release && ./scripts/build_tui.sh` and verify universal libraries and `bin/ttzip`.
- [x] T014 [US4] Run `swift test` ensuring all 893+ tests pass with 0 failures and 0 warnings.
- [x] T015 [US4] Run `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
