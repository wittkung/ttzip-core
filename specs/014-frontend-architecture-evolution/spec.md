# Feature Specification: 014 Frontend Architecture Audit & Paradigm Evolution

- **Feature Directory**: `specs/014-frontend-architecture-evolution`
- **Classification**: `[Full SDD]`
- **Status**: `Specified`
- **Created**: 2026-08-25
- **Author**: Antigravity AI & TTZip Architectural Governance Team

---

## 1. Executive Summary & Problem Statement

Following an exhaustive audit of TTZip's native macOS front-end codebase (`apple/Sources/TTZipApp`), supporting AppKit/SwiftUI bridges, and web assets, multiple fundamental architectural bottlenecks and outdated paradigms have been identified:

1. **State Management Storm & Re-render Amplification (Combine `ObservableObject` Bottleneck)**:
   - While `AppViewState` is partitioned into domain sub-states (`NavigationState`, `ArchiveExplorerState`, `TaskExecutionState`, `OverlayState`), it forwards all sub-state mutations via `sink { self?.objectWillChange.send() }`.
   - High-frequency mutations (e.g., 60Hz progress updates in `TaskExecutionState`, keystroke inputs in `ArchiveExplorerState.searchQuery`, or hover changes) invalidate the entire SwiftUI view tree, causing main-thread frame drops and layout re-evaluations.
   - Large views (e.g., `CompressModalView`) suffer from "State Swamp" anti-patterns, holding 20+ loose `@State` variables passed across 24 Binding parameters without a cohesive Form ViewModel.

2. **Synchronous I/O & Redundant Multi-Level Archive Tree Construction**:
   - `MillerColumnDirectoryScanner` scans directories using `contentsOfDirectory(atPath:)` and instantiates `DiskItemInfo` for each file, synchronously executing individual `FileManager.attributesOfItem` calls (thousands of synchronous `stat` system calls per folder).
   - In-archive browsing repeatedly triggers `TTZipEngineFacade.shared.inspectArchive` and full entry tree reconstruction (`ArchiveComponentTreeBuilder.buildTree`) on every subdirectory navigation, causing $O(N)$ re-parsing overhead for archives containing 100,000+ files.
   - Duplicate directory scanning logic exists between `DiskDirectoryBrowserView` and `FinderMillerColumnsView`.

3. **Main-Thread Regex Catastrophe in Syntax Highlighting & Fake Async in Thumbnail Decoding**:
   - `CodeHighlightingEditorNSView` re-compiles and executes 5 global regular expressions synchronously on the main thread inside `textDidChange` on every keystroke, choking on large files.
   - `ImageIOThumbnailCache.getThumbnailAsync` marks itself `async` but executes synchronous CoreGraphics decoding directly on the caller's thread without Actor isolation.

4. **AppKit Interoperability Anti-Patterns & Lifecycle Fragility**:
   - Focus handling in `HomeExplorerContainerView` synthesizes synthetic `NSEvent` key events dispatched to `NSApp.sendEvent` rather than using modern `@FocusState` / Action routing.
   - Global `NSEvent.addLocalMonitorForEvents` monitors lack strict lifecycle encapsulation, posing event intercept leak risks.
   - `DocxTextEditorNSView` hardcodes `.white` backgrounds and lacks dark mode adaptation or dynamic updates in `updateNSView`.

5. **Theme Token Fragmentation & Incomplete i18n Guardrails**:
   - Hardcoded magic layout numbers (`padding(38)`, `frame(width: 280)`) bypass `TTZipTheme`.
   - Remnant English literals (e.g., alert dialogs, file explorer action headers, status messages) bypass the Rust UniFFI unified localization engine.

This specification defines the complete paradigm evolution across state management, async I/O actors, non-blocking viewport-based rendering, AppKit bridge safety, and design token unification.

---

## 2. User Stories & Acceptance Criteria

### User Story 1: Modern Swift Observation Framework Paradigm (`@Observable`)
- **As a** macOS user navigating massive file hierarchies and running high-speed compression tasks,
- **I want** the GUI to maintain a consistent 120 FPS ProMotion refresh rate without stutter,
- **So that** 60Hz progress updates and rapid search typing only re-render the exact leaf components that consume those properties.
- **Acceptance Criteria**:
  - `AppViewState` and all domain sub-states migrate from `ObservableObject` / `@Published` to Swift 5.9+ / macOS 14+ `@Observable`.
  - Elimination of all `objectWillChange.send()` forwarding sinks.
  - Property updates in `TaskExecutionState.progressValue` trigger zero body evaluations in navigation or explorer sibling views.
  - `CompressModalView` extracts all configuration states into an `@Observable class CompressFormSession` model.

### User Story 2: Non-Blocking High-Throughput I/O Scanning & Archive Hierarchy Cache
- **As a** user browsing folders with tens of thousands of items or deeply nested archives,
- **I want** directory contents and archive folders to load instantly without freezing the UI,
- **So that** column expansion and folder traversal feel instantaneous.
- **Acceptance Criteria**:
  - `DiskDirectoryScannerActor` implements batch prefetching via `URL.resourceValues(forKeys:)` / POSIX `getattrlistbulk`, reducing file attribute system calls by $> 90\%$.
  - An `ArchiveHierarchySessionCache` retains parsed immutable tree nodes per archive path, achieving $O(1)$ child lookup when traversing subdirectories without re-inspecting the archive.
  - Dead state and duplicate directory scanning in `DiskDirectoryBrowserView` are completely removed.

### User Story 3: Viewport-Based Async Tokenization & True Non-Blocking Thumbnails
- **As a** developer previewing large source files (10,000+ LOC) and high-resolution images,
- **I want** code editing and image loading to be smooth and responsive,
- **So that** typing never lags and thumbnail generation runs completely off the main thread.
- **Acceptance Criteria**:
  - Syntax regular expressions are precompiled static constants with debounced or TextKit 2 background layout tokenization.
  - `ThumbnailGeneratorActor` executes CoreGraphics downsampling on a background cooperative thread pool with true non-blocking async contracts.

### User Story 4: Clean AppKit/SwiftUI Focus & Theme Token Unification
- **As a** macOS power user relying on keyboard navigation and system Dark/Light mode,
- **I want** native focus traversal (`@FocusState`), dynamic color adaptation across all custom AppKit views, and zero keyboard hacks,
- **So that** the entire app looks and behaves like an Apple Design Award-tier native application.
- **Acceptance Criteria**:
  - Removal of all synthetic `NSEvent.keyEvent` injections in favor of SwiftUI `@FocusState` and Command dispatching.
  - `DocxTextEditorNSView` and all `NSViewRepresentable` components adapt automatically to dark/light appearances using semantic `NSColor` tokens.
  - All spacing, typography, and corner radii adhere strictly to `TTZipTheme`.

### User Story 5: 100% Localization Coverage & Swift 6 Strict Concurrency
- **As an** international user,
- **I want** every dialog, alert, button, and status text translated through the UniFFI Rust i18n engine,
- **So that** no untranslated English strings appear anywhere in the UI.
- **Acceptance Criteria**:
  - All hardcoded UI strings are replaced with `l10n.t(...)` keys.
  - Codebase compiles cleanly under Swift 6 strict concurrency checks (`-strict-concurrency=complete`) with zero data race warnings.

---

## 3. Functional Requirements

- **FR-001**: System MUST migrate `AppViewState`, `NavigationState`, `ArchiveExplorerState`, `TaskExecutionState`, and `OverlayState` to the `@Observable` macro.
- **FR-002**: System MUST encapsulate all compression modal state fields (format, level, algorithm, password, volumes, options) into a dedicated `CompressFormSession` model.
- **FR-003**: System MUST provide an isolated `actor DiskDirectoryScannerActor` implementing pre-fetched batch directory queries using `keys: [.isDirectoryKey, .fileSizeKey, .contentModificationDateKey, .isPackageKey]`.
- **FR-004**: System MUST introduce an `ArchiveHierarchySessionCache` storing parsed `ArchiveCompositeDirectory` trees keyed by `(archivePath, modificationDate)` to prevent re-inspection during subpath navigation.
- **FR-005**: System MUST eliminate duplicate scanning between `DiskDirectoryBrowserView` and `FinderMillerColumnsView`.
- **FR-006**: System MUST isolate thumbnail generation in `ImageIOThumbnailCache` via an async background actor dispatching CoreGraphics downsampling to detached cooperative tasks.
- **FR-007**: System MUST precompile syntax highlighting regular expressions and execute tokenization asynchronously or viewport-bounded for source code previews.
- **FR-008**: System MUST replace synthetic `NSEvent` key event generation with SwiftUI native `@FocusState` and standard menu/command bindings.
- **FR-009**: System MUST update all `NSViewRepresentable` implementations (`DocxTextEditorNSView`, `StreamingTextNSView`, `CodeHighlightingEditorNSView`) to fully adapt to `NSAppearance` changes and provide complete `updateNSView` diffing.
- **FR-010**: System MUST eliminate all hardcoded English UI strings in `HomeExplorerContainerView`, `FinderMillerColumnsView`, alerts, and editors, routing them through `AppLocalizationState.shared`.
- **FR-011**: System MUST enforce compile-time Swift 6 strict concurrency safety across all UI view models and services.

---

## 4. Success Criteria

- **SC-001 (Zero Re-render Storm)**: 60Hz progress updates in `TaskExecutionState` cause 0 redundant body invalidations in navigation, sidebar, or explorer components.
- **SC-002 (Directory Browsing Throughput)**: Loading a folder with 5,000 items completes in $< 25\text{ ms}$ on Apple Silicon (a $> 5\times$ speedup over un-prefetched scanning).
- **SC-003 (Archive Subdirectory Traversal)**: Subdirectory navigation in a 100,000-entry archive executes in $< 1\text{ ms}$ ($O(1)$ memory lookup via `ArchiveHierarchySessionCache`).
- **SC-004 (Typing Latency)**: Keystroke latency in `CodeHighlightingEditorNSView` on 5,000-line source files stays $< 8\text{ ms}$ (zero dropped frames).
- **SC-005 (Zero Synthetic Event Hacks)**: 100% of focus transitions and keyboard shortcuts operate via standard SwiftUI focus / command routing.
- **SC-006 (Zero Hardcoded String Remnants)**: 100% of user-facing UI labels, alerts, and tooltips are wired to `AppLocalizationState`.
- **SC-007 (Swift 6 Strict Concurrency)**: Zero warnings or data races under `-strict-concurrency=complete`.
