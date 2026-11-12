// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Hashing, path normalization, tree indexing, and search operations for `VfsCompactArena`.

use std::collections::HashMap;
use std::ffi::c_char;
use std::hash::{BuildHasher, Hasher};

use super::{
    VfsCompactArena, VfsNodeAllocParams, VFS_FLAG_IS_DIR, VFS_FLAG_IS_ENCRYPTED,
};

/// Ultra-fast zero-dependency FxHash algorithm compatible with 32-bit and 64-bit architectures.
#[derive(Clone, Copy, Default)]
pub struct FxHasher {
    hash: usize,
}

impl Hasher for FxHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.hash as u64
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        const K: usize = if std::mem::size_of::<usize>() == 8 {
            0x517c_c1b7_2722_0a95
        } else {
            0x9e37_79b9
        };
        let mut hash = self.hash;
        let (chunks, remainder) = bytes.as_chunks::<{ std::mem::size_of::<usize>() }>();
        for chunk in chunks {
            let word = usize::from_ne_bytes(*chunk);
            hash = (hash.rotate_left(5) ^ word).wrapping_mul(K);
        }
        for &byte in remainder {
            hash = (hash.rotate_left(5) ^ (byte as usize)).wrapping_mul(K);
        }
        self.hash = hash;
    }
}

/// BuildHasher provider for `FxHasher`.
#[derive(Clone, Copy, Default)]
pub struct FxBuildHasher;

impl BuildHasher for FxBuildHasher {
    type Hasher = FxHasher;
    #[inline]
    fn build_hasher(&self) -> Self::Hasher {
        FxHasher::default()
    }
}

impl VfsCompactArena {
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
            let (name_off, name_len) = self.name_offset_and_len[i];
            let path_off = self.full_path_offsets[i] as usize;
            let path_len = self.full_path_lens[i] as usize;

            let name_str = std::str::from_utf8(
                &self.names_pool[name_off as usize..name_off as usize + name_len as usize],
            )
            .unwrap_or("");
            let path_str =
                std::str::from_utf8(&self.names_pool[path_off..path_off + path_len]).unwrap_or("");

            let match_score = crate::fs::vfs::search::fuzzy_match_zero_alloc(name_str, query)
                .map(|s| s + 100)
                .or_else(|| crate::fs::vfs::search::fuzzy_match_zero_alloc(path_str, query));

            if let Some(score) = match_score {
                if matched_count < capacity {
                    let is_dir = (self.node_attributes[i] & VFS_FLAG_IS_DIR) != 0;
                    let is_enc = (self.node_attributes[i] & VFS_FLAG_IS_ENCRYPTED) != 0;
                    out_results[matched_count] = crate::fs::vfs::search::TTZipVfsMatchDto {
                        struct_size: std::mem::size_of::<crate::fs::vfs::search::TTZipVfsMatchDto>()
                            as u32,
                        abi_version: crate::types::TTZIP_ABI_VERSION_3,
                        name: self.names_pool[name_off as usize..].as_ptr() as *const c_char,
                        name_len: name_len as usize,
                        path: self.names_pool[path_off..].as_ptr() as *const c_char,
                        path_len,
                        uncompressed_size: self.uncompressed_sizes[i],
                        compressed_size: self.compressed_sizes[i],
                        crc32: self.crc32s[i],
                        score,
                        is_directory: is_dir,
                        is_encrypted: is_enc,
                    };
                    matched_count += 1;
                    if matched_count == capacity {
                        break;
                    }
                }
            }
        }

        matched_count
    }

    /// Resolves a logical POSIX path to a node ID using binary search on pre-sorted child partitions.
    pub fn resolve_path(&self, path: &str) -> Option<u32> {
        let wants_dir = raw_path_implies_dir(path);
        let Some((clean_cow, _)) = normalize_vfs_entry_path(path, wants_dir) else {
            return if path.split(['/', '\\']).any(|seg| seg == "..") {
                None
            } else {
                Some(0)
            };
        };
        let mut curr = 0u32;
        let segments: Vec<&str> = clean_cow.split('/').collect();
        for (idx, &segment) in segments.iter().enumerate() {
            let must_be_dir = idx + 1 < segments.len() || wants_dir;
            let children = self.get_children(curr);
            if children.is_empty() {
                return None;
            }
            let dir_count = children
                .iter()
                .take_while(|&&cid| (self.node_attributes[cid as usize] & VFS_FLAG_IS_DIR) != 0)
                .count();

            let dir_slice = &children[..dir_count];
            let found = dir_slice
                .binary_search_by(|&cid| {
                    crate::fs::natural_sort::natural_cmp(self.node_name(cid), segment)
                })
                .map(|d_idx| dir_slice[d_idx])
                .ok()
                .or_else(|| {
                    if must_be_dir {
                        return None;
                    }
                    let file_slice = &children[dir_count..];
                    file_slice
                        .binary_search_by(|&cid| {
                            crate::fs::natural_sort::natural_cmp(self.node_name(cid), segment)
                        })
                        .ok()
                        .map(|f_idx| file_slice[f_idx])
                });
            curr = found?;
        }
        Some(curr)
    }
}

pub(super) unsafe fn packed_slice<'a, T>(ptr: *const T, len: usize) -> &'a [T] {
    if len == 0 || ptr.is_null() {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}

pub(super) struct RawArenaEntry<'a> {
    pub raw_path: &'a str,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub mtime: i64,
    pub mode: u32,
    pub flags: u8,
    pub entry_idx: usize,
}

pub(super) fn raw_path_implies_dir(raw_path: &str) -> bool {
    let trimmed = raw_path.trim_end_matches(['/', '\\']);
    raw_path.ends_with('/')
        || raw_path.ends_with('\\')
        || trimmed == "."
        || trimmed.ends_with("/.")
        || trimmed.ends_with("\\.")
}

pub(super) fn normalize_vfs_entry_path(
    raw_path: &str,
    raw_is_dir: bool,
) -> Option<(std::borrow::Cow<'_, str>, bool)> {
    let is_dir = raw_is_dir || raw_path_implies_dir(raw_path);
    let nfc = crate::fs::safe_extract::normalize_to_nfc(raw_path);
    if let std::borrow::Cow::Borrowed(s) = nfc {
        let clean = s.trim_matches('/');
        if !clean.is_empty()
            && !clean.contains('\\')
            && clean.split('/').all(|seg| !seg.is_empty() && seg != "." && seg != "..")
        {
            return Some((std::borrow::Cow::Borrowed(clean), is_dir));
        }
    }
    let mut segments = Vec::new();
    for seg in nfc.split(['/', '\\']) {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            return None;
        }
        segments.push(seg);
    }
    (!segments.is_empty()).then(|| (std::borrow::Cow::Owned(segments.join("/")), is_dir))
}

pub(super) fn insert_arena_entry(
    arena: &mut VfsCompactArena,
    dir_map: &mut HashMap<String, u32, FxBuildHasher>,
    root_id: u32,
    entry: RawArenaEntry<'_>,
) {
    let raw_is_dir = (entry.flags & VFS_FLAG_IS_DIR) != 0;
    let Some((clean_cow, is_dir)) = normalize_vfs_entry_path(entry.raw_path, raw_is_dir) else {
        return;
    };
    let clean_path = clean_cow.as_ref();
    let (parent_path, leaf_name) = match clean_path.rfind('/') {
        Some(idx) => (&clean_path[..idx], &clean_path[idx + 1..]),
        None => ("", clean_path),
    };

    let curr_parent_id = ensure_directory(arena, dir_map, parent_path, entry.mtime, root_id);
    let node_flags = if is_dir {
        entry.flags | VFS_FLAG_IS_DIR
    } else {
        entry.flags & !VFS_FLAG_IS_DIR
    };

    if is_dir {
        if let Some(&existing_id) = dir_map.get(clean_path) {
            let eid = existing_id as usize;
            arena.uncompressed_sizes[eid] = entry.uncompressed_size;
            arena.compressed_sizes[eid] = entry.compressed_size;
            arena.crc32s[eid] = entry.crc32;
            arena.modified_timestamps[eid] = entry.mtime;
            arena.modes[eid] = entry.mode;
            arena.node_attributes[eid] |= node_flags;
            arena.entry_indices[eid] = entry.entry_idx as u32;
            return;
        }
    }

    let node_id = arena.alloc_node(VfsNodeAllocParams {
        name: leaf_name,
        full_path: clean_path,
        uncompressed_size: entry.uncompressed_size,
        compressed_size: entry.compressed_size,
        crc32: entry.crc32,
        mtime: entry.mtime,
        mode: entry.mode,
        flags: node_flags,
        parent_id: curr_parent_id,
        entry_idx: Some(entry.entry_idx),
    });
    if is_dir {
        dir_map.insert(clean_path.to_string(), node_id);
    }
}

fn ensure_directory(
    arena: &mut VfsCompactArena,
    dir_map: &mut HashMap<String, u32, FxBuildHasher>,
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

    let path_start = parent_path.as_ptr() as usize;
    let mut p_id = root_id;
    for segment in parent_path.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        let seg_end = (segment.as_ptr() as usize + segment.len()) - path_start;
        let prefix = &parent_path[..seg_end];

        if let Some(&existing_id) = dir_map.get(prefix) {
            p_id = existing_id;
        } else {
            let new_dir_id = arena.alloc_dir_node(segment, prefix, mtime, p_id);
            dir_map.insert(prefix.to_string(), new_dir_id);
            p_id = new_dir_id;
        }
    }
    p_id
}
