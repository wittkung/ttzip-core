// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Mozilla UniFFI In-Memory VFS Tree & Windowed Paging Session Object Scaffolding.

use std::path::Path;
use std::sync::Arc;
pub(crate) use super::vfs_loader::build_arena_from_archive_path;
use super::types::{UniFFIEntryMetadata, UniFFIVfsMatch, UniFFIVfsNodeSummary, UniFFIVfsPagedResult, UniFFIVfsStats};

/// Lightweight node entry in a windowed paging result.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct VfsPagedNode {
    pub node_id: u32,
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub is_encrypted: bool,
    pub size: u64,
    pub compressed_size: u64,
    pub mtime_secs: i64,
}

/// Windowed paging result containing total count, current offset, and slice nodes.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct VfsPageResult {
    pub total_count: u32,
    pub offset: u32,
    pub nodes: Vec<VfsPagedNode>,
}

/// Detailed node attributes and metadata.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct VfsNodeDetail {
    pub node_id: u32,
    pub full_path: String,
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub symlink_target: Option<String>,
    pub is_encrypted: bool,
    pub size: u64,
    pub compressed_size: u64,
    pub mtime_secs: i64,
    pub crc32: u32,
    pub mode: u32,
}

/// Typed VFS session errors mapped directly to Swift throws.
#[derive(uniffi::Error, thiserror::Error, Debug)]
pub enum VfsSessionError {
    #[error("Node ID not found: {node_id}")]
    NodeNotFound { node_id: u32 },
    #[error("Path traversal attack detected: {attempted_path}")]
    PathTraversalDetected { attempted_path: String },
    #[error("Archive inspection failed: {reason}")]
    InspectionFailed { reason: String },
    #[error("Bad password or password required")]
    BadPassword,
    #[error("Internal memory corruption or boundary violation")]
    InternalError,
    #[error("Invalid embedding vector: expected 128 dimensions, got {actual}")]
    InvalidVectorDimension { actual: u32 },
    #[error("Embeddings not available in current session")]
    EmbeddingsUnavailable,
}

pub use super::vfs_search::{VfsHybridMatch, VfsSemanticSearchOptions};

/// Thread-safe in-memory VFS session.
#[derive(uniffi::Object)]
pub struct VfsSession {
    arena: parking_lot::RwLock<crate::fs::vfs::VfsCompactArena>,
    sort_cache: super::vfs_paging::VfsSortCache,
    total_entries: usize,
    embeddings: parking_lot::RwLock<Option<Vec<ttzip_types::snapshot::CompactVector128>>>,
}

impl std::fmt::Debug for VfsSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VfsSession")
            .field("total_entries", &self.total_entries)
            .finish()
    }
}

#[uniffi::export]
impl VfsSession {
    pub fn total_nodes(&self) -> u32 {
        self.total_entries as u32
    }

    pub fn get_children_paged(
        &self,
        parent_id: u32,
        offset: u32,
        limit: u32,
    ) -> Result<VfsPageResult, VfsSessionError> {
        let guard = self.arena.read();
        super::vfs_paging::vfs_get_children_paged_core(&guard, parent_id, offset, limit)
    }

    pub fn get_children_paged_sorted(
        &self,
        parent_id: u32,
        offset: u32,
        limit: u32,
        sort_by: super::vfs_paging::VfsSortKey,
        direction: super::vfs_paging::VfsSortDirection,
    ) -> Result<VfsPageResult, VfsSessionError> {
        let guard = self.arena.read();
        super::vfs_paging::vfs_get_children_paged_sorted_core(
            &guard,
            &self.sort_cache,
            parent_id,
            offset,
            limit,
            sort_by,
            direction,
        )
    }

    pub fn resolve_path(&self, path: &str) -> Result<u32, VfsSessionError> {
        let guard = self.arena.read();
        super::vfs_paging::vfs_resolve_path_core(&guard, path)
    }

    pub fn get_node_detail(&self, node_id: u32) -> Result<VfsNodeDetail, VfsSessionError> {
        let guard = self.arena.read();
        super::vfs_paging::vfs_get_node_detail_core(&guard, node_id)
    }

    pub fn search_top_k(&self, query: &str, limit: u32) -> Result<Vec<VfsPagedNode>, VfsSessionError> {
        let guard = self.arena.read();
        super::vfs_search::vfs_search_top_k_core(&guard, query, limit)
    }

    /// Executes a hybrid search combining lexical fzf Bloom pruning
    /// and NEON SIMD vector dot product convergence.
    pub fn search_hybrid_top_k(
        &self,
        query: String,
        query_vector: Option<Vec<i8>>,
        limit: u32,
        lexical_weight: f32,
        semantic_weight: f32,
    ) -> Result<Vec<VfsHybridMatch>, VfsSessionError> {
        let guard = self.arena.read();
        let emb_guard = self.embeddings.read();
        super::vfs_search::vfs_search_hybrid_top_k_core(
            &guard,
            emb_guard.as_deref(),
            &query,
            query_vector.as_deref(),
            limit,
            lexical_weight,
            semantic_weight,
        )
    }

    /// Fast-path semantic-only top-K search over pre-computed embeddings.
    pub fn search_semantic_top_k(
        &self,
        query_vector: Vec<i8>,
        limit: u32,
    ) -> Result<Vec<VfsPagedNode>, VfsSessionError> {
        let guard = self.arena.read();
        let emb_guard = self.embeddings.read();
        super::vfs_search::vfs_search_semantic_top_k_core(
            &guard,
            emb_guard.as_deref(),
            &query_vector,
            limit,
        )
    }

    /// Returns true if the active snapshot contains an embedded vector section (TT_SECTION_TYPE_EMBEDDINGS).
    pub fn has_vector_embeddings(&self) -> bool {
        self.embeddings.read().as_ref().is_some_and(|e| !e.is_empty())
    }

    /// Attaches an existing .ttvec sidecar cache file if present and valid.
    pub fn attach_vector_sidecar(&self, sidecar_path: String) -> Result<bool, VfsSessionError> {
        let path = Path::new(&sidecar_path);
        if !path.exists() {
            return Ok(false);
        }
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(_) => return Ok(false),
        };
        if data.len() < 64 {
            return Ok(false);
        }
        let magic = u32::from_le_bytes(data[0..4].try_into().unwrap());
        if magic != 0x54545645 {
            return Ok(false);
        }
        let entry_count = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
        let expected_len = 64 + entry_count * 128;
        if data.len() < expected_len {
            return Ok(false);
        }

        let mut vectors = Vec::with_capacity(entry_count);
        for i in 0..entry_count {
            let offset = 64 + i * 128;
            let mut arr = [0i8; 128];
            for j in 0..128 {
                arr[j] = data[offset + j] as i8;
            }
            vectors.push(ttzip_types::snapshot::CompactVector128 { data: arr });
        }

        *self.embeddings.write() = Some(vectors);
        Ok(true)
    }

    /// Ingests a batch of pre-computed 128-dimensional INT8 vectors into the active session.
    pub fn ingest_vector_batch(
        &self,
        batch: VfsVectorIngestBatch,
    ) -> Result<u32, VfsSessionError> {
        if batch.flattened_vectors.len() != (batch.count as usize * 128) {
            let actual = if batch.count > 0 {
                (batch.flattened_vectors.len() / batch.count as usize) as u32
            } else {
                batch.flattened_vectors.len() as u32
            };
            return Err(VfsSessionError::InvalidVectorDimension { actual });
        }

        let mut emb_guard = self.embeddings.write();
        let vectors = emb_guard.get_or_insert_with(|| {
            vec![ttzip_types::snapshot::CompactVector128::zero(); self.total_entries]
        });

        let start = batch.start_node_id as usize;
        let count = batch.count as usize;
        let end = (start + count).min(vectors.len());

        for i in 0..(end - start) {
            let offset = i * 128;
            let mut arr = [0i8; 128];
            arr.copy_from_slice(&batch.flattened_vectors[offset..offset + 128]);
            vectors[start + i] = ttzip_types::snapshot::CompactVector128 { data: arr };
        }

        Ok((end - start) as u32)
    }

    /// Queries the real-time vector embedding indexing health.
    pub fn get_ingest_status(&self) -> VfsIngestStatus {
        let guard = self.embeddings.read();
        match guard.as_ref() {
            Some(vecs) => VfsIngestStatus {
                has_embeddings: !vecs.is_empty(),
                total_nodes: self.total_entries as u32,
                indexed_nodes: vecs.len() as u32,
                is_complete: vecs.len() == self.total_entries,
            },
            None => VfsIngestStatus {
                has_embeddings: false,
                total_nodes: self.total_entries as u32,
                indexed_nodes: 0,
                is_complete: false,
            },
        }
    }
}

/// Batch vector payload for streaming ingestion across FFI.
#[derive(uniffi::Record, Clone, Debug)]
pub struct VfsVectorIngestBatch {
    pub start_node_id: u32,
    pub flattened_vectors: Vec<i8>,
    pub count: u32,
}

/// Health and coverage status of VFS vector indexing.
#[derive(uniffi::Record, Clone, Debug)]
pub struct VfsIngestStatus {
    pub has_embeddings: bool,
    pub total_nodes: u32,
    pub indexed_nodes: u32,
    pub is_complete: bool,
}

/// Ingests and opens an archive into a persistent in-memory VfsSession.
#[uniffi::export]
pub fn open_vfs_session(
    archive_path: String,
    password: Option<String>,
) -> Result<Arc<VfsSession>, VfsSessionError> {
    let p = Path::new(&archive_path);
    let arena = super::vfs_loader::build_arena_from_archive_path(p, password.as_deref())?;
    Ok(Arc::new(VfsSession::from_arena(arena)))
}

impl VfsSession {
    pub fn from_arena(arena: crate::fs::vfs::VfsCompactArena) -> Self {
        Self::from_arena_with_embeddings(arena, None)
    }

    pub fn from_arena_with_embeddings(
        arena: crate::fs::vfs::VfsCompactArena,
        embeddings: Option<Vec<ttzip_types::snapshot::CompactVector128>>,
    ) -> Self {
        let total_entries = arena.total_nodes;
        Self {
            arena: parking_lot::RwLock::new(arena),
            sort_cache: parking_lot::RwLock::new(std::collections::HashMap::new()),
            total_entries,
            embeddings: parking_lot::RwLock::new(embeddings),
        }
    }

    pub fn set_embeddings(&self, embeddings: Vec<ttzip_types::snapshot::CompactVector128>) {
        *self.embeddings.write() = Some(embeddings);
    }

    pub fn from_entries(entries: &[crate::fs::vfs::VfsEntry], root_name: &str) -> Self {
        let arena = crate::fs::vfs::VfsCompactArena::build_from_entries(entries, root_name);
        Self::from_arena(arena)
    }
}


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
        let vfs_entries: Vec<crate::fs::vfs::VfsEntry> = entries
            .into_iter()
            .filter_map(|e| {
                let is_dir = e.is_directory || super::vfs_loader::raw_path_implies_directory(&e.path);
                super::vfs_loader::sanitize_archive_entry_path(&e.path).map(|path| crate::fs::vfs::VfsEntry {
                    path,
                    uncompressed_size: e.uncompressed_size,
                    compressed_size: e.compressed_size,
                    crc32: e.crc32,
                    mtime_epoch_secs: e.mtime_epoch_secs,
                    mode: e.mode,
                    is_directory: is_dir,
                    is_encrypted: e.is_encrypted,
                })
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
            match super::vfs_paging::vfs_resolve_path_core(&guard, sp) {
                Ok(id) => id,
                Err(_) => {
                    return UniFFIVfsPagedResult {
                        nodes: Vec::new(),
                        total_count: 0,
                    };
                }
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

        let total_in_dir = guard.child_slice_ranges[target_node_id as usize].1 as usize;
        let offset = offset as usize;
        let limit = limit as usize;

        if total_in_dir == 0 || offset >= total_in_dir || limit == 0 {
            return UniFFIVfsPagedResult {
                nodes: Vec::new(),
                total_count: total_in_dir as u32,
            };
        }

        let paged_ids = guard.get_children_paged(target_node_id, offset, limit);
        let mut nodes = Vec::with_capacity(paged_ids.len());

        for &cid_u32 in paged_ids {
            let cid = cid_u32 as usize;
            let name = guard.node_name(cid_u32).to_string();
            let path = guard.node_full_path(cid_u32);
            let attr = guard.node_attributes[cid];
            let is_dir = (attr & crate::fs::vfs::arena::VFS_FLAG_IS_DIR) != 0;
            let is_enc = (attr & crate::fs::vfs::arena::VFS_FLAG_IS_ENCRYPTED) != 0;
            let has_children = guard.child_slice_ranges[cid].1 > 0;

            nodes.push(UniFFIVfsNodeSummary {
                name,
                path,
                uncompressed_size: guard.uncompressed_sizes[cid],
                compressed_size: guard.compressed_sizes[cid],
                crc32: guard.crc32s[cid],
                mtime_epoch_secs: guard.modified_timestamps[cid],
                mode: guard.modes[cid],
                is_directory: is_dir,
                is_encrypted: is_enc,
                has_children,
            });
        }

        UniFFIVfsPagedResult {
            nodes,
            total_count: total_in_dir as u32,
        }
    }

    pub fn render_tree(&self) -> String {
        let guard = self.arena.read();
        guard.render_tree()
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

    pub fn vfs_total_nodes(&self) -> u32 {
        let guard = self.arena.read();
        guard.total_nodes as u32
    }

    pub fn vfs_get_children_paged(
        &self,
        parent_id: u32,
        offset: u32,
        limit: u32,
    ) -> Result<VfsPageResult, VfsSessionError> {
        let guard = self.arena.read();
        super::vfs_paging::vfs_get_children_paged_core(&guard, parent_id, offset, limit)
    }

    pub fn vfs_resolve_path(&self, path: String) -> Result<u32, VfsSessionError> {
        let guard = self.arena.read();
        super::vfs_paging::vfs_resolve_path_core(&guard, &path)
    }

    pub fn vfs_get_node_detail(&self, node_id: u32) -> Result<VfsNodeDetail, VfsSessionError> {
        let guard = self.arena.read();
        super::vfs_paging::vfs_get_node_detail_core(&guard, node_id)
    }

    pub fn vfs_search_top_k(&self, query: String, limit: u32) -> Result<Vec<VfsPagedNode>, VfsSessionError> {
        let guard = self.arena.read();
        super::vfs_search::vfs_search_top_k_core(&guard, &query, limit)
    }
}

/// Loads and constructs a high-performance in-memory VFS tree directly from an archive header
/// into continuous memory VfsArena and VfsTree.
#[uniffi::export]
pub fn load_archive_vfs_tree(
    archive_path: String,
    password: Option<String>,
) -> Result<Arc<UniFFIVfsTree>, super::types::TTZipError> {
    let p = Path::new(&archive_path);
    if !p.exists() {
        return Err(super::types::TTZipError::FileNotFound {
            path: archive_path,
        });
    }

    let arena = super::vfs_loader::build_arena_from_archive_path(p, password.as_deref())
        .map_err(|e| match e {
            VfsSessionError::NodeNotFound { .. } => super::types::TTZipError::FileNotFound {
                path: archive_path,
            },
            VfsSessionError::PathTraversalDetected { attempted_path } => {
                super::types::TTZipError::SecurityViolation { reason: attempted_path }
            }
            VfsSessionError::InspectionFailed { reason } => {
                super::types::TTZipError::IoError { message: reason }
            }
            VfsSessionError::BadPassword => super::types::TTZipError::InvalidPassword,
            VfsSessionError::InternalError => super::types::TTZipError::EngineError { code: -1 },
            VfsSessionError::InvalidVectorDimension { actual } => {
                super::types::TTZipError::InvalidArgument {
                    message: format!("Invalid vector dimension: {}", actual),
                }
            }
            VfsSessionError::EmbeddingsUnavailable => {
                super::types::TTZipError::EngineError { code: -2 }
            }
        })?;

    let total_entries = arena.total_nodes;
    Ok(Arc::new(UniFFIVfsTree {
        arena: parking_lot::RwLock::new(arena),
        total_entries,
    }))
}

fn map_nested_err(e: crate::archive::nested_vfs::NestedVfsError) -> super::types::TTZipError {
    match e {
        crate::archive::nested_vfs::NestedVfsError::FileNotFound { path } => super::types::TTZipError::FileNotFound { path },
        crate::archive::nested_vfs::NestedVfsError::InvalidPassword => super::types::TTZipError::InvalidPassword,
        crate::archive::nested_vfs::NestedVfsError::CorruptHeader { details, offset } => super::types::TTZipError::CorruptHeader { details, offset },
        crate::archive::nested_vfs::NestedVfsError::SecurityViolation { reason } => super::types::TTZipError::SecurityViolation { reason },
        crate::archive::nested_vfs::NestedVfsError::Cancelled => super::types::TTZipError::Cancelled,
        crate::archive::nested_vfs::NestedVfsError::EngineError { code } => super::types::TTZipError::EngineError { code },
        crate::archive::nested_vfs::NestedVfsError::Status(s) => match s {
            crate::types::TTZipStatus::ErrInvalidPassword => super::types::TTZipError::InvalidPassword,
            crate::types::TTZipStatus::ErrCorruptHeader => super::types::TTZipError::CorruptHeader {
                details: "Corrupted nested archive header or payload".to_string(),
                offset: 0,
            },
            crate::types::TTZipStatus::ErrFileNotFound => super::types::TTZipError::FileNotFound {
                path: "Nested entry or archive not found".to_string(),
            },
            crate::types::TTZipStatus::ErrOpenFailed | crate::types::TTZipStatus::ErrMmapFailed => {
                super::types::TTZipError::IoError { message: s.as_str().to_string() }
            }
            crate::types::TTZipStatus::ErrOutOfMemory => super::types::TTZipError::OutOfMemory { details: s.as_str().to_string() },
            crate::types::TTZipStatus::Cancelled => super::types::TTZipError::Cancelled,
            crate::types::TTZipStatus::ErrSecurityViolation | crate::types::TTZipStatus::ErrSolidBudgetExceeded => {
                super::types::TTZipError::SecurityViolation { reason: s.as_str().to_string() }
            }
            _ => super::types::TTZipError::EngineError { code: s.to_i32() },
        },
        crate::archive::nested_vfs::NestedVfsError::IoError { message } => super::types::TTZipError::IoError { message },
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
