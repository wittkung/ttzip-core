# Requirements Checklist: Feature 233 - Treemap VFS & Storage Space Inspector

- [x] R1. **Core Data Structure Invariant**: The Treemap tile layout engine MUST operate directly over TTZip's existing 24-byte `CompactNode` and contiguous `StringArena` without allocating separate 88-byte tree nodes.
- [x] R2. **Bruls Squarified Layout**: The layout algorithm MUST implement the Bruls, Huizing & van Wijk squarified algorithm to minimize tile aspect ratio $\max(w/h, h/w)$.
- [x] R3. **Tail Merging Invariant**: Nodes with computed screen area $< 4.0\text{ px}^2$ or relative container weight $< 0.05\%$ MUST be aggregated into a single `Others` tile (`node_id = 0xFFFFFFFF`) to cap visible tiles at $\le 2,048$ per screen.
- [x] R4. **Dual-Track Integration**:
  - Track 1 (Archive-Internal Treemap): Switchable via `⌘3` in `ArchiveExplorerTableView` toolbar, operating 100% in-memory without extra disk I/O.
  - Track 2 (Storage Space Inspector): Dedicated sidebar destination under `Locations`, scanning user-selected directories and mounted storage volumes.
- [x] R5. **Darwin Bulk Scanner**: Local directory scanning MUST invoke `getattrlistbulk(2)` with thread-local 64KB stack buffers, bypassing per-file `lstat` syscall overhead.
- [x] R6. **Seen Inode Deduplication**: Hardlink and shared extent deduplication MUST utilize an 8MB atomic bitset ($2^{26}$ bits) for IDs $< 64\text{M}$, with a 64-shard hash table fallback for overflow.
- [x] R7. **Cloud-Storage Eviction Defense**: Scanning threads MUST enforce `setiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, IOPOL_SCOPE_PROCESS, IOPOL_MATERIALIZE_DATALESS_FILES_OFF)` to prevent automatic iCloud / OneDrive downloads.
- [x] R8. **App Sandbox Compliance**: The Mac App Store build MUST restrict storage inspection to user-selected folders, standard sandbox locations, and mounted external volumes. Full disk root access (`/`) MUST be restricted to the Direct distribution channel with explicit TCC Full Disk Access prompts.
- [x] R9. **Forensic Trash Safety**: File cleanup operations MUST route through native macOS `NSFileManager.trashItem` (System Trash) and enforce strict path blacklists preventing modification of system directories (`/System`, `/usr`, `/Applications`, `~/Library/Keychains`).
- [x] R10. **High-DPI Presentation Geometry**: Treemap rendering MUST execute on 120Hz ProMotion displays via native SwiftUI `Canvas` or Metal quad instancing, adhering to the TTKitDesign 4-layer optical model.
