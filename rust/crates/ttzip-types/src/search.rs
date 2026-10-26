// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use crate::node::{make_sort_prefix, TTWorkspaceEntryNode};

/// Purely arithmetic branchless ASCII uppercase-to-lowercase folding.
#[inline(always)]
pub const fn fold_ascii(b: u8) -> u8 {
    b | (((b.wrapping_sub(b'A') < 26) as u8) << 5)
}

/// Converts 8 bytes in u64 to ASCII lowercase in register branchlessly.
#[inline(always)]
pub fn ascii_to_lower_u64(val: u64) -> u64 {
    let bytes = val.to_be_bytes();
    let lower = [
        fold_ascii(bytes[0]),
        fold_ascii(bytes[1]),
        fold_ascii(bytes[2]),
        fold_ascii(bytes[3]),
        fold_ascii(bytes[4]),
        fold_ascii(bytes[5]),
        fold_ascii(bytes[6]),
        fold_ascii(bytes[7]),
    ];
    u64::from_be_bytes(lower)
}

pub const MASK_CHAR_SET: u64 = 0x0000_01FF_FFFF_FFFF; // Bits 0..40
pub const MASK_WORD_INITIAL_SET: u64 = 0xFFFF_FE00_0000_0000; // Bits 41..63

/// Computes a 64-bit Bloom character classification mask for an entry name.
///
/// Bit layout:
#[inline(always)]
const fn classify_delimiter(b: u8) -> Option<u64> {
    match b {
        b'-' => Some(1u64 << 36),
        b'_' => Some(1u64 << 37),
        b'.' => Some(1u64 << 38),
        b'/' => Some(1u64 << 39),
        b' ' => Some(1u64 << 40),
        _ => None,
    }
}

#[inline(always)]
const fn classify_alnum(lower: u8) -> Option<u64> {
    if lower >= b'a' && lower <= b'z' {
        Some((lower - b'a') as u64)
    } else if lower >= b'0' && lower <= b'9' {
        Some(26 + (lower - b'0') as u64)
    } else {
        None
    }
}

/// - Bit 0..25:  'a'..='z' (case-folded)
/// - Bit 26..35: '0'..='9'
/// - Bit 36..40: Common delimiters ('-', '_', '.', '/', ' ')
/// - Bit 41..63: Word initial character hash buckets (modulo 23)
#[inline]
pub fn compute_name_mask(name: &[u8]) -> u64 {
    let mut mask: u64 = 0;
    let mut prev_is_delim = true;
    let mut prev_is_lower = false;

    for &b in name {
        let is_upper = b.is_ascii_uppercase();
        let lower = fold_ascii(b);

        if let Some(bit) = classify_alnum(lower) {
            mask |= 1u64 << bit;
            let is_alpha = lower >= b'a';
            let is_boundary = prev_is_delim || (is_alpha && is_upper && prev_is_lower);
            if is_boundary {
                let bucket = bit % 23;
                mask |= 1u64 << (41 + bucket);
            }
            prev_is_delim = false;
            prev_is_lower = is_alpha && !is_upper;
        } else {
            if let Some(delim_bit) = classify_delimiter(lower) {
                mask |= delim_bit;
            }
            prev_is_delim = true;
            prev_is_lower = false;
        }
    }

    mask
}

/// Single-cycle bitwise test verifying that all required query bits exist in node mask.
#[inline(always)]
pub const fn matches_name_mask(node_mask: u64, query_mask: u64) -> bool {
    (query_mask & !node_mask) == 0
}

/// Encodes signed score and entry_id into a monotonic unsigned 64-bit composite key.
///
/// Mapping formula:
/// `(((score as i64 - i32::MIN as i64) as u64) << 32) | (!entry_id as u64)`
///
/// Order guarantees:
/// - Higher scores yield larger keys.
/// - Equal scores break ties in favor of smaller entry_id (since `!id` is larger for smaller id).
#[inline(always)]
pub const fn make_composite_sort_key(score: i32, entry_id: u32) -> u64 {
    (((score as i64 - i32::MIN as i64) as u64) << 32) | (!entry_id as u64)
}

/// Inverse deconstruction of 64-bit composite sort key into (score, entry_id).
#[inline(always)]
pub const fn unpack_composite_sort_key(key: u64) -> (i32, u32) {
    let high = key >> 32;
    let score = (high as i64 + i32::MIN as i64) as i32;
    let entry_id = !(key as u32);
    (score, entry_id)
}

/// Adaptive lazy truncation Top-K selector maintaining a contiguous buffer of composite keys.
///
/// Defers heap adjustment by accumulating up to 2*k items before truncating via `select_nth_unstable_by`.
#[derive(Debug, Clone)]
pub struct LazyTopKSelector {
    k: usize,
    keys: Vec<u64>,
    threshold: Option<u64>,
}

impl LazyTopKSelector {
    /// Creates a new LazyTopKSelector with pre-allocated capacity for 2 * k elements.
    #[inline]
    pub fn new(k: usize) -> Self {
        let capacity = if k == 0 { 0 } else { (2 * k).max(16) };
        Self {
            k,
            keys: Vec::with_capacity(capacity),
            threshold: None,
        }
    }

    /// Pushes a composite key, lazily truncating when the buffer reaches 2 * k.
    #[inline]
    pub fn push(&mut self, key: u64) {
        if self.k == 0 {
            return;
        }
        if let Some(threshold) = self.threshold {
            if key <= threshold && self.keys.len() >= self.k {
                return;
            }
        }
        self.keys.push(key);
        if self.keys.len() >= 2 * self.k {
            self.truncate();
        }
    }

    #[inline]
    fn truncate(&mut self) {
        if self.keys.len() > self.k && self.k > 0 {
            self.keys.select_nth_unstable_by(self.k - 1, |a, b| b.cmp(a));
            self.threshold = Some(self.keys[self.k - 1]);
            self.keys.truncate(self.k);
        }
    }

    /// Returns the number of currently retained elements (up to k).
    #[inline]
    pub fn len(&self) -> usize {
        self.keys.len().min(self.k)
    }

    /// Returns true if the selector contains no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Finalizes and returns the top k keys sorted in descending order.
    pub fn finish(mut self) -> Vec<u64> {
        if self.keys.is_empty() || self.k == 0 {
            return Vec::new();
        }
        if self.keys.len() > self.k {
            self.truncate();
        }
        self.keys.sort_unstable_by(|a, b| b.cmp(a));
        self.keys
    }

    /// Finalizes and returns (score, entry_id) tuples sorted by descending rank.
    pub fn finish_tuples(self) -> Vec<(i32, u32)> {
        self.finish()
            .into_iter()
            .map(unpack_composite_sort_key)
            .collect()
    }

    /// Finalizes and returns entry IDs sorted by descending rank.
    pub fn finish_entries(self) -> Vec<u32> {
        self.finish()
            .into_iter()
            .map(|k| unpack_composite_sort_key(k).1)
            .collect()
    }
}

#[cfg(target_arch = "aarch64")]
#[inline]
unsafe fn neon_ascii_to_lower_16(v: core::arch::aarch64::uint8x16_t) -> core::arch::aarch64::uint8x16_t {
    use core::arch::aarch64::*;
    let a = vdupq_n_u8(b'A');
    let z = vdupq_n_u8(b'Z');
    let bit5 = vdupq_n_u8(0x20);
    let ge_a = vcgeq_u8(v, a);
    let le_z = vcleq_u8(v, z);
    let is_upper = vandq_u8(ge_a, le_z);
    vorrq_u8(v, vandq_u8(is_upper, bit5))
}

#[cfg(target_arch = "aarch64")]
#[inline]
unsafe fn neon_contains_ascii(haystack: &[u8], needle: &[u8]) -> bool {
    use core::arch::aarch64::*;
    if needle.is_empty() {
        return true;
    }
    if haystack.len() < needle.len() {
        return false;
    }
    let first_needle = needle[0].to_ascii_lowercase();
    let v_needle_first = vdupq_n_u8(first_needle);

    let mut i = 0;
    while i + 16 <= haystack.len() {
        let raw = vld1q_u8(haystack.as_ptr().add(i));
        let lower = neon_ascii_to_lower_16(raw);
        let cmp = vceqq_u8(lower, v_needle_first);
        let max_val = vmaxvq_u8(cmp);
        if max_val != 0 {
            let end = (i + 16).min(haystack.len() - needle.len() + 1);
            for j in i..end {
                if haystack[j..j + needle.len()].eq_ignore_ascii_case(needle) {
                    return true;
                }
            }
        }
        i += 16;
    }
    while i + needle.len() <= haystack.len() {
        if haystack[i..i + needle.len()].eq_ignore_ascii_case(needle) {
            return true;
        }
        i += 1;
    }
    false
}

#[inline]
fn contains_ascii(haystack: &[u8], needle: &[u8]) -> bool {
    #[cfg(target_arch = "aarch64")]
    {
        unsafe { neon_contains_ascii(haystack, needle) }
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        haystack.windows(needle.len()).any(|w| w.eq_ignore_ascii_case(needle))
    }
}

#[inline]
fn match_substring_bytes(
    name_bytes: &[u8],
    query_bytes: &[u8],
    query_lower: Option<&str>,
    is_ascii_query: bool,
) -> bool {
    if is_ascii_query && name_bytes.is_ascii() {
        contains_ascii(name_bytes, query_bytes)
    } else if let Some(q_low) = query_lower {
        if let Ok(name_str) = core::str::from_utf8(name_bytes) {
            name_str.to_lowercase().contains(q_low)
        } else {
            contains_ascii(name_bytes, query_bytes)
        }
    } else {
        contains_ascii(name_bytes, query_bytes)
    }
}

pub fn vfs_search_simd(
    nodes: &[TTWorkspaceEntryNode],
    arena: &[u8],
    query: &str,
    max_results: usize,
) -> Vec<u32> {
    let mut results = Vec::new();
    if query.is_empty() || max_results == 0 {
        return results;
    }

    let query_bytes = query.as_bytes();
    let query_len = query_bytes.len();
    let is_short_prefix = query_len <= 8;

    let q_prefix_lower = if is_short_prefix {
        ascii_to_lower_u64(make_sort_prefix(query_bytes))
    } else {
        0
    };

    let prefix_mask = if is_short_prefix {
        if query_len == 8 {
            0xFFFF_FFFF_FFFF_FFFF
        } else {
            !((1u64 << ((8 - query_len) * 8)) - 1)
        }
    } else {
        0
    };

    let is_ascii_query = query_bytes.is_ascii();
    let query_lower = (!is_ascii_query).then(|| query.to_lowercase());

    for (i, node) in nodes.iter().enumerate() {
        if results.len() >= max_results {
            break;
        }

        let prefix_matched = is_short_prefix
            && (ascii_to_lower_u64(node.sort_prefix) & prefix_mask) == q_prefix_lower;

        let matched = prefix_matched
            || node
                .name(arena)
                .is_some_and(|name_bytes| {
                    match_substring_bytes(name_bytes, query_bytes, query_lower.as_deref(), is_ascii_query)
                });

        if matched {
            results.push(i as u32);
        }
    }

    results
}

#[inline]
fn find_match_position(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    if haystack.len() < needle.len() {
        return None;
    }
    let first = fold_ascii(needle[0]);
    let end = haystack.len() - needle.len();
    for i in 0..=end {
        if fold_ascii(haystack[i]) == first && haystack[i..i + needle.len()].eq_ignore_ascii_case(needle) {
            return Some(i);
        }
    }
    None
}

#[inline]
fn calculate_match_score(name: &[u8], query: &[u8]) -> i32 {
    if name.len() == query.len() && name.eq_ignore_ascii_case(query) {
        return 1000;
    }
    if name.len() >= query.len() && name[..query.len()].eq_ignore_ascii_case(query) {
        return 500 - (name.len() as i32).min(200);
    }
    if let Some(pos) = find_match_position(name, query) {
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

#[inline]
pub(crate) fn evaluate_node_match(
    node: &TTWorkspaceEntryNode,
    string_arena: &[u8],
    query_bytes: &[u8],
    query_lower: Option<&str>,
    is_ascii_query: bool,
    is_short_prefix: bool,
    prefix_mask: u64,
    q_prefix_lower: u64,
) -> Option<i32> {
    let query_len = query_bytes.len();
    let node_prefix_lower = ascii_to_lower_u64(node.sort_prefix);

    // Tier 1: In-register prefix match (zero StringArena access for <= 8 bytes)
    if (node_prefix_lower & prefix_mask) == q_prefix_lower {
        if is_short_prefix {
            let score = if (node.name_len as usize) == query_len {
                1000
            } else {
                500 - (node.name_len as i32).min(200)
            };
            return Some(score);
        }
        if let Some(name_bytes) = node.name(string_arena) {
            if name_bytes.len() >= query_len && name_bytes[..query_len].eq_ignore_ascii_case(query_bytes) {
                let score = if name_bytes.len() == query_len {
                    1000
                } else {
                    500 - (name_bytes.len() as i32).min(200)
                };
                return Some(score);
            }
        }
    }

    // Tier 2: Universal substring scan via NEON SIMD
    if let Some(name_bytes) = node.name(string_arena) {
        if match_substring_bytes(name_bytes, query_bytes, query_lower, is_ascii_query) {
            return Some(calculate_match_score(name_bytes, query_bytes));
        }
    }

    None
}

/// Four-tier hardware pipeline search:
/// - Tier 0: 64-bit character classification Bloom mask pre-filter (single-cycle SLC check)
/// - Tier 1: In-register prefix match (zero StringArena dereference for k <= 8)
/// - Tier 2: Vectorized NEON SIMD substring scan
/// - Tier 3: 64-bit composite sort key convergence via LazyTopKSelector
pub fn search_with_four_tier_pipeline(
    name_masks: &[u64],
    nodes: &[TTWorkspaceEntryNode],
    string_arena: &[u8],
    query: &str,
    limit: usize,
) -> Vec<u32> {
    if query.is_empty() || limit == 0 || nodes.is_empty() {
        return Vec::new();
    }

    let query_bytes = query.as_bytes();
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
        } else {
            !((1u64 << ((8 - query_len) * 8)) - 1)
        }
    } else {
        0xFFFF_FFFF_FFFF_FFFF
    };

    let is_ascii_query = query_bytes.is_ascii();
    let query_lower = (!is_ascii_query).then(|| query.to_lowercase());
    let mut selector = LazyTopKSelector::new(limit);

    for (i, node) in nodes.iter().enumerate() {
        // Tier 0: Single-cycle character Bloom mask pre-filtering (SLC-resident)
        if let Some(&node_mask) = name_masks.get(i) {
            if (q_char_mask & !node_mask) != 0 {
                continue;
            }
        }

        if let Some(score) = evaluate_node_match(
            node,
            string_arena,
            query_bytes,
            query_lower.as_deref(),
            is_ascii_query,
            is_short_prefix,
            prefix_mask,
            q_prefix_lower,
        ) {
            let key = make_composite_sort_key(score, i as u32);
            selector.push(key);
        }
    }

    selector.finish_entries()
}

pub use crate::semantic_search::search_hybrid_nodes_top_k;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fold_ascii_exhaustive() {
        for b in 0..=255u8 {
            assert_eq!(fold_ascii(b), b.to_ascii_lowercase(), "Mismatch on byte {}", b);
        }
        const CONST_A: u8 = fold_ascii(b'A');
        const CONST_Z: u8 = fold_ascii(b'Z');
        const CONST_LOWER: u8 = fold_ascii(b'm');
        const CONST_DIGIT: u8 = fold_ascii(b'5');
        assert_eq!(CONST_A, b'a');
        assert_eq!(CONST_Z, b'z');
        assert_eq!(CONST_LOWER, b'm');
        assert_eq!(CONST_DIGIT, b'5');
    }

    #[test]
    fn test_compute_name_mask_and_matching() {
        let mask = compute_name_mask(b"Focus.2015.2160p.mkv");

        // Bits 0..25: contains f, o, c, u, s, p, m, k, v
        assert_ne!(mask & (1 << (b'f' - b'a')), 0);
        assert_ne!(mask & (1 << (b'o' - b'a')), 0);
        assert_ne!(mask & (1 << (b'c' - b'a')), 0);
        assert_ne!(mask & (1 << (b'u' - b'a')), 0);
        assert_ne!(mask & (1 << (b's' - b'a')), 0);
        assert_ne!(mask & (1 << (b'm' - b'a')), 0);
        assert_ne!(mask & (1 << (b'k' - b'a')), 0);
        assert_ne!(mask & (1 << (b'v' - b'a')), 0);

        // Bits 26..35: contains '0', '1', '2', '5', '6'
        assert_ne!(mask & (1 << 26), 0);
        assert_ne!(mask & (1 << (26 + (b'1' - b'0'))), 0);
        assert_ne!(mask & (1 << (26 + (b'2' - b'0'))), 0);
        assert_ne!(mask & (1 << (26 + (b'5' - b'0'))), 0);
        assert_ne!(mask & (1 << (26 + (b'6' - b'0'))), 0);

        // Bit 38: contains '.'
        assert_ne!(mask & (1 << 38), 0);

        // Case-insensitivity: "focus" vs "FOCUS" produces identical alpha mask
        let mask_upper = compute_name_mask(b"FOCUS");
        let mask_lower = compute_name_mask(b"focus");
        assert_eq!(mask_upper & 0x03FF_FFFF, mask_lower & 0x03FF_FFFF);

        // Matching logic
        let q_focus = compute_name_mask(b"focus");
        assert!(matches_name_mask(mask, q_focus));

        let q_mkv = compute_name_mask(b"mkv");
        assert!(matches_name_mask(mask, q_mkv));

        // Query with missing character ('z')
        let q_invalid = compute_name_mask(b"focus_zip");
        assert!(!matches_name_mask(mask, q_invalid));

        // Word initial camelCase test
        let camel_mask = compute_name_mask(b"NativeArchiveTableViewAdapter");
        let q_adapter = compute_name_mask(b"adapter");
        assert!(matches_name_mask(camel_mask, q_adapter));
    }

    #[test]
    fn test_composite_sort_key_order_and_roundtrip() {
        let test_cases: &[(i32, u32)] = &[
            (0, 0),
            (0, 42),
            (1000, 1),
            (-500, 10),
            (i32::MIN, 0),
            (i32::MIN, u32::MAX),
            (i32::MAX, 0),
            (i32::MAX, u32::MAX),
        ];

        for &(score, entry_id) in test_cases {
            let key = make_composite_sort_key(score, entry_id);
            let (unpacked_score, unpacked_id) = unpack_composite_sort_key(key);
            assert_eq!(unpacked_score, score);
            assert_eq!(unpacked_id, entry_id);
        }

        // Monotonic ordering: higher score yields strictly larger key
        let key_high = make_composite_sort_key(100, 5);
        let key_low = make_composite_sort_key(99, 5);
        assert!(key_high > key_low);

        let key_neg_high = make_composite_sort_key(-10, 5);
        let key_neg_low = make_composite_sort_key(-20, 5);
        assert!(key_neg_high > key_neg_low);

        // Equal score tie-breaking: smaller entry_id yields larger key
        let key_id1 = make_composite_sort_key(100, 1);
        let key_id2 = make_composite_sort_key(100, 2);
        assert!(key_id1 > key_id2);
    }

    #[test]
    fn test_lazy_top_k_selector() {
        let mut selector = LazyTopKSelector::new(3);

        // Push 7 items to trigger lazy truncation at 2*k = 6
        selector.push(make_composite_sort_key(10, 0));
        selector.push(make_composite_sort_key(50, 1));
        selector.push(make_composite_sort_key(30, 2));
        selector.push(make_composite_sort_key(80, 3));
        selector.push(make_composite_sort_key(20, 4));
        selector.push(make_composite_sort_key(70, 5)); // Reaches 6: truncated to top 3 (80, 70, 50)
        selector.push(make_composite_sort_key(60, 6)); // 60 > 50: added

        let results = selector.finish_tuples();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], (80, 3));
        assert_eq!(results[1], (70, 5));
        assert_eq!(results[2], (60, 6));

        // Edge case: k = 0
        let mut zero_sel = LazyTopKSelector::new(0);
        zero_sel.push(make_composite_sort_key(10, 0));
        assert!(zero_sel.finish().is_empty());

        // Edge case: fewer items than k
        let mut small_sel = LazyTopKSelector::new(5);
        small_sel.push(make_composite_sort_key(10, 0));
        small_sel.push(make_composite_sort_key(20, 1));
        let small_res = small_sel.finish_tuples();
        assert_eq!(small_res, vec![(20, 1), (10, 0)]);
    }

    #[test]
    fn test_search_with_four_tier_pipeline_comprehensive() {
        let mut arena = Vec::new();
        let names = [
            "Focus.2015.2160p.mkv",
            "README.md",
            "Cargo.toml",
            "document_presentation.pdf",
            "avatar_profile_large_image.png",
            "test_suite_runner.rs",
            "NativeArchiveTableViewAdapter.swift",
        ];

        let mut nodes = Vec::new();
        let mut name_masks = Vec::new();

        for (i, name) in names.iter().enumerate() {
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

        // Tier 0 pruning: query with characters not in any file returns empty immediately
        let r_none = search_with_four_tier_pipeline(&name_masks, &nodes, &arena, "xyz999", 10);
        assert!(r_none.is_empty());

        // Tier 1 prefix match: "focus" matches node 0
        let r_focus = search_with_four_tier_pipeline(&name_masks, &nodes, &arena, "focus", 10);
        assert_eq!(r_focus, vec![0]);

        // Tier 2 substring match: "presentation" matches node 3
        let r_pres = search_with_four_tier_pipeline(&name_masks, &nodes, &arena, "presentation", 10);
        assert_eq!(r_pres, vec![3]);

        // Tier 2 camelCase match: "adapter" matches node 6
        let r_adapt = search_with_four_tier_pipeline(&name_masks, &nodes, &arena, "adapter", 10);
        assert_eq!(r_adapt, vec![6]);

        // Top-K ranking: searching "test" matches "test_suite_runner.rs" as prefix (node 5)
        let r_test = search_with_four_tier_pipeline(&name_masks, &nodes, &arena, "test", 10);
        assert_eq!(r_test, vec![5]);

        // Limit truncation test
        let r_limit = search_with_four_tier_pipeline(&name_masks, &nodes, &arena, "e", 2);
        assert_eq!(r_limit.len(), 2);
    }
}

