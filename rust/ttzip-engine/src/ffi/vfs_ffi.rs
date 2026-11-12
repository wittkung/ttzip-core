// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! C-ABI / FFI export functions for Mach-VM VFS Snapshot and Search.

use libc::c_char;
use std::ffi::CStr;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub use super::vfs_cache_ffi::TTZipVfsCacheHandle;
pub use super::vfs_snapshot_ffi::{
    ttzip_vfs_open_compact_catalog, ttzip_vfs_open_snapshot, ttzip_vfs_release_snapshot,
};

use crate::types::{TTWorkspaceSnapshotHeader, TTWorkspaceEntryNode};
use ttzip_types::snapshot::{TT_SECTION_VFS_NODES, TT_SECTION_STRING_ARENA};
pub use ttzip_types::vfs_handle::ttvfs_lease_s;
use ttzip_types::semantic_search::{search_hybrid_compact_nodes_top_k, HybridSearchQuery};
use ttzip_types::search::search_hybrid_nodes_top_k;

pub use ttzip_types::snapshot::TT_FEATURE_FLAG_COMPACT_NODES;

pub type TTWorkspaceCompactNode = ttzip_types::CompactNode;

/// 48-byte, 8-byte aligned borrowed snapshot view (`ttworkspace_cabi.h`).
/// Pointers borrow from the owning `ttvfs_lease_s` mapping and must not outlive it; `nodes` points
/// to 24B `TTWorkspaceCompactNode` or 64B `TTWorkspaceEntryNode` records per `header.feature_flags`.
#[repr(C, align(8))]
#[derive(Copy, Clone, Debug)]
pub struct TTWorkspaceVfsView {
    pub header: *const TTWorkspaceSnapshotHeader,
    pub nodes: *const TTWorkspaceEntryNode,
    pub node_count: u64,
    pub string_arena: *const libc::c_char,
    pub string_arena_len: u64,
    pub generation: u64,
}

const _: () = {
    assert!(core::mem::size_of::<ttvfs_lease_s>() == 32);
    assert!(core::mem::align_of::<ttvfs_lease_s>() == 8);
    assert!(core::mem::size_of::<TTWorkspaceVfsView>() == 48);
    assert!(core::mem::align_of::<TTWorkspaceVfsView>() == 8);
    assert!(core::mem::size_of::<TTWorkspaceCompactNode>() == 24);
    assert!(core::mem::align_of::<TTWorkspaceCompactNode>() == 8);
};

impl TTWorkspaceVfsView {
    /// Reinterprets `nodes` as a pointer to 24-byte `TTWorkspaceCompactNode` records without checking `is_compact_format()`.
    #[inline]
    pub const fn compact_nodes(&self) -> *const TTWorkspaceCompactNode {
        self.nodes as *const TTWorkspaceCompactNode
    }

    #[inline]
    pub fn set_compact_nodes(&mut self, ptr: *const TTWorkspaceCompactNode) {
        self.nodes = ptr as *const TTWorkspaceEntryNode;
    }

    /// # Safety
    /// This view must use the 24-byte compact node layout, `nodes` must be 8-byte aligned and valid
    /// for `node_count` `TTWorkspaceCompactNode` records, and the backing lease must outlive the slice.
    #[inline]
    pub unsafe fn as_compact_slice(&self) -> Option<&[TTWorkspaceCompactNode]> {
        unsafe {
            if self.nodes.is_null() {
                None
            } else {
                Some(std::slice::from_raw_parts(
                    self.compact_nodes(),
                    self.node_count as usize,
                ))
            }
        }
    }

    /// Returns the node record byte stride: 24 bytes when `is_compact_format()` is true, else 64 bytes.
    #[inline]
    pub fn node_byte_size(&self) -> usize {
        if self.is_compact_format() {
            core::mem::size_of::<TTWorkspaceCompactNode>()
        } else {
            core::mem::size_of::<TTWorkspaceEntryNode>()
        }
    }

    /// Returns `true` when `header` is null, `TT_FEATURE_FLAG_COMPACT_NODES` (`1 << 4`) is set, or `TT_FEATURE_FLAG_CSR` (`1 << 0`) is clear.
    #[inline]
    pub fn is_compact_format(&self) -> bool {
        if self.header.is_null() {
            true
        } else {
            unsafe { (*self.header).is_compact_format() }
        }
    }

    /// Returns `true` when `header` is non-null, `TT_FEATURE_FLAG_CSR` is set, and `TT_FEATURE_FLAG_COMPACT_NODES` is clear.
    #[inline]
    pub fn is_legacy_format(&self) -> bool {
        !self.is_compact_format()
    }

    #[inline]
    pub fn from_compact_nodes(
        header: *const TTWorkspaceSnapshotHeader,
        compact_nodes: *const TTWorkspaceCompactNode,
        node_count: u64,
        string_arena: *const libc::c_char,
        string_arena_len: u64,
        generation: u64,
    ) -> Self {
        Self {
            header,
            nodes: compact_nodes as *const TTWorkspaceEntryNode,
            node_count,
            string_arena,
            string_arena_len,
            generation,
        }
    }
}

#[inline(always)]
fn contains_case_insensitive_ascii(haystack: &[u8], needle: &[u8]) -> bool {
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

#[inline(always)]
fn contains_case_insensitive(
    haystack: &[u8],
    needle: &[u8],
    needle_lower: Option<&str>,
    is_ascii: bool,
) -> bool {
    if needle.is_empty() {
        return true;
    }
    if is_ascii && haystack.is_ascii() {
        contains_case_insensitive_ascii(haystack, needle)
    } else if let Some(q_low) = needle_lower {
        if let Ok(s) = core::str::from_utf8(haystack) {
            s.to_lowercase().contains(q_low)
        } else {
            haystack.windows(needle.len()).any(|w| w.eq_ignore_ascii_case(needle))
        }
    } else {
        haystack.windows(needle.len()).any(|w| w.eq_ignore_ascii_case(needle))
    }
}

unsafe fn validate_lease_and_arena<'a>(
    lease: *const ttvfs_lease_s,
) -> Result<(&'a TTWorkspaceSnapshotHeader, &'a [u8], &'a [u8]), i32> {
    let l = unsafe { &*lease };
    let ptr = l.mapping;
    if ptr.is_null() || l.size < std::mem::size_of::<TTWorkspaceSnapshotHeader>() {
        return Err(-1);
    }
    let (header, slice) = unsafe { TTWorkspaceSnapshotHeader::validate_mapping(ptr, l.size) }
        .map_err(|_| -5)?;
    let (_, arena_slice) = header
        .validate_section_bounds(slice, TT_SECTION_STRING_ARENA, 1, 1)
        .ok_or(-5)?;
    Ok((header, slice, arena_slice))
}

#[allow(clippy::too_many_arguments, reason = "Internal VFS search loop forwards validated arena slices and query state")]
unsafe fn scan_compact_nodes_matching(
    compact_nodes: &[TTWorkspaceCompactNode],
    arena_slice: &[u8],
    query_bytes: &[u8],
    query_lower: Option<&str>,
    is_ascii_query: bool,
    limit: usize,
    out_indices: *mut u32,
) -> usize {
    let mut count = 0;
    for (i, node) in compact_nodes.iter().enumerate() {
        if count >= limit {
            break;
        }
        let start = usize::try_from(node.name_offset).ok();
        let end = start.and_then(|s| s.checked_add(usize::from(node.name_len)));
        if let Some(path_bytes) = start.zip(end).and_then(|(s, e)| arena_slice.get(s..e)) {
            if contains_case_insensitive(path_bytes, query_bytes, query_lower, is_ascii_query) {
                unsafe { *out_indices.add(count) = i as u32 };
                count += 1;
            }
        }
    }
    count
}

#[allow(clippy::too_many_arguments, reason = "Internal VFS search loop forwards validated arena slices and query state")]
unsafe fn scan_legacy_nodes_matching(
    nodes: &[TTWorkspaceEntryNode],
    arena_slice: &[u8],
    query_bytes: &[u8],
    query_lower: Option<&str>,
    is_ascii_query: bool,
    limit: usize,
    out_indices: *mut u32,
) -> usize {
    let mut count = 0;
    for (i, node) in nodes.iter().enumerate() {
        if count >= limit {
            break;
        }
        if let Some(path_bytes) = node.name(arena_slice) {
            if contains_case_insensitive(path_bytes, query_bytes, query_lower, is_ascii_query) {
                unsafe { *out_indices.add(count) = i as u32 };
                count += 1;
            }
        }
    }
    count
}

/// Searches the VFS string arena for nodes matching `query` (`0` on success, `-1` invalid arg, `-4` panic, `-5` corrupt/unaligned).
/// # Safety
/// `lease` must be null or point to a live `ttvfs_lease_s` whose `mapping` (when non-null) is readable for `lease.size` bytes;
/// `query` must be null or a valid UTF-8 C string; `out_indices` must be writable for `max_results` `u32`s and `out_count` for one `u32`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ttzip_vfs_search(
    lease: *mut ttvfs_lease_s,
    query: *const c_char,
    max_results: u32,
    out_indices: *mut u32,
    out_count: *mut u32,
) -> i32 {
    unsafe {
        let result = catch_unwind(AssertUnwindSafe(|| {
            if lease.is_null() || query.is_null() || out_indices.is_null() || out_count.is_null() {
                return -1;
            }
            if max_results == 0 {
                *out_count = 0;
                return 0;
            }

            let Ok(q_str) = CStr::from_ptr(query).to_str() else {
                return -1;
            };
            let query_bytes = q_str.as_bytes();
            let is_ascii_query = query_bytes.is_ascii();
            let query_lower = (!is_ascii_query).then(|| q_str.to_lowercase());

            let (header, slice, arena_slice) = match validate_lease_and_arena(lease) {
                Ok(v) => v,
                Err(code) => return code,
            };

            let limit = max_results as usize;
            let count = if header.is_compact_format() {
                let Some((_, compact_nodes)) = header
                    .get_typed_section_slice::<TTWorkspaceCompactNode>(slice, TT_SECTION_VFS_NODES)
                else {
                    return -5;
                };
                scan_compact_nodes_matching(
                    compact_nodes,
                    arena_slice,
                    query_bytes,
                    query_lower.as_deref(),
                    is_ascii_query,
                    limit,
                    out_indices,
                )
            } else {
                let Some((_, nodes)) =
                    header.get_typed_section_slice::<TTWorkspaceEntryNode>(slice, TT_SECTION_VFS_NODES)
                else {
                    return -5;
                };
                scan_legacy_nodes_matching(
                    nodes,
                    arena_slice,
                    query_bytes,
                    query_lower.as_deref(),
                    is_ascii_query,
                    limit,
                    out_indices,
                )
            };

            *out_count = count as u32;
            0
        }));

        result.unwrap_or(-4)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ttzip_hybrid_query_t {
    pub text_query: *const c_char,
    pub query_vector: *const i8,
    pub limit: u32,
    pub lexical_weight: f32,
    pub semantic_weight: f32,
    pub min_similarity: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ttzip_hybrid_match_t {
    pub node_id: u32,
    pub lexical_score: i32,
    pub semantic_score: i32,
    pub fused_score: f32,
}

const _: () = {
    assert!(core::mem::size_of::<ttzip_hybrid_query_t>() == 32);
    assert!(core::mem::align_of::<ttzip_hybrid_query_t>() == 8);
    assert!(core::mem::size_of::<ttzip_hybrid_match_t>() == 16);
    assert!(core::mem::align_of::<ttzip_hybrid_match_t>() == 4);
};

/// Direct ARM NEON SIMD (or scalar fallback) dot product over 128 `i8` values in `[-2048383, 2048383]`, or `0` if null.
/// # Safety
/// `a` and `b` must each be null or point to at least 128 readable `i8` elements.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ttzip_neon_dot_product_128(
    a: *const i8,
    b: *const i8,
) -> i32 {
    unsafe {
        let result = catch_unwind(AssertUnwindSafe(|| {
            if a.is_null() || b.is_null() {
                return 0;
            }
            let a_slice = std::slice::from_raw_parts(a, 128);
            let b_slice = std::slice::from_raw_parts(b, 128);
            let a_arr: &[i8; 128] = match a_slice.try_into() {
                Ok(arr) => arr,
                Err(_) => return 0,
            };
            let b_arr: &[i8; 128] = match b_slice.try_into() {
                Ok(arr) => arr,
                Err(_) => return 0,
            };
            ttzip_types::simd_dot::dot_product_128(a_arr, b_arr)
        }));
        result.unwrap_or(0)
    }
}

unsafe fn build_hybrid_query_from_ffi<'a>(
    q: &'a ttzip_hybrid_query_t,
    max_matches: u32,
) -> Result<Option<HybridSearchQuery<'a>>, i32> {
    let text_str = if q.text_query.is_null() {
        ""
    } else {
        unsafe { CStr::from_ptr(q.text_query) }.to_str().map_err(|_| -1)?
    };
    let vec_ref: Option<&'a [i8; 128]> = if q.query_vector.is_null() {
        None
    } else {
        let vec_slice = unsafe { std::slice::from_raw_parts(q.query_vector, 128) };
        vec_slice.try_into().ok()
    };
    let req_limit = if q.limit == 0 {
        max_matches
    } else {
        q.limit.min(max_matches)
    };
    if req_limit == 0 {
        return Ok(None);
    }
    let lex_weight = if q.lexical_weight > 0.0 {
        q.lexical_weight
    } else {
        ttzip_types::semantic_search::DEFAULT_LEXICAL_WEIGHT
    };
    let sem_weight = if q.semantic_weight > 0.0 {
        q.semantic_weight
    } else {
        ttzip_types::semantic_search::DEFAULT_SEMANTIC_WEIGHT
    };
    let mut h_query = HybridSearchQuery::new(text_str)
        .with_limit(req_limit)
        .with_weights(lex_weight, sem_weight)
        .with_min_similarity(q.min_similarity);
    if let Some(v) = vec_ref {
        h_query = h_query.with_vector(v);
    }
    Ok(Some(h_query))
}

unsafe fn run_hybrid_search_on_snapshot(
    header: &TTWorkspaceSnapshotHeader,
    slice: &[u8],
    arena_slice: &[u8],
    h_query: &HybridSearchQuery<'_>,
) -> Result<Vec<ttzip_types::semantic_search::HybridSearchCandidate>, i32> {
    let name_masks: &[u64] = if header
        .find_section(slice, ttzip_types::snapshot::TT_SECTION_TYPE_NAME_MASKS)
        .is_some()
    {
        unsafe {
            header.get_typed_section_slice::<u64>(
                slice,
                ttzip_types::snapshot::TT_SECTION_TYPE_NAME_MASKS,
            )
        }
        .map(|(_, masks)| masks)
        .ok_or(-5)?
    } else {
        &[]
    };
    let vectors_slice = header.find_embeddings(slice).map(|(_hdr, vecs)| vecs);
    if header.is_compact_format() {
        let (_, compact_nodes) = unsafe {
            header.get_typed_section_slice::<TTWorkspaceCompactNode>(slice, TT_SECTION_VFS_NODES)
        }
        .ok_or(-5)?;
        Ok(search_hybrid_compact_nodes_top_k(
            name_masks,
            compact_nodes,
            arena_slice,
            vectors_slice,
            h_query,
        ))
    } else {
        let (_, nodes) = unsafe {
            header.get_typed_section_slice::<TTWorkspaceEntryNode>(slice, TT_SECTION_VFS_NODES)
        }
        .ok_or(-5)?;
        Ok(search_hybrid_nodes_top_k(
            name_masks,
            nodes,
            arena_slice,
            vectors_slice,
            h_query,
        ))
    }
}

/// Hybrid lexical + 128-D `i8` SIMD top-K search over a Mach VM snapshot (`0` on success, `-1` invalid arg, `-4` panic, `-5` corrupt/unaligned).
/// # Safety
/// `snapshot_handle` must be null or point to a live `ttvfs_lease_s` whose `mapping` (when non-null) is readable for `lease.size` bytes;
/// `query` must be null or valid (`text_query` null/UTF-8 C string, `query_vector` null/128 `i8`s); `out_matches` writable for `max_matches` and `out_count` for one `u32`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ttzip_vfs_hybrid_search(
    snapshot_handle: *const libc::c_void,
    query: *const ttzip_hybrid_query_t,
    out_matches: *mut ttzip_hybrid_match_t,
    max_matches: u32,
    out_count: *mut u32,
) -> i32 {
    unsafe {
        let result = catch_unwind(AssertUnwindSafe(|| {
            if snapshot_handle.is_null()
                || query.is_null()
                || out_matches.is_null()
                || out_count.is_null()
            {
                return -1;
            }
            if max_matches == 0 {
                *out_count = 0;
                return 0;
            }

            let lease_ptr = snapshot_handle as *const ttvfs_lease_s;
            let (header, slice, arena_slice) = match validate_lease_and_arena(lease_ptr) {
                Ok(v) => v,
                Err(code) => return code,
            };

            let h_query = match build_hybrid_query_from_ffi(&*query, max_matches) {
                Ok(Some(hq)) => hq,
                Ok(None) => {
                    *out_count = 0;
                    return 0;
                }
                Err(code) => return code,
            };

            let candidates =
                match run_hybrid_search_on_snapshot(header, slice, arena_slice, &h_query) {
                    Ok(c) => c,
                    Err(code) => return code,
                };

            let write_count = candidates.len().min(max_matches as usize);
            for (i, cand) in candidates[..write_count].iter().enumerate() {
                *out_matches.add(i) = ttzip_hybrid_match_t {
                    node_id: cand.node_id,
                    lexical_score: cand.lexical_score,
                    semantic_score: cand.semantic_dot_product,
                    fused_score: cand.fused_score,
                };
            }

            *out_count = write_count as u32;
            0
        }));

        result.unwrap_or(-4)
    }
}
