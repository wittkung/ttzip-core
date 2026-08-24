// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

//! Flat Struct-of-Arrays (SoA) VFS Arena with String Interning and O(N) Hash Indexing.

use crate::types::{TTZipPackedEntryArray, TTZipVfsNodeSummary};
use std::collections::HashMap;
use std::ffi::c_char;

pub const VFS_NULL_NODE: u32 = u32::MAX;

pub const VFS_FLAG_IS_DIR: u8 = 1 << 0;
pub const VFS_FLAG_IS_ENCRYPTED: u8 = 1 << 1;
pub const VFS_FLAG_IS_SYMLINK: u8 = 1 << 2;

/// High-density Struct-of-Arrays Flat Arena for virtual filesystem nodes.
#[derive(Debug, Clone, Default)]
pub struct VfsArena {
    pub string_arena: Vec<u8>,
    pub name_offsets: Vec<u32>,
    pub name_lens: Vec<u32>,
    pub full_path_offsets: Vec<u32>,
    pub full_path_lens: Vec<u32>,

    pub uncompressed_sizes: Vec<u64>,
    pub compressed_sizes: Vec<u64>,
    pub crc32s: Vec<u32>,
    pub mtimes: Vec<i64>,
    pub modes: Vec<u32>,
    pub flags: Vec<u8>,

    pub parent_ids: Vec<u32>,
    pub first_child: Vec<u32>,
    pub next_sibling: Vec<u32>,
    pub child_counts: Vec<u32>,

    pub total_nodes: usize,
}

impl VfsArena {
    pub fn with_capacity(capacity: usize) -> Self {
        let cap = capacity.max(16);
        Self {
            string_arena: Vec::with_capacity(cap * 32),
            name_offsets: Vec::with_capacity(cap),
            name_lens: Vec::with_capacity(cap),
            full_path_offsets: Vec::with_capacity(cap),
            full_path_lens: Vec::with_capacity(cap),
            uncompressed_sizes: Vec::with_capacity(cap),
            compressed_sizes: Vec::with_capacity(cap),
            crc32s: Vec::with_capacity(cap),
            mtimes: Vec::with_capacity(cap),
            modes: Vec::with_capacity(cap),
            flags: Vec::with_capacity(cap),
            parent_ids: Vec::with_capacity(cap),
            first_child: Vec::with_capacity(cap),
            next_sibling: Vec::with_capacity(cap),
            child_counts: Vec::with_capacity(cap),
            total_nodes: 0,
        }
    }

    /// Appends a raw string slice to the interned string arena.
    pub fn intern_string(&mut self, s: &str) -> (u32, u32) {
        let bytes = s.as_bytes();
        let offset = self.string_arena.len() as u32;
        let len = bytes.len() as u32;
        self.string_arena.extend_from_slice(bytes);
        self.string_arena.push(0); // Null-terminate for zero-copy C-string compatibility
        (offset, len)
    }

    /// Allocates a new node in the SoA arena and returns its node ID.
    pub fn alloc_node(
        &mut self,
        name: &str,
        full_path: &str,
        uncompressed_size: u64,
        compressed_size: u64,
        crc32: u32,
        mtime: i64,
        mode: u32,
        flags: u8,
        parent_id: u32,
    ) -> u32 {
        let id = self.total_nodes as u32;
        let (name_off, name_len) = self.intern_string(name);
        let (path_off, path_len) = self.intern_string(full_path);

        self.name_offsets.push(name_off);
        self.name_lens.push(name_len);
        self.full_path_offsets.push(path_off);
        self.full_path_lens.push(path_len);
        self.uncompressed_sizes.push(uncompressed_size);
        self.compressed_sizes.push(compressed_size);
        self.crc32s.push(crc32);
        self.mtimes.push(mtime);
        self.modes.push(mode);
        self.flags.push(flags);
        self.parent_ids.push(parent_id);
        self.first_child.push(VFS_NULL_NODE);
        self.next_sibling.push(VFS_NULL_NODE);
        self.child_counts.push(0);

        self.total_nodes += 1;
        id
    }

    /// Adds a child node to a parent directory node in O(1) time.
    pub fn add_child(&mut self, parent_id: u32, child_id: u32) {
        if parent_id as usize >= self.total_nodes || child_id as usize >= self.total_nodes {
            return;
        }

        let old_first = self.first_child[parent_id as usize];
        self.next_sibling[child_id as usize] = old_first;
        self.first_child[parent_id as usize] = child_id;
        self.parent_ids[child_id as usize] = parent_id;
        self.child_counts[parent_id as usize] += 1;
    }

    /// Builds a full hierarchical VfsArena from a packed entry array in O(N) linear time.
    pub fn build_from_packed(packed: &TTZipPackedEntryArray, root_name: &str) -> Self {
        let count = packed.count;
        let mut arena = Self::with_capacity(count + 16);

        // Root Node (ID = 0)
        let root_id = arena.alloc_node(
            root_name,
            "",
            0,
            0,
            0,
            0,
            0o755,
            VFS_FLAG_IS_DIR,
            VFS_NULL_NODE,
        );

        let mut dir_map: HashMap<String, u32> = HashMap::with_capacity(count / 4 + 16);
        dir_map.insert(String::new(), root_id);

        let raw_utf8 = unsafe {
            std::slice::from_raw_parts(packed.utf8_bytes, packed.total_bytes_len)
        };
        let offsets = unsafe { std::slice::from_raw_parts(packed.path_offsets, count) };
        let lens = unsafe { std::slice::from_raw_parts(packed.path_lens, count) };
        let uncompressed = unsafe { std::slice::from_raw_parts(packed.uncompressed_sizes, count) };
        let compressed = unsafe { std::slice::from_raw_parts(packed.compressed_sizes, count) };
        let crcs = unsafe { std::slice::from_raw_parts(packed.crc32s, count) };
        let mtimes = unsafe { std::slice::from_raw_parts(packed.mtimes, count) };
        let modes = unsafe { std::slice::from_raw_parts(packed.modes, count) };
        let flags = unsafe { std::slice::from_raw_parts(packed.flags, count) };

        for i in 0..count {
            let start = offsets[i] as usize;
            let len = lens[i] as usize;
            if start + len > raw_utf8.len() {
                continue;
            }

            let path_str = std::str::from_utf8(&raw_utf8[start..start + len]).unwrap_or("");
            let clean_path = path_str.trim_matches('/');
            if clean_path.is_empty() {
                continue;
            }

            let is_dir = (flags[i] & VFS_FLAG_IS_DIR) != 0;

            // Resolve parent directory in O(1) via hash map
            let (parent_path, leaf_name) = match clean_path.rfind('/') {
                Some(idx) => (&clean_path[..idx], &clean_path[idx + 1..]),
                None => ("", clean_path),
            };

            let curr_parent_id = ensure_directory(&mut arena, &mut dir_map, parent_path, mtimes[i], root_id);

            if is_dir {
                if !dir_map.contains_key(clean_path) {
                    let dir_id = arena.alloc_node(
                        leaf_name,
                        clean_path,
                        uncompressed[i],
                        compressed[i],
                        crcs[i],
                        mtimes[i],
                        modes[i],
                        flags[i] | VFS_FLAG_IS_DIR,
                        curr_parent_id,
                    );
                    arena.add_child(curr_parent_id, dir_id);
                    dir_map.insert(clean_path.to_string(), dir_id);
                }
            } else {
                let file_id = arena.alloc_node(
                    leaf_name,
                    clean_path,
                    uncompressed[i],
                    compressed[i],
                    crcs[i],
                    mtimes[i],
                    modes[i],
                    flags[i],
                    curr_parent_id,
                );
                arena.add_child(curr_parent_id, file_id);
            }
        }

        arena
    }

    /// Retrieves windowed children slice for a directory node ID.
    pub fn get_children_slice(
        &self,
        dir_node_id: u32,
        offset: usize,
        limit: usize,
    ) -> (Vec<TTZipVfsNodeSummary>, usize) {
        if dir_node_id as usize >= self.total_nodes {
            return (Vec::new(), 0);
        }

        let total_in_dir = self.child_counts[dir_node_id as usize] as usize;
        let mut summaries = Vec::with_capacity(limit.min(total_in_dir));

        let mut curr_child = self.first_child[dir_node_id as usize];
        let mut idx = 0usize;

        while curr_child != VFS_NULL_NODE {
            if idx >= offset && summaries.len() < limit {
                let cid = curr_child as usize;
                let name_off = self.name_offsets[cid] as usize;
                let name_len = self.name_lens[cid];
                let is_dir = (self.flags[cid] & VFS_FLAG_IS_DIR) != 0;
                let is_enc = (self.flags[cid] & VFS_FLAG_IS_ENCRYPTED) != 0;
                let has_children = self.child_counts[cid] > 0;

                let name_ptr = self.string_arena[name_off..].as_ptr() as *const c_char;

                summaries.push(TTZipVfsNodeSummary {
                    struct_size: std::mem::size_of::<TTZipVfsNodeSummary>() as u32,
                    abi_version: crate::types::TTZIP_ABI_VERSION_2,
                    node_id: curr_child,
                    name_utf8: name_ptr,
                    name_len,
                    uncompressed_size: self.uncompressed_sizes[cid],
                    compressed_size: self.compressed_sizes[cid],
                    crc32: self.crc32s[cid],
                    mtime_epoch_secs: self.mtimes[cid],
                    mode: self.modes[cid],
                    is_directory: is_dir,
                    is_encrypted: is_enc,
                    has_children,
                });
            }

            curr_child = self.next_sibling[curr_child as usize];
            idx += 1;
        }

        (summaries, total_in_dir)
    }

    /// Performs zero-heap-allocation fuzzy search directly across arena nodes.
    pub fn search_zero_alloc(
        &self,
        query: &str,
        out_results: &mut [crate::fs::vfs::search::TTZipVfsMatchDto],
    ) -> usize {
        let mut matched_count = 0usize;
        let capacity = out_results.len();
        if capacity == 0 || self.total_nodes == 0 {
            return 0;
        }

        for i in 1..self.total_nodes {
            let name_off = self.name_offsets[i] as usize;
            let name_len = self.name_lens[i] as usize;
            let path_off = self.full_path_offsets[i] as usize;
            let path_len = self.full_path_lens[i] as usize;

            let name_bytes = &self.string_arena[name_off..name_off + name_len];
            let path_bytes = &self.string_arena[path_off..path_off + path_len];

            let name_str = std::str::from_utf8(name_bytes).unwrap_or("");
            let path_str = std::str::from_utf8(path_bytes).unwrap_or("");

            let is_dir = (self.flags[i] & VFS_FLAG_IS_DIR) != 0;
            let is_enc = (self.flags[i] & VFS_FLAG_IS_ENCRYPTED) != 0;

            if let Some(score) = crate::fs::vfs::search::fuzzy_match_zero_alloc(name_str, query) {
                if matched_count < capacity {
                    out_results[matched_count] = crate::fs::vfs::search::TTZipVfsMatchDto {
                        struct_size: std::mem::size_of::<crate::fs::vfs::search::TTZipVfsMatchDto>() as u32,
                        abi_version: crate::types::TTZIP_ABI_VERSION_2,
                        name: self.string_arena[name_off..].as_ptr() as *const c_char,
                        name_len,
                        path: self.string_arena[path_off..].as_ptr() as *const c_char,
                        path_len,
                        uncompressed_size: self.uncompressed_sizes[i],
                        compressed_size: self.compressed_sizes[i],
                        crc32: self.crc32s[i],
                        score: score + 100,
                        is_directory: is_dir,
                        is_encrypted: is_enc,
                    };
                    matched_count += 1;
                }
            } else if let Some(score) = crate::fs::vfs::search::fuzzy_match_zero_alloc(path_str, query) {
                if matched_count < capacity {
                    out_results[matched_count] = crate::fs::vfs::search::TTZipVfsMatchDto {
                        struct_size: std::mem::size_of::<crate::fs::vfs::search::TTZipVfsMatchDto>() as u32,
                        abi_version: crate::types::TTZIP_ABI_VERSION_2,
                        name: self.string_arena[name_off..].as_ptr() as *const c_char,
                        name_len,
                        path: self.string_arena[path_off..].as_ptr() as *const c_char,
                        path_len,
                        uncompressed_size: self.uncompressed_sizes[i],
                        compressed_size: self.compressed_sizes[i],
                        crc32: self.crc32s[i],
                        score,
                        is_directory: is_dir,
                        is_encrypted: is_enc,
                    };
                    matched_count += 1;
                }
            }
        }

        matched_count
    }

    /// Aggregates file and directory statistics from arena.
    pub fn get_stats(&self) -> (u64, u64, u64) {
        let mut total_files = 0u64;
        let mut total_dirs = 0u64;
        let mut total_size = 0u64;

        for i in 1..self.total_nodes {
            if (self.flags[i] & VFS_FLAG_IS_DIR) != 0 {
                total_dirs += 1;
            } else {
                total_files += 1;
                total_size += self.uncompressed_sizes[i];
            }
        }

        (total_files, total_dirs, total_size)
    }
}

fn ensure_directory(
    arena: &mut VfsArena,
    dir_map: &mut HashMap<String, u32>,
    parent_path: &str,
    mtime: i64,
    root_id: u32,
) -> u32 {
    if parent_path.is_empty() {
        return root_id;
    }
    if let Some(&id) = dir_map.get(parent_path) {
        return id;
    }

    let mut accumulated = String::new();
    let mut p_id = root_id;
    for segment in parent_path.split('/') {
        if segment.is_empty() {
            continue;
        }
        if !accumulated.is_empty() {
            accumulated.push('/');
        }
        accumulated.push_str(segment);

        if let Some(&existing_id) = dir_map.get(&accumulated) {
            p_id = existing_id;
        } else {
            let new_dir_id = arena.alloc_node(
                segment,
                &accumulated,
                0,
                0,
                0,
                mtime,
                0o755,
                VFS_FLAG_IS_DIR,
                p_id,
            );
            arena.add_child(p_id, new_dir_id);
            dir_map.insert(accumulated.clone(), new_dir_id);
            p_id = new_dir_id;
        }
    }
    p_id
}
