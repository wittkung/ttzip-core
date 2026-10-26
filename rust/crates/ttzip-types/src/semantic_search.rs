// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! In-Memory Hybrid Lexical-Semantic Search Types and Calibrated Fusion Algebra.
//!
//! # Architecture & Mathematical Formulation
//! Features:
//! - [`HybridSearchQuery`]: Query configuration pairing lexical string and INT8 dense vector.
//! - [`HybridSearchCandidate`]: Intermediate scored candidate with dual scores and fused rank.
//! - [`make_hybrid_sort_key`]: Monotonic 64-bit IEEE 754 float transformation with deterministic
//!   `node_id` tie-breaking for single-cycle branchless sorting in [`LazyTopKSelector`].
//! - Single-Pass Streaming Calibrated Fusion:
//!   $$\text{FusedScore}(d) = w_{\text{lex}} \cdot (\bar{S}_{\text{lex}}(d))^{1.5} + w_{\text{sem}} \cdot \bar{S}_{\text{sem}}(d) + \text{SynergyBonus} + \text{DefBonus}$$

use crate::node::{make_sort_prefix, CompactNode, TTWorkspaceEntryNode};
use crate::search::{
    ascii_to_lower_u64, compute_name_mask, evaluate_node_match, LazyTopKSelector, MASK_CHAR_SET,
};
use crate::snapshot::CompactVector128;

/// Unit-vector INT8 dot product normalization scale: 127^2 = 16,129.
pub const INT8_UNIT_DOT_BASE: i32 = 16_129;
pub const INV_INT8_UNIT_DOT_BASE: f32 = 1.0 / (INT8_UNIT_DOT_BASE as f32);

/// Calibrated semantic noise rejection threshold: cosine >= 0.55.
pub const DEFAULT_SEMANTIC_THRESHOLD_COSINE: f32 = 0.55;
pub const DEFAULT_SEMANTIC_THRESHOLD_RAW: i32 = 8_871; // round(0.55 * 16,129)
pub const SEMANTIC_RANGE_INV: f32 = 1.0 / (16_129.0 - 8_871.0);

pub const DEFAULT_LEXICAL_WEIGHT: f32 = 0.60;
pub const DEFAULT_SEMANTIC_WEIGHT: f32 = 0.40;
pub const DEFAULT_SYNERGY_BONUS: f32 = 0.15;
pub const DEFAULT_DEFINITION_BONUS: f32 = 0.25;

/// Returns true if the filename slice has a known source code or configuration extension.
#[inline]
pub fn is_code_filename(name: &[u8]) -> bool {
    let lower_ext = if let Some(dot_idx) = name.iter().rposition(|&b| b == b'.') {
        &name[dot_idx + 1..]
    } else {
        return false;
    };
    matches!(
        lower_ext,
        b"rs" | b"swift" | b"c" | b"cpp" | b"cc" | b"cxx" | b"h" | b"hpp"
            | b"py" | b"go" | b"ts" | b"tsx" | b"js" | b"jsx" | b"java" | b"kt"
            | b"m" | b"mm" | b"zig" | b"rb" | b"php" | b"cs" | b"sh" | b"json" | b"toml"
            | b"yaml" | b"yml" | b"xml" | b"html" | b"css" | b"scss" | b"sql"
    )
}

/// Query configuration passed into the hybrid search engine.
#[derive(Clone, Debug)]
pub struct HybridSearchQuery<'a> {
    pub text_query: &'a str,
    pub query_vector: Option<&'a [i8; 128]>,
    pub limit: u32,
    pub lexical_weight: f32,
    pub semantic_weight: f32,
    pub min_raw_dot_threshold: i32,
    pub is_code_lookup: bool,
}

impl<'a> HybridSearchQuery<'a> {
    #[inline]
    pub fn new(text_query: &'a str) -> Self {
        let is_code = text_query.trim_start().to_ascii_lowercase().starts_with("code:");
        Self {
            text_query,
            query_vector: None,
            limit: 50,
            lexical_weight: DEFAULT_LEXICAL_WEIGHT,
            semantic_weight: DEFAULT_SEMANTIC_WEIGHT,
            min_raw_dot_threshold: DEFAULT_SEMANTIC_THRESHOLD_RAW,
            is_code_lookup: is_code,
        }
    }

    #[inline]
    pub fn with_vector(mut self, query_vector: &'a [i8; 128]) -> Self {
        self.query_vector = Some(query_vector);
        self
    }

    #[inline]
    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = limit;
        self
    }

    #[inline]
    pub fn with_weights(mut self, lexical_weight: f32, semantic_weight: f32) -> Self {
        self.lexical_weight = lexical_weight;
        self.semantic_weight = semantic_weight;
        self
    }

    #[inline]
    pub fn with_min_threshold(mut self, threshold: i32) -> Self {
        self.min_raw_dot_threshold = threshold;
        self
    }

    #[inline]
    pub fn with_min_similarity(self, threshold: i32) -> Self {
        self.with_min_threshold(threshold)
    }

    #[inline]
    pub fn with_code_lookup(mut self, is_code_lookup: bool) -> Self {
        self.is_code_lookup = is_code_lookup;
        self
    }
}

/// Intermediate candidate evaluated during streaming single-pass convergence.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HybridSearchCandidate {
    pub node_id: u32,
    pub lexical_score: i32,
    pub semantic_dot_product: i32,
    pub fused_score: f32,
}

/// Strict monotonic transformation of IEEE 754 float into a 64-bit sort key.
#[inline(always)]
pub fn make_hybrid_sort_key(fused_score: f32, node_id: u32) -> u64 {
    let bits = fused_score.to_bits();
    let monotonic_float = if (bits & 0x8000_0000) != 0 {
        !bits
    } else {
        bits ^ 0x8000_0000
    };
    ((monotonic_float as u64) << 32) | (!node_id as u64)
}

/// Inverses a 64-bit monotonic hybrid sort key back into (fused_score, node_id).
#[inline(always)]
pub fn unpack_hybrid_sort_key(key: u64) -> (f32, u32) {
    let high = (key >> 32) as u32;
    let bits = if (high & 0x8000_0000) != 0 {
        high ^ 0x8000_0000
    } else {
        !high
    };
    (f32::from_bits(bits), !(key as u32))
}

/// Computes calibrated score fusion directly in registers with zero heap allocation.
#[inline(always)]
pub fn compute_calibrated_fused_score(
    lexical_score: i32,
    raw_dot_product: i32,
    w_lex: f32,
    w_sem: f32,
    min_dot_threshold: i32,
    definition_strength: u8,
) -> f32 {
    let mut fused = 0.0f32;
    let mut has_lex = false;
    let mut has_sem = false;

    // 1. Lexical Component: normalized to 0.0..1.0 and non-linear (coverage)^1.5 boosted
    if lexical_score > 0 {
        let norm_lex = (lexical_score as f32) * 0.001f32;
        let boosted_lex = norm_lex * norm_lex.sqrt();
        fused += w_lex * boosted_lex;
        has_lex = true;
    }

    // 2. Semantic Component: gated against background noise and mapped to 0.0..1.0
    if raw_dot_product >= min_dot_threshold {
        let excess = (raw_dot_product - min_dot_threshold) as f32;
        let norm_sem = (excess * SEMANTIC_RANGE_INV).min(1.0f32);
        fused += w_sem * norm_sem;
        has_sem = true;
    }

    // 3. Synergy Bonus: Co-occurrence of lexical accuracy and semantic resonance
    if has_lex && has_sem {
        fused += DEFAULT_SYNERGY_BONUS;
    }

    // 4. Code Definition Elevation
    if definition_strength == 2 {
        fused += DEFAULT_DEFINITION_BONUS;
    }

    fused
}

struct PreparedLexicalQuery<'a> {
    has_text: bool,
    query_bytes: &'a [u8],
    q_char_mask: u64,
    is_short_prefix: bool,
    prefix_mask: u64,
    q_prefix_lower: u64,
    is_ascii_query: bool,
    query_lower: Option<String>,
}

impl<'a> PreparedLexicalQuery<'a> {
    fn new(text_query: &'a str) -> Self {
        let has_text = !text_query.trim().is_empty();
        let query_bytes = text_query.as_bytes();
        let query_len = query_bytes.len();
        let q_mask = compute_name_mask(query_bytes);
        let q_char_mask = q_mask & MASK_CHAR_SET;

        let is_short_prefix = query_len <= 8;
        let q_prefix_lower = if is_short_prefix {
            ascii_to_lower_u64(make_sort_prefix(query_bytes))
        } else {
            ascii_to_lower_u64(make_sort_prefix(&query_bytes[..8]))
        };

        let prefix_mask = if is_short_prefix {
            if query_len == 8 {
                0xFFFF_FFFF_FFFF_FFFF
            } else if query_len == 0 {
                0
            } else {
                !((1u64 << ((8 - query_len) * 8)) - 1)
            }
        } else {
            0xFFFF_FFFF_FFFF_FFFF
        };

        let is_ascii_query = query_bytes.is_ascii();
        let query_lower = (!is_ascii_query).then(|| text_query.to_lowercase());

        Self {
            has_text,
            query_bytes,
            q_char_mask,
            is_short_prefix,
            prefix_mask,
            q_prefix_lower,
            is_ascii_query,
            query_lower,
        }
    }

    #[inline(always)]
    fn evaluate_node(
        &self,
        node: &TTWorkspaceEntryNode,
        string_arena: &[u8],
        name_mask: Option<u64>,
    ) -> i32 {
        if !self.has_text {
            return 0;
        }
        if let Some(m) = name_mask {
            if (self.q_char_mask & !m) != 0 {
                return 0;
            }
        }
        evaluate_node_match(
            node,
            string_arena,
            self.query_bytes,
            self.query_lower.as_deref(),
            self.is_ascii_query,
            self.is_short_prefix,
            self.prefix_mask,
            self.q_prefix_lower,
        )
        .unwrap_or(0)
    }

    #[inline(always)]
    fn evaluate_compact_node(
        &self,
        node: &CompactNode,
        string_arena: &[u8],
        name_mask: Option<u64>,
    ) -> i32 {
        if !self.has_text {
            return 0;
        }
        if let Some(m) = name_mask {
            if (self.q_char_mask & !m) != 0 {
                return 0;
            }
        }
        let Some(name_bytes) = compact_node_name(node, string_arena) else {
            return 0;
        };
        let query_len = self.query_bytes.len();
        if self.is_short_prefix
            && name_bytes.len() >= query_len
            && name_bytes[..query_len].eq_ignore_ascii_case(self.query_bytes)
        {
            return if name_bytes.len() == query_len {
                1000
            } else {
                500 - (name_bytes.len() as i32).min(200)
            };
        }
        if match_compact_substring(name_bytes, self.query_bytes, self.query_lower.as_deref(), self.is_ascii_query) {
            calculate_compact_match_score(name_bytes, self.query_bytes)
        } else {
            0
        }
    }
}

#[inline(always)]
fn compact_node_name<'a>(node: &CompactNode, arena: &'a [u8]) -> Option<&'a [u8]> {
    let start = node.name_offset as usize;
    let end = start.checked_add(node.name_len as usize)?;
    arena.get(start..end)
}

#[inline(always)]
fn contains_ascii_case_insensitive(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.len() < needle.len() {
        return false;
    }
    let first = needle[0].to_ascii_lowercase();
    let end = haystack.len() - needle.len();
    for i in 0..=end {
        if haystack[i].to_ascii_lowercase() == first
            && haystack[i..i + needle.len()].eq_ignore_ascii_case(needle)
        {
            return true;
        }
    }
    false
}

#[inline]
fn match_compact_substring(
    name_bytes: &[u8],
    query_bytes: &[u8],
    query_lower: Option<&str>,
    is_ascii_query: bool,
) -> bool {
    if is_ascii_query && name_bytes.is_ascii() {
        contains_ascii_case_insensitive(name_bytes, query_bytes)
    } else if let Some(q_low) = query_lower {
        if let Ok(name_str) = core::str::from_utf8(name_bytes) {
            name_str.to_lowercase().contains(q_low)
        } else {
            contains_ascii_case_insensitive(name_bytes, query_bytes)
        }
    } else {
        contains_ascii_case_insensitive(name_bytes, query_bytes)
    }
}

#[inline]
fn find_compact_match_pos(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    if haystack.len() < needle.len() {
        return None;
    }
    let first = needle[0].to_ascii_lowercase();
    let end = haystack.len() - needle.len();
    for i in 0..=end {
        if haystack[i].to_ascii_lowercase() == first
            && haystack[i..i + needle.len()].eq_ignore_ascii_case(needle)
        {
            return Some(i);
        }
    }
    None
}

#[inline]
fn calculate_compact_match_score(name: &[u8], query: &[u8]) -> i32 {
    if name.len() == query.len() && name.eq_ignore_ascii_case(query) {
        return 1000;
    }
    if name.len() >= query.len() && name[..query.len()].eq_ignore_ascii_case(query) {
        return 500 - (name.len() as i32).min(200);
    }
    if let Some(pos) = find_compact_match_pos(name, query) {
        let is_word_boundary = if pos == 0 {
            true
        } else {
            let prev = name[pos - 1];
            prev == b'-' || prev == b'_' || prev == b'.' || prev == b'/' || prev == b' '
                || (name[pos].is_ascii_uppercase() && prev.is_ascii_lowercase())
        };
        if is_word_boundary {
            300 - (pos as i32).min(100)
        } else {
            100 - (pos as i32).min(50)
        }
    } else {
        100
    }
}

#[inline(always)]
fn evaluate_semantic_dot(
    idx: usize,
    q_vec: Option<&[i8; 128]>,
    vectors: Option<&[CompactVector128]>,
) -> i32 {
    match (q_vec, vectors) {
        (Some(qv), Some(vs)) if idx < vs.len() => {
            crate::simd_dot::dot_product_128(&vs[idx].data, qv)
        }
        _ => 0,
    }
}

fn reconstruct_candidate(
    k: u64,
    nodes: &[TTWorkspaceEntryNode],
    string_arena: &[u8],
    lex_prep: &PreparedLexicalQuery<'_>,
    q_vec: Option<&[i8; 128]>,
    vectors: Option<&[CompactVector128]>,
) -> HybridSearchCandidate {
    let (fused, id) = unpack_hybrid_sort_key(k);
    let idx = id as usize;
    let lex = if idx < nodes.len() {
        lex_prep.evaluate_node(&nodes[idx], string_arena, None)
    } else {
        0
    };
    let sem = evaluate_semantic_dot(idx, q_vec, vectors);
    HybridSearchCandidate {
        node_id: id,
        lexical_score: lex,
        semantic_dot_product: sem,
        fused_score: fused,
    }
}

/// Evaluates hybrid search in a single streaming pass over VFS workspace nodes.
/// Zero dynamic allocations in hot path; strictly bounded by Top-K selector buffer.
pub fn search_hybrid_nodes_top_k(
    name_masks: &[u64],
    nodes: &[TTWorkspaceEntryNode],
    string_arena: &[u8],
    vectors: Option<&[CompactVector128]>,
    query: &HybridSearchQuery<'_>,
) -> Vec<HybridSearchCandidate> {
    let limit = query.limit as usize;
    if limit == 0 || nodes.is_empty() {
        return Vec::new();
    }

    let lex_prep = PreparedLexicalQuery::new(query.text_query);
    let has_vectors = query.query_vector.is_some() && vectors.is_some();
    if !lex_prep.has_text && !has_vectors {
        return Vec::new();
    }

    let mut selector = LazyTopKSelector::new(limit);
    let q_vec = query.query_vector;

    for (i, node) in nodes.iter().enumerate() {
        let lex_score = lex_prep.evaluate_node(node, string_arena, name_masks.get(i).copied());
        let sem_dot = evaluate_semantic_dot(i, q_vec, vectors);

        if lex_score <= 0 && sem_dot < query.min_raw_dot_threshold {
            continue;
        }

        let active_dot_threshold = if lex_score > 0 {
            (query.min_raw_dot_threshold * 6) / 10
        } else {
            query.min_raw_dot_threshold
        };

        let is_code = node.name(string_arena).is_some_and(is_code_filename);
        let definition_strength = if query.is_code_lookup && is_code { 2 } else { 0 };

        let mut fused = compute_calibrated_fused_score(
            lex_score,
            sem_dot,
            query.lexical_weight,
            query.semantic_weight,
            active_dot_threshold,
            definition_strength,
        );

        if query.is_code_lookup && !is_code {
            fused *= 0.35;
        }

        if fused > 0.0 {
            let key = make_hybrid_sort_key(fused, i as u32);
            selector.push(key);
        }
    }

    selector
        .finish()
        .into_iter()
        .map(|k| reconstruct_candidate(k, nodes, string_arena, &lex_prep, q_vec, vectors))
        .collect()
}

fn reconstruct_compact_candidate(
    k: u64,
    nodes: &[CompactNode],
    string_arena: &[u8],
    lex_prep: &PreparedLexicalQuery<'_>,
    q_vec: Option<&[i8; 128]>,
    vectors: Option<&[CompactVector128]>,
) -> HybridSearchCandidate {
    let (fused, id) = unpack_hybrid_sort_key(k);
    let idx = id as usize;
    let lex = if idx < nodes.len() {
        lex_prep.evaluate_compact_node(&nodes[idx], string_arena, None)
    } else {
        0
    };
    let sem = evaluate_semantic_dot(idx, q_vec, vectors);
    HybridSearchCandidate {
        node_id: id,
        lexical_score: lex,
        semantic_dot_product: sem,
        fused_score: fused,
    }
}

/// Evaluates hybrid search in a single streaming pass over 24-byte compact VFS nodes.
/// Zero dynamic allocations in hot path; strictly bounded by Top-K selector buffer.
pub fn search_hybrid_compact_nodes_top_k(
    name_masks: &[u64],
    nodes: &[CompactNode],
    string_arena: &[u8],
    vectors: Option<&[CompactVector128]>,
    query: &HybridSearchQuery<'_>,
) -> Vec<HybridSearchCandidate> {
    let limit = query.limit as usize;
    if limit == 0 || nodes.is_empty() {
        return Vec::new();
    }

    let lex_prep = PreparedLexicalQuery::new(query.text_query);
    let has_vectors = query.query_vector.is_some() && vectors.is_some();
    if !lex_prep.has_text && !has_vectors {
        return Vec::new();
    }

    let mut selector = LazyTopKSelector::new(limit);
    let q_vec = query.query_vector;

    for (i, node) in nodes.iter().enumerate() {
        let lex_score = lex_prep.evaluate_compact_node(node, string_arena, name_masks.get(i).copied());
        let sem_dot = evaluate_semantic_dot(i, q_vec, vectors);

        if lex_score <= 0 && sem_dot < query.min_raw_dot_threshold {
            continue;
        }

        let active_dot_threshold = if lex_score > 0 {
            (query.min_raw_dot_threshold * 6) / 10
        } else {
            query.min_raw_dot_threshold
        };

        let is_code = compact_node_name(node, string_arena).is_some_and(is_code_filename);
        let definition_strength = if query.is_code_lookup && is_code { 2 } else { 0 };

        let mut fused = compute_calibrated_fused_score(
            lex_score,
            sem_dot,
            query.lexical_weight,
            query.semantic_weight,
            active_dot_threshold,
            definition_strength,
        );

        if query.is_code_lookup && !is_code {
            fused *= 0.35;
        }

        if fused > 0.0 {
            let key = make_hybrid_sort_key(fused, i as u32);
            selector.push(key);
        }
    }

    selector
        .finish()
        .into_iter()
        .map(|k| reconstruct_compact_candidate(k, nodes, string_arena, &lex_prep, q_vec, vectors))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_hybrid_sort_key_monotonicity() {
        let key_neg_large = make_hybrid_sort_key(-100.0, 1);
        let key_neg_small = make_hybrid_sort_key(-0.5, 1);
        let key_zero = make_hybrid_sort_key(0.0, 1);
        let key_pos_small = make_hybrid_sort_key(0.5, 1);
        let key_pos_large = make_hybrid_sort_key(100.0, 1);

        assert!(key_neg_large < key_neg_small);
        assert!(key_neg_small < key_zero);
        assert!(key_zero < key_pos_small);
        assert!(key_pos_small < key_pos_large);

        let key_id1 = make_hybrid_sort_key(1.0, 1);
        let key_id2 = make_hybrid_sort_key(1.0, 2);
        assert!(key_id1 > key_id2);

        let (unpacked_score, unpacked_id) = unpack_hybrid_sort_key(key_pos_large);
        assert!((unpacked_score - 100.0).abs() < 1e-6);
        assert_eq!(unpacked_id, 1);
    }

    #[test]
    fn test_calibrated_fused_score_math() {
        let score_pure_lex = compute_calibrated_fused_score(1000, 0, 0.6, 0.4, 8871, 0);
        assert!((score_pure_lex - 0.60).abs() < 1e-4);

        let score_pure_sem = compute_calibrated_fused_score(0, 16129, 0.6, 0.4, 8871, 0);
        assert!((score_pure_sem - 0.40).abs() < 1e-4);

        let score_synergy = compute_calibrated_fused_score(1000, 16129, 0.6, 0.4, 8871, 0);
        assert!((score_synergy - (0.60 + 0.40 + 0.15)).abs() < 1e-4);

        let score_def = compute_calibrated_fused_score(1000, 0, 0.6, 0.4, 8871, 2);
        assert!((score_def - (0.60 + 0.25)).abs() < 1e-4);

        let score_noise = compute_calibrated_fused_score(0, 5000, 0.6, 0.4, 8871, 0);
        assert_eq!(score_noise, 0.0);
    }

    #[test]
    fn test_search_hybrid_nodes_top_k() {
        let mut arena = Vec::new();
        let mut nodes = Vec::new();
        let mut name_masks = Vec::new();

        let files = ["document.pdf", "image", "notes.txt"];
        for (i, name) in files.iter().enumerate() {
            let offset = arena.len() as u32;
            let len = name.len() as u16;
            arena.extend_from_slice(name.as_bytes());

            nodes.push(TTWorkspaceEntryNode {
                name_offset: offset,
                name_len: len,
                sort_prefix: make_sort_prefix(name.as_bytes()),
                stable_id: i as u64,
                ..TTWorkspaceEntryNode::default()
            });

            name_masks.push(compute_name_mask(name.as_bytes()));
        }

        let mut v0 = CompactVector128::zero();
        v0[0] = 70;
        let mut v1 = CompactVector128::zero();
        v1[0] = 10;
        let mut v2 = CompactVector128::zero();
        v2[0] = 127;
        let vectors = [v0, v1, v2];

        let mut query_vec = [0i8; 128];
        query_vec[0] = 127;

        // 1. Semantic-only search with threshold 8871
        // v0 dot: 70 * 127 = 8890 (>= 8871)
        // v1 dot: 10 * 127 = 1270 (< 8871, filtered)
        // v2 dot: 127 * 127 = 16129 (>= 8871)
        let query_sem = HybridSearchQuery::new("")
            .with_vector(&query_vec)
            .with_min_threshold(8871)
            .with_limit(3);
        let res_sem = search_hybrid_nodes_top_k(&name_masks, &nodes, &arena, Some(&vectors), &query_sem);
        assert_eq!(res_sem.len(), 2);
        assert_eq!(res_sem[0].node_id, 2);
        assert_eq!(res_sem[1].node_id, 0);

        // 2. Lexical-only fallback
        let query_lex = HybridSearchQuery::new("document").with_limit(3);
        let res_lex = search_hybrid_nodes_top_k(&name_masks, &nodes, &arena, Some(&vectors), &query_lex);
        assert_eq!(res_lex.len(), 1);
        assert_eq!(res_lex[0].node_id, 0);

        // 3. Hybrid search: exact name match on "image" + semantic boost on node 2
        let query_hyb = HybridSearchQuery::new("image")
            .with_vector(&query_vec)
            .with_limit(3);
        let res_hyb = search_hybrid_nodes_top_k(&name_masks, &nodes, &arena, Some(&vectors), &query_hyb);
        assert!(!res_hyb.is_empty());
        assert_eq!(res_hyb[0].node_id, 1);
    }

    #[test]
    fn test_search_hybrid_compact_nodes_top_k() {
        let mut arena = Vec::new();
        let mut compact_nodes = Vec::new();
        let mut name_masks = Vec::new();

        let files = ["document.pdf", "image", "notes.txt"];
        for name in files.iter() {
            let offset = arena.len() as u32;
            let len = name.len() as u16;
            arena.extend_from_slice(name.as_bytes());

            compact_nodes.push(CompactNode {
                name_offset: offset,
                name_len: len,
                flags: 0,
                size: 100,
                mtime: 0,
                dir_children_idx: 0,
            });

            name_masks.push(compute_name_mask(name.as_bytes()));
        }

        let mut v0 = CompactVector128::zero();
        v0[0] = 70;
        let mut v1 = CompactVector128::zero();
        v1[0] = 10;
        let mut v2 = CompactVector128::zero();
        v2[0] = 127;
        let vectors = [v0, v1, v2];

        let mut query_vec = [0i8; 128];
        query_vec[0] = 127;

        let query_hyb = HybridSearchQuery::new("image")
            .with_vector(&query_vec)
            .with_limit(3);
        let res_hyb = search_hybrid_compact_nodes_top_k(&name_masks, &compact_nodes, &arena, Some(&vectors), &query_hyb);
        assert!(!res_hyb.is_empty());
        assert_eq!(res_hyb[0].node_id, 1);
    }
}
