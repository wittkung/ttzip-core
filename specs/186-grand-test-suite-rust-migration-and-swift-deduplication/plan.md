# Implementation Plan: 186-grand-test-suite-rust-migration-and-swift-deduplication

## Technical Context
- **Objective**: Migrate all low-level invariant and pattern tests to `rust/ttzip-glue/tests/`, and purge redundant Swift test files from `Tests/TTZipTests/`.

---

## Constitution Check
- [x] **Safe Rust Test Foundation**: 100% of engine invariants covered in Cargo integration suites.
- [x] **Zero Cloud Actions Quota**: 100% local validation.
- [x] **Clean Architecture**: 40+ redundant Swift test files removed.

---

## Phase 0: Research Items
- R001 [SUBAGENT:research] 《Swift 冗余测试清单盘点与清理策略》: Completed.
- R002 [SUBAGENT:research] 《Rust 集成测试套件扩充与跨平台自闭环》: Completed.

---

## Phase 1: Component Change List

### 1. Rust Integration Test Suite Consolidation
- **`rust/ttzip-glue/tests/pattern_and_pipeline_tests.rs`**: Rayon concurrency pipelines, worker pools, in-place edit sessions.
- **`rust/ttzip-glue/tests/container_matrix_tests.rs`**: Full 17-format matrix roundtrips and split-volume spanning.

### 2. Purge Redundant Swift Tests
- Delete 40+ pattern, buffer, and low-level tests in `Tests/TTZipTests/`.

### 3. Local CI Gate Alignment
- Update `./scripts/run_local_ci_gate.sh` to reference streamlined test suites.

---

## Phase 2: Verification Plan
1. `cargo test --workspace` on all Rust crates.
2. `swift test` ensuring all remaining Swift tests pass with 0 failures and 0 warnings.
3. `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
