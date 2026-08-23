# Implementation Plan: 198-modernize-multilingual-readmes-and-homebrew-formulas

## Technical Context
- Delete `安装TTZip.command`.
- Update `scripts/package_local_release.sh` to generate both `Formula/ttzip-cli.rb` and `Formula/ttzip.rb`.
- Modernize `README.md`, `README_zh.md`, `README_ja.md`, `README_ko.md`.

---

## Constitution Check
- [x] Zero Cloud Quota.
- [x] Single-file LOC $\le 800$.

---

## Phase 0: Research Items
- R001 [SUBAGENT:research] 《4 语言 README 与现代双核架构对齐方案》: Completed.
- R002 [SUBAGENT:research] 《Homebrew Formula 双别名自动化发布方案》: Completed.

---

## Phase 1: Script & Formula Modernization
- Delete `安装TTZip.command`.
- Update `scripts/package_local_release.sh` to generate both `Formula/ttzip-cli.rb` and `Formula/ttzip.rb`.
- Update `Formula/ttzip.rb`.

## Phase 2: Multilingual README Alignment
- Update `README.md` (English).
- Update `README_zh.md` (Simplified Chinese).
- Update `README_ja.md` (Japanese).
- Update `README_ko.md` (Korean).

## Phase 3: Verification & Gate
- Run `./scripts/lint_loc_gate.sh`.
- Run `swift test` and `cargo test --workspace`.
- Run `./scripts/run_local_ci_gate.sh`.
