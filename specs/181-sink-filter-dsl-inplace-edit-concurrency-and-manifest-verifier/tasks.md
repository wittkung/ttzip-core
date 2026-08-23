# Tasks: 181-sink-filter-dsl-inplace-edit-concurrency-and-manifest-verifier

## Phase 1: Filter DSL Lexer, Parser & AST Evaluator in Rust (US1)
- [x] T001 [P] [US1] Implement `rust/ttzip-glue/src/fs/filter_dsl.rs` with token scanner, recursive-descent parser, and AST evaluation.
- [x] T002 [P] [US1] Export C-ABI `ttzip_rust_eval_filter_dsl` in `rust/ttzip-glue/src/ffi/fs_ffi.rs` and `Sources/CTTZipBridge/include/ttzip_rust_glue.h`.
- [x] T003 [P] [US1] Refactor `Sources/TTZipCore/InterpreterPattern/ArchiveFilterDSLLexerParser.swift` to delegate to Rust C-ABI, maintaining LOC < 350.
- [x] T004 [P] [US1] Add unit tests for filter DSL expressions in `rust/ttzip-glue/src/fs/filter_dsl.rs` and `Tests/TTZipTests/`.

## Phase 2: In-Place Atomic Archive Editing Engine (US2)
- [x] T005 [P] [US2] Implement `rust/ttzip-glue/src/archive/in_place_edit.rs` supporting atomic entry append, replace, and delete for ZIP/7z.
- [x] T006 [P] [US2] Export C-ABI for in-place archive transactions in `rust/ttzip-glue/src/ffi/archive_ffi/` and `Sources/CTTZipBridge/include/ttzip_rust_glue.h`.
- [x] T007 [P] [US2] Refactor `Sources/TTZipCore/InPlaceEdit/InPlaceEditEngine.swift` and `InPlaceEditSession.swift` to delegate to Rust C-ABI, maintaining LOC < 350.
- [x] T008 [P] [US2] Add unit tests for in-place archive editing in `Tests/TTZipTests/`.

## Phase 3: Differential Manifest Scanner & Facade Consolidation (US3)
- [x] T009 [P] [US3] Implement `rust/ttzip-glue/src/testing/differential.rs` with multi-threaded directory tree hashing and differential oracle verifier.
- [x] T010 [P] [US3] Refactor `Sources/TTZipCore/Testing/DifferentialManifestScanner.swift`, `DifferentialManifestVerifier.swift`, and `LibarchiveGoldenCorpusVerifier.swift`.
- [x] T011 [P] [US3] Consolidate and thin out `ArchiveBatchFacade.swift`, `ArchiveBuilders.swift`, `ArchiveOperationsFacade.swift`, `ArchiveStreamingFacade.swift`, `ConcreteVisitors.swift`, and `ConcreteRepositories.swift`.
- [x] T012 [P] [US3] Add unit tests for differential manifest scanning and verification.

## Phase 4: Verification, CI Gates & Standalone Validation (US4)
- [x] T013 [US4] Run `cargo test` across all Rust crates (`ttzip-glue`, `ttzip-tui`).
- [x] T014 [US4] Run `./scripts/build_rust.sh --release && ./scripts/build_tui.sh` and verify universal libraries and `bin/ttzip`.
- [x] T015 [US4] Run `swift test` ensuring all 885+ tests pass with 0 failures and 0 warnings.
- [x] T016 [US4] Run `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
