// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! In-process archive catalog representation with compact node layout.
//!
//! Provides a flat, high-density catalog structure pairing 24-byte [`CompactNode`]
//! entries with a contiguous byte arena for UTF-8 names and indexed child buckets
//! for fast hierarchy traversal.

use std::borrow::Cow;
use crate::node::CompactNode;

/// Flag indicating that a compact node is a directory.
pub const TT_NODE_FLAG_DIR: u16 = 1 << 0;
/// Alias for [`TT_NODE_FLAG_DIR`].
pub const COMPACT_NODE_FLAG_DIR: u16 = TT_NODE_FLAG_DIR;

/// Flag indicating that a compact node is a symbolic link.
pub const TT_NODE_FLAG_SYMLINK: u16 = 1 << 1;
/// Alias for [`TT_NODE_FLAG_SYMLINK`].
pub const COMPACT_NODE_FLAG_SYMLINK: u16 = TT_NODE_FLAG_SYMLINK;

/// Flag indicating that a compact node is encrypted.
pub const TT_NODE_FLAG_ENCRYPTED: u16 = 1 << 2;
/// Alias for [`TT_NODE_FLAG_ENCRYPTED`].
pub const COMPACT_NODE_FLAG_ENCRYPTED: u16 = TT_NODE_FLAG_ENCRYPTED;

/// Flag indicating that a compact node is read-only.
pub const TT_NODE_FLAG_READONLY: u16 = 1 << 3;
/// Alias for [`TT_NODE_FLAG_READONLY`].
pub const COMPACT_NODE_FLAG_READONLY: u16 = TT_NODE_FLAG_READONLY;

impl CompactNode {
    /// Returns true if this node represents a directory.
    #[inline]
    pub const fn is_dir(&self) -> bool {
        (self.flags & TT_NODE_FLAG_DIR) != 0
    }

    /// Returns true if this node represents a symbolic link.
    #[inline]
    pub const fn is_symlink(&self) -> bool {
        (self.flags & TT_NODE_FLAG_SYMLINK) != 0
    }

    /// Returns true if this node is encrypted.
    #[inline]
    pub const fn is_encrypted(&self) -> bool {
        (self.flags & TT_NODE_FLAG_ENCRYPTED) != 0
    }

    /// Returns true if this node is read-only.
    #[inline]
    pub const fn is_readonly(&self) -> bool {
        (self.flags & TT_NODE_FLAG_READONLY) != 0
    }
}

/// Flat in-process archive catalog containing compact nodes, string arena, and child buckets.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArchiveCatalog {
    /// Contiguous 24-byte POD nodes.
    pub nodes: Vec<CompactNode>,
    /// Single unified contiguous UTF-8 byte arena for entry names.
    pub string_arena: Vec<u8>,
    /// Index buckets mapping directory node index to slices of child node indices.
    pub dir_children: Vec<Vec<u32>>,
}

impl ArchiveCatalog {
    /// Constructs a new [`ArchiveCatalog`].
    pub fn new(
        nodes: Vec<CompactNode>,
        string_arena: Vec<u8>,
        dir_children: Vec<Vec<u32>>,
    ) -> Self {
        Self {
            nodes,
            string_arena,
            dir_children,
        }
    }

    /// Returns the number of entries in the catalog.
    #[inline]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns true if the catalog contains zero entries.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Returns a reference to the compact node at index `idx` if within bounds.
    #[inline]
    pub fn node(&self, idx: usize) -> Option<&CompactNode> {
        self.nodes.get(idx)
    }

    /// Extracts the raw byte slice for the entry name from `string_arena`.
    ///
    /// Returns an empty slice if offsets exceed the arena boundary or overflow.
    #[inline]
    pub fn name_bytes(&self, node: &CompactNode) -> &[u8] {
        let start = node.name_offset as usize;
        let end = match start.checked_add(node.name_len as usize) {
            Some(end) => end,
            None => return &[],
        };
        if end <= self.string_arena.len() {
            &self.string_arena[start..end]
        } else {
            &[]
        }
    }

    /// Extracts the UTF-8 entry name from `string_arena`.
    ///
    /// Falls back to an empty string `""` if bounds are exceeded or the slice contains invalid UTF-8.
    #[inline]
    pub fn name(&self, node: &CompactNode) -> &str {
        let bytes = self.name_bytes(node);
        std::str::from_utf8(bytes).unwrap_or("")
    }

    /// Returns lossy decoded UTF-8 string, allocating only if invalid UTF-8 sequences exist.
    #[inline]
    pub fn name_lossy(&self, node: &CompactNode) -> Cow<'_, str> {
        String::from_utf8_lossy(self.name_bytes(node))
    }

    /// Returns a slice of child node indices for a directory entry.
    ///
    /// If the node is not a directory or `dir_children_idx` is out of bounds, returns an empty slice.
    #[inline]
    pub fn children(&self, node: &CompactNode) -> &[u32] {
        if node.is_dir() {
            let idx = node.dir_children_idx as usize;
            if idx < self.dir_children.len() {
                &self.dir_children[idx]
            } else {
                &[]
            }
        } else {
            &[]
        }
    }

    /// Calculates the physical heap memory bytes occupied by the catalog structures.
    pub fn memory_usage(&self) -> usize {
        let nodes_bytes = self.nodes.len() * std::mem::size_of::<CompactNode>();
        let arena_bytes = self.string_arena.len();
        let dir_children_bytes: usize = self
            .dir_children
            .iter()
            .map(|c| c.len() * std::mem::size_of::<u32>() + std::mem::size_of::<Vec<u32>>())
            .sum();
        nodes_bytes + arena_bytes + dir_children_bytes
    }
}

const _: () = {
    const fn assert_send_sync<T: Send + Sync + 'static>() {}
    assert_send_sync::<ArchiveCatalog>();
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_catalog() {
        let empty = ArchiveCatalog::default();
        assert_eq!(empty.len(), 0);
        assert!(empty.is_empty());
        assert!(empty.node(0).is_none());
        assert_eq!(empty.memory_usage(), 0);

        let new_empty = ArchiveCatalog::new(Vec::new(), Vec::new(), Vec::new());
        assert_eq!(new_empty.len(), 0);
        assert!(new_empty.is_empty());
        assert_eq!(new_empty.memory_usage(), 0);
    }

    #[test]
    fn test_catalog_construction_and_access() {
        let mut string_arena = Vec::new();
        let name_1 = "root";
        let offset_1 = string_arena.len() as u32;
        string_arena.extend_from_slice(name_1.as_bytes());

        let name_2 = "child.txt";
        let offset_2 = string_arena.len() as u32;
        string_arena.extend_from_slice(name_2.as_bytes());

        let root_node = CompactNode {
            name_offset: offset_1,
            name_len: name_1.len() as u16,
            flags: TT_NODE_FLAG_DIR,
            size: 0,
            mtime: 1700000000,
            dir_children_idx: 0,
        };

        let file_node = CompactNode {
            name_offset: offset_2,
            name_len: name_2.len() as u16,
            flags: 0,
            size: 1024,
            mtime: 1700000001,
            dir_children_idx: 0,
        };

        let nodes = vec![root_node, file_node];
        let dir_children = vec![vec![1]];

        let catalog = ArchiveCatalog::new(nodes, string_arena, dir_children);

        assert_eq!(catalog.len(), 2);
        assert!(!catalog.is_empty());

        let r = catalog.node(0).expect("root node should exist");
        assert_eq!(catalog.name(r), "root");
        assert_eq!(catalog.name_bytes(r), b"root");
        assert!(r.is_dir());
        assert_eq!(catalog.children(r), &[1]);

        let f = catalog.node(1).expect("file node should exist");
        assert_eq!(catalog.name(f), "child.txt");
        assert_eq!(catalog.name_bytes(f), b"child.txt");
        assert!(!f.is_dir());
        assert_eq!(catalog.children(f), &[] as &[u32]);

        assert!(catalog.node(2).is_none());
    }

    #[test]
    fn test_name_resolution_and_boundary_safety() {
        let string_arena = b"valid_filename.bin\xFF\xFEtrailing".to_vec();
        let catalog = ArchiveCatalog::new(Vec::new(), string_arena, Vec::new());

        // Valid slice
        let valid_node = CompactNode {
            name_offset: 0,
            name_len: 18,
            flags: 0,
            size: 100,
            mtime: 0,
            dir_children_idx: 0,
        };
        assert_eq!(catalog.name(&valid_node), "valid_filename.bin");
        assert_eq!(catalog.name_bytes(&valid_node), b"valid_filename.bin");

        // Out of bounds length
        let oob_len_node = CompactNode {
            name_offset: 0,
            name_len: 1000,
            flags: 0,
            size: 0,
            mtime: 0,
            dir_children_idx: 0,
        };
        assert_eq!(catalog.name(&oob_len_node), "");
        assert_eq!(catalog.name_bytes(&oob_len_node), b"");

        // Out of bounds offset
        let oob_offset_node = CompactNode {
            name_offset: 500,
            name_len: 10,
            flags: 0,
            size: 0,
            mtime: 0,
            dir_children_idx: 0,
        };
        assert_eq!(catalog.name(&oob_offset_node), "");
        assert_eq!(catalog.name_bytes(&oob_offset_node), b"");

        // Offset + Len overflow
        let overflow_node = CompactNode {
            name_offset: u32::MAX - 5,
            name_len: 10,
            flags: 0,
            size: 0,
            mtime: 0,
            dir_children_idx: 0,
        };
        assert_eq!(catalog.name(&overflow_node), "");
        assert_eq!(catalog.name_bytes(&overflow_node), b"");

        // Invalid UTF-8 sequence
        let invalid_utf8_node = CompactNode {
            name_offset: 18,
            name_len: 2, // b"\xFF\xFE"
            flags: 0,
            size: 0,
            mtime: 0,
            dir_children_idx: 0,
        };
        assert_eq!(catalog.name(&invalid_utf8_node), "");
        assert_eq!(catalog.name_bytes(&invalid_utf8_node), b"\xFF\xFE");
        assert_eq!(catalog.name_lossy(&invalid_utf8_node), "\u{FFFD}\u{FFFD}");
    }

    #[test]
    fn test_children_navigation() {
        let dir_children = vec![vec![1, 2, 3], vec![4, 5]];
        let catalog = ArchiveCatalog::new(Vec::new(), Vec::new(), dir_children);

        let valid_dir = CompactNode {
            flags: TT_NODE_FLAG_DIR,
            dir_children_idx: 0,
            ..Default::default()
        };
        assert_eq!(catalog.children(&valid_dir), &[1, 2, 3]);

        let second_dir = CompactNode {
            flags: TT_NODE_FLAG_DIR,
            dir_children_idx: 1,
            ..Default::default()
        };
        assert_eq!(catalog.children(&second_dir), &[4, 5]);

        let oob_dir = CompactNode {
            flags: TT_NODE_FLAG_DIR,
            dir_children_idx: 99,
            ..Default::default()
        };
        assert_eq!(catalog.children(&oob_dir), &[] as &[u32]);

        let non_dir = CompactNode {
            flags: 0,
            dir_children_idx: 0,
            ..Default::default()
        };
        assert_eq!(catalog.children(&non_dir), &[] as &[u32]);
    }

    #[test]
    fn test_memory_usage_calculation() {
        let nodes = vec![
            CompactNode::default(),
            CompactNode::default(),
        ];
        let string_arena = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let dir_children = vec![
            vec![10, 20, 30],
            vec![40],
        ];

        let catalog = ArchiveCatalog::new(nodes, string_arena, dir_children);

        let expected_nodes = 2 * std::mem::size_of::<CompactNode>();
        let expected_arena = 10;
        let expected_dir_children = (3 * 4 + std::mem::size_of::<Vec<u32>>())
            + (4 + std::mem::size_of::<Vec<u32>>());
        let expected_total = expected_nodes + expected_arena + expected_dir_children;

        assert_eq!(catalog.memory_usage(), expected_total);
    }

    #[test]
    fn test_compact_node_flags() {
        let mut node = CompactNode::default();
        assert!(!node.is_dir());
        assert!(!node.is_symlink());
        assert!(!node.is_encrypted());
        assert!(!node.is_readonly());

        node.flags |= TT_NODE_FLAG_DIR;
        assert!(node.is_dir());

        node.flags |= TT_NODE_FLAG_SYMLINK;
        assert!(node.is_symlink());

        node.flags |= TT_NODE_FLAG_ENCRYPTED;
        assert!(node.is_encrypted());

        node.flags |= TT_NODE_FLAG_READONLY;
        assert!(node.is_readonly());
    }
}
