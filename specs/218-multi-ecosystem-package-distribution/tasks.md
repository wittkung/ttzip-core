# Tasks: Multi-Ecosystem Package Distribution

**Feature**: `218-multi-ecosystem-package-distribution`  
**Directory**: `specs/218-multi-ecosystem-package-distribution`  

---

## Phase 1: Homebrew Official Tap (`wittkung/homebrew-ttzip`)

- [x] T001 [P] [US1] Create GitHub repository `wittkung/homebrew-ttzip`.
- [x] T002 [US1] Create local repository `/Users/kevintung/Documents/dev/homebrew-ttzip` with `Formula/ttzip.rb`.
- [x] T003 [US1] Push `Formula/ttzip.rb` to `wittkung/homebrew-ttzip`.

---

## Phase 2: Rust Crates.io Dry-Run Packaging

- [x] T004 [P] [US2] Audit and refine crate metadata in `rust/ttzip-engine/Cargo.toml`, `rust/ttzip-glue/Cargo.toml`, and `rust/ttzip-tui/Cargo.toml`.
- [x] T005 [US2] Execute `scripts/publish_crates.sh --dry-run` and verify 3 crates pass package creation.

---

## Phase 3: Python PyPI Maturin Wheel Build

- [x] T006 [P] [US3] Build production `.whl` package in `dist/` using Maturin backend.
- [x] T007 [US3] Verify wheel contents (`_ttzip.abi3.so`, PEP 561 stubs) and test local pip installation.

---

## Phase 4: Unified Verification & Verification Gate

- [x] T008 [P] [US4] Create `scripts/verify_distribution.sh` executing all 3 distribution dry-runs in sequence.
- [x] T009 [US4] Execute `scripts/lint_loc_gate.sh` to enforce $\le 800\text{ LOC}$ across all modified/new files.
- [x] T010 [US4] Synchronize updates to `../ttzip-core`.
