// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! 64-byte POD cache-line aligned workspace entry node.
//!
//! # Memory Geometry & Invariants
//! - Size: Exactly 64 bytes (L1 Cache line matched on Apple Silicon M-series).
//! - Alignment: 64-byte aligned (`#[repr(C, align(64))]`).
//! - CSR Child Slices: `first_child_id` and `child_count` define contiguous slice in VFS node table.
//! - StringArena Offset: `name_offset` + `name_len` locates the raw UTF-8 filename in StringArena.
//! - Dual-Key Sort: `sort_prefix` (8-byte big-endian) enables O(log K) branchless prefix binary search
//!   with fallback to full name comparison in StringArena only on prefix equality.

use core::cmp::Ordering;
use core::fmt;

// --- Physical Filesystem Flags ---
pub const TTWORKSPACE_FLAG_ENCRYPTED: u16 = 1 << 0;          // 0x0001
pub const TTWORKSPACE_FLAG_DEPRECATED_PINNED: u16 = 1 << 1;   // 0x0002
pub const TTWORKSPACE_FLAG_PINNED: u16 = TTWORKSPACE_FLAG_DEPRECATED_PINNED;
pub const TTWORKSPACE_FLAG_HIDDEN: u16 = 1 << 2;             // 0x0004
pub const TTWORKSPACE_FLAG_SECURITY_SCOPED: u16 = 1 << 3;    // 0x0008
pub const TTWORKSPACE_FLAG_READ_ONLY: u16 = 1 << 4;          // 0x0010: Read-only permissions
pub const TTWORKSPACE_FLAG_INLINE_PAYLOAD: u16 = 1 << 5;     // 0x0020: Small content in StringArena
pub const TTWORKSPACE_FLAG_DEPRECATED_STREAMING: u16 = 1 << 6; // 0x0040: Streamed from external pipe
pub const TTWORKSPACE_FLAG_STREAMING: u16 = TTWORKSPACE_FLAG_DEPRECATED_STREAMING;
pub const TTWORKSPACE_FLAG_NON_STANDARD_NAME: u16 = 1 << 7;  // 0x0080: Non-UTF8 preserved escape bytes

// --- Bits 14-15: aux union discriminator (aux_mode) ---
pub const TTWORKSPACE_AUX_MODE_SHIFT: u16 = 14;
pub const TTWORKSPACE_AUX_MODE_MASK: u16 = 0x3 << TTWORKSPACE_AUX_MODE_SHIFT; // 0xC000
pub const TTWORKSPACE_AUX_MODE_NONE: u16 = 0x0 << TTWORKSPACE_AUX_MODE_SHIFT;
pub const TTWORKSPACE_AUX_MODE_COMPRESSED: u16 = 0x1 << TTWORKSPACE_AUX_MODE_SHIFT;
pub const TTWORKSPACE_AUX_MODE_DEVICE: u16 = 0x2 << TTWORKSPACE_AUX_MODE_SHIFT;

// --- POSIX File Types (Low 16 Bits of file_type) ---
pub const TTWORKSPACE_POSIX_MODE_MASK: u32 = 0x0000_FFFF;
pub const TTWORKSPACE_POSIX_TYPE_MASK: u32 = 0x0000_F000;
pub const TTWORKSPACE_POSIX_PERM_MASK: u32 = 0x0000_0FFF;

pub const TTWORKSPACE_S_IFIFO: u32 = 0x0000_1000; // Named pipe (FIFO)
pub const TTWORKSPACE_S_IFCHR: u32 = 0x0000_2000; // Character device
pub const TTWORKSPACE_S_IFDIR: u32 = 0x0000_4000; // Directory
pub const TTWORKSPACE_S_IFBLK: u32 = 0x0000_6000; // Block device
pub const TTWORKSPACE_S_IFREG: u32 = 0x0000_8000; // Regular file
pub const TTWORKSPACE_S_IFLNK: u32 = 0x0000_A000; // Symbolic link
pub const TTWORKSPACE_S_IFSOCK: u32 = 0x0000_C000; // Unix domain socket
pub const TTWORKSPACE_S_IFWHT: u32 = 0x0000_E000; // BSD Whiteout

// --- Sentinels ---
pub const TTWORKSPACE_NODE_NONE: u32 = 0xFFFF_FFFF;
pub const TTWORKSPACE_STABLE_ID_NONE: u64 = 0xFFFF_FFFF_FFFF_FFFF;

/// Mutually exclusive 64-bit auxiliary payload union.
#[repr(C)]
#[derive(Copy, Clone)]
pub union TTWorkspaceAux {
    pub compressed_size: u64,
    pub device_object_id: u64,
    pub aux_payload: u64,
}

impl Default for TTWorkspaceAux {
    #[inline]
    fn default() -> Self {
        Self { aux_payload: 0 }
    }
}

impl fmt::Debug for TTWorkspaceAux {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Safe read of raw 64-bit bits
        write!(f, "TTWorkspaceAux(0x{:016x})", unsafe { self.aux_payload })
    }
}

/// 64-byte POD cache-line aligned workspace entry node.
#[repr(C, align(64))]
#[derive(Copy, Clone)]
pub struct TTWorkspaceEntryNode {
    pub parent_id: u32,          // 0..4:   Parent index (0xFFFFFFFF = root)
    pub first_child_id: u32,     // 4..8:   CSR first child index (0xFFFFFFFF = None)
    pub child_count: u32,        // 8..12:  CSR direct children count in contiguous slice
    pub name_offset: u32,        // 12..16: UTF-8 Base Name offset in StringArena
    pub sort_prefix: u64,        // 16..24: Big-Endian leading 8 bytes of UTF-8 filename (zero-padded)
    pub uncompressed_size: u64,  // 24..32: Original size in bytes (0 for S_IFDIR)
    pub mtime_epoch_s: i64,      // 32..40: Epoch seconds Unix timestamp
    pub aux: TTWorkspaceAux,     // 40..48: Mutually exclusive auxiliary payload
    pub stable_id: u64,          // 48..56: Cross-generation stable entry ID
    pub file_type: u32,          // 56..60: [0..15] POSIX st_mode, [16..31] FormatClass
    pub flags: u16,              // 60..62: Physical flags including aux_mode
    pub name_len: u16,           // 62..64: Base Name length in bytes
}

// Compile-time static assertions verifying exact geometry against C-ABI v6.0
const _: () = {
    assert!(core::mem::size_of::<TTWorkspaceEntryNode>() == 64);
    assert!(core::mem::align_of::<TTWorkspaceEntryNode>() == 64);
    assert!(core::mem::size_of::<TTWorkspaceAux>() == 8);
    assert!(core::mem::align_of::<TTWorkspaceAux>() == 8);
};

impl Default for TTWorkspaceEntryNode {
    #[inline]
    fn default() -> Self {
        Self {
            parent_id: TTWORKSPACE_NODE_NONE,
            first_child_id: TTWORKSPACE_NODE_NONE,
            child_count: 0,
            name_offset: 0,
            sort_prefix: 0,
            uncompressed_size: 0,
            mtime_epoch_s: 0,
            aux: TTWorkspaceAux::default(),
            stable_id: 0,
            file_type: TTWORKSPACE_S_IFREG,
            flags: 0,
            name_len: 0,
        }
    }
}

impl fmt::Debug for TTWorkspaceEntryNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TTWorkspaceEntryNode")
            .field("parent_id", &self.parent_id)
            .field("first_child_id", &self.first_child_id)
            .field("child_count", &self.child_count)
            .field("name_offset", &self.name_offset)
            .field("name_len", &self.name_len)
            .field("sort_prefix", &format_args!("0x{:016x}", self.sort_prefix))
            .field("uncompressed_size", &self.uncompressed_size)
            .field("mtime_epoch_s", &self.mtime_epoch_s)
            .field("stable_id", &self.stable_id)
            .field("file_type", &format_args!("0x{:08x}", self.file_type))
            .field("flags", &format_args!("0x{:04x}", self.flags))
            .field("aux", &self.aux)
            .finish()
    }
}

impl TTWorkspaceEntryNode {
    #[inline]
    pub const fn is_dir(&self) -> bool {
        (self.file_type & TTWORKSPACE_POSIX_TYPE_MASK) == TTWORKSPACE_S_IFDIR
    }

    #[inline]
    pub const fn is_file(&self) -> bool {
        (self.file_type & TTWORKSPACE_POSIX_TYPE_MASK) == TTWORKSPACE_S_IFREG
    }

    #[inline]
    pub const fn is_symlink(&self) -> bool {
        (self.file_type & TTWORKSPACE_POSIX_TYPE_MASK) == TTWORKSPACE_S_IFLNK
    }

    #[inline]
    pub const fn aux_mode(&self) -> u16 {
        self.flags & TTWORKSPACE_AUX_MODE_MASK
    }

    /// Read compressed size if valid according to aux_mode discriminator.
    #[inline]
    pub fn compressed_size(&self) -> Option<u64> {
        if self.aux_mode() == TTWORKSPACE_AUX_MODE_COMPRESSED {
            Some(unsafe { self.aux.compressed_size })
        } else {
            None
        }
    }

    /// Read device object id if valid according to aux_mode discriminator.
    #[inline]
    pub fn device_object_id(&self) -> Option<u64> {
        if self.aux_mode() == TTWORKSPACE_AUX_MODE_DEVICE {
            Some(unsafe { self.aux.device_object_id })
        } else {
            None
        }
    }

    /// Extract filename slice from the provided StringArena byte slice.
    #[inline]
    pub fn name<'a>(&self, arena: &'a [u8]) -> Option<&'a [u8]> {
        let start = self.name_offset as usize;
        let end = start.checked_add(self.name_len as usize)?;
        arena.get(start..end)
    }
}

/// Compute big-endian 8-byte sort prefix for accelerated binary search.
#[inline]
pub fn make_sort_prefix(name: &[u8]) -> u64 {
    let mut prefix = 0u64;
    let limit = name.len().min(8);
    let mut i = 0;
    while i < limit {
        prefix = (prefix << 8) | (name[i] as u64);
        i += 1;
    }
    if limit > 0 && limit < 8 {
        prefix <<= 8 * (8 - limit);
    }
    prefix
}

/// Branchless comparison of 8-byte big-endian sort prefixes.
#[inline]
pub fn compare_sort_prefix(a: u64, b: u64) -> Ordering {
    a.cmp(&b)
}

/// Find a child node by name within a contiguous CSR slice of TTWorkspaceEntryNode.
///
/// Uses sort_prefix for O(log K) comparisons, falling back to full StringArena memcmp
/// only when sort prefixes are identical.
#[inline]
pub fn find_child_by_name(
    children: &[TTWorkspaceEntryNode],
    arena: &[u8],
    target_name: &[u8],
) -> Option<usize> {
    let target_prefix = make_sort_prefix(target_name);
    let mut lo = 0usize;
    let mut hi = children.len();

    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let mid_node = &children[mid];
        match target_prefix.cmp(&mid_node.sort_prefix) {
            Ordering::Less => {
                hi = mid;
            }
            Ordering::Greater => {
                lo = mid + 1;
            }
            Ordering::Equal => {
                // Prefix collision: resolve with full string comparison in StringArena
                let mid_name = mid_node.name(arena)?;
                match target_name.cmp(mid_name) {
                    Ordering::Less => hi = mid,
                    Ordering::Greater => lo = mid + 1,
                    Ordering::Equal => return Some(mid),
                }
            }
        }
    }
    None
}


/// 24-byte compact node representation for directory catalog hierarchies.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CompactNode {
    pub name_offset: u32,
    pub name_len: u16,
    pub flags: u16,
    pub size: u64,
    pub mtime: u32,
    pub dir_children_idx: u32,
}

const _: () = {
    assert!(core::mem::size_of::<CompactNode>() == 24);
    assert!(core::mem::align_of::<CompactNode>() == 8);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact_node_geometry_and_offsets() {
        assert_eq!(core::mem::size_of::<CompactNode>(), 24);
        assert_eq!(core::mem::align_of::<CompactNode>(), 8);

        assert_eq!(core::mem::offset_of!(CompactNode, name_offset), 0);
        assert_eq!(core::mem::offset_of!(CompactNode, name_len), 4);
        assert_eq!(core::mem::offset_of!(CompactNode, flags), 6);
        assert_eq!(core::mem::offset_of!(CompactNode, size), 8);
        assert_eq!(core::mem::offset_of!(CompactNode, mtime), 16);
        assert_eq!(core::mem::offset_of!(CompactNode, dir_children_idx), 20);

        let default_node = CompactNode::default();
        assert_eq!(default_node.name_offset, 0);
        assert_eq!(default_node.name_len, 0);
        assert_eq!(default_node.flags, 0);
        assert_eq!(default_node.size, 0);
        assert_eq!(default_node.mtime, 0);
        assert_eq!(default_node.dir_children_idx, 0);
    }

    #[test]
    fn test_node_geometry_and_alignment() {
        assert_eq!(core::mem::size_of::<TTWorkspaceEntryNode>(), 64);
        assert_eq!(core::mem::align_of::<TTWorkspaceEntryNode>(), 64);
        assert_eq!(core::mem::size_of::<TTWorkspaceAux>(), 8);
        assert_eq!(core::mem::align_of::<TTWorkspaceAux>(), 8);
    }

    #[test]
    fn test_aux_mode_discrimination() {
        let mut node = TTWorkspaceEntryNode::default();
        node.aux.compressed_size = 4096;
        assert_eq!(node.compressed_size(), None);
        assert_eq!(node.device_object_id(), None);

        node.flags |= TTWORKSPACE_AUX_MODE_COMPRESSED;
        assert_eq!(node.compressed_size(), Some(4096));
        assert_eq!(node.device_object_id(), None);

        node.flags = (node.flags & !TTWORKSPACE_AUX_MODE_MASK) | TTWORKSPACE_AUX_MODE_DEVICE;
        node.aux.device_object_id = 0x1234_5678_ABCD_EF01;
        assert_eq!(node.compressed_size(), None);
        assert_eq!(node.device_object_id(), Some(0x1234_5678_ABCD_EF01));
    }

    #[test]
    fn test_sort_prefix_and_binary_search() {
        let mut arena = Vec::new();
        let names = ["alpha.txt", "beta.txt", "document_a.pdf", "document_b.pdf", "omega.zip"];
        let mut nodes = Vec::new();

        for name in names {
            let offset = arena.len() as u32;
            let len = name.len() as u16;
            arena.extend_from_slice(name.as_bytes());

            let node = TTWorkspaceEntryNode {
                name_offset: offset,
                name_len: len,
                sort_prefix: make_sort_prefix(name.as_bytes()),
                ..TTWorkspaceEntryNode::default()
            };
            nodes.push(node);
        }

        // Verify binary search on each existing item
        for (idx, name) in names.iter().enumerate() {
            let found = find_child_by_name(&nodes, &arena, name.as_bytes());
            assert_eq!(found, Some(idx), "failed to find {}", name);
        }

        // Verify non-existent search
        assert_eq!(find_child_by_name(&nodes, &arena, b"gamma.txt"), None);
        assert_eq!(find_child_by_name(&nodes, &arena, b"document_c.pdf"), None);
    }


    #[test]
    fn test_flags_and_aliases() {
        assert_eq!(TTWORKSPACE_FLAG_ENCRYPTED, 1 << 0);
        assert_eq!(TTWORKSPACE_FLAG_DEPRECATED_PINNED, 1 << 1);
        assert_eq!(TTWORKSPACE_FLAG_PINNED, 1 << 1);
        assert_eq!(TTWORKSPACE_FLAG_HIDDEN, 1 << 2);
        assert_eq!(TTWORKSPACE_FLAG_SECURITY_SCOPED, 1 << 3);
        assert_eq!(TTWORKSPACE_FLAG_READ_ONLY, 1 << 4);
        assert_eq!(TTWORKSPACE_FLAG_INLINE_PAYLOAD, 1 << 5);
        assert_eq!(TTWORKSPACE_FLAG_DEPRECATED_STREAMING, 1 << 6);
        assert_eq!(TTWORKSPACE_FLAG_STREAMING, 1 << 6);
        assert_eq!(TTWORKSPACE_FLAG_NON_STANDARD_NAME, 1 << 7);
        assert_eq!(TTWORKSPACE_AUX_MODE_MASK, 0xC000);
        assert_eq!(TTWORKSPACE_AUX_MODE_SHIFT, 14);
    }
}
