# Tasks: 192-cli-realignment-legacy-adapter-purge-and-scripts-cleanup

## Phase 1: CLI Domain Realignment (US1)
- [x] T001 [P] [US1] Move `Sources/TTZipCore/CLI/` (20 files) to `Sources/TTZipCLI/`.

## Phase 2: Purge Legacy Adapters & Patterns (US2)
- [x] T002 [P] [US2] Delete `Sources/TTZipCore/Adapters/` (9 files).
- [x] T003 [P] [US2] Delete `Sources/TTZipCore/Proxies/` (4 files).
- [x] T004 [P] [US2] Delete `Sources/TTZipCore/RepositoryPattern/` (7 files).

## Phase 3: Script Consolidation (US3)
- [x] T005 [P] [US3] Delete 15 obsolete/duplicate scripts in `scripts/`.

## Phase 4: CI Alignment & Final Verification (US4)
- [x] T006 [US4] Run `./scripts/lint_loc_gate.sh` to ensure all files $\le 800\text{ LOC}$.
- [x] T007 [US4] Verify `swift build` and `swift test` 100% PASS with zero warnings.
- [x] T008 [US4] Run `cargo test --workspace` on all Rust crates.
- [x] T009 [US4] Run `./scripts/run_local_ci_gate.sh` full CI validation.
