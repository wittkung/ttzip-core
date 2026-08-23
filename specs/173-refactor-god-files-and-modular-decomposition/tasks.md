# Tasks: 173-refactor-god-files-and-modular-decomposition

## Phase 1: Swift Core Standards & Facades Decomposition (US1)
- [x] T001 [P] [US1] Decompose `Sources/TTZipCore/Standards/StandardsComplianceChecker.swift` (1,356 LOC) into main dispatcher + `Compliance/` submodules (`+Zip.swift`, `+Tar.swift`, `+Modern.swift`, `+Streams.swift`, `+DiskImages.swift`).
- [x] T002 [P] [US1] Decompose `Sources/TTZipCore/Standards/ArchiveFormatStandardSpec.swift` (923 LOC) into models (`ArchiveFormatStandardSpec.swift`), registry (`ArchiveFormatStandardRegistry.swift`), and `Registry/` category extensions (`+Archives.swift`, `+Streams.swift`, `+DiskImages.swift`).
- [x] T003 [P] [US1] Decompose `Sources/TTZipCore/Facades/TTZipEngineFacade.swift` (907 LOC) into `TTZipEngineFacading.swift`, `TTZipEngineFacade.swift`, and domain extensions (`+Compress.swift`, `+Extract.swift`, `+Inspect.swift`, `+TemplateAndProxies.swift`).
- [x] T004 [P] [US1] Decompose `Sources/TTZipCore/Testing/DifferentialOracleTestHarness.swift` (782 LOC) into `DifferentialManifestModels.swift`, `DifferentialManifestScanner.swift`, `DifferentialManifestVerifier.swift`, `DifferentialOracleRegistry.swift`, `DifferentialOracleTestHarness.swift`.
- [x] T005 [P] [US1] Decompose `Sources/TTZipCore/Benchmark/RasterParetoPlotter.swift` (747 LOC) into `RasterParetoPlotter.swift`, `+Coordinates.swift`, `+Axes.swift`, `+Trajectories.swift`, `+ScatterLabels.swift`.

## Phase 2: Rust Glue & TUI Decomposition (US2)
- [x] T006 [P] [US2] Decompose `rust/ttzip-glue/src/ffi/codecs_ffi.rs` (886 LOC) into directory module `rust/ttzip-glue/src/ffi/codecs_ffi/` (`mod.rs`, `deflate.rs`, `zstd.rs`, `lzma2.rs`, `fast_blocks.rs`, `chardet.rs`).
- [x] T007 [P] [US2] Decompose `rust/ttzip-glue/src/ffi/archive_ffi.rs` (695 LOC) into directory module `rust/ttzip-glue/src/ffi/archive_ffi/` (`mod.rs`, `sys.rs`, `guards.rs`, `inspect.rs`, `extract.rs`, `create.rs`).
- [x] T008 [P] [US2] Decompose `rust/ttzip-tui/src/vfs.rs` (897 LOC) into directory module `rust/ttzip-tui/src/vfs/` (`mod.rs`, `meta.rs`, `node.rs`, `view.rs`, `search.rs`, `tree.rs`, `tests.rs`).
- [x] T009 [P] [US2] Decompose `rust/ttzip-tui/src/main.rs` (802 LOC) into `rust/ttzip-tui/src/cli/` (`mod.rs`, `args.rs`, `format.rs`, `handlers.rs`, `tui_runner.rs`, `tests.rs`) and a concise `main.rs`.
- [x] T010 [P] [US2] Decompose `rust/ttzip-tui/src/app.rs` (671 LOC) into directory module `rust/ttzip-tui/src/app/` (`mod.rs`, `types.rs`, `state.rs`, `input.rs`, `extract.rs`, `preview.rs`, `tests.rs`).

## Phase 3: Swift CLI & App ViewModels/Views Decomposition (US3)
- [x] T011 [P] [US3] Decompose `Sources/TTZipCLI/CLICommandRouter.swift` (873 LOC) into `CLIConsoleObserver.swift`, `CLICommandRouter.swift`, and subcommand extensions (`+Compress.swift`, `+Extract.swift`, `+Inspect.swift`, `+Maintenance.swift`, `+Benchmark.swift`).
- [x] T012 [P] [US3] Decompose `Sources/TTZipApp/ViewModels/AppViewState.swift` (709 LOC) into `RecentArchiveRecord.swift`, `AppViewState.swift`, and domain extensions (`+Mediator.swift`, `+Tasks.swift`, `+ArchiveOperations.swift`, `+Commands.swift`).
- [x] T013 [P] [US3] Decompose `Sources/TTZipApp/Views/ArchiveExplorerView.swift` (562 LOC) into `ArchiveExplorerView.swift`, `Explorer/ArchiveExplorerHeaderBar.swift`, `Explorer/ArchiveExplorerTableView.swift`, `ArchiveExplorerView+Operations.swift`.
- [x] T014 [P] [US3] Decompose `Sources/TTZipApp/Views/Explorer/FinderMillerColumnsView.swift` (556 LOC) into `FinderMillerColumnsView.swift`, `+Navigation.swift`, `+Selection.swift`, `+FileOps.swift`.
- [x] T015 [P] [US3] Decompose `Sources/TTZipCore/CLI/TUI/InteractiveTUIExplorer.swift` (557 LOC) & `Sources/TTZipCore/CLI/CLICommandSpec.swift` (572 LOC) into modular extensions.

## Phase 4: Core Compression Pipelines & Types Decomposition (US4)
- [x] T016 [P] [US4] Decompose `Sources/TTZipCore/Pipeline/DeflateStreamEngine.swift` (644 LOC) & `ArchiveCompressionTypes.swift` (643 LOC).
- [x] T017 [P] [US4] Decompose `rust/ttzip-glue/src/zip/writer.rs` (571 LOC) & `rust/ttzip-glue/src/crypto/aes256.rs` (571 LOC).
- [x] T018 [P] [US4] Decompose `Sources/TTZipCLI/TestCommand.swift` (539 LOC) & `Sources/TTZipBench/main.swift` (508 LOC).

## Phase 5: Build, Test & CI Gate Verification (US5)
- [x] T019 [US5] Run `cargo check` and `cargo test` across all Rust workspace crates (`ttzip-glue`, `ttzip-tui`).
- [x] T020 [US5] Run `./scripts/build_rust.sh --release` and `./scripts/build_tui.sh`.
- [x] T021 [US5] Run `swift test` across all 850+ tests ensuring 100% green.
- [x] T022 [US5] Run `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
- [x] T023 [US5] Execute physical LOC scan ensuring zero first-party source files exceed 500 lines.
