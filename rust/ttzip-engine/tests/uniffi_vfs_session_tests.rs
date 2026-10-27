// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Integration tests for UniFFI VfsSessionProtocol, windowed paging, and NFC/traversal guards.

use ttzip_engine::fs::vfs::VfsEntry;
use ttzip_engine::uniffi_api::types::UniFFIEntryMetadata;
use ttzip_engine::uniffi_api::vfs::{
    UniFFIVfsTree, VfsSession, VfsSessionError,
};

fn create_sample_entries() -> Vec<VfsEntry> {
    vec![
        VfsEntry {
            path: "docs/readme.txt".to_string(),
            uncompressed_size: 1024,
            compressed_size: 512,
            crc32: 0x11223344,
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
        VfsEntry {
            path: "docs/ArchitectureGuide.md".to_string(),
            uncompressed_size: 2048,
            compressed_size: 1024,
            crc32: 0x55667788,
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
        VfsEntry {
            path: "docs/changelog.txt".to_string(),
            uncompressed_size: 512,
            compressed_size: 256,
            crc32: 0x99AABBCC,
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
        VfsEntry {
            path: "src/main.rs".to_string(),
            uncompressed_size: 4096,
            compressed_size: 1500,
            crc32: 0xDDEEFF00,
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
        VfsEntry {
            path: "photos/café.txt".to_string(), // NFC accented e (\u{e9})
            uncompressed_size: 128,
            compressed_size: 64,
            crc32: 0x12345678,
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
    ]
}

#[test]
fn test_vfs_session_windowed_paging() {
    let entries = create_sample_entries();
    let session = VfsSession::from_entries(&entries, "archive");

    // Root (ID 0) has children: docs, src, photos (3 directories)
    let root_page_1 = session.get_children_paged(0, 0, 2).expect("root page 1");
    assert_eq!(root_page_1.total_count, 3);
    assert_eq!(root_page_1.offset, 0);
    assert_eq!(root_page_1.nodes.len(), 2);
    assert!(root_page_1.nodes[0].is_dir);

    // Second page of root
    let root_page_2 = session.get_children_paged(0, 2, 2).expect("root page 2");
    assert_eq!(root_page_2.total_count, 3);
    assert_eq!(root_page_2.offset, 2);
    assert_eq!(root_page_2.nodes.len(), 1);

    // Paging out of range
    let root_page_empty = session.get_children_paged(0, 10, 5).expect("out of range");
    assert_eq!(root_page_empty.total_count, 3);
    assert!(root_page_empty.nodes.is_empty());

    // Non-existent parent ID
    let err = session.get_children_paged(99999, 0, 10).unwrap_err();
    match err {
        VfsSessionError::NodeNotFound { node_id } => assert_eq!(node_id, 99999),
        other => panic!("Unexpected error: {:?}", other),
    }
}

#[test]
fn test_vfs_session_resolve_path_and_nfd_normalization() {
    let entries = create_sample_entries();
    let session = VfsSession::from_entries(&entries, "archive");

    // Resolve POSIX path
    let readme_id = session.resolve_path("docs/readme.txt").expect("resolve readme");
    assert!(readme_id > 0);

    // Verify detail
    let detail = session.get_node_detail(readme_id).expect("node detail");
    assert_eq!(detail.name, "readme.txt");
    assert_eq!(detail.full_path, "docs/readme.txt");
    assert!(!detail.is_dir);
    assert_eq!(detail.size, 1024);
    assert_eq!(detail.compressed_size, 512);
    assert_eq!(detail.crc32, 0x11223344);

    // Decomposed NFD unicode matching (macOS HFS+/APFS input defense)
    // "café" in NFD is 'c', 'a', 'f', 'e', '\u{301}'
    let nfd_path = "photos/cafe\u{301}.txt";
    let cafe_id = session.resolve_path(nfd_path).expect("resolve nfd accented path");
    let cafe_detail = session.get_node_detail(cafe_id).expect("cafe detail");
    assert_eq!(cafe_detail.name, "café.txt");

    // Path traversal defense
    let traversal_err = session.resolve_path("docs/../../etc/passwd").unwrap_err();
    match traversal_err {
        VfsSessionError::PathTraversalDetected { attempted_path } => {
            assert!(attempted_path.contains(".."));
        }
        other => panic!("Expected PathTraversalDetected, got: {:?}", other),
    }

    // Windows separator normalization
    let win_id = session.resolve_path("docs\\ArchitectureGuide.md").expect("windows separator");
    let win_detail = session.get_node_detail(win_id).expect("win detail");
    assert_eq!(win_detail.name, "ArchitectureGuide.md");
}

#[test]
fn test_vfs_session_search_top_k() {
    let entries = create_sample_entries();
    let session = VfsSession::from_entries(&entries, "archive");

    // Search for "guide"
    let matches = session.search_top_k("guide", 5).expect("search matches");
    assert!(!matches.is_empty());
    assert_eq!(matches[0].name, "ArchitectureGuide.md");

    // Limit clamp
    let single_match = session.search_top_k("txt", 1).expect("search limit 1");
    assert_eq!(single_match.len(), 1);

    // Empty query returns empty vector without panic
    let empty_matches = session.search_top_k("", 10).expect("empty search");
    assert!(empty_matches.is_empty());
}

#[test]
fn test_uniffi_vfs_tree_backward_compatibility() {
    let meta_entries = vec![
        UniFFIEntryMetadata {
            path: "sub/file.rs".to_string(),
            uncompressed_size: 100,
            compressed_size: 50,
            crc32: 0x1234,
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
            compression_method: "Deflate".to_string(),
            detected_encoding: None,
        },
    ];

    let tree = UniFFIVfsTree::build(meta_entries, "root".to_string());
    assert_eq!(tree.total_entries(), 1);

    let paged = tree.vfs_get_children_paged(0, 0, 10).expect("tree paged");
    assert_eq!(paged.total_count, 1);
    assert_eq!(paged.nodes[0].name, "sub");
    assert_eq!(paged.nodes[0].path, "sub");

    let file_id = tree.vfs_resolve_path("sub/file.rs".to_string()).expect("resolve");
    let detail = tree.vfs_get_node_detail(file_id).expect("detail");
    assert_eq!(detail.name, "file.rs");
    assert_eq!(detail.size, 100);

    let search = tree.vfs_search_top_k("file".to_string(), 5).expect("tree search");
    assert_eq!(search.len(), 1);
    assert_eq!(search[0].name, "file.rs");
    assert_eq!(search[0].path, "sub/file.rs");
}

#[test]
fn test_vfs_session_get_children_paged_sorted() {
    use ttzip_engine::uniffi_api::vfs_paging::{VfsSortKey, VfsSortDirection};

    let entries = vec![
        VfsEntry {
            path: "small.txt".to_string(),
            uncompressed_size: 100,
            compressed_size: 50,
            crc32: 0x1111,
            mtime_epoch_secs: 1000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
        VfsEntry {
            path: "large.zip".to_string(),
            uncompressed_size: 90000,
            compressed_size: 45000,
            crc32: 0x2222,
            mtime_epoch_secs: 2000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
        VfsEntry {
            path: "docs/nested.md".to_string(),
            uncompressed_size: 200,
            compressed_size: 100,
            crc32: 0x3333,
            mtime_epoch_secs: 3000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
    ];

    let session = VfsSession::from_entries(&entries, "test_root");
    
    // Sort by Size Descending: "docs" directory should be first (size 0), then large.zip (90000), then small.txt (100)
    let paged_size = session.get_children_paged_sorted(0, 0, 10, VfsSortKey::Size, VfsSortDirection::Descending).expect("sorted paged");
    assert_eq!(paged_size.total_count, 3);
    assert_eq!(paged_size.nodes[0].name, "docs");
    assert!(paged_size.nodes[0].is_dir);
    assert_eq!(paged_size.nodes[1].name, "large.zip");
    assert_eq!(paged_size.nodes[1].size, 90000);
    assert_eq!(paged_size.nodes[2].name, "small.txt");
    assert_eq!(paged_size.nodes[2].size, 100);

    // Sort by Extension Ascending: "docs" dir first, then small.txt (.txt), then large.zip (.zip)
    let paged_ext = session.get_children_paged_sorted(0, 0, 10, VfsSortKey::Extension, VfsSortDirection::Ascending).expect("sorted ext");
    assert_eq!(paged_ext.nodes[0].name, "docs");
    assert_eq!(paged_ext.nodes[1].name, "small.txt");
    assert_eq!(paged_ext.nodes[2].name, "large.zip");
}

#[test]
fn test_open_archive_session_split_volume_interception() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let p1 = dir.path().join("dataset.7z.001");
    let p3 = dir.path().join("dataset.7z.003");
    std::fs::File::create(&p1).unwrap().write_all(b"part1_content").unwrap();
    std::fs::File::create(&p3).unwrap().write_all(b"part3_content").unwrap();

    let status = ttzip_engine::uniffi_api::open_archive_session(p1.to_string_lossy().to_string(), None).expect("open status");
    match status {
        ttzip_engine::uniffi_api::VfsSessionStatus::MissingVolume { descriptor } => {
            assert_eq!(descriptor.format, "NumberedExtension");
            assert_eq!(descriptor.missing_indices, vec![2]);
            assert_eq!(descriptor.missing_filenames, vec!["dataset.7z.002"]);
            assert_eq!(descriptor.expected_total_volumes, Some(3));
        }
        _ => panic!("Expected MissingVolume status"),
    }
}

#[test]
fn test_vfs_session_hybrid_and_semantic_search() {
    use ttzip_types::snapshot::CompactVector128;

    let entries = create_sample_entries();
    let session = VfsSession::from_entries(&entries, "test_root");

    // 1. Verify initially no embeddings
    assert!(!session.has_vector_embeddings());

    // 2. Semantic search fails with EmbeddingsUnavailable
    let mut q_vec = vec![0i8; 128];
    q_vec[0] = 2;
    let err_unavail = session.search_semantic_top_k(q_vec.clone(), 5);
    assert!(matches!(err_unavail, Err(VfsSessionError::EmbeddingsUnavailable)));

    // 3. Dimension mismatch check
    let invalid_dim_vec = vec![0i8; 64];
    let err_dim = session.search_semantic_top_k(invalid_dim_vec, 5);
    assert!(matches!(err_dim, Err(VfsSessionError::InvalidVectorDimension { actual: 64 })));

    // 4. Set embeddings for all nodes in the arena
    let total = session.total_nodes() as usize;
    let mut vectors = vec![CompactVector128::zero(); total];
    if total > 2 {
        vectors[1][0] = 50;  // High dot product with q_vec
        vectors[2][0] = 100; // Even higher dot product
    }
    session.set_embeddings(vectors);
    assert!(session.has_vector_embeddings());

    // 5. Semantic-only search succeeds
    let sem_results = session.search_semantic_top_k(q_vec.clone(), 5).expect("semantic search");
    assert!(!sem_results.is_empty());
    assert_eq!(sem_results[0].node_id, 2);

    // 6. Hybrid search combining lexical and semantic vectors
    let hybrid_results = session.search_hybrid_top_k("readme".to_string(), Some(q_vec), 5, 0.6, 0.4).expect("hybrid search");
    assert!(!hybrid_results.is_empty());
}

#[test]
fn test_vfs_tree_strips_dot_segments_and_preserves_exec_file_type() {
    // 1. Verify UniFFIVfsTree::build strips ".", "./", and internal "/./" segments
    //    so root never contains a self-referential "." directory child.
    let meta_entries = vec![
        UniFFIEntryMetadata {
            path: ".".to_string(),
            uncompressed_size: 0,
            compressed_size: 0,
            crc32: 0,
            mtime_epoch_secs: 1700000000,
            mode: 0o755,
            is_directory: true,
            is_encrypted: false,
            compression_method: "Stored".to_string(),
            detected_encoding: None,
        },
        UniFFIEntryMetadata {
            path: "./src/./main.rs".to_string(),
            uncompressed_size: 256,
            compressed_size: 128,
            crc32: 0xABCD,
            mtime_epoch_secs: 1700000000,
            mode: 0o755,
            is_directory: false,
            is_encrypted: false,
            compression_method: "Deflate".to_string(),
            detected_encoding: None,
        },
    ];

    let tree = UniFFIVfsTree::build(meta_entries, "root".to_string());
    let root_paged = tree.get_children_paged(Some("".to_string()), 0, 10);
    assert_eq!(root_paged.total_count, 1);
    assert_eq!(root_paged.nodes[0].name, "src");
    assert_eq!(root_paged.nodes[0].path, "src");
    assert!(root_paged.nodes[0].is_directory);

    let src_paged = tree.get_children_paged(Some("src".to_string()), 0, 10);
    assert_eq!(src_paged.total_count, 1);
    assert_eq!(src_paged.nodes[0].name, "main.rs");
    assert_eq!(src_paged.nodes[0].path, "src/main.rs");
    assert!(!src_paged.nodes[0].is_directory);

    // 2. Verify load_archive_vfs_tree on a TAR archive with "./" root entry and
    //    0o755 executable file does not create a "." child or misclassify 0o755 files as directories.
    let dir = tempfile::tempdir().unwrap();
    let tar_path = dir.path().join("dot_prefix_exec.tar");
    {
        let file = std::fs::File::create(&tar_path).unwrap();
        let mut builder = tar::Builder::new(file);

        let mut root_hdr = tar::Header::new_gnu();
        root_hdr.set_entry_type(tar::EntryType::Directory);
        root_hdr.set_mode(0o755);
        root_hdr.set_size(0);
        builder.append_data(&mut root_hdr, "./", std::io::empty()).unwrap();

        let payload = b"#!/bin/sh\necho hello\n";
        let mut file_hdr = tar::Header::new_gnu();
        file_hdr.set_entry_type(tar::EntryType::Regular);
        file_hdr.set_mode(0o755);
        file_hdr.set_size(payload.len() as u64);
        builder.append_data(&mut file_hdr, "./bin/run.sh", &payload[..]).unwrap();
        builder.finish().unwrap();
    }

    let loaded_tree = ttzip_engine::uniffi_api::load_archive_vfs_tree(
        tar_path.to_string_lossy().to_string(),
        None,
    )
    .expect("load tar vfs tree");

    let top_level = loaded_tree.get_children_paged(Some("".to_string()), 0, 10);
    assert_eq!(top_level.total_count, 1);
    assert_eq!(top_level.nodes[0].name, "bin");
    assert!(top_level.nodes[0].is_directory);

    let bin_children = loaded_tree.get_children_paged(Some("bin".to_string()), 0, 10);
    assert_eq!(bin_children.total_count, 1);
    assert_eq!(bin_children.nodes[0].name, "run.sh");
    assert_eq!(bin_children.nodes[0].path, "bin/run.sh");
    assert!(
        !bin_children.nodes[0].is_directory,
        "Executable file (0o755) must not be marked as directory"
    );
}

fn append_raw_tar_entry<W: std::io::Write>(
    builder: &mut tar::Builder<W>,
    raw_path: &str,
    payload: &[u8],
    is_dir: bool,
    mode: u32,
) {
    let mut hdr = tar::Header::new_gnu();
    hdr.set_entry_type(if is_dir {
        tar::EntryType::Directory
    } else {
        tar::EntryType::Regular
    });
    hdr.set_mode(mode);
    hdr.set_size(if is_dir { 0 } else { payload.len() as u64 });
    hdr.set_mtime(1_700_000_000);
    let path_bytes = raw_path.as_bytes();
    let name_buf = &mut hdr.as_old_mut().name;
    name_buf.fill(0);
    name_buf[..path_bytes.len()].copy_from_slice(path_bytes);
    hdr.set_cksum();
    let reader: &[u8] = if is_dir { &[] } else { payload };
    builder.append(&hdr, reader).expect("append raw tar entry");
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn assert_arena_has_no_self_or_dot_cycles(arena: &ttzip_engine::fs::vfs::arena::VfsCompactArena) {
    assert_eq!(arena.node_name(0), arena.root_path);
    for node_id in 1..arena.len() {
        let nid = node_id as u32;
        let name = arena.node_name(nid);
        assert!(!name.is_empty(), "Non-root node {node_id} must not have empty name");
        assert_ne!(name, ".", "Node {node_id} must not be '.'");
        assert_ne!(name, "..", "Node {node_id} must not be '..'");
        let parent_id = arena.parent_ids[node_id];
        assert_ne!(
            parent_id,
            ttzip_engine::fs::vfs::arena::VFS_NULL_NODE,
            "Non-root node {node_id} must have valid parent"
        );
        assert_ne!(parent_id as usize, node_id, "Node {node_id} must not be its own parent");
        let full_path = arena.node_full_path_str(nid);
        assert!(!full_path.is_empty(), "Non-root node {node_id} must have non-empty path");
        if parent_id > 0 {
            let parent_path = arena.node_full_path_str(parent_id);
            assert_ne!(full_path, parent_path, "Child path must differ from parent path");
        }
    }
}

fn build_packed_arena_from_entries(
    entries: &[VfsEntry],
    root_name: &str,
) -> ttzip_engine::fs::vfs::arena::VfsCompactArena {
    use ttzip_engine::fs::vfs::arena::{
        VfsCompactArena, VFS_FLAG_IS_DIR, VFS_FLAG_IS_ENCRYPTED,
    };
    use ttzip_types::TTZipPackedEntryArray;

    let mut utf8_bytes = Vec::new();
    let mut path_offsets = Vec::with_capacity(entries.len());
    let mut path_lens = Vec::with_capacity(entries.len());
    let mut uncompressed_sizes = Vec::with_capacity(entries.len());
    let mut compressed_sizes = Vec::with_capacity(entries.len());
    let mut crc32s = Vec::with_capacity(entries.len());
    let mut mtimes = Vec::with_capacity(entries.len());
    let mut modes = Vec::with_capacity(entries.len());
    let mut flags = Vec::with_capacity(entries.len());

    for e in entries {
        path_offsets.push(utf8_bytes.len() as u32);
        path_lens.push(e.path.len() as u32);
        utf8_bytes.extend_from_slice(e.path.as_bytes());
        uncompressed_sizes.push(e.uncompressed_size);
        compressed_sizes.push(e.compressed_size);
        crc32s.push(e.crc32);
        mtimes.push(e.mtime_epoch_secs);
        modes.push(e.mode);
        let mut f = 0u8;
        if e.is_directory {
            f |= VFS_FLAG_IS_DIR;
        }
        if e.is_encrypted {
            f |= VFS_FLAG_IS_ENCRYPTED;
        }
        flags.push(f);
    }

    let packed = TTZipPackedEntryArray {
        struct_size: std::mem::size_of::<TTZipPackedEntryArray>() as u32,
        abi_version: ttzip_types::TTZIP_ABI_VERSION_3,
        count: entries.len(),
        utf8_bytes: utf8_bytes.as_mut_ptr(),
        total_bytes_len: utf8_bytes.len(),
        path_offsets: path_offsets.as_mut_ptr(),
        path_lens: path_lens.as_mut_ptr(),
        uncompressed_sizes: uncompressed_sizes.as_mut_ptr(),
        compressed_sizes: compressed_sizes.as_mut_ptr(),
        crc32s: crc32s.as_mut_ptr(),
        mtimes: mtimes.as_mut_ptr(),
        modes: modes.as_mut_ptr(),
        flags: flags.as_mut_ptr(),
    };
    VfsCompactArena::build_from_packed(&packed, root_name)
}

fn build_normalization_fixture_entries() -> Vec<(String, &'static [u8], bool)> {
    vec![
        (".".to_string(), b"", true),
        ("./".to_string(), b"", true),
        ("././".to_string(), b"", true),
        ("a/.".to_string(), b"", true),
        ("a/./b/alpha.txt".to_string(), b"alpha_payload_123", false),
        ("a//b/beta.txt".to_string(), b"beta_payload_4567", false),
        ("win\\sub\\gamma.txt".to_string(), b"gamma_payload_89", false),
        ("cafe\u{301}/re\u{301}sume\u{301}.txt".to_string(), b"unicode_nfd_payload", false),
        ("../escape_root.txt".to_string(), b"blocked_1", false),
        ("a/../../escape_nested.txt".to_string(), b"blocked_2", false),
    ]
}

fn verify_normalized_arena_queries(arena: &ttzip_engine::fs::vfs::arena::VfsCompactArena) {
    use ttzip_engine::fs::vfs::arena::VFS_FLAG_IS_DIR;
    assert_arena_has_no_self_or_dot_cycles(arena);
    let root_children = arena.get_children(0);
    let root_names: Vec<&str> = root_children.iter().map(|&id| arena.node_name(id)).collect();
    assert_eq!(root_names, vec!["a", "caf\u{e9}", "win"]);

    for query in ["a", "a/", "./a", "a/."] {
        let a_id = arena.resolve_path(query).expect("resolve a");
        assert_eq!(arena.node_name(a_id), "a");
        assert_ne!(arena.node_attributes[a_id as usize] & VFS_FLAG_IS_DIR, 0);
    }
    for query in ["a/b/alpha.txt", "./a/./b/alpha.txt", "a//b/alpha.txt", "a\\b\\alpha.txt"] {
        let id = arena.resolve_path(query).expect("resolve alpha");
        assert_eq!(arena.node_full_path_str(id), "a/b/alpha.txt");
        assert_eq!(arena.uncompressed_sizes[id as usize], 17);
    }
    for query in ["caf\u{e9}/r\u{e9}sum\u{e9}.txt", "cafe\u{301}/re\u{301}sume\u{301}.txt"] {
        let id = arena.resolve_path(query).expect("resolve NFC/NFD");
        assert_eq!(arena.node_full_path_str(id), "caf\u{e9}/r\u{e9}sum\u{e9}.txt");
        assert_eq!(arena.uncompressed_sizes[id as usize], 19);
    }
    assert!(arena.resolve_path("../escape_root.txt").is_none());
    assert!(arena.resolve_path("a/../../escape_nested.txt").is_none());
}

#[test]
fn test_four_vfs_layers_path_normalization_and_no_self_cycle() {
    use ttzip_engine::fs::vfs::arena::VfsCompactArena;
    let raw_items = build_normalization_fixture_entries();
    let vfs_entries: Vec<VfsEntry> = raw_items
        .iter()
        .map(|(path, data, is_dir)| VfsEntry {
            path: path.clone(),
            uncompressed_size: if *is_dir { 0 } else { data.len() as u64 },
            compressed_size: if *is_dir { 0 } else { data.len() as u64 },
            crc32: crc32fast::hash(data),
            mtime_epoch_secs: 1_700_000_000,
            mode: if *is_dir { 0o755 } else { 0o644 },
            is_directory: *is_dir,
            is_encrypted: false,
        })
        .collect();

    // Layer 1: VfsCompactArena::build_from_packed
    let packed_arena = build_packed_arena_from_entries(&vfs_entries, "packed_root");
    verify_normalized_arena_queries(&packed_arena);

    // Layer 2: VfsCompactArena::build_from_entries
    let entries_arena = VfsCompactArena::build_from_entries(&vfs_entries, "entries_root");
    verify_normalized_arena_queries(&entries_arena);

    // Layer 3: UniFFIVfsTree::build
    let uniffi_meta: Vec<UniFFIEntryMetadata> = vfs_entries
        .iter()
        .map(|e| UniFFIEntryMetadata {
            path: e.path.clone(),
            uncompressed_size: e.uncompressed_size,
            compressed_size: e.compressed_size,
            crc32: e.crc32,
            mtime_epoch_secs: e.mtime_epoch_secs,
            mode: e.mode,
            is_directory: e.is_directory,
            is_encrypted: e.is_encrypted,
            compression_method: "Stored".to_string(),
            detected_encoding: None,
        })
        .collect();
    let uniffi_tree = UniFFIVfsTree::build(uniffi_meta, "uniffi_root".to_string());
    let root_paged = uniffi_tree.get_children_paged(Some("".to_string()), 0, 20);
    let top_names: Vec<&str> = root_paged.nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(top_names, vec!["a", "caf\u{e9}", "win"]);
    assert!(uniffi_tree.vfs_resolve_path("../escape_root.txt".to_string()).is_err());

    // Layer 4: Real archive loader (load_archive_vfs_tree + open_vfs_session)
    let dir = tempfile::tempdir().unwrap();
    let tar_path = dir.path().join("four_layers_norm.tar");
    {
        let mut builder = tar::Builder::new(std::fs::File::create(&tar_path).unwrap());
        for (path, payload, is_dir) in &raw_items {
            append_raw_tar_entry(&mut builder, path, payload, *is_dir, if *is_dir { 0o755 } else { 0o644 });
        }
        builder.finish().unwrap();
    }
    let loaded_session = ttzip_engine::uniffi_api::open_vfs_session(
        tar_path.to_string_lossy().to_string(),
        None,
    )
    .expect("open_vfs_session");
    let loaded_root = loaded_session.get_children_paged(0, 0, 20).expect("loaded root");
    let loaded_names: Vec<&str> = loaded_root.nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(loaded_names, vec!["a", "caf\u{e9}", "win"]);
}

#[test]
fn test_collision_contract_duplicate_normalized_and_file_dir_same_path() {
    use ttzip_engine::fs::vfs::arena::{VfsCompactArena, VFS_FLAG_IS_DIR};
    let collision_entries = vec![
        VfsEntry {
            path: "./dup.txt".to_string(),
            uncompressed_size: 11,
            compressed_size: 11,
            crc32: 0x1111,
            mtime_epoch_secs: 1_700_000_000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
        VfsEntry {
            path: "dup.txt".to_string(),
            uncompressed_size: 12,
            compressed_size: 12,
            crc32: 0x2222,
            mtime_epoch_secs: 1_700_000_001,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
        VfsEntry {
            path: "app/config.json".to_string(),
            uncompressed_size: 15,
            compressed_size: 15,
            crc32: 0x3333,
            mtime_epoch_secs: 1_700_000_002,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
        VfsEntry {
            path: "app".to_string(),
            uncompressed_size: 23,
            compressed_size: 23,
            crc32: 0x4444,
            mtime_epoch_secs: 1_700_000_003,
            mode: 0o755,
            is_directory: false,
            is_encrypted: false,
        },
    ];

    let packed_arena = build_packed_arena_from_entries(&collision_entries, "packed_collision");
    let entries_arena = VfsCompactArena::build_from_entries(&collision_entries, "entries_collision");

    for arena in [&packed_arena, &entries_arena] {
        let root_children = arena.get_children(0);
        // Must contain: 1 directory "app", 1 file "app", and 2 files "dup.txt" (4 children total, none dropped!)
        assert_eq!(root_children.len(), 4, "No colliding file or directory may be dropped");
        assert_ne!(arena.node_attributes[root_children[0] as usize] & VFS_FLAG_IS_DIR, 0);
        assert_eq!(arena.node_name(root_children[0]), "app");
        assert_eq!(arena.node_attributes[root_children[1] as usize] & VFS_FLAG_IS_DIR, 0);
        assert_eq!(arena.node_name(root_children[1]), "app");
        assert_eq!(arena.uncompressed_sizes[root_children[1] as usize], 23);
        assert_eq!(arena.node_name(root_children[2]), "dup.txt");
        assert_eq!(arena.uncompressed_sizes[root_children[2] as usize], 11);
        assert_eq!(arena.node_name(root_children[3]), "dup.txt");
        assert_eq!(arena.uncompressed_sizes[root_children[3] as usize], 12);

        // Trailing slash resolves directory "app/"; bare "app" prefers directory when descending
        let dir_id = arena.resolve_path("app/").expect("resolve app/") as usize;
        assert_ne!(arena.node_attributes[dir_id] & VFS_FLAG_IS_DIR, 0);
        let cfg_id = arena.resolve_path("app/config.json").expect("resolve app/config.json") as usize;
        assert_eq!(arena.uncompressed_sizes[cfg_id], 15);
    }
}

#[test]
fn test_normalized_vfs_entry_content_sha256_across_open_preview_and_extract() {
    let dir = tempfile::tempdir().unwrap();
    let tar_path = dir.path().join("sha256_identity_verify.tar");
    let p_dup1 = b"first_dup_v1";
    let p_dup2 = b"second_dup_v2_longer";
    let p_cfg = b"{\"port\":8080}\n";
    let p_app_bin = b"#!/bin/sh\necho app_bin\n";
    let p_dot_norm = b"normalized_dot_segment_payload";
    {
        let mut builder = tar::Builder::new(std::fs::File::create(&tar_path).unwrap());
        append_raw_tar_entry(&mut builder, "./dup.txt", p_dup1, false, 0o644);
        append_raw_tar_entry(&mut builder, "dup.txt", p_dup2, false, 0o644);
        append_raw_tar_entry(&mut builder, "app/config.json", p_cfg, false, 0o644);
        append_raw_tar_entry(&mut builder, "app", p_app_bin, false, 0o755);
        append_raw_tar_entry(&mut builder, "./a/./b/note.txt", p_dot_norm, false, 0o644);
        builder.finish().unwrap();
    }
    let tar_str = tar_path.to_string_lossy().to_string();

    // 1. Open VFS tree and verify all 4 root children + nested children are present
    let tree = ttzip_engine::uniffi_api::load_archive_vfs_tree(tar_str.clone(), None).expect("vfs");
    let root_page = tree.get_children_paged(Some("".to_string()), 0, 20);
    assert_eq!(root_page.total_count, 5); // dirs: a, app; files: app, dup.txt, dup.txt

    // 2. Verify exact content SHA-256 by raw entry index (0..5) via extract_single_entry_stream
    let expected_digests = [
        (0u64, sha256_hex(p_dup1)),
        (1u64, sha256_hex(p_dup2)),
        (2u64, sha256_hex(p_cfg)),
        (3u64, sha256_hex(p_app_bin)),
        (4u64, sha256_hex(p_dot_norm)),
    ];
    for (entry_idx, expected_sha) in expected_digests {
        let got_bytes = ttzip_engine::uniffi_api::extract_single_entry_stream(
            tar_str.clone(),
            entry_idx,
            None,
        )
        .expect("stream extract by index");
        assert_eq!(sha256_hex(&got_bytes), expected_sha, "SHA-256 mismatch at entry_idx={entry_idx}");
    }

    // 3. Verify preview by path (`extract_single_entry_by_path`) on clean path
    let cfg_bytes = ttzip_engine::uniffi_api::extract_single_entry_by_path(
        tar_str,
        "app/config.json".to_string(),
        None,
    )
    .expect("preview app/config.json");
    assert_eq!(sha256_hex(&cfg_bytes), sha256_hex(p_cfg));
}

#[test]
fn test_vfs_5000_level_deep_tree_iterative_sizes_and_paging() {
    use ttzip_engine::fs::vfs::arena::VfsCompactArena;
    let depth = 5_000usize;
    let mut segments = Vec::with_capacity(depth + 1);
    for idx in 0..depth {
        segments.push(format!("d{idx}"));
    }
    segments.push("leaf.bin".to_string());
    let deep_path = segments.join("/");

    let entries = vec![VfsEntry {
        path: format!("./{deep_path}"),
        uncompressed_size: 65_536,
        compressed_size: 16_384,
        crc32: 0xDEAD_BEEF,
        mtime_epoch_secs: 1_700_000_000,
        mode: 0o644,
        is_directory: false,
        is_encrypted: false,
    }];

    let arena = VfsCompactArena::build_from_entries(&entries, "deep5000");
    assert_eq!(arena.len(), depth + 2); // root + 5000 dirs + 1 leaf
    assert_eq!(arena.uncompressed_sizes[0], 65_536);

    let leaf_id = arena.resolve_path(&deep_path).expect("resolve 5000-level leaf");
    assert_eq!(arena.node_name(leaf_id), "leaf.bin");
    assert_eq!(arena.uncompressed_sizes[leaf_id as usize], 65_536);
}


