// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Flat Struct-of-Arrays (SoA) VFS Arena with Contiguous Pre-Sorted Child Pool and True O(1) Slice Paging.

#[path = "arena_index.rs"]
mod arena_index;

pub use arena_index::{FxBuildHasher, FxHasher};

use arena_index::{insert_arena_entry, packed_slice, RawArenaEntry};
use crate::types::TTZipPackedEntryArray;
use std::cmp::Ordering;
use std::collections::HashMap;
#[cfg(test)]
use std::hash::Hasher;

pub const VFS_NULL_NODE: u32 = u32::MAX;
pub const VFS_NULL_ENTRY: u32 = u32::MAX;
pub const VFS_FLAG_IS_DIR: u8 = 1 << 0;
pub const VFS_FLAG_IS_ENCRYPTED: u8 = 1 << 1;
pub const VFS_FLAG_IS_SYMLINK: u8 = 1 << 2;

/// Raw metadata describing an entry to insert into the VFS tree.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VfsEntry {
    pub path: String,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub mtime_epoch_secs: i64,
    pub mode: u32,
    pub is_directory: bool,
    pub is_encrypted: bool,
}

pub type VfsEntryMeta = VfsEntry;

/// Node allocation parameters for inserting a new VFS entity into `VfsCompactArena`.
#[derive(Debug, Clone, Copy)]
pub struct VfsNodeAllocParams<'a> {
    pub name: &'a str,
    pub full_path: &'a str,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub mtime: i64,
    pub mode: u32,
    pub flags: u8,
    pub parent_id: u32,
    pub entry_idx: Option<usize>,
}

/// Ultra-compact Structure-of-Arrays (SoA) virtual filesystem arena.
#[derive(Debug, Clone, Default)]
pub struct VfsCompactArena {
    pub root_path: String,
    pub names_pool: Vec<u8>,
    pub name_offset_and_len: Vec<(u32, u16)>,
    pub full_path_offsets: Vec<u32>,
    pub full_path_lens: Vec<u32>,
    pub parent_ids: Vec<u32>,
    pub uncompressed_sizes: Vec<u64>,
    pub compressed_sizes: Vec<u64>,
    pub modified_timestamps: Vec<i64>,
    pub node_attributes: Vec<u8>,
    pub child_slice_ranges: Vec<(u32, u32)>,
    pub sorted_child_pool: Vec<u32>,
    pub crc32s: Vec<u32>,
    pub modes: Vec<u32>,
    pub total_nodes: usize,
    pub total_entries_count: usize,
    pub entry_indices: Vec<u32>,
    pub expanded: Vec<bool>,
    pub selected: Vec<bool>,
    pub name_masks: Vec<u64>,
    pub subtree_end_ids: Vec<u32>,
}

pub type VfsArena = VfsCompactArena;

impl VfsCompactArena {
    pub fn with_capacity(capacity: usize) -> Self {
        let cap = capacity.max(16);
        Self {
            root_path: String::new(),
            names_pool: Vec::with_capacity(cap * 24),
            name_offset_and_len: Vec::with_capacity(cap),
            full_path_offsets: Vec::with_capacity(cap),
            full_path_lens: Vec::with_capacity(cap),
            parent_ids: Vec::with_capacity(cap),
            uncompressed_sizes: Vec::with_capacity(cap),
            compressed_sizes: Vec::with_capacity(cap),
            modified_timestamps: Vec::with_capacity(cap),
            node_attributes: Vec::with_capacity(cap),
            child_slice_ranges: Vec::with_capacity(cap),
            sorted_child_pool: Vec::with_capacity(cap),
            crc32s: Vec::with_capacity(cap),
            modes: Vec::with_capacity(cap),
            total_nodes: 0,
            total_entries_count: 0,
            entry_indices: Vec::with_capacity(cap),
            expanded: Vec::with_capacity(cap),
            selected: Vec::with_capacity(cap),
            name_masks: Vec::with_capacity(cap),
            subtree_end_ids: Vec::with_capacity(cap),
        }
    }

    /// Returns original archive entry index if present.
    #[inline]
    pub fn entry_index(&self, node_id: u32) -> Option<usize> {
        let nid = node_id as usize;
        let val = *self.entry_indices.get(nid)?;
        if val == VFS_NULL_ENTRY {
            None
        } else {
            Some(val as usize)
        }
    }

    /// Appends a raw string slice to the interned string pool.
    pub fn intern_string(&mut self, s: &str) -> (u32, u32) {
        let bytes = s.as_bytes();
        let offset = self.names_pool.len() as u32;
        let len = bytes.len() as u32;
        self.names_pool.extend_from_slice(bytes);
        self.names_pool.push(0);
        (offset, len)
    }

    /// Allocates a new node in the SoA arena and returns its node ID.
    pub fn alloc_node(&mut self, params: VfsNodeAllocParams<'_>) -> u32 {
        let id = self.total_nodes as u32;
        let name_len = params.name.len() as u32;
        let (name_off, path_off, path_len) =
            if !params.full_path.is_empty() && params.full_path.ends_with(params.name) {
                let (poff, plen) = self.intern_string(params.full_path);
                (poff + (plen - name_len), poff, plen)
            } else {
                let (noff, _) = self.intern_string(params.name);
                let (poff, plen) = self.intern_string(params.full_path);
                (noff, poff, plen)
            };

        self.name_offset_and_len.push((name_off, name_len as u16));
        self.full_path_offsets.push(path_off);
        self.full_path_lens.push(path_len);
        self.parent_ids.push(params.parent_id);
        self.uncompressed_sizes.push(params.uncompressed_size);
        self.compressed_sizes.push(params.compressed_size);
        self.crc32s.push(params.crc32);
        self.modified_timestamps.push(params.mtime);
        self.modes.push(params.mode);
        self.node_attributes.push(params.flags);
        self.child_slice_ranges.push((0, 0));
        self.entry_indices.push(params.entry_idx.map(|idx| idx as u32).unwrap_or(VFS_NULL_ENTRY));
        self.expanded.push(false);
        self.selected.push(false);
        self.name_masks.push(ttzip_types::compute_name_mask(params.name.as_bytes()));
        self.subtree_end_ids.push(id + 1);

        self.total_nodes += 1;
        id
    }

    /// Updates parent-child linkage. Children indices are fully built in `finalize_tree`.
    pub fn add_child(&mut self, parent_id: u32, child_id: u32) {
        if (parent_id as usize) < self.total_nodes && (child_id as usize) < self.total_nodes {
            self.parent_ids[child_id as usize] = parent_id;
        }
    }

    /// Finalizes the VFS arena by sorting children in-place and populating contiguous slice ranges.
    pub fn finalize_tree(&mut self) {
        let n = self.total_nodes;
        if n == 0 {
            return;
        }

        let mut counts = vec![0u32; n];
        for i in 1..n {
            let p = self.parent_ids[i];
            if p != VFS_NULL_NODE && (p as usize) < n {
                counts[p as usize] += 1;
            }
        }

        self.child_slice_ranges = vec![(0, 0); n];
        let mut cursors = vec![0u32; n];
        let mut total_children = 0u32;
        for i in 0..n {
            let cnt = counts[i];
            self.child_slice_ranges[i] = (total_children, cnt);
            cursors[i] = total_children;
            total_children += cnt;
        }

        self.sorted_child_pool = vec![0u32; total_children as usize];
        for i in 1..n {
            let p = self.parent_ids[i];
            if p != VFS_NULL_NODE && (p as usize) < n {
                let idx = cursors[p as usize] as usize;
                self.sorted_child_pool[idx] = i as u32;
                cursors[p as usize] += 1;
            }
        }

        for i in 0..n {
            let (start, count) = self.child_slice_ranges[i];
            if count > 1 {
                let s = start as usize;
                let c = count as usize;
                let slice = &mut self.sorted_child_pool[s..s + c];
                slice.sort_by(|&a, &b| {
                    let a_is_dir = (self.node_attributes[a as usize] & VFS_FLAG_IS_DIR) != 0;
                    let b_is_dir = (self.node_attributes[b as usize] & VFS_FLAG_IS_DIR) != 0;
                    match (a_is_dir, b_is_dir) {
                        (true, false) => Ordering::Less,
                        (false, true) => Ordering::Greater,
                        _ => {
                            let (a_off, a_len) = self.name_offset_and_len[a as usize];
                            let (b_off, b_len) = self.name_offset_and_len[b as usize];
                            let name_a = std::str::from_utf8(
                                &self.names_pool[a_off as usize..(a_off as usize + a_len as usize)],
                            )
                            .unwrap_or("");
                            let name_b = std::str::from_utf8(
                                &self.names_pool[b_off as usize..(b_off as usize + b_len as usize)],
                            )
                            .unwrap_or("");
                            crate::fs::natural_sort::natural_cmp(name_a, name_b)
                                .then_with(|| a.cmp(&b))
                        }
                    }
                });
            }
        }

        self.recalculate_sizes();
    }

    /// Recalculates uncompressed and compressed sizes non-recursively using an explicit heap stack.
    pub fn recalculate_sizes(&mut self) {
        let n = self.total_nodes;
        if n == 0 {
            return;
        }

        for i in 0..n {
            if (self.node_attributes[i] & VFS_FLAG_IS_DIR) != 0 {
                self.uncompressed_sizes[i] = 0;
                self.compressed_sizes[i] = 0;
            }
        }

        let mut visit_stack = Vec::with_capacity(1024);
        let mut post_order = Vec::with_capacity(n);
        let mut visited = vec![false; n];

        for root_candidate in 0..n {
            if visited[root_candidate] {
                continue;
            }
            visit_stack.push(root_candidate as u32);
            visited[root_candidate] = true;

            while let Some(node_id) = visit_stack.pop() {
                post_order.push(node_id);
                let (start, count) = self.child_slice_ranges[node_id as usize];
                let s = start as usize;
                let c = count as usize;
                for &child_id in &self.sorted_child_pool[s..s + c] {
                    if (child_id as usize) < n && !visited[child_id as usize] {
                        visited[child_id as usize] = true;
                        visit_stack.push(child_id);
                    }
                }
            }
        }

        self.subtree_end_ids.resize(n, 0);
        for i in 0..n {
            self.subtree_end_ids[i] = i as u32 + 1;
        }

        for &node_id in post_order.iter().rev() {
            let nid = node_id as usize;
            let p = self.parent_ids[nid];
            if p != VFS_NULL_NODE && (p as usize) < n && p != node_id {
                let pid = p as usize;
                self.uncompressed_sizes[pid] =
                    self.uncompressed_sizes[pid].saturating_add(self.uncompressed_sizes[nid]);
                self.compressed_sizes[pid] =
                    self.compressed_sizes[pid].saturating_add(self.compressed_sizes[nid]);
                self.subtree_end_ids[pid] =
                    self.subtree_end_ids[pid].max(self.subtree_end_ids[nid]);
            }
        }
    }

    /// Returns true O(1) random-access slice of pre-sorted child node IDs.
    #[inline]
    pub fn get_children_paged(&self, parent_id: u32, offset: usize, limit: usize) -> &[u32] {
        if (parent_id as usize) >= self.total_nodes {
            return &[];
        }
        let (start, count) = self.child_slice_ranges[parent_id as usize];
        let available = (count as usize).saturating_sub(offset);
        let take = limit.min(available);
        if take == 0 {
            return &[];
        }
        let start_idx = (start as usize) + offset;
        &self.sorted_child_pool[start_idx..start_idx + take]
    }

    /// Returns a slice of all pre-sorted child node IDs for the specified parent.
    #[inline]
    pub fn get_children(&self, parent_id: u32) -> &[u32] {
        if (parent_id as usize) >= self.total_nodes {
            return &[];
        }
        let (start, count) = self.child_slice_ranges[parent_id as usize];
        let s = start as usize;
        let c = count as usize;
        &self.sorted_child_pool[s..s + c]
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.total_nodes
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.total_nodes == 0
    }

    pub(self) fn alloc_dir_node(&mut self, name: &str, full_path: &str, mtime: i64, parent_id: u32) -> u32 {
        self.alloc_node(VfsNodeAllocParams {
            name,
            full_path,
            uncompressed_size: 0,
            compressed_size: 0,
            crc32: 0,
            mtime,
            mode: 0o755,
            flags: VFS_FLAG_IS_DIR,
            parent_id,
            entry_idx: None,
        })
    }

    fn init_root_arena(
        count: usize,
        entries_count: usize,
        root_name: &str,
    ) -> (Self, HashMap<String, u32, FxBuildHasher>, u32) {
        let mut arena = Self::with_capacity(count + 16);
        arena.root_path = root_name.to_string();
        arena.total_entries_count = entries_count;
        let root_id = arena.alloc_dir_node(root_name, "", 0, VFS_NULL_NODE);
        let mut dir_map = HashMap::with_capacity_and_hasher(count / 4 + 16, FxBuildHasher);
        dir_map.insert(String::new(), root_id);
        (arena, dir_map, root_id)
    }

    /// Builds a full hierarchical VfsCompactArena from a packed entry array in O(N) linear time.
    pub fn build_from_packed(packed: &TTZipPackedEntryArray, root_name: &str) -> Self {
        let count = packed.count;
        let (mut arena, mut dir_map, root_id) = Self::init_root_arena(count, count, root_name);

        let raw_utf8 = unsafe { packed_slice(packed.utf8_bytes, packed.total_bytes_len) };
        let offsets = unsafe { packed_slice(packed.path_offsets, count) };
        let lens = unsafe { packed_slice(packed.path_lens, count) };
        let uncompressed = unsafe { packed_slice(packed.uncompressed_sizes, count) };
        let compressed = unsafe { packed_slice(packed.compressed_sizes, count) };
        let crcs = unsafe { packed_slice(packed.crc32s, count) };
        let mtimes = unsafe { packed_slice(packed.mtimes, count) };
        let modes = unsafe { packed_slice(packed.modes, count) };
        let flags = unsafe { packed_slice(packed.flags, count) };

        for i in 0..count {
            let start = offsets[i] as usize;
            let len = lens[i] as usize;
            if start + len > raw_utf8.len() {
                continue;
            }
            let path_str = std::str::from_utf8(&raw_utf8[start..start + len]).unwrap_or("");
            insert_arena_entry(
                &mut arena,
                &mut dir_map,
                root_id,
                RawArenaEntry {
                    raw_path: path_str,
                    uncompressed_size: uncompressed[i],
                    compressed_size: compressed[i],
                    crc32: crcs[i],
                    mtime: mtimes[i],
                    mode: modes[i],
                    flags: flags[i],
                    entry_idx: i,
                },
            );
        }

        arena.finalize_tree();
        arena
    }

    /// Builds a full hierarchical VfsCompactArena from structured VfsEntry list in continuous memory.
    pub fn build_from_entries(entries: &[VfsEntry], root_name: &str) -> Self {
        let non_empty = entries.iter().filter(|e| !e.path.trim().is_empty()).count();
        let (mut arena, mut dir_map, root_id) =
            Self::init_root_arena(entries.len(), non_empty, root_name);

        for (idx, e) in entries.iter().enumerate() {
            let mut flags = 0u8;
            if e.is_directory {
                flags |= VFS_FLAG_IS_DIR;
            }
            if e.is_encrypted {
                flags |= VFS_FLAG_IS_ENCRYPTED;
            }
            insert_arena_entry(
                &mut arena,
                &mut dir_map,
                root_id,
                RawArenaEntry {
                    raw_path: &e.path,
                    uncompressed_size: e.uncompressed_size,
                    compressed_size: e.compressed_size,
                    crc32: e.crc32,
                    mtime: e.mtime_epoch_secs,
                    mode: e.mode,
                    flags,
                    entry_idx: idx,
                },
            );
        }

        arena.finalize_tree();
        arena
    }

    /// Backward-compatible alias for `build_from_entries`.
    pub fn from_metadata_list(root_path: &str, entries: &[VfsEntry]) -> Self {
        Self::build_from_entries(entries, root_path)
    }

    /// Aggregates file and directory statistics from arena.
    pub fn get_stats(&self) -> (u64, u64, u64) {
        let mut total_files = 0u64;
        let mut total_dirs = 0u64;
        let mut total_size = 0u64;
        for i in 1..self.total_nodes {
            if (self.node_attributes[i] & VFS_FLAG_IS_DIR) != 0 {
                total_dirs += 1;
            } else {
                total_files += 1;
                total_size += self.uncompressed_sizes[i];
            }
        }
        (total_files, total_dirs, total_size)
    }

    /// Returns the node's name slice directly from the contiguous names pool.
    #[inline]
    pub fn node_name(&self, node_id: u32) -> &str {
        if (node_id as usize) >= self.total_nodes {
            return "";
        }
        let (off, len) = self.name_offset_and_len[node_id as usize];
        let s = off as usize;
        let e = s + len as usize;
        std::str::from_utf8(&self.names_pool[s..e]).unwrap_or("")
    }

    /// Returns borrowed slice of logical POSIX path in O(1) time without heap allocation.
    #[inline]
    pub fn node_full_path_str(&self, node_id: u32) -> &str {
        let nid = node_id as usize;
        if nid >= self.total_nodes || node_id == 0 {
            return "";
        }
        let off = self.full_path_offsets[nid] as usize;
        let len = self.full_path_lens[nid] as usize;
        let end = off + len;
        if end <= self.names_pool.len() {
            std::str::from_utf8(&self.names_pool[off..end]).unwrap_or("")
        } else {
            ""
        }
    }

    /// Reconstructs the full normalized path of a node directly from the contiguous names pool.
    #[inline]
    pub fn node_full_path(&self, node_id: u32) -> String {
        self.node_full_path_str(node_id).to_string()
    }

    #[inline]
    pub fn total_entries_count(&self) -> usize {
        self.total_entries_count
    }
}

#[cfg(test)]
#[path = "tests_arena.rs"]
mod tests;
