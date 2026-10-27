// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Mozilla UniFFI VFS Search and Subtree Scoped Acceleration Engine.
//!
//! Integrates 64-bit character Bloom pre-filtering, zero-allocation fuzzy matching,
//! and 64-bit composite sort key ranking with LazyTopK convergence.

use super::vfs::{VfsPagedNode, VfsSessionError};
use crate::fs::normalize_to_nfc;
use ttzip_types::search::{
    compute_name_mask, make_composite_sort_key, matches_name_mask,
    LazyTopKSelector,
};
use ttzip_types::semantic_search::{
    compute_calibrated_fused_score, is_code_filename, make_hybrid_sort_key, unpack_hybrid_sort_key,
    DEFAULT_SEMANTIC_THRESHOLD_RAW,
};
use ttzip_types::simd_dot::dot_product_128;
use ttzip_types::snapshot::CompactVector128;

#[derive(uniffi::Record, Clone, Debug)]
pub struct VfsHybridMatch {
    pub node_id: u32,
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub lexical_score: i32,
    pub semantic_score: i32,
    pub fused_rank_score: f32,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct VfsSemanticSearchOptions {
    pub limit: u32,
    pub lexical_weight: f32,
    pub semantic_weight: f32,
    pub rrf_k: u32,
}

/// High-throughput Top-K search across an in-memory VFS compact arena.
pub(crate) fn vfs_search_top_k_core(
    arena: &crate::fs::vfs::VfsCompactArena,
    query: &str,
    limit: u32,
) -> Result<Vec<VfsPagedNode>, VfsSessionError> {
    vfs_scoped_search_top_k_core(arena, query, 1, arena.total_nodes, limit)
}

/// Scoped Top-K search operating over a contiguous [start_id, end_id) slice.
///
/// When searching within a directory subtree (DFS Block Layout), `start_id` and `end_id`
/// bound the traversal to O(M) without scanning un-related branches of the tree.
pub(crate) fn vfs_scoped_search_top_k_core(
    arena: &crate::fs::vfs::VfsCompactArena,
    query: &str,
    start_id: usize,
    end_id: usize,
    limit: u32,
) -> Result<Vec<VfsPagedNode>, VfsSessionError> {
    let max_k = (limit as usize).clamp(1, 1024);
    if query.is_empty() || arena.total_nodes <= 1 || start_id >= end_id {
        return Ok(Vec::new());
    }

    let norm_query = normalize_to_nfc(query);
    let query_bytes = norm_query.as_bytes();
    let query_mask = compute_name_mask(query_bytes);
    let query_has_separator = norm_query.contains('/') || norm_query.contains('\\');

    let mut selector = LazyTopKSelector::new(max_k);
    let clamped_start = start_id.max(1);
    let clamped_end = end_id.min(arena.total_nodes);

    for i in clamped_start..clamped_end {
        let name = arena.node_name(i as u32);
        let path = arena.node_full_path(i as u32);

        // Tier 0: 64-bit Bloom mask pre-filter (single CPU cycle rejection)
        if !query_has_separator {
            let name_mask = if i < arena.name_masks.len() {
                arena.name_masks[i]
            } else {
                compute_name_mask(name.as_bytes())
            };
            if !matches_name_mask(name_mask, query_mask) {
                let path_mask = compute_name_mask(path.as_bytes());
                if !matches_name_mask(path_mask, query_mask) {
                    continue;
                }
            }
        }

        // Tier 2: Zero-allocation scoring
        let name_match = crate::fs::vfs::search::fuzzy_match_zero_alloc(name, &norm_query);
        let (score, matched) = if let Some(s) = name_match {
            (s + 100, true)
        } else if let Some(s) = crate::fs::vfs::search::fuzzy_match_zero_alloc(&path, &norm_query) {
            (s, true)
        } else {
            (0, false)
        };

        if matched {
            let key = make_composite_sort_key(score as i32, i as u32);
            selector.push(key);
        }
    }

    let top_entries = selector.finish_entries();
    let mut results = Vec::with_capacity(top_entries.len());

    for id in top_entries {
        let i = id as usize;
        let attr = arena.node_attributes[i];
        results.push(VfsPagedNode {
            node_id: id,
            name: arena.node_name(id).to_string(),
            path: arena.node_full_path(id),
            is_dir: (attr & crate::fs::vfs::arena::VFS_FLAG_IS_DIR) != 0,
            is_symlink: (arena.modes[i] & 0o170000) == 0o120000,
            is_encrypted: (attr & crate::fs::vfs::arena::VFS_FLAG_IS_ENCRYPTED) != 0,
            size: arena.uncompressed_sizes[i],
            compressed_size: arena.compressed_sizes[i],
            mtime_secs: arena.modified_timestamps[i],
        });
    }

    Ok(results)
}

/// Executes a hybrid search combining lexical fuzzy / Bloom pruning and NEON vector dot products.
pub(crate) fn vfs_search_hybrid_top_k_core(
    arena: &crate::fs::vfs::VfsCompactArena,
    embeddings: Option<&[CompactVector128]>,
    query: &str,
    query_vector: Option<&[i8]>,
    limit: u32,
    lexical_weight: f32,
    semantic_weight: f32,
) -> Result<Vec<VfsHybridMatch>, VfsSessionError> {
    let max_k = (limit as usize).clamp(1, 1024);
    if arena.total_nodes <= 1 {
        return Ok(Vec::new());
    }

    let q_arr_opt = if let Some(qv) = query_vector {
        if qv.len() != 128 {
            return Err(VfsSessionError::InvalidVectorDimension { actual: qv.len() as u32 });
        }
        let arr: &[i8; 128] = qv.try_into().map_err(|_| VfsSessionError::InternalError)?;
        Some(arr)
    } else {
        None
    };

    let has_text = !query.trim().is_empty();
    let has_vector = q_arr_opt.is_some() && embeddings.is_some();

    if !has_text && !has_vector {
        return Ok(Vec::new());
    }

    let is_code_query = query.trim_start().to_ascii_lowercase().starts_with("code:");
    let norm_query = if has_text { normalize_to_nfc(query).into_owned() } else { String::new() };
    let query_bytes = norm_query.as_bytes();
    let query_mask = if has_text { compute_name_mask(query_bytes) } else { 0 };
    let query_has_separator = norm_query.contains('/') || norm_query.contains('\\');

    let mut selector = LazyTopKSelector::new(max_k);

    for i in 1..arena.total_nodes {
        let mut lex_score = 0i32;
        let name = arena.node_name(i as u32);

        if has_text {
            let path = arena.node_full_path(i as u32);

            let mut pass = true;
            if !query_has_separator {
                let name_mask = if i < arena.name_masks.len() {
                    arena.name_masks[i]
                } else {
                    compute_name_mask(name.as_bytes())
                };
                if !matches_name_mask(name_mask, query_mask) {
                    let path_mask = compute_name_mask(path.as_bytes());
                    if !matches_name_mask(path_mask, query_mask) {
                        pass = false;
                    }
                }
            }

            if pass {
                let name_match = crate::fs::vfs::search::fuzzy_match_zero_alloc(name, &norm_query);
                if let Some(s) = name_match {
                    lex_score = (s as i32) + 100;
                } else if let Some(s) = crate::fs::vfs::search::fuzzy_match_zero_alloc(&path, &norm_query) {
                    lex_score = s as i32;
                }
            }
        }

        let mut sem_dot = 0i32;
        if let (Some(q_arr), Some(vectors)) = (q_arr_opt, embeddings) {
            if i < vectors.len() {
                sem_dot = dot_product_128(&vectors[i].data, q_arr);
            }
        }

        if lex_score <= 0 && sem_dot < DEFAULT_SEMANTIC_THRESHOLD_RAW {
            continue;
        }

        let active_dot_threshold = if lex_score > 0 {
            (DEFAULT_SEMANTIC_THRESHOLD_RAW * 6) / 10
        } else {
            DEFAULT_SEMANTIC_THRESHOLD_RAW
        };

        let is_code = is_code_filename(name.as_bytes());
        let def_strength = if is_code_query && is_code { 2 } else { 0 };

        let mut fused = compute_calibrated_fused_score(
            lex_score,
            sem_dot,
            lexical_weight,
            semantic_weight,
            active_dot_threshold,
            def_strength,
        );

        if is_code_query && !is_code {
            fused *= 0.35;
        }

        if fused > 0.0 {
            selector.push(make_hybrid_sort_key(fused, i as u32));
        }
    }

    let top_keys = selector.finish();
    let mut results = Vec::with_capacity(top_keys.len());

    for key in top_keys {
        let (fused_score, id) = unpack_hybrid_sort_key(key);
        let i = id as usize;
        if i >= arena.total_nodes {
            continue;
        }

        let attr = arena.node_attributes[i];
        let is_dir = (attr & crate::fs::vfs::arena::VFS_FLAG_IS_DIR) != 0;
        let size = arena.uncompressed_sizes[i];

        let mut lex_score = 0i32;
        if has_text {
            let name = arena.node_name(id);
            let path = arena.node_full_path(id);
            if let Some(s) = crate::fs::vfs::search::fuzzy_match_zero_alloc(name, &norm_query) {
                lex_score = (s as i32) + 100;
            } else if let Some(s) = crate::fs::vfs::search::fuzzy_match_zero_alloc(&path, &norm_query) {
                lex_score = s as i32;
            }
        }

        let sem_dot = if let (Some(q_arr), Some(vectors)) = (q_arr_opt, embeddings) {
            if i < vectors.len() {
                dot_product_128(&vectors[i].data, q_arr)
            } else {
                0
            }
        } else {
            0
        };

        results.push(VfsHybridMatch {
            node_id: id,
            name: arena.node_name(id).to_string(),
            path: arena.node_full_path(id),
            is_dir,
            size,
            lexical_score: lex_score,
            semantic_score: sem_dot,
            fused_rank_score: fused_score,
        });
    }

    Ok(results)
}

/// Fast-path semantic-only top-K search over pre-computed embeddings.
pub(crate) fn vfs_search_semantic_top_k_core(
    arena: &crate::fs::vfs::VfsCompactArena,
    embeddings: Option<&[CompactVector128]>,
    query_vector: &[i8],
    limit: u32,
) -> Result<Vec<VfsPagedNode>, VfsSessionError> {
    let max_k = (limit as usize).clamp(1, 1024);
    if query_vector.len() != 128 {
        return Err(VfsSessionError::InvalidVectorDimension {
            actual: query_vector.len() as u32,
        });
    }
    let vectors = match embeddings {
        Some(v) => v,
        None => return Err(VfsSessionError::EmbeddingsUnavailable),
    };

    let q_arr: &[i8; 128] = query_vector
        .try_into()
        .map_err(|_| VfsSessionError::InternalError)?;

    let mut selector = LazyTopKSelector::new(max_k);
    let total = arena.total_nodes.min(vectors.len());

    for i in 1..total {
        let dot = dot_product_128(&vectors[i].data, q_arr);
        let key = make_composite_sort_key(dot, i as u32);
        selector.push(key);
    }

    let top_entries = selector.finish_entries();
    let mut results = Vec::with_capacity(top_entries.len());

    for id in top_entries {
        let i = id as usize;
        if i >= arena.total_nodes {
            continue;
        }
        let attr = arena.node_attributes[i];
        results.push(VfsPagedNode {
            node_id: id,
            name: arena.node_name(id).to_string(),
            path: arena.node_full_path(id),
            is_dir: (attr & crate::fs::vfs::arena::VFS_FLAG_IS_DIR) != 0,
            is_symlink: (arena.modes[i] & 0o170000) == 0o120000,
            is_encrypted: (attr & crate::fs::vfs::arena::VFS_FLAG_IS_ENCRYPTED) != 0,
            size: arena.uncompressed_sizes[i],
            compressed_size: arena.compressed_sizes[i],
            mtime_secs: arena.modified_timestamps[i],
        });
    }

    Ok(results)
}
