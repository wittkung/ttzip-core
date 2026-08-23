# Tasks: 189-production-core-de-tox-and-pure-facade-sinking

## Phase 1: Pure Facade Sinking (US1)
- [x] T001 [P] [US1] Route `ArchiveFilter` in `Sources/TTZipCore/` directly to `ttzip_rust_eval_filter_dsl`.
- [x] T002 [P] [US1] Route VFS tree printing in `ArchiveReader.swift` directly to `ttzip_rust_vfs_tree_render`.

## Phase 2: Purge Misplaced & Duplicate Code (US2)
- [x] T003 [P] [US2] Delete `Sources/TTZipCore/Testing/` (17 files).
- [x] T004 [P] [US2] Delete `Sources/TTZipCore/Mocks/` (1 file).
- [x] T005 [P] [US2] Delete `Sources/TTZipCore/InterpreterPattern/` (5 files).
- [x] T006 [P] [US2] Delete `Sources/TTZipCore/VisitorPattern/` (7 files).
- [x] T007 [P] [US2] Delete `Sources/TTZipCore/Pipeline/DeflateStreamEngine*.swift` (3 files).

## Phase 3: CI Alignment & Final Verification (US3)
- [x] T008 [US3] Verify `swift build` and `swift test` 100% PASS with zero warnings.
- [x] T009 [US3] Run `cargo test --workspace` on all Rust crates.
- [x] T010 [US3] Run `./scripts/run_local_ci_gate.sh` full CI validation.
