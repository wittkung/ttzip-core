// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! C-ABI / FFI export functions for Mach-VM VFS Snapshot and Search.

use libc::c_char;
use std::ffi::CStr;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub use super::vfs_cache_ffi::TTZipVfsCacheHandle;

use crate::types::{TTWorkspaceSnapshotHeader, TTWorkspaceEntryNode};
use crate::types::unified_node::UnifiedVfsTable;
use ttzip_types::builder::VfsSnapshotBuilder;
use ttzip_types::mach_vm::MachVmAllocation;
use ttzip_types::snapshot::{TT_SECTION_VFS_NODES, TT_SECTION_STRING_ARENA};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
pub use ttzip_types::vfs_handle::ttvfs_lease_s;
use ttzip_types::semantic_search::{search_hybrid_compact_nodes_top_k, HybridSearchQuery};
use ttzip_types::search::search_hybrid_nodes_top_k;

pub const TT_FEATURE_FLAG_COMPACT_NODES: u32 = 1 << 4;

pub type TTWorkspaceCompactNode = ttzip_types::CompactNode;

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
    /// Returns the nodes pointer reinterpreted as a pointer to 24-byte `TTWorkspaceCompactNode` array.
    #[inline]
    pub const fn compact_nodes(&self) -> *const TTWorkspaceCompactNode {
        self.nodes as *const TTWorkspaceCompactNode
    }

    /// Sets the node pointer from a `TTWorkspaceCompactNode` pointer.
    #[inline]
    pub fn set_compact_nodes(&mut self, ptr: *const TTWorkspaceCompactNode) {
        self.nodes = ptr as *const TTWorkspaceEntryNode;
    }

    /// Borrows the compact nodes as a slice if the pointer is non-null.
    ///
    /// # Safety
    /// The caller must ensure that `nodes` points to a contiguous array of at least
    /// `node_count` elements of `TTWorkspaceCompactNode`.
    #[inline]
    pub unsafe fn as_compact_slice(&self) -> Option<&[TTWorkspaceCompactNode]> {
        if self.nodes.is_null() {
            None
        } else {
            Some(std::slice::from_raw_parts(
                self.compact_nodes(),
                self.node_count as usize,
            ))
        }
    }

    /// Returns the byte size of each node in this view:
    /// 24 bytes if compact node format is active, 64 bytes if legacy CSR format is active.
    #[inline]
    pub fn node_byte_size(&self) -> usize {
        if self.is_compact_format() {
            core::mem::size_of::<TTWorkspaceCompactNode>()
        } else {
            core::mem::size_of::<TTWorkspaceEntryNode>()
        }
    }

    /// Returns true if the compact 24-byte node format is active.
    #[inline]
    pub fn is_compact_format(&self) -> bool {
        if self.header.is_null() {
            true
        } else {
            unsafe {
                let flags = (*self.header).feature_flags;
                (flags & TT_FEATURE_FLAG_COMPACT_NODES != 0)
                    || (flags & ttzip_types::snapshot::TT_FEATURE_FLAG_CSR == 0)
            }
        }
    }

    /// Returns true if the legacy 64-byte CSR node format is active.
    #[inline]
    pub fn is_legacy_format(&self) -> bool {
        !self.is_compact_format()
    }

    /// Constructs a view from a compact node array pointer and string arena metadata.
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



#[no_mangle]
pub unsafe extern "C" fn ttzip_vfs_open_snapshot(
    archive_path: *const c_char,
    password: *const c_char,
    out_view: *mut TTWorkspaceVfsView,
) -> *mut ttvfs_lease_s {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if archive_path.is_null() || out_view.is_null() {
            return std::ptr::null_mut();
        }

        let path_str = match CStr::from_ptr(archive_path).to_str() {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        };
        let path = std::path::Path::new(path_str);

        let pwd_str = if password.is_null() {
            None
        } else {
            CStr::from_ptr(password).to_str().ok()
        };

        let mut table = UnifiedVfsTable::new();
        let res = crate::archive::unified::inspect::inspect_archive_into_vfs(
            path,
            pwd_str,
            true,
            &mut table,
        );

        if res.is_err() {
            return std::ptr::null_mut();
        }

        let alloc = VfsSnapshotBuilder::build_snapshot(&table);
        let ptr = alloc.as_ptr();
        
        let header = &*(ptr as *const TTWorkspaceSnapshotHeader);
        
        let nodes_sec = header.find_section(
            std::slice::from_raw_parts(ptr, header.total_mapping_size as usize),
            TT_SECTION_VFS_NODES
        );
        let arena_sec = header.find_section(
            std::slice::from_raw_parts(ptr, header.total_mapping_size as usize),
            TT_SECTION_STRING_ARENA
        );
        
        if nodes_sec.is_none() || arena_sec.is_none() {
            return std::ptr::null_mut();
        }

        let n_sec = nodes_sec.unwrap();
        let a_sec = arena_sec.unwrap();

        (*out_view).header = header as *const _;
        (*out_view).nodes = ptr.add(n_sec.offset as usize) as *const _;
        (*out_view).node_count = n_sec.entry_count;
        (*out_view).string_arena = ptr.add(a_sec.offset as usize) as *const _;
        (*out_view).string_arena_len = a_sec.size_bytes;
        (*out_view).generation = header.generation;

        let alloc_raw = Arc::into_raw(alloc);

        let lease = Box::new(ttvfs_lease_s {
            mapping: ptr as *mut u8,
            size: header.total_mapping_size as usize,
            ref_count: AtomicU64::new(1),
            alloc_ptr: alloc_raw as *const libc::c_void,
        });

        Box::into_raw(lease)
    }));

    result.unwrap_or(std::ptr::null_mut())
}

/// Opens an archive and constructs an in-process 24-byte compact node catalog snapshot.
#[no_mangle]
pub unsafe extern "C" fn ttzip_vfs_open_compact_catalog(
    archive_path: *const c_char,
    password: *const c_char,
    out_view: *mut TTWorkspaceVfsView,
) -> *mut ttvfs_lease_s {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if archive_path.is_null() || out_view.is_null() {
            return std::ptr::null_mut();
        }

        let path_str = match CStr::from_ptr(archive_path).to_str() {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        };
        let path = std::path::Path::new(path_str);

        let pwd_str = if password.is_null() {
            None
        } else {
            CStr::from_ptr(password).to_str().ok()
        };

        let mut table = UnifiedVfsTable::new();
        let res = crate::archive::unified::inspect::inspect_archive_into_vfs(
            path,
            pwd_str,
            true,
            &mut table,
        );

        if res.is_err() {
            return std::ptr::null_mut();
        }

        let (csr_nodes, arena) = VfsSnapshotBuilder::build(&table);
        let arena_bytes = arena.into_bytes();

        let mut compact_nodes = Vec::with_capacity(csr_nodes.len());
        let mut dir_children_map: Vec<Vec<u32>> = Vec::new();

        for node in &csr_nodes {
            let is_dir = node.is_dir();
            let dir_idx = if is_dir && node.child_count > 0 && node.first_child_id != u32::MAX {
                let children: Vec<u32> = (node.first_child_id..(node.first_child_id + node.child_count)).collect();
                let idx = dir_children_map.len() as u32;
                dir_children_map.push(children);
                idx
            } else {
                0
            };

            let mut flags = 0u16;
            if is_dir {
                flags |= 0x01; // TT_NODE_FLAG_DIR
            }
            if (node.flags & 0x02) != 0 {
                flags |= 0x02; // TT_NODE_FLAG_SYMLINK
            }
            if (node.flags & 0x04) != 0 {
                flags |= 0x04; // TT_NODE_FLAG_ENCRYPTED
            }

            compact_nodes.push(TTWorkspaceCompactNode {
                name_offset: node.name_offset,
                name_len: node.name_len,
                flags,
                size: node.uncompressed_size,
                mtime: (node.mtime_epoch_s.max(0)) as u32,
                dir_children_idx: dir_idx,
            });
        }

        let header_size = 128usize;
        let sections_size = 2 * std::mem::size_of::<ttzip_types::snapshot::TTWorkspaceSectionEntry>();
        let nodes_offset = (header_size + sections_size + 63) & !63;
        let nodes_bytes = compact_nodes.len() * std::mem::size_of::<TTWorkspaceCompactNode>();
        let arena_offset = (nodes_offset + nodes_bytes + 63) & !63;
        let arena_len = arena_bytes.len();
        let total_size = (arena_offset + arena_len + 16383) & !16383;

        let alloc = match MachVmAllocation::allocate(total_size) {
            Ok(a) => a,
            Err(_) => return std::ptr::null_mut(),
        };

        let ptr = alloc.as_ptr() as *mut u8;

        let header = TTWorkspaceSnapshotHeader {
            magic: ttzip_types::snapshot::TT_SNAPSHOT_MAGIC,
            abi_major: ttzip_types::snapshot::TT_ABI_VERSION_MAJOR,
            abi_minor: ttzip_types::snapshot::TT_ABI_VERSION_MINOR,
            header_size: 128,
            section_header_size: 32,
            section_count: 2,
            root_node_id: 0,
            total_mapping_size: total_size as u64,
            generation: 1,
            snapshot_sequence: 1,
            section_headers_offset: 128,
            feature_flags: TT_FEATURE_FLAG_COMPACT_NODES,
            reserved_flags: 0,
            snapshot_checksum: 0,
            _reserved: [0u8; 56],
        };

        std::ptr::write(ptr as *mut TTWorkspaceSnapshotHeader, header);

        let sec_nodes = ttzip_types::snapshot::TTWorkspaceSectionEntry {
            section_type: TT_SECTION_VFS_NODES,
            flags: ttzip_types::snapshot::TT_SECTION_FLAG_ALIGNED_64 | ttzip_types::snapshot::TT_SECTION_FLAG_READ_ONLY,
            offset: nodes_offset as u64,
            size_bytes: nodes_bytes as u64,
            entry_count: compact_nodes.len() as u64,
        };

        let sec_arena = ttzip_types::snapshot::TTWorkspaceSectionEntry {
            section_type: TT_SECTION_STRING_ARENA,
            flags: ttzip_types::snapshot::TT_SECTION_FLAG_READ_ONLY,
            offset: arena_offset as u64,
            size_bytes: arena_len as u64,
            entry_count: arena_len as u64,
        };

        let sec_ptr = ptr.add(128) as *mut ttzip_types::snapshot::TTWorkspaceSectionEntry;
        std::ptr::write(sec_ptr, sec_nodes);
        std::ptr::write(sec_ptr.add(1), sec_arena);

        if !compact_nodes.is_empty() {
            std::ptr::copy_nonoverlapping(
                compact_nodes.as_ptr() as *const u8,
                ptr.add(nodes_offset),
                nodes_bytes,
            );
        }

        if !arena_bytes.is_empty() {
            std::ptr::copy_nonoverlapping(
                arena_bytes.as_ptr(),
                ptr.add(arena_offset),
                arena_len,
            );
        }

        let alloc_arc = Arc::new(alloc);
        let alloc_raw = Arc::into_raw(alloc_arc);

        (*out_view).header = ptr as *const TTWorkspaceSnapshotHeader;
        (*out_view).nodes = ptr.add(nodes_offset) as *const TTWorkspaceEntryNode;
        (*out_view).node_count = compact_nodes.len() as u64;
        (*out_view).string_arena = ptr.add(arena_offset) as *const libc::c_char;
        (*out_view).string_arena_len = arena_len as u64;
        (*out_view).generation = 1;

        let lease = Box::new(ttvfs_lease_s {
            mapping: ptr,
            size: total_size,
            ref_count: AtomicU64::new(1),
            alloc_ptr: alloc_raw as *const libc::c_void,
        });

        Box::into_raw(lease)
    }));

    result.unwrap_or(std::ptr::null_mut())
}

#[cfg(target_os = "macos")]
extern "C" {
    fn mach_task_self() -> u32;
    fn mach_vm_deallocate(target: u32, address: u64, size: u64) -> i32;
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_vfs_release_snapshot(lease: *mut ttvfs_lease_s) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if lease.is_null() {
            return;
        }
        let l = unsafe { &*lease };
        if l.ref_count.fetch_sub(1, Ordering::Release) == 1 {
            std::sync::atomic::fence(Ordering::Acquire); // Ensure all memory effects are visible
            if !l.alloc_ptr.is_null() {
                drop(unsafe { Arc::from_raw(l.alloc_ptr as *const MachVmAllocation) });
            } else if !l.mapping.is_null() && l.size > 0 {
                #[cfg(target_os = "macos")]
                {
                    let _ = unsafe {
                        mach_vm_deallocate(mach_task_self(), l.mapping as u64, l.size as u64)
                    };
                }
            }
            drop(unsafe { Box::from_raw(lease) });
        }
    }));
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

/// Searches the VFS string arena for nodes matching the given query string.
///
/// Returns 0 on success with matching indices written to `out_indices` and total matches to `out_count`.
/// Returns a negative error code on invalid parameters or corrupt snapshot data.
///
/// # Safety
/// Caller must ensure `lease` is valid and `out_indices` can hold up to `max_results` elements.
#[no_mangle]
pub unsafe extern "C" fn ttzip_vfs_search(
    lease: *mut ttvfs_lease_s,
    query: *const c_char,
    max_results: u32,
    out_indices: *mut u32,
    out_count: *mut u32,
) -> i32 {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if lease.is_null() || query.is_null() || out_indices.is_null() || out_count.is_null() {
            return -1; // TT_ERROR_INVALID_ARGUMENT
        }

        if max_results == 0 {
            *out_count = 0;
            return 0;
        }

        let q_str = match CStr::from_ptr(query).to_str() {
            Ok(s) => s,
            Err(_) => return -1,
        };
        let query_bytes = q_str.as_bytes();
        let is_ascii_query = query_bytes.is_ascii();
        let query_lower = (!is_ascii_query).then(|| q_str.to_lowercase());
        
        let l = &*lease;
        let ptr = l.mapping;
        if ptr.is_null() || l.size < std::mem::size_of::<TTWorkspaceSnapshotHeader>() {
            return -1;
        }
        let header = &*(ptr as *const TTWorkspaceSnapshotHeader);
        
        let slice = std::slice::from_raw_parts(ptr, header.total_mapping_size as usize);
        let nodes_sec = header.find_section(slice, TT_SECTION_VFS_NODES);
        let arena_sec = header.find_section(slice, TT_SECTION_STRING_ARENA);
        
        if nodes_sec.is_none() || arena_sec.is_none() {
            return -5; // TT_ERROR_CORRUPT_DATA
        }
        
        let n_sec = nodes_sec.unwrap();
        let a_sec = arena_sec.unwrap();
        
        let arena_slice = std::slice::from_raw_parts(
            ptr.add(a_sec.offset as usize) as *const u8,
            a_sec.size_bytes as usize,
        );

        let is_compact = (header.feature_flags & TT_FEATURE_FLAG_COMPACT_NODES != 0)
            || (header.feature_flags & ttzip_types::snapshot::TT_FEATURE_FLAG_CSR == 0);

        let mut count = 0;
        let limit = max_results as usize;

        if is_compact {
            let target_ptr = ptr.add(n_sec.offset as usize);
            if !(target_ptr as usize).is_multiple_of(core::mem::align_of::<TTWorkspaceCompactNode>()) {
                return -5;
            }
            let compact_nodes = std::slice::from_raw_parts(
                target_ptr as *const TTWorkspaceCompactNode,
                n_sec.entry_count as usize,
            );
            for (i, node) in compact_nodes.iter().enumerate() {
                if count >= limit {
                    break;
                }
                let start = node.name_offset as usize;
                let end = start + (node.name_len as usize);
                if let Some(path_bytes) = arena_slice.get(start..end) {
                    if contains_case_insensitive(path_bytes, query_bytes, query_lower.as_deref(), is_ascii_query) {
                        *out_indices.add(count) = i as u32;
                        count += 1;
                    }
                }
            }
        } else {
            let target_ptr = ptr.add(n_sec.offset as usize);
            if !(target_ptr as usize).is_multiple_of(core::mem::align_of::<TTWorkspaceEntryNode>()) {
                return -5;
            }
            let nodes = std::slice::from_raw_parts(
                target_ptr as *const TTWorkspaceEntryNode,
                n_sec.entry_count as usize,
            );
            for (i, node) in nodes.iter().enumerate() {
                if count >= limit {
                    break;
                }
                let start = node.name_offset as usize;
                let end = start + (node.name_len as usize);
                if let Some(path_bytes) = arena_slice.get(start..end) {
                    if contains_case_insensitive(path_bytes, query_bytes, query_lower.as_deref(), is_ascii_query) {
                        *out_indices.add(count) = i as u32;
                        count += 1;
                    }
                }
            }
        }
        
        *out_count = count as u32;
        0 // TT_SUCCESS
    }));
    
    result.unwrap_or(-4) // TT_ERROR_CANCELLED
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

/// Direct ARM NEON SIMD dot product kernel across 128 signed 8-bit integers.
/// Returns the dot product sum in the range [-2048383, 2048383].
#[no_mangle]
pub unsafe extern "C" fn ttzip_neon_dot_product_128(
    a: *const i8,
    b: *const i8,
) -> i32 {
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

/// Searches the Mach VM memory-mapped snapshot using SIMD NEON dot products
/// and character masks, writing top-K matched candidates into out_matches.
///
/// Returns 0 on success, negative error code on failure.
#[no_mangle]
pub unsafe extern "C" fn ttzip_vfs_hybrid_search(
    snapshot_handle: *const libc::c_void,
    query: *const ttzip_hybrid_query_t,
    out_matches: *mut ttzip_hybrid_match_t,
    max_matches: u32,
    out_count: *mut u32,
) -> i32 {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if snapshot_handle.is_null() || query.is_null() || out_matches.is_null() || out_count.is_null() {
            return -1; // TT_ERROR_INVALID_ARGUMENT
        }

        if max_matches == 0 {
            *out_count = 0;
            return 0;
        }

        let lease = &*(snapshot_handle as *const ttvfs_lease_s);
        let ptr = lease.mapping;
        if ptr.is_null() || lease.size < std::mem::size_of::<TTWorkspaceSnapshotHeader>() {
            return -1;
        }

        let header = &*(ptr as *const TTWorkspaceSnapshotHeader);
        let slice = std::slice::from_raw_parts(ptr, header.total_mapping_size as usize);

        let nodes_sec = header.find_section(slice, TT_SECTION_VFS_NODES);
        let arena_sec = header.find_section(slice, TT_SECTION_STRING_ARENA);
        if nodes_sec.is_none() || arena_sec.is_none() {
            return -5; // TT_ERROR_CORRUPT_DATA
        }

        let n_sec = nodes_sec.unwrap();
        let a_sec = arena_sec.unwrap();

        let arena_slice = std::slice::from_raw_parts(
            ptr.add(a_sec.offset as usize),
            a_sec.size_bytes as usize,
        );

        let q = &*query;
        let text_str = if q.text_query.is_null() {
            ""
        } else {
            match CStr::from_ptr(q.text_query).to_str() {
                Ok(s) => s,
                Err(_) => return -1,
            }
        };

        let vec_ref: Option<&[i8; 128]> = if q.query_vector.is_null() {
            None
        } else {
            let vec_slice = std::slice::from_raw_parts(q.query_vector, 128);
            vec_slice.try_into().ok()
        };

        let req_limit = if q.limit == 0 { max_matches } else { q.limit.min(max_matches) };
        if req_limit == 0 {
            *out_count = 0;
            return 0;
        }

        let lex_weight = if q.lexical_weight > 0.0 { q.lexical_weight } else { ttzip_types::semantic_search::DEFAULT_LEXICAL_WEIGHT };
        let sem_weight = if q.semantic_weight > 0.0 { q.semantic_weight } else { ttzip_types::semantic_search::DEFAULT_SEMANTIC_WEIGHT };

        let mut h_query = HybridSearchQuery::new(text_str)
            .with_limit(req_limit)
            .with_weights(lex_weight, sem_weight)
            .with_min_similarity(q.min_similarity);
        if let Some(v) = vec_ref {
            h_query = h_query.with_vector(v);
        }

        let masks_sec = header.find_section(slice, ttzip_types::snapshot::TT_SECTION_TYPE_NAME_MASKS);
        let name_masks: &[u64] = if let Some(m_sec) = masks_sec {
            let m_ptr = ptr.add(m_sec.offset as usize);
            if !(m_ptr as usize).is_multiple_of(core::mem::align_of::<u64>()) {
                return -5;
            }
            std::slice::from_raw_parts(
                m_ptr as *const u64,
                m_sec.entry_count as usize,
            )
        } else {
            &[]
        };

        let embeddings = header.find_embeddings(slice);
        let vectors_slice = embeddings.map(|(_hdr, vecs)| vecs);

        let is_compact = (header.feature_flags & TT_FEATURE_FLAG_COMPACT_NODES != 0)
            || (header.feature_flags & ttzip_types::snapshot::TT_FEATURE_FLAG_CSR == 0);

        let candidates = if is_compact {
            let target_ptr = ptr.add(n_sec.offset as usize);
            if !(target_ptr as usize).is_multiple_of(core::mem::align_of::<TTWorkspaceCompactNode>()) {
                return -5;
            }
            let compact_nodes = std::slice::from_raw_parts(
                target_ptr as *const TTWorkspaceCompactNode,
                n_sec.entry_count as usize,
            );
            search_hybrid_compact_nodes_top_k(name_masks, compact_nodes, arena_slice, vectors_slice, &h_query)
        } else {
            let target_ptr = ptr.add(n_sec.offset as usize);
            if !(target_ptr as usize).is_multiple_of(core::mem::align_of::<TTWorkspaceEntryNode>()) {
                return -5;
            }
            let nodes = std::slice::from_raw_parts(
                target_ptr as *const TTWorkspaceEntryNode,
                n_sec.entry_count as usize,
            );
            search_hybrid_nodes_top_k(name_masks, nodes, arena_slice, vectors_slice, &h_query)
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
        0 // TT_SUCCESS
    }));

    result.unwrap_or(-4) // TT_ERROR_CANCELLED
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ttzip_vfs_release_snapshot_null_safe() {
        unsafe {
            ttzip_vfs_release_snapshot(std::ptr::null_mut());
        }
    }

    #[test]
    fn test_ttzip_vfs_release_snapshot_lifecycle() {
        let lease = Box::into_raw(Box::new(ttvfs_lease_s {
            mapping: std::ptr::null_mut(),
            size: 0,
            ref_count: AtomicU64::new(2),
            alloc_ptr: std::ptr::null(),
        }));
        unsafe {
            ttzip_vfs_release_snapshot(lease);
            assert_eq!((*lease).ref_count.load(Ordering::Acquire), 1);
            ttzip_vfs_release_snapshot(lease);
        }
    }
}
