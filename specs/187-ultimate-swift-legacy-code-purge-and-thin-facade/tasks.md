# Tasks: 187-ultimate-swift-legacy-code-purge-and-thin-facade

## Phase 1: Self-Contained Ultra-Thin Swift Facades (US1)
- [x] T001 [P] [US1] Update `ArchiveWriter.swift` and `ArchiveWriter+Helpers.swift` to invoke Rust C-ABI directly with 0 dependencies on `Zip/` or `SevenZip/`.
- [x] T002 [P] [US1] Update `ArchiveExtractor.swift` and `ArchiveReader.swift` to invoke Rust C-ABI directly.
- [x] T003 [P] [US1] Update `ArchiveIntegrityChecker.swift` and `ArchiveRepairEngine.swift` to invoke Rust C-ABI directly.

## Phase 2: Purge 200+ Redundant Legacy Swift Files (US2)
- [x] T004 [P] [US2] Delete `Sources/TTZipCore/Zip/` (18 files).
- [x] T005 [P] [US2] Delete `Sources/TTZipCore/SevenZip/` (15 files).
- [x] T006 [P] [US2] Delete `Sources/TTZipCore/Zstd/` (4 files), `Snappy/` (3 files), `Tar/` (8 files).
- [x] T007 [P] [US2] Delete `TemplateMethod/`, `StatePattern/`, `MediatorPattern/`, `MementoPattern/`, `ChainOfResponsibility/`, `Decorators/`, `Flyweights/`, `Observers/`, `Adaptive/`, `DependencyInjection/`, `IteratorPattern/` (65+ files).
- [x] T008 [P] [US2] Delete redundant services (`FolderStatsCalculator.swift`, `ArchiveDiskPreallocator.swift`, `ArchiveEntropyEvaluator.swift`, `SmartCodecSelector.swift`, `SolidArchiveEngine.swift`).

## Phase 3: Test Alignment & Verification (US3)
- [x] T009 [US3] Remove obsolete Swift test files referencing purged internal scaffolding.
- [x] T010 [US3] Verify `swift build` and `swift test` cleanly pass with 100% success.
- [x] T011 [US3] Run `cargo test --workspace` on all Rust crates.
- [x] T012 [US3] Run `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
