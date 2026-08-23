# Implementation Plan: 196-purge-legacy-c-test-harness-obsolete-cli-and-relic-build-dirs

## Technical Context
- Purge obsolete `cli/` folder.
- Purge `Tests/c/` and `Tests/fuzz/`.
- Purge `build/`, `build_asan/`, `build_dist/`, and `scratch/`.
- Remove redundant `scripts/build_mas.sh`.
- Update `ARCHITECTURE.md` and `.gitignore`.

---

## Constitution Check
- [x] Zero Cloud Quota.
- [x] Single-file LOC $\le 800$.

---

## Phase 0: Research Items
- R001 [SUBAGENT:research] 《遗留 C 测试套件与 C CLI 替代路径审计》: Completed.
- R002 [SUBAGENT:research] 《根目录历史 CMake 构建残渣盘点》: Completed.

---

## Phase 1: Purge Obsolete C CLI & Legacy C Test Suites
- Delete `cli/` directory.
- Delete `Tests/c/` and `Tests/fuzz/` directories.

## Phase 2: Purge Root Build Debris
- Delete `build/`, `build_asan/`, `build_dist/`, `scratch/`.

## Phase 3: Script & Architecture Alignment
- Delete `scripts/build_mas.sh`.
- Update `ARCHITECTURE.md` to reflect modern dual-engine architecture.
- Update `.gitignore` to remove outdated patterns.

## Phase 4: Verification & Gate
- Run `./scripts/lint_loc_gate.sh`.
- Run `swift test` and `cargo test --workspace`.
- Run `./scripts/run_local_ci_gate.sh`.
