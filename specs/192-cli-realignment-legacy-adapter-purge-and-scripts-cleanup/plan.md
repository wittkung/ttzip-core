# Implementation Plan: 192-cli-realignment-legacy-adapter-purge-and-scripts-cleanup

## Technical Context
- **Objective**: Realign 20 CLI files to `Sources/TTZipCLI/`, purge 20 legacy Swift files (`Adapters/`, `Proxies/`, `RepositoryPattern/`), and purge 15 obsolete scripts from `scripts/`.

---

## Constitution Check
- [x] **Target Purity**: `TTZipCore` contains 0 CLI code and 0 legacy C adapters.
- [x] **Zero Cloud Actions Quota**: 100% local validation.
- [x] **Single Source of Truth**: Unified scripts for CI, packaging, standards, and Rust build.

---

## Phase 0: Research Items
- R001 [SUBAGENT:research] 《CLI 逻辑完整归位至 TTZipCLI》: Completed.
- R002 [SUBAGENT:research] 《废弃 C 适配器与重复脚本大清理》: Completed.

---

## Phase 1: CLI Domain Realignment
- Move `Sources/TTZipCore/CLI/` (20 files) to `Sources/TTZipCLI/`.

## Phase 2: Legacy Adapters, Proxies & Repositories Purge
- Delete `Sources/TTZipCore/Adapters/` (9 files).
- Delete `Sources/TTZipCore/Proxies/` (4 files).
- Delete `Sources/TTZipCore/RepositoryPattern/` (7 files).

## Phase 3: Scripts Consolidation
- Delete 15 redundant/obsolete scripts from `scripts/`.

## Phase 4: Verification Plan
1. `swift build` and `swift test` pass cleanly.
2. `cargo test --workspace` passes cleanly.
3. `./scripts/lint_loc_gate.sh` and `./scripts/run_local_ci_gate.sh` pass 100%.
