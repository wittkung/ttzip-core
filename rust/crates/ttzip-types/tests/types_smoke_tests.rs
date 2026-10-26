// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use ttzip_types::*;

#[test]
fn test_types_smoke() {
    assert_eq!(TTZipArchiveFormat::Zip.as_str(), "zip");
    assert_eq!(TTZipArchiveFormat::SevenZip.as_str(), "7z");
    assert_eq!(TTZipStatus::Ok.to_i32(), 0);
    assert!(TTZipStatus::Ok.is_ok());
    assert!(TTZipStatus::ErrInvalidParam.is_err());
    assert_eq!(TTZIP_ABI_VERSION, 3);

    let buf = TTZipBufferRef::from_slice(b"hello");
    assert_eq!(unsafe { buf.as_slice() }, b"hello");

    let mut vec = vec![1, 2, 3];
    let mut buf_mut = TTZipBufferMut::from_vec(&mut vec);
    assert_eq!(unsafe { buf_mut.as_slice() }, &[1, 2, 3]);
    unsafe { buf_mut.as_mut_slice()[0] = 42; }
    assert_eq!(vec[0], 42);

    let bytes = bytes::Bytes::from_static(b"world");
    let buf_bytes = TTZipBufferRef::from_bytes(&bytes);
    assert_eq!(unsafe { buf_bytes.as_slice() }, b"world");

    // Tier-1 capability and constructor assertions
    let fmt_7z = ArchiveFormat::SevenZip;
    assert!(fmt_7z.is_solid());
    assert!(fmt_7z.is_encrypted());
    assert!(!fmt_7z.supports_streaming());

    let fmt_tar = ArchiveFormat::Tar;
    assert!(fmt_tar.is_solid());
    assert!(!fmt_tar.is_encrypted());
    assert!(fmt_tar.supports_streaming());

    let fmt_zip = ArchiveFormat::Zip;
    assert!(!fmt_zip.is_solid());
    assert!(fmt_zip.is_encrypted());
    assert!(!fmt_zip.supports_streaming());

    let detail = TTZipEntryDetail::new("path/to/file.txt", 1024);
    assert_eq!(detail.path, "path/to/file.txt");
    assert_eq!(detail.uncompressed_size, 1024);
    assert!(!detail.is_directory());
    assert!(!detail.is_encrypted());
    assert!(!detail.is_solid());
    assert!(detail.supports_streaming());

    let dir_detail = TTZipEntryDetail::new("path/to/dir/", 0);
    assert!(dir_detail.is_directory());
}

#[test]
fn test_vfs_search_simd_neon() {
    let mut arena = Vec::new();
    let names = [
        "Focus.2015.2160p.mkv",
        "README.md",
        "Cargo.toml",
        "document_presentation.pdf",
        "avatar_profile_large_image.png",
        "test_suite_runner.rs",
    ];

    let mut nodes = Vec::new();
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
    }

    // 1. In-register prefix match (case-insensitive: "focus" matches "Focus.2015...")
    let r1 = vfs_search_simd(&nodes, &arena, "focus", 10);
    assert_eq!(r1, vec![0]);

    // 2. Short prefix match (case-insensitive: "read" matches "README.md")
    let r2 = vfs_search_simd(&nodes, &arena, "read", 10);
    assert_eq!(r2, vec![1]);

    // 3. Substring match ("profile" matches "avatar_profile_large_image.png")
    let r3 = vfs_search_simd(&nodes, &arena, "profile", 10);
    assert_eq!(r3, vec![4]);

    // 4. Substring across chunk ("presentation" matches "document_presentation.pdf")
    let r4 = vfs_search_simd(&nodes, &arena, "presentation", 10);
    assert_eq!(r4, vec![3]);

    // 5. Non-existent query
    let r5 = vfs_search_simd(&nodes, &arena, "non_existent_file", 10);
    assert!(r5.is_empty());
}

#[test]
fn test_permutation_vector_and_sort_cache() {
    let mut arena = Vec::new();
    let names = ["zebra.txt", "docs", "apple.zip", "banana.png", "media"];
    let mut nodes = Vec::new();

    // 0: zebra.txt (file, size 500)
    // 1: docs (dir, size 0)
    // 2: apple.zip (file, size 1000)
    // 3: banana.png (file, size 200)
    // 4: media (dir, size 0)
    for (i, name) in names.iter().enumerate() {
        let offset = arena.len() as u32;
        let len = name.len() as u16;
        arena.extend_from_slice(name.as_bytes());

        let is_dir = *name == "docs" || *name == "media";
        let size = match *name {
            "zebra.txt" => 500,
            "apple.zip" => 1000,
            "banana.png" => 200,
            _ => 0,
        };

        nodes.push(TTWorkspaceEntryNode {
            name_offset: offset,
            name_len: len,
            sort_prefix: make_sort_prefix(name.as_bytes()),
            file_type: if is_dir { TTWORKSPACE_S_IFDIR } else { 0 },
            uncompressed_size: size,
            stable_id: i as u64,
            ..TTWorkspaceEntryNode::default()
        });
    }

    // 1. Name Ascending returns None (identity slice)
    let p_default = compute_permutation_vector(&nodes, &arena, SortKey::Name, SortDirection::Ascending);
    assert!(p_default.is_none());

    // 2. Size Descending: Dirs on top (docs, media), then largest file (apple.zip 1000, zebra.txt 500, banana.png 200)
    let p_size_desc = compute_permutation_vector(&nodes, &arena, SortKey::Size, SortDirection::Descending).unwrap();
    // Dirs: index 1 ("docs"), index 4 ("media")
    // Files: index 2 ("apple.zip", 1000), index 0 ("zebra.txt", 500), index 3 ("banana.png", 200)
    assert_eq!(p_size_desc[0], 1); // docs
    assert_eq!(p_size_desc[1], 4); // media
    assert_eq!(p_size_desc[2], 2); // apple.zip (1000)
    assert_eq!(p_size_desc[3], 0); // zebra.txt (500)
    assert_eq!(p_size_desc[4], 3); // banana.png (200)

    // 3. Extension Ascending: Dirs on top, files sorted by ext (.png -> index 3, .txt -> index 0, .zip -> index 2)
    let p_ext_asc = compute_permutation_vector(&nodes, &arena, SortKey::Extension, SortDirection::Ascending).unwrap();
    assert_eq!(p_ext_asc[0], 1); // docs
    assert_eq!(p_ext_asc[1], 4); // media
    assert_eq!(p_ext_asc[2], 3); // banana.png
    assert_eq!(p_ext_asc[3], 0); // zebra.txt
    assert_eq!(p_ext_asc[4], 2); // apple.zip

    // 4. DirectorySortCache caching roundtrip
    let mut cache = DirectorySortCache::new();
    let c1 = cache.get_or_compute(0, &nodes, &arena, SortKey::Size, SortDirection::Descending).unwrap();
    assert_eq!(*c1, p_size_desc);
    let c2 = cache.get_or_compute(0, &nodes, &arena, SortKey::Size, SortDirection::Descending).unwrap();
    assert!(std::sync::Arc::ptr_eq(&c1, &c2), "Second fetch must return identical cached Arc");
}

