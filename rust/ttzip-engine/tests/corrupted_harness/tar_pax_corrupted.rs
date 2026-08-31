// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Corrupted TAR, PAX Extended Header, and GNU Tar Sparse/LongLink Security Test Suite.
//!
//! Validates:
//! 1. PAX negative timestamps (pre-1970 dates) and sub-second nanosecond precision.
//! 2. PAX Year 2038+ and 64-bit integer overflow timestamps (Year 9999, i64::MAX).
//! 3. PAX negative size, malformed length prefixes, and oversized attribute payloads.
//! 4. GNU Tar redundant LongName ('L') and LongLink ('K') sequence flooding defenses.
//! 5. Malformed GNU Tar sparse block overlap, wrapping, and out-of-bounds offset sanitization.

use std::panic::{catch_unwind, AssertUnwindSafe};

use super::uudecode::load_libarchive_asset;
use ttzip_engine::archive::tar::header::{
    compute_tar_checksum, format_numeric, TAR_BLOCK_SIZE, TYPE_GNU_LONGNAME, TYPE_REGULAR,
};
use ttzip_engine::archive::tar::pax::{parse_pax_data, parse_pax_timestamp};
use ttzip_engine::archive::tar::reader::TarArchive;
use ttzip_engine::archive::tar::scanner::TarSeekScanner;
use ttzip_engine::archive::unified::entry::{
    clean_sparse_extents, coalesce_sparse_extents, SparseExtent,
};

/// Helper to assemble a valid 512-byte TAR header block with calculated checksum.
fn make_tar_block(name: &str, size: u64, typeflag: u8, mtime: i64) -> [u8; TAR_BLOCK_SIZE] {
    let mut block = [0u8; TAR_BLOCK_SIZE];

    let name_bytes = name.as_bytes();
    let name_len = name_bytes.len().min(100);
    block[..name_len].copy_from_slice(&name_bytes[..name_len]);

    // Mode
    block[100..108].copy_from_slice(b"0000644\0");
    // UID / GID
    block[108..116].copy_from_slice(b"0000000\0");
    block[116..124].copy_from_slice(b"0000000\0");

    // Size (octal)
    let mut size_buf = [0u8; 12];
    format_numeric(size, &mut size_buf);
    block[124..136].copy_from_slice(&size_buf);

    // MTime (octal)
    let mut mtime_buf = [0u8; 12];
    format_numeric(mtime.max(0) as u64, &mut mtime_buf);
    block[136..148].copy_from_slice(&mtime_buf);

    // Checksum placeholder spaces
    block[148..156].copy_from_slice(b"        ");

    // Typeflag
    block[156] = typeflag;

    // Magic & Version
    block[257..263].copy_from_slice(b"ustar\0");
    block[263..265].copy_from_slice(b"00");

    // Compute and insert checksum
    let (chksum, _) = compute_tar_checksum(&block);
    let chk_str = format!("{:06o}\0 ", chksum);
    block[148..156].copy_from_slice(chk_str.as_bytes());

    block
}

#[test]
pub fn test_corrupted_pax_negative_time_real_asset() {
    let asset = load_libarchive_asset("test_read_format_tar_pax_negative_time.tar");
    assert!(
        asset.is_some(),
        "test_read_format_tar_pax_negative_time.tar fixture must exist"
    );
    let bytes = asset.unwrap();

    let tar = TarArchive::open_slice(&bytes).expect("TAR parsing must succeed for pax negative time");
    assert_eq!(tar.len(), 1);

    let entry = &tar.entries()[0];
    assert_eq!(entry.path, "empty");
    // Pre-1970 timestamp (-2146608000 corresponds to 1902-01-01)
    assert_eq!(entry.mtime_epoch_secs, -2146608000);
}

#[test]
pub fn test_corrupted_pax_invalid_negative_size_real_asset() {
    if let Some(bytes) = load_libarchive_asset("test_read_format_tar_invalid_pax_size.tar") {
        let res = catch_unwind(AssertUnwindSafe(|| {
            let mut scanner = TarSeekScanner::new(&bytes);
            let _ = scanner.scan_all();
            let _ = TarArchive::open_slice(&bytes);
        }));
        assert!(
            res.is_ok(),
            "Parsing invalid negative PAX size must never panic or underflow"
        );
    }
}

#[test]
pub fn test_corrupted_pax_timestamp_parsing_variations() {
    // 1. Negative integer timestamp
    let (secs1, nanos1) = parse_pax_timestamp("-2146608000");
    assert_eq!(secs1, -2146608000);
    assert_eq!(nanos1, 0);

    // 2. Negative timestamp with sub-second nanoseconds
    let (secs2, nanos2) = parse_pax_timestamp("-123456789.987654321");
    assert_eq!(secs2, -123456789);
    assert_eq!(nanos2, 987654321);

    // 3. Year 2038+ overflow timestamp
    let (secs3, nanos3) = parse_pax_timestamp("253402300799.500");
    assert_eq!(secs3, 253402300799);
    assert_eq!(nanos3, 500000000);

    // 4. i64::MAX timestamp
    let (secs4, nanos4) = parse_pax_timestamp("9223372036854775807");
    assert_eq!(secs4, i64::MAX);
    assert_eq!(nanos4, 0);

    // 5. Invalid non-numeric timestamp falls back safely to 0
    let (secs5, nanos5) = parse_pax_timestamp("invalid_corrupted_time");
    assert_eq!(secs5, 0);
    assert_eq!(nanos5, 0);
}

#[test]
pub fn test_corrupted_pax_payload_records_boundary_fuzz() {
    // 1. Record length < minimum space delimiter
    let malformed_record = b"3 a=b\n";
    let attrs = parse_pax_data(malformed_record);
    assert_eq!(attrs.path, None);

    // 2. Record length exceeds payload buffer
    let oob_record = b"9999999 path=test.txt\n";
    let attrs2 = parse_pax_data(oob_record);
    assert_eq!(attrs2.path, None);

    // 3. Negative size attribute in PAX payload
    let neg_size_payload = b"25 size=-9223372036854775808\n";
    let attrs3 = parse_pax_data(neg_size_payload);
    assert_eq!(attrs3.size, None, "Negative size must not populate unsigned size");

    // 4. Multiple valid and invalid interleaved records
    let mut mixed = Vec::new();
    mixed.extend_from_slice(b"28 path=legitimate_file.txt\n");
    mixed.extend_from_slice(b"14 mtime=-100\n");
    mixed.extend_from_slice(b"12 invalid\n"); // malformed
    let attrs4 = parse_pax_data(&mixed);
    assert_eq!(attrs4.path.as_deref(), Some("legitimate_file.txt"));
    assert_eq!(attrs4.mtime_secs, Some(-100));
}

#[test]
pub fn test_corrupted_gtar_redundant_longname_and_longlink_attacks() {
    let mut archive_bytes = Vec::new();

    // Inject 3 consecutive redundant 'L' (GNU LongName) header blocks
    let name1 = "first_discarded_name.txt";
    let block_l1 = make_tar_block("././@LongLink", name1.len() as u64, TYPE_GNU_LONGNAME, 0);
    archive_bytes.extend_from_slice(&block_l1);
    let mut payload1 = [0u8; 512];
    payload1[..name1.len()].copy_from_slice(name1.as_bytes());
    archive_bytes.extend_from_slice(&payload1);

    let name2 = "second_discarded_name.txt";
    let block_l2 = make_tar_block("././@LongLink", name2.len() as u64, TYPE_GNU_LONGNAME, 0);
    archive_bytes.extend_from_slice(&block_l2);
    let mut payload2 = [0u8; 512];
    payload2[..name2.len()].copy_from_slice(name2.as_bytes());
    archive_bytes.extend_from_slice(&payload2);

    let name3 = "final_persisted_name.txt";
    let block_l3 = make_tar_block("././@LongLink", name3.len() as u64, TYPE_GNU_LONGNAME, 0);
    archive_bytes.extend_from_slice(&block_l3);
    let mut payload3 = [0u8; 512];
    payload3[..name3.len()].copy_from_slice(name3.as_bytes());
    archive_bytes.extend_from_slice(&payload3);

    // Actual file header block
    let file_block = make_tar_block("fallback.txt", 4, TYPE_REGULAR, 1700000000);
    archive_bytes.extend_from_slice(&file_block);
    let mut file_payload = [0u8; 512];
    file_payload[..4].copy_from_slice(b"data");
    archive_bytes.extend_from_slice(&file_payload);

    // End-of-Archive 2 zero blocks
    archive_bytes.extend_from_slice(&[0u8; 1024]);

    // Parse and verify scanner safely overrides name to the latest 'L' header
    let tar = TarArchive::open_slice(&archive_bytes).expect("redundant 'L' archive must parse cleanly");
    assert_eq!(tar.len(), 1);
    assert_eq!(tar.entries()[0].path, "final_persisted_name.txt");
    assert_eq!(tar.extract_entry_bytes(0).unwrap(), b"data");
}

#[test]
pub fn test_corrupted_gtar_sparse_extents_overlap_and_overflow_defense() {
    // 1. Overlapping sparse extents coalescing
    let mut raw_extents = vec![
        SparseExtent { offset: 0, length: 100 },
        SparseExtent { offset: 50, length: 100 }, // overlaps previous extent
        SparseExtent { offset: 120, length: 80 }, // overlaps both
    ];

    coalesce_sparse_extents(&mut raw_extents);
    assert_eq!(raw_extents.len(), 1);
    assert_eq!(raw_extents[0].offset, 0);
    assert_eq!(raw_extents[0].length, 200);

    // 2. Out-of-bounds sparse extents and hole validation
    let mut hole_extents = vec![
        SparseExtent { offset: 0, length: 300 },
        SparseExtent { offset: 600, length: 200 },
    ];

    let is_sparse = clean_sparse_extents(&mut hole_extents, 1000);
    assert!(is_sparse, "Should identify file as sparse containing holes");
    assert_eq!(hole_extents.len(), 2);
}
