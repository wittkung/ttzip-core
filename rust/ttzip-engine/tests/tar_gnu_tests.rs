// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unit and integration tests for USTAR path split/join and GNU @LongLink extension stream.

use ttzip_engine::archive::tar::gnu::*;
use ttzip_engine::archive::tar::header::*;
use ttzip_engine::archive::tar::reader::*;

#[test]
fn test_split_ustar_path_short_and_exact_boundaries() {
    // Short path fits entirely in name
    let short_path = "simple/file.txt";
    let (prefix, name) = split_ustar_path(short_path).expect("short path must fit");
    assert_eq!(prefix, "");
    assert_eq!(name, short_path);

    // Exact 100-byte path without slash fits in name
    let exact_100 = "a".repeat(100);
    let (p100, n100) = split_ustar_path(&exact_100).expect("100 byte path must fit in name");
    assert_eq!(p100, "");
    assert_eq!(n100, exact_100.as_str());

    // 101-byte path without slash cannot fit in USTAR
    let unseparable_101 = "a".repeat(101);
    assert!(split_ustar_path(&unseparable_101).is_none());
}

#[test]
fn test_split_ustar_path_valid_prefix_and_name_splits() {
    // Standard prefix (60 bytes) / name (50 bytes) -> total 111 bytes > 100
    let dir = "a".repeat(60);
    let file = "b".repeat(50);
    let full_path = format!("{}/{}", dir, file);

    let (prefix, name) = split_ustar_path(&full_path).expect("valid split expected for >100 byte path");
    assert_eq!(prefix, dir.as_str());
    assert_eq!(name, file.as_str());


    // Maximum theoretical USTAR split: 155-byte prefix and 100-byte name
    let max_prefix = "p".repeat(155);
    let max_name = "n".repeat(100);
    let max_path = format!("{}/{}", max_prefix, max_name);

    let (p_split, n_split) = split_ustar_path(&max_path).expect("max USTAR path must split");
    assert_eq!(p_split, max_prefix.as_str());
    assert_eq!(n_split, max_name.as_str());

    // Multiple slashes: should intelligently choose the rightmost separator that keeps name <= 100
    let deeply_nested = format!("{}/{}/{}/{}", "dir1".repeat(10), "dir2".repeat(10), "dir3".repeat(5), "leaf.txt");
    let (deep_p, deep_n) = split_ustar_path(&deeply_nested).expect("deeply nested path should split");
    assert!(!deep_p.is_empty() && deep_p.len() <= 155);
    assert!(!deep_n.is_empty() && deep_n.len() <= 100);
    assert_eq!(format!("{}/{}", deep_p, deep_n), deeply_nested);
}

#[test]
fn test_split_ustar_path_unrepresentable_paths() {
    // Total length > 256 bytes
    let too_long = format!("{}/{}", "a".repeat(160), "b".repeat(100));
    assert!(split_ustar_path(&too_long).is_none());

    // Filename component (after last slash) > 100 bytes
    let long_leaf = format!("{}/{}", "short_dir", "a".repeat(105));
    assert!(split_ustar_path(&long_leaf).is_none());

    // Directory prefix > 155 bytes before first separator
    let long_prefix_no_slash = format!("{}/{}", "a".repeat(160), "short.txt");
    assert!(split_ustar_path(&long_prefix_no_slash).is_none());
}

#[test]
fn test_join_ustar_path_operations() {
    // Normal join
    let joined = join_ustar_path(b"usr/local", b"bin/tool").expect("valid join");
    assert_eq!(joined, "usr/local/bin/tool");

    // Trailing slash in prefix
    let joined_slash = join_ustar_path(b"usr/local/", b"bin/tool").expect("valid join with slash");
    assert_eq!(joined_slash, "usr/local/bin/tool");

    // Empty prefix
    let name_only = join_ustar_path(b"", b"my_file.txt").expect("name only join");
    assert_eq!(name_only, "my_file.txt");

    // Empty name
    let prefix_only = join_ustar_path(b"my_dir/", b"").expect("prefix only join");
    assert_eq!(prefix_only, "my_dir/");

    // Both empty -> error
    let err = join_ustar_path(b"", b"").unwrap_err();
    assert_eq!(err, TarGnuError::EmptyPath);

    // Byte slices with trailing NUL bytes (as in raw TAR header blocks)
    let mut raw_prefix = [0u8; 155];
    raw_prefix[..7].copy_from_slice(b"var/log");
    let mut raw_name = [0u8; 100];
    raw_name[..7].copy_from_slice(b"sys.log");

    let nul_joined = join_ustar_path(&raw_prefix, &raw_name).expect("nul padded join");
    assert_eq!(nul_joined, "var/log/sys.log");
}

#[test]
fn test_gnu_long_name_manager_and_state_roundtrip() {
    let long_filename = "nested/deeply/embedded/structure/with/a/very/long/filename_that_exceeds_one_hundred_characters_easily_1234567890.txt";
    assert!(long_filename.len() > 100);

    // 1. Format GNU LongName header and payload
    let (header, payload) = GnuLongLinkManager::format_long_name_header(long_filename);
    assert_eq!(header.name, GNU_LONGLINK_NAME);
    assert_eq!(header.typeflag, TYPE_GNU_LONGNAME);
    assert_eq!(header.size, (long_filename.len() + 1) as u64);
    assert_eq!(payload.len() % TAR_BLOCK_SIZE, 0);

    // 2. Feed into state machine
    let mut state = GnuLongLinkState::new();
    let consumed = state.feed_header(&header, &payload).expect("must feed successfully");
    assert!(consumed);
    assert!(state.has_pending());

    // 3. Create dummy entity header with truncated name and apply state
    let mut entity_header = TarHeader {
        name: "truncated_name".to_string(),
        mode: 0o644,
        uid: 501,
        gid: 20,
        size: 2048,
        mtime: 1700000000,
        chksum: 0,
        typeflag: TYPE_REGULAR,
        linkname: String::new(),
        magic: *MAGIC_GNU,
        version: *b"  ",
        uname: "ttzip".to_string(),
        gname: "staff".to_string(),
        devmajor: 0,
        devminor: 0,
        prefix: String::new(),
    };

    state.apply_to_entry(&mut entity_header);
    assert_eq!(entity_header.name, long_filename);
    assert!(!state.has_pending());
}

#[test]
fn test_gnu_long_link_manager_and_state_roundtrip() {
    let long_symlink_target = "symlink/target/pointing/to/a/deeply/nested/location/which_is_extremely_long_and_exceeds_tar_standard_limits_9876543210.so";
    assert!(long_symlink_target.len() > 100);

    // 1. Format GNU LongLink header and payload
    let (link_hdr, link_payload) = GnuLongLinkManager::format_long_link_header(long_symlink_target);
    assert_eq!(link_hdr.name, GNU_LONGLINK_NAME);
    assert_eq!(link_hdr.typeflag, TYPE_GNU_LONGLINK);
    assert_eq!(link_hdr.size, (long_symlink_target.len() + 1) as u64);
    assert_eq!(link_payload.len() % TAR_BLOCK_SIZE, 0);

    // 2. Feed into state machine
    let mut state = GnuLongLinkState::new();
    let consumed = state.feed_header(&link_hdr, &link_payload).expect("feed link");
    assert!(consumed);

    // 3. Apply to entity header
    let mut entity_header = TarHeader {
        name: "short_link_name".to_string(),
        mode: 0o777,
        uid: 501,
        gid: 20,
        size: 0,
        mtime: 1700000000,
        chksum: 0,
        typeflag: TYPE_SYMLINK,
        linkname: "short_target".to_string(),
        magic: *MAGIC_GNU,
        version: *b"  ",
        uname: "ttzip".to_string(),
        gname: "staff".to_string(),
        devmajor: 0,
        devminor: 0,
        prefix: String::new(),
    };

    state.apply_to_entry(&mut entity_header);
    assert_eq!(entity_header.linkname, long_symlink_target);
    assert_eq!(entity_header.name, "short_link_name");
}

#[test]
fn test_gnu_combined_long_name_and_long_link() {
    let long_path = "a/".repeat(60) + "complex_symlink.bin";
    let long_target = "b/".repeat(60) + "target_destination.bin";

    let (name_hdr, name_payload) = GnuLongLinkManager::format_long_name_header(&long_path);
    let (link_hdr, link_payload) = GnuLongLinkManager::format_long_link_header(&long_target);

    let mut state = GnuLongLinkState::new();
    assert!(state.feed_header(&name_hdr, &name_payload).expect("feed name"));
    assert!(state.feed_header(&link_hdr, &link_payload).expect("feed link"));
    assert!(state.has_pending());

    let mut entity_header = TarHeader {
        name: "dummy".to_string(),
        mode: 0o777,
        uid: 0,
        gid: 0,
        size: 0,
        mtime: 0,
        chksum: 0,
        typeflag: TYPE_SYMLINK,
        linkname: "dummy_link".to_string(),
        magic: *MAGIC_GNU,
        version: *b"  ",
        uname: "".to_string(),
        gname: "".to_string(),
        devmajor: 0,
        devminor: 0,
        prefix: "".to_string(),
    };

    state.apply_to_entry(&mut entity_header);
    assert_eq!(entity_header.name, long_path);
    assert_eq!(entity_header.linkname, long_target);
    assert!(!state.has_pending());
}

#[test]
fn test_gnu_longlink_malformed_defense() {
    // 1. Missing NUL terminator defense
    let bad_payload = vec![b'A'; 512]; // No NUL byte
    let err = GnuLongLinkManager::parse_gnu_payload(&bad_payload, 512, 1024 * 1024).unwrap_err();
    assert_eq!(err, TarGnuError::MissingNulTerminator);

    // 2. Payload size budget exceeded defense (e.g. zip bomb / tar bomb)
    let state_limited = GnuLongLinkState::with_max_payload_size(1024);
    let huge_hdr = TarHeader {
        name: GNU_LONGLINK_NAME.to_string(),
        mode: 0,
        uid: 0,
        gid: 0,
        size: 2048, // Exceeds 1024 limit
        mtime: 0,
        chksum: 0,
        typeflag: TYPE_GNU_LONGNAME,
        linkname: "".to_string(),
        magic: *MAGIC_GNU,
        version: *b"  ",
        uname: "".to_string(),
        gname: "".to_string(),
        devmajor: 0,
        devminor: 0,
        prefix: "".to_string(),
    };
    let dummy_payload = vec![0u8; 2048];
    let mut state = state_limited;
    let err = state.feed_header(&huge_hdr, &dummy_payload).unwrap_err();
    assert_eq!(
        err,
        TarGnuError::PayloadTooLarge {
            size: 2048,
            max: 1024
        }
    );

    // 3. Short payload slice defense
    let short_payload = vec![0u8; 10];
    let err = GnuLongLinkManager::parse_gnu_payload(&short_payload, 100, 1024 * 1024).unwrap_err();
    assert!(matches!(err, TarGnuError::MalformedLongLink(_)));
}

#[test]
fn test_gnu_tar_full_archive_stream_scan_and_extract() {
    let mut tar_bytes = Vec::new();

    // Entry 1: Standard short file
    let short_hdr = TarHeader {
        name: "short.txt".to_string(),
        mode: 0o644,
        uid: 501,
        gid: 20,
        size: 5,
        mtime: 1700000000,
        chksum: 0,
        typeflag: TYPE_REGULAR,
        linkname: "".to_string(),
        magic: *MAGIC_GNU,
        version: *b"  ",
        uname: "user".to_string(),
        gname: "group".to_string(),
        devmajor: 0,
        devminor: 0,
        prefix: "".to_string(),
    };
    tar_bytes.extend_from_slice(&build_tar_header_block(&short_hdr));
    let mut content1 = b"hello".to_vec();
    content1.resize(512, 0);
    tar_bytes.extend_from_slice(&content1);

    // Entry 2: GNU @LongLink file (256-character path)
    let long_file_path = "directory/".repeat(20) + "extreme_long_filename_for_gnu_tar_validation.bin";
    let (long_name_hdr, long_name_payload) = GnuLongLinkManager::format_long_name_header(&long_file_path);
    tar_bytes.extend_from_slice(&build_tar_header_block(&long_name_hdr));
    tar_bytes.extend_from_slice(&long_name_payload);

    let mut entity2_hdr = short_hdr.clone();
    entity2_hdr.name = "truncated_entity2".to_string();
    entity2_hdr.size = 8;
    tar_bytes.extend_from_slice(&build_tar_header_block(&entity2_hdr));
    let mut content2 = b"12345678".to_vec();
    content2.resize(512, 0);
    tar_bytes.extend_from_slice(&content2);

    // Entry 3: GNU @LongLink symlink (long path + long target)
    let symlink_path = "symlinks/".repeat(15) + "link_to_somewhere";
    let symlink_target = "targets/".repeat(15) + "final_destination";

    let (s_name_hdr, s_name_payload) = GnuLongLinkManager::format_long_name_header(&symlink_path);
    let (s_link_hdr, s_link_payload) = GnuLongLinkManager::format_long_link_header(&symlink_target);

    tar_bytes.extend_from_slice(&build_tar_header_block(&s_name_hdr));
    tar_bytes.extend_from_slice(&s_name_payload);
    tar_bytes.extend_from_slice(&build_tar_header_block(&s_link_hdr));
    tar_bytes.extend_from_slice(&s_link_payload);

    let mut sym_entity_hdr = short_hdr.clone();
    sym_entity_hdr.name = "truncated_symlink".to_string();
    sym_entity_hdr.typeflag = TYPE_SYMLINK;
    sym_entity_hdr.size = 0;
    sym_entity_hdr.linkname = "truncated_target".to_string();
    tar_bytes.extend_from_slice(&build_tar_header_block(&sym_entity_hdr));

    // End-of-Archive: Two 512-byte zero blocks
    tar_bytes.extend_from_slice(&[0u8; 1024]);

    // Parse and verify with TarArchive
    let archive = TarArchive::open_slice(&tar_bytes).expect("open archive slice");
    assert_eq!(archive.len(), 3);

    assert_eq!(archive.entries()[0].path, "short.txt");
    assert_eq!(archive.entries()[0].size, 5);

    assert_eq!(archive.entries()[1].path, long_file_path);
    assert_eq!(archive.entries()[1].size, 8);

    assert_eq!(archive.entries()[2].path, symlink_path);
    assert_eq!(archive.entries()[2].link_target.as_deref(), Some(symlink_target.as_str()));
    assert!(archive.entries()[2].is_symlink);
}
