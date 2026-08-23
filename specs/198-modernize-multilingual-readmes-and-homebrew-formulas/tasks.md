# Tasks: 198-modernize-multilingual-readmes-and-homebrew-formulas

## Phase 1: Script & Formula Modernization (US1)
- [x] T001 [P] [US1] Delete `安装TTZip.command` (keep `Install-TTZip.command`).
- [x] T002 [P] [US1] Update `scripts/package_local_release.sh` to generate both `Formula/ttzip-cli.rb` and `Formula/ttzip.rb`.
- [x] T003 [P] [US1] Update `Formula/ttzip.rb` to match `Formula/ttzip-cli.rb`.

## Phase 2: Multilingual README Alignment (US2)
- [x] T004 [P] [US2] Modernize `README.md` to Swift 6 + Safe Rust architecture and SwiftPM/Cargo build commands ($\le 800\text{ LOC}$).
- [x] T005 [P] [US2] Modernize `README_zh.md` to Swift 6 + Safe Rust architecture and SwiftPM/Cargo build commands ($\le 800\text{ LOC}$).
- [x] T006 [P] [US2] Modernize `README_ja.md` to Swift 6 + Safe Rust architecture and SwiftPM/Cargo build commands ($\le 800\text{ LOC}$).
- [x] T007 [P] [US2] Modernize `README_ko.md` to Swift 6 + Safe Rust architecture and SwiftPM/Cargo build commands ($\le 800\text{ LOC}$).

## Phase 3: CI Alignment & Final Verification (US3)
- [x] T008 [US3] Run `./scripts/lint_loc_gate.sh` to ensure all files $\le 800\text{ LOC}$.
- [x] T009 [US3] Verify `swift build` and `swift test` 100% PASS with zero warnings.
- [x] T010 [US3] Run `cargo test --workspace` on all Rust crates.
- [x] T011 [US3] Run `./scripts/run_local_ci_gate.sh` full CI validation.
