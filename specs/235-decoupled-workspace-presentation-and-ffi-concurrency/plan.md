# Implementation Plan: 235 - Decoupled Workspace Presentation, Sandbox RAII Leases, and FFI Concurrency Hardening

> **Target Version**: TTWorkspace v6.3  
> **Status**: Accepted Implementation Plan  

---

## 1. Execution Architecture & Phasing

```
┌────────────────────────────────────────────────────────────────────────┐
│                        Spec 235 Execution Pipeline                     │
├────────────────────────────────────────────────────────────────────────┤
│ Phase 1: Contracts & Data Models (Zero-Debt Interface Definitions)     │
│   - contracts/presentation-coordinator.json                            │
│   - contracts/workspace-session.json                                   │
│   - data-model.md                                                      │
├────────────────────────────────────────────────────────────────────────┤
│ Phase 2: FFI Concurrency & Memory Safety Hardening (Carmack Grade)     │
│   - Purge dummy stubs in VfsSnapshotContext.swift                      │
│   - Implement VfsSnapshotLease RAII wrapper                            │
│   - Implement HardwareIOActor for ADB TCP & MTP USB                    │
│   - Implement Carmack 8-byte prefix sorting (TTSortEntry)              │
│   - Wire CancellationToken across FFI boundary                         │
├────────────────────────────────────────────────────────────────────────┤
│ Phase 3: Multi-Tab Workspace Isolation & Session State SSOT            │
│   - Implement WorkspaceTabSession, WorkspaceHistoryStack, TabViewState │
│   - Fix closeTab(at:) index drift in AppSubStates.swift                │
│   - Eliminate global currentArchivePath hijacking                      │
│   - Persist Miller column hierarchy and scroll target in TabViewState  │
├────────────────────────────────────────────────────────────────────────┤
│ Phase 4: Centralized Presentation Coordinator & Key-Routing Fence      │
│   - Implement ActiveModalPresentation & PresentationCoordinator        │
│   - Fix GlobalSearchPanelController Escape handling                    │
│   - Install key-routing fence in FinderMillerColumnsView               │
│   - Reassign shortcuts (View modes ⌘1..⌘3, Tabs ⌥⌘1..⌥⌘8)             │
│   - Replace NSThemeFrame private hacks with standard AppKit APIs       │
├────────────────────────────────────────────────────────────────────────┤
│ Phase 5: Sandbox Governance, RAII Leases & Volume Management           │
│   - Implement SecurityScopeLease reference-counted tokens              │
│   - Purge highestRootURL path escalation (/ and ~)                     │
│   - Move FileIdentity out of SwiftUI body to background ViewModel      │
│   - Make MountedVolumeManager.ejectVolume async with 5s watchdog       │
│   - Synchronize Swift favorites with Rust FavoritesRegistry            │
├────────────────────────────────────────────────────────────────────────┤
│ Phase 6: Inspector Surface Reducer & PreviewKit Lifecycle              │
│   - Implement InspectorSurfaceMode state machine                       │
│   - Implement SelectionMetricsAggregator actor (256-item yields)       │
│   - Implement PreviewSessionCoordinator with 80ms debounce             │
│   - Implement dismantleNSView for EPUBNativeWKWebView                  │
│   - Stream CSV/TSV parsing off MainActor                               │
├────────────────────────────────────────────────────────────────────────┤
│ Phase 7: Verification, LOC Gate & Monorepo Bazel 8 Build               │
│   - Run swift test & bazel test -c opt //...                           │
│   - Verify LOC <= 800 per file, fn LOC <= 80, CC <= 15                │
│   - Bundle & deploy to /Applications/TTZip.app                         │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Risk Mitigation & Verification Matrix

| Area | Primary Risk | Prevention / Invariant | Verification Gate |
|---|---|---|---|
| **FFI Safety** | UAF on snapshot release; table adapter retaining freed native pointers. | `VfsSnapshotLease` reference count locks; zero raw pointer retention in Swift. | AddressSanitizer & `testVfsSnapshotReader` suite. |
| **Tab Isolation** | State corruption on closing active tab; navigation history loss. | Immutable `activeTabID: UUID` SSOT; non-destructive `WorkspaceHistoryStack`. | Tab lifecycle unit tests in `WorkspaceSessionManagerTests`. |
| **Modal Presentation** | Multiple sheets presented simultaneously; Escape key hiding app. | `ActiveModalPresentation` mutually exclusive sum type; local monitor fence. | UI event integration tests; Escape key assertions. |
| **Sandbox Compliance** | App Store rejection on path escalation; Mach port exhaustion. | Strict Least Privilege (no `/` or `~` escalation); RAII `SecurityScopeLease`. | `license_audit.py` & security lease unit tests. |
| **PreviewKit** | Memory spikes during 60 FPS keyboard scrolling; orphaned WebKit. | 80ms debounce window; monotonic `sessionGeneration`; `dismantleNSView`. | 100-file rapid scrolling benchmark in Instruments. |

---

## 3. Archive Miller Navigation Follow-up Plan (2026-10-24)

> **Slice Status**: Implementation, formal three-target regression (`466/466`), and bounded single-window GUI acceptance verified; Batch N staging, full-repo gates, and merge pending coordinator integration. This appendix preserves the historical phases above and does not reopen unrelated FFI or media work.

Contract: [Archive Miller Navigation](products/ttzip/docs/twin/navigation/archive-miller-navigation.md), implementing User Story 6 in this feature specification.

### Phase A: Accepted Inspection and Owner Commit

1. Prepare discriminating RED cases for current parent location, originating session, accepted inspection reuse, cancelled/late completion and password recovery. Keep synthetic fixtures independent of user files.
2. Reuse the existing `ArchiveOpenRequestTicket` and navigation invalidation callback. Prepare the accepted-inspection projection using existing tree/session types, without changing the inspection reader, adding an epoch or introducing another mutable navigation store.
3. Define a synchronous `NavigationState` commit for the originating session: preserve physical parent columns and publish the archive location/context. Route both normal success and existing password resume through the same acceptance semantics. Projection preparation before any suspension must not authorize a later stale commit.

### Phase B: Existing History and Miller Consumer

4. In parallel with projection preparation, make the existing history/location identity and parent semantics archive-aware. Preserve query-contained entry paths and encoded names; expose existing-owner-compatible semantics for breadcrumbs and view restoration. Do not create a second history.
5. Integrate those semantics in the owner, then route ordinary archive browsing to the existing Miller consumer. Keep `columnPaths`/selection as a session view projection, and list only the accepted hierarchy's direct children. Keep old table functionality behind explicit entry points during this bounded slice.
6. Move error/prompt decisions behind current-result acceptance. Verify window identity, cancellation before acceptance and cancellation while classification is pending; no background scanner may publish a stale prompt.

### Phase C: Existing Actions and Evidence

7. Reuse inspector/read-only resource context, preview leases and authorized extraction. Correct virtual multi-selection to carry entry paths. Do not expand archive mutation capabilities or nested archive semantics.
8. The coordinator serially executes formal targeted regressions and relevant production builds, then validates a real folder → archive root → internal directory → filesystem return in the same Miller columns. Report automated and GUI results separately.

### Write Ownership and Integration Boundary

- **Owner/projection lane**: `AppViewState+ArchiveOperations.swift`, `ArchivePasswordOperationContext.swift`, the accepted-result interface in `ArchiveHierarchySessionCache.swift`, and the `NavigationState` commit surface in `AppSubStates.swift`. Assign one writer for `AppSubStates.swift`.
- **History/location lane**: `WorkspaceHistoryStack.swift` and the existing path-presentation/breadcrumb semantics. It may prepare direct model regressions independently; it MUST coordinate any `AppSubStates.swift` change with the owner lane rather than editing that file concurrently.
- **Miller/view lane after owner/history agreement**: `HomeExplorerContainerView.swift`, `DiskDirectoryBrowserView.swift`, `FinderMillerColumnsView` and its selection/navigation extensions, and `MillerColumnDirectoryScanner.swift`. These consume the agreed owner/location behavior; they do not establish another owner.
- Keep unrelated text-preview diagnosis files, production packaging and shared build configuration outside this slice's parallel edit ownership.

### Follow-up Risk Matrix

| Risk | Required invariant | Verification status |
| --- | --- | --- |
| Separate archive page remains the default | Same session and retained parent columns | Verified: `ArchiveOpenNavigationOwnershipTests` (`19/19`) + single-window AX snapshots `02-miller-acceptance-zip-root.txt` & `11-file-panel-open-acceptance-zip.txt` |
| Different archive subpaths collapse | Semantic location identity (`?subpath=...`) and parent chain | Verified: `NavigationSessionOwnershipTests` (`13/13`), `FinderMillerColumnsNavigationTests` (`33/33`), + single-window AX snapshots `05a`–`05d` |
| Reader or password retry boundary silently changes | Accepted inspection reused by listing; physical read source (`archivePath`) separated from browsing location (`location`) | Verified: `ArchiveEntryPreviewPasswordRoutingTests` (`9/9`, including alias RED/GREEN and `inspectCount == 1`) + single-window AX snapshots `06`–`10b` |
| Late completion writes another tab or prompts | Existing ticket/epoch and synchronous acceptance | Verified by controlled tests: `ArchiveOpenNavigationOwnershipTests` (`19/19`) & `ArchiveEntryPreviewPasswordRoutingTests` (`9/9`) |
| Directory UI grants unintended write access | Read-only provenance and authorized extraction only | Verified by controlled tests: `FinderMillerColumnsNavigationTests` (`33/33`) & `MediaPreviewAuditTests` (`22/22`) |
