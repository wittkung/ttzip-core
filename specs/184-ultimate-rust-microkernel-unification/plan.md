# Implementation Plan: 184-ultimate-rust-microkernel-unification

## Technical Context
- **Objective**: Unify archive lifecycle orchestration and VFS tree indexing into Safe Rust microkernel, and transform Swift into a zero-fat native skin.

---

## Constitution Check
- [x] **Safe Rust Microkernel**: Unified archive creation, extraction, inspection, repair, and VFS indexing in Rust.
- [x] **Zero Cloud Actions Quota**: 100% local validation.
- [x] **SRP & LOC Budget**: 100% of files maintained strictly under $< 350\text{ LOC}$.

---

## Phase 0: Research Items
- R001 [SUBAGENT:research] 《Rust 统一归档生命周期调度引擎设计》: Completed.
- R002 [SUBAGENT:research] 《Rust 统一 VFS 树形索引与高性能模糊搜索》: Completed.

---

## Phase 1: Component Change List

### 1. Rust Glue Layer
- **`rust/ttzip-glue/src/archive/unified.rs`**: Single-entry archive lifecycle orchestrator (`create`, `extract`, `inspect`, `repair`, `test`).
- **`rust/ttzip-glue/src/fs/vfs.rs`**: Unified VFS tree builder, node hierarchy, and fuzzy search.
- **`rust/ttzip-glue/src/ffi/archive_ffi/unified.rs`**: Export C-ABI symbols.

### 2. Swift Facades & Bridges
- **`Sources/TTZipCore/ArchiveWriter.swift`** & **`ArchiveExtractor.swift`**: Direct delegation to unified Rust C-ABI.
- **`Sources/TTZipCore/ArchiveReader.swift`** & **`ArchiveRepairEngine.swift`**: Direct delegation to unified Rust C-ABI.
- **`Sources/TTZipCore/ArchiveComponentTraversals.swift`**: Direct delegation to Rust VFS engine.

---

## Phase 2: Verification Plan
1. `cargo test --workspace` on all Rust crates.
2. `./scripts/build_rust.sh --release && ./scripts/build_tui.sh`.
3. `swift test` ensuring all 893+ tests pass with 0 failures and 0 warnings.
4. `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
