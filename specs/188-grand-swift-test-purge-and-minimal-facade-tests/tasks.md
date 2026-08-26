# Tasks: 188-grand-swift-test-purge-and-minimal-facade-tests

## Phase 1: Create Unified High-Level Facade Test (US1)
- [x] T001 [P] [US1] Create `Tests/TTZipTests/TTZipCoreIntegrationTests.swift` covering `ArchiveWriter`, `ArchiveExtractor`, `ArchiveReader`, `SplitVolumeEngine`, `PasswordVaultManager`, and `PasswordRecoveryEngine`.
- [x] T002 [P] [US1] Retain essential system and CLI tests: `CLICommandE2ETests.swift`, `CLIPOSIXStandardTests.swift`, `QuickLookPreviewTests.swift`, `AppStorePackageAuditTests.swift`.

## Phase 2: Purge 70+ Redundant Low-Level Swift Test Files (US2)
- [x] T003 [P] [US2] Delete all remaining low-level stream, matrix, fuzzing, and pseudo-pattern test files in `Tests/TTZipTests/`.
- [x] T004 [P] [US2] Run `swift test` and confirm clean, lightning-fast execution in $<1.0\text{s}$.

## Phase 3: CI Gate Streamlining & Final Verification (US3)
- [x] T005 [US3] Update `./scripts/run_local_ci_gate.sh` to reflect streamlined test stages.
- [x] T006 [US3] Run `cargo test --workspace` on all Rust crates.
- [x] T007 [US3] Run `swift test` on the streamlined Swift test suite.
- [x] T008 [US3] Run `./scripts/run_local_ci_gate.sh` full CI validation.
