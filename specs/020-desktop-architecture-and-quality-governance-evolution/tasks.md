# Task Breakdown: 020 Desktop Architecture Evolution & Full-Chain Quality Governance

- **Feature Directory**: `specs/020-desktop-architecture-and-quality-governance-evolution`
- **Classification**: `[Full SDD]`
- **Status**: `Complete`
- **Created**: 2026-08-26
- **Author**: Antigravity AI & TTZip Architectural Governance Team

---

## Dependencies & Execution Graph

```
[Phase 1: Setup & Hygiene] ──► [Phase 2: Foundational Concurrency]
                                              │
         ┌──────────────────┬─────────────────┼──────────────────┬─────────────────┐
         ▼                  ▼                 ▼                  ▼                 ▼
  [Phase 3: US1]     [Phase 4: US2]    [Phase 5: US3]     [Phase 6: US4]    [Phase 7: US5]
(Coop Cancellation)  (Operations Queue) (Multi-Session)   (VFS Tree Nav)    (Preview Memory)
         │                  │                 │                  │                 │
         └──────────────────┴─────────────────┼──────────────────┴─────────────────┘
                                              ▼
                                       [Phase 8: US6] (Error Diagnostics)
                                              │
                                              ▼
                                       [Phase 9: US7] (CI & Test Hardening)
                                              │
                                              ▼
                                       [Phase 10: Convergence & Verification]
```

---

## Tasks List

### Phase 1: Setup & Broken-Window Remediation
- [x] T001 [P] Fix LOC gate script reference in `apple/scripts/lint_loc_gate.sh` to correctly invoke `core/scripts/lint_loc_gate.py` with `--dir` and `--min-files 10`
- [x] T002 [P] Clean up orphaned duplicate test directory `core/Tests/TTZipAppTests/` to restore repository hygiene
- [x] T003 Enhance repository hygiene linter in `scripts/lint_repo_hygiene.sh` with SPM target-to-disk symmetry check

### Phase 2: Foundational Concurrency & Bridge Hardening
- [x] T004 [P] Verify `CancellationToken` in `core/rust/ttzip-engine/src/uniffi_api/archive.rs` for chunk-level checks and rollback guards
- [x] T005 Wire `TaskExecutionHandle.uniffiToken` through `ArchiveWriter.swift` in `core/Sources/TTZipCore/ArchiveWriter.swift`
- [x] T006 Wire `TaskExecutionHandle.uniffiToken` through `ArchiveExtractor.swift` in `core/Sources/TTZipCore/ArchiveExtractor.swift`
- [x] T007 Propagate `TaskExecutionHandle` through `TTZipEngineFacade.swift` in `core/Sources/TTZipCore/Facades/TTZipEngineFacade.swift`

### Phase 3: User Story 1 - True Cooperative Task Cancellation & Rollback Guard (`US1`)
- [x] T008 [P] [US1] Create `ArchiveTaskCoordinator` singleton in `apple/Sources/TTZipApp/Services/ArchiveTaskCoordinator.swift`
- [x] T009 [US1] Refactor `AppViewState+Tasks.swift` in `apple/Sources/TTZipApp/ViewModels/AppViewState+Tasks.swift` to invoke `ArchiveTaskCoordinator`
- [x] T010 [US1] Refactor `CompressFormSession.swift` in `apple/Sources/TTZipApp/ViewModels/CompressFormSession.swift` to bind real cancellation tokens and rollback incomplete outputs
- [x] T011 [US1] Update `CompressModalView.swift` in `apple/Sources/TTZipApp/Views/CompressModalView.swift` to trigger immediate cooperative abort
- [x] T012 [P] [US1] Implement `CooperativeCancellationLatencyTests.swift` in `apple/Tests/TTZipAppTests/CooperativeCancellationLatencyTests.swift`

### Phase 4: User Story 2 - Global Background Operations Queue & Monotonic Telemetry (`US2`)
- [x] T013 [P] [US2] Implement `ArchiveOperationsQueueCenter` with monotonic batch progress calculation in `apple/Sources/TTZipApp/Services/ArchiveOperationsQueueCenter.swift`
- [x] T014 [US2] Refactor `OperationsQueueViewModel.swift` in `apple/Sources/TTZipApp/ViewModels/OperationsQueueViewModel.swift` to bind to `ArchiveOperationsQueueCenter`
- [x] T015 [US2] Update `OperationsQueueView.swift` in `apple/Sources/TTZipApp/Views/OperationsQueueView.swift` to observe global task states and render progress
- [x] T016 [US2] Synchronize Dock icon progress in `apple/Sources/TTZipApp/Services/DockProgressManager.swift` with cached `DockProgressTileView`
- [x] T017 [P] [US2] Implement `OperationsQueueCoordinatorTests.swift` in `apple/Tests/TTZipAppTests/OperationsQueueCoordinatorTests.swift`

### Phase 5: User Story 3 - Multi-Session Document Architecture & Tab Merging (`US3`)
- [x] T018 [P] [US3] Create `ArchiveSessionContext.swift` in `apple/Sources/TTZipApp/ViewModels/ArchiveSessionContext.swift`
- [x] T019 [US3] Configure `NSWindow.tabbingMode = .preferred` in `apple/Sources/TTZipApp/TTZipApp.swift`
- [x] T020 [US3] Update `AppIntentDispatcher.swift` in `apple/Sources/TTZipApp/Services/AppIntentDispatcher.swift` to manage multi-session window routing
- [x] T021 [P] [US3] Implement `MultiSessionIsolationTests.swift` in `apple/Tests/TTZipAppTests/MultiSessionIsolationTests.swift`

### Phase 6: User Story 4 - Deep VFS Tree Navigation & Ancestor Auto-Expansion (`US4`)
- [x] T022 [P] [US4] Implement recursive ancestor node expansion algorithm with path-prefix pruning in `apple/Sources/TTZipApp/Views/Explorer/NativeArchiveOutlineView.swift`
- [x] T023 [US4] Update selection coordination in `apple/Sources/TTZipApp/Views/Explorer/NativeArchiveOutlineView+Delegates.swift`
- [x] T024 [P] [US4] Implement `DeepVfsTreeAutoExpansionTests.swift` in `apple/Tests/TTZipAppTests/DeepVfsTreeAutoExpansionTests.swift`

### Phase 7: User Story 5 - Memory-Budgeted Media Previews with Downsampling (`US5`)
- [x] T025 [P] [US5] Implement `DownsampledImageLoader.swift` in `apple/Sources/TTZipApp/Services/DownsampledImageLoader.swift`
- [x] T026 [US5] Refactor `MediaPreviewFactory.swift` in `apple/Sources/TTZipApp/Services/MediaPreviewFactory.swift` to eliminate unbounded fallback decoders
- [x] T027 [US5] Implement `MemoryPressureObserver.swift` in `apple/Sources/TTZipApp/Services/MemoryPressureObserver.swift`
- [x] T028 [P] [US5] Implement `DownsampledMediaPreviewMemoryTests.swift` in `apple/Tests/TTZipAppTests/DownsampledImageLoaderTests.swift`

### Phase 8: User Story 6 - Universal Error Diagnosis & Recovery Presentation (`US6`)
- [x] T029 [P] [US6] Implement `AppErrorReporter.swift` and structured diagnostic codes in `apple/Sources/TTZipApp/Services/AppErrorReporter.swift`
- [x] T030 [US6] Fix `ArchiveReader.inspect` and `CompressFormSession.swift` to eliminate silent catches and false password prompts
- [x] T031 [US6] Create `ErrorPresentationSheetView.swift` in `apple/Sources/TTZipApp/Views/Components/ErrorPresentationSheetView.swift`
- [x] T032 [P] [US6] Implement `AppErrorReporterTests.swift` in `apple/Tests/TTZipAppTests/AppErrorReporterTests.swift`

### Phase 9: User Story 7 - Full-Chain CI/CD Governance & Stress Testing (`US7`)
- [x] T033 [P] [US7] Implement `StressAndConcurrencySuiteTests.swift` in `apple/Tests/TTZipAppTests/StressAndConcurrencySuiteTests.swift`
- [x] T034 [US7] Enforce strict concurrency and test hardening across `apple/Package.swift` and `core/Package.swift`

### Phase 10: Convergence & Verification
- [x] T035 Run `scripts/lint_repo_hygiene.sh` and verify 0 violations
- [x] T036 Run `apple/scripts/lint_loc_gate.sh` and verify 0 files > 800 LOC
- [x] T037 Run `swift test` across `apple/` and `core/` to verify 100% pass rate
- [x] T038 Validate contract integrity via `specs/020-desktop-architecture-and-quality-governance-evolution/contracts/lint-contracts.sh`
