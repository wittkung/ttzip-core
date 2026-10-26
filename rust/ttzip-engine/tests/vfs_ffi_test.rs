// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Integration and C-ABI validation tests for VFS snapshot lease and hybrid search.

use std::sync::atomic::AtomicU64;
use ttzip_engine::ffi::vfs_ffi::*;
use ttzip_engine::types::TTWorkspaceSnapshotHeader;
use ttzip_types::builder::VfsSnapshotBuilder;
use ttzip_types::snapshot::CompactVector128;
use ttzip_types::unified_node::{UnifiedEntryNode, UnifiedVfsTable, NODE_FLAG_DIRECTORY, NO_NODE, ROOT_PARENT_ID};

#[test]
fn test_vfs_view_compact_node_geometry() {
    assert_eq!(core::mem::size_of::<TTWorkspaceVfsView>(), 48);
    assert_eq!(core::mem::align_of::<TTWorkspaceVfsView>(), 8);
    assert_eq!(core::mem::size_of::<TTWorkspaceCompactNode>(), 24);
    assert_eq!(core::mem::align_of::<TTWorkspaceCompactNode>(), 8);

    let compact = TTWorkspaceCompactNode {
        name_offset: 12,
        name_len: 8,
        flags: 1,
        size: 1024,
        mtime: 1700000000,
        dir_children_idx: 42,
    };

    let nodes = [compact];
    let view = TTWorkspaceVfsView::from_compact_nodes(
        std::ptr::null(),
        nodes.as_ptr(),
        1,
        std::ptr::null(),
        0,
        100,
    );

    assert_eq!(view.node_count, 1);
    assert_eq!(view.generation, 100);
    assert_eq!(view.compact_nodes(), nodes.as_ptr());
    assert_eq!(view.node_byte_size(), 24);
    assert!(view.is_compact_format());
    assert!(!view.is_legacy_format());

    unsafe {
        let slice = view.as_compact_slice().unwrap();
        assert_eq!(slice.len(), 1);
        assert_eq!(slice[0].name_offset, 12);
        assert_eq!(slice[0].name_len, 8);
        assert_eq!(slice[0].flags, 1);
        assert_eq!(slice[0].size, 1024);
        assert_eq!(slice[0].mtime, 1700000000);
        assert_eq!(slice[0].dir_children_idx, 42);
    }
}

#[test]
fn test_vfs_view_legacy_format_detection() {
    let header = TTWorkspaceSnapshotHeader {
        magic: ttzip_types::snapshot::TT_SNAPSHOT_MAGIC,
        abi_major: ttzip_types::snapshot::TT_ABI_VERSION_MAJOR,
        abi_minor: ttzip_types::snapshot::TT_ABI_VERSION_MINOR,
        header_size: 128,
        section_header_size: 32,
        section_count: 2,
        root_node_id: 0,
        total_mapping_size: 16384,
        generation: 1,
        snapshot_sequence: 1,
        section_headers_offset: 128,
        feature_flags: ttzip_types::snapshot::TT_FEATURE_FLAG_CSR, // Explicit legacy CSR
        reserved_flags: 0,
        snapshot_checksum: 0,
        _reserved: [0u8; 56],
    };

    let view = TTWorkspaceVfsView {
        header: &header as *const _,
        nodes: std::ptr::null(),
        node_count: 0,
        string_arena: std::ptr::null(),
        string_arena_len: 0,
        generation: 1,
    };

    assert!(!view.is_compact_format());
    assert!(view.is_legacy_format());
    assert_eq!(view.node_byte_size(), 64);
}

#[test]
fn test_open_compact_catalog_invalid_params() {
    let mut view = std::mem::MaybeUninit::<TTWorkspaceVfsView>::uninit();
    unsafe {
        let res = ttzip_vfs_open_compact_catalog(
            std::ptr::null(),
            std::ptr::null(),
            view.as_mut_ptr(),
        );
        assert!(res.is_null());
    }
}

#[test]
fn test_vfs_search_invalid_params() {
    let mut out_indices = [0u32; 8];
    let mut out_count = 0u32;
    let query = std::ffi::CString::new("test").unwrap();
    let mut lease = ttvfs_lease_s {
        mapping: std::ptr::null_mut(),
        size: 0,
        ref_count: AtomicU64::new(1),
        alloc_ptr: std::ptr::null(),
    };

    unsafe {
        let p_null: *mut ttvfs_lease_s = std::ptr::null_mut();
        assert_eq!(ttzip_vfs_search(p_null, query.as_ptr(), 8, out_indices.as_mut_ptr(), &mut out_count), -1);
        assert_eq!(ttzip_vfs_search(&mut lease, std::ptr::null(), 8, out_indices.as_mut_ptr(), &mut out_count), -1);
        assert_eq!(ttzip_vfs_search(&mut lease, query.as_ptr(), 8, out_indices.as_mut_ptr(), &mut out_count), -1);
        assert_eq!(ttzip_vfs_search(&mut lease, query.as_ptr(), 0, out_indices.as_mut_ptr(), &mut out_count), 0);
        assert_eq!(out_count, 0);
    }
}

#[test]
fn test_neon_dot_product_128_cabi_null_and_computation() {
    unsafe {
        // Null checks
        assert_eq!(ttzip_neon_dot_product_128(std::ptr::null(), std::ptr::null()), 0);

        let mut a = [0i8; 128];
        let mut b = [0i8; 128];
        assert_eq!(ttzip_neon_dot_product_128(a.as_ptr(), std::ptr::null()), 0);
        assert_eq!(ttzip_neon_dot_product_128(std::ptr::null(), b.as_ptr()), 0);

        // Arithmetic correctness
        a[0] = 50;
        b[0] = 2;
        a[127] = -10;
        b[127] = 3;
        // 50 * 2 + (-10) * 3 = 100 - 30 = 70
        let dot = ttzip_neon_dot_product_128(a.as_ptr(), b.as_ptr());
        assert_eq!(dot, 70);
    }
}

#[test]
fn test_vfs_hybrid_search_cabi_invalid_params() {
    let mut out_matches = [ttzip_hybrid_match_t {
        node_id: 0,
        lexical_score: 0,
        semantic_score: 0,
        fused_score: 0.0,
    }; 4];
    let mut out_count = 0u32;
    let query_text = std::ffi::CString::new("test").unwrap();
    let query = ttzip_hybrid_query_t {
        text_query: query_text.as_ptr(),
        query_vector: std::ptr::null(),
        limit: 4,
        lexical_weight: 0.6,
        semantic_weight: 0.4,
        min_similarity: 0,
    };

    let lease = ttvfs_lease_s {
        mapping: std::ptr::null_mut(),
        size: 0,
        ref_count: AtomicU64::new(1),
        alloc_ptr: std::ptr::null(),
    };

    unsafe {
        // Null handle
        assert_eq!(ttzip_vfs_hybrid_search(std::ptr::null(), &query, out_matches.as_mut_ptr(), 4, &mut out_count), -1);
        // Null query
        assert_eq!(ttzip_vfs_hybrid_search(&lease as *const _ as *const libc::c_void, std::ptr::null(), out_matches.as_mut_ptr(), 4, &mut out_count), -1);
        // Null out_matches
        assert_eq!(ttzip_vfs_hybrid_search(&lease as *const _ as *const libc::c_void, &query, std::ptr::null_mut(), 4, &mut out_count), -1);
        // Null out_count
        assert_eq!(ttzip_vfs_hybrid_search(&lease as *const _ as *const libc::c_void, &query, out_matches.as_mut_ptr(), 4, std::ptr::null_mut()), -1);

        // Zero limit
        assert_eq!(ttzip_vfs_hybrid_search(&lease as *const _ as *const libc::c_void, &query, out_matches.as_mut_ptr(), 0, &mut out_count), 0);
        assert_eq!(out_count, 0);
    }
}

#[test]
fn test_vfs_hybrid_search_cabi_end_to_end() {
    let mut table = UnifiedVfsTable::with_capacity(4, 128);
    let mut root = UnifiedEntryNode::empty();
    root.parent_id = ROOT_PARENT_ID;
    root.flags = NODE_FLAG_DIRECTORY;
    root.first_child_id = 1;
    table.root_id = table.add_entry(root, "root");

    let mut child1 = UnifiedEntryNode::empty();
    child1.parent_id = 0;
    child1.next_sibling_id = 2;
    table.add_entry(child1, "document.pdf");

    let mut child2 = UnifiedEntryNode::empty();
    child2.parent_id = 0;
    child2.next_sibling_id = 3;
    table.add_entry(child2, "photo.png");

    let mut child3 = UnifiedEntryNode::empty();
    child3.parent_id = 0;
    child3.next_sibling_id = NO_NODE;
    table.add_entry(child3, "notes.txt");

    // 5 nodes in total (0: synthetic root, 1: root, 2: document.pdf, 3: photo.png, 4: notes.txt)
    let v0 = CompactVector128::zero();
    let v1 = CompactVector128::zero();
    let mut v2 = CompactVector128::zero();
    let mut v3 = CompactVector128::zero();
    let mut v4 = CompactVector128::zero();

    v2[0] = 50;  // node 2: document.pdf
    v3[0] = 100; // node 3: photo.png
    v4[0] = 10;  // node 4: notes.txt

    let vectors = vec![v0, v1, v2, v3, v4];
    let alloc = VfsSnapshotBuilder::build_snapshot_with_embeddings(&table, Some(&vectors), 0xABCD_1234);
    let ptr = alloc.as_ptr();
    let header = unsafe { &*(ptr as *const TTWorkspaceSnapshotHeader) };

    let lease = Box::new(ttvfs_lease_s {
        mapping: ptr as *mut u8,
        size: header.total_mapping_size as usize,
        ref_count: AtomicU64::new(1),
        alloc_ptr: std::ptr::null(),
    });
    let lease_ptr = Box::into_raw(lease);

    let mut query_vec = [0i8; 128];
    query_vec[0] = 2;

    // 1. Semantic-only search via C-ABI
    let query_sem = ttzip_hybrid_query_t {
        text_query: std::ptr::null(),
        query_vector: query_vec.as_ptr(),
        limit: 3,
        lexical_weight: 0.6,
        semantic_weight: 0.4,
        min_similarity: 0,
    };

    let mut matches = [ttzip_hybrid_match_t {
        node_id: 0,
        lexical_score: 0,
        semantic_score: 0,
        fused_score: 0.0,
    }; 4];
    let mut match_count = 0u32;

    unsafe {
        let ret = ttzip_vfs_hybrid_search(
            lease_ptr as *const libc::c_void,
            &query_sem,
            matches.as_mut_ptr(),
            4,
            &mut match_count,
        );
        assert_eq!(ret, 0);
        assert_eq!(match_count, 3);
        // photo.png dot product = 100 * 2 = 200 (node 3)
        // document.pdf dot product = 50 * 2 = 100 (node 2)
        // notes.txt dot product = 10 * 2 = 20 (node 4)
        assert_eq!(matches[0].node_id, 3);
        assert_eq!(matches[0].semantic_score, 200);
        assert_eq!(matches[1].node_id, 2);
        assert_eq!(matches[1].semantic_score, 100);
        assert_eq!(matches[2].node_id, 4);
        assert_eq!(matches[2].semantic_score, 20);

        // 2. Lexical query via C-ABI
        let q_str = std::ffi::CString::new("document").unwrap();
        let query_lex = ttzip_hybrid_query_t {
            text_query: q_str.as_ptr(),
            query_vector: std::ptr::null(),
            limit: 3,
            lexical_weight: 0.6,
            semantic_weight: 0.4,
            min_similarity: 0,
        };

        let ret_lex = ttzip_vfs_hybrid_search(
            lease_ptr as *const libc::c_void,
            &query_lex,
            matches.as_mut_ptr(),
            4,
            &mut match_count,
        );
        assert_eq!(ret_lex, 0);
        assert_eq!(match_count, 1);
        assert_eq!(matches[0].node_id, 2);

        // 3. Clean up lease
        drop(Box::from_raw(lease_ptr));
    }
}
