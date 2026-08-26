# Implementation Plan: 187-ultimate-swift-legacy-code-purge-and-thin-facade

## Technical Context
- **Objective**: Purge 200+ redundant legacy Swift files, consolidate TTZipCore into ~30 ultra-thin facades, and verify 0 regressions.

---

## Constitution Check
- [x] **Safe Architecture**: 100% of files maintained strictly under $< 350\text{ LOC}$.
- [x] **Zero Cloud Actions Quota**: 100% local validation.
- [x] **Zero Regression**: 100% pass rate on test suite.

---

## Phase 0: Research Items
- R001 [SUBAGENT:research] 《独立超薄 Swift 门面收敛方案》: Completed.
- R002 [SUBAGENT:research] 《冗余测试清理与双端门禁对齐》: Completed.

---

## Phase 1: Component Change List

### 1. Ultra-Thin Facade Self-Containment
- Update `ArchiveWriter.swift`, `ArchiveExtractor.swift`, `ArchiveReader.swift` to ensure 0 imports/references to `Zip/`, `SevenZip/`, `TemplateMethod/`, `StatePattern/`, etc.

### 2. Purge Legacy Swift Directories
- Delete `Sources/TTZipCore/Zip/`
- Delete `Sources/TTZipCore/SevenZip/`
- Delete `Sources/TTZipCore/Zstd/`
- Delete `Sources/TTZipCore/Snappy/`
- Delete `Sources/TTZipCore/Tar/`
- Delete `Sources/TTZipCore/TemplateMethod/`
- Delete `Sources/TTZipCore/StatePattern/`
- Delete `Sources/TTZipCore/MediatorPattern/`
- Delete `Sources/TTZipCore/MementoPattern/`
- Delete `Sources/TTZipCore/ChainOfResponsibility/`
- Delete `Sources/TTZipCore/Decorators/`
- Delete `Sources/TTZipCore/Flyweights/`
- Delete `Sources/TTZipCore/Observers/`
- Delete `Sources/TTZipCore/Adaptive/`
- Delete `Sources/TTZipCore/DependencyInjection/`
- Delete `Sources/TTZipCore/IteratorPattern/`

### 3. Test Alignment & Verification
- Remove test files referencing deleted internal classes.
- Verify `swift test` and `cargo test --workspace`.
- Run `./scripts/run_local_ci_gate.sh`.

---

## Phase 2: Verification Plan
1. `cargo test --workspace` on all Rust crates.
2. `swift test` ensuring all remaining tests pass with 0 failures and 0 warnings.
3. `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
