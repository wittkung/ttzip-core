# Tasks: macOS 原生 Quick Look 与 Finder 拖拽集成 (Feature 168)

**Feature ID**: `168-native-macos-quicklook-and-finder-integration`  
**Created**: 2026-08-21  
**Status**: Ready for Implementation  

---

## Phase 1: Setup & Cache Sandbox Infrastructure

- [x] T001 [P] [US3] Implement `Sources/TTZipApp/Services/EphemeralPreviewCacheManager.swift` with POSIX `0o700` sandbox, atomic file staging, and lifecycle cleanup

---

## Phase 2: User Story 1 (P1) - Quick Look Preview Coordinator & Space Bar Monitor

- [x] T002 [P] [US1] Implement `Sources/TTZipApp/Services/QuickLookPreviewCoordinator.swift` with Space bar monitor and async `ArchiveSelectiveExtractor` staging
- [x] T003 [P] [US1] Wire `.quickLookPreview` binding and Space bar handler in `Sources/TTZipApp/Views/Explorer/HomeExplorerContainerView.swift` and `Sources/TTZipApp/Views/ArchiveExplorerView.swift`

---

## Phase 3: User Story 2 (P2) - Finder Drag-and-Drop File Promise Provider

- [x] T004 [P] [US2] Implement `Sources/TTZipApp/Services/ArchiveFilePromiseProvider.swift` wrapping `NSFilePromiseProvider` and `NSFilePromiseProviderDelegate` for lazy extraction
- [x] T005 [P] [US2] Wire `.onDrag` in item row views (`MillerColumnItemRowView.swift` and `ArchiveRowView`) with `ArchiveFilePromiseProvider`

---

## Phase 4: Verification & Gating

- [x] T006 [US1] Implement `Tests/TTZipAppTests/QuickLookAndFinderIntegrationTests.swift`
- [x] T007 [US1] Run `swift test --filter QuickLookAndFinderIntegrationTests`
- [x] T008 [US1] Run `./scripts/run_optimization_gate.sh --bail --json build/gate_report.json`
- [x] T009 [US1] Run `./scripts/benchmark_ab.sh HEAD WIP --runs 5` and verify `PASSED_NO_REGRESSION`
