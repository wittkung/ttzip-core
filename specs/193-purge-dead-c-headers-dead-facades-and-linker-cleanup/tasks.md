# Tasks: 193-purge-dead-c-headers-dead-facades-and-linker-cleanup

## Phase 1: Purge Dead C Headers (US1)
- [x] T001 [P] [US1] Delete 20+ obsolete C headers from `Sources/CTTZipBridge/include/`.
- [x] T002 [P] [US1] Update `scripts/lint_codebase_standards.sh` to remove exclusions for deleted C headers.

## Phase 2: Purge Dead Facades (US2)
- [x] T003 [P] [US2] Delete `ArchiveOperationsFacade.swift`, `ArchiveSecurityFacade.swift`, `ArchiveStreamingFacade.swift`, and `TTZipEngineFacade+TemplateAndProxies.swift` from `Sources/TTZipCore/Facades/`.

## Phase 3: Package.swift & Final CI Verification (US3)
- [x] T004 [US3] Clean up `Package.swift` linker settings.
- [x] T005 [US3] Run `./scripts/lint_loc_gate.sh` to ensure all files $\le 800\text{ LOC}$.
- [x] T006 [US3] Verify `swift build` and `swift test` 100% PASS with zero warnings.
- [x] T007 [US3] Run `cargo test --workspace` on all Rust crates.
- [x] T008 [US3] Run `./scripts/run_local_ci_gate.sh` full CI validation.
