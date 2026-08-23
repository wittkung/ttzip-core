# Tasks: 185-total-rust-microkernel-migration-and-c-swift-pruning

## Phase 1: Purge Legacy C Source Trees from SPM (US1)
- [ ] T001 [P] [US1] Remove legacy C/C++ trees (`zopfli/`, `fast-lzma2/`, `lzfse/`, `snappy/`, `CTTZipBridge.c`, `CTTZipBridge_Archive.c`) from `Sources/CTTZipBridge/`.
- [ ] T002 [P] [US1] Simplify `Sources/CTTZipBridge/include/CTTZipBridge.h` and verify module map and C headers.
- [ ] T003 [P] [US1] Update `Package.swift` to remove obsolete C compilation flags and header search paths.
- [ ] T004 [P] [US1] Build via `swift build` and verify 100% clean compilation without C sources.

## Phase 2: Rust Multi-Core Password Recovery Engine Sinking (US2)
- [x] T005 [P] [US2] Enhance `rust/ttzip-glue/src/crypto/password_recovery.rs` with multi-core dictionary attack and candidate generator.
- [x] T006 [P] [US2] Export C-ABI functions `ttzip_rust_password_recovery_start_dictionary` and `ttzip_rust_password_recovery_cancel` in `rust/ttzip-glue/src/ffi/crypto_ffi/` and `Sources/CTTZipBridge/include/ttzip_rust_glue.h`.
- [x] T007 [P] [US2] Refactor `Sources/TTZipCore/PasswordRecoveryEngine.swift` to delegate directly to Rust C-ABI, maintaining LOC < 350.
- [x] T008 [P] [US2] Add unit tests for password recovery in `rust/ttzip-glue/src/crypto/password_recovery.rs` and `Tests/TTZipTests/`.

## Phase 3: Swift Redundancy Pruning & Zero-Fat Thinning (US3)
- [x] T009 [P] [US3] Refactor `Sources/TTZipCore/Split/SplitVolumeEngine.swift` to delegate directly to Rust split engine, maintaining LOC < 350.
- [x] T010 [P] [US3] Re-verify all first-party Swift files in `Sources/TTZipCore/` and `Sources/TTZipBench/` strictly adhere to `< 350 LOC`.
- [x] T011 [P] [US3] Run full `swift test` suite.
- [x] T012 [P] [US3] Run full `cargo test` suite across all crates.

## Phase 4: Verification, CI Gates & Standalone Validation (US4)
- [ ] T013 [US4] Run `./scripts/build_rust.sh --release && ./scripts/build_tui.sh` and verify universal libraries and `bin/ttzip`.
- [ ] T014 [US4] Run `swift test` ensuring all 897+ tests pass with 0 failures and 0 warnings.
- [ ] T015 [US4] Run `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
