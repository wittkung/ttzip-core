# Task Breakdown: 235 - Decoupled Workspace Presentation, Sandbox RAII Leases, and FFI Concurrency Hardening

> **Target Version**: TTWorkspace v6.3  
> **Status**: Completed  

---

## Task Matrix

### Workstream 1: FFI Concurrency & Carmack Prefix Sorting
- [x] **T001**: Purge dummy C-ABI stubs in `products/ttzip/apple/Sources/TTZipApp/Services/Vfs/VfsSnapshotContext.swift`.
- [x] **T002**: Implement `VfsSnapshotLease` RAII reference-counting wrapper and update `NativeArchiveTableViewAdapter.swift` to eliminate closure-escape UAF.
- [x] **T003**: Implement `TTSortEntry` 8-byte big-endian prefix sorting in `ttzip-types` / `ttzip-engine` and Swift adapter, reducing 100k sort time from 3,000ms to $< 10$ms.
- [x] **T004**: Isolate ADB/MTP network and USB I/O to background actor `HardwareIOActor`, removing 4.0s blocking timeout from `@MainActor`.

### Workstream 2: Multi-Tab Workspace Isolation & Session State
- [x] **T005**: Implement `WorkspaceTabSession`, `WorkspaceHistoryStack`, and `TabViewState` in `Models/`.
- [x] **T006**: Fix `closeTab(at:)` index drift bug in `products/ttzip/apple/Sources/TTZipApp/ViewModels/AppSubStates.swift`.
- [x] **T007**: Eliminate global `currentArchivePath` view hijacking in `HomeExplorerContainerView.swift` by scoping archive paths to active tab session.
- [x] **T008**: Persist Miller column navigation hierarchy and scroll positions in `TabViewState` across tab switches.

### Workstream 3: Centralized Presentation Coordinator & Key-Routing Fence
- [x] **T009**: Implement `ActiveModalPresentation` discriminated union and `PresentationCoordinator` singleton in `ViewModels/PresentationCoordinator.swift`.
- [x] **T010**: Fix `GlobalSearchPanelController.swift` Escape key handling (remove `NSApp.hide(nil)`).
- [x] **T011**: Install key-routing fence in `FinderMillerColumnsView.swift` to block Space/Delete leakage when modals are presented.
- [x] **T012**: Reassign tab jumping to ⌥⌘1..⌥⌘8; preserve ⌘1..⌘3 for view modes and ⌘W for closing active tab.

### Workstream 4: Sandbox Governance, RAII Leases & Volume Management
- [x] **T013**: Implement `SecurityScopeLease` reference-counted token model in `RootFolderAccessManager.swift`.
- [x] **T014**: Purge `highestRootURL` path escalation to `/` and `~` in `RootFolderAccessManager.swift`, enforcing strict Least Privilege.
- [x] **T015**: Move `FileIdentity(url:)` out of SwiftUI `body` in `SidebarLocationsSectionView.swift`, `SidebarFavoritesSectionView.swift`, and `SidebarPinnedSectionView.swift`.
- [x] **T016**: Convert `MountedVolumeManager.ejectVolume` to async non-blocking method with a 5.0-second network share timeout watchdog.

### Workstream 5: Inspector Surface Reducer & PreviewKit Lifecycle
- [x] **T017**: Model inspector surface as `InspectorSurfaceMode` discriminated union in `InspectorColumnView.swift`.
- [x] **T018**: Implement `SelectionMetricsAggregator` actor with 256-item cooperative yields (`Task.yield()`) for multi-selection.
- [x] **T019**: Implement 80ms debounced `PreviewSessionCoordinator` with monotonic generation counter in `TTZipPreviewKit`.
- [x] **T020**: Implement `dismantleNSView` in `EPUBNativeWKWebView.swift` to terminate WebKit processes cleanly.

### Workstream 6: Verification, Quality Gates & Deployment
- [x] **T021**: Add comprehensive unit tests in `TTZipAppTests` covering `VfsSnapshotLease`, `WorkspaceHistoryStack`, and `PresentationCoordinator`.
- [x] **T022**: Verify all files satisfy Single-File $\le 800$ LOC, Function $\le 80$ LOC, McCabe CC $\le 15$.
- [x] **T023**: Run `swift test`, `bazel test -c opt //...`, and `python3 tools/ci/license_audit.py`.
- [x] **T024**: Bundle and deploy release app to `/Applications/TTZip.app`.

---

## Follow-up: Archive Miller Navigation (2026-10-24)

> **Slice Status**: Implementation, formal three-target regression (`466/466`), and bounded real GUI acceptance verified; final Batch N staging, full-repo gates, and merge pending coordinator integration. The historical `Completed` status and checked T001–T024 above are preserved as historical records only.

Contract: [Archive Miller Navigation](products/ttzip/docs/twin/navigation/archive-miller-navigation.md). Scope is User Story 6 and the follow-up plan; no new reader, navigation owner/epoch or archive-write capability.

### Workstream 7: Accepted Inspection and Navigation Owner

- [x] **T025**: Add discriminating RED regressions for archive opening in the originating session with physical parent columns retained; include current success, stale completion, cancellation and T1 → T2 → T1. Record actual results without weakening the contract (`ArchiveOpenNavigationOwnershipTests`, `FinderMillerColumnsNavigationTests`).
- [x] **T026**: Expose accepted inspection entries through existing archive hierarchy projection/session types for Miller listing, preserving the existing reader/password-evidence boundary and cache invalidation. Prove sibling listing does not re-inspect or implicitly take the C-ABI VFS fast path (`AcceptedArchiveDirectoryProjection.swift`, `MillerColumnDirectoryScanner.swift`). Depends on T025.
- [x] **T027**: Wire synchronous archive location/context/projection acceptance through the existing `NavigationState` and `ArchiveOpenRequestTicket`; retain session identity and existing loading ownership. One writer owns `AppSubStates.swift`. Depends on T026 and T028's agreed location semantics.

### Workstream 8: Existing History, Columns and Password Resume

- [x] **T028**: Make existing `WorkspaceHistoryStack` identity, parent traversal and breadcrumb location semantics archive-subpath-aware; cover encoded names, root/subdirectories, Back/Forward and return to filesystem (`NavigationSessionOwnershipTests` 13/13). This model work may proceed alongside T026; do not concurrently edit `AppSubStates.swift`.
- [x] **T029**: Route ordinary archive opening through the existing Miller view, retaining physical parent columns and restoring column depth/selection from the same session. List direct children from the accepted projection, including implicit and explicit empty directories. Keep table/treemap explicit (`FinderMillerColumnsView+ArchiveLocations.swift`, `FinderMillerColumnsView+Contents.swift`). Depends on T027–T028.
- [x] **T030**: Resume password verification into the same accepted Miller location; cancelled/stale operations cannot commit a location or prompt. Gate typed errors and window-targeted prompting behind current consumer acceptance; separate physical read/credential source (`archivePath`) from semantic browsing location (`location`) across `/private/tmp` vs `/tmp` aliases (`ArchiveEntryPreviewPasswordRoutingTests` 9/9, `/tmp/ttzip-main-324-acceptance/inspector-alias-green.log`). Depends on T027 and T029.

### Workstream 9: Existing Preview/Extraction and Formal Acceptance

- [x] **T031**: Reuse read-only archive preview provenance, cache keys and consumer leases; route virtual multi-selection to existing `extractEntries` with archive path and entry paths. Preserve destination/collision authorization and do not add archive writes, recursive nested navigation or whole-archive extraction for browsing (`FinderMillerColumnsNavigationTests` 33/33, `MediaPreviewAuditTests` 22/22). Depends on T029–T030.
- [x] **T032**: Execute formal three-target regressions (console summary `/tmp/ttzip-antigravity-miller/formal-three-targets-2152.log` and XCTest logs `bazel-testlogs/products/ttzip/apple/{AppCoreGateTests,NavigationSessionOwnershipTests,UIPreviewKitTests}/test.log`: `466/466` passed across `AppCoreGateTests` 264, `NavigationSessionOwnershipTests` 13, `UIPreviewKitTests` 189), production candidate packaging via `build_bazel_channels.sh` (`/tmp/ttzip-main-324-acceptance/inspector-alias-fixed-package.log`), and bounded single-window AX GUI acceptance (`/tmp/ttzip-antigravity-miller/gui-2130/` covering file-panel and Miller-row open of `acceptance.zip`, `Back`/`Forward`/`Up`, and direct encrypted `README.txt` same-window password overlay/wrong-password/cancel/close-and-reopen-reprompt/unlock-and-AXTextArea-update in `encrypted-acceptance.zip` with preferences restored). Multi-window GUI, full-repo `bazel test -c opt //...`, `ttaction run`, and Batch N commit/merge remain with the coordinator. Depends on T025–T031.
