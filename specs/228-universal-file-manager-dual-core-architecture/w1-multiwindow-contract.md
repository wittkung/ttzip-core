# W1 Multi-Window Browsing & Window State Isolation Contract

**Feature Track**: `228-universal-file-manager-dual-core-architecture` (Workstream W1)  
**Date**: 2026-10-17  
**Status**: Approved for Implementation  
**Scope**: macOS Finder-Style Multi-Window Browsing, Per-Window State Isolation, Command Routing, Modal Ownership, and Lifecycle Cleanup

---

## 1. Window Identity, Creation, Registration, Activation & Destruction Lifecycle

### 1.1 Window Identity (`windowID: UUID`)
Every browser window in TTZip is uniquely identified by an immutable `windowID: UUID`:
- Each `AppViewState` instance owns a stable `windowID: UUID` assigned at initialization (`AppViewState(windowID: ...)`).
- Each window owns a dedicated `PresentationCoordinator` instance bound 1:1 to that window's `AppViewState`, eliminating reliance on `PresentationCoordinator.shared`'s "last-bound" state.
- When `MainView` attaches to an `NSWindow` (via `WindowConfiguratorNSView.viewDidMoveToWindow`), the `NSWindow` is associated with the `windowID` in `BrowserWindowRegistry.shared`.

### 1.2 Unified Single-Path Window Creation (`BrowserWindowRouter`)
To prevent duplicate or "ghost" windows caused by SwiftUI `WindowGroup` and AppKit creating windows concurrently:
1. **Single Window Group Scene**: `TTZipApp` declares `WindowGroup(id: BrowserWindowRouter.windowGroupID, for: UUID.self) { $windowID in ... }` with a default `UUID()` value so the initial launch creates exactly one primary browser window.
2. **Creation Gateway**: `BrowserWindowRouter.shared` (in `Sources/TTZipApp/Services/Windows/`) serves as the sole programmatic window creation coordinator:
   - `MainView` registers its `@Environment(\.openWindow)` bridge with `BrowserWindowRouter.shared`.
   - When `openNewWindow(with:)` is invoked (via `⌘N`, sidebar "Open in New Window", Dock reopen when 0 windows exist, or external multi-file open):
     1. A `BrowserWindowLaunchSeed` (initial directory URL, archive URL, entry subpath, or deferred `AppIntentEnvelope`) is enqueued in `BrowserWindowRegistry.shared` (`enqueueLaunchSeed(seed)`).
     2. If the SwiftUI `openWindow(id:)` bridge (`swiftUIWindowOpener`) is registered, it is invoked once.
     3. In headless/unit-test environments or when all SwiftUI scenes are torn down and no `swiftUIWindowOpener` is active, `BrowserWindowRouter` invokes its registered `fallbackWindowCreator` without double-triggering.
     4. **Unconsumed Seed Rollback Guarantee**: If neither `swiftUIWindowOpener` nor `fallbackWindowCreator` is registered and `NSApp.sendAction` does not increase the registered window count synchronously, the newly enqueued `BrowserWindowLaunchSeed` is immediately rolled back (`discardUnconsumedLaunchSeed(id:)`) so it never lingers in `pendingLaunchSeeds` to misroute into a later unrelated window.
   - **Single-Consumption Lock (`hasAppliedLaunchSeed`)**: When `MainView` mounts (`onAppear`), if `!viewModel.hasAppliedLaunchSeed`, it immediately marks `viewModel.hasAppliedLaunchSeed = true` **regardless of whether `consumeNextLaunchSeed()` returns a seed or `nil`**. This guarantees that a cold-launched primary window (which mounts when `pendingLaunchSeeds` is empty) can never steal a queued launch seed on a subsequent `onAppear` re-entry.

### 1.3 Registration, Activation (MRU Focus Tracking) & Destruction
`BrowserWindowRegistry.shared` (`@MainActor`) maintains the authoritative table of live browser windows:
- **Registration**: On `MainView.onAppear` and `NSWindow` attachment, the window session is registered (`register(windowID:session:nsWindow:)`). If no active window is currently recorded, the newly registered window becomes `activeWindowID`.
- **Activation**: Observers on `NSWindow.didBecomeKeyNotification` and `NSWindow.didBecomeMainNotification` (filtered strictly to registered browser windows, ignoring floating panels like `GlobalSearchPanelWindow` or sheets) update `activeWindowID` and push `windowID` to the front of the Most-Recently-Used (`mruWindowIDs`) stack.
- **Destruction**: On `NSWindow.willCloseNotification` or `MainView.onDisappear`:
  1. `viewModel.prepareForWindowClose()` executes deterministic cleanup (resolving pending collision continuations, releasing window-owned preview leases, stopping media playback only if owned by this window).
  2. `AppIntentDispatcher.shared.unregisterWindow(id: windowID)` and `BrowserWindowRegistry.shared.unregister(windowID: windowID)` remove the window entry.
  3. Focus automatically falls back to the next live window in `mruWindowIDs` (or `nil` if all browser windows are closed).

```
┌────────────────────────────────────────────────────────────────────────────┐
│                        BrowserWindowRouter & Registry                      │
├────────────────────────────────────────────────────────────────────────────┤
│  ⌘N / Sidebar "Open in New Window" / External URLs / Dock Reopen           │
│         │                                                                  │
│         ▼                                                                  │
│  BrowserWindowRouter.shared.openNewWindow(seed)                            │
│    ├─ Stages BrowserWindowLaunchSeed[windowID]                             │
│    └─ Invokes SwiftUI openWindow(id: "ttzip.browser", value: windowID)     │
│         │                                                                  │
│         ▼                                                                  │
│  MainView(windowID) mounts                                                 │
│    ├─ Creates isolated AppViewState(windowID) + PresentationCoordinator    │
│    ├─ Registers in BrowserWindowRegistry.shared & AppIntentDispatcher      │
│    └─ Consumes BrowserWindowLaunchSeed[windowID] once                      │
└────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Per-Window Private State Isolation

Each browser window owns an isolated object graph rooted at its `AppViewState` and `PresentationCoordinator`:

| Domain | Owner | Isolated Properties | Isolation Guarantee |
| :--- | :--- | :--- | :--- |
| **Directory & History** | `viewModel.navigationState` | `currentDirectory`, `tabSessions`, `activeTabIndex`, `historyStack` (back/forward/up), `isLeftSidebarVisible`, `isInspectorVisible`, `layoutMode` | Navigating or switching history in Window A never mutates Window B's `currentDirectory` or history stack. |
| **Selection & Inspector** | `viewModel.selectionState` | `selectedDiskItem`, `selectedDiskItems`, `selectedEntryID` | Selecting a file or archive entry in Window A updates only Window A's right-hand Inspector column. |
| **Search & Filtering** | `MainView` & `viewModel.explorerState` | Window directory `searchQuery` + `SpotlightSearchService`, in-archive `explorerState.searchQuery` + `filteredEntries` + `SemanticSearchService` | Typing a search query in Window A filters only Window A. |
| **Archive Session** | `viewModel.explorerState` | `currentArchivePath`, `activePassword`, `currentEntries`, `vfsSession`, `snapshotContext`, `operationEpoch` | Two windows can inspect different archives (or the same archive at different virtual folders/queries) independently. |
| **Compression Form** | `viewModel.compressSession` | `CompressFormSession` (`itemsList`, format, level, encryption inputs) | Opening the compress workspace in Window A does not alter Window B. |
| **Modals, Sheets & Overlays** | Window-owned `PresentationCoordinator` + `viewModel.overlayState` | `.omnibar`, `.immersiveMedia`, `.extract`, `.archiveInspector`, `.archiveIntegrity`, `.secondaryTool`, `.passwordPrompt`, `.collisionPrompt` | Every window drives its `.sheet` and `MainRootOverlaysView` from its own `PresentationCoordinator` and `OverlayState`. |

---

## 3. Application-Shared State

The following services remain process-wide singletons shared safely across all browser windows:

1. **Localization & Formatting (`AppLocalizationState.shared`, `TTZipLocalizationManager.shared`)**:
   - Switching language updates `AppLocalizationState.shared.currentLanguage` and `AppKitMenuSynchronizer.shared` in real time.
   - Because `AppLocalizationState` is `@Observable`, all open windows re-render localized text in-place **without recreating `MainView` or resetting `AppViewState`**.
2. **Theme & Layout Defaults (`@AppStorage`)**:
   - Accent mineral theme (`ttzip_accent_theme`) applies globally across all windows' fluid backgrounds.
   - Sidebar width (`TTZip_UserLeftSidebarWidth`) and inspector ratio (`TTZip_UserRightInspectorRatio`) provide initial defaults for new windows while allowing live per-window resizing.
3. **Favorites, Volumes & Security Scoped Bookmarks**:
   - `FinderFavoritesViewModel.shared`, `MountedVolumeManager.shared`, `RootFolderAccessManager.shared`, and `RecentArchiveRecord` storage are shared across windows so pinning a folder or granting sandbox access in one window benefits all windows immediately.
4. **Task Coordination & Path Mutual Exclusion**:
   - `ArchiveTaskCoordinator.shared`: Tracks active `TaskExecutionHandle` instances by `taskId: UUID`. Multiple windows can run extraction/compression tasks concurrently without overwriting each other's cancellation tokens.
   - `ArchivePathLockPool.shared` & `InPlaceMutationCoordinator.shared`: Enforces strict FIFO mutual exclusion when multiple windows attempt in-place mutations on the same canonical archive path. Upon completion, `TTZipArchiveUnlockedRefresh` carrying `archivePath` refreshes all windows currently inspecting that archive path.
5. **Shared Caches**:
   - `ArchiveHierarchySessionCache.shared`, `VFSLz4CachePool.shared`, and `PreviewLRUCacheManager.shared` remain shared and keyed by deterministic archive path / session identifiers.

---

## 4. Command, Shortcut & External Event Routing Matrix

### 4.1 Menu Bar & Keyboard Shortcuts
- **`⌘N` (New Window)**: Creates a new browser window via `BrowserWindowRouter.shared.openNewWindow(inheritingFrom: activeViewState)`. If an active browser window exists and is browsing a local directory, the new window inherits that starting directory (or defaults to `~/Downloads` if the active window is inside a special workspace).
- **`⇧⌘N` (New Archive...)**: Assigned to "New Archive..." (`L10n.Menu.newArchiveMenu`) to avoid conflict with `⌘N` (New Window) and system shortcuts (`⌘M` Minimize, `⌥⌘N`, etc.). Routes `.createArchive` to the currently focused browser window (or opens a new window with the compress workspace if 0 windows are open).
- **Focused Window Routing**:
  - `TTZipMenuCommands` resolves target `AppViewState` via `@FocusedValue(\.appViewState) ?? BrowserWindowDispatcherBridge.activeViewState`.
  - Commands (`⌘O` Open Archive, `⌘K` Omnibar, `⌘E` Extract, `⌥⌘E` Quick Extract All, `⌃⌘S` Toggle Sidebar, `⌘I` Toggle Inspector) execute **exclusively on the resolved active window**.
  - When 0 browser windows are open:
    - `⌘N` opens a new browser window.
    - `⇧⌘N` opens a new browser window directly in the New Archive (`compressWorkspace`) state.
    - `⌘O` prompts the open panel (or opens a new window and dispatches `.pickAndOpenArchive`).

### 4.2 External Open Requests (`application(_:open:)` / `onOpenURL`)
- **Broadcast Elimination**: `AppDelegate` no longer broadcasts opened URLs to a dictionary of `openURLHandlers`.
- **Deduplication**: `BrowserWindowRouter.shared` deduplicates identical external URL events arriving within a 150ms coalescing window (preventing double handling when macOS delivers both `NSApplicationDelegate.application(_:open:)` and SwiftUI `.onOpenURL`).
- **Routing Policy for 1..N External Files / URLs**:
  1. **Custom `ttzip://` Action Envelopes** (e.g. from FinderSync extension: extract, compress, inspect):
     - Routed once to `AppIntentDispatcher.shared.dispatch(envelope, targetWindowID: activeWindowID)`. If no window is open, a single window is created and seeded with the envelope.
  2. **File URLs (Archives or Folders opened from Finder / Dock)**:
     - For each incoming file URL in a batch `[url_0, url_1, ..., url_{k-1}]`:
       - **Already Open Check**: If an existing browser window already has `currentArchivePath == url.path` (for archives) or is actively browsing `url` without an archive open (for folders), that window is activated and brought to front (`makeKeyAndOrderFront`).
       - **Reusable Idle Window Check** (for the first unmatched URL in the batch): If the currently active window is "clean/idle" (on startup default directory, no archive loaded, not in compress/storage workspace, no active task or modal) and hasn't been claimed by an earlier URL in the same batch, reuse it.
       - **Dedicated New Window**: For any remaining URLs in a multi-file batch, or when the active window already has user browsing context/archive loaded, create a dedicated new window seeded with that URL.

### 4.3 Sidebar "Open in New Window"
- Invoked from `FinderFavoritesSidebarView`, `SidebarRowContextMenuModifier`, `SidebarLocationsSectionView`, `SidebarVolumeRowView`, or `SidebarColumnView` (via context menu, double-click on volume/inspector, or `⌘Return` / `⌥⌘O` / `⌥⌘↓`).
- Always calls `BrowserWindowRouter.shared.openNewWindow(for: url)`.
- **Invariant**: Creates **exactly one** new browser window navigated to `url` (or `ttzip://storage-inspector`). The originating window's directory, selection, archive state, and scroll position remain completely untouched.

### 4.4 Global Search Panel (`GlobalSearchPanelController` & `BrowserWindowRouter`)
- **Three Distinct Target Types (`GlobalSearchResultItem`)**:
  1. **Disk File / Folder (`.diskFile`, `diskPath`)**:
     - Routed via `BrowserWindowRouter.shared.routeGlobalSearchDiskItem(url:preferredWindowID:)`.
     - Window selection priority: (a) live `preferredWindowID`, else (b) existing window already browsing that parent directory without an open archive, else (c) `activeController`, else (d) `openNewWindow`.
     - Calls `revealItemInWindow(url)` on the target window.
  2. **Archive File (`.archiveFile` / `.recentArchive`, `archivePath`, `entrySubpath == nil`)**:
     - Routed via `BrowserWindowRouter.shared.routeGlobalSearchArchive(archivePath:preferredWindowID:)`.
     - Window selection priority: (a) live `preferredWindowID`, else (b) existing window whose `canonicalArchivePath(currentArchivePath) == canonicalArchivePath(archivePath)`, else (c) `activeController`, else (d) `openNewWindow`.
     - If the selected window already has `archivePath` loaded, focuses that window without closing or reloading the archive; otherwise calls `openArchiveInWindow(at: URL(fileURLWithPath: archivePath), password: nil, bookmarkSession: nil)`.
  3. **Archive Internal Entry (`.archiveEntry`, `archivePath` + `entrySubpath`)**:
     - Routed via `BrowserWindowRouter.shared.routeGlobalSearchArchiveEntry(archivePath:entrySubpath:preferredWindowID:highlightLine:)`.
     - **Window Selection & Closed-Window Fallback**:
       1. If `preferredWindowID` is non-nil and still alive in `BrowserWindowRegistry.shared`, select that window; if that window was closed before activation, automatically fall back to step 2.
       2. If an open browser window already has `canonicalArchivePath(currentArchivePath) == canonicalArchivePath(archivePath)`, select and focus that window.
       3. Otherwise select `BrowserWindowRegistry.shared.activeController`.
       4. If 0 browser windows are open, call `openNewWindow` with `initialArchiveURL`, `initialArchiveEntrySubpath`, and `initialHighlightLine`.
     - **In-Place Selection & Generational Async Load (`selectArchiveEntryInWindow`)**:
       - Never calls `closeCurrentArchive()` when activating an entry inside the currently loaded archive.
       - Matches `entrySubpath` by exact normalized full internal path (`normalizeArchiveEntrySubpath($0.path) == normalizeArchiveEntrySubpath(entrySubpath)`) and encodes `?subpath=` losslessly for special characters (`#`, `?`, `&`, `=`, `+`, `%`, spaces, CJK, brackets).
       - When the target archive is not yet loaded, awaits `loadArchive(path:)` under a monotonic `searchNavigationGeneration` token; if a newer search selection arrives or the window closes before `loadArchive` finishes, the stale completion is discarded.
- **Active Window Archive Session Search Source**:
  - `GlobalSearchPanelController.executeFastSearch(query:)` queries live open browser windows (`BrowserWindowRegistry.shared.allControllers`) directly via `state.explorerState.vfsSession` and `state.explorerState.currentEntries`, tagging hits with `targetWindowID: state.windowID`, and deduplicating against `ArchiveHierarchySessionCache.shared.allActiveSessions`.

### 4.5 Window-Scoped Notifications
- `TTZipEncryptedArchivePromptRequired`: Carries `userInfo["windowID"]` (or matches `viewModel.explorerState.currentArchivePath == path` / active window when `windowID` is omitted). Only the target window presents the password prompt.
- `.ttzipToggleMediaFocus` / `TTMediaToggleMediaFocusNotification`: Filtered by `userInfo["windowID"]` or whether the receiving window is the current key window / owns the media item URL. Background windows never pop up immersive media browsers when the user toggles media focus in the key window.
- `NSWindow.didExitFullScreenNotification` / `didEnterFullScreenNotification`: Filtered by matching `notification.object as? NSWindow` against the window's own `NSWindow` reference.

---

## 5. Window Close Semantics: Preview Resources, Pending Conflicts & Background Tasks

When a browser window `W` closes (`prepareForWindowClose()`):

1. **Pending Forensic Collision Prompt Continuation**:
   - If `W.overlayState.collisionPrompt` is non-nil, `W` immediately invokes `prompt.resolver(.stop, false)` and clears `overlayState.collisionPrompt = nil`.
   - This guarantees the underlying `CheckedContinuation<(ConflictResolutionAction, Bool)?, Never>` inside `CollisionPromptPresenter` is resumed with `.stop`, cleanly aborting the interactive preflight without leaking or trapping a suspended continuation.
2. **Preview Resources & Ephemeral Leases**:
   - If `W.overlayState.showImmersiveMediaBrowser` is true, `W.closeImmersiveMedia()` is called to cancel `mediaScanTask`, end fullscreen ephemeral handoff (`TTMediaDetector.endFullscreenEphemeralHandoff`), and release `immersiveMediaLeaseBox`.
   - In-archive search tasks (`explorerState` background tasks) are cancelled.
3. **Background Archive Tasks**:
   - Window-local UI progress sampling (`uiProgressTask`) is cancelled on deallocation, while non-interactive or already-running extraction/compression tasks in other windows continue uninterrupted.
   - If `W` had an interactive task blocked on a collision prompt, resolving `.stop` terminates that task cleanly; non-blocking background tasks initiated in `W` continue to completion unless explicitly cancelled by the user before closing, and `ArchiveTaskCoordinator.shared` unregisters the task handle via `defer` upon completion.
4. **Dock / Menu Reopen After Closing All Windows**:
   - Closing all browser windows leaves the app running (`NSApplication.ActivationPolicy.regular`).
   - Clicking the Dock icon (`applicationShouldHandleReopen(_:hasVisibleWindows: false)`) or pressing `⌘N` in the menu bar calls `BrowserWindowRouter.shared.openNewWindow()` to create a fresh browser window.

---

## 6. Relationship with Global Media Playback Policy

- **Single-Stream Audio/Video Invariant Preserved**: TTZip maintains a strict single-stream media playback policy via `MediaPlaybackCoordinator.shared` / `TTPreviewPlaybackManager`. At most one audio or video stream may play at any time across the entire application.
- **Cross-Window Non-Interference**:
  - Previously, `SelectionState.select(_:)` and `SelectionState.clear()` unconditionally called `TTPreviewPlaybackManager.stopAll()`. In a multi-window session, clicking a plain folder or clearing selection in Window B would unexpectedly kill audio/video playback running in Window A!
  - **New Ownership Rule**: `BrowserWindowRegistry.shared` tracks `playbackOwnerWindowID: UUID?`:
    1. When a window starts or changes media preview/immersive playback, it claims `playbackOwnerWindowID = windowID`.
    2. When `SelectionState.select(_:)` or `SelectionState.clear()` runs in a window `W`, `TTPreviewPlaybackManager.stopAll()` is invoked **only if `W.windowID == playbackOwnerWindowID`** (or if `playbackOwnerWindowID == nil` and `W` is the only window).
    3. When window `W` closes, `TTPreviewPlaybackManager.stopAll()` is invoked **only if `W.windowID == playbackOwnerWindowID`** (or if `W` is actively showing immersive media).
    4. If the user starts playing a media file in Window B while Window A is playing, `MediaPlaybackCoordinator.shared` automatically stops Window A's player and transfers `playbackOwnerWindowID` to Window B—never playing two audio streams simultaneously.

---

## 7. Acceptance Test Cases & Non-Goals

### 7.1 Acceptance Test Cases
1. **Triple `⌘N` Window Creation**: Pressing `⌘N` three times creates 3 distinct, registered browser windows (`windowID` 1..3) with zero duplicate or ghost windows.
2. **State Isolation Across 3 Windows**: Navigating Window 1..3 to different directories, performing back/forward history navigation, entering search queries, selecting items, opening archives, and toggling inspectors never mutates the other two windows.
3. **Sidebar "Open in New Window"**: Invoking "Open in New Window" from the sidebar creates exactly one new window at the target path while preserving the source window's directory, history, and selection.
4. **Focused Command Routing**: Menu commands (`⌘O`, `⇧⌘N`, `⌘K`, `⌘E`, `⌥⌘E`, `⌃⌘S`, `⌘I`) and context menu intents execute strictly on the focused (`activeWindowID`) window and never affect background windows.
5. **External File Open (Single & Batch)**: Opening 1 or multiple files/folders externally routes deterministically without broadcasting to multiple handlers; each request is processed once.
6. **Concurrent Per-Window Collision Prompts**: Two windows initiating interactive extractions with file collisions each display their own `ForensicCollisionPanel` bound to their respective `OverlayState` and resolve back to their own task continuation.
7. **Isolated Window Close**: Closing Window B leaves Window A's directory, archive session, active media preview, and running tasks completely intact, while safely resolving any pending collision continuation in Window B with `.stop`.
8. **Zero-Window Reopen**: After closing all browser windows, clicking the Dock icon (`applicationShouldHandleReopen`) or pressing `⌘N` opens a new browser window cleanly.
9. **Rapid Stress Lifecycle**: Rapidly opening, switching focus among, and closing multiple windows leaves zero dangling sessions in `BrowserWindowRegistry` or `AppIntentDispatcher` and zero unresumed continuations.
10. **i18n Menu & Shortcut Consistency**: Switching between `zh-Hans`, `zh-Hant`, and `en` dynamically updates "New Window (`⌘N`)" and "New Archive... (`⇧⌘N`)" menu titles and tooltips without recreating window state.

### 7.2 Explicit Non-Goals (Out of Scope for W1)
- Cross-window tab drag-and-drop merging (`NSWindow.tabbingMode` remains `.disallowed`).
- Cross-window synchronized scrolling or dual-pane linked navigation.
- Multi-window crash state persistence / full window geometry session restoration across app relaunches.
- Simultaneous multi-stream audio mixing across windows.

---

## 8. Directory Navigation Responsiveness & Cancellation Contract (Sidebar & Miller Columns)

### 8.1 End-to-End Stage Pipeline & Latency Budget
Every folder navigation triggered from the sidebar (`SidebarFavoriteRowView`, `SidebarLocationsSectionView`, `SidebarVolumeRowView`) or Miller Columns (`FinderMillerColumnsView`) is decomposed into 7 observable stages:

| Stage | Description | Target Latency Budget | Execution Actor / Context |
| :--- | :--- | :--- | :--- |
| **1. Mouse Event Arrival** | User clicks sidebar folder row or column item | `0 ms` | AppKit main runloop |
| **2. Selection Callback** | Gesture fires `onSelect()` without waiting for double-click timeout | `< 5 ms` | `@MainActor` (immediate single-click dispatch) |
| **3. Navigation State Update** | `navigationState.navigate(to:)` updates `currentDirectory`, history, and column root | `< 10 ms` (visual selection & column header feedback `< 100 ms` total) | `@MainActor` |
| **4. Permission & Lease Check** | Fast path check via `RootFolderAccessManager` without redundant bookmark/symlink round-trips | `< 5 ms` (cached/authorized) | `@MainActor` fast check + scoped lease |
| **5. Directory Scan** | `getattrlistbulk` (Rust `TTZipCore.scanDirectory`) or virtual archive VFS enumeration | `< 15 ms` (small dir, ~50 items), `< 80 ms` (large dir, ~2,000 items) | Concurrent background task (off `@MainActor`, non-serialized across independent paths) |
| **6. Model Transform & Sort** | Map `DiskItemSummary` to `DiskItemInfo` + sort according to `DiskSortOption` | `< 15 ms` (skip redundant sort when `.nameAsc` already sorted by Rust; otherwise sort off `@MainActor`) | Background cooperative executor |
| **7. First-Screen Render** | Commit `[DiskItemInfo]` to `cachedColumnItems` and render visible `LazyVStack` rows | `< 20 ms` | `@MainActor` |

### 8.2 Diagnosed Root Causes of the ~1-Second Delay (Pre-Fix Baseline)
1. **Single-Click vs. Double-Click Gesture Competition (~250–500 ms delay)**:
   - `SidebarFavoriteRowView`, `SidebarVolumeRowView`, and `SidebarStorageInspectorRowView` attached `.simultaneousGesture(TapGesture(count: 2))` alongside single-tap `.onTapGesture` / `Button`.
   - SwiftUI's gesture arena waited for `NSEvent.doubleClickInterval` (~300–500 ms on macOS) to expire before invoking `onSelect()`.
2. **Main-Thread Synchronous Filesystem Probes & Double-Render Hop**:
   - `MillerColumnDirectoryScanner.loadContentsOf(dirURL:)` executed `FileManager.default.fileExists` and `attributesOfItem` synchronously on `@MainActor` before hopping to `DiskDirectoryScanner`.
   - `FinderMillerColumnsView.onChange(of: rootDirectory)` applied an ancestor-chain heuristic (rule #3) meant only for `navigateUp()`, which unintentionally built multi-column chains when clicking a parent folder (e.g., Home) from a deep subfolder in the sidebar.
3. **Actor Serialization, Redundant Bookmark Resolution & Main-Thread Re-Sorting**:
   - `DiskDirectoryScanner.shared` was a single global serial `actor` that ran `TTZipCore.scanDirectory` synchronously inside the actor executor; a slow scan on directory `A` blocked subsequent scans on `B` and `C` even across different windows.
   - `DiskDirectoryScanner.scanDirectory` hopped back to `@MainActor` three times (`canonicalDirectoryURL`, `ensureAccess`, `acquireLease`), resolving symlinks and security-scoped bookmarks twice per scan.
   - `FinderMillerColumnsView` re-sorted the entire `[DiskItemInfo]` array via `DiskItemSorter.sort(scanned, by: sortOpt)` **synchronously on `@MainActor`**, even when `sortOpt == .nameAsc` was already natural-sorted by `TTZipCore.scanDirectory`!
4. **Right Inspector Uncancelled Detached Enumeration**:
   - Switching directories cleared selection, causing `RightInspectorSidePanel` to instantiate `FolderMediaArtboardView`, which launched an unstructured `Task.detached` running a recursive `FileManager.enumerator` up to depth 3 that ignored SwiftUI `.task(id:)` cancellation and contended for disk I/O.

### 8.3 Immediate Selection & Gesture Contract
- Clicking a sidebar row (`SidebarFavoriteRowView`, `SidebarVolumeRowView`, `SidebarStorageInspectorRowView`) MUST fire `onSelect()` immediately on the first click (`< 5 ms`) without waiting for the double-click disambiguation window.
- Double-clicking a sidebar favorite or volume row is detected via monotonic timestamp delta (`ProcessInfo.processInfo.systemUptime` within `NSEvent.doubleClickInterval`) on the second click to invoke `openInNewWindow`, while the context menu ("Open in New Window") remains the primary explicit multi-window trigger.
- Directory list rendering MUST NOT wait for right-hand inspector recursive folder sizing, thumbnails, or media previews.

### 8.4 Generational Cancellation & `A -> B -> C` Convergence Contract
- Each `FinderMillerColumnsView` instance (per window) maintains a monotonic `navigationGeneration: UInt64` per column slot.
- Rapidly clicking `A -> B -> C` cancels the in-flight tasks for `A` and `B` immediately via Swift structured concurrency (`Task.isCancelled`).
- `MillerColumnDirectoryScanner` and `DiskDirectoryScanner` check `Task.isCancelled` before and after filesystem scan and sort phases:
  - Cancelled tasks abort before sorting or mutating UI state.
  - Before writing `cachedColumnItems[cacheKey]` on `@MainActor`, the view verifies `!Task.isCancelled` and that `dirURL` is still present in the window's active `columnPaths`. Stale results from `A` or `B` NEVER overwrite `C`.
- `DiskDirectoryScanner` only holds actor isolation for lightweight metadata/cache/generation bookkeeping; the blocking `TTZipCore.scanDirectory` syscall and background sorting run outside the actor lock so Window 1 and Window 2 (or rapid `A -> B -> C` transitions) never queue behind a stale scan.

### 8.5 Permission, Error & Cache Invalidation Contract
- **No Permission Bypass**: `RootFolderAccessManager.shared.ensureAccess` and `acquireLease` remain mandatory for every physical directory scan.
- **Explicit Access-Required & Read-Failure States**:
  - If `ensureAccess` returns `false`, `MillerColumnDirectoryScanner` returns the explicit `[DiskItemInfo]` authorization lock sentinel (`kindText == "Access Authorization"`), causing `SingleMillerColumnView` to render `FolderAccessRequiredView` rather than an empty folder.
  - If `TTZipCore.scanDirectory` fails due to permission denial (`EACCES` / `EPERM`) or I/O failure on an external/unpermitted volume, `DiskDirectoryScanner` surfaces an explicit access/read-failure sentinel rather than caching or displaying a false "Empty Folder" success state.
- **Cache Correctness**:
  - `DiskDirectoryScanner` caches raw name-sorted `[DiskItemInfo]` keyed by canonical directory path together with the directory's modification timestamp (`mtime`) and a TTL (`2.0s`).
  - Manual refresh (`onRefresh`), file creation/deletion/rename notifications (`TTZipDirectoryRefreshNotification`), or permission grant changes explicitly invalidate the affected path (or all cached paths on manual refresh) so stale entries are never served.

---

## 9. Archive Loading Authorization Recovery & Invalidation Invariants

### 9.1 Epoch Establishment Before First Async Suspension
- When `loadArchive(path:password:bookmarkSession:)` is called, `self.operationEpoch &+= 1` must execute synchronously on `@MainActor` as the very first statement before any asynchronous suspension point (`await RootFolderAccessManager.shared.requestRootAccess`).
- The local generation `let currentGen = self.operationEpoch` binds the lifetime of this specific archive load operation.

### 9.2 Immediate Post-Suspension Invalidation Check
- Immediately after resuming from `requestRootAccess` and after `inspectArchiveWithEvidence`:
  ```swift
  guard !Task.isCancelled else { return false }
  guard self.operationEpoch == currentGen else { return false }
  guard !self.isWindowClosing else { return false }
  ```
- If a newer operation arrived (advancing `operationEpoch`), the window closed, or `closeCurrentArchive()` was called during the prompt wait, the suspended operation aborts immediately and returns `false`.

### 9.3 State Mutation Isolation
- Stale or superseded load operations never mutate `explorerState.currentArchivePath`, `explorerState.currentEntries`, `explorerState.activePassword`, `navigationState`, or `selectedDiskItem`.
- Stale operations never register or synchronize archive sessions via `AppIntentDispatcher.shared`.
- All temporary bookmark sessions and directory leases are guaranteed to release via `defer`.


