# Tasks: Deep Swift Core Slimming Phase 2 (OOP Strategy Hierarchy Purge)

**Feature**: `204-deep-core-slimming`  
**Status**: `COMPLETED`  

---

## Tasks

- [x] **Task 1: Physical Deletion of Dead OOP Strategy Files**
  - [x] Delete `Sources/TTZipCore/Strategies/ArchiveRepairStrategyProtocol.swift`
  - [x] Delete `Sources/TTZipCore/Strategies/CharsetDetectionStrategyProtocol.swift`
  - [x] Delete `Sources/TTZipCore/Strategies/CompressionStrategyFactory.swift`
  - [x] Delete `Sources/TTZipCore/Strategies/CompressionStrategyProtocol.swift`
  - [x] Delete `Sources/TTZipCore/Strategies/PasswordRecoveryStrategyProtocol.swift`
  - [x] Remove directory `Sources/TTZipCore/Strategies`

- [x] **Task 2: Physical Deletion of Redundant Abstract Factories & Traversal Classes**
  - [x] Delete `Sources/TTZipCore/ArchiveEngineFamilyFactory.swift`
  - [x] Delete `Sources/TTZipCore/ArchiveEngineStrategy.swift`
  - [x] Delete `Sources/TTZipCore/ArchiveComponentTraversals.swift`
  - [x] Delete `Sources/TTZipCore/Commands/CompressCommandBuilder.swift`

- [x] **Task 3: Refactor Thin Native Facades**
  - [x] Simplify `ArchiveEngineFactory.swift` to directly instantiate writers/extractors/readers
  - [x] Streamline `ArchiveOperationPipeline.swift` and `ArchivePipelineBuilder.swift`
  - [x] Consolidate `ArchiveComponentTreeBuilder` and `flattenLeaves` in `ArchiveComponentProtocol.swift`
  - [x] Optimize `ArchiveTreeNode.swift` bottom-up size calculation
  - [x] Implement lightweight `CharsetDetector.swift`
  - [x] Connect `ArchiveEngineBridge.makeImplementor` to `RustUnifiedArchiveEngineBridgeImplementor`

- [x] **Task 4: Zero-Regression Test Suite & CI Gate**
  - [x] Verify `swift test` (138/138 passing)
  - [x] Verify `./scripts/lint_loc_gate.sh` (641 files $\le 800\text{ LOC}$)
  - [x] Verify `swift run ttzip-bench gate` (100% passing)
  - [x] Verify `./scripts/run_rust_tests.sh --unit --props --fuzz` (100% passing)
  - [x] Verify `./scripts/run_local_ci_gate.sh` (4-stage gate passing 100%)
