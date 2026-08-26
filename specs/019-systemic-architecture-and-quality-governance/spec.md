# Feature Specification: 019 Systemic Architecture & Quality Governance Hardening

- **Feature Directory**: `specs/019-systemic-architecture-and-quality-governance`
- **Classification**: `[Full SDD]`
- **Status**: `Specified`
- **Created**: 2026-08-25
- **Author**: Antigravity AI & TTZip Architectural Governance Team

---

## 1. Executive Summary & Problem Statement

Through deep systemic retrospection of historical UI synchronization failures (such as the context-menu "New Archive" failing to bind selected paths into keep-alive tab state, visual layout padding gaps, dead frontend asset accumulation, build scripts defaulting to Debug mode, and 8+ persistent deprecation warnings in build logs), we identified fundamental architectural and process vulnerabilities:

1. **State Lifecycle & Keep-Alive Desynchronization**: While `KeepAliveTabContainer` preserves view hierarchy performance during tab switching, child view states (`CompressModalView`, `@State session`) followed a naive single-initialization assumption, causing external dynamic invocations to be silently dropped.
2. **Fragmented Multi-Entrypoint Routing**: Five independent entrypoints (UI Context Menus, macOS FinderSync Extension, `ttzip://` URL Schemes, Drag-and-Drop, and AppKit Menu Commands) implemented bespoke routing and payload parsing logic with `default: break` fallthroughs instead of a unified, type-safe Intent Dispatcher.
3. **Testing Paradigm Blindspot**: 100% of existing tests focused on stateless algorithms and microkernels (195+ unit tests passing), with zero automated coverage for state transitions, cross-tab payload re-entrancy, or multi-entrypoint routing consistency.
4. **Engineering Standards & Release Debt**: Build scripts defaulted to non-optimized Debug modes for end-user execution, and broken-window tolerance allowed deprecation warnings to pollute compiler logs.

This feature establishes an enterprise-grade architectural defense line across TTZip: introducing the unified `AppIntentDispatcher`, formalizing reactive lifecycle contracts for keep-alive tabs, deploying an end-to-end State Transition Test Suite, enforcing zero-warning compilation gates, and locking release-by-default engineering standards.

---

## 2. User Stories & Acceptance Criteria

### User Story 1: Unified Intent Routing (`US1`)
- **As a** macOS user invoking TTZip through any native entrypoint (Finder right-click, Drag & Drop, URL Scheme, or in-app Context Menu),
- **I want** the application to instantly activate, transition to the correct workspace tab, and inject all target file/folder paths into the active session without loss,
- **So that** repetitive file operations execute with zero friction and 100% predictable state.
- **Acceptance Criteria**:
  - All 5 entrypoints dispatch a strongly-typed `AppIntent` enum (`.openArchive(URL)`, `.createArchive(paths: [String], preset: UUID?)`, `.quickExtract(paths: [String], to: URL?)`, `.openTab(WorkspaceTab)`).
  - A singleton `AppIntentDispatcher` handles lifecycle arbitration on `@MainActor`.
  - Re-entrant triggers to an already-active or cached tab immediately refresh and populate target paths.

### User Story 2: Reactive Tab Lifecycle Invariant (`US2`)
- **As an** application developer building persistent workspace tabs,
- **I want** a standardized `StatefulTabContainerProtocol` and `@Observable` view model binding pattern,
- **So that** any cached view in `KeepAliveTabContainer` automatically synchronizes with external state updates upon re-activation.
- **Acceptance Criteria**:
  - `CompressFormSession` and child workspace view models expose reactive payload receivers (`loadInputPaths(_:)`, `resetSession()`).
  - No `@State` property in cached tabs relies purely on initial `init` values without `.onChange` or `@Observable` dynamic observation.

### User Story 3: Multi-Entrypoint & State Transition Test Suite (`US3`)
- **As a** core maintainer,
- **I want** automated XCTest integration tests verifying complete intent dispatch and state transition workflows,
- **So that** regressions in tab switching, context-menu path bindings, and URL routing are caught before deployment.
- **Acceptance Criteria**:
  - `AppNavigationStateFlowTests` validates intent injection across all tabs under warm cache conditions.
  - `FinderSyncIntentMappingTests` validates 100% parity between FinderSync action identifiers and `AppIntent` handlers.
  - Zero-allocation and memory-leak verification on repeated intent dispatches.

### User Story 4: Zero-Warning Compiler & Release-by-Default Engineering (`US4`)
- **As a** release engineer and power user,
- **I want** all installation scripts and builds to default strictly to optimized Release configurations and fail on any compiler warning,
- **So that** binaries delivered to users run at peak Apple Silicon hardware speed (LTO, SIMD, stripped symbols) with 100% pristine compiler logs.
- **Acceptance Criteria**:
  - `bundle_app.sh`, `.command` files, and CI pipelines default to `release` mode.
  - All Swift packages build with zero deprecation warnings and zero compiler warnings.
  - Automated CI check verifies that no obsolete assets (`node_modules`, orphaned spec drafts) exist in the repository tree.

---

## 3. Functional Requirements

- **FR-001**: System MUST route all external and internal navigation commands through `AppIntentDispatcher.shared.dispatch(_:)`.
- **FR-002**: System MUST support the full matrix of `FinderSyncActionIdentifier` actions in `TTZipApp.handleIncomingURL(_:)`.
- **FR-003**: System MUST automatically sanitize, expand, and existence-check all incoming path lists before workspace initialization.
- **FR-004**: System MUST ensure that right-clicking multiple items in Miller columns or Finder creates a consolidated archive session with all selected items.
- **FR-005**: System MUST maintain 1px zero-gutter visual alignment for all draggable column dividers while maintaining an 11pt interactive hit box.
- **FR-006**: System MUST enforce `-warnings-as-errors` in release build configurations.
- **FR-007**: System MUST provide an automated repository hygiene validator script (`scripts/lint_repo_hygiene.sh`) to detect orphaned build artifacts or dead directories.

---

## 4. Key Entities & Architecture Model

```
                    ┌──────────────────────────────────────────────┐
                    │               Incoming Triggers              │
                    ├──────────────┬──────────────┬────────────────┤
                    │ FinderSync   │  URL Scheme  │ In-App Context │
                    │  Extension   │  (ttzip://)  │   Menu Action  │
                    └───────┬──────┴───────┬──────┴────────┬───────┘
                            │              │               │
                            ▼              ▼               ▼
                    ┌──────────────────────────────────────────────┐
                    │       AppIntentDispatcher (@MainActor)       │
                    ├──────────────────────────────────────────────┤
                    │ • Sanitizes and existence-checks paths       │
                    │ • Normalizes into strongly-typed AppIntent   │
                    │ • Broadcasts state mutation to AppViewState  │
                    └──────────────────────┬───────────────────────┘
                                           │
                                           ▼
                    ┌──────────────────────────────────────────────┐
                    │           AppViewState (Coordinator)         │
                    ├──────────────────────────────────────────────┤
                    │ • navigationState.activeTab = .compress      │
                    │ • compressSession.loadInputPaths(paths)      │
                    └──────────────────────┬───────────────────────┘
                                           │
                                           ▼
                    ┌──────────────────────────────────────────────┐
                    │     KeepAliveTabContainer (Warm Cache)       │
                    ├──────────────────────────────────────────────┤
                    │ • CompressModalView.onChange(of: paths)      │
                    │ • Renders 100% refreshed input file list     │
                    └──────────────────────────────────────────────┘
```

---

## 5. Success Criteria

1. **Path Injection Reliability**: 100% of valid file/folder paths triggered from right-click or external commands appear in the Compression Workspace within < 50ms.
2. **Compiler Purity**: 0 warnings, 0 deprecations across the entire build pipeline (`swift build -c release`).
3. **Execution Performance**: Release binaries built via default scripts exhibit 100% optimization flags enabled (-O, LTO) with 0 debug assertion overhead.
4. **Test Coverage**: Minimum 10 new state-transition and multi-entrypoint integration test cases passing with 100% success rate.
5. **Codebase Cleanliness**: 0 orphaned artifacts or deprecated subsystem directories.
