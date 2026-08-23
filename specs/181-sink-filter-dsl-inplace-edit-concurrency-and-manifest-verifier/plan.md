# Implementation Plan: 181-sink-filter-dsl-inplace-edit-concurrency-and-manifest-verifier

## Technical Context
- **Objective**: Complete the final round of deep non-Rust code sinking, moving Filter DSL Lexer/Parser, In-Place Archive Editing, Concurrency Buffers, and Manifest Verifiers into Safe Rust (`rust/ttzip-glue`), while consolidating Swift design pattern facades.

---

## Constitution Check
- [x] **Safe Rust Engine**: Filter DSL, In-Place Edit, Concurrency, and Differential Verifiers all in Safe Rust.
- [x] **Zero Cloud Actions Quota**: 100% local validation.
- [x] **SRP & LOC Budget**: All files maintained strictly under $< 350\sim 500\text{ LOC}$.

---

## Phase 0: Research Items
- R001 [SUBAGENT:research] 《Filter DSL 词法解析与 AST 零分配评估引擎》: Completed.
- R002 [SUBAGENT:research] 《就地归档原子编辑与 TOC 增量重写引擎》: Completed.
- R003 [SUBAGENT:research] 《多核差异化清单扫描器与 Golden Corpus 验证器》: Completed.

---

## Phase 1: Component Change List

### 1. Rust Glue Layer
- **`rust/ttzip-glue/src/fs/filter_dsl.rs`**: Filter DSL Lexer, AST Parser, and evaluation engine.
- **`rust/ttzip-glue/src/archive/in_place_edit.rs`**: Atomic in-place entry append, replace, and delete.
- **`rust/ttzip-glue/src/testing/differential.rs`**: Multi-threaded tree hashing and differential manifest comparison.
- **`rust/ttzip-glue/src/ffi/`**: Export unified C-ABIs.

### 2. Swift Facades & Bridges
- **`Sources/TTZipCore/InterpreterPattern/ArchiveFilterDSLLexerParser.swift`**: Delegate AST evaluation to Rust C-ABI.
- **`Sources/TTZipCore/InPlaceEdit/InPlaceEditEngine.swift`**: Delegate in-place editing to Rust C-ABI.
- **`Sources/TTZipCore/Testing/DifferentialManifestScanner.swift`** & **`DifferentialManifestVerifier.swift`**: Delegate to Rust differential engine.
- **`Sources/TTZipCore/Facades/`** & **`Sources/TTZipCore/TemplateMethod/`**: Consolidate and thin out.

---

## Phase 2: Verification Plan
1. `cargo test --workspace` on all Rust crates.
2. `./scripts/build_rust.sh --release && ./scripts/build_tui.sh`.
3. `swift test` ensuring all 885+ tests pass with 0 failures and 0 warnings.
4. `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
