# Tasks: 183-archive-dispatch-decoupling-and-protocol-modularization

## Phase 1: Archive Dispatch & Writer SRP Decomposition (US1)
- [x] T001 [P] [US1] Decompose `Sources/TTZipCore/Bridge/ArchiveEngineBridge.swift` into `ArchiveEngineBridge.swift` and `ArchiveEngineBridge+Formats.swift`, maintaining LOC < 350.
- [x] T002 [P] [US1] Decompose `Sources/TTZipCore/ArchiveWriter+Dispatch.swift` into `ArchiveWriter+ZipDispatch.swift` and `ArchiveWriter+TarSevenZipDispatch.swift`, maintaining LOC < 350.
- [x] T003 [P] [US1] Decompose `Sources/TTZipCore/Commands/CompressCommand.swift` into `CompressCommand.swift` and `CompressCommand+Validation.swift`, maintaining LOC < 350.
- [x] T004 [P] [US1] Verify archive dispatch with `swift test --filter ArchiveWriterTests`.

## Phase 2: Strategy & Component Protocol Modularization (US2)
- [x] T005 [P] [US2] Decompose `Sources/TTZipCore/Strategies/CompressionStrategyProtocol.swift` into `CompressionStrategyProtocol.swift` and `CompressionStrategyFactory.swift`, maintaining LOC < 350.
- [x] T006 [P] [US2] Decompose `Sources/TTZipCore/ArchiveComponentProtocol.swift` into `ArchiveComponentProtocol.swift` and `ArchiveComponentTraversals.swift`, maintaining LOC < 350.
- [x] T007 [P] [US2] Decompose `Sources/TTZipCore/ArchiveProtocols.swift` into `ArchiveProtocols.swift` and `ArchiveEngineConformances.swift`, maintaining LOC < 350.
- [x] T008 [P] [US2] Verify strategy and protocols with `swift test --filter StrategyPatternTests`.

## Phase 3: Terminal Renderer & Formatting Segregation (US3)
- [x] T009 [P] [US3] Decompose `Sources/TTZipCore/Testing/TestTerminalRenderer.swift` into `TestTerminalRenderer.swift` and `TestTerminalANSIFormatter.swift`, maintaining LOC < 350.
- [x] T010 [P] [US3] Verify LOC compliance across all files in `TTZipCore` (< 350 LOC).
- [x] T011 [P] [US3] Run full `swift test` suite.
- [x] T012 [P] [US3] Run `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
