# Implementation Plan: 014 Frontend Architecture Audit & Paradigm Evolution

- **Feature Directory**: `specs/014-frontend-architecture-evolution`
- **Classification**: `[Full SDD]`
- **Status**: `Planning`
- **Created**: 2026-08-25
- **Author**: Antigravity AI & TTZip Architectural Governance Team

---

## 1. Technical Context & Architectural Architecture

### 1.1 Scope of Changes
This feature addresses all 10 verified architectural bottlenecks across TTZip's native macOS client (`apple/Sources/TTZipApp`), supporting services, and core bridges:

1. **State Management Migration (`ViewModels/`)**:
   - Migrate `AppViewState`, `NavigationState`, `ArchiveExplorerState`, `TaskExecutionState`, and `OverlayState` to Swift 5.9+ `@Observable`.
   - Eliminate Combine `objectWillChange` forwarding sinks in `AppViewState.init`.
   - Introduce `CompressFormSession` model, replacing 38 `@State` variables and 24 Binding arguments in `CompressModalView` with a single cohesive session.

2. **I/O Concurrency & Session Cache (`Services/`, `Views/Explorer/`)**:
   - Implement `DiskDirectoryScannerActor` leveraging `URLResourceValues` and POSIX `getattrlistbulk` prefetching.
   - Enhance `DiskItemInfo` with `init(url:resourceValues:)` constructor (0 extra system calls).
   - Remove dead `@State var items` and duplicate scanning in `DiskDirectoryBrowserView`.
   - Implement `ArchiveHierarchySessionCache` to eliminate repeated archive FFI inspections and full composite tree rebuilds during subfolder navigation.

3. **Rendering & Syntax Highlighting Engine (`Views/Preview/`, `Services/`)**:
   - Introduce `PrecompiledSyntaxEngine` and `BackgroundSyntaxTokenizer` Actor to offload regex tokenization and view-port bounding, eliminating main-thread typing lag.
   - Migrate `ImageIOThumbnailCache` to `ImageIOThumbnailService` Actor with in-flight task deduplication and detached cooperative decoding.

4. **AppKit Interoperability & Lifecycle Governance (`Views/Explorer/`, `Components/`)**:
   - Replace synthetic `NSEvent` keydown injections (`keyCode: 37`) with native SwiftUI `@FocusState` / `NavigationState` focus triggers.
   - Enforce explicit monitor retention and de-registration in `QuickLookPreviewCoordinator` and explorer views.
   - Update `DocxTextEditorNSView` with dynamic `NSColor.labelColor` dark mode support and complete `updateNSView` diff handling.

5. **Design Tokens & Complete i18n Guardrails (`Theme/`, `Localization/`)**:
   - Expand `TTZipTheme.Layout` to standardize header heights, column constraints, and offsets.
   - Register 35 missing localization keys in Rust UniFFI i18n core and `LocaleKey.swift`, eliminating 45 hardcoded English strings across all UI components.

---

### 1.2 Constitution Check
- **Zero-Subprocess Policy**: Fully compliant. All directory scanning, archive inspection, and rendering operate in-process via direct Foundation/CoreGraphics APIs and UniFFI FFI bindings.
- **Strict Single-File LOC Threshold ($\le 800$ LOC)**: All refactored models, views, and actors strictly maintain single-file LOC $\le 350$ lines (with hard limit $\le 800$).
- **Zero In-Tree Path Invariant**: Fully compliant. All cache management, bookmarks, and configurations resolve dynamically without assuming local Git workspace root.
- **Swift 6 Strict Concurrency**: All actors, session structs, and callback closures enforce Sendable protocol safety under `-strict-concurrency=complete`.

---

## 2. Execution Phases & Deliverables

### Phase 0: Research & Benchmarking (`research.md`)
- [x] Research Observation framework runtime getter tracking and Combine decoupling.
- [x] Benchmark POSIX `getattrlistbulk` vs individual `attributesOfItem` calls.
- [x] Design $O(1)$ subpath indexing and cache invalidation policies for archive sessions.
- [x] Design Viewport-bounded syntax tokenization using TextKit 2.

### Phase 1: Design Artifacts (`data-model.md`, `contracts/`, `quickstart.md`)
- [x] Generate `data-model.md` covering `CompressFormSession`, `ArchiveHierarchySession`, `TokenSpan`, and `DiskItemInfo`.
- [x] Generate `contracts/frontend-session-contracts.json` (verified with `lint-contracts.sh`).
- [x] Generate `contracts/frontend-architecture.md` defining state, I/O actor, and tokenizer boundaries.
- [x] Generate `quickstart.md` defining automated verification scenarios.

### Phase 2: Implementation Sequencing
1. **Core Data & Actor Foundation**: Implement `DiskDirectoryScannerActor`, `ArchiveHierarchySessionCache`, `PrecompiledSyntaxEngine`, and `ImageIOThumbnailService`.
2. **State Layer Refactoring**: Migrate `AppSubStates` and `AppViewState` to `@Observable`; extract `CompressFormSession`.
3. **View Layer Refactoring**: Refactor `CompressModalView`, `FinderMillerColumnsView`, `DiskDirectoryBrowserView`, `CodeHighlightingEditorNSView`, `DocxTextEditorNSView`, and `HomeExplorerContainerView`.
4. **i18n & Design Token Convergence**: Populate missing UniFFI keys and replace hardcoded literals and magic numbers.

### Phase 3: Verification & Performance Profiling
- Execute `FrontendPerformanceMetricsTests`, `DiskDirectoryScannerTests`, and `ArchiveHierarchySessionCacheTests`.
- Verify full test suite pass rate and lint gates (`lint_loc_gate.py`, `lint-contracts.sh`).
