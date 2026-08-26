# Implementation Plan: macOS 原生 Quick Look 与 Finder 拖拽集成 (Feature 168)

**Feature ID**: `168-native-macos-quicklook-and-finder-integration`  
**Created**: 2026-08-21  
**Status**: Ready for Tasks  

---

## 1. Technical Context & Constitution Check

### 1.1 Technical Context
- **Toolchain**: Swift 6, macOS AppKit / SwiftUI (`.quickLookPreview`, `QLPreviewController`, `NSFilePromiseProvider`), C11 bridge (`ArchiveSelectiveExtractor`).
- **Core Design Decisions**:
  - `EphemeralPreviewCacheManager` Swift `actor` with POSIX `0o700` directory permissions and atomic file staging via POSIX `O_CREAT | O_EXCL` + `rename(2)`.
  - `QuickLookPreviewCoordinator` with local `NSEvent` space-bar monitor and automatic toggle.
  - `ArchiveFilePromiseProvider` (`NSFilePromiseProviderDelegate`) for lazy decompression only upon actual Finder drop.

### 1.2 Constitution Check
- [x] **Zero Cloud Quota / 100% Local**: All previews and extractions happen strictly on local system.
- [x] **Zero Bare Objects & Schema Strictness**: JSON schema contract in `contracts/quicklook-finder-contract-schema.json`.
- [x] **Defensive Memory & Security**: Restrictive `0o700` sandbox prevents local multi-user data leaks.

---

## 2. Phase 0 & Phase 1 Artifacts Index

- [x] **Phase 0 Research**: [`research.md`](research.md)
  - `- R001 [SUBAGENT:research] 《macOS QLPreviewController 与 SwiftUI .quickLookPreview 绑定机制》`
  - `- R002 [SUBAGENT:research] 《NSFilePromiseProvider 与 Finder 延迟拖拽提取流水线》`
  - `- R003 [SUBAGENT:research] 《EphemeralPreviewCacheManager 临时沙盒生命周期管理》`
- [x] **Phase 1 Data Model**: [`data-model.md`](data-model.md)
- [x] **Phase 1 Contract**: [`contracts/quicklook-finder-contract-schema.json`](contracts/quicklook-finder-contract-schema.json)
- [x] **Phase 1 Quickstart**: [`quickstart.md`](quickstart.md)

---

## 3. Component Breakdown & Planned Changes

### Component 1: Ephemeral Preview Cache Manager (`EphemeralPreviewCacheManager.swift`)
- [NEW] `Sources/TTZipApp/Services/EphemeralPreviewCacheManager.swift`: Actor managing sandboxed `0o700` directory, atomic file staging, TTL eviction, and `willTerminateNotification` cleanup.

### Component 2: Quick Look Preview Coordinator (`QuickLookPreviewCoordinator.swift`)
- [NEW] `Sources/TTZipApp/Services/QuickLookPreviewCoordinator.swift`: `@MainActor` coordinator intercepting Space-bar key events, executing single-entry staging via `ArchiveSelectiveExtractor`, and managing active preview URL binding.

### Component 3: Finder Drag-and-Drop File Promise Provider (`ArchiveFilePromiseProvider.swift`)
- [NEW] `Sources/TTZipApp/Services/ArchiveFilePromiseProvider.swift`: `NSFilePromiseProvider` wrapper implementing `NSFilePromiseProviderDelegate` for lazy in-archive entry extraction upon Finder drop.

### Component 4: SwiftUI View Integration
- [MODIFY] `Sources/TTZipApp/Views/Explorer/HomeExplorerContainerView.swift`: Wire `.quickLookPreview` and Space-bar shortcut.
- [MODIFY] `Sources/TTZipApp/Views/ArchiveExplorerView.swift`: Wire `.quickLookPreview` and Space-bar shortcut.

### Component 5: Unit Testing & A/B Benchmarking
- [NEW] `Tests/TTZipAppTests/QuickLookAndFinderIntegrationTests.swift`: Comprehensive unit tests for preview staging, Space bar toggle, and file promise generation.

---

## 4. Verification Plan

1. **Swift Unit Tests**:
   - `swift test --filter QuickLookAndFinderIntegrationTests`
2. **Local CI 6-Stage Gate**:
   - `./scripts/local-ci.sh`
3. **Statistical Worktree A/B Gate**:
   - `./scripts/benchmark_ab.sh HEAD WIP --runs 5`
