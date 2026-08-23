# Implementation Plan: 199-purge-obsolete-examples-and-hidden-build-dirs

## Technical Context
- Delete `examples/` directory (`examples/quickstart.c`).
- Delete `.build_custom/`, `.build_di_test/`, `.build_tmp/`.

---

## Constitution Check
- [x] Zero Cloud Quota.
- [x] Single-file LOC $\le 800$.

---

## Phase 0: Research Items
- R001 [SUBAGENT:research] 《历史示例代码与隐藏构建目录清理方案》: Completed.

---

## Phase 1: Dead Example & Hidden Residue Purge
- Delete `examples/` directory.
- Delete `.build_custom/`, `.build_di_test/`, `.build_tmp/`.

## Phase 2: Verification & Gate
- Run `./scripts/lint_loc_gate.sh`.
- Run `swift test` and `cargo test --workspace`.
- Run `./scripts/run_local_ci_gate.sh`.
