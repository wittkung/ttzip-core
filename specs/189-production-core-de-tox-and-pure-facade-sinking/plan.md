# Implementation Plan: 189-production-core-de-tox-and-pure-facade-sinking

## Technical Context
- **Objective**: Eliminate 33 misplaced/redundant files from `Sources/TTZipCore/` (testing harnesses, mocks, duplicate DSL interpreters, visitor traversals, legacy stream pipelines).

---

## Constitution Check
- [x] **Production Purity**: Zero test harnesses or mock facades inside `Sources/TTZipCore/`.
- [x] **Zero Cloud Actions Quota**: 100% local validation.
- [x] **Single Source of Truth**: Filter DSL, VFS trees, and Codecs 100% in Rust.

---

## Phase 0: Research Items
- R001 [SUBAGENT:research] 《Filter DSL 直调 Rust C-ABI 方案》: Completed.
- R002 [SUBAGENT:research] 《生产核心库解耦 Testing 与 Mocks》: Completed.

---

## Phase 1: Component Change List

### 1. Facade Self-Containment & Rust Delegation
- Direct `ArchiveFilter` to `ttzip_rust_eval_filter_dsl`.
- Direct `VfsTreeRenderer` to `ttzip_rust_vfs_tree_render`.

### 2. Purge Misplaced & Redundant Files
- Delete `Sources/TTZipCore/Testing/` (17 files).
- Delete `Sources/TTZipCore/Mocks/` (1 file).
- Delete `Sources/TTZipCore/InterpreterPattern/` (5 files).
- Delete `Sources/TTZipCore/VisitorPattern/` (7 files).
- Delete `Sources/TTZipCore/Pipeline/DeflateStreamEngine*.swift` (3 files).

### 3. Verification Plan
1. `swift build` and `swift test` pass cleanly.
2. `cargo test --workspace` passes cleanly.
3. `./scripts/run_local_ci_gate.sh` passes 100%.
