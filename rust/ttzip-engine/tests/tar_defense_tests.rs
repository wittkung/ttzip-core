// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Tar-Slip Sandbox Path Defense, Link Escape Interception, and Extraction Quota Guard Tests.
//!
//! Validates:
//! 1. `enclosed_tar_path` depth-aware path stack underflow defense (`../../../../etc/passwd`, `a/../../b`).
//! 2. Multi-hop symlink sandbox escape interception (`../../outside`, absolute targets, drive letters).
//! 3. Hardlink target sandbox containment and pre-existence verification.
//! 4. GHSA-3cv2-h65g-fgmm PAX size smuggling defense and stream synchronization.
//! 5. `TarExtractionQuotaGuard` size limit, entry count quota, and expansion ratio breaker.

use std::fs::{self, File};
use std::io::Write;
use tempfile::tempdir;

use ttzip_engine::archive::tar::header::{
    compute_tar_checksum, format_numeric, TAR_BLOCK_SIZE, TYPE_GNU_LONGNAME, TYPE_PAX_EXT_HEADER,
    TYPE_REGULAR,
};
use ttzip_engine::archive::tar::pax::build_pax_payload;
use ttzip_engine::archive::tar::reader::TarArchive;
use ttzip_engine::archive::tar::scanner::TarSeekScanner;
use ttzip_engine::security::tar_defense::{
    compute_pax_stream_stride, enclosed_tar_path, validate_hardlink_target,
    validate_pax_entry_isolation, validate_symlink_escape, TarExtractionQuotaGuard,
};
use ttzip_engine::types::TTZipStatus;

/// Helper to create a valid 512-byte TAR header block.
fn make_tar_header(name: &str, size: u64, typeflag: u8, mtime: i64) -> [u8; TAR_BLOCK_SIZE] {
    let mut block = [0u8; TAR_BLOCK_SIZE];

    let name_bytes = name.as_bytes();
    let name_len = name_bytes.len().min(100);
    block[..name_len].copy_from_slice(&name_bytes[..name_len]);

    block[100..108].copy_from_slice(b"0000644\0");
    block[108..116].copy_from_slice(b"0000000\0");
    block[116..124].copy_from_slice(b"0000000\0");

    let mut size_buf = [0u8; 12];
    format_numeric(size, &mut size_buf);
    block[124..136].copy_from_slice(&size_buf);

    let mut mtime_buf = [0u8; 12];
    format_numeric(mtime.max(0) as u64, &mut mtime_buf);
    block[136..148].copy_from_slice(&mtime_buf);

    block[148..156].copy_from_slice(b"        ");
    block[156] = typeflag;

    block[257..263].copy_from_slice(b"ustar\0");
    block[263..265].copy_from_slice(b"00");

    let (chksum, _) = compute_tar_checksum(&block);
    let chk_str = format!("{:06o}\0 ", chksum);
    block[148..156].copy_from_slice(chk_str.as_bytes());

    block
}

#[test]
fn test_tar_slip_traversal_paths_rejection() {
    let tmp = tempdir().unwrap();
    let dest_root = tmp.path();

    // 1. Classic parent traversal attacks
    assert_eq!(
        enclosed_tar_path(dest_root, "../../../../etc/passwd"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        enclosed_tar_path(dest_root, "a/../../b"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        enclosed_tar_path(dest_root, "sub/dir/../../../escaped.txt"),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 2. Embedded null byte attacks
    assert_eq!(
        enclosed_tar_path(dest_root, "safe_name.txt\0/etc/shadow"),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 3. URI scheme injection
    assert_eq!(
        enclosed_tar_path(dest_root, "file:///etc/passwd"),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 4. Windows reserved device stems
    assert_eq!(
        enclosed_tar_path(dest_root, "aux/exploit.txt"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        enclosed_tar_path(dest_root, "nested/CON.txt/file"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        enclosed_tar_path(dest_root, "com1/payload"),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 5. Embedded drive letters
    assert_eq!(
        enclosed_tar_path(dest_root, "foo/C:/bar"),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 6. Empty path or dot-only underflow
    assert_eq!(
        enclosed_tar_path(dest_root, ""),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        enclosed_tar_path(dest_root, "."),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        enclosed_tar_path(dest_root, "./."),
        Err(TTZipStatus::ErrSecurityViolation)
    );
}

#[test]
fn test_tar_slip_valid_enclosed_paths() {
    let tmp = tempdir().unwrap();
    let dest_root = tmp.path();

    // 1. Standard relative path
    let p1 = enclosed_tar_path(dest_root, "docs/report.pdf").unwrap();
    assert_eq!(p1, dest_root.join("docs/report.pdf"));

    // 2. Normalizing internal parent traversals without underflow
    let p2 = enclosed_tar_path(dest_root, "a/b/../c/file.txt").unwrap();
    assert_eq!(p2, dest_root.join("a/c/file.txt"));

    // 3. Stripping leading slashes
    let p3 = enclosed_tar_path(dest_root, "/var/log/syslog").unwrap();
    assert_eq!(p3, dest_root.join("var/log/syslog"));

    // 4. Stripping Windows drive letters
    let p4 = enclosed_tar_path(dest_root, r"C:\Windows\system.ini").unwrap();
    assert_eq!(p4, dest_root.join("Windows/system.ini"));

    // 5. Stripping UNC network prefixes
    let p5 = enclosed_tar_path(dest_root, r"\\?\UNC\server\share\data.bin").unwrap();
    assert_eq!(p5, dest_root.join("server/share/data.bin"));
}

#[test]
fn test_symlink_escape_validation() {
    let tmp = tempdir().unwrap();
    let dest_root = tmp.path();

    let sub_dir = dest_root.join("nested/level2");
    fs::create_dir_all(&sub_dir).unwrap();

    // 1. Valid symlink pointing inside sandbox
    let valid_symlink = validate_symlink_escape(dest_root, "../file.txt", &sub_dir).unwrap();
    assert_eq!(valid_symlink, dest_root.join("nested/file.txt"));

    // 2. Valid multi-hop symlink staying within dest_root
    let valid_multi = validate_symlink_escape(dest_root, "../../root_file.txt", &sub_dir).unwrap();
    assert_eq!(valid_multi, dest_root.join("root_file.txt"));

    // 3. Escape attempt: 3 levels up from depth 2 -> escapes dest_root!
    let escape_res = validate_symlink_escape(dest_root, "../../../outside.txt", &sub_dir);
    assert_eq!(escape_res, Err(TTZipStatus::ErrSecurityViolation));

    // 4. Absolute symlink targets
    assert_eq!(
        validate_symlink_escape(dest_root, "/etc/passwd", &sub_dir),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        validate_symlink_escape(dest_root, r"C:\Windows\System32", &sub_dir),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        validate_symlink_escape(dest_root, r"\\evil_server\share", &sub_dir),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 5. Parent dir outside sandbox
    let outside_dir = dest_root.parent().unwrap();
    assert_eq!(
        validate_symlink_escape(dest_root, "target.txt", outside_dir),
        Err(TTZipStatus::ErrSecurityViolation)
    );
}

#[test]
fn test_hardlink_target_validation() {
    let tmp = tempdir().unwrap();
    let dest_root = tmp.path();

    let real_file = dest_root.join("target_file.dat");
    let mut f = File::create(&real_file).unwrap();
    f.write_all(b"hardlink target content").unwrap();
    drop(f);

    // 1. Valid existing hardlink target inside sandbox
    let valid_target = validate_hardlink_target(dest_root, "target_file.dat").unwrap();
    assert_eq!(valid_target, real_file);

    // 2. Non-existent hardlink target
    assert_eq!(
        validate_hardlink_target(dest_root, "non_existent.txt"),
        Err(TTZipStatus::ErrFileNotFound)
    );

    // 3. Hardlink target attempting path traversal escape
    assert_eq!(
        validate_hardlink_target(dest_root, "../../../etc/passwd"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
}

#[test]
fn test_pax_size_smuggling_ghsa_defense() {
    // GHSA-3cv2-h65g-fgmm:
    // Attacker crafts an entry with PAX Header specifying size = 1024 bytes (2 blocks),
    // but in the ustar header specifies size = 0.
    // Naive parsers only advance 0 blocks, misinterpreting payload data as next headers.
    // TTZip's defense enforces PAX size as authoritative for stream stride advancement.

    let mut archive_bytes = Vec::new();

    // 1. PAX Extended Header for entry 1
    let pax_payload = build_pax_payload(&[("size", "1024"), ("path", "smuggled_file.bin")]);
    let pax_header = make_tar_header(
        "PaxHeaders.0/smuggled_file.bin",
        pax_payload.len() as u64,
        TYPE_PAX_EXT_HEADER,
        0,
    );
    archive_bytes.extend_from_slice(&pax_header);

    // Pad PAX payload to 512-byte block
    let pax_pad_len = 512 - pax_payload.len();
    archive_bytes.extend_from_slice(&pax_payload);
    archive_bytes.extend_from_slice(&vec![0u8; pax_pad_len]);

    // 2. Regular entry with ustar size = 0 (attack payload)
    let ustar_header = make_tar_header("placeholder_name.txt", 0, TYPE_REGULAR, 1700000000);
    archive_bytes.extend_from_slice(&ustar_header);

    // Write 1024 bytes of payload (2 blocks)
    let payload = vec![0xAAu8; 1024];
    archive_bytes.extend_from_slice(&payload);

    // 3. Legitimate second entry immediately following
    let second_header = make_tar_header("second_entry.txt", 4, TYPE_REGULAR, 1700000000);
    archive_bytes.extend_from_slice(&second_header);
    archive_bytes.extend_from_slice(b"test");
    archive_bytes.extend_from_slice(&[0u8; 508]); // pad to 512 bytes

    // 4. End-of-Archive two zero blocks
    archive_bytes.extend_from_slice(&[0u8; 1024]);

    // Test compute_pax_stream_stride
    let (eff_size, stride) = compute_pax_stream_stride(0, Some(1024)).unwrap();
    assert_eq!(eff_size, 1024);
    assert_eq!(stride, 1024);

    // Test isolation helper
    assert!(validate_pax_entry_isolation(false, &None));

    // Parse archive and verify both entries are correctly synchronized
    let mut scanner = TarSeekScanner::new(&archive_bytes);
    let entries = scanner.scan_all().expect("Archive must scan cleanly with PAX size synchronization");
    assert_eq!(entries.len(), 2);

    assert_eq!(entries[0].path, "smuggled_file.bin");
    assert_eq!(entries[0].size, 1024);

    assert_eq!(entries[1].path, "second_entry.txt");
    assert_eq!(entries[1].size, 4);

    let tar = TarArchive::open_slice(&archive_bytes).unwrap();
    let bytes0 = tar.extract_entry_bytes(0).unwrap();
    assert_eq!(bytes0.len(), 1024);
    assert_eq!(bytes0, &payload[..]);

    let bytes1 = tar.extract_entry_bytes(1).unwrap();
    assert_eq!(bytes1, b"test");
}

#[test]
fn test_pax_isolation_with_gnu_longlink() {
    let mut archive_bytes = Vec::new();

    // 1. PAX Header with size = 512 and mtime
    let pax_payload = build_pax_payload(&[("size", "512"), ("mtime", "1750000000")]);
    let pax_header = make_tar_header(
        "PaxHeaders.0/dummy",
        pax_payload.len() as u64,
        TYPE_PAX_EXT_HEADER,
        0,
    );
    archive_bytes.extend_from_slice(&pax_header);
    let pax_pad_len = 512 - pax_payload.len();
    archive_bytes.extend_from_slice(&pax_payload);
    archive_bytes.extend_from_slice(&vec![0u8; pax_pad_len]);

    // 2. GNU LongName header ('L')
    let long_name = "extremely_long_nested_directory_structure/target_data_file.bin";
    let gnu_l_header = make_tar_header("././@LongLink", long_name.len() as u64, TYPE_GNU_LONGNAME, 0);
    archive_bytes.extend_from_slice(&gnu_l_header);
    let mut name_block = [0u8; 512];
    name_block[..long_name.len()].copy_from_slice(long_name.as_bytes());
    archive_bytes.extend_from_slice(&name_block);

    // 3. Regular file entry
    let file_header = make_tar_header("short.txt", 0, TYPE_REGULAR, 1000000000);
    archive_bytes.extend_from_slice(&file_header);
    let payload = vec![0x55u8; 512];
    archive_bytes.extend_from_slice(&payload);

    // End-of-Archive
    archive_bytes.extend_from_slice(&[0u8; 1024]);

    let tar = TarArchive::open_slice(&archive_bytes).expect("PAX + GNU LongName must parse cleanly");
    assert_eq!(tar.len(), 1);
    let entry = &tar.entries()[0];
    assert_eq!(entry.path, long_name);
    assert_eq!(entry.size, 512);
    assert_eq!(entry.mtime_epoch_secs, 1750000000);
}

#[test]
fn test_tar_extraction_quota_guard_size_breaker() {
    let mut guard = TarExtractionQuotaGuard::new(1000, 100, 10.0);

    // 1. Within limit
    assert!(guard.track_bytes(100, 500).is_ok());
    assert_eq!(guard.cumulative_uncompressed(), 500);
    assert_eq!(guard.cumulative_compressed(), 100);

    // 2. Exceeding max uncompressed bytes (500 + 600 = 1100 > 1000)
    let res = guard.track_bytes(100, 600);
    assert_eq!(res, Err(TTZipStatus::ErrSecurityViolation));
}

#[test]
fn test_tar_extraction_quota_guard_entry_count_breaker() {
    let mut guard = TarExtractionQuotaGuard::new(1_000_000, 3, 10.0);

    assert!(guard.track_entry().is_ok());
    assert!(guard.track_entry().is_ok());
    assert!(guard.track_entry().is_ok());
    assert_eq!(guard.entry_count(), 3);

    // 4th entry exceeds quota of 3
    let res = guard.track_entry();
    assert_eq!(res, Err(TTZipStatus::ErrSecurityViolation));
}

#[test]
fn test_tar_extraction_quota_guard_expansion_ratio_breaker() {
    // Threshold 100 bytes, max ratio 5.0
    let mut guard = TarExtractionQuotaGuard::with_threshold(1_000_000, 100, 5.0, 100);

    // Under threshold: 50 uncompressed, 1 compressed (ratio 50 > 5, but uncompressed <= 100)
    assert!(guard.track_bytes(1, 50).is_ok());

    // Crossing threshold with excessive ratio: 500 uncompressed, 10 total compressed (ratio 55.0 > 5.0)
    let res = guard.track_bytes(9, 500);
    assert_eq!(res, Err(TTZipStatus::ErrSecurityViolation));
}
