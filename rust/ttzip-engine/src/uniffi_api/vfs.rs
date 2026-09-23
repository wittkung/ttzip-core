// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Mozilla UniFFI In-Memory VFS Tree Object Scaffolding.

use std::sync::Arc;
use super::types::{UniFFIEntryMetadata, UniFFIVfsMatch, UniFFIVfsNodeSummary, UniFFIVfsPagedResult, UniFFIVfsStats};

/// Thread-safe in-memory VFS Tree object exposed to Swift and multi-language SDKs.
#[derive(uniffi::Object)]
pub struct UniFFIVfsTree {
    arena: parking_lot::RwLock<crate::fs::vfs::VfsArena>,
    total_entries: usize,
}

#[uniffi::export]
impl UniFFIVfsTree {
    #[uniffi::constructor]
    pub fn build(entries: Vec<UniFFIEntryMetadata>, root_name: String) -> Arc<Self> {
        let total_entries = entries.len();
        let vfs_entries: Vec<crate::fs::vfs::node::VfsEntry> = entries
            .into_iter()
            .map(|e| crate::fs::vfs::node::VfsEntry {
                path: e.path,
                uncompressed_size: e.uncompressed_size,
                compressed_size: e.compressed_size,
                crc32: e.crc32,
                mtime_epoch_secs: e.mtime_epoch_secs,
                mode: e.mode,
                is_directory: e.is_directory,
                is_encrypted: e.is_encrypted,
            })
            .collect();

        let arena = crate::fs::vfs::VfsArena::build_from_entries(&vfs_entries, &root_name);
        Arc::new(Self {
            arena: parking_lot::RwLock::new(arena),
            total_entries,
        })
    }

    pub fn search(&self, query: String, max_results: u32) -> Vec<UniFFIVfsMatch> {
        let guard = self.arena.read();
        let limit = (max_results as usize).clamp(1, 1024);
        let mut out_matches = vec![crate::fs::vfs::search::TTZipVfsMatchDto::default(); limit];
        let count = guard.search_zero_alloc(&query, &mut out_matches);
        out_matches[..count]
            .iter()
            .map(|m| {
                let name = if !m.name.is_null() && m.name_len > 0 {
                    let bytes = unsafe { std::slice::from_raw_parts(m.name as *const u8, m.name_len) };
                    String::from_utf8_lossy(bytes).into_owned()
                } else {
                    String::new()
                };
                let path = if !m.path.is_null() && m.path_len > 0 {
                    let bytes = unsafe { std::slice::from_raw_parts(m.path as *const u8, m.path_len) };
                    String::from_utf8_lossy(bytes).into_owned()
                } else {
                    String::new()
                };
                UniFFIVfsMatch {
                    path,
                    name,
                    is_directory: m.is_directory,
                    size: m.uncompressed_size,
                }
            })
            .collect()
    }

    pub fn get_children(&self, subpath: Option<String>, offset: u32, limit: u32) -> Vec<UniFFIVfsNodeSummary> {
        self.get_children_paged(subpath, offset, limit).nodes
    }

    pub fn get_children_paged(&self, subpath: Option<String>, offset: u32, limit: u32) -> UniFFIVfsPagedResult {
        let guard = self.arena.read();
        let target_node_id = if let Some(ref sp) = subpath {
            let clean = sp.trim_matches('/');
            if clean.is_empty() {
                0
            } else {
                let mut curr = 0;
                let mut not_found = false;
                for seg in clean.split('/').filter(|s| !s.is_empty()) {
                    let mut found_child = crate::fs::vfs::arena::VFS_NULL_NODE;
                    let mut child = guard.first_child[curr as usize];
                    let seg_bytes = seg.as_bytes();
                    while child != crate::fs::vfs::arena::VFS_NULL_NODE {
                        let cid = child as usize;
                        let is_dir = (guard.flags[cid] & crate::fs::vfs::arena::VFS_FLAG_IS_DIR) != 0;
                        if is_dir {
                            let noff = guard.name_offsets[cid] as usize;
                            let nlen = guard.name_lens[cid] as usize;
                            if &guard.string_arena[noff..noff + nlen] == seg_bytes {
                                found_child = child;
                                break;
                            }
                        }
                        child = guard.next_sibling[cid];
                    }
                    if found_child == crate::fs::vfs::arena::VFS_NULL_NODE {
                        not_found = true;
                        break;
                    }
                    curr = found_child;
                }
                if not_found {
                    return UniFFIVfsPagedResult {
                        nodes: Vec::new(),
                        total_count: 0,
                    };
                }
                curr
            }
        } else {
            0
        };

        if target_node_id as usize >= guard.total_nodes {
            return UniFFIVfsPagedResult {
                nodes: Vec::new(),
                total_count: 0,
            };
        }

        let total_in_dir = guard.child_counts[target_node_id as usize] as usize;
        let offset = offset as usize;
        let limit = limit as usize;

        if total_in_dir == 0 || offset >= total_in_dir || limit == 0 {
            return UniFFIVfsPagedResult {
                nodes: Vec::new(),
                total_count: total_in_dir as u32,
            };
        }

        let start_idx = offset;
        let end_idx = (start_idx + limit).min(total_in_dir);
        // In VfsArena, children are chained via head-insertion (LIFO).
        // A linked list of N items has node at list index k corresponding to forward index (N - 1 - k).
        // Thus, forward index range [start_idx, end_idx) corresponds to list index range:
        // k in [total_in_dir - end_idx, total_in_dir - 1 - start_idx].
        let k_start = total_in_dir - end_idx;
        let k_end = total_in_dir - 1 - start_idx;

        let mut nodes = Vec::with_capacity(end_idx - start_idx);
        let mut curr_child = guard.first_child[target_node_id as usize];
        let mut k = 0usize;

        while curr_child != crate::fs::vfs::arena::VFS_NULL_NODE {
            if k > k_end {
                break;
            }
            if k >= k_start {
                let cid = curr_child as usize;
                let noff = guard.name_offsets[cid] as usize;
                let nlen = guard.name_lens[cid] as usize;
                let poff = guard.full_path_offsets[cid] as usize;
                let plen = guard.full_path_lens[cid] as usize;

                let name = std::str::from_utf8(&guard.string_arena[noff..noff + nlen])
                    .unwrap_or("")
                    .to_string();
                let path = std::str::from_utf8(&guard.string_arena[poff..poff + plen])
                    .unwrap_or("")
                    .to_string();

                let is_dir = (guard.flags[cid] & crate::fs::vfs::arena::VFS_FLAG_IS_DIR) != 0;
                let is_enc = (guard.flags[cid] & crate::fs::vfs::arena::VFS_FLAG_IS_ENCRYPTED) != 0;
                let has_children = guard.child_counts[cid] > 0;

                nodes.push(UniFFIVfsNodeSummary {
                    name,
                    path,
                    uncompressed_size: guard.uncompressed_sizes[cid],
                    compressed_size: guard.compressed_sizes[cid],
                    crc32: guard.crc32s[cid],
                    mtime_epoch_secs: guard.mtimes[cid],
                    mode: guard.modes[cid],
                    is_directory: is_dir,
                    is_encrypted: is_enc,
                    has_children,
                });
            }
            curr_child = guard.next_sibling[curr_child as usize];
            k += 1;
        }

        nodes.reverse();

        UniFFIVfsPagedResult {
            nodes,
            total_count: total_in_dir as u32,
        }
    }

    pub fn render_tree(&self) -> String {
        let guard = self.arena.read();
        let mut out = String::new();
        render_arena_node(&guard, 0, "", true, &mut out);
        out
    }

    pub fn get_stats(&self) -> UniFFIVfsStats {
        let guard = self.arena.read();
        let (files, dirs, bytes) = guard.get_stats();
        UniFFIVfsStats {
            total_files: files,
            total_dirs: dirs,
            total_uncompressed_bytes: bytes,
        }
    }

    pub fn total_entries(&self) -> u64 {
        self.total_entries as u64
    }
}

fn render_arena_node(
    arena: &crate::fs::vfs::VfsArena,
    node_id: u32,
    prefix: &str,
    is_last: bool,
    out: &mut String,
) {
    if node_id as usize >= arena.total_nodes {
        return;
    }
    let nid = node_id as usize;
    let noff = arena.name_offsets[nid] as usize;
    let nlen = arena.name_lens[nid] as usize;
    let name = std::str::from_utf8(&arena.string_arena[noff..noff + nlen]).unwrap_or("");
    let display_name = if name.is_empty() { "." } else { name };

    let is_dir = (arena.flags[nid] & crate::fs::vfs::arena::VFS_FLAG_IS_DIR) != 0;
    let size_str = if is_dir {
        "<DIR>".to_string()
    } else {
        crate::fs::vfs::node::format_byte_size(arena.uncompressed_sizes[nid])
    };

    if prefix.is_empty() {
        out.push_str(&format!("{} ({})\n", display_name, size_str));
    } else {
        let connector = if is_last { "└── " } else { "├── " };
        out.push_str(&format!("{}{}{} ({})\n", prefix, connector, display_name, size_str));
    }

    let child_prefix = if prefix.is_empty() {
        String::new()
    } else if is_last {
        format!("{}    ", prefix)
    } else {
        format!("{}│   ", prefix)
    };

    let child_count = arena.child_counts[nid] as usize;
    let mut children = Vec::with_capacity(child_count);
    let mut curr = arena.first_child[nid];
    while curr != crate::fs::vfs::arena::VFS_NULL_NODE {
        children.push(curr);
        curr = arena.next_sibling[curr as usize];
    }
    children.reverse();

    for (i, &child_id) in children.iter().enumerate() {
        render_arena_node(arena, child_id, &child_prefix, i + 1 == child_count, out);
    }
}

/// Loads and constructs a high-performance in-memory VFS tree directly from an archive header
/// into continuous memory VfsArena and VfsTree.
#[uniffi::export]
pub fn load_archive_vfs_tree(
    archive_path: String,
    password: Option<String>,
) -> Result<Arc<UniFFIVfsTree>, super::types::TTZipError> {
    let p = std::path::Path::new(&archive_path);
    if !p.exists() {
        return Err(super::types::TTZipError::FileNotFound {
            path: archive_path,
        });
    }

    let root_name = p
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("archive")
        .to_string();

    let entries = super::archive::inspect_archive_entries(archive_path, password)?;
    let total_entries = entries.len();
    let vfs_entries: Vec<crate::fs::vfs::node::VfsEntry> = entries
        .into_iter()
        .map(|e| crate::fs::vfs::node::VfsEntry {
            path: e.path,
            uncompressed_size: e.uncompressed_size,
            compressed_size: e.compressed_size,
            crc32: e.crc32,
            mtime_epoch_secs: e.mtime_epoch_secs,
            mode: e.mode,
            is_directory: e.is_directory,
            is_encrypted: e.is_encrypted,
        })
        .collect();

    let arena = crate::fs::vfs::VfsArena::build_from_entries(&vfs_entries, &root_name);

    Ok(Arc::new(UniFFIVfsTree {
        arena: parking_lot::RwLock::new(arena),
        total_entries,
    }))
}

fn map_nested_err(e: crate::archive::nested_vfs::NestedVfsError) -> super::types::TTZipError {
    match e {
        crate::archive::nested_vfs::NestedVfsError::FileNotFound { path } => {
            super::types::TTZipError::FileNotFound { path }
        }
        crate::archive::nested_vfs::NestedVfsError::InvalidPassword => {
            super::types::TTZipError::InvalidPassword
        }
        crate::archive::nested_vfs::NestedVfsError::CorruptHeader { details, offset } => {
            super::types::TTZipError::CorruptHeader { details, offset }
        }
        crate::archive::nested_vfs::NestedVfsError::SecurityViolation { reason } => {
            super::types::TTZipError::SecurityViolation { reason }
        }
        crate::archive::nested_vfs::NestedVfsError::Cancelled => {
            super::types::TTZipError::Cancelled
        }
        crate::archive::nested_vfs::NestedVfsError::EngineError { code } => {
            super::types::TTZipError::EngineError { code }
        }
        crate::archive::nested_vfs::NestedVfsError::Status(s) => {
            super::types::TTZipError::EngineError { code: s.to_i32() }
        }
        crate::archive::nested_vfs::NestedVfsError::IoError { message } => {
            super::types::TTZipError::IoError { message }
        }
    }
}

/// Thread-safe bounded in-memory virtual file stream exposed to UniFFI.
#[derive(uniffi::Object)]
pub struct VirtualFileStream {
    pub(crate) inner: Arc<crate::archive::nested_vfs::VirtualFileStream>,
}

#[uniffi::export]
impl VirtualFileStream {
    #[uniffi::constructor]
    pub fn new_empty() -> Arc<Self> {
        Arc::new(Self {
            inner: crate::archive::nested_vfs::VirtualFileStream::new_empty(),
        })
    }

    pub fn size(&self) -> u64 {
        self.inner.size()
    }

    pub fn position(&self) -> u64 {
        self.inner.position()
    }

    pub fn seek(&self, offset: u64) -> Result<u64, super::types::TTZipError> {
        self.inner.seek(offset).map_err(map_nested_err)
    }

    pub fn read(&self, max_bytes: u32) -> Result<Vec<u8>, super::types::TTZipError> {
        self.inner.read(max_bytes).map_err(map_nested_err)
    }

    pub fn read_exact_at(&self, offset: u64, length: u32) -> Result<Vec<u8>, super::types::TTZipError> {
        self.inner.read_exact_at(offset, length).map_err(map_nested_err)
    }

    pub fn read_all(&self) -> Result<Vec<u8>, super::types::TTZipError> {
        self.inner.read_all().map_err(map_nested_err)
    }
}

/// Recursively drills down nested archives in-memory and returns inner archive entries.
#[uniffi::export]
pub fn drill_down_nested_archive(
    archive_path: String,
    drill_path: Vec<String>,
    password: Option<String>,
) -> Result<Vec<UniFFIEntryMetadata>, super::types::TTZipError> {
    let entries = crate::archive::nested_vfs::drill_down_nested_archive(&archive_path, &drill_path, password.as_deref())
        .map_err(map_nested_err)?;
    Ok(entries.into_iter().map(Into::into).collect())
}

/// Recursively drills down nested archives in-memory and returns a streaming VirtualFileStream.
#[uniffi::export]
pub fn open_virtual_file_stream(
    archive_path: String,
    drill_path: Vec<String>,
    target_entry: String,
    password: Option<String>,
) -> Result<Arc<VirtualFileStream>, super::types::TTZipError> {
    let stream = crate::archive::nested_vfs::open_virtual_file_stream(&archive_path, &drill_path, &target_entry, password.as_deref())
        .map_err(map_nested_err)?;
    Ok(Arc::new(VirtualFileStream { inner: stream }))
}

