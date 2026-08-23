# Implementation Plan: 173-refactor-god-files-and-modular-decomposition

## Technical Context
- **Target Line Limit**: <= 500 lines per first-party file (ideal target: 100~350 LOC).
- **Target Files**:
  - Swift Core: `StandardsComplianceChecker.swift` (1,356 LOC), `ArchiveFormatStandardSpec.swift` (923 LOC), `TTZipEngineFacade.swift` (907 LOC), `DifferentialOracleTestHarness.swift` (782 LOC), `DeflateStreamEngine.swift` (644 LOC), `ArchiveCompressionTypes.swift` (643 LOC).
  - Rust Glue & TUI: `vfs.rs` (897 LOC), `codecs_ffi.rs` (886 LOC), `main.rs` (802 LOC), `archive_ffi.rs` (695 LOC), `app.rs` (671 LOC).
  - CLI, App & Benchmark: `CLICommandRouter.swift` (873 LOC), `RasterParetoPlotter.swift` (747 LOC), `AppViewState.swift` (709 LOC), `ArchiveExplorerView.swift` (562 LOC), `FinderMillerColumnsView.swift` (556 LOC).
- **Zero Breaking Changes Guarantee**: All existing public API signatures, CLI command invocations, C-ABI bindings, and SwiftUI view bindings remain 100% functionally and behaviorally identical.

---

## Constitution Check
- [x] **Principle 1: Single Responsibility & Modularity**: Each decomposed file has a single well-defined responsibility.
- [x] **Principle 2: Safe Rust C-ABI Stability**: All 30 global exported C symbols in `ttzip_rust_glue.h` remain exact.
- [x] **Principle 3: Swift 6 Strict Concurrency**: All `extension` methods preserve `@MainActor` and `Sendable` guarantees.
- [x] **Principle 4: Zero Regression & Full CI/CD Compliance**: All 850+ unit tests, property tests, fuzzing, and benchmark gates must pass 100% without bypassing verification.

---

## Phase 0: Research Items Index
- R001 [SUBAGENT:research] 《Swift Core 巨型门面与标准检测器拆解方案》: Completed (see `research.md`).
- R002 [SUBAGENT:research] 《Rust Glue FFI 与 TUI 模块化拆分方案》: Completed (see `research.md`).
- R003 [SUBAGENT:research] 《UI & CLI 业务大文件模块化拆解方案》: Completed (see `research.md`).

---

## Phase 1: Architecture Artifacts & Component Change List

### 1. Swift Core Standards & Facades (`Sources/TTZipCore/`)
- `Standards/StandardsComplianceChecker.swift` + `Standards/Compliance/*.swift` (6 files)
- `Standards/ArchiveFormatStandardSpec.swift` + `Standards/ArchiveFormatStandardRegistry.swift` + `Standards/Registry/*.swift` (5 files)
- `Facades/TTZipEngineFacading.swift` + `Facades/TTZipEngineFacade.swift` + `Facades/TTZipEngineFacade+*.swift` (6 files)
- `Testing/DifferentialManifestModels.swift` + `Testing/DifferentialManifestScanner.swift` + `Testing/DifferentialManifestVerifier.swift` + `Testing/DifferentialOracleRegistry.swift` + `Testing/DifferentialOracleTestHarness.swift` (5 files)
- `Benchmark/RasterParetoPlotter.swift` + `Benchmark/RasterParetoPlotter+*.swift` (5 files)

### 2. Rust Glue & TUI (`rust/`)
- `rust/ttzip-glue/src/ffi/codecs_ffi/` (`mod.rs`, `deflate.rs`, `zstd.rs`, `lzma2.rs`, `fast_blocks.rs`, `chardet.rs`)
- `rust/ttzip-glue/src/ffi/archive_ffi/` (`mod.rs`, `sys.rs`, `guards.rs`, `inspect.rs`, `extract.rs`, `create.rs`)
- `rust/ttzip-tui/src/vfs/` (`mod.rs`, `meta.rs`, `node.rs`, `view.rs`, `search.rs`, `tree.rs`, `tests.rs`)
- `rust/ttzip-tui/src/cli/` (`mod.rs`, `args.rs`, `format.rs`, `handlers.rs`, `tui_runner.rs`, `tests.rs`) + `rust/ttzip-tui/src/main.rs`
- `rust/ttzip-tui/src/app/` (`mod.rs`, `types.rs`, `state.rs`, `input.rs`, `extract.rs`, `preview.rs`, `tests.rs`)

### 3. Swift CLI & App ViewModels/Views (`Sources/TTZipCLI/`, `Sources/TTZipApp/`)
- `Sources/TTZipCLI/CLIConsoleObserver.swift` + `Sources/TTZipCLI/CLICommandRouter.swift` + `Sources/TTZipCLI/CLICommandRouter+*.swift` (7 files)
- `Sources/TTZipApp/ViewModels/RecentArchiveRecord.swift` + `Sources/TTZipApp/ViewModels/AppViewState.swift` + `Sources/TTZipApp/ViewModels/AppViewState+*.swift` (6 files)
- `Sources/TTZipApp/Views/ArchiveExplorerView.swift` + `Sources/TTZipApp/Views/Explorer/ArchiveExplorerHeaderBar.swift` + `Sources/TTZipApp/Views/Explorer/ArchiveExplorerTableView.swift` + `Sources/TTZipApp/Views/ArchiveExplorerView+Operations.swift` (4 files)
- `Sources/TTZipApp/Views/Explorer/FinderMillerColumnsView.swift` + `Sources/TTZipApp/Views/Explorer/FinderMillerColumnsView+*.swift` (4 files)

---

## Phase 2: Verification Plan
1. `cargo check` & `cargo test` on all Rust crates (`ttzip-glue`, `ttzip-tui`).
2. `./scripts/build_rust.sh --release` & `./scripts/build_tui.sh`.
3. `swift build` & `swift test` across all targets.
4. `./scripts/run_local_ci_gate.sh` full 7-stage validation.
5. Final physical line count audit confirming zero first-party files >= 500 LOC.
