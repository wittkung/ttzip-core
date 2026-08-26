# Feature Specification: 020 Desktop Architecture Evolution & Full-Chain Quality Governance

- **Feature Directory**: `specs/020-desktop-architecture-and-quality-governance-evolution`
- **Classification**: `[Full SDD]`
- **Status**: `Specified`
- **Created**: 2026-08-26
- **Author**: Antigravity AI & TTZip Architectural Governance Team

---

## 1. Executive Summary & Problem Statement

A thorough, comprehensive architectural and quality governance audit across the native desktop application (`apple/`), shared core foundation (`core/`), UniFFI bridge layer, and CI/CD pipelines has identified foundational bottlenecks, paradigm limitations, and governance broken windows:

1. **Disconnected Task Cancellation & Cooperative Token Breakage**: While UI layers provide buttons for Pause, Resume, and Cancel (`AppViewState+Tasks.swift`, `OperationsQueueView.swift`), they merely mutate transient string status properties (`taskStateName = "Cancelled"`). The underlying operations dispatch `createArchiveStream` and `extractArchiveStream` with `token: nil`, leaving the Rust Rayon multi-threaded engine running without cancellation signals, resulting in sustained CPU and disk I/O churn even after the user aborts an action.
2. **Global Single-State Overwrite & Workspace Collision**: The desktop application binds a singleton `AppViewState` directly to a single `WindowGroup`. Opening multiple archives from Finder concurrently or opening a second archive while performing batch operations overwrites the active directory and archive session (`openArchiveAsFolder`), leading to state stomping, lost form inputs, and lack of multi-document/multi-window independence.
3. **Ghost Operations Queue & Telemetry Disconnection**: `OperationsQueueView` instantiates an isolated `OperationsQueueViewModel()` with an empty `tasks` array on every presentation. It is completely disconnected from real-world operations in `TTZipEngineFacade`, leaving users without a unified, accurate overview of background tasks, throughput rates, and queue prioritization.
4. **Massive Archive VFS Virtualization & Ancestor Navigation Blindspot**: In archives with tens of thousands of entries, `ArchiveTreeStore` constructs the complete tree in memory, and `NativeArchiveOutlineView` uses linear visible-row scans (`0..<outlineView.numberOfRows`) that fail to reveal or select entries hidden inside collapsed ancestor directories.
5. **High-Resolution Media Preview Memory Explosion Risk**: `MediaPreviewFactory` decodes raw images directly via unconstrained `NSImage(contentsOf:)`. For gigapixel images or multi-hundred-megabyte RAW/TIFF graphics, this can consume gigabytes of uncompressed bitmap memory without downsampling or memory pressure eviction triggers.
6. **Silent Failure & Missing Unified Error Boundaries**: Critical errors in `CompressFormSession` (such as disk full, read permission errors, or archive write failures) are silently caught and discarded (`catch { session.isProgressModalPresented = false }`), giving the user zero actionable diagnostics or recovery guidance.
7. **CI/CD Governance & Repository Hygiene Debt**: `apple/scripts/lint_loc_gate.sh` invokes a non-existent `lint_loc_gate.py` script; orphaned legacy test targets (`core/Tests/TTZipAppTests`) remain duplicated in the tree; and automated testing lacks end-to-end integration coverage for cooperative cancellation latency, memory sanitization, and heavy VFS stress.

This specification defines the architectural evolution and governance hardening plan: introducing the **Unified Task Execution & Cooperative Cancellation Coordinator**, establishing the **Multi-Session Document Workspace Paradigm**, deploying the **Global Asynchronous Operations & Telemetry Hub**, implementing **Memory-Budgeted Media Previews with Downsampling Protection**, providing a **Universal Error Diagnostic & Recovery Presentation Framework**, and enforcing **Zero-Debt Quality Gates** across the entire project.

---

## Clarifications

### Session 2026-08-26
- Q: 当用户从 Finder 双击打开多个归档文件，或者在当前已有打开归档的窗口中触发新的归档打开操作时，应用应采用何种窗口与会话隔离范式？ → A: 原生多窗口与系统级标签页合并架构（`NSWindow.tabbingMode = .preferred`），每个窗口独立绑定 `ArchiveSessionContext` 与独立的 VFS 缓存生命周期，共享全局 `ArchiveOperationsQueueCenter`。

---

## 2. User Stories & Acceptance Criteria

### User Story 1: True Cooperative Task Cancellation & Control (`US1`)
- **As a** user compressing or extracting multi-gigabyte files,
- **I want** clicking "Cancel" in the UI to instantly abort the operation and halt all disk and CPU activity within < 100ms,
- **So that** my machine's resources are immediately freed without creating corrupted partial outputs or wasting battery power.
- **Acceptance Criteria**:
  - Every asynchronous archive operation binds a bidirectional `TaskExecutionHandle` linked to a native Rust `CancellationToken`.
  - Triggering cancellation propagates atomically to native worker threads, immediately terminating chunk processing and cleaning up incomplete temporary target files.
  - UI progress overlays dismiss cleanly with a "Cancelled" confirmation state.

### User Story 2: Global Background Operations Queue & Telemetry Hub (`US2`)
- **As a** power user executing multiple concurrent batch compressions and extractions,
- **I want** a persistent, centralized Operations Queue panel displaying real-time progress, aggregate throughput (MB/s), elapsed time, and per-task cancel buttons,
- **So that** I can monitor and manage all running and queued background jobs from anywhere in the app.
- **Acceptance Criteria**:
  - A shared `ArchiveOperationsQueueCenter` acts as the single source of truth for all ongoing and queued engine operations.
  - `OperationsQueueView` dynamically observes the active queue center without recreating empty state models.
  - Dock icon progress bar and menu bar indicators reflect true aggregate throughput and completion percentages.

### User Story 3: Multi-Session Document Architecture & State Isolation (`US3`)
- **As a** macOS user opening multiple archive files simultaneously from Finder,
- **I want** each archive to open in an isolated workspace session or dedicated window without overwriting existing tabs or interrupting in-flight tasks,
- **So that** I can inspect, compare, and extract multiple archives side-by-side without interference.
- **Acceptance Criteria**:
  - `ArchiveSessionState` encapsulates independent archive path, VFS cache pool, active password, selected entries, and search filters.
  - Opening a new archive creates or focuses an independent session instead of clobbering active form inputs or compression workspaces.

### User Story 4: Deep VFS Tree Navigation & Auto-Expanding Selection (`US4`)
- **As a** user navigating complex, deeply nested archives (e.g. tarballs with 10+ directory levels),
- **I want** selecting or searching an entry to automatically expand all ancestor directories and scroll the item into view,
- **So that** I can immediately locate and preview files regardless of tree collapse state.
- **Acceptance Criteria**:
  - `NativeArchiveOutlineView` resolves full path hierarchies and recursively expands parent container nodes before triggering row selection.
  - Tree node generation maintains sub-50ms response times for archives containing up to 100,000 items.

### User Story 5: Memory-Budgeted Media Previews & Downsampling Shield (`US5`)
- **As a** user browsing archives containing massive high-resolution images or graphic assets,
- **I want** the media preview panel to render quick, smooth previews without memory spikes or app sluggishness,
- **So that** the app remains responsive even when inspecting gigapixel or raw image files.
- **Acceptance Criteria**:
  - Image previews utilize downsampled decoding (`CGImageSourceCreateThumbnailAtIndex`) matching the target viewport display scale.
  - A global memory budget observer monitors system memory pressure and clears non-essential LRU preview caches upon warning notifications.

### User Story 6: Universal Error Diagnosis & Recovery Presentation (`US6`)
- **As a** user encountering operation errors (e.g. read-only volume, password failure, corrupted header, disk full),
- **I want** a clear, human-readable error modal with diagnostic details and actionable recovery suggestions (e.g. choose another destination, enter password, repair archive),
- **So that** I understand why an operation failed and how to resolve it immediately.
- **Acceptance Criteria**:
  - Zero silent failure catches across all ViewModels and session handlers.
  - `AppErrorReporter` presents standardized, localized error dialogs with recovery actions.

### User Story 7: Zero-Debt Quality Governance & CI Hardening (`US7`)
- **As a** release engineer,
- **I want** all repository scripts, linters, and CI test suites to run deterministically with zero broken paths, zero orphaned files, and 100% test coverage for critical state transitions,
- **So that** regressions are caught before builds are packaged or released.
- **Acceptance Criteria**:
  - `apple/scripts/lint_loc_gate.sh` executes with complete script dependencies.
  - Orphaned duplicate test directories (`core/Tests/TTZipAppTests`) are eliminated, and `scripts/lint_repo_hygiene.sh` enforces repo purity.
  - Comprehensive automated tests validate cancellation responsiveness, multi-session state isolation, and downsampled preview memory limits.

---

## 3. Functional Requirements

- **FR-001**: System MUST supply a valid `CancellationToken` through UniFFI for all `createArchiveStream`, `extractArchiveStream`, and `extractSelectedEntries` engine calls.
- **FR-002**: System MUST halt disk writes and thread pool execution within $\le 100\text{ms}$ upon receiving a cancellation signal.
- **FR-003**: System MUST provide an application-wide `@MainActor` singleton `ArchiveOperationsQueueCenter` tracking lifecycle states (`queued`, `running`, `paused`, `completed`, `failed`, `cancelled`), byte progress, and throughput.
- **FR-004**: System MUST synchronize Dock progress indicators and status messages with `ArchiveOperationsQueueCenter`.
- **FR-005**: System MUST decouple archive workspace state into discrete, independent `ArchiveSessionContext` instances per window/tab to prevent multi-file state collisions.
- **FR-006**: System MUST support native multi-window and system tab merging (`NSWindow.tabbingMode = .preferred`) allowing independent side-by-side or tabbed inspection.
- **FR-007**: System MUST automatically expand all ancestor folder nodes in `NativeArchiveOutlineView` when selecting an item by path.
- **FR-008**: System MUST decode image previews using bounded thumbnail downsampling (`kCGImageSourceThumbnailMaxPixelSize` based on 2x screen resolution) rather than full-resolution raw decoding.
- **FR-009**: System MUST register an `OSMemoryNotification` / `didReceiveMemoryWarningNotification` listener to flush ephemeral preview caches under memory pressure.
- **FR-010**: System MUST present structured error dialogs with actionable recovery buttons for all compression, extraction, and VFS operation failures.
- **FR-011**: System MUST enforce single-file LOC thresholds ($\le 800$ LOC) across both `apple/` and `core/` via unified, functional CI scripts.
- **FR-012**: System MUST validate repository cleanliness and eliminate all orphaned files or duplicate legacy test directories via `scripts/lint_repo_hygiene.sh`.
- **FR-013**: System MUST enforce `-warnings-as-errors` across all Swift packages in release mode.

---

## 4. Key Entities & Architecture Model

```
 ┌──────────────────────────────────────────────────────────────────────────────────────────┐
 │                                   User Interaction Layer                                 │
 │  ┌─────────────────────────┐  ┌─────────────────────────┐  ┌──────────────────────────┐  │
 │  │ Compression Workspace   │  │ Archive Explorer View   │  │ Operations Queue Sheet   │  │
 │  │ (CompressFormSession)   │  │ (ArchiveSessionContext) │  │ (OperationsQueueVM)      │  │
 │  └────────────┬────────────┘  └────────────┬────────────┘  └────────────┬─────────────┘  │
 └───────────────┼────────────────────────────┼────────────────────────────┼────────────────┘
                 │                            │                            │
                 ▼                            ▼                            ▼
 ┌──────────────────────────────────────────────────────────────────────────────────────────┐
 │                       ArchiveOperationsQueueCenter (@MainActor)                          │
 │  • Registers active & queued archive tasks with unique UUIDs                             │
 │  • Dispatches unified TaskExecutionHandle (linking Swift Task & UniFFI CancellationToken)│
 │  • Broadcasts real-time throughput & progress to Dock, MenuBar, and UI Sheets            │
 │  • Routes unhandled operation errors to AppErrorReporter                                 │
 └────────────────────────────────────────────┬─────────────────────────────────────────────┘
                                              │
                                              ▼
 ┌──────────────────────────────────────────────────────────────────────────────────────────┐
 │                                TTZipEngineFacade (Core)                                  │
 │  ┌────────────────────────────────────────────────────────────────────────────────────┐  │
 │  │ TaskExecutionHandle.uniffiToken                                                    │  │
 │  └─────────────────────────────────────────┬──────────────────────────────────────────┘  │
 └────────────────────────────────────────────┼─────────────────────────────────────────────┘
                                              │ UniFFI Bridge
                                              ▼
 ┌──────────────────────────────────────────────────────────────────────────────────────────┐
 │                           Rust Native Microkernel (Rayon Pool)                           │
 │  • Periodically checks CancellationToken.is_cancelled() during chunk I/O                 │
 │  • Immediately aborts compression/extraction loops on cancellation signal                │
 │  • Atomically rolls back temporary incomplete output files                               │
 └──────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Assumptions & Non-Goals

### Assumptions
- macOS 14.0+ (Sonoma) is the primary target platform utilizing Swift 6 strict concurrency and the `Observation` framework.
- Apple Silicon (M-series) unified memory architecture provides high memory bandwidth, but large uncompressed raw bitmaps must still be guarded by downsampling to avoid UI memory pressure.
- Mozilla UniFFI proc-macro bindings remain the sole interop bridge between Swift and Rust.

### Non-Goals
- Re-implementing existing core compression algorithms in Swift (all compute remains strictly in Rust).
- Modifying third-party binary dependencies in `Vendor/` unless addressing critical security vulnerabilities.
- Supporting legacy macOS versions prior to macOS 14.0.

---

## 6. Success Criteria

1. **Cancellation Response Time**: Halts CPU and disk I/O within $< 100\text{ms}$ of user cancellation across all compression and extraction tasks.
2. **Operations Queue Integrity**: 100% synchronization parity between active background operations and `OperationsQueueView` / Dock progress indicators.
3. **Memory Stability**: Previewing 100MB+ high-resolution images maintains a peak memory overhead $< 35\text{MB}$ via viewport downsampling.
4. **Deep Tree Navigation**: 100% success rate in auto-expanding and selecting deeply nested archive entries regardless of initial collapse state.
5. **Zero Silent Failures**: 100% of caught engine errors surface appropriate user-facing notifications or recovery modals.
6. **Repository & CI Purity**: 0 broken scripts, 0 orphaned test directories, 0 compiler warnings (`-warnings-as-errors`), and 100% passing test suites across `apple/` and `core/`.
