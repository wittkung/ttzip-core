# Tasks: 197-purge-broken-scripts-and-deduplicate-site-docs

## Phase 1: Broken Script Purge (US1)
- [x] T001 [P] [US1] Delete `scripts/run_delta_audit.sh`.

## Phase 2: Site Deduplication & Repository Hygiene (US2)
- [x] T002 [P] [US2] Delete `site/` directory (8 duplicate files).
- [x] T003 [P] [US2] Delete loose duplicate files in `docs/` (`docs/index.html`, `docs/privacy.html`, `docs/terms.html`, `docs/CNAME`, `docs/appcast.xml`).
- [x] T004 [P] [US2] Update `.gitattributes` to remove `site/**` rule.

## Phase 3: CI Alignment & Final Verification (US3)
- [x] T005 [US3] Run `./scripts/lint_loc_gate.sh` to ensure all files $\le 800\text{ LOC}$.
- [x] T006 [US3] Verify `swift build` and `swift test` 100% PASS with zero warnings.
- [x] T007 [US3] Run `cargo test --workspace` on all Rust crates.
- [x] T008 [US3] Run `./scripts/run_local_ci_gate.sh` full CI validation.
