# Feature Specification: 233 - Treemap VFS & Storage Space Inspector

> **Target Version**: TTWorkspace v6.2 (Treemap & Storage Inspector Edition)  
> **Status**: Accepted Architecture Specification  
> **Classification**: Core Subsystem & Presentation Extension  

---

## 1. System Overview & Problem Statement

### 1.1 Problem Statement
1. **Archive Space Opacity**: Modern archives (ZIP, 7Z, TAR, RAR) often exceed tens of gigabytes with deeply nested directory structures. Standard list views (`NSTableView`) require users to expand directories manually to gauge size distributions, introducing cognitive friction when locating disk-wasting files.
2. **Mac Storage Friction**: Developers and power users frequently accumulate gigabytes of build artifacts (`target/`, `node_modules/`, `DerivedData`) and download caches. Existing disk sweeper utilities (e.g., CleanMyMac, DaisyDisk) are proprietary closed ecosystems that run invasive background daemons.
3. **Architecture Impedance**: While tools like `tobi/disktree` demonstrate the efficacy of squarified treemaps, their 88-byte node representation and Linux GPUI rendering model cannot be imported directly into TTZip's 24-byte `CompactNode` and native Swift/AppKit presentation architecture.

### 1.2 Objective & System Boundary
This specification defines the architecture, data models, and presentation contracts to incorporate:
- **Track 1 (Archive-Internal Treemap)**: Instant zero-disk-I/O visual treemap projection of opened archives within `ArchiveExplorerTableView`.
- **Track 2 (Storage Space Inspector)**: Dedicated workspace page scanning local directories, mounted drives, and developer caches using Darwin `getattrlistbulk(2)` and 64M atomic deduplication.

---

## 2. User Stories & Acceptance Criteria

### User Story 1: Visualizing Large Archive Space Distribution (Track 1)
- **As a** developer opening a 25GB uncompressed TAR/ZIP archive,
- **I want to** switch to a squarified treemap view using `⌘3` or the toolbar toggle,
- **So that** I immediately identify which directory or file consumes the majority of the uncompressed bytes without manual sorting.

#### Acceptance Criteria (Given / When / Then)
1. **Given** an open archive with 50,000 files in TTZip,
   **When** the user clicks the Treemap view button (`⌘3`),
   **Then** the treemap tiles MUST render on screen in $\le 5.0\text{ ms}$, with zero bytes written to disk.
2. **Given** an active treemap viewport,
   **When** the user hovers over a tile,
   **Then** the tile MUST respond with subtle spring physics ($\zeta = 0.82$, scale 1.015), and the right Inspector panel MUST display file path, uncompressed size, compression ratio, and CRC32.
3. **Given** an active treemap viewport,
   **When** the user double-clicks or zooms into a directory tile that occupies $> 80\%$ of the screen,
   **Then** the viewport MUST smoothly transition inward, pushing a new breadcrumb crumb to the top path bar.

### User Story 2: Inspecting and Cleaning Local Developer Caches (Track 2)
- **As a** macOS engineer managing low disk space,
- **I want to** select my `Projects` or `Downloads` folder in the Storage Inspector,
- **So that** I view an interactive treemap highlighting reclaimable caches (`target/`, `node_modules/`, `DerivedData`) and safely move them to Trash.

#### Acceptance Criteria (Given / When / Then)
1. **Given** a directory containing 100,000 files across multiple git repos,
   **When** scanning begins via `getattrlistbulk(2)`,
   **Then** the scanner MUST achieve a scanning throughput $\ge 25,000\text{ items/sec}$, using thread-local 64KB stack buffers without per-file heap allocation.
2. **Given** identified reclaimable directories,
   **When** rendered in the Storage Inspector,
   **Then** reclaimable tiles MUST be visually distinguished using a Display P3 amber diagonal hatch pattern.
3. **Given** a user triggers deletion on a marked tile,
   **When** the confirmation modal is accepted,
   **Then** the operation MUST invoke `NSFileManager.trashItem` (system Trash), verify destination volume boundaries, and reject system paths (`/System`, `/usr`, `/Applications`, `~/Library/Keychains`).

---

## 3. Functional Requirements

### 3.1 Data Plane & Tile Geometry
- **FR-01 (Tile Format)**: The Rust engine MUST output a flat, contiguous array of 32-byte `TreemapTile` structures aligned to 8 bytes, strictly in logical points (pixels).
- **FR-02 (Bruls Squarified Layout)**: The layout engine MUST evaluate row aspect ratios dynamically using zero-heap-allocation scalar `RowState (sum, min, max)` arithmetic in $O(1)$.
- **FR-03 (Tail Merging & Pruning)**: Children with computed short edge $\min(w, h) < 6.0\text{ pt}$, area $< 36.0\text{ pt}^2$, or container ratio $< 0.1\%$ MUST be folded into an `Others` tile (`node_id = 0xFFFFFFFF`). Total visible tiles per viewport MUST NOT exceed 256.

### 3.2 Scanning & Deduplication
- **FR-04 (Darwin Bulk Enumeration & Physical Footprint)**: Directory enumeration on macOS MUST use `getattrlistbulk(2)` requesting `ATTR_CMN_NAME`, `ATTR_CMN_OBJTYPE`, `ATTR_CMN_MODTIME`, `ATTR_CMN_FLAGS`, `ATTR_FILE_ALLOCSIZE` (physical bytes), and `ATTR_CMNEXT_REALDEVID`. Scanner MUST detect `SF_FIRMLINK` (`0x00800000`) and enforce single-filesystem boundaries (`xdev` rule).
- **FR-05 (Device-Isolated Seen Bitset)**: Hardlink deduplication MUST use an atomic bitset of $2^{26}$ bits (8MB memory) for `inode < 67,108,864` on the primary device, falling back to 64-shard `FxHashSet<(u64, u64)>` for cross-device or high-range Inodes.
- **FR-06 (Dataless & Mount Defense)**: Scanner threads MUST call `setiopolicy_np` with `IOPOL_MATERIALIZE_DATALESS_FILES_OFF`, `IOPOL_ATIME_UPDATES_OFF`, and `IOPOL_VFS_TRIGGER_RESOLVE_OFF`.

### 3.3 Presentation & Navigation Layer
- **FR-07 (Toolbar Segmented Control)**: Toolbar MUST expose `[ Column (⌘1) | List (⌘2) | Treemap (⌘3) ]` as pure visual projections of the current directory, without replacing the top-level workspace.
- **FR-08 (First-Class Workspace Page)**: Storage Inspector MUST be modeled as a first-class `DirectoryTabItem` workspace page. Sidebar single-click navigates in-place / reuses existing tab; ⌘-click spawns a new tab; closing the sole tab smoothly falls back to `~/Downloads`. Safe URL resolver MUST preserve `ttzip://` scheme.
- **FR-09 (120Hz ProMotion Pipeline)**: Treemap canvas rendering MUST batch draw calls into 10 `TileCategory` paths, cull CoreText layout for tiles $< 50\times 30\text{ pt}$, and use `SpatialTapGesture` to eliminate the 300ms gesture delay.
- **FR-10 (Anchor-Validated Cache Safety)**: Reclaim classifier MUST protect Xcode `Archives` (`~/Library/Developer/Xcode/Archives`), require build script anchors (`package.json`, `Cargo.toml`) for `build`/`out`/`dist`, and resolve symlinks in `ForensicPathGuard`.

---

## 4. Non-Functional & SLO Requirements

| Metric ID | Description | Target | Verification Method |
| :--- | :--- | :--- | :--- |
| `SLO-TREEMAP-LAYOUT-P99` | 50,000-node treemap layout compute latency | $\le 2.0\text{ ms}$ | Criterion microbench |
| `SLO-TREEMAP-TILES-MAX` | Maximum tiles emitted per layout pass | $\le 256$ | Static assertion in layout engine |
| `SLO-SCAN-BULK-RATE` | Local SSD directory scanning throughput | $\ge 25,000\text{ items/s}$ | Real-drive NVMe benchmark |
| `SLO-TREEMAP-MEM-RSS` | Transient heap overhead for treemap layout | $\le 1.0\text{ MB}$ (0 allocs in hot loop) | Instruments Allocations |
| `SLO-FRAME-RENDER-P99` | UI Canvas render time during pan/zoom | $\le 8.33\text{ ms}$ (120Hz) | Metal System Trace |

---

## 5. Security & Boundary Governance

1. **Mac App Store (MAS) Sandboxing**:
   - In MAS builds, Storage Inspector is restricted to directories explicitly picked by the user (`NSOpenPanel`) and persistent bookmarks (`com.apple.security.files.bookmarks.app-scope`).
   - Root scanning (`/`) is strictly prohibited in the MAS target.
2. **Direct Distribution Target**:
   - In Direct distribution (`#if DIRECT_DISTRIBUTION`), Full Disk Access is prompted gracefully via `access.rs` TCC detection.
3. **Execution Safety**:
   - File deletion MUST NEVER execute `rm -rf` directly. All deletions MUST route through macOS `NSFileManager.default.trashItem(at:resultingItemURL:)`.
