# Tasks: 194-purge-dead-services-legacy-vendor-artifacts-and-cmake-relics

## Phase 1: Purge Dead Services & Utilities (US1)
- [x] T001 [P] [US1] Delete 8 dead service/utility files from `Sources/TTZipCore/`.

## Phase 2: Purge Legacy Vendor Artifacts (US2)
- [x] T002 [P] [US2] Delete `Vendor/include/` and `Vendor/lib/`.

## Phase 3: Purge CMake & Duplicate Root Files (US3)
- [x] T003 [P] [US3] Delete `CMakeLists.txt`, `cmake/`, and root `reinstall.sh`.

## Phase 4: CI Alignment & Final Verification (US4)
- [x] T004 [US4] Run `./scripts/lint_loc_gate.sh` to ensure all files $\le 800\text{ LOC}$.
- [x] T005 [US4] Verify `swift build` and `swift test` 100% PASS with zero warnings.
- [x] T006 [US4] Run `cargo test --workspace` on all Rust crates.
- [x] T007 [US4] Run `./scripts/run_local_ci_gate.sh` full CI validation.
