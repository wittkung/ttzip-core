# Tasks: 196-purge-legacy-c-test-harness-obsolete-cli-and-relic-build-dirs

## Phase 1: Purge Obsolete C CLI & Legacy C Test Suites (US1)
- [x] T001 [P] [US1] Delete `cli/` directory (`cli/main.c`).
- [x] T002 [P] [US1] Delete `Tests/c/` (35 files) and `Tests/fuzz/` (2 files).

## Phase 2: Purge Root Build Debris (US2)
- [x] T003 [P] [US2] Delete `build/`, `build_asan/`, `build_dist/`, and `scratch/` (> 605 MB).

## Phase 3: Script & Architecture Alignment (US3)
- [x] T004 [P] [US3] Delete `scripts/build_mas.sh`.
- [x] T005 [P] [US3] Update `ARCHITECTURE.md` to reflect modern Swift 6 + Safe Rust microkernel architecture.
- [x] T006 [P] [US3] Update `.gitignore` to remove outdated patterns (`!tests/c/**`, `Vendor/lib/`, `Vendor/libTTZipVendor.a`).

## Phase 4: CI Alignment & Final Verification (US4)
- [x] T007 [US4] Run `./scripts/lint_loc_gate.sh` to ensure all files $\le 800\text{ LOC}$.
- [x] T008 [US4] Verify `swift build` and `swift test` 100% PASS with zero warnings.
- [x] T009 [US4] Run `cargo test --workspace` on all Rust crates.
- [x] T010 [US4] Run `./scripts/run_local_ci_gate.sh` full CI validation.
