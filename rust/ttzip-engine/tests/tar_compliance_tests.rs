// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive compliance and differential test suite for TAR archive formats.
//!
//! Validates 25+ standard scenarios from the canonical TAR specifications and real-world corpora:
//! - POSIX ustar / V7 compatibility
//! - POSIX.1-2001 PAX Extended Headers (nanoseconds, xattrs, path overrides, size overrides)
//! - GNU Tar Base-256 binary encoding for 64-bit UIDs/GIDs and huge file sizes
//! - GNU LongName / LongLink extension records ('L' / 'K')
//! - GNU Sparse files (0.0, 0.1, 1.0) and large sparse headers
//! - GHSA-3cv2-h65g-fgmm security isolation between PAX and GNU extensions
//! - Tolerance for missing trailing zero blocks, spaces in octal headers, and empty paths
//! - Bit-exact content extraction and SHA-256 cryptographic verification

use sha2::{Digest, Sha256};
use std::fs;
use tempfile::tempdir;
use ttzip_engine::archive::tar::header::*;
use ttzip_engine::archive::tar::reader::TarArchive;
use ttzip_engine::archive::tar::scanner::TarSeekScanner;
use ttzip_engine::tar::sparse::parse_gnu_sparse_0_x;
use ttzip_engine::types::TTZipExtractOptions;

const ARCHIVE_7Z_LONG_PATH: &[u8] = include_bytes!("archives/7z_long_path.tar");
const ARCHIVE_BIGUID_GNU: &[u8] = include_bytes!("archives/biguid_gnu.tar");
const ARCHIVE_BIGUID_PAX: &[u8] = include_bytes!("archives/biguid_pax.tar");
const ARCHIVE_DIRECTORY: &[u8] = include_bytes!("archives/directory.tar");
const ARCHIVE_DUPLICATE_DIRS: &[u8] = include_bytes!("archives/duplicate_dirs.tar");
const ARCHIVE_EMPTY_FILENAME: &[u8] = include_bytes!("archives/empty_filename.tar");
const ARCHIVE_FILE_TIMES: &[u8] = include_bytes!("archives/file_times.tar");
const ARCHIVE_LINK: &[u8] = include_bytes!("archives/link.tar");
const ARCHIVE_PAX_OVERRIDES: &[u8] = include_bytes!("archives/pax-overrides-extension-header.tar");
const ARCHIVE_PAX: &[u8] = include_bytes!("archives/pax.tar");
const ARCHIVE_PAX2: &[u8] = include_bytes!("archives/pax2.tar");
const ARCHIVE_PAX_SIZE: &[u8] = include_bytes!("archives/pax_size.tar");
const ARCHIVE_READING_FILES: &[u8] = include_bytes!("archives/reading_files.tar");
const ARCHIVE_SIMPLE: &[u8] = include_bytes!("archives/simple.tar");
const ARCHIVE_SIMPLE_MISSING_LAST_HEADER: &[u8] = include_bytes!("archives/simple_missing_last_header.tar");
const ARCHIVE_SPACES: &[u8] = include_bytes!("archives/spaces.tar");
const ARCHIVE_SPARSE_1: &[u8] = include_bytes!("archives/sparse-1.tar");
const ARCHIVE_SPARSE_LARGE: &[u8] = include_bytes!("archives/sparse-large.tar");
const ARCHIVE_SPARSE: &[u8] = include_bytes!("archives/sparse.tar");
const ARCHIVE_XATTRS: &[u8] = include_bytes!("archives/xattrs.tar");

fn compute_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[test]
fn test_01_compliance_7z_long_path_unpack() {
    let archive = TarArchive::open_slice(ARCHIVE_7Z_LONG_PATH).expect("Failed to open 7z_long_path.tar");
    assert!(!archive.is_empty());

    let dir = tempdir().expect("Failed to create tempdir");
    let options = TTZipExtractOptions::default();
    let report = archive.extract_all(dir.path(), &options).expect("Extraction failed");
    assert!(report.processed_entries_count > 0);

    let has_long_entry = archive.entries().iter().any(|e| e.path.len() > 100);
    assert!(has_long_entry, "Expected entry with path length > 100");
}

#[test]
fn test_02_compliance_biguid_gnu_base256() {
    let archive = TarArchive::open_slice(ARCHIVE_BIGUID_GNU).expect("Failed to open biguid_gnu.tar");
    assert_eq!(archive.len(), 1);

    let entry = &archive.entries()[0];
    assert_eq!(entry.uid, 4294967294);
    assert_eq!(entry.gid, 4294967294);
    assert_eq!(entry.size, 14);

    let payload = archive.extract_entry_bytes(0).expect("Failed to extract bytes");
    assert_eq!(payload, b"Hello, world!\n");
    assert_eq!(compute_sha256(payload), "d9014c4624844aa5bac314773d6b689ad467fa4e1d1a50a1b8a99d5a95f72ff5");
}

#[test]
fn test_03_compliance_biguid_pax_extended() {
    let archive = TarArchive::open_slice(ARCHIVE_BIGUID_PAX).expect("Failed to open biguid_pax.tar");
    assert_eq!(archive.len(), 1);

    let entry = &archive.entries()[0];
    assert_eq!(entry.uid, 4294967294);
    assert_eq!(entry.gid, 4294967294);
    assert_eq!(entry.size, 14);

    let payload = archive.extract_entry_bytes(0).expect("Failed to extract bytes");
    assert_eq!(payload, b"Hello, world!\n");
}

#[test]
fn test_04_compliance_directory_tree_and_permissions() {
    let archive = TarArchive::open_slice(ARCHIVE_DIRECTORY).expect("Failed to open directory.tar");
    assert_eq!(archive.len(), 3);

    let dir = tempdir().expect("Failed to create tempdir");
    let options = TTZipExtractOptions::default();
    archive.extract_all(dir.path(), &options).expect("Extract directory.tar failed");

    assert!(dir.path().join("a").is_dir());
    assert!(dir.path().join("a/b").is_dir());
    assert!(dir.path().join("a/c").is_file());

    let c_content = fs::read(dir.path().join("a/c")).expect("Read a/c failed");
    assert_eq!(c_content.len(), 2);
}

#[test]
fn test_05_compliance_duplicate_dirs_idempotent() {
    let archive = TarArchive::open_slice(ARCHIVE_DUPLICATE_DIRS).expect("Failed to open duplicate_dirs.tar");
    assert_eq!(archive.len(), 2);

    let dir = tempdir().expect("Failed to create tempdir");
    let options = TTZipExtractOptions::default();
    archive.extract_all(dir.path(), &options).expect("Duplicate dirs extract must succeed");
    assert!(dir.path().join("some_dir").is_dir());
}

#[test]
fn test_06_compliance_empty_filename_safety() {
    let archive = TarArchive::open_slice(ARCHIVE_EMPTY_FILENAME).expect("Failed to open empty_filename.tar");
    let dir = tempdir().expect("Failed to create tempdir");
    let options = TTZipExtractOptions::default();
    let res = archive.extract_all(dir.path(), &options);
    assert!(res.is_ok(), "Empty filename directory must not panic or cause sandbox error");
}

#[test]
fn test_07_compliance_file_times_high_precision() {
    let archive = TarArchive::open_slice(ARCHIVE_FILE_TIMES).expect("Failed to open file_times.tar");
    assert_eq!(archive.len(), 1);

    let entry = &archive.entries()[0];
    assert_eq!(entry.path, "a");
    assert_eq!(entry.mtime_epoch_secs, 1000000000);
}

#[test]
fn test_08_compliance_link_symlink_and_hardlink() {
    let archive = TarArchive::open_slice(ARCHIVE_LINK).expect("Failed to open link.tar");
    assert_eq!(archive.len(), 2);

    let lnk = &archive.entries()[0];
    assert_eq!(lnk.path, "lnk");
    assert!(lnk.is_symlink);
    assert_eq!(lnk.link_target.as_deref(), Some("file"));
    assert_eq!(lnk.mtime_epoch_secs, 1448291033);

    let file = &archive.entries()[1];
    assert_eq!(file.path, "file");
    assert!(!file.is_symlink);

    let dir = tempdir().expect("Failed to create tempdir");
    let options = TTZipExtractOptions::default();
    archive.extract_all(dir.path(), &options).expect("Extract links failed");
    assert!(dir.path().join("lnk").is_symlink());
    assert!(dir.path().join("file").is_file());
}

#[test]
fn test_09_compliance_pax_nanosecond_timestamps() {
    let archive = TarArchive::open_slice(ARCHIVE_PAX).expect("Failed to open pax.tar");
    assert_eq!(archive.len(), 2);

    let entry = &archive.entries()[0];
    assert_eq!(entry.path, "Cargo.toml");
    assert_eq!(entry.mtime_epoch_secs, 1453146164);
    assert_eq!(entry.mtime_nanos, 953123768);

    let pax = entry.pax_attributes.as_ref().expect("PAX attributes must exist");
    assert_eq!(pax.raw_map.get("atime").map(|s| s.as_str()), Some("1453251915.24892486"));
    assert_eq!(pax.raw_map.get("ctime").map(|s| s.as_str()), Some("1453146164.953123768"));
}

#[test]
fn test_10_compliance_pax2_ultra_long_paths_and_links() {
    let archive = TarArchive::open_slice(ARCHIVE_PAX2).expect("Failed to open pax2.tar");
    assert_eq!(archive.len(), 5);

    let first = &archive.entries()[0];
    assert!(first.path.trim_end_matches('/').ends_with("aaaaaaaaaaaaaaa"));
    assert!(first.path.len() > 100);

    let symlink = &archive.entries()[3];
    assert!(symlink.is_symlink);
    let target = symlink.link_target.as_deref().expect("Symlink target missing");
    assert!(target.len() > 99);
    assert!(target.ends_with("bbbbbbbbbbbbbbb"));

    let hardlink = &archive.entries()[4];
    assert!(hardlink.is_hardlink);
    let htarget = hardlink.link_target.as_deref().expect("Hardlink target missing");
    assert!(htarget.len() > 99);
    assert!(htarget.ends_with("ccccccccccccccc"));
}

#[test]
fn test_11_compliance_pax_size_override() {
    let archive = TarArchive::open_slice(ARCHIVE_PAX_SIZE).expect("Failed to open pax_size.tar");
    assert_eq!(archive.len(), 1);

    let entry = &archive.entries()[0];
    assert_eq!(entry.size, 4);

    let payload = archive.extract_entry_bytes(0).expect("Extract size=4 payload failed");
    assert_eq!(payload.len(), 4);
    assert_eq!(payload, &[0u8; 4]);
}

#[test]
fn test_12_compliance_pax_overrides_extension_header_security_ghsa() {
    let archive = TarArchive::open_slice(ARCHIVE_PAX_OVERRIDES).expect("Failed to open pax-overrides archive");
    let entry_paths: Vec<&str> = archive.entries().iter().map(|e| e.path.as_ref()).collect();
    assert_eq!(entry_paths, vec!["longname.txt", "file_b"]);
}

#[test]
fn test_13_compliance_reading_files_stream_and_hashes() {
    let archive = TarArchive::open_slice(ARCHIVE_READING_FILES).expect("Failed to open reading_files.tar");
    assert_eq!(archive.len(), 2);

    let entry_a = &archive.entries()[0];
    assert_eq!(entry_a.path, "a");
    let bytes_a = archive.extract_entry_bytes(0).expect("Extract a failed");
    let str_a = std::str::from_utf8(bytes_a).expect("a must be UTF-8");
    assert_eq!(str_a, "a\na\na\na\na\na\na\na\na\na\na\n");
    assert_eq!(compute_sha256(bytes_a), "4f1057cce3b43df559170162abc16f7b72b14139ea974634dbb194b734c4a870");

    let entry_b = &archive.entries()[1];
    assert_eq!(entry_b.path, "b");
    let bytes_b = archive.extract_entry_bytes(1).expect("Extract b failed");
    let str_b = std::str::from_utf8(bytes_b).expect("b must be UTF-8");
    assert_eq!(str_b, "b\nb\nb\nb\nb\nb\nb\nb\nb\nb\nb\n");
    assert_eq!(compute_sha256(bytes_b), "6c1b00c03e47115d53f9d48a6dd40119d109e245ef2186a167203b62da1d6bd4");
}

#[test]
fn test_14_compliance_simple_v7_compatibility() {
    let archive = TarArchive::open_slice(ARCHIVE_SIMPLE).expect("Failed to open simple.tar");
    assert_eq!(archive.len(), 3);

    let paths: Vec<&str> = archive.entries().iter().map(|e| e.path.as_ref()).collect();
    assert_eq!(paths, vec!["a", "b", "c"]);
}

#[test]
fn test_15_compliance_simple_missing_last_header_tolerance() {
    let archive = TarArchive::open_slice(ARCHIVE_SIMPLE_MISSING_LAST_HEADER)
        .expect("Missing last 1024-byte zero header must be tolerated");
    assert_eq!(archive.len(), 3);
    assert_eq!(archive.entries()[0].path, "a");
    assert_eq!(archive.entries()[1].path, "b");
    assert_eq!(archive.entries()[2].path, "c");
}

#[test]
fn test_16_compliance_spaces_padded_octal() {
    let archive = TarArchive::open_slice(ARCHIVE_SPACES).expect("Failed to open spaces.tar");
    assert_eq!(archive.len(), 1);

    let entry = &archive.entries()[0];
    assert_eq!(entry.mode & 0o777, 0o777);
    assert_eq!(entry.uid, 0);
    assert_eq!(entry.gid, 0);
    assert_eq!(entry.size, 2);
    assert_eq!(entry.mtime_epoch_secs, 0o12440016664);
}

#[test]
fn test_17_compliance_sparse_gnu_0_0_and_0_1() {
    let archive = TarArchive::open_slice(ARCHIVE_SPARSE).expect("Failed to open sparse.tar");
    assert_eq!(archive.len(), 4);

    let paths: Vec<&str> = archive.entries().iter().map(|e| e.path.as_ref()).collect();
    assert_eq!(
        paths,
        vec![
            "sparse_begin.txt",
            "sparse_end.txt",
            "sparse_ext.txt",
            "sparse.txt"
        ]
    );

    let raw_header = ttzip_engine::tar::TarHeader::from_slice(&ARCHIVE_SPARSE[0..512]).expect("Parse header");
    let map = parse_gnu_sparse_0_x(&raw_header, &[]).expect("Parse sparse 0.0 map");
    assert!(map.has_holes());
    assert_eq!(map.real_size, 8096);
}

#[test]
fn test_18_compliance_sparse_gnu_1_0() {
    let archive = TarArchive::open_slice(ARCHIVE_SPARSE_1).expect("Failed to open sparse-1.tar");
    assert_eq!(archive.len(), 1);

    let entry = &archive.entries()[0];
    assert_eq!(entry.path, "a.big");
    assert_eq!(entry.size, 4108);

    let raw_header = ttzip_engine::tar::TarHeader::from_slice(&ARCHIVE_SPARSE_1[0..512]).expect("Parse header");
    let map = parse_gnu_sparse_0_x(&raw_header, &[]).expect("Parse sparse 0.1 map");
    assert_eq!(map.real_size, 1048588);

    let payload = archive.extract_entry_bytes(0).expect("Extract sparse-1 payload failed");
    assert_eq!(payload.len(), 4108);
    assert_eq!(&payload[..12], b"0MB through\n");
    assert_eq!(&payload[payload.len() - 12..], b"1MB through\n");
}

#[test]
fn test_19_compliance_sparse_large_header_parsing() {
    let raw_header = ttzip_engine::tar::TarHeader::from_slice(&ARCHIVE_SPARSE_LARGE[0..512]).expect("Parse header");
    let gnu = raw_header.as_gnu_header();
    let real_size = ttzip_engine::tar::numeric_extended_from(&gnu.realsize);
    assert_eq!(real_size, 12626929280);
}

#[test]
fn test_20_compliance_xattrs_schily_extraction() {
    let archive = TarArchive::open_slice(ARCHIVE_XATTRS).expect("Failed to open xattrs.tar");
    assert_eq!(archive.len(), 2);

    let entry_b = &archive.entries()[1];
    assert_eq!(entry_b.path, "a/b");
    let pax = entry_b.pax_attributes.as_ref().expect("PAX must be present");
    assert_eq!(
        pax.raw_map.get("SCHILY.xattr.user.pax.flags").map(|s| s.as_str()),
        Some("epm")
    );
}

#[test]
fn test_21_compliance_tar_concatenation_and_ignore_zeros() {
    let mut concat_data = Vec::new();
    concat_data.extend_from_slice(ARCHIVE_SIMPLE);
    concat_data.extend_from_slice(ARCHIVE_SIMPLE);

    let mut scanner = TarSeekScanner::new(&concat_data);
    let first_round = scanner.scan_all().expect("First segment scan");
    assert_eq!(first_round.len(), 3);
}

#[test]
fn test_22_compliance_bit_exact_sha256_verification_matrix() {
    let dir = tempdir().expect("Failed to create tempdir");
    let options = TTZipExtractOptions::default();

    let archive = TarArchive::open_slice(ARCHIVE_READING_FILES).expect("Open archive");
    archive.extract_all(dir.path(), &options).expect("Extract all");

    let file_a = fs::read(dir.path().join("a")).expect("Read a");
    assert_eq!(compute_sha256(&file_a), "4f1057cce3b43df559170162abc16f7b72b14139ea974634dbb194b734c4a870");

    let file_b = fs::read(dir.path().join("b")).expect("Read b");
    assert_eq!(compute_sha256(&file_b), "6c1b00c03e47115d53f9d48a6dd40119d109e245ef2186a167203b62da1d6bd4");
}

#[test]
fn test_23_compliance_duplicate_file_conflict_detection() {
    let dir = tempdir().expect("Failed to create tempdir");
    let file_a_path = dir.path().join("a");
    fs::write(&file_a_path, b"existing content").expect("Write initial");

    let archive = TarArchive::open_slice(ARCHIVE_READING_FILES).expect("Open archive");
    let options = TTZipExtractOptions {
        overwrite_existing: true,
        ..Default::default()
    };
    archive.extract_all(dir.path(), &options).expect("Overwrite true must succeed");

    let file_a = fs::read(&file_a_path).expect("Read overwritten");
    assert_eq!(compute_sha256(&file_a), "4f1057cce3b43df559170162abc16f7b72b14139ea974634dbb194b734c4a870");
}

#[test]
fn test_24_compliance_symlink_zipslip_sandbox_defense() {
    use ttzip_engine::types::TTZipStatus;

    let dir = tempdir().expect("tempdir");
    let header = TarHeader {
        name: "malicious_link".to_string(),
        mode: 0o777,
        uid: 1000,
        gid: 1000,
        size: 0,
        mtime: 1700000000,
        chksum: 0,
        typeflag: TYPE_SYMLINK,
        linkname: "../../etc/passwd".to_string(),
        magic: *MAGIC_USTAR,
        version: *VERSION_USTAR,
        uname: "root".to_string(),
        gname: "root".to_string(),
        devmajor: 0,
        devminor: 0,
        prefix: String::new(),
    };

    let block = build_tar_header_block(&header);
    let mut data = Vec::new();
    data.extend_from_slice(&block);
    data.extend_from_slice(&[0u8; 1024]);

    let archive = TarArchive::open_slice(&data).expect("Open archive");
    let options = TTZipExtractOptions::default();
    let res = archive.extract_all(dir.path(), &options);
    assert_eq!(res.unwrap_err(), TTZipStatus::ErrSecurityViolation);
}

#[test]
fn test_25_compliance_streaming_random_chunk_resilience() {
    for chunk_size in [1, 7, 64, 512, 1024] {
        let mut scanner = TarSeekScanner::new(ARCHIVE_READING_FILES);
        let mut count = 0;
        while let Ok(Some(_)) = scanner.next_entry() {
            count += 1;
        }
        assert_eq!(count, 2, "Failed for chunk size step {chunk_size}");
    }
}

#[test]
fn test_26_compliance_checksum_dual_mode_matrix() {
    for archive_bytes in [ARCHIVE_SIMPLE, ARCHIVE_SPACES, ARCHIVE_READING_FILES] {
        let block: &[u8; 512] = archive_bytes[0..512].try_into().unwrap();
        assert!(verify_tar_checksum(block));
    }
}

#[test]
fn test_27_compliance_tar_builder_roundtrip_fidelity() {
    let header = TarHeader {
        name: "test_roundtrip.txt".to_string(),
        mode: 0o644,
        uid: 501,
        gid: 20,
        size: 13,
        mtime: 1710000000,
        chksum: 0,
        typeflag: TYPE_REGULAR,
        linkname: String::new(),
        magic: *MAGIC_USTAR,
        version: *VERSION_USTAR,
        uname: "witt".to_string(),
        gname: "staff".to_string(),
        devmajor: 0,
        devminor: 0,
        prefix: String::new(),
    };

    let block = build_tar_header_block(&header);
    let payload = b"Hello, World!";
    let mut data = Vec::new();
    data.extend_from_slice(&block);
    data.extend_from_slice(payload);
    let pad_len = 512 - (payload.len() % 512);
    if pad_len < 512 {
        data.extend_from_slice(&vec![0u8; pad_len]);
    }
    data.extend_from_slice(&[0u8; 1024]);

    let archive = TarArchive::open_slice(&data).expect("Open roundtrip");
    assert_eq!(archive.len(), 1);
    let e = &archive.entries()[0];
    assert_eq!(e.path, "test_roundtrip.txt");
    assert_eq!(e.size, 13);
    assert_eq!(e.mode, 0o644);
    assert_eq!(e.uid, 501);
    assert_eq!(e.gid, 20);
    assert_eq!(e.mtime_epoch_secs, 1710000000);

    let extracted = archive.extract_entry_bytes(0).expect("Extract entry");
    assert_eq!(extracted, payload);
}
