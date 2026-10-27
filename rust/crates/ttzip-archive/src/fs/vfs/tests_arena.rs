// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Unit and integration verification tests for `VfsCompactArena`.

use super::*;
use std::time::Instant;

#[test]
fn test_fx_hasher_basic() {
    let mut hasher1 = FxHasher::default();
    hasher1.write(b"dir/file.txt");
    let hash1 = hasher1.finish();

    let mut hasher2 = FxHasher::default();
    hasher2.write(b"dir/file.bin");
    let hash2 = hasher2.finish();

    assert_ne!(hash1, hash2);
}

#[test]
fn test_arena_alloc_node_intern_slice() {
    let mut arena = VfsCompactArena::with_capacity(10);
    let id = arena.alloc_node(VfsNodeAllocParams {
        name: "file.txt",
        full_path: "documents/file.txt",
        uncompressed_size: 100,
        compressed_size: 50,
        crc32: 0x12345678,
        mtime: 1000,
        mode: 0o644,
        flags: 0,
        parent_id: VFS_NULL_NODE,
        entry_idx: None,
    });

    assert_eq!(id, 0);
    let (name_off, name_len) = arena.name_offset_and_len[0];
    let path_off = arena.full_path_offsets[0];
    let path_len = arena.full_path_lens[0];

    let name = std::str::from_utf8(
        &arena.names_pool[name_off as usize..name_off as usize + name_len as usize],
    )
    .unwrap();
    let path = std::str::from_utf8(
        &arena.names_pool[path_off as usize..path_off as usize + path_len as usize],
    )
    .unwrap();

    assert_eq!(name, "file.txt");
    assert_eq!(path, "documents/file.txt");
    assert_eq!(name_off, path_off + (path_len - name_len as u32));
}

#[test]
fn test_vfs_compact_arena_paged_slice_o1() {
    let mut entries = Vec::new();
    for i in 0..10 {
        entries.push(VfsEntry {
            path: format!("folder/file_{:02}.txt", i),
            uncompressed_size: 100 * (i as u64 + 1),
            compressed_size: 50 * (i as u64 + 1),
            crc32: i as u32,
            mtime_epoch_secs: 1000 + i as i64,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        });
    }

    let arena = VfsCompactArena::build_from_entries(&entries, "Root");
    let folder_id = arena.resolve_path("folder").expect("folder must exist");

    let page1 = arena.get_children_paged(folder_id, 0, 5);
    assert_eq!(page1.len(), 5);
    assert_eq!(arena.node_name(page1[0]), "file_00.txt");
    assert_eq!(arena.node_name(page1[4]), "file_04.txt");

    let page2 = arena.get_children_paged(folder_id, 5, 5);
    assert_eq!(page2.len(), 5);
    assert_eq!(arena.node_name(page2[0]), "file_05.txt");
    assert_eq!(arena.node_name(page2[4]), "file_09.txt");

    let (summaries, total) = arena.get_children_slice(folder_id, 0, 3);
    assert_eq!(total, 10);
    assert_eq!(summaries.len(), 3);
    assert_eq!(summaries[0].node_id, page1[0]);
}

#[test]
fn test_vfs_compact_arena_natural_sort_order() {
    let entries = vec![
        VfsEntry {
            path: "docs/ch10.txt".to_string(),
            uncompressed_size: 10,
            compressed_size: 5,
            crc32: 0,
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
        VfsEntry {
            path: "docs/ch2.txt".to_string(),
            uncompressed_size: 10,
            compressed_size: 5,
            crc32: 0,
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
        VfsEntry {
            path: "docs/ch1.txt".to_string(),
            uncompressed_size: 10,
            compressed_size: 5,
            crc32: 0,
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
        VfsEntry {
            path: "docs/sub_dir/readme.txt".to_string(),
            uncompressed_size: 10,
            compressed_size: 5,
            crc32: 0,
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
    ];

    let arena = VfsCompactArena::build_from_entries(&entries, "Root");
    let docs_id = arena.resolve_path("docs").expect("docs must exist");
    let children = arena.get_children(docs_id);

    assert_eq!(children.len(), 4);
    assert_eq!(arena.node_name(children[0]), "sub_dir");
    assert_eq!(arena.node_name(children[1]), "ch1.txt");
    assert_eq!(arena.node_name(children[2]), "ch2.txt");
    assert_eq!(arena.node_name(children[3]), "ch10.txt");
}

#[test]
fn test_vfs_compact_arena_100k_entries_perf_and_memory() {
    const COUNT: usize = 100_000;
    let mut entries = Vec::with_capacity(COUNT);
    for i in 0..COUNT {
        entries.push(VfsEntry {
            path: format!("root/dir_{}/sub_{}/file_{}.bin", i / 1000, (i / 10) % 100, i),
            uncompressed_size: 1024,
            compressed_size: 512,
            crc32: 0x12345678,
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        });
    }

    let start = Instant::now();
    let arena = VfsCompactArena::build_from_entries(&entries, "BenchRoot");
    let elapsed = start.elapsed();

    assert_eq!(arena.uncompressed_sizes[0], (COUNT as u64) * 1024);
    assert!(elapsed.as_secs_f64() < 1.0, "VFS build took too long: {:?}", elapsed);

    let query_start = Instant::now();
    let sample_id = arena.resolve_path("root/dir_50/sub_50/file_50500.bin");
    let query_elapsed = query_start.elapsed();

    assert!(sample_id.is_some());
    assert!(query_elapsed.as_micros() < 500, "Path lookup took too long: {:?}", query_elapsed);
}

#[test]
fn test_vfs_compact_arena_5000_deep_nesting_no_stack_overflow() {
    const DEPTH: usize = 5000;
    let mut segments = Vec::with_capacity(DEPTH);
    for i in 0..DEPTH {
        segments.push(format!("d{}", i));
    }
    segments.push("leaf.txt".to_string());
    let deep_path = segments.join("/");

    let entries = vec![VfsEntry {
        path: deep_path,
        uncompressed_size: 4096,
        compressed_size: 2048,
        crc32: 0xABCDEF01,
        mtime_epoch_secs: 1000,
        mode: 0o644,
        is_directory: false,
        is_encrypted: false,
    }];

    let arena = VfsCompactArena::build_from_entries(&entries, "Root");
    assert_eq!(arena.total_nodes, DEPTH + 2);
    assert_eq!(arena.uncompressed_sizes[0], 4096);
    assert_eq!(arena.compressed_sizes[0], 2048);
}

#[test]
fn test_vfs_tree_build_and_render() {
    let entries = vec![
        VfsEntry {
            path: "Project/src/main.rs".to_string(),
            uncompressed_size: 150,
            compressed_size: 100,
            crc32: 0x1234,
            mtime_epoch_secs: 1000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
        VfsEntry {
            path: "Project/README.md".to_string(),
            uncompressed_size: 200,
            compressed_size: 120,
            crc32: 0x5678,
            mtime_epoch_secs: 1000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
    ];

    let tree = VfsCompactArena::build_from_entries(&entries, "Project");
    let (files, dirs, uncomp) = tree.get_stats();
    assert_eq!(files, 2);
    assert_eq!(dirs, 2);
    assert_eq!(uncomp, 350);

    let rendered = tree.render_tree();
    assert!(rendered.contains("Project (<DIR>)"));
    assert!(rendered.contains("src (<DIR>)"));
    assert!(rendered.contains("main.rs (150 B)"));
    assert!(rendered.contains("README.md (200 B)"));
}

#[test]
fn test_vfs_fuzzy_search() {
    let entries = vec![
        VfsEntry {
            path: "docs/architecture.md".to_string(),
            uncompressed_size: 1000,
            compressed_size: 500,
            crc32: 0,
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
        VfsEntry {
            path: "src/archive/writer.rs".to_string(),
            uncompressed_size: 2000,
            compressed_size: 800,
            crc32: 0,
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
    ];

    let tree = VfsCompactArena::build_from_entries(&entries, "root");
    let results = tree.fuzzy_search("arch");
    assert!(!results.is_empty());
    assert_eq!(results[0].name, "architecture.md");

    let writer_results = tree.fuzzy_search("writer");
    assert!(!writer_results.is_empty());
    assert_eq!(writer_results[0].name, "writer.rs");
}

#[test]
fn test_vfs_nested_deep_tree_sizes() {
    let entries = vec![
        VfsEntry {
            path: "a/b/c/d/file1.bin".to_string(),
            uncompressed_size: 1024,
            compressed_size: 512,
            crc32: 0x1111,
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
        VfsEntry {
            path: "a/b/c/d/file2.bin".to_string(),
            uncompressed_size: 2048,
            compressed_size: 1024,
            crc32: 0x2222,
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
    ];

    let tree = VfsCompactArena::build_from_entries(&entries, "root");
    let (files, dirs, uncomp) = tree.get_stats();
    assert_eq!(files, 2);
    assert_eq!(dirs, 4); // a, b, c, d
    assert_eq!(uncomp, 3072);
}

fn sample_metadata() -> Vec<VfsEntry> {
    vec![
        VfsEntry {
            path: "src/main.rs".to_string(),
            uncompressed_size: 1024,
            compressed_size: 400,
            crc32: 0x12345678,
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
        VfsEntry {
            path: "src/vfs.rs".to_string(),
            uncompressed_size: 2048,
            compressed_size: 800,
            crc32: 0x87654321,
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
        VfsEntry {
            path: "src/ui/mod.rs".to_string(),
            uncompressed_size: 512,
            compressed_size: 200,
            crc32: 0xABCDEF01,
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
        VfsEntry {
            path: "assets/logo.png".to_string(),
            uncompressed_size: 65536,
            compressed_size: 60000,
            crc32: 0xCAFEBABE,
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: true,
        },
        VfsEntry {
            path: "README.md".to_string(),
            uncompressed_size: 256,
            compressed_size: 128,
            crc32: 0xDEADBEEF,
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        },
    ]
}

#[test]
fn test_toggle_expanded_and_flatten_visible() {
    let metas = sample_metadata();
    let mut tree = VfsCompactArena::build_from_entries(&metas, "test.zip");

    // Initially nothing expanded -> 3 top-level items (assets, src, README.md)
    assert_eq!(tree.flatten_visible().len(), 3);

    // Expand src
    let expanded = tree.toggle_expanded("src");
    assert_eq!(expanded, Some(true));

    // assets, src, src/ui, src/main.rs, src/vfs.rs, README.md
    assert_eq!(tree.flatten_visible().len(), 6);

    // Expand src/ui
    let expanded_ui = tree.toggle_expanded("src/ui");
    assert_eq!(expanded_ui, Some(true));

    // assets, src, src/ui, src/ui/mod.rs, src/main.rs, src/vfs.rs, README.md
    assert_eq!(tree.flatten_visible().len(), 7);

    // Collapse src
    tree.toggle_expanded("src");
    assert_eq!(tree.flatten_visible().len(), 3);
}

#[test]
fn test_toggle_selected_and_indices() {
    let metas = sample_metadata();
    let mut tree = VfsCompactArena::build_from_entries(&metas, "test.zip");

    // Select src directory (should select src/main.rs, src/vfs.rs, src/ui/mod.rs)
    tree.toggle_selected("src");
    let selected_indices = tree.get_selected_entry_indices();
    assert_eq!(selected_indices.len(), 3);
    assert!(selected_indices.contains(&0)); // main.rs
    assert!(selected_indices.contains(&1)); // vfs.rs
    assert!(selected_indices.contains(&2)); // ui/mod.rs

    let paths = tree.get_selected_paths();
    assert_eq!(paths.len(), 3);

    // Toggle README.md
    tree.toggle_selected("README.md");
    let paths2 = tree.get_selected_paths();
    assert_eq!(paths2.len(), 4);

    // Deselect src
    tree.toggle_selected("src");
    let paths3 = tree.get_selected_paths();
    assert_eq!(paths3.len(), 1);
    assert_eq!(paths3[0], "README.md");
}

#[test]
fn test_contract_compliance_json_structure() {
    let metas = sample_metadata();
    let tree = VfsCompactArena::build_from_entries(&metas, "archive.zip");
    let json_val = tree.to_contract_json();

    assert_eq!(json_val["rootPath"], "archive.zip");
    assert_eq!(json_val["totalEntriesCount"], 5);
    assert_eq!(json_val["totalUncompressedBytes"], 1024 + 2048 + 512 + 65536 + 256);

    let nodes = json_val["nodes"].as_array().expect("nodes must be an array");
    assert!(!nodes.is_empty());

    for node in nodes {
        assert!(node.get("name").is_some());
        assert!(node.get("relativePath").is_some());
        assert!(node.get("isDirectory").is_some());
        assert!(node.get("uncompressedSize").is_some());
        assert!(node.get("compressedSize").is_some());
        assert!(node.get("crc32").is_some());
        assert!(node.get("isEncrypted").is_some());
    }
}
