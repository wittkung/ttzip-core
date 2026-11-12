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
        let score = compute_node_lexical_score(
            arena,
            i,
            &norm_query,
            query_mask,
            query_has_separator,
        );
        if score > 0 {
            let key = make_composite_sort_key(score, i as u32);
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

fn compute_node_lexical_score(
    arena: &crate::fs::vfs::VfsCompactArena,
    node_idx: usize,
    norm_query: &str,
    query_mask: u64,
    query_has_separator: bool,
) -> i32 {
    let node_id = node_idx as u32;
    let name = arena.node_name(node_id);

    if !query_has_separator {
        let name_mask = arena
            .name_masks
            .get(node_idx)
            .copied()
            .unwrap_or_else(|| compute_name_mask(name.as_bytes()));
        if matches_name_mask(name_mask, query_mask)
            && let Some(s) = crate::fs::vfs::search::fuzzy_match_zero_alloc(name, norm_query)
        {
            return (s as i32) + 100;
        }
        let path = arena.node_full_path_str(node_id);
        if !matches_name_mask(compute_name_mask(path.as_bytes()), query_mask) {
            return 0;
        }
        return crate::fs::vfs::search::fuzzy_match_zero_alloc(path, norm_query)
            .map_or(0, |s| s as i32);
    }

    if let Some(s) = crate::fs::vfs::search::fuzzy_match_zero_alloc(name, norm_query) {
        (s as i32) + 100
    } else {
        let path = arena.node_full_path_str(node_id);
        crate::fs::vfs::search::fuzzy_match_zero_alloc(path, norm_query).map_or(0, |s| s as i32)
    }
}

fn compute_node_semantic_dot(
    embeddings: Option<&[CompactVector128]>,
    q_arr_opt: Option<&[i8; 128]>,
    node_idx: usize,
) -> i32 {
    match (q_arr_opt, embeddings.and_then(|v| v.get(node_idx))) {
        (Some(q_arr), Some(vec_entry)) => dot_product_128(&vec_entry.data, q_arr),
        _ => 0,
    }
}

struct HybridNodeScoreQuery<'a> {
    arena: &'a crate::fs::vfs::VfsCompactArena,
    embeddings: Option<&'a [CompactVector128]>,
    q_arr_opt: Option<&'a [i8; 128]>,
    has_text: bool,
    norm_query: &'a str,
    query_mask: u64,
    query_has_separator: bool,
    is_code_query: bool,
    lexical_weight: f32,
    semantic_weight: f32,
}

fn evaluate_node_hybrid_score(
    query: &HybridNodeScoreQuery<'_>,
    node_idx: usize,
) -> Option<f32> {
    let lex_score = if query.has_text {
        compute_node_lexical_score(
            query.arena,
            node_idx,
            query.norm_query,
            query.query_mask,
            query.query_has_separator,
        )
    } else {
        0
    };
    let sem_dot = compute_node_semantic_dot(query.embeddings, query.q_arr_opt, node_idx);

    if lex_score <= 0 && sem_dot < DEFAULT_SEMANTIC_THRESHOLD_RAW {
        return None;
    }

    let active_dot_threshold = if lex_score > 0 {
        (DEFAULT_SEMANTIC_THRESHOLD_RAW * 6) / 10
    } else {
        DEFAULT_SEMANTIC_THRESHOLD_RAW
    };

    let name = query.arena.node_name(node_idx as u32);
    let is_code = is_code_filename(name.as_bytes());
    let def_strength = if query.is_code_query && is_code { 2 } else { 0 };

    let mut fused = compute_calibrated_fused_score(
        lex_score,
        sem_dot,
        query.lexical_weight,
        query.semantic_weight,
        active_dot_threshold,
        def_strength,
    );
    if query.is_code_query && !is_code {
        fused *= 0.35;
    }
    (fused > 0.0).then_some(fused)
}

fn materialize_hybrid_match(
    query: &HybridNodeScoreQuery<'_>,
    key: u64,
) -> Option<VfsHybridMatch> {
    let (fused_score, id) = unpack_hybrid_sort_key(key);
    let i = id as usize;
    if i >= query.arena.total_nodes {
        return None;
    }

    let is_dir = (query.arena.node_attributes[i] & crate::fs::vfs::arena::VFS_FLAG_IS_DIR) != 0;
    let size = query.arena.uncompressed_sizes[i];
    let name = query.arena.node_name(id);
    let path = query.arena.node_full_path_str(id);

    let lex_score = if query.has_text {
        if let Some(s) = crate::fs::vfs::search::fuzzy_match_zero_alloc(name, query.norm_query) {
            (s as i32) + 100
        } else if let Some(s) =
            crate::fs::vfs::search::fuzzy_match_zero_alloc(path, query.norm_query)
        {
            s as i32
        } else {
            0
        }
    } else {
        0
    };
    let sem_dot = compute_node_semantic_dot(query.embeddings, query.q_arr_opt, i);

    Some(VfsHybridMatch {
        node_id: id,
        name: name.to_string(),
        path: path.to_string(),
        is_dir,
        size,
        lexical_score: lex_score,
        semantic_score: sem_dot,
        fused_rank_score: fused_score,
    })
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

    let q_arr_opt: Option<&[i8; 128]> = match query_vector {
        Some(qv) if qv.len() != 128 => {
            return Err(VfsSessionError::InvalidVectorDimension {
                actual: qv.len() as u32,
            })
        }
        Some(qv) => Some(qv.try_into().map_err(|_| VfsSessionError::InternalError)?),
        None => None,
    };

    let has_text = !query.trim().is_empty();
    let has_vector = q_arr_opt.is_some() && embeddings.is_some();
    if !has_text && !has_vector {
        return Ok(Vec::new());
    }

    let is_code_query = query.trim_start().to_ascii_lowercase().starts_with("code:");
    let norm_query = if has_text {
        normalize_to_nfc(query).into_owned()
    } else {
        String::new()
    };
    let query_mask = if has_text {
        compute_name_mask(norm_query.as_bytes())
    } else {
        0
    };
    let query_has_separator = norm_query.contains('/') || norm_query.contains('\\');
    let score_query = HybridNodeScoreQuery {
        arena,
        embeddings,
        q_arr_opt,
        has_text,
        norm_query: &norm_query,
        query_mask,
        query_has_separator,
        is_code_query,
        lexical_weight,
        semantic_weight,
    };

    let mut selector = LazyTopKSelector::new(max_k);
    for i in 1..arena.total_nodes {
        if let Some(fused) = evaluate_node_hybrid_score(&score_query, i) {
            selector.push(make_hybrid_sort_key(fused, i as u32));
        }
    }

    let results = selector
        .finish()
        .into_iter()
        .filter_map(|key| materialize_hybrid_match(&score_query, key))
        .collect();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::vfs::{VfsCompactArena, VfsEntry};

    fn sample_entry(path: &str, size: u64) -> VfsEntry {
        VfsEntry {
            path: path.to_string(),
            uncompressed_size: size,
            mode: 0o100644,
            ..Default::default()
        }
    }

    #[test]
    fn test_vfs_search_zero_alloc_path_borrowing_and_parent_matching() {
        let entries = vec![
            sample_entry("src/core/engine_kernel.rs", 1024),
            sample_entry("src/ui/window_controller.swift", 2048),
            sample_entry("docs/architecture/overview.md", 512),
        ];
        let arena = VfsCompactArena::build_from_entries(&entries, "archive.zip");

        let by_name = vfs_search_top_k_core(&arena, "engine_kernel", 10).expect("search name");
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name[0].path, "src/core/engine_kernel.rs");

        let by_parent = vfs_search_top_k_core(&arena, "architecture", 10).expect("search parent");
        assert!(!by_parent.is_empty());
        assert!(by_parent.iter().any(|n| n.path.contains("architecture")));

        let by_sep = vfs_search_top_k_core(&arena, "src/core", 10).expect("search separator");
        assert!(!by_sep.is_empty());
        assert!(by_sep.iter().any(|n| n.path == "src/core/engine_kernel.rs"));

        let no_match = vfs_search_top_k_core(&arena, "nonexistent_token_xyz", 10).expect("empty");
        assert!(no_match.is_empty());
    }
}
