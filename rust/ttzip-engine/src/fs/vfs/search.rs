// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

//! High-performance fuzzy string matching and scoring algorithms for VFS trees.

use super::node::VfsNode;

/// Result of a fuzzy query match against a VFS node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VfsSearchResult {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub is_encrypted: bool,
    pub score: i64,
    pub match_indices: Vec<usize>,
}

/// Evaluates fuzzy string matching score and byte match indices with zero heap allocations.
pub fn fuzzy_match(target: &str, pattern: &str) -> Option<(i64, Vec<usize>)> {
    let pat = pattern.trim();
    if pat.is_empty() {
        return Some((0, Vec::new()));
    }

    let mut pat_chars = pat.chars();
    let mut current_pat_char = match pat_chars.next() {
        Some(c) => c,
        None => return Some((0, Vec::new())),
    };

    let mut indices = Vec::with_capacity(pat.len());
    let mut score = 0i64;
    let mut prev_matched_idx: Option<usize> = None;
    let mut prev_c = '\0';

    for (t_idx, tc) in target.char_indices() {
        if tc.eq_ignore_ascii_case(&current_pat_char) {
            indices.push(t_idx);
            let mut char_score = 10i64;
            if tc == current_pat_char {
                char_score += 5; // Exact case match
            }

            // Word boundary bonus
            if t_idx == 0 || prev_c == '/' || prev_c == '\\' || prev_c == '_' || prev_c == '-' || prev_c == '.' || prev_c == ' ' {
                char_score += 30;
            } else if prev_c.is_lowercase() && tc.is_uppercase() {
                char_score += 25; // CamelCase boundary
            }

            // Consecutive match bonus
            if let Some(prev) = prev_matched_idx {
                if t_idx == prev + 1 {
                    char_score += 20;
                } else {
                    char_score -= (t_idx.saturating_sub(prev + 1)) as i64; // Distance penalty
                }
            }

            score += char_score;
            prev_matched_idx = Some(t_idx);

#[inline]
fn starts_with_ignore_case(s: &str, prefix: &str) -> bool {
    let mut s_chars = s.chars().flat_map(|c| c.to_lowercase());
    let mut p_chars = prefix.chars().flat_map(|c| c.to_lowercase());
    loop {
        match p_chars.next() {
            Some(pc) => match s_chars.next() {
                Some(sc) if sc == pc => continue,
                _ => return false,
            },
            None => return true,
        }
    }
}

            match pat_chars.next() {
                Some(next_c) => current_pat_char = next_c,
                None => {
                    // Pattern completely matched!
                    if target.eq_ignore_ascii_case(pat) {
                        score += 500;
                    } else if starts_with_ignore_case(target, pat) {
                        score += 200;
                    }
                    return Some((score, indices));
                }
            }
        }
        prev_c = tc;
    }

    None
}

/// Recursively searches nodes in hierarchy and returns sorted matching items.
pub fn fuzzy_search_tree(root_node: &VfsNode, query: &str) -> Vec<VfsSearchResult> {
    let mut results = Vec::new();
    fn traverse(node: &VfsNode, query: &str, out: &mut Vec<VfsSearchResult>) {
        let name_match = fuzzy_match(&node.name, query);
        let path_match = fuzzy_match(&node.path, query);

        if let Some((score, indices)) = name_match {
            out.push(VfsSearchResult {
                name: node.name.clone(),
                path: node.path.clone(),
                is_directory: node.is_directory,
                uncompressed_size: node.uncompressed_size,
                compressed_size: node.compressed_size,
                crc32: node.crc32,
                is_encrypted: node.is_encrypted,
                score: score + 100, // Boost filename matches
                match_indices: indices,
            });
        } else if let Some((score, indices)) = path_match {
            out.push(VfsSearchResult {
                name: node.name.clone(),
                path: node.path.clone(),
                is_directory: node.is_directory,
                uncompressed_size: node.uncompressed_size,
                compressed_size: node.compressed_size,
                crc32: node.crc32,
                is_encrypted: node.is_encrypted,
                score,
                match_indices: indices,
            });
        }

        for child in &node.children {
            traverse(child, query, out);
        }
    }

    for child in &root_node.children {
        traverse(child, query, &mut results);
    }
    results.sort_by_key(|b| std::cmp::Reverse(b.score));
    results
}

/// Zero-allocation fuzzy matching returning purely the match score without heap allocations.
#[inline]
pub fn fuzzy_match_zero_alloc(target: &str, pattern: &str) -> Option<i64> {
    let pat = pattern.trim();
    if pat.is_empty() {
        return Some(0);
    }

    let mut pat_chars = pat.chars();
    let mut current_pat_char = match pat_chars.next() {
        Some(c) => c,
        None => return Some(0),
    };

    let mut score = 0i64;
    let mut prev_matched_idx: Option<usize> = None;
    let mut prev_c = '\0';

    for (t_idx, tc) in target.char_indices() {
        if tc.eq_ignore_ascii_case(&current_pat_char) {
            let mut char_score = 10i64;
            if tc == current_pat_char {
                char_score += 5;
            }

            if t_idx == 0 || prev_c == '/' || prev_c == '\\' || prev_c == '_' || prev_c == '-' || prev_c == '.' || prev_c == ' ' {
                char_score += 30;
            } else if prev_c.is_lowercase() && tc.is_uppercase() {
                char_score += 25;
            }

            if let Some(prev) = prev_matched_idx {
                if t_idx == prev + 1 {
                    char_score += 20;
                } else {
                    char_score -= (t_idx.saturating_sub(prev + 1)) as i64;
                }
            }

            score += char_score;
            prev_matched_idx = Some(t_idx);

            match pat_chars.next() {
                Some(next_c) => current_pat_char = next_c,
                None => {
                    if target.eq_ignore_ascii_case(pat) {
                        score += 500;
                    }
                    return Some(score);
                }
            }
        }
        prev_c = tc;
    }
    None
}

/// Fixed C-ABI DTO for populating pre-allocated result buffers with zero heap allocation.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TTZipVfsMatchDto {
    pub struct_size: u32,
    pub abi_version: u32,
    pub name: *const std::os::raw::c_char,
    pub name_len: usize,
    pub path: *const std::os::raw::c_char,
    pub path_len: usize,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub score: i64,
    pub is_directory: bool,
    pub is_encrypted: bool,
}

impl Default for TTZipVfsMatchDto {
    fn default() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_2,
            name: std::ptr::null(),
            name_len: 0,
            path: std::ptr::null(),
            path_len: 0,
            uncompressed_size: 0,
            compressed_size: 0,
            crc32: 0,
            score: 0,
            is_directory: false,
            is_encrypted: false,
        }
    }
}

/// Zero-allocation recursive VFS search populating a caller-provided fixed buffer slice.
pub fn search_vfs_tree_zero_alloc(
    root: &VfsNode,
    query: &str,
    out_results: &mut [TTZipVfsMatchDto],
) -> usize {
    let mut matched_count = 0usize;
    let capacity = out_results.len();

    fn traverse(
        node: &VfsNode,
        query: &str,
        out_results: &mut [TTZipVfsMatchDto],
        matched_count: &mut usize,
        capacity: usize,
    ) {
        if let Some(score) = fuzzy_match_zero_alloc(&node.name, query) {
            if *matched_count < capacity {
                out_results[*matched_count] = TTZipVfsMatchDto {
                    struct_size: std::mem::size_of::<TTZipVfsMatchDto>() as u32,
                    abi_version: crate::types::TTZIP_ABI_VERSION_2,
                    name: node.name.as_ptr() as *const _,
                    name_len: node.name.len(),
                    path: node.path.as_ptr() as *const _,
                    path_len: node.path.len(),
                    uncompressed_size: node.uncompressed_size,
                    compressed_size: node.compressed_size,
                    crc32: node.crc32,
                    score: score + 100,
                    is_directory: node.is_directory,
                    is_encrypted: node.is_encrypted,
                };
                *matched_count += 1;
            }
        } else if let Some(score) = fuzzy_match_zero_alloc(&node.path, query) {
            if *matched_count < capacity {
                out_results[*matched_count] = TTZipVfsMatchDto {
                    struct_size: std::mem::size_of::<TTZipVfsMatchDto>() as u32,
                    abi_version: crate::types::TTZIP_ABI_VERSION_2,
                    name: node.name.as_ptr() as *const _,
                    name_len: node.name.len(),
                    path: node.path.as_ptr() as *const _,
                    path_len: node.path.len(),
                    uncompressed_size: node.uncompressed_size,
                    compressed_size: node.compressed_size,
                    crc32: node.crc32,
                    score,
                    is_directory: node.is_directory,
                    is_encrypted: node.is_encrypted,
                };
                *matched_count += 1;
            }
        }

        for child in &node.children {
            traverse(child, query, out_results, matched_count, capacity);
        }
    }

    for child in &root.children {
        traverse(child, query, out_results, &mut matched_count, capacity);
    }

    matched_count
}

