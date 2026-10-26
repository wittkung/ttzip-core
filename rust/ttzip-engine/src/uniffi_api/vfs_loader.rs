// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Archive inspection and VfsCompactArena loader for UniFFI VFS sessions.

use std::path::Path;
use super::vfs::VfsSessionError;

pub(super) fn raw_path_implies_directory(raw_path: &str) -> bool {
    let trimmed = raw_path.trim_end_matches(['/', '\\']);
    raw_path.ends_with('/')
        || raw_path.ends_with('\\')
        || trimmed == "."
        || trimmed.ends_with("/.")
        || trimmed.ends_with("\\.")
}

pub(super) fn sanitize_archive_entry_path(raw_path: &str) -> Option<String> {
    let normalized = crate::fs::normalize_to_nfc(raw_path);
    let mut segments = Vec::new();
    for seg in normalized.split(['/', '\\']) {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            return None;
        }
        segments.push(seg);
    }
    if segments.is_empty() {
        None
    } else {
        Some(segments.join("/"))
    }
}

fn ensure_dir_in_arena(
    arena: &mut crate::fs::vfs::VfsCompactArena,
    dir_map: &mut std::collections::HashMap<String, u32>,
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

    let mut p_id = root_id;
    let mut current_prefix = String::with_capacity(parent_path.len());

    for segment in parent_path.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if !current_prefix.is_empty() {
            current_prefix.push('/');
        }
        current_prefix.push_str(segment);

        if let Some(&existing_id) = dir_map.get(&current_prefix) {
            p_id = existing_id;
        } else {
            let new_dir_id = arena.alloc_node(crate::fs::vfs::arena::VfsNodeAllocParams {
                name: segment,
                full_path: &current_prefix,
                uncompressed_size: 0,
                compressed_size: 0,
                crc32: 0,
                mtime,
                mode: 0o755,
                flags: crate::fs::vfs::arena::VFS_FLAG_IS_DIR,
                parent_id: p_id,
                entry_idx: None,
            });
            dir_map.insert(current_prefix.clone(), new_dir_id);
            p_id = new_dir_id;
        }
    }
    p_id
}

pub(crate) fn build_arena_from_archive_path(
    path: &Path,
    password: Option<&str>,
) -> Result<crate::fs::vfs::VfsCompactArena, VfsSessionError> {
    if !path.exists() {
        return Err(VfsSessionError::InspectionFailed {
            reason: format!("Archive file not found: {}", path.display()),
        });
    }

    let root_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("archive");

    let mut arena = crate::fs::vfs::VfsCompactArena::with_capacity(1024);
    let root_id = arena.alloc_node(crate::fs::vfs::arena::VfsNodeAllocParams {
        name: root_name,
        full_path: "",
        uncompressed_size: 0,
        compressed_size: 0,
        crc32: 0,
        mtime: 0,
        mode: 0o755,
        flags: crate::fs::vfs::arena::VFS_FLAG_IS_DIR,
        parent_id: crate::fs::vfs::arena::VFS_NULL_NODE,
        entry_idx: None,
    });

    let mut dir_map: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    dir_map.insert(String::new(), root_id);

    let secure_pwd = password.map(|p| crate::crypto::ZeroizingString::new(p.to_string()));
    let mut raw_entry_idx: usize = 0;

    let res = crate::archive::unified::inspect::inspect_archive(
        path,
        secure_pwd.as_deref(),
        true,
        |entry_ref| {
            let current_idx = raw_entry_idx;
            raw_entry_idx += 1;

            let Some(clean_path) = sanitize_archive_entry_path(entry_ref.path) else {
                return true;
            };
            let is_dir = entry_ref.is_directory || raw_path_implies_directory(entry_ref.path);

            let (parent_path, leaf_name) = match clean_path.rfind('/') {
                Some(idx) => (&clean_path[..idx], &clean_path[idx + 1..]),
                None => ("", clean_path.as_str()),
            };

            let curr_parent_id = ensure_dir_in_arena(
                &mut arena,
                &mut dir_map,
                parent_path,
                entry_ref.mtime_epoch_secs,
                root_id,
            );

            let mut flags = 0u8;
            if is_dir {
                flags |= crate::fs::vfs::arena::VFS_FLAG_IS_DIR;
            }
            if entry_ref.is_encrypted {
                flags |= crate::fs::vfs::arena::VFS_FLAG_IS_ENCRYPTED;
            }
            if (entry_ref.mode & 0o170000) == 0o120000 {
                flags |= crate::fs::vfs::arena::VFS_FLAG_IS_SYMLINK;
            }

            if is_dir {
                if let Some(&existing_id) = dir_map.get(&clean_path) {
                    let eid = existing_id as usize;
                    arena.uncompressed_sizes[eid] = entry_ref.uncompressed_size;
                    arena.compressed_sizes[eid] = entry_ref.compressed_size;
                    arena.crc32s[eid] = entry_ref.crc32;
                    arena.modified_timestamps[eid] = entry_ref.mtime_epoch_secs;
                    arena.modes[eid] = entry_ref.mode;
                    arena.node_attributes[eid] |= flags;
                    arena.entry_indices[eid] = current_idx as u32;
                    return true;
                }
            }

            let node_id = arena.alloc_node(crate::fs::vfs::arena::VfsNodeAllocParams {
                name: leaf_name,
                full_path: &clean_path,
                uncompressed_size: entry_ref.uncompressed_size,
                compressed_size: entry_ref.compressed_size,
                crc32: entry_ref.crc32,
                mtime: entry_ref.mtime_epoch_secs,
                mode: entry_ref.mode,
                flags,
                parent_id: curr_parent_id,
                entry_idx: Some(current_idx),
            });
            if is_dir {
                dir_map.insert(clean_path, node_id);
            }
            true
        },
    );

    match res {
        Ok(_) => {
            arena.finalize_tree();
            Ok(arena)
        }
        Err(crate::types::TTZipStatus::ErrInvalidPassword) => Err(VfsSessionError::BadPassword),
        Err(status) => Err(VfsSessionError::InspectionFailed {
            reason: format!("Archive inspection failed with status: {:?}", status),
        }),
    }
}
