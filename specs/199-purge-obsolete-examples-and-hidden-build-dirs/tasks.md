# Tasks: 199-purge-obsolete-examples-and-hidden-build-dirs

## Phase 1: Dead Example & Hidden Residue Purge (US1 & US2)
- [x] T001 [P] [US1] Delete `examples/` directory (`examples/quickstart.c`).
- [x] T002 [P] [US2] Delete `.build_custom/`, `.build_di_test/`, and `.build_tmp/`.

## Phase 2: CI Alignment & Final Verification (US3)
- [x] T003 [US3] Run `./scripts/lint_loc_gate.sh` to ensure all files $\le 800\text{ LOC}$.
- [x] T004 [US3] Verify `swift build` and `swift test` 100% PASS with zero warnings.
- [x] T005 [US3] Run `cargo test --workspace` on all Rust crates.
- [x] T006 [US3] Run `./scripts/run_local_ci_gate.sh` full CI validation.
