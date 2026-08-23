# Tasks: 186-grand-test-suite-rust-migration-and-swift-deduplication

## Phase 1: Rust Integration Test Expansion (US1)
- [x] T001 [P] [US1] Create `rust/ttzip-glue/tests/pattern_and_pipeline_tests.rs` covering producer-consumer streaming, in-place edit sessions, and worker pool patterns.
- [x] T002 [P] [US1] Create `rust/ttzip-glue/tests/container_matrix_tests.rs` covering 17-format roundtrips and split volume spanning.
- [x] T003 [P] [US1] Verify Rust test suite with `cargo test --workspace`.

## Phase 2: Purge Redundant Swift Low-Level Tests (US2)
- [x] T004 [P] [US2] Delete 14 redundant pattern test files in `Tests/TTZipTests/` (`AdapterPatternTests`, `BridgePatternTests`, `CompositePatternTests`, `DecoratorPatternTests`, `FacadePatternTests`, `FlyweightPatternTests`, `InterpreterPatternTests`, `IteratorPatternTests`, `ProxyPatternTests`, `ReadWriteLockPatternTests`, `StrategyPatternTests`, `TemplateMethodPatternTests`, `VisitorPatternTests`, `WorkerPoolPatternTests`).
- [x] T005 [P] [US2] Delete redundant memory buffer and low-level stream tests (`MmapBufferHandleTests.swift`, `VirtualMultiBlockArenaTests.swift`).
- [x] T006 [P] [US2] Retain and streamline high-level facade tests in `Tests/TTZipTests/` and `Tests/TTZipAppTests/`.
- [x] T007 [P] [US2] Run `swift test` and verify clean, error-free execution.

## Phase 3: CI Gate Alignment & Final Verification (US3)
- [x] T008 [US3] Align `./scripts/run_local_ci_gate.sh` and `./scripts/run_rust_tests.sh`.
- [x] T009 [US3] Run `cargo test --workspace` on all Rust crates.
- [x] T010 [US3] Run `swift test` on full Swift test suite.
- [x] T011 [US3] Run `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
