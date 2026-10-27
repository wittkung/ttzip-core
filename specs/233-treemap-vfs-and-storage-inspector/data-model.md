# Data Model Specification: 233 - Treemap VFS & Storage Space Inspector

> **Target Version**: TTWorkspace v6.2  
> **Classification**: Normative Data Contracts & Memory Layouts  

---

## 1. Treemap Tile Geometry Layout

### 1.1 `TreemapTile` (32 Bytes, 8-Byte Aligned)

```rust
// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

/// Canonical 32-byte squarified treemap tile layout.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreemapTile {
    pub x: f32,          // 0..4:   Viewport logical point (pixel) X coordinate
    pub y: f32,          // 4..8:   Viewport logical point (pixel) Y coordinate
    pub width: f32,      // 8..12:  Tile width in logical points (pixels)
    pub height: f32,     // 12..16: Tile height in logical points (pixels)
    pub node_id: u32,    // 16..20: VFS CompactNode ID (0xFFFFFFFF = Others aggregate)
    pub depth: u16,      // 20..22: Relative directory depth from current root
    pub category: u8,    // 22..23: TileCategory discriminant
    pub flags: u8,       // 23..24: TileFlags bitmask
    pub bytes: u64,      // 24..32: Total uncompressed / physical size in bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_treemap_tile_layout() {
        assert_eq!(std::mem::size_of::<TreemapTile>(), 32);
        assert_eq!(std::mem::align_of::<TreemapTile>(), 8);
    }
}
```

### 1.2 Byte-Level Field Breakdown

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                           x (f32)                             |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                           y (f32)                             |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         width (f32)                           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                        height (f32)                           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                        node_id (u32)                          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|          depth (u16)          | category (u8) |   flags (u8)  |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
+                          bytes (u64)                          +
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

### 1.3 Tile Flags Bitmask (`flags: u8`)
- `TREEMAP_FLAG_IS_DIR        = (1 << 0)`: 0x01 — Entry is a directory container
- `TREEMAP_FLAG_IS_LEAF       = (1 << 1)`: 0x02 — Entry is a leaf node / standalone file
- `TREEMAP_FLAG_TAIL_COLLAPSED= (1 << 2)`: 0x04 — Entry is an `Others` synthetic aggregate
- `TREEMAP_FLAG_RECLAIMABLE   = (1 << 3)`: 0x08 — Marked as developer-reclaimable space
- `TREEMAP_FLAG_CLOUD_STUB    = (1 << 4)`: 0x10 — Evicted dataless cloud file (iCloud / OneDrive)
- `TREEMAP_FLAG_SELECTED      = (1 << 5)`: 0x20 — Currently highlighted by user cursor/tap

---

## 2. Category Taxonomy (`category: u8`)

```rust
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileCategory {
    GeneralCode     = 0,  // Source code: .rs, .swift, .c, .cpp, .ts, .go, .py
    MediaImage      = 1,  // Visual media: .png, .jpg, .heic, .webp, .svg
    MediaVideo      = 2,  // Video streams: .mp4, .mov, .mkv, .webm
    MediaAudio      = 3,  // Acoustic streams: .flac, .wav, .aac, .mp3
    CompressedArchive= 4, // Nested archives: .zip, .tar, .7z, .rar, .gz
    DocumentText    = 5,  // Textual assets: .pdf, .docx, .md, .csv
    ExecutableBinary= 6,  // Mach-O binaries, .dylib, .framework
    ReclaimCache    = 7,  // target/, node_modules/, DerivedData, .gradle
    SystemMeta      = 8,  // .DS_Store, .git/, .localized
    Others          = 9,  // Tail-collapsed or unrecognized entries
}
```

---

## 3. Storage Scanner & Deduplication Models

### 3.1 Inode Deduplication (`Seen`)
```rust
pub struct SeenDeduplicator {
    /// Lock-free L1 bitset for Inodes < 67,108,864 (8MB memory)
    bits: Box<[std::sync::atomic::AtomicU64]>,
    /// L2 64-shard partitioned hash set for overflow Inodes
    shards: Box<[std::sync::Mutex<rustc_hash::FxHashSet<(u64, u64)>>; 64]>,
}
```

### 3.2 Storage Scan Progress DTO
```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct StorageScanProgressDto {
    pub total_scanned_files: u64,
    pub total_scanned_dirs: u64,
    pub total_scanned_bytes: u64,
    pub total_reclaimable_bytes: u64,
    pub is_complete: bool,
    pub current_directory_path: String,
}
```

---

## 4. Workspace View Mode Extension

In `products/ttzip/app-core/src/workspace/types.rs`:

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabViewMode {
    MillerColumns,
    IconGrid,
    DetailList,
    Treemap, // Feature 233 addition
}
```
