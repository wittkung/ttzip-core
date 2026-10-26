# Feature Specification: 235 - Decoupled Workspace Presentation, Sandbox RAII Leases, and FFI Concurrency Hardening

> **Target Version**: TTWorkspace v6.3 (Decoupled Workspace Presentation & Concurrency Hardened Edition)  
> **Status**: Accepted Architecture Specification  
> **Classification**: Core Presentation Architecture, Sandbox Governance & FFI Concurrency Invariants  

---

## 1. System Overview & Problem Statement

### 1.1 Problem Statement
During forensic architecture audit and user interaction analysis, five critical cross-cutting smells and boundary violations were discovered across the macOS presentation layer, sandboxing kernel interface, and Rust FFI boundary:

1. **FFI Concurrency & Memory Safety Hazards**:
   - `VfsSnapshotContext.swift` maintained dummy stubs shadowing actual C-ABI exports `ttvfs_acquire_snapshot` and `ttvfs_release_snapshot`.
   - `NativeArchiveTableViewAdapter.swift` performed closure-escaping assignments (`viewCopy = view` from inside `withSnapshot` to `self.compactView`), introducing Use-After-Free (UAF) undefined behavior upon closure unwinding.
   - `AndroidDeviceViewModel.swift` executed a 4.0-second synchronous TCP socket connection timeout on `@MainActor` via `engine.listAdbDevices()`, freezing AppKit event processing.
   - 100,000-file table column sorting executed `localizedStandardCompare` on the main actor, generating 3.4M transient `String` allocations and blocking for >3,000ms.
   - Swift `Task.cancel()` was completely disconnected from underlying native Rust worker threads.

2. **Multi-Tab Workspace State Leakage & Index Drift**:
   - `activeTabIndex: Int` acted as the state pointer. Closing tabs to the left of the active tab (`index < activeTabIndex`) failed to decrement the index, causing pointer drift to adjacent tabs.
   - Window-global singletons (`currentArchivePath`, `archiveExplorerState`) were shared across tabs. Switching from an archive tab to a regular folder tab erroneously hijacked the view.
   - Context-menu Back navigation invoked `navigateTo()`, which pushed a new URL onto the history stack and permanently truncated the Forward history stack.
   - Miller column paths, selection sets, and scroll offsets were stored in ephemeral `@State`, resetting navigation depth on every tab switch.

3. **Modal Presentation State Explosion & Key-Routing Collisions**:
   - Over 23 uncoordinated boolean flags and 6 concurrent `.sheet` modifiers were mounted simultaneously on `MainView`, causing presentation races and rendering anomalies.
   - `GlobalSearchPanelController` executed `NSApp.hide(nil)` upon receiving the Escape key, hiding the entire application process rather than dismissing the search panel.
   - Key shortcut collisions occurred between view modes (⌘1..⌘3) and tab switching, and between archive resetting (⌘W) and tab closure.
   - `NSEvent.addLocalMonitorForEvents` captured Space (media preview) and Delete (entry removal) even while modal sheets or input fields were active.
   - `WindowTabbingConfigurator` executed window-snapping and private `NSThemeFrame` subview scraping in `updateNSView`.

4. **Sandbox Kernel Port Leaks & Least Privilege Violations**:
   - `SidebarLocationsSectionView`, `SidebarFavoritesSectionView`, and `SidebarPinnedSectionView` executed POSIX `lstat` and `readlink` inside `var body: some View` via `FileIdentity(url:)`.
   - `RootFolderAccessManager` utilized an uncounted `Set<URL>`, leaking Darwin MACF sandbox extension tokens and Mach ports upon repeated access.
   - `highestRootURL` heuristically escalated child paths (e.g. `~/Downloads/file.zip`) to `/Users/<username>` or `/`, violating App Store Review Guideline 2.4.5(i).
   - `MountedVolumeManager` executed synchronous `NSWorkspace.shared.unmountAndEjectDevice` on `@MainActor`, blocking for up to 60 seconds on stalled network shares (SMB/NFS).
   - Swift bypassed the Rust microkernel's `FavoritesRegistry`, storing raw bookmarks in `UserDefaults`.

5. **Inspector Tight Coupling & PreviewKit Decoder Leaks**:
   - `RightInspectorSidePanel` directly consumed the `AppViewState` god-object, synthesizing virtual URLs on the render pass.
   - The inspector lacked a multi-selection domain model, falling back to single-item metadata when multiple files were selected.
   - Rapid arrow-key navigation spawned un-cancellable `Task.detached` single-entry extractions.
   - `AVSampleBufferVideoPlayerView` raced between `.onAppear` and `.task(id:)`, leaking CoreMedia decoders.
   - `EPUBNativeWKWebView` omitted `dismantleNSView`, leaving orphaned WebKit rendering processes.
   - Large CSV/TSV spreadsheets were parsed synchronously on the UI thread.

### 1.2 System Boundary & Objectives
This specification establishes five unified architectural contracts:
- **Contract 1: FFI RAII Lease & Carmack Prefix Sorting**: Introduce `VfsSnapshotLease`, isolate hardware I/O to `HardwareIOActor`, implement 8-byte big-endian prefix sorting (`TTSortEntry`), and wire cooperative `CancellationToken`.
- **Contract 2: Multi-Tab Workspace Session SSOT**: Re-architect workspace tabs under `WorkspaceTabSession` with immutable UUID pointers, dual-stack `WorkspaceHistoryStack`, and persistent `TabViewState`.
- **Contract 3: Centralized Presentation Coordinator & Key-Routing Fence**: Unify modals under `PresentationCoordinator` with 3 priority tiers, monotonic Escape dismiss, key-routing fences, and standard window titlebar styling.
- **Contract 4: Reference-Counted Sandbox Leases & Async Volume Management**: Introduce `SecurityScopeLease`, enforce strict Least Privilege (zero path escalation), make volume eject async with a 5s watchdog timeout, and move POSIX calls out of SwiftUI `body`.
- **Contract 5: Inspector Surface Reducer & PreviewKit Lifecycle**: Model the inspector as `InspectorSurfaceMode`, add `SelectionMetricsAggregator` with 256-item yields, introduce 80ms debounced `PreviewSessionCoordinator`, and enforce WebKit `dismantleNSView`.

---

## 2. User Stories & Acceptance Criteria

### User Story 1: Reliable FFI Memory Safety & Hang Immunity
- **As a** user browsing large archives (100,000 files) or connecting Android devices,
- **I want** column sorting and device detection to never freeze the application or leak native pointers,
- **So that** 120 FPS UI responsiveness and memory safety invariants are strictly upheld.

#### Acceptance Criteria
1. `VfsSnapshotContext` MUST NOT contain dummy stubs shadowing C-ABI exports.
2. `NativeArchiveTableViewAdapter` MUST retain snapshots via `VfsSnapshotLease` without closure pointer escape.
3. Sorting 100,000 entries MUST execute in $< 10$ms using Carmack 8-byte prefix comparison (`TTSortEntry`) with zero transient `String` heap allocations on the main thread.
4. All ADB and MTP socket communications MUST run on `HardwareIOActor` off `@MainActor`.
5. Cancelling a Swift `Task` MUST toggle the native `AtomicBool` cancellation token within 128 items in Rust.

### User Story 2: Deterministic Multi-Tab Navigation & Session Isolation
- **As a** power user working with multiple tabs (local folders, archives, storage inspector),
- **I want** each tab to maintain complete isolation of its history, selection, and scroll position,
- **So that** closing or switching tabs never corrupts navigation state or hijacks the view.

#### Acceptance Criteria
1. Closing a tab to the left of the active tab MUST NOT cause active tab index drift.
2. Switching tabs MUST NOT leak archive paths or hijack regular folder views.
3. Back/Forward navigation MUST use `WorkspaceHistoryStack` without truncating forward history during Back operations.
4. Miller column navigation hierarchy and scroll positions MUST persist across tab switches in `TabViewState`.

### User Story 3: Predictable Modal Presentation & Keyboard Shortcuts
- **As a** keyboard-centric macOS user,
- **I want** predictable shortcuts and clean modal dismissal,
- **So that** pressing Escape never hides the application, and modals never conflict.

#### Acceptance Criteria
1. Modals MUST be coordinated by `PresentationCoordinator` with a discriminated union `ActiveModalPresentation`.
2. Pressing Escape MUST dismiss the topmost modal/overlay and MUST NOT call `NSApp.hide(nil)`.
3. ⌘1..⌘3 MUST switch view modes; ⌥⌘1..⌥⌘8 MUST jump to tabs; ⌘W MUST close only the active tab.
4. Active modals MUST install a key-routing fence blocking Space and Delete keys from propagating to background views.
5. Window titlebar MUST use standard AppKit public APIs without private `NSThemeFrame` pruning.

### User Story 4: Sandbox Resource Governance & Async Volume Management
- **As an** App Store user,
- **I want** file access to follow Least Privilege without kernel Mach port leaks,
- **So that** sandbox extensions are deterministically released and ejecting volumes never hangs the UI.

#### Acceptance Criteria
1. Security-scoped bookmarks MUST be governed by reference-counted `SecurityScopeLease` tokens.
2. Path requests MUST NOT escalate to `/` or `~` via `highestRootURL`.
3. SwiftUI `body` evaluation MUST NOT execute POSIX `lstat` or `readlink`.
4. Volume ejection MUST be asynchronous with a 5.0-second network timeout watchdog.
5. Pinned folders MUST synchronize with the Rust microkernel `FavoritesRegistry`.

### User Story 5: Responsive Inspector Surface & Fluid Previews
- **As a** user inspecting large files and navigating with arrow keys,
- **I want** the right inspector to update instantly and previews to debounce cleanly,
- **So that** rapid scrolling never causes memory spikes, audio leaks, or WebKit process leaks.

#### Acceptance Criteria
1. Inspector surface MUST follow `InspectorSurfaceMode` (empty, single, multiple, storageInspectorActive).
2. Multi-selection MUST aggregate size, item count, and MIME distribution off the main thread with 256-item yields.
3. Media preview MUST debounce with an 80ms window and monotonic generation counter.
4. `EPUBNativeWKWebView` MUST implement `dismantleNSView` to terminate WebKit processes.
5. CSV/TSV parsing MUST be streamed from a background actor with a 64KB first-chunk threshold.

---

## 3. Archive Miller Navigation Follow-up Slice (2026-10-24)

> **Slice Status**: Implementation, formal three-target regression (`466/466`), and bounded single-window GUI acceptance (including encrypted-entry preview flow) verified; Batch N staging, full-repo gates, and merge pending coordinator integration. Historical acceptance criteria above remain preserved and are separate from this slice's verification evidence.

### User Story 6: Archives Open Inside the Existing Miller Columns

- **As a** user browsing a folder containing archives,
- **I want** opening an archive to expand its root and internal directories in the existing Miller columns while retaining the physical parent columns,
- **So that** archive browsing, filesystem browsing, Back/Forward and the inspector share one continuous navigation session.

The detailed contract is [Archive Miller Navigation](products/ttzip/docs/twin/navigation/archive-miller-navigation.md). This slice refines Contract 2 and does not introduce a second navigation owner, reader, history stack or operation epoch.

#### Acceptance Criteria

1. Ordinary archive opening MUST preserve the originating `WorkspaceTabSession.id` and physical parent columns. It MUST NOT force the separate archive table page; archive tables/treemaps may remain explicit functions.
2. `NavigationState` MUST own location commits. An archive root and each internal directory MUST be distinct semantic locations in the existing `WorkspaceHistoryStack`; comparison MUST NOT collapse them by physical `URL.path` alone. Parent traversal MUST follow internal child → internal parent → archive root → physical parent. Back MUST preserve Forward.
3. Archive-open success MUST validate the existing session/epoch request ticket and commit its location, archive context and accepted inspection projection together. Miller column paths and selection remain projections of that session, restored through existing session view state.
4. Directory listing MUST reuse entries accepted by the existing inspection/password-evidence boundary and existing hierarchy projection. Sibling traversal MUST NOT re-inspect the archive or implicitly switch ordinary opening to C-ABI VFS. Implicit directories, explicit empty directories and encoded names MUST retain their identities.
5. Password waiting MUST retain the parent location without adding a successful archive location to history. Successful verification MUST resume the same owner and archive location. Cancellation, close, navigation-away and T1 → T2 → T1 MUST reject late successes, errors and prompts. Unsupported/corrupt/permission errors MUST NOT become password requests.
6. Directory scanners MUST NOT present UI before the active consumer accepts their result. Existing task/request identity and window targeting MUST gate commit and necessary prompting.
7. Virtual selection MUST retain read-only archive provenance, deterministic cache identity and existing consumer leases. Whole-archive and selected-entry extraction MUST reuse existing authorized operations, destination and collision handling. Multi-selection MUST submit archive path plus entry paths, never fabricated local URLs.
8. This slice MUST NOT add archive writes, recursive nested-archive navigation or automatic whole-archive extraction for browsing. Existing permission boundaries and unrelated background tasks MUST remain governed by their current owners.

#### Verification Status

- **Model & Driver Regressions (Verified)**:
  - Alias-mismatch RED/GREEN (`testArchiveAddressAliasKeepsReadSourceCredentialAndBrowsingOwner` + `ArchiveEntryPreviewPasswordRoutingTests` 9/9, verifying no re-inspection `inspectCount(driver) == 1`, preserved `WorkspaceHistoryStack`, and window/entry-scoped `TTZipArchiveUnlockedRefresh` retry): `/tmp/ttzip-main-324-acceptance/inspector-alias-red.log`, `/tmp/ttzip-main-324-acceptance/inspector-alias-green.log`.
  - Formal three-target regression (`466/466` passed, `0` failures): target pass summary in `/tmp/ttzip-antigravity-miller/formal-three-targets-2152.log` and per-suite XCTest case counts in `bazel-testlogs/products/ttzip/apple/AppCoreGateTests/test.log` (`264` tests, including `ArchiveEntryPreviewPasswordRoutingTests` 9/9 and `ArchiveOpenNavigationOwnershipTests` 19/19), `bazel-testlogs/products/ttzip/apple/NavigationSessionOwnershipTests/test.log` (`13` tests), and `bazel-testlogs/products/ttzip/apple/UIPreviewKitTests/test.log` (`189` tests, including `FinderMillerColumnsNavigationTests` 33/33 and `MediaPreviewAuditTests` 22/22).
- **Production Packaging & Real GUI Acceptance (Verified at Single-Window AX Control Layer)**:
  - Packaged `/tmp/ttzip-inspector-alias-fixed-20261024/acceptance/TTZip.app` via `products/ttzip/apple/scripts/build_bazel_channels.sh --channel acceptance` (log `/tmp/ttzip-main-324-acceptance/inspector-alias-fixed-package.log`).
  - Verified on PID `77034` (`2026-10-24 21:29:45`, AX control snapshots `/tmp/ttzip-antigravity-miller/gui-2130/01-fixtures-dir-ax.txt` through `11-file-panel-open-acceptance-zip.txt`) using self-owned fixtures `/tmp/ttzip-main-324-acceptance/fixtures/{acceptance.zip,encrypted-acceptance.zip}`: file-panel (`⌘O`) and Miller-row archive opening with preserved parent column, internal directory traversal (`Documents` → `README.txt`), `Back (⌘[)` / `Forward (⌘])` / `Up (⌘↑)` to physical parent, and real encrypted-entry preview flow (`README.txt` inside `encrypted-acceptance.zip` showing same-window password overlay `请输入解密密码`, keeping failure on wrong password, staying locked after pressing `取消`, re-triggering the overlay after closing/reopening `encrypted-acceptance.zip` and selecting `README.txt` in `10a`, and automatically populating the Inspector `AXTextArea` with the three-line decrypted text on submitting disposable test password `ttzip-acceptance-only` via `解锁钥匙库` in `10b`, with `WINDOW_COUNT=1` and visible breadcrumb unchanged). User `com.metastudyline.TTZip` preferences were backed up and restored identically.
- **Unverified / Pending Coordinator Integration**:
  - Pixel-level screenshot composition and multi-window interactive GUI isolation (cross-window/no-reinspection/history invariants are covered at model/router layer in `ArchiveEntryPreviewPasswordRoutingTests` and `ArchiveOpenNavigationOwnershipTests`).
  - Full-repo `bazel test -c opt //...`, `ttaction run`, exact 39-path Batch N staging, and commit/PR/merge.
