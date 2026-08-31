// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Special Container Formats Matrix (WARC 1.0/1.1, CAB, LHA -lh5-, MTREE).

use super::{read_archive_buffer, write_archive_buffer, SyntheticEntry};
use ttzip_engine::ffi::archive_ffi::sys::*;
use ttzip_engine::standards::signatures::DetectedFormat;
use ttzip_engine::standards::sniffer::detect_format_buffer;

/// 1. WARC 1.0 / 1.1 Web Archive format matrix.
pub fn run_warc_matrix_test() {
    let html_payload = b"<html><body><h1>TTZip WARC 2026</h1></body></html>".to_vec();
    let js_payload = b"console.log('TTZip high-speed web archive engine');".to_vec();

    let entries = vec![
        SyntheticEntry::file("http://ttzip.dev/index.html", html_payload.clone())
            .with_perm(0o644)
            .with_mtime(1_700_000_000, 0),
        SyntheticEntry::file("http://ttzip.dev/assets/app.js", js_payload.clone())
            .with_perm(0o644)
            .with_mtime(1_700_000_100, 0),
    ];

    let bytes = write_archive_buffer(&entries, |a| unsafe {
        let rc = archive_write_set_format_warc(a);
        if rc != 0 {
            Err("archive_write_set_format_warc failed".to_string())
        } else {
            Ok(())
        }
    })
    .expect("Failed to write WARC archive");

    assert!(!bytes.is_empty());
    // Verify WARC record magic header "WARC/1.0"
    assert!(bytes.starts_with(b"WARC/1.0") || bytes.starts_with(b"WARC/1.1"), "WARC must start with WARC header");

    let extracted = read_archive_buffer(&bytes).expect("Failed to read WARC archive");
    assert_eq!(extracted.len(), entries.len());
    assert!(extracted[0].path.contains("index.html"));
    assert!(!extracted[0].data.is_empty());
    assert!(extracted[1].path.contains("app.js"));
    assert!(!extracted[1].data.is_empty());
}

/// 2. MTREE (BSD Directory Hierarchy Specification Manifest) matrix.
pub fn run_mtree_matrix_test() {
    let entries = vec![
        SyntheticEntry::file("etc/ttzip.conf", b"threads = 8\ncompression_level = 6\n".to_vec())
            .with_perm(0o644)
            .with_mtime(1_680_000_000, 0),
        SyntheticEntry::file("usr/bin/ttzip", vec![0x7F, 0x45, 0x4C, 0x46, 0x02, 0x01, 0x01])
            .with_perm(0o755)
            .with_mtime(1_680_000_100, 0),
        SyntheticEntry::symlink("usr/bin/ttunzip", "ttzip")
            .with_mtime(1_680_000_200, 0),
    ];

    let bytes = write_archive_buffer(&entries, |a| unsafe {
        let rc = archive_write_set_format_mtree(a);
        if rc != 0 {
            Err("archive_write_set_format_mtree failed".to_string())
        } else {
            Ok(())
        }
    })
    .expect("Failed to write MTREE manifest");

    assert!(!bytes.is_empty());
    let manifest_text = String::from_utf8_lossy(&bytes);
    // MTREE must contain #mtree header signature
    assert!(manifest_text.contains("#mtree"), "MTREE manifest must contain #mtree");
    assert!(manifest_text.contains("type=file"), "MTREE manifest must contain file entries");
}

/// 3. CAB (Microsoft Cabinet Format) Sniffing & Boundary Validation.
pub fn run_cab_boundary_matrix_test() {
    // Construct synthetic CAB header (MSCF signature + 36 header bytes)
    let mut cab_data = vec![0u8; 128];
    cab_data[0..4].copy_from_slice(b"MSCF"); // signature
    cab_data[4..8].copy_from_slice(&0u32.to_le_bytes()); // cbCabinet reserved
    let total_len = cab_data.len() as u32;
    cab_data[8..12].copy_from_slice(&total_len.to_le_bytes()); // total size
    cab_data[16..20].copy_from_slice(&36u32.to_le_bytes()); // coffFiles
    cab_data[24..26].copy_from_slice(&0x0103u16.to_le_bytes()); // version 1.3
    cab_data[26..28].copy_from_slice(&1u16.to_le_bytes()); // cFolders
    cab_data[28..29].copy_from_slice(&1u8.to_le_bytes()); // cFiles

    let detected = detect_format_buffer(&cab_data, None);
    assert_eq!(detected.format, DetectedFormat::Cab, "CAB header must be identified");
}

/// 4. LHA / LZH (-lh5-) Sniffing & Boundary Validation.
pub fn run_lha_boundary_matrix_test() {
    // Construct synthetic LHA header with Level 1 -lh5- method signature
    let mut lha_data = vec![0u8; 128];
    lha_data[0] = 50; // header size
    lha_data[1] = 0x20; // header checksum
    lha_data[2..7].copy_from_slice(b"-lh5-"); // method ID
    lha_data[7..11].copy_from_slice(&64u32.to_le_bytes()); // packed size
    lha_data[11..15].copy_from_slice(&64u32.to_le_bytes()); // original size

    let detected = detect_format_buffer(&lha_data, None);
    assert_eq!(detected.format, DetectedFormat::Lzh, "LHA -lh5- stream must be identified");
}
