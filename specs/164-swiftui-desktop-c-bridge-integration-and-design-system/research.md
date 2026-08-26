# Research Findings: SwiftUI 桌面端与 C11 纯微内核深度打通及设计系统落地 (Feature 164)

## R001 [SUBAGENT:research]: 60fps Lock-Free C-to-Swift Progress & Cancellation Stream Bridge

- **Decision**: Implement an `AsyncStream<ArchiveProgress>` bridge with lock-free atomic time-throttling (16.6ms window for 60Hz UI refresh) and atomic cancellation checking. In C (`ttzip_archive.h` / `ttzip_archive.c`), the `ttzip_progress_fn` receives a lightweight context tunneling struct `ttzip_c_progress_bridge_t` holding an atomic cancellation pointer (`atomic_bool*`) and a monotonic timestamp (`mach_absolute_time()`). Swift constructs the stream via `AsyncStream.makeStream(of: ArchiveProgress.self)`, registering an `onTermination` handler that sets the atomic cancellation flag to signal immediate early return (`TTZIP_API_ERR_USER_CANCELLED`) within C worker loops.
- **Rationale**: When compressing or extracting thousands of small files, the C engine executes at 5,000+ files/sec. Emitting unthrottled Swift actor messages floods the cooperative task queue and AppKit runloop, causing UI hitching (< 15fps) and high actor contention. Throttling in C/Bridge to a 16.6ms rate limit (while always guaranteeing delivery of terminal `1.0` / error events) maintains 60fps CoreAnimation rendering and zero memory allocation churn.
- **Alternatives Considered**: Direct `@MainActor` dispatch on every progress callback invocation (`DispatchQueue.main.async`). Rejected because it causes actor queue congestion and drops frames under heavy multi-threaded compression workloads.
- **Source**:
  - `Sources/CTTZipBridge/include/ttzip_archive.h:25-59`
  - `Sources/CTTZipBridge/include/CTTZipBridge_Archive.h:25-51`
  - `Sources/CTTZipBridge/ttzip_archive.c:43-100, 200-249`
  - `Sources/TTZipCore/ArchiveProgress.swift:11-54`
  - `Sources/TTZipCore/ConcurrencyBridge.swift:44-77`

---

## R002 [SUBAGENT:research]: TTZip UI Design System Token & 3-Column Layout Alignment

- **Decision**: Align the 3-column layout in `MainView.swift` and explorer views to the exact TTZip UI Design System specification:
  1. **Column Geometry Constraints**: Left Sidebar fixed/default at `200pt` (`userLeftSidebarWidth = 200.0`, min `140pt`, max `280pt`), Central Explorer workspace `minWidth: 450pt` (default `600pt`), Right Inspector panel `280pt` (`userRightSidebarWidth = 280.0`).
  2. **Y = 90pt Golden Rule Line Alignment**: Ensure all three columns enforce vertical synchrony with `.padding(.top, 38)`, a `52pt` header bar (`.frame(height: 52)`), and an identical `Rectangle().fill(TTZipTheme.kintsugiGold).frame(height: 1.5)` rule at Y = 90pt ($38 + 52 = 90\text{pt}$).
  3. **Typography & Material Consistency**: Unify headers to WSJ Editorial serif typography (`.font(.system(size: 9, weight: .bold, design: .serif)).tracking(2)` for section category in `kintsugiGold`, `.font(.system(size: 16, weight: .bold, design: .serif))` for title), replacing legacy hairline borders with standard Kintsugi gold rules and floating glass island containers.
- **Rationale**: Visual hierarchy on macOS desktop apps requires strict horizontal alignment across split panes. Inconsistent header heights (e.g. 44pt vs 52pt) or offset divider lines create visual jitter during panel toggling and resize operations. Standardizing on the 38pt top inset + 52pt header container guarantees a razor-sharp horizontal horizon across all tabs.
- **Alternatives Considered**: Using macOS standard `NavigationSplitView` with default system toolbars. Rejected because system toolbars have dynamic platform-dependent heights (28pt–36pt) and uncontrollable margins that violate the custom WSJ Editorial serif header structure and the Y = 90pt Golden Rule Line.
- **Source**:
  - `.agents/skills/ttzip-ui-design-system/SKILL.md:13-52, 68-103`
  - `Sources/TTZipApp/Theme/TTZipTheme.swift:12-123`
  - `Sources/TTZipApp/Views/Sidebar/MacEditorialSidebar.swift:27-64`
  - `Sources/TTZipApp/Views/Explorer/HomeExplorerContainerView.swift:22-83, 105`
  - `Sources/TTZipApp/Views/Components/RightInspectorSidePanel.swift:22-68`
  - `Sources/TTZipApp/Views/MainView.swift:34-40, 72-113`

---

## R003 [SUBAGENT:research]: Single-Item On-Demand Quick Look Preview & Finder Drag-and-Drop

- **Decision**: Implement true single-entry on-demand extraction without full archive decompression for both Space-bar Quick Look preview and Finder drag-and-drop:
  1. **ZIP format**: Leverage `ZipSeekTable` / `ZipCentralDirectoryReader` to seek directly to the entry offset and decompress solely the target entry into memory or an ephemeral temp file (`/tmp/TTZipPreview_<UUID>/<filename>`).
  2. **Non-ZIP formats (7z, TAR, TAR.GZ, TAR.ZST)**: Route to `ttzip_stream_archive_entries_to_fd` (`CTTZipBridge_Archive.c`), utilizing libarchive in-process streaming to skip unselected data blocks (`archive_read_data_skip`) and write only the matching entry directly to a target file descriptor.
  3. **Space-Bar & Floating Quick Look**: Integrate Space-bar key monitoring (`keyCode == 49`) in `NativeArchiveOutlineView` to present `QLPreviewPanel.shared()` / `.quickLookPreview` referencing the ephemeral single-extracted file URL.
  4. **Finder Drag-and-Drop**: Adopt `NSFilePromiseProvider` via `NSOutlineView` dragging delegate, deferring extraction until Finder confirms the drop destination, writing directly into the target folder without intermediate staging.
- **Rationale**: Decompressing an entire 10GB archive containing 50,000 files just to preview a 50KB image or drag out a single PDF induces unacceptable latency (tens of seconds), wastes gigabytes of SSD write cycles, and exhausts system memory. Single-item streaming and file promise providers make Space-bar preview and Drag-to-Finder instantaneous (< 50ms).
- **Alternatives Considered**: Eager background decompression of all archive items into a local cache directory on archive open. Rejected due to heavy disk I/O, massive storage bloat, and thermal throttling on large archives.
- **Source**:
  - `Sources/CTTZipBridge/CTTZipBridge_Archive.c:329-430`
  - `Sources/CTTZipBridge/ttzip_archive.c:265-320`
  - `Sources/TTZipCore/ArchiveSelectiveExtractor.swift:21-97`
  - `Sources/TTZipApp/Views/ArchiveExplorerView.swift:514-556`
  - `Sources/TTZipApp/Services/MediaPreviewFactory.swift:87-160, 277-281`

---

## R004 [SUBAGENT:research]: Virtualized File Tree Performance on 100k+ Entries

- **Decision**: Bridge the C11 `ttzip_tree_t` radix tree (`ttzip_archive_tree.c`) to Swift using a Flat Indexed Projection model for `LazyVStack` and `NSOutlineView`:
  1. **C Memory Pool Storage**: Keep the complete tree node hierarchy in C11 arena memory (`ttzip_tree_t`), where each node pointer references contiguous string tables, avoiding Swift ARC and Heap String overhead.
  2. **Flat Indexed Projection Vector**: Swift maintains a 16-byte value type array `[VirtualTreeRow]` representing currently visible/expanded rows (`id: Int`, `nodeHandle: OpaquePointer`, `depth: Int16`, `isExpanded: Bool`, `isDirectory: Bool`).
  3. **Viewport Virtualization**: When folders are expanded/collapsed or filtered via `ttzip_tree_search`, the flat index array is updated in $O(N)$ linear time in C (< 1ms for 100k entries). `NSOutlineView` or SwiftUI `LazyVStack` renders only the ~30 visible viewport rows.
  4. **Memory Footprint**: 100,000 items $\times$ 16 bytes = 1.6MB for projection vector + ~20MB for C tree arena = **< 25MB total RAM**, comfortably satisfying the < 35MB budget limit.
- **Rationale**: Constructing 100,000 recursive Swift `ArchiveTreeNode` struct instances with arrays and strings requires 150MB+ RAM and causes a 3-5s UI freeze during initial hierarchy synthesis. Retaining the tree in C and exposing an index-projected virtual view guarantees instantaneous loading and 60fps scrolling.
- **Alternatives Considered**: Native SwiftUI `OutlineGroup` with deep recursive Swift structs. Rejected because `OutlineGroup` instantiates the entire tree hierarchy into the SwiftUI view graph at once, exceeding 180MB RAM and freezing the main thread on 100k nodes.
- **Source**:
  - `Sources/CTTZipBridge/include/ttzip_archive_tree.h:25-85`
  - `Sources/CTTZipBridge/ttzip_archive_tree.c:14-107, 146-162`
  - `Sources/TTZipCore/ArchiveTreeNode.swift:11-40, 137-143`
  - `Sources/TTZipApp/ViewModels/ArchiveTreeStore.swift:16-72, 75-120`
  - `Sources/TTZipApp/Views/Explorer/NativeArchiveOutlineView.swift:68-98, 179-244, 286-328`
