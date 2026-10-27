// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Contiguous memory layout unified node and string arena for TTZip VFS.
//!
//! # Microarchitectural Invariants
//! 1. Memory alignment: [`UnifiedEntryNode`] is strictly `#[repr(C, align(8))]` with a deterministic
//!    48-byte footprint (3 nodes fit across exactly 2 L1 cache lines of 64 bytes).
//! 2. Left-Child Right-Sibling (LCRS) tree: Hierarchical directories and flat sequential lists are
//!    unified in a single flat array without heap pointer indirection.
//! 3. Zero-fragmentation string storage: All path strings are sequentially appended into a contiguous
//!    UTF-8 [`StringArena`]. Slices are accessed via relative 32-bit offset and 16-bit length.

use serde::{Deserialize, Serialize};

/// Sentinel index indicating no node, null pointer, or root boundary.
pub const NO_NODE: u32 = 0xFFFF_FFFF;

/// Sentinel parent index indicating a root-level node.
pub const ROOT_PARENT_ID: u32 = 0xFFFF_FFFF;

/// Node flag indicating the entry is a directory.
pub const NODE_FLAG_DIRECTORY: u16 = 1 << 0;

/// Node flag indicating the entry is encrypted.
pub const NODE_FLAG_ENCRYPTED: u16 = 1 << 1;

/// Node flag indicating the entry is a symbolic link.
pub const NODE_FLAG_SYMLINK: u16 = 1 << 2;

/// Node flag indicating the entry is hidden.
pub const NODE_FLAG_HIDDEN: u16 = 1 << 3;

/// 48-byte cache-compact unified entry node supporting both flat listing and LCRS tree traversal.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnifiedEntryNode {
    /// Uncompressed file size in bytes (8 bytes, offset [0..8]).
    pub uncompressed_size: u64,
    /// Compressed file size in bytes (8 bytes, offset [8..16]).
    pub compressed_size: u64,
    /// Byte offset within [`StringArena`] (4 bytes, offset [16..20]).
    pub path_offset: u32,
    /// Byte length within [`StringArena`] (2 bytes, offset [20..22]).
    pub path_len: u16,
    /// Entry flags bitfield (2 bytes, offset [22..24]).
    pub flags: u16,
    /// CRC32 checksum (4 bytes, offset [24..28]).
    pub crc32: u32,
    /// Index of parent node in table or `ROOT_PARENT_ID` (4 bytes, offset [28..32]).
    pub parent_id: u32,
    /// Index of first child node in table or `NO_NODE` (4 bytes, offset [32..36]).
    pub first_child_id: u32,
    /// Index of next sibling node in table or `NO_NODE` (4 bytes, offset [36..40]).
    pub next_sibling_id: u32,
    /// Modification timestamp in seconds since Unix epoch (4 bytes, offset [40..44]).
    pub mtime_epoch_secs: u32,
    /// Reserved for 8-byte alignment padding (4 bytes, offset [44..48]).
    pub _reserved: u32,
}

// Compile-time static assertions guaranteeing binary footprint and alignment contracts.
const _: () = assert!(std::mem::size_of::<UnifiedEntryNode>() == 48);
const _: () = assert!(std::mem::align_of::<UnifiedEntryNode>() == 8);

impl UnifiedEntryNode {
    /// Creates an empty default node with sentinel pointers initialized to `NO_NODE`.
    #[inline]
    pub const fn empty() -> Self {
        Self {
            uncompressed_size: 0,
            compressed_size: 0,
            path_offset: 0,
            path_len: 0,
            flags: 0,
            crc32: 0,
            parent_id: ROOT_PARENT_ID,
            first_child_id: NO_NODE,
            next_sibling_id: NO_NODE,
            mtime_epoch_secs: 0,
            _reserved: 0,
        }
    }

    /// Returns `true` if this entry represents a directory.
    #[inline]
    pub const fn is_dir(&self) -> bool {
        (self.flags & NODE_FLAG_DIRECTORY) != 0
    }

    /// Returns `true` if this entry represents a regular file.
    #[inline]
    pub const fn is_file(&self) -> bool {
        !self.is_dir()
    }

    /// Returns `true` if this entry is encrypted.
    #[inline]
    pub const fn is_encrypted(&self) -> bool {
        (self.flags & NODE_FLAG_ENCRYPTED) != 0
    }

    /// Returns `true` if this entry is a symbolic link.
    #[inline]
    pub const fn is_symlink(&self) -> bool {
        (self.flags & NODE_FLAG_SYMLINK) != 0
    }

    /// Returns `true` if this entry has a valid parent node.
    #[inline]
    pub const fn has_parent(&self) -> bool {
        self.parent_id != ROOT_PARENT_ID
    }

    /// Returns `true` if this entry has at least one child.
    #[inline]
    pub const fn has_children(&self) -> bool {
        self.first_child_id != NO_NODE
    }

    /// Returns `true` if this entry has a subsequent sibling.
    #[inline]
    pub const fn has_next_sibling(&self) -> bool {
        self.next_sibling_id != NO_NODE
    }
}

impl Default for UnifiedEntryNode {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

/// Contiguous UTF-8 string arena minimizing heap allocations across millions of entries.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct StringArena {
    storage: String,
}

impl StringArena {
    /// Creates a new empty string arena.
    #[inline]
    pub fn new() -> Self {
        Self {
            storage: String::new(),
        }
    }

    /// Creates a new string arena preallocated with the specified byte capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            storage: String::with_capacity(capacity),
        }
    }

    /// Appends a string slice to the arena and returns its relative byte offset and length.
    ///
    /// # Panics
    /// Panics if total arena bytes exceed `u32::MAX` or the slice length exceeds `u16::MAX`.
    #[inline]
    pub fn push_str(&mut self, s: &str) -> (u32, u16) {
        let offset = self.storage.len();
        assert!(offset <= u32::MAX as usize, "StringArena capacity overflow");
        let len = s.len();
        assert!(len <= u16::MAX as usize, "String slice length exceeds u16::MAX");
        self.storage.push_str(s);
        (offset as u32, len as u16)
    }

    /// Safely attempts to append a string slice to the arena without panicking.
    #[inline]
    pub fn try_push_str(&mut self, s: &str) -> Result<(u32, u16), &'static str> {
        let offset = self.storage.len();
        if offset > u32::MAX as usize {
            return Err("StringArena capacity overflow");
        }
        let len = s.len();
        if len > u16::MAX as usize {
            return Err("String slice length exceeds u16::MAX");
        }
        self.storage.push_str(s);
        Ok((offset as u32, len as u16))
    }

    /// Extracts a string slice by offset and length, returning `None` if out of bounds or invalid boundary.
    #[inline]
    pub fn get(&self, offset: u32, len: u16) -> Option<&str> {
        let start = offset as usize;
        let end = start.checked_add(len as usize)?;
        self.storage.get(start..end)
    }

    /// Returns the total byte length of all strings stored in the arena.
    #[inline]
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    /// Returns `true` if the arena contains zero bytes.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }

    /// Returns the allocated byte capacity of the arena buffer.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.storage.capacity()
    }

    /// Clears the arena buffer while preserving allocated heap capacity.
    #[inline]
    pub fn clear(&mut self) {
        self.storage.clear();
    }

    /// Preallocates additional byte capacity.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.storage.reserve(additional);
    }

    /// Returns the full arena content as a contiguous string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.storage
    }

    /// Returns the full arena content as a contiguous byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.storage.as_bytes()
    }
}

/// In-memory contiguous virtual filesystem table combining flat entry indexing and LCRS hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct UnifiedVfsTable {
    /// Contiguous node storage.
    pub nodes: Vec<UnifiedEntryNode>,
    /// Contiguous string pool.
    pub strings: StringArena,
    /// Root node index or first root sibling index.
    pub root_id: u32,
}

impl UnifiedVfsTable {
    /// Creates an empty VFS table.
    #[inline]
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            strings: StringArena::new(),
            root_id: NO_NODE,
        }
    }

    /// Creates an empty VFS table with pre-allocated node and string capacities.
    #[inline]
    pub fn with_capacity(node_capacity: usize, string_capacity: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(node_capacity),
            strings: StringArena::with_capacity(string_capacity),
            root_id: NO_NODE,
        }
    }

    /// Returns total number of nodes in the table.
    #[inline]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns `true` if the table contains zero nodes.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Returns an immutable reference to the node at index `id`.
    #[inline]
    pub fn get_node(&self, id: u32) -> Option<&UnifiedEntryNode> {
        self.nodes.get(id as usize)
    }

    /// Returns a mutable reference to the node at index `id`.
    #[inline]
    pub fn get_node_mut(&mut self, id: u32) -> Option<&mut UnifiedEntryNode> {
        self.nodes.get_mut(id as usize)
    }

    /// Appends an entry node and its associated path string into the table.
    #[inline]
    pub fn add_entry(&mut self, mut node: UnifiedEntryNode, path: &str) -> u32 {
        let (offset, len) = self.strings.push_str(path);
        node.path_offset = offset;
        node.path_len = len;
        let id = self.nodes.len() as u32;
        self.nodes.push(node);
        id
    }

    /// Appends a raw pre-configured node into the table.
    #[inline]
    pub fn add_raw_node(&mut self, node: UnifiedEntryNode) -> u32 {
        let id = self.nodes.len() as u32;
        self.nodes.push(node);
        id
    }

    /// Resolves the path string for the given node.
    #[inline]
    pub fn get_path(&self, node: &UnifiedEntryNode) -> Option<&str> {
        self.strings.get(node.path_offset, node.path_len)
    }

    /// Resolves the path string for the given node index.
    #[inline]
    pub fn get_node_path(&self, id: u32) -> Option<&str> {
        let node = self.get_node(id)?;
        self.get_path(node)
    }

    /// Reconstructs Left-Child Right-Sibling (LCRS) tree indices from `parent_id` fields in $O(N)$ time.
    ///
    /// Preserves insertion order among siblings and connects top-level root items.
    pub fn build_lcrs_indices(&mut self) {
        let n = self.nodes.len();
        if n == 0 {
            self.root_id = NO_NODE;
            return;
        }

        // Reset existing child and sibling pointers
        for node in &mut self.nodes {
            node.first_child_id = NO_NODE;
            node.next_sibling_id = NO_NODE;
        }

        let mut last_child = vec![NO_NODE; n];
        let mut first_root = NO_NODE;
        let mut last_root = NO_NODE;

        for i in 0..n {
            let pid = self.nodes[i].parent_id;
            if pid == ROOT_PARENT_ID {
                if first_root == NO_NODE {
                    first_root = i as u32;
                } else {
                    self.nodes[last_root as usize].next_sibling_id = i as u32;
                }
                last_root = i as u32;
            } else if (pid as usize) < n && pid != (i as u32) {
                let p_idx = pid as usize;
                let prev = last_child[p_idx];
                if prev == NO_NODE {
                    self.nodes[p_idx].first_child_id = i as u32;
                } else {
                    self.nodes[prev as usize].next_sibling_id = i as u32;
                }
                last_child[p_idx] = i as u32;
            }
        }

        self.root_id = first_root;
    }

    /// Returns an iterator yielding `(child_id, &UnifiedEntryNode)` for all direct children of `parent_id`.
    ///
    /// Pass `ROOT_PARENT_ID` to iterate over all root-level entries.
    #[inline]
    pub fn children_of(&self, parent_id: u32) -> ChildIter<'_> {
        let start_id = if parent_id == ROOT_PARENT_ID {
            self.root_id
        } else {
            match self.nodes.get(parent_id as usize) {
                Some(parent_node) => parent_node.first_child_id,
                None => NO_NODE,
            }
        };
        ChildIter {
            nodes: &self.nodes,
            current: start_id,
            remaining: self.nodes.len(),
        }
    }

    /// Returns an iterator yielding child node indices for `parent_id`.
    #[inline]
    pub fn child_ids_of(&self, parent_id: u32) -> impl Iterator<Item = u32> + '_ {
        self.children_of(parent_id).map(|(id, _)| id)
    }

    /// Returns an iterator yielding immutable references to child nodes for `parent_id`.
    #[inline]
    pub fn child_nodes_of(&self, parent_id: u32) -> impl Iterator<Item = &UnifiedEntryNode> + '_ {
        self.children_of(parent_id).map(|(_, node)| node)
    }

    /// Clears all nodes, string arena data, and reset root indices while retaining memory.
    #[inline]
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.strings.clear();
        self.root_id = NO_NODE;
    }
}

/// Sibling chain iterator for LCRS child traversal with cycle-safeguard.
#[derive(Debug, Clone)]
pub struct ChildIter<'a> {
    nodes: &'a [UnifiedEntryNode],
    current: u32,
    remaining: usize,
}

impl<'a> Iterator for ChildIter<'a> {
    type Item = (u32, &'a UnifiedEntryNode);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.current == NO_NODE || self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        let idx = self.current as usize;
        let node = self.nodes.get(idx)?;
        let yield_id = self.current;
        self.current = node.next_sibling_id;
        Some((yield_id, node))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.current == NO_NODE {
            (0, Some(0))
        } else {
            (0, Some(self.remaining))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_size_and_alignment() {
        assert_eq!(std::mem::size_of::<UnifiedEntryNode>(), 48);
        assert_eq!(std::mem::align_of::<UnifiedEntryNode>(), 8);

        let default_node = UnifiedEntryNode::default();
        assert_eq!(default_node.parent_id, ROOT_PARENT_ID);
        assert_eq!(default_node.first_child_id, NO_NODE);
        assert_eq!(default_node.next_sibling_id, NO_NODE);
        assert!(!default_node.is_dir());
        assert!(default_node.is_file());
    }

    #[test]
    fn test_string_arena_push_and_get() {
        let mut arena = StringArena::with_capacity(128);
        assert!(arena.is_empty());

        let (off1, len1) = arena.push_str("first/path/file.txt");
        assert_eq!(off1, 0);
        assert_eq!(len1, 19);

        let (off2, len2) = arena.push_str("second/file.rs");
        assert_eq!(off2, 19);
        assert_eq!(len2, 14);

        assert_eq!(arena.get(off1, len1), Some("first/path/file.txt"));
        assert_eq!(arena.get(off2, len2), Some("second/file.rs"));
        assert_eq!(arena.get(0, 5), Some("first"));
        assert_eq!(arena.get(100, 10), None);
    }

    #[test]
    fn test_lcrs_tree_construction_and_traversal() {
        let mut table = UnifiedVfsTable::new();

        // 0: root_dir_a/ (parent: ROOT)
        let n0 = UnifiedEntryNode {
            flags: NODE_FLAG_DIRECTORY,
            parent_id: ROOT_PARENT_ID,
            ..Default::default()
        };
        let id0 = table.add_entry(n0, "root_dir_a/");

        // 1: root_dir_b/ (parent: ROOT)
        let n1 = UnifiedEntryNode {
            flags: NODE_FLAG_DIRECTORY,
            parent_id: ROOT_PARENT_ID,
            ..Default::default()
        };
        let id1 = table.add_entry(n1, "root_dir_b/");

        // 2: child_a1.txt (parent: 0)
        let n2 = UnifiedEntryNode {
            parent_id: id0,
            uncompressed_size: 1024,
            ..Default::default()
        };
        let id2 = table.add_entry(n2, "child_a1.txt");

        // 3: child_a2.txt (parent: 0)
        let n3 = UnifiedEntryNode {
            parent_id: id0,
            uncompressed_size: 2048,
            ..Default::default()
        };
        let id3 = table.add_entry(n3, "child_a2.txt");

        // 4: child_b1.txt (parent: 1)
        let n4 = UnifiedEntryNode {
            parent_id: id1,
            uncompressed_size: 4096,
            ..Default::default()
        };
        let id4 = table.add_entry(n4, "child_b1.txt");

        table.build_lcrs_indices();

        // Validate root siblings
        let root_children: Vec<u32> = table.child_ids_of(ROOT_PARENT_ID).collect();
        assert_eq!(root_children, vec![id0, id1]);

        // Validate children of dir_a
        let dir_a_children: Vec<u32> = table.child_ids_of(id0).collect();
        assert_eq!(dir_a_children, vec![id2, id3]);

        // Validate children of dir_b
        let dir_b_children: Vec<u32> = table.child_ids_of(id1).collect();
        assert_eq!(dir_b_children, vec![id4]);

        // Validate leaf children are empty
        assert_eq!(table.children_of(id2).count(), 0);

        // Validate path resolution
        assert_eq!(table.get_node_path(id0), Some("root_dir_a/"));
        assert_eq!(table.get_node_path(id2), Some("child_a1.txt"));
    }

    #[test]
    fn test_corrupt_cycle_resilience() {
        let mut table = UnifiedVfsTable::new();
        let node = UnifiedEntryNode {
            next_sibling_id: 0, // Self-loop cycle
            ..Default::default()
        };
        table.add_raw_node(node);

        // Traverse: must terminate safely within `nodes.len()` iterations without infinite looping
        let items: Vec<_> = table.children_of(ROOT_PARENT_ID).collect();
        // Since root_id is NO_NODE initially, yields 0
        assert_eq!(items.len(), 0);

        // Manually set root_id to node 0
        table.root_id = 0;
        let cycle_items: Vec<_> = table.children_of(ROOT_PARENT_ID).collect();
        assert_eq!(cycle_items.len(), 1); // Bounded by remaining counter
    }

    #[test]
    fn test_multibyte_utf8_strings() {
        let mut arena = StringArena::new();
        let (off_c, len_c) = arena.push_str("文档/资料/归档.tar.gz");
        let (off_j, len_j) = arena.push_str("音楽/アルバム/トラック.flac");
        let (off_e, len_e) = arena.push_str("emoji/🎉/rocket_🚀.zip");

        assert_eq!(arena.get(off_c, len_c), Some("文档/资料/归档.tar.gz"));
        assert_eq!(arena.get(off_j, len_j), Some("音楽/アルバム/トラック.flac"));
        assert_eq!(arena.get(off_e, len_e), Some("emoji/🎉/rocket_🚀.zip"));
    }

    #[test]
    fn test_deep_hierarchy_traversal() {
        let mut table = UnifiedVfsTable::new();

        // Level 0: root_dir (parent: ROOT)
        let l0 = table.add_entry(
            UnifiedEntryNode {
                flags: NODE_FLAG_DIRECTORY,
                parent_id: ROOT_PARENT_ID,
                ..Default::default()
            },
            "level0/",
        );

        // Level 1: level1_dir (parent: level0)
        let l1 = table.add_entry(
            UnifiedEntryNode {
                flags: NODE_FLAG_DIRECTORY,
                parent_id: l0,
                ..Default::default()
            },
            "level1/",
        );

        // Level 2: level2_dir (parent: level1)
        let l2 = table.add_entry(
            UnifiedEntryNode {
                flags: NODE_FLAG_DIRECTORY,
                parent_id: l1,
                ..Default::default()
            },
            "level2/",
        );

        // Level 3: leaf_file.bin (parent: level2)
        let l3 = table.add_entry(
            UnifiedEntryNode {
                uncompressed_size: 9999,
                parent_id: l2,
                ..Default::default()
            },
            "leaf_file.bin",
        );

        table.build_lcrs_indices();

        assert_eq!(table.child_ids_of(ROOT_PARENT_ID).collect::<Vec<_>>(), vec![l0]);
        assert_eq!(table.child_ids_of(l0).collect::<Vec<_>>(), vec![l1]);
        assert_eq!(table.child_ids_of(l1).collect::<Vec<_>>(), vec![l2]);
        assert_eq!(table.child_ids_of(l2).collect::<Vec<_>>(), vec![l3]);
        assert_eq!(table.child_ids_of(l3).count(), 0);
    }

    #[test]
    fn test_serde_json_roundtrip() {
        let mut table = UnifiedVfsTable::new();
        let id0 = table.add_entry(
            UnifiedEntryNode {
                flags: NODE_FLAG_DIRECTORY,
                parent_id: ROOT_PARENT_ID,
                ..Default::default()
            },
            "folder/",
        );
        let _id1 = table.add_entry(
            UnifiedEntryNode {
                uncompressed_size: 1024,
                parent_id: id0,
                ..Default::default()
            },
            "file.txt",
        );
        table.build_lcrs_indices();

        let json = serde_json::to_string(&table).expect("Serialization failed");
        let decoded: UnifiedVfsTable = serde_json::from_str(&json).expect("Deserialization failed");
        assert_eq!(table, decoded);
    }
}
