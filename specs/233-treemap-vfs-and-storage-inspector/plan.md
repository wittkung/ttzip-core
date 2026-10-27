# Implementation Plan: 233 - Treemap VFS & Storage Space Inspector

> **Target Version**: TTWorkspace v6.2  
> **Status**: Accepted Implementation Plan  

---

## 1. Architectural Strategy & Phased Execution

To maintain zero-regression in production archive exploration while introducing high-performance space visualization, execution follows three sequential, test-driven phases:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ Phase 1: Rust Core Treemap Layout Engine                                   │
│ - Bruls Squarified Layout algorithm operating over CompactNode              │
│ - Tail merging & Others tile aggregation (Area < 4px², Max 2048 tiles)      │
│ - Unit tests against 100k synthetic node trees (< 3.0ms target)            │
├─────────────────────────────────────────────────────────────────────────────┤
│ Phase 2: Darwin Bulk Scanner & Inode Deduplication                         │
│ - Darwin getattrlistbulk(2) wrapper with thread-local 64KB stack buffers    │
│ - Seen 64M atomic bitset & 64-shard hash table                             │
│ - Reclaim category classifier (target/, node_modules/, DerivedData)        │
│ - Cloud dataless file prevention (setiopolicy_np)                          │
├─────────────────────────────────────────────────────────────────────────────┤
│ Phase 3: UniFFI & C-ABI Surface Synchronization                            │
│ - Export TreemapTileDto & compute_treemap_layout via UniFFI                │
│ - Export start_storage_scan & StorageScanCallback                          │
│ - Swift 6 type generation and async stream bridge                          │
├─────────────────────────────────────────────────────────────────────────────┤
│ Phase 4: Swift UI Track 1 (Archive-Internal Treemap View)                  │
│ - MainView+Toolbar: segmented control update [Column | List | Treemap (⌘3)]│
│ - TTZipTreemapCanvasView: SwiftUI Canvas / Metal quad instancing           │
│ - Zoom-in, drill-down breadcrumb push, QuickLook integration               │
├─────────────────────────────────────────────────────────────────────────────┤
│ Phase 5: Swift UI Track 2 (Dedicated Storage Space Inspector Page)         │
│ - SidebarLocationsSectionView: "Storage Inspector" navigation item         │
│ - StorageInspectorWorkspaceView: Disk volume selection, scan trigger       │
│ - Reclaimable space display, system Trash integration via NSFileManager    │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. File Modification & Creation Inventory

| File Path | Action | Responsibilities | Target LOC |
| :--- | :--- | :--- | :--- |
| `products/ttzip/app-core/src/workspace/types.rs` | Edit | Add `TabViewMode::Treemap` and `TreemapTile` struct | $+45$ |
| `products/ttzip/app-core/src/workspace/treemap.rs` | New | Pure Rust Bruls squarified treemap layout engine | $\le 450$ |
| `products/ttzip/app-core/src/workspace/scanner/darwin.rs` | New | Darwin `getattrlistbulk(2)` batch directory scanner | $\le 400$ |
| `products/ttzip/app-core/src/workspace/scanner/seen.rs` | New | Atomic 64M bitset + 64-shard Inode deduplicator | $\le 200$ |
| `products/ttzip/app-core/src/workspace/scanner/classify.rs`| New | Developer cache and redundant artifact classifier | $\le 180$ |
| `products/ttzip/app-core/src/workspace/scanner/mod.rs` | New | Scanner lifecycle & thread pool orchestration | $\le 150$ |
| `products/ttzip/app-core/src/uniffi_api/treemap.rs` | New | UniFFI export bridge for treemap and scanner | $\le 180$ |
| `products/ttzip/apple/Sources/TTZipUI/Views/Treemap/TTZipTreemapCanvasView.swift` | New | 120Hz SwiftUI Canvas renderer for treemap tiles | $\le 400$ |
| `products/ttzip/apple/Sources/TTZipApp/Views/StorageInspector/StorageInspectorView.swift` | New | Dedicated Storage Inspector workspace page | $\le 450$ |
| `products/ttzip/apple/Sources/TTZipApp/Views/MainView+Toolbar.swift` | Edit | Add Treemap segment and `⌘3` keyboard shortcut | $+35$ |
| `products/ttzip/apple/Sources/TTZipApp/Views/Sidebar/SidebarLocationsSectionView.swift`| Edit | Host Storage Inspector sidebar destination | $+25$ |

---

## 3. Risk Mitigation & Defenses

1. **Sandboxing Invariant**:
   - MAS target enforces `com.apple.security.app-sandbox`. In MAS builds, scanning the root path `/` is disabled; only user-selected directories and standard locations (`Downloads`, `Documents`, `Desktop`, external mounted volumes) are selectable.
2. **Infinite Recursion & Symlink Loops**:
   - `Seen` deduplicator marks directory Inodes as visited. Symlink targets pointing upward are detected and pruned.
3. **Out-of-Memory Defense**:
   - Dynamic down-sampling via Tail Merging guarantees total tiles $\le 2,048$.
