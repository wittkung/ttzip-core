// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use std::sync::Arc;
use std::collections::HashMap;
use parking_lot::RwLock;

use super::vfs::{VfsNodeDetail, VfsPageResult, VfsPagedNode, VfsSessionError};
use crate::fs::vfs::arena::VFS_FLAG_IS_DIR;
use crate::fs::normalize_to_nfc;
use crate::fs::vfs::VfsCompactArena;

/// Sorting keys for windowed paging.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VfsSortKey {
    Name,
    Size,
    ModifiedTime,
    Extension,
}

/// Sorting direction.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VfsSortDirection {
    Ascending,
    Descending,
}

pub type VfsSortCache = RwLock<HashMap<(u32, VfsSortKey, VfsSortDirection), Arc<Vec<u32>>>>;

#[inline]
fn extract_ext(name: &str) -> &str {
    if let Some(pos) = name.rfind('.') {
        &name[pos + 1..]
    } else {
        ""
    }
}

pub fn vfs_get_children_paged_sorted_core(
    arena: &VfsCompactArena,
    sort_cache: &VfsSortCache,
    parent_id: u32,
    offset: u32,
    limit: u32,
    sort_by: VfsSortKey,
    direction: VfsSortDirection,
) -> Result<VfsPageResult, VfsSessionError> {
    let pid = parent_id as usize;
    if pid >= arena.total_nodes {
        return Err(VfsSessionError::NodeNotFound { node_id: parent_id });
    }

    let (child_start, total_children) = arena.child_slice_ranges[pid];
    let offset_usize = offset as usize;
    let limit_usize = limit as usize;

    if total_children == 0 || offset_usize >= total_children as usize || limit_usize == 0 {
        return Ok(VfsPageResult {
            total_count: total_children,
            offset,
            nodes: Vec::new(),
        });
    }

    let take = limit_usize.min((total_children as usize) - offset_usize);
    let raw_children = &arena.sorted_child_pool[child_start as usize..(child_start + total_children) as usize];

    let paged_ids: Vec<u32> = if sort_by == VfsSortKey::Name && direction == VfsSortDirection::Ascending {
        // Direct O(1) slice (zero-copy order)
        raw_children[offset_usize..offset_usize + take].to_vec()
    } else {
        // Permutation vector via pdqsort with caching
        let key = (parent_id, sort_by, direction);
        let perm_arc = {
            let cache_read = sort_cache.read();
            cache_read.get(&key).cloned()
        };

        let perm = match perm_arc {
            Some(arc) => arc,
            None => {
                let mut p: Vec<u32> = (0..total_children).collect();
                p.sort_unstable_by(|&a_idx, &b_idx| {
                    let cid_a = raw_children[a_idx as usize] as usize;
                    let cid_b = raw_children[b_idx as usize] as usize;

                    // Folders always precede files (Finder UX invariant)
                    let is_dir_a = (arena.node_attributes[cid_a] & VFS_FLAG_IS_DIR) != 0;
                    let is_dir_b = (arena.node_attributes[cid_b] & VFS_FLAG_IS_DIR) != 0;
                    if is_dir_a != is_dir_b {
                        return if is_dir_a {
                            core::cmp::Ordering::Less
                        } else {
                            core::cmp::Ordering::Greater
                        };
                    }

                    let ord = match sort_by {
                        VfsSortKey::Name => {
                            arena.node_name(cid_a as u32).cmp(arena.node_name(cid_b as u32))
                        }
                        VfsSortKey::Size => {
                            arena.uncompressed_sizes[cid_a].cmp(&arena.uncompressed_sizes[cid_b])
                        }
                        VfsSortKey::ModifiedTime => {
                            arena.modified_timestamps[cid_a].cmp(&arena.modified_timestamps[cid_b])
                        }
                        VfsSortKey::Extension => {
                            let ext_a = extract_ext(arena.node_name(cid_a as u32));
                            let ext_b = extract_ext(arena.node_name(cid_b as u32));
                            ext_a.cmp(ext_b)
                        }
                    };

                    let ord = match direction {
                        VfsSortDirection::Ascending => ord,
                        VfsSortDirection::Descending => ord.reverse(),
                    };

                    if ord == core::cmp::Ordering::Equal {
                        arena.node_name(cid_a as u32).cmp(arena.node_name(cid_b as u32))
                    } else {
                        ord
                    }
                });

                let arc = Arc::new(p);
                let mut cache_write = sort_cache.write();
                if cache_write.len() >= 64 {
                    cache_write.clear();
                }
                cache_write.insert(key, Arc::clone(&arc));
                arc
            }
        };

        perm[offset_usize..offset_usize + take]
            .iter()
            .map(|&idx| raw_children[idx as usize])
            .collect()
    };

    let mut nodes = Vec::with_capacity(paged_ids.len());
    for cid in paged_ids {
        let cidx = cid as usize;
        let name = arena.node_name(cid).to_string();
        let attr = arena.node_attributes[cidx];
        let is_dir = (attr & VFS_FLAG_IS_DIR) != 0;
        let is_symlink = (arena.modes[cidx] & 0o170000) == 0o120000;
        let is_encrypted = (attr & crate::fs::vfs::arena::VFS_FLAG_IS_ENCRYPTED) != 0;
        let size = arena.uncompressed_sizes[cidx];
        let compressed_size = arena.compressed_sizes[cidx];
        let mtime_secs = arena.modified_timestamps[cidx];

        nodes.push(VfsPagedNode {
            node_id: cid,
            name,
            path: arena.node_full_path(cid),
            is_dir,
            is_symlink,
            is_encrypted,
            size,
            compressed_size,
            mtime_secs,
        });
    }

    Ok(VfsPageResult {
        total_count: total_children,
        offset,
        nodes,
    })
}

pub(crate) fn vfs_get_children_paged_core(
    arena: &crate::fs::vfs::VfsCompactArena,
    parent_id: u32,
    offset: u32,
    limit: u32,
) -> Result<VfsPageResult, VfsSessionError> {
    let pid = parent_id as usize;
    if pid >= arena.total_nodes {
        return Err(VfsSessionError::NodeNotFound { node_id: parent_id });
    }

    let total_children = arena.child_slice_ranges[pid].1;
    let offset_usize = offset as usize;
    let limit_usize = limit as usize;

    if total_children == 0 || offset_usize >= total_children as usize || limit_usize == 0 {
        return Ok(VfsPageResult {
            total_count: total_children,
            offset,
            nodes: Vec::new(),
        });
    }

    let paged_ids = arena.get_children_paged(parent_id, offset_usize, limit_usize);
    let mut nodes = Vec::with_capacity(paged_ids.len());

    for &cid in paged_ids {
        let cidx = cid as usize;
        let name = arena.node_name(cid).to_string();
        let attr = arena.node_attributes[cidx];
        let is_dir = (attr & crate::fs::vfs::arena::VFS_FLAG_IS_DIR) != 0;
        let is_symlink = (arena.modes[cidx] & 0o170000) == 0o120000;
        let is_encrypted = (attr & crate::fs::vfs::arena::VFS_FLAG_IS_ENCRYPTED) != 0;
        let size = arena.uncompressed_sizes[cidx];
        let compressed_size = arena.compressed_sizes[cidx];
        let mtime_secs = arena.modified_timestamps[cidx];

        nodes.push(VfsPagedNode {
            node_id: cid,
            name,
            path: arena.node_full_path(cid),
            is_dir,
            is_symlink,
            is_encrypted,
            size,
            compressed_size,
            mtime_secs,
        });
    }

    Ok(VfsPageResult {
        total_count: total_children,
        offset,
        nodes,
    })
}

pub(crate) fn vfs_resolve_path_core(
    arena: &crate::fs::vfs::VfsCompactArena,
    path: &str,
) -> Result<u32, VfsSessionError> {
    let normalized = normalize_to_nfc(path);
    let clean = normalized.replace('\\', "/");
    let trimmed = clean.trim_matches('/');
    if trimmed.is_empty() {
        return Ok(0);
    }

    let mut curr = 0u32;
    for segment in trimmed.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            return Err(VfsSessionError::PathTraversalDetected {
                attempted_path: path.to_string(),
            });
        }

        let children = arena.get_children(curr);
        let mut found = None;
        for &cid in children {
            if arena.node_name(cid) == segment {
                found = Some(cid);
                break;
            }
        }
        match found {
            Some(next_id) => curr = next_id,
            None => return Err(VfsSessionError::NodeNotFound { node_id: curr }),
        }
    }

    Ok(curr)
}

pub(crate) fn vfs_get_node_detail_core(
    arena: &crate::fs::vfs::VfsCompactArena,
    node_id: u32,
) -> Result<VfsNodeDetail, VfsSessionError> {
    let nid = node_id as usize;
    if nid >= arena.total_nodes {
        return Err(VfsSessionError::NodeNotFound { node_id });
    }

    let name = arena.node_name(node_id).to_string();
    let full_path = arena.node_full_path(node_id);
    let attr = arena.node_attributes[nid];
    let is_dir = (attr & crate::fs::vfs::arena::VFS_FLAG_IS_DIR) != 0;
    let is_symlink = (arena.modes[nid] & 0o170000) == 0o120000;
    let symlink_target = None;
    let is_encrypted = (attr & crate::fs::vfs::arena::VFS_FLAG_IS_ENCRYPTED) != 0;
    let size = arena.uncompressed_sizes[nid];
    let compressed_size = arena.compressed_sizes[nid];
    let mtime_secs = arena.modified_timestamps[nid];
    let crc32 = arena.crc32s[nid];
    let mode = arena.modes[nid];

    Ok(VfsNodeDetail {
        node_id,
        full_path,
        name,
        is_dir,
        is_symlink,
        symlink_target,
        is_encrypted,
        size,
        compressed_size,
        mtime_secs,
        crc32,
        mode,
    })
}
