// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive Integration & Defensive Invariant Test Suite for `EocdScanner`.
//!
//! Tests backward sliding window scanning, 64KB comments, spoofed EOCD signatures,
//! SFX preamble auto-detection, zero-panic boundary guarantees, and SIMD vector equivalence.

use std::io::Cursor;
use ttzip_engine::types::TTZipEncryptionMethod;
use ttzip_engine::zip::scanner::find_eocd_candidates_scalar;
use ttzip_engine::zip::{
    assemble_zip_archive, compress_items_parallel, find_eocd_candidate_offsets, EocdScanner,
    ZipEngineError, ZipInputItem, EOCD_MIN_SIZE, MAX_COMMENT_LEN, MAX_EOCD_SEARCH_WINDOW,
};

/// Helper to generate a valid minimal in-memory ZIP archive.
fn build_sample_zip(entry_count: usize) -> Vec<u8> {
    let mut items = Vec::with_capacity(entry_count);
    for i in 0..entry_count {
        items.push(ZipInputItem {
            rel_path: format!("file_{}.txt", i),
            data: format!("Payload content for test file index {}", i).into_bytes(),
            mtime_epoch_secs: 1700000000 + (i as u32 * 10),
            mode: 0o644,
            is_directory: false,
        });
    }

    let compressed = compress_items_parallel(
        items,
        1,
        TTZipEncryptionMethod::None,
        None,
        2,
    ).expect("compress_items_parallel failed");

    assemble_zip_archive(&compressed).expect("assemble_zip_archive failed")
}

#[test]
fn test_standard_zip_no_comment() {
    let zip_bytes = build_sample_zip(3);
    assert!(zip_bytes.len() >= EOCD_MIN_SIZE);

    // Test scan_slice
    let info = EocdScanner::scan_slice(&zip_bytes).expect("scan_slice failed on standard zip");
    assert_eq!(info.archive_offset, 0);
    assert_eq!(info.total_entries, 3);
    assert!(info.comment.is_empty());
    assert_eq!(info.eocd_offset, (zip_bytes.len() - EOCD_MIN_SIZE) as u64);
    assert!(info.cd_size > 0);
    assert_eq!(info.cd_offset + info.cd_size, info.eocd_offset);

    // Test scan on Read + Seek stream
    let mut cursor = Cursor::new(&zip_bytes);
    let stream_info = EocdScanner::scan(&mut cursor, zip_bytes.len() as u64)
        .expect("scan Read+Seek failed on standard zip");
    assert_eq!(info, stream_info);
}

#[test]
fn test_max_comment_zip_65535_bytes() {
    let mut zip_bytes = build_sample_zip(1);
    let original_eocd_pos = zip_bytes.len() - EOCD_MIN_SIZE;

    // Create maximum 65535-byte comment
    let comment_payload = vec![0x5A; MAX_COMMENT_LEN]; // 'Z' * 65535

    // Patch comment length field in EOCD (offset 20 from EOCD start)
    zip_bytes[original_eocd_pos + 20] = 0xFF;
    zip_bytes[original_eocd_pos + 21] = 0xFF;

    // Append comment bytes to zip
    zip_bytes.extend_from_slice(&comment_payload);

    assert_eq!(zip_bytes.len(), original_eocd_pos + EOCD_MIN_SIZE + MAX_COMMENT_LEN);

    // Test scan_slice
    let info = EocdScanner::scan_slice(&zip_bytes).expect("scan_slice failed with max comment");
    assert_eq!(info.archive_offset, 0);
    assert_eq!(info.total_entries, 1);
    assert_eq!(info.eocd_offset, original_eocd_pos as u64);
    assert_eq!(info.comment.len(), MAX_COMMENT_LEN);
    assert_eq!(info.comment, comment_payload);

    // Test scan on Read + Seek stream
    let mut cursor = Cursor::new(&zip_bytes);
    let stream_info = EocdScanner::scan(&mut cursor, zip_bytes.len() as u64)
        .expect("scan Read+Seek failed with max comment");
    assert_eq!(info, stream_info);
}

#[test]
fn test_fake_eocd_magic_in_comment_spoofing_defense() {
    let mut zip_bytes = build_sample_zip(2);
    let real_eocd_pos = zip_bytes.len() - EOCD_MIN_SIZE;

    // Create a crafted comment containing fake PK\x05\x06 signatures and fake records
    let mut comment = Vec::new();
    comment.extend_from_slice(b"Leading comment text...");

    // Inject spoofed EOCD magic PK\x05\x06
    let fake_eocd_start = comment.len();
    comment.extend_from_slice(&[0x50, 0x4B, 0x05, 0x06]); // PK\x05\x06
    comment.extend_from_slice(&[0x00, 0x00]); // disk 0
    comment.extend_from_slice(&[0x00, 0x00]); // cd start disk 0
    comment.extend_from_slice(&[0x00, 0x00]); // disk entries 0
    comment.extend_from_slice(&[0x05, 0x00]); // total entries 5
    comment.extend_from_slice(&[0x20, 0x00, 0x00, 0x00]); // fake cd_size 32
    comment.extend_from_slice(&[0x10, 0x00, 0x00, 0x00]); // fake cd_offset 16
    comment.extend_from_slice(&[0x04, 0x00]); // fake comment_len 4
    comment.extend_from_slice(b"TAIL"); // fake comment payload

    let comment_len = comment.len();
    assert!(comment_len <= MAX_COMMENT_LEN);

    // Patch real EOCD comment length
    zip_bytes[real_eocd_pos + 20] = (comment_len & 0xFF) as u8;
    zip_bytes[real_eocd_pos + 21] = ((comment_len >> 8) & 0xFF) as u8;
    zip_bytes.extend_from_slice(&comment);

    // Ensure the fake magic actually exists in the stream
    assert_eq!(
        &zip_bytes[real_eocd_pos + EOCD_MIN_SIZE + fake_eocd_start..real_eocd_pos + EOCD_MIN_SIZE + fake_eocd_start + 4],
        &[0x50, 0x4B, 0x05, 0x06]
    );

    // The scanner must reject the fake EOCD signature in the comment and correctly identify the real EOCD
    let info = EocdScanner::scan_slice(&zip_bytes)
        .expect("EocdScanner should successfully bypass fake EOCD in comment");

    assert_eq!(info.eocd_offset, real_eocd_pos as u64);
    assert_eq!(info.archive_offset, 0);
    assert_eq!(info.total_entries, 2);
    assert_eq!(info.comment, comment);

    // Verify Read + Seek stream mode
    let mut cursor = Cursor::new(&zip_bytes);
    let stream_info = EocdScanner::scan(&mut cursor, zip_bytes.len() as u64)
        .expect("scan Read+Seek should bypass fake EOCD");
    assert_eq!(info, stream_info);
}

#[test]
fn test_sfx_10kb_preamble_adaptive_offset() {
    let zip_bytes = build_sample_zip(4);
    let original_eocd_pos = zip_bytes.len() - EOCD_MIN_SIZE;

    // Create 10KB (10240 bytes) fake Mach-O / ELF binary stub
    let sfx_preamble_len = 10240;
    let mut sfx_executable = vec![0x90u8; sfx_preamble_len]; // NOP sled
    // Emulate Mach-O 64-bit header magic MH_MAGIC_64 (0xFEEDFACF)
    sfx_executable[0] = 0xCF;
    sfx_executable[1] = 0xFA;
    sfx_executable[2] = 0xED;
    sfx_executable[3] = 0xFE;

    // Concatenate SFX preamble + ZIP payload
    let mut sfx_binary = sfx_executable;
    sfx_binary.extend_from_slice(&zip_bytes);

    assert_eq!(sfx_binary.len(), sfx_preamble_len + zip_bytes.len());

    // Run EocdScanner
    let info = EocdScanner::scan_slice(&sfx_binary).expect("scan_slice on SFX archive failed");
    assert_eq!(info.archive_offset, sfx_preamble_len as u64);
    assert_eq!(info.eocd_offset, (sfx_preamble_len + original_eocd_pos) as u64);
    assert_eq!(info.total_entries, 4);

    // Verify stream mode
    let mut cursor = Cursor::new(&sfx_binary);
    let stream_info = EocdScanner::scan(&mut cursor, sfx_binary.len() as u64)
        .expect("scan on SFX Read+Seek stream failed");
    assert_eq!(info, stream_info);
}

#[test]
fn test_truncated_and_empty_files_zero_panic() {
    // 0 bytes empty file
    let empty_res = EocdScanner::scan_slice(&[]);
    assert_eq!(
        empty_res,
        Err(ZipEngineError::FileTooSmall {
            required: EOCD_MIN_SIZE,
            actual: 0,
        })
    );

    // 10 bytes file (< 22)
    let ten_bytes = vec![0x42; 10];
    let short_res = EocdScanner::scan_slice(&ten_bytes);
    assert_eq!(
        short_res,
        Err(ZipEngineError::FileTooSmall {
            required: EOCD_MIN_SIZE,
            actual: 10,
        })
    );

    // 21 bytes file (1 byte short)
    let twenty_one = vec![0x42; 21];
    let short_res2 = EocdScanner::scan_slice(&twenty_one);
    assert_eq!(
        short_res2,
        Err(ZipEngineError::FileTooSmall {
            required: EOCD_MIN_SIZE,
            actual: 21,
        })
    );

    // Random non-ZIP garbage 64KB
    let garbage = vec![0xAA; 65536];
    let garbage_res = EocdScanner::scan_slice(&garbage);
    assert_eq!(garbage_res, Err(ZipEngineError::EocdNotFound));

    // Stream reader zero-panic checks
    let mut empty_cursor = Cursor::new(Vec::new());
    assert_eq!(
        EocdScanner::scan(&mut empty_cursor, 0),
        Err(ZipEngineError::FileTooSmall {
            required: EOCD_MIN_SIZE,
            actual: 0,
        })
    );
}

#[test]
fn test_simd_vs_scalar_candidate_offsets_equivalence() {
    // Test on multiple synthetic buffers
    let mut buffer = vec![0u8; 1024];

    // Place several PK\x05\x06 signatures
    buffer[100] = 0x50;
    buffer[101] = 0x4B;
    buffer[102] = 0x05;
    buffer[103] = 0x06;

    buffer[450] = 0x50;
    buffer[451] = 0x4B;
    buffer[452] = 0x05;
    buffer[453] = 0x06;

    buffer[900] = 0x50;
    buffer[901] = 0x4B;
    buffer[902] = 0x05;
    buffer[903] = 0x06;

    let simd_candidates = find_eocd_candidate_offsets(&buffer);

    let max_pos = buffer.len() - EOCD_MIN_SIZE;
    let mut scalar_candidates = Vec::new();
    find_eocd_candidates_scalar(&buffer, max_pos, &mut scalar_candidates);

    assert_eq!(simd_candidates, scalar_candidates);
    assert_eq!(simd_candidates, vec![900, 450, 100]);
}

#[test]
fn test_constants_invariants() {
    assert_eq!(EOCD_MIN_SIZE, 22);
    assert_eq!(MAX_COMMENT_LEN, 65535);
    assert_eq!(MAX_EOCD_SEARCH_WINDOW, 65557);
}
