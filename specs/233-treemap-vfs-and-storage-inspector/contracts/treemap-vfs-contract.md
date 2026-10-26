# UniFFI Contract Specification: 233 - Treemap VFS & Storage Space Inspector

> **Target Version**: TTWorkspace v6.2  
> **Interface Definer**: `app-core/src/uniffi_api/treemap.rs` & `workspace.rs`  

---

## 1. UniFFI Interface Definition (UDL / Procedural Macro)

```rust
// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

#[uniffi::export]
impl VfsSession {
    /// Compute a squarified treemap layout for the children of the specified parent node
    /// within bounding box [0, 0, width_px, height_px].
    pub fn compute_treemap_layout(
        &self,
        parent_id: u32,
        width_px: f32,
        height_px: f32,
        max_depth: u16,
    ) -> Result<Vec<TreemapTileDto>, VfsError>;

    /// Resolve the navigation path crumbs from root to a specific node_id.
    pub fn resolve_node_crumbs(&self, node_id: u32) -> Result<Vec<u32>, VfsError>;
}

#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct TreemapTileDto {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub node_id: u32,
    pub depth: u16,
    pub category: u8,
    pub flags: u8,
    pub bytes: u64,
    pub name: String,
    pub relative_path: String,
}

#[uniffi::export]
pub fn start_storage_scan(
    root_path: String,
    progress_callback: Box<dyn StorageScanCallback>,
) -> Result<Arc<StorageScanSessionHandle>, StorageScanError>;

#[uniffi::export(callback_interface)]
pub trait StorageScanCallback: Send + Sync {
    fn on_progress(&self, progress: StorageScanProgressDto);
    fn on_completed(&self, session_id: String);
    fn on_error(&self, error_message: String);
}
```

---

## 2. Swift 6 Invocations

```swift
// Swift 6 consumption pattern
let tiles: [TreemapTileDto] = try session.computeTreemapLayout(
    parentId: currentParentId,
    widthPx: Float(geometry.size.width),
    heightPx: Float(geometry.size.height),
    maxDepth: 3
)
```
