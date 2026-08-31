// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Corrupted 7z Header Boundary Destruction and Security Injection Test Suite.
//!
//! Covers:
//! 1. Corrupted StartHeaderCRC in 32-byte Signature Header.
//! 2. Corrupted NextHeaderCRC with valid StartHeaderCRC.
//! 3. Varint overflow attacks (9-byte 0xFF, pseudo u64::MAX sizes, out-of-bounds stream counts).
//! 4. Cyclical Coder DAG attacks (Folder BindPairs self-loops and directed cycles).
//! 5. Zip-Slip, Absolute Path, and Null-Byte path traversal injection defenses.
//! 6. Truncated EncodedHeader byte stream boundary invariants.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::time::Instant;

use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::fs::safe_extract::sanitize_and_validate_path;
use ttzip_engine::sevenz::format::*;
use ttzip_engine::sevenz::header::models::SevenZHeaderInfo;
use ttzip_engine::sevenz::header::parse_7z_header_stream;
use ttzip_engine::sevenz::writer::{build_7z_metadata_header, create_7z_solid_archive_bytes};
use ttzip_engine::sevenz::{parse_7z_metadata, SevenZArchive};
use ttzip_engine::types::{TTZipExtractOptions, TTZipStatus};
use ttzip_engine::zip::writer::ZipInputItem;

/// Helper to generate a valid baseline 7z archive for mutation testing.
fn make_valid_baseline_7z() -> Vec<u8> {
    let items = vec![
        ZipInputItem {
            rel_path: "document.txt".to_string(),
            data: b"TTZip 7z Boundary Destruction Baseline Payload".to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "assets/".to_string(),
            data: Vec::new(),
            mtime_epoch_secs: 1700000000,
            mode: 0o755,
            is_directory: true,
        },
        ZipInputItem {
            rel_path: "assets/binary.dat".to_string(),
            data: vec![0xABu8; 1024],
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
    ];

    create_7z_solid_archive_bytes(&items, 1, 2).expect("failed to create baseline 7z")
}

/// Helper to assemble a complete 7z archive from custom metadata header bytes.
fn assemble_synthetic_7z(header_bytes: &[u8], payload_bytes: &[u8]) -> Vec<u8> {
    let mut archive = Vec::new();
    let next_header_offset = payload_bytes.len() as u64;
    let next_header_size = header_bytes.len() as u64;
    let next_header_crc = crc32_fast(0, header_bytes);

    let sig = SevenZSignatureHeader {
        major_version: 0,
        minor_version: 4,
        start_header_crc: 0, // Calculated automatically in serialize()
        next_header_offset,
        next_header_size,
        next_header_crc,
    };

    archive.extend_from_slice(&sig.serialize());
    archive.extend_from_slice(payload_bytes);
    archive.extend_from_slice(header_bytes);
    archive
}

#[test]
fn test_corrupt_signature_header_crc() {
    let valid_7z = make_valid_baseline_7z();
    assert!(valid_7z.len() >= 32);

    // 1. Mutate StartHeaderCRC (bytes 8..12)
    let mut corrupt_crc = valid_7z.clone();
    corrupt_crc[8] ^= 0xFF;

    let start_instant = Instant::now();
    let parse_res = SevenZSignatureHeader::parse(&corrupt_crc);
    let elapsed = start_instant.elapsed();

    assert_eq!(
        parse_res,
        Err(TTZipStatus::ErrCorruptHeader),
        "Corrupted StartHeaderCRC must return ErrCorruptHeader"
    );
    assert!(
        elapsed.as_millis() < 5,
        "Header interception must be instantaneous (< 5ms), took {:?}",
        elapsed
    );

    let meta_res = parse_7z_metadata(&corrupt_crc, None);
    assert_eq!(meta_res.err(), Some(TTZipStatus::ErrCorruptHeader));

    let open_res = SevenZArchive::open_slice(&corrupt_crc);
    assert!(
        open_res.is_err(),
        "SevenZArchive::open_slice must fail on corrupt StartHeaderCRC"
    );

    // 2. Truncate Signature Header to < 32 bytes
    for len in 0..32 {
        let truncated = &valid_7z[..len];
        assert_eq!(
            SevenZSignatureHeader::parse(truncated),
            Err(TTZipStatus::ErrCorruptHeader)
        );
        assert_eq!(
            parse_7z_metadata(truncated, None).err(),
            Some(TTZipStatus::ErrCorruptHeader)
        );
    }

    // 3. Corrupt Magic Bytes (bytes 0..6)
    let mut corrupt_magic = valid_7z.clone();
    corrupt_magic[0] = 0x00;
    assert_eq!(
        SevenZSignatureHeader::parse(&corrupt_magic),
        Err(TTZipStatus::ErrCorruptHeader)
    );
}

#[test]
fn test_corrupt_next_header_crc() {
    let valid_7z = make_valid_baseline_7z();
    assert!(valid_7z.len() >= 32);

    // 1. Invert NextHeaderCRC field in Signature Header and recalculate StartHeaderCRC
    // so StartHeaderCRC is 100% valid, but NextHeaderCRC mismatches actual header.
    let mut corrupt_next_crc = valid_7z.clone();
    corrupt_next_crc[28] ^= 0x5A; // Flip bits in next_header_crc

    // Recalculate valid StartHeaderCRC over bytes 12..32
    let updated_start_crc = crc32_fast(0, &corrupt_next_crc[12..32]);
    corrupt_next_crc[8..12].copy_from_slice(&updated_start_crc.to_le_bytes());

    // Verify StartHeader passes, but NextHeaderCRC fails safely before parsing
    assert!(SevenZSignatureHeader::parse(&corrupt_next_crc).is_ok());

    let meta_res = parse_7z_metadata(&corrupt_next_crc, None);
    assert_eq!(
        meta_res.err(),
        Some(TTZipStatus::ErrCorruptHeader),
        "NextHeaderCRC mismatch must be intercepted before decompression"
    );

    let open_res = SevenZArchive::open_slice(&corrupt_next_crc);
    assert!(open_res.is_err());

    // 2. Mutate metadata header payload while keeping NextHeaderCRC unchanged
    let mut corrupt_body = valid_7z.clone();
    let body_len = corrupt_body.len();
    if body_len > 32 {
        corrupt_body[body_len - 1] ^= 0xFF; // Corrupt last byte of header stream
        let meta_res2 = parse_7z_metadata(&corrupt_body, None);
        assert_eq!(
            meta_res2.err(),
            Some(TTZipStatus::ErrCorruptHeader),
            "Tampered metadata body must fail NextHeaderCRC"
        );
    }
}

#[test]
fn test_varint_overflow_attack() {
    // 1. Extreme 9-byte Varint with all bits set (0xFF) -> returns u64::MAX safely
    let max_varint_bytes = [0xFFu8; 9];
    let (val, consumed) = read_varint(&max_varint_bytes).expect("read_varint failed");
    assert_eq!(val, u64::MAX);
    assert_eq!(consumed, 9);

    // 2. Incomplete 9-byte Varint sequence -> returns None without panic
    let truncated_varint = [0xFFu8; 5];
    assert!(read_varint(&truncated_varint).is_none());

    // 3. Synthetic Header with num_pack_streams = u64::MAX
    let mut header_overflow_pack = Vec::new();
    header_overflow_pack.push(K_HEADER);
    header_overflow_pack.push(K_MAIN_STREAMS_INFO);
    header_overflow_pack.push(K_PACK_INFO);
    write_varint(0, &mut header_overflow_pack); // packPos
    write_varint(u64::MAX, &mut header_overflow_pack); // numPackStreams = u64::MAX
    header_overflow_pack.push(K_SIZE);
    write_varint(1024, &mut header_overflow_pack);
    header_overflow_pack.push(K_END);
    header_overflow_pack.push(K_END);

    let mut info1 = SevenZHeaderInfo::default();
    let res1 = parse_7z_header_stream(&header_overflow_pack, &mut info1);
    assert_eq!(
        res1,
        Err(TTZipStatus::ErrCorruptHeader),
        "Oversized num_pack_streams must fail safely"
    );

    // 4. Synthetic Header with num_folders = u64::MAX
    let mut header_overflow_folders = vec![K_HEADER, K_MAIN_STREAMS_INFO, K_UNPACK_INFO, K_FOLDER];
    write_varint(u64::MAX, &mut header_overflow_folders); // numFolders = u64::MAX
    header_overflow_folders.push(0); // external = 0
    header_overflow_folders.push(K_END);
    header_overflow_folders.push(K_END);

    let mut info2 = SevenZHeaderInfo::default();
    let res2 = parse_7z_header_stream(&header_overflow_folders, &mut info2);
    assert_eq!(
        res2,
        Err(TTZipStatus::ErrCorruptHeader),
        "Oversized num_folders must fail safely"
    );

    // 5. Synthetic Header with num_files = u64::MAX
    let mut header_overflow_files = vec![K_HEADER, K_FILES_INFO];
    write_varint(u64::MAX, &mut header_overflow_files); // numFiles = u64::MAX
    header_overflow_files.push(K_END);

    let mut info3 = SevenZHeaderInfo::default();
    let res3 = parse_7z_header_stream(&header_overflow_files, &mut info3);
    assert_eq!(
        res3,
        Err(TTZipStatus::ErrCorruptHeader),
        "Oversized num_files must fail safely without OOM"
    );

    // 6. Assemble synthetic archive with u64::MAX pack_pos and verify open_slice
    let synthetic_archive = assemble_synthetic_7z(&header_overflow_pack, &[]);
    let open_res = SevenZArchive::open_slice(&synthetic_archive);
    assert!(open_res.is_err());
}

#[test]
fn test_cyclical_coder_dag_attack() {
    // 1. Attack Case A: Self-Loop (InCoder == OutCoder, InStream 0 -> OutStream 0 on Coder 0)
    let mut header_self_loop = vec![K_HEADER, K_MAIN_STREAMS_INFO, K_UNPACK_INFO, K_FOLDER];
    write_varint(1, &mut header_self_loop); // numFolders = 1
    header_self_loop.push(0); // external = 0
    write_varint(2, &mut header_self_loop); // numCoders = 2

    // Coder 0: 1 In, 1 Out (Copy)
    header_self_loop.push(0x01); // method size = 1
    header_self_loop.push(0x00); // METHOD_COPY

    // Coder 1: 1 In, 1 Out (Copy)
    header_self_loop.push(0x01);
    header_self_loop.push(0x00);

    // BindPair: InStream 0 (Coder 0), OutStream 0 (Coder 0) -> SELF LOOP!
    write_varint(0, &mut header_self_loop);
    write_varint(0, &mut header_self_loop);

    header_self_loop.push(K_END);
    header_self_loop.push(K_END);

    let mut info_self_loop = SevenZHeaderInfo::default();
    let res_self_loop = parse_7z_header_stream(&header_self_loop, &mut info_self_loop);
    assert_eq!(
        res_self_loop,
        Err(TTZipStatus::ErrCorruptHeader),
        "Self-loop BindPair (In == Out) must be rejected immediately"
    );

    // 2. Attack Case B: Directed 2-Cycle (Coder 0 -> Coder 1 -> Coder 0)
    // 3 Coders: Coder 0 (1 in, 1 out), Coder 1 (1 in, 1 out), Coder 2 (1 in, 1 out)
    // Bind pairs (2 pairs):
    // Pair 1: InStream 1 (Coder 1) <- OutStream 0 (Coder 0) => Edge 0 -> 1
    // Pair 2: InStream 0 (Coder 0) <- OutStream 1 (Coder 1) => Edge 1 -> 0 (Cycle!)
    let mut header_cycle = vec![K_HEADER, K_MAIN_STREAMS_INFO, K_UNPACK_INFO, K_FOLDER];
    write_varint(1, &mut header_cycle); // numFolders = 1
    header_cycle.push(0); // external = 0
    write_varint(3, &mut header_cycle); // numCoders = 3

    // Coder 0
    header_cycle.push(0x01);
    header_cycle.push(0x00);
    // Coder 1
    header_cycle.push(0x01);
    header_cycle.push(0x00);
    // Coder 2
    header_cycle.push(0x01);
    header_cycle.push(0x00);

    // BindPair 1: in_idx = 1 (Coder 1), out_idx = 0 (Coder 0)
    write_varint(1, &mut header_cycle);
    write_varint(0, &mut header_cycle);

    // BindPair 2: in_idx = 0 (Coder 0), out_idx = 1 (Coder 1) [Cycle!]
    write_varint(0, &mut header_cycle);
    write_varint(1, &mut header_cycle);

    header_cycle.push(K_END);
    header_cycle.push(K_END);

    let start_instant = Instant::now();
    let mut info_cycle = SevenZHeaderInfo::default();
    let res_cycle = parse_7z_header_stream(&header_cycle, &mut info_cycle);
    let elapsed = start_instant.elapsed();

    assert_eq!(
        res_cycle,
        Err(TTZipStatus::ErrCorruptHeader),
        "Cyclical Coder DAG must be detected and rejected via topological sort"
    );
    assert!(
        elapsed.as_millis() < 5,
        "Cycle detection must complete in < 5ms (0ms nominal), took {:?}",
        elapsed
    );

    // 3. Attack Case C: Out-of-bounds Stream Indices in BindPairs
    let mut header_oob = vec![K_HEADER, K_MAIN_STREAMS_INFO, K_UNPACK_INFO, K_FOLDER];
    write_varint(1, &mut header_oob);
    header_oob.push(0);
    write_varint(2, &mut header_oob); // 2 coders

    // Coder 0: 1 in, 1 out
    header_oob.push(0x01);
    header_oob.push(0x00);
    // Coder 1: 1 in, 1 out
    header_oob.push(0x01);
    header_oob.push(0x00);

    // Out-of-bounds in_idx = 999, out_idx = 999
    write_varint(999, &mut header_oob);
    write_varint(999, &mut header_oob);
    header_oob.push(K_END);
    header_oob.push(K_END);

    let mut info_oob = SevenZHeaderInfo::default();
    let res_oob = parse_7z_header_stream(&header_oob, &mut info_oob);
    assert_eq!(res_oob, Err(TTZipStatus::ErrCorruptHeader));
}

#[test]
fn test_zip_slip_and_null_byte_path_traversal() {
    let temp_dir = tempfile::tempdir().expect("create temp dir failed");
    let dest_path = temp_dir.path();

    // 1. Direct unit assertions on sanitize_and_validate_path
    let malicious_paths = [
        "../../../../etc/shadow",
        "../escape.txt",
        "/etc/passwd",
        "/private/var/log/system.log",
        "C:\\Windows\\System32\\cmd.exe",
        "\\\\server\\share\\malware.exe",
        "foo/bar/../../../../root/.bashrc",
        "folder/evil\0bypass.txt",
        "....//etc//hosts",
        "http://evil.com/payload.sh",
        "",
    ];

    for &bad_path in &malicious_paths {
        let res = sanitize_and_validate_path(dest_path, bad_path);
        assert_eq!(
            res.err(),
            Some(TTZipStatus::ErrSecurityViolation),
            "Path '{}' must be rejected with ErrSecurityViolation",
            bad_path
        );
    }

    // 2. Integration: Construct 7z archive with Zip-Slip paths and test extraction
    let malicious_items = vec![
        ZipInputItem {
            rel_path: "../../../../tmp/pwned_7z_shadow.txt".to_string(),
            data: b"Hacked by Zip-Slip".to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "/etc/pwned_7z_passwd".to_string(),
            data: b"Root escalation attempt".to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
    ];

    // Build raw metadata header directly with these filenames
    let header_bytes = build_7z_metadata_header(
        &malicious_items,
        &[18, 23],
        &[0x12345678, 0x87654321],
        41,
        41,
        METHOD_COPY,
        &[],
    );

    let synthetic_archive = assemble_synthetic_7z(&header_bytes, &[0x41u8; 41]);
    let archive = SevenZArchive::open_slice(&synthetic_archive).expect("metadata open failed");
    assert_eq!(archive.len(), 2);

    // Assert extract_all intercepts path traversal securely
    let extract_opts = TTZipExtractOptions::default();
    let extract_res = archive.extract_all(dest_path, &extract_opts);
    assert_eq!(
        extract_res.err(),
        Some(TTZipStatus::ErrSecurityViolation),
        "extract_all must refuse to write Zip-Slip files"
    );

    // Verify no files escaped sandbox to /tmp
    let escaped_file = Path::new("/tmp/pwned_7z_shadow.txt");
    assert!(
        !escaped_file.exists(),
        "Escaped file must NOT exist on filesystem!"
    );
}

#[test]
fn test_truncated_encoded_header() {
    let valid_7z = make_valid_baseline_7z();
    assert!(valid_7z.len() > 32);

    // 1. Truncate baseline archive at every single byte boundary (0..len)
    for cutoff in 0..valid_7z.len() {
        let truncated = &valid_7z[..cutoff];
        let catch_res = catch_unwind(AssertUnwindSafe(|| {
            let _ = parse_7z_metadata(truncated, None);
            let _ = SevenZArchive::open_slice(truncated);
        }));

        assert!(
            catch_res.is_ok(),
            "Parser must never panic on truncated input at byte offset {}",
            cutoff
        );

        let meta_res = parse_7z_metadata(truncated, None);
        assert!(
            meta_res.is_err(),
            "Truncated stream at cutoff {} must return error",
            cutoff
        );
    }

    // 2. Construct synthetic EncodedHeader (tag 0x17) and truncate at various internal boundaries
    let mut encoded_header_payload = Vec::new();
    encoded_header_payload.push(K_ENCODED_HEADER);
    encoded_header_payload.push(K_PACK_INFO);
    write_varint(0, &mut encoded_header_payload); // packPos
    write_varint(1, &mut encoded_header_payload); // numPackStreams
    encoded_header_payload.push(K_SIZE);
    write_varint(128, &mut encoded_header_payload);
    encoded_header_payload.push(K_END);
    encoded_header_payload.push(K_UNPACK_INFO);
    encoded_header_payload.push(K_FOLDER);
    write_varint(1, &mut encoded_header_payload);
    encoded_header_payload.push(0);
    write_varint(1, &mut encoded_header_payload);
    encoded_header_payload.push(0x01); // method size = 1
    encoded_header_payload.push(0x00); // METHOD_COPY
    encoded_header_payload.push(K_CODERS_UNPACK_SIZE);
    write_varint(256, &mut encoded_header_payload);
    encoded_header_payload.push(K_END);
    encoded_header_payload.push(K_END);

    let synthetic_archive = assemble_synthetic_7z(&encoded_header_payload, &[0xAAu8; 128]);

    for cutoff in 32..synthetic_archive.len() {
        let truncated = &synthetic_archive[..cutoff];
        let res = parse_7z_metadata(truncated, None);
        assert!(
            res.is_err(),
            "Truncated EncodedHeader at cutoff {} must fail defensively",
            cutoff
        );
    }
}
