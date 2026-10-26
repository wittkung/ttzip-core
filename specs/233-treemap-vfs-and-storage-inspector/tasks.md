# Task Breakdown: 233 - Treemap VFS & Storage Space Inspector

> **Target Version**: TTWorkspace v6.2  
> **Status**: Ready for Implementation  

---

## Task Dependency Graph

```
[T1.1: Core Types & Layout Structs]
       │
       ▼
[T1.2: Bruls Layout Engine & Unit Tests]
       │
       ├──────────────────────────────────────────┐
       ▼                                          ▼
[T2.1: Seen Atomic Inode Deduplicator]    [T3.1: UniFFI Treemap Exports]
       │                                          │
       ▼                                          ▼
[T2.2: Darwin Bulk Scanner & Tests]       [T4.1: SwiftUI Treemap Canvas]
       │                                          │
       ▼                                          ▼
[T2.3: Reclaim Cache Classifier]          [T4.2: ArchiveExplorer Toolbar ⌘3]
       │                                          │
       └───────────────────┬──────────────────────┘
                           ▼
              [T5.1: Storage Inspector View]
                           │
                           ▼
              [T5.2: Safe Trash Execution Net]
                           │
                           ▼
              [T6.1: Physical Invariant Audit & E2E]
```

---

## Work Packages & Execution Items

### Work Package 1: Core Layout Engine (Rust Microkernel)

- [x] **T1.1 (Data Types)**: Extend `products/ttzip/app-core/src/workspace/types.rs` with `TreemapTile` (32 bytes), `TileCategory`, and `TabViewMode::Treemap`.
  - *Verification*: `cargo test -p ttzip-app-core test_treemap_tile_layout` asserts 32 bytes and 8-byte alignment.
- [x] **T1.2 (Layout Engine)**: Implement `products/ttzip/app-core/src/workspace/treemap.rs` executing Bruls squarified layout over `CompactNode`.
  - *Verification*: Criterion benchmark confirms $\le 2.5\text{ ms}$ for 50,000 nodes.
- [x] **T1.3 (Tail Merging)**: Implement area cutoff ($< 4.0\text{ px}^2$) and relative weight cutoff ($< 0.05\%$) aggregating into `Others` tile.
  - *Verification*: Unit test validates total tiles $\le 2,048$ under dense synthetic tree with 200,000 leaf nodes.

---

### Work Package 2: Darwin High-Concurrency Local Scanner (macOS Core)

- [x] **T2.1 (Seen Deduplicator)**: Implement `products/ttzip/app-core/src/workspace/scanner/seen.rs` with $2^{26}$ atomic bitset (8MB) and 64-shard hash table fallback.
  - *Verification*: Parallel benchmark with 16 threads verifies $< 150\text{ ms}$ contention latency across 4,000,000 IDs.
- [x] **T2.2 (getattrlistbulk)**: Implement `products/ttzip/app-core/src/workspace/scanner/darwin.rs` wrapping Darwin `getattrlistbulk(2)` using `thread_local!` 64KB stack buffer.
  - *Verification*: Real NVMe scan benchmark confirms $\ge 25,000\text{ items/sec}$.
- [x] **T2.3 (Classifier & Sandbox Defense)**: Implement `classify.rs` recognizing `target/`, `node_modules/`, `DerivedData`, and inject `setiopolicy_np(3, 0, 1)`.
  - *Verification*: Test confirms iCloud dataless files are marked `TREEMAP_FLAG_CLOUD_STUB` without triggering network download.

---

### Work Package 3: UniFFI & C-ABI Bridge

- [x] **T3.1 (Export Bindings)**: Create `products/ttzip/app-core/src/uniffi_api/treemap.rs` exporting `compute_treemap_layout` and `start_storage_scan`.
  - *Verification*: `generate_uniffi.sh` runs cleanly without symbol conflicts.
- [x] **T3.2 (Swift Bridge)**: Expose async progress stream `StorageScanSession` in `products/ttzip/app-core/Sources/TTZipAppCore/`.
  - *Verification*: Swift package builds cleanly under Swift 6 strict concurrency (`-warnings-as-errors`).

---

### Work Package 4: UI Track 1 (Archive-Internal Treemap View)

- [x] **T4.1 (Treemap Canvas)**: Implement `TTZipTreemapCanvasView.swift` in `TTZipUI` using SwiftUI `Canvas` and Display P3 semantic palette.
  - *Verification*: 120 FPS scrolling and zoom transition verified in Instruments.
- [x] **T4.2 (Toolbar Integration)**: Update `MainView+Toolbar.swift` to add Treemap icon button and `⌘3` keyboard accelerator.
  - *Verification*: Toggling `⌘1`, `⌘2`, `⌘3` switches views seamlessly with 0ms visual flicker.
- [x] **T4.3 (Interactive Drill-down & QuickLook)**: Implement click-to-select, double-click to push crumb, and spacebar to trigger native QuickLook preview.
  - *Verification*: Spacebar on image tile launches `QLPreviewPanel` with zero disk write.

---

### Work Package 5: UI Track 2 (Storage Space Inspector Page)

- [x] **T5.1 (Sidebar Entry)**: Add `Storage Inspector` item in `SidebarLocationsSectionView.swift` under Locations.
  - *Verification*: Clicking navigates to the dedicated storage inspector workspace.
- [x] **T5.2 (Inspector Workspace View)**: Implement `StorageInspectorView.swift` showing volume usage bar, reclaimable space badge, and scan controls.
  - *Verification*: Scanning user's `Projects` folder streams progress and renders completed treemap.
- [x] **T5.3 (Forensic Trash Protocol)**: Implement safe deletion via `NSFileManager.trashItem` with confirmation modal and path blacklist.
  - *Verification*: Unit test verifies attempts to trash `/System` or `/usr` throw an immediate fatal permission rejection.

---

### Work Package 6: Verification & Governance

- [x] **T6.1 (LOC Gate)**: Run `scripts/lint_loc_gate.sh` to ensure all new files stay $\le 800$ LOC (target $\le 350$).
- [x] **T6.2 (Copyright Audit)**: Verify all new files carry standard proprietary header:
  `// Copyright 2026 MetastudyLine. All rights reserved. // SPDX-License-Identifier: BSD-3-Clause`
- [x] **T6.3 (Sandbox Verification)**: Run `sandbox-exec` against MAS profile to confirm zero unauthorized syscall violations.
