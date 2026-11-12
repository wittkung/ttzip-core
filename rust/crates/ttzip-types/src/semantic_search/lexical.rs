// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Lexical pre-filtering and compact substring evaluation for hybrid search.

use crate::node::{make_sort_prefix, CompactNode, TTWorkspaceEntryNode};
use crate::search::{ascii_to_lower_u64, compute_name_mask, evaluate_node_match, MASK_CHAR_SET};

pub(super) struct PreparedLexicalQuery<'a> {
    pub(super) has_text: bool,
    query_bytes: &'a [u8],
    q_char_mask: u64,
    is_short_prefix: bool,
    prefix_mask: u64,
    q_prefix_lower: u64,
    is_ascii_query: bool,
    query_lower: Option<String>,
}

impl<'a> PreparedLexicalQuery<'a> {
    pub(super) fn new(text_query: &'a str) -> Self {
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
    pub(super) fn evaluate_node(
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
    pub(super) fn evaluate_compact_node(
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
        if match_compact_substring(
            name_bytes,
            self.query_bytes,
            self.query_lower.as_deref(),
            self.is_ascii_query,
        ) {
            calculate_compact_match_score(name_bytes, self.query_bytes)
        } else {
            0
        }
    }
}

#[inline(always)]
pub(super) fn compact_node_name<'a>(node: &CompactNode, arena: &'a [u8]) -> Option<&'a [u8]> {
    let start = node.name_offset as usize;
    let end = start.checked_add(node.name_len as usize)?;
    arena.get(start..end)
}

#[inline(always)]
fn contains_ascii_case_insensitive(haystack: &[u8], needle: &[u8]) -> bool {
    find_compact_match_pos(haystack, needle).is_some()
}

#[inline]
fn match_compact_substring(
    name_bytes: &[u8],
    query_bytes: &[u8],
    query_lower: Option<&str>,
    is_ascii_query: bool,
) -> bool {
    if is_ascii_query && name_bytes.is_ascii() {
        return contains_ascii_case_insensitive(name_bytes, query_bytes);
    }
    if let Some(q_low) = query_lower {
        if let Ok(name_str) = core::str::from_utf8(name_bytes) {
            return name_str.to_lowercase().contains(q_low);
        }
    }
    contains_ascii_case_insensitive(name_bytes, query_bytes)
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
    let Some(pos) = find_compact_match_pos(name, query) else {
        return 100;
    };
    let is_word_boundary = pos == 0 || {
        let prev = name[pos - 1];
        matches!(prev, b'-' | b'_' | b'.' | b'/' | b' ')
            || (name[pos].is_ascii_uppercase() && prev.is_ascii_lowercase())
    };
    if is_word_boundary {
        300 - (pos as i32).min(100)
    } else {
        100 - (pos as i32).min(50)
    }
}
