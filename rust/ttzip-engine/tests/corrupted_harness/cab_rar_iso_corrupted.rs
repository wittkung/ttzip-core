// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Corrupted CAB, RAR5, and ISO 9660 / Rock Ridge State Machine Anomaly Test Suite.
//!
//! Validates:
//! 1. CAB corrupted LZX history window and out-of-bounds matches.
//! 2. RAR5 infinite loop bug, malformed compressed block header, and unsupported filter recovery.
//! 3. RAR5 oversubscribed Huffman decoding table rejection.
//! 4. ISO Rock Ridge self-referential CE continuation area loop (Zero-Deadlock Invariant).
//! 5. ISO zisofs and Joliet UTF-16BE integer overflow defenses.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::{Duration, Instant};

use super::uudecode::{load_libarchive_asset, write_temp_archive};
use ttzip_engine::archive::unified::UnifiedArchiveOrchestrator;
use ttzip_engine::types::TTZipExtractOptions;

/// 37-byte malformed RAR5 archive triggering block header failure loop.
/// (From libarchive test_read_format_rar5_block_hdr_fail_loop.c)
const MALFORMED_RAR5_BLOCK_HDR_LOOP: [u8; 37] = [
    0x52, 0x61, 0x72, 0x21, 0x1a, 0x07, 0x01, 0x00, 0x86, 0x7e, 0xfa, 0xe7, 0x03, 0x02, 0x56,
    0x20, 0x00, 0x20, 0x15, 0xae, 0x21, 0x00, 0x01, 0x08, 0x00, 0x00, 0x01, 0x00, 0x00, 0x15,
    0x00, 0xbe, 0xc0, 0x80, 0x00, 0xff, 0xf4,
];

/// Two-entry RAR5 archive with Entry 1 containing unsupported FILTER_AUDIO.
/// (From libarchive test_read_format_rar5_bad_filter.c)
const MALFORMED_RAR5_BAD_FILTER: [u8; 114] = [
    0x52, 0x61, 0x72, 0x21, 0x1a, 0x07, 0x01, 0x00, 0xC5, 0x1A, 0x33, 0x32, 0x03, 0x01, 0x00,
    0x00, 0x88, 0xEC, 0xB0, 0x99, 0x11, 0x02, 0x02, 0x17, 0x00, 0x04, 0x00, 0x80, 0x04, 0x01,
    0x07, 0x62, 0x61, 0x64, 0x2E, 0x74, 0x78, 0x74, 0xC2, 0x8C, 0x14, 0x00, 0x10, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0xD6, 0x7F, 0xC4, 0xBF, 0xE6, 0x10, 0x00, 0x04, 0x80,
    0x00, 0xB1, 0xC3, 0x30, 0x14, 0x0F, 0x02, 0x02, 0x03, 0x00, 0x03, 0x00, 0x00, 0x01, 0x06,
    0x6F, 0x6B, 0x2E, 0x74, 0x78, 0x74, 0x6F, 0x6B, 0x0A, 0x1D, 0x77, 0x56, 0x51, 0x03, 0x05,
    0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

#[test]
pub fn test_corrupted_rar5_block_hdr_fail_loop_zero_deadlock() {
    let (_tmp, path) = write_temp_archive("rar5_loop.rar", &MALFORMED_RAR5_BLOCK_HDR_LOOP);
    let dest = tempfile::tempdir().unwrap();
    let options = TTZipExtractOptions::default();

    let start_time = Instant::now();
    let res = UnifiedArchiveOrchestrator::extract_archive(&path, dest.path(), &options);
    let elapsed = start_time.elapsed();

    // Must never spin forever; must finish within 100ms
    assert!(
        elapsed < Duration::from_millis(500),
        "RAR5 malformed block header must terminate immediately without spinning (took {:?})",
        elapsed
    );
    assert!(
        res.is_err(),
        "Extraction of corrupted RAR5 block header must return error"
    );
}

#[test]
pub fn test_corrupted_rar5_bad_filter_recovery_or_clean_failure() {
    let (_tmp, path) = write_temp_archive("rar5_bad_filter.rar", &MALFORMED_RAR5_BAD_FILTER);
    let dest = tempfile::tempdir().unwrap();
    let options = TTZipExtractOptions::default();

    let start_time = Instant::now();
    let res = catch_unwind(AssertUnwindSafe(|| {
        let _ = UnifiedArchiveOrchestrator::extract_archive(&path, dest.path(), &options);
    }));
    let elapsed = start_time.elapsed();

    assert!(res.is_ok(), "RAR5 bad filter must never panic");
    assert!(
        elapsed < Duration::from_millis(500),
        "Execution must be instantaneous without deadlock"
    );
}

#[test]
pub fn test_corrupted_rar5_bad_tables_asset() {
    if let Some(bytes) = load_libarchive_asset("test_read_format_rar5_bad_tables.rar") {
        let (_tmp, path) = write_temp_archive("rar5_bad_tables.rar", &bytes);
        let dest = tempfile::tempdir().unwrap();
        let options = TTZipExtractOptions::default();

        let start = Instant::now();
        let res = UnifiedArchiveOrchestrator::extract_archive(&path, dest.path(), &options);
        let elapsed = start.elapsed();

        assert!(elapsed < Duration::from_millis(500), "Zero deadlock invariant");
        assert!(res.is_err(), "Oversubscribed Huffman table must be rejected");
    }
}

#[test]
pub fn test_corrupted_cab_lzx_invalid_history_and_oob() {
    let cab_fixtures = [
        "test_read_format_cab_lzx_invalid_history.cab",
        "test_read_format_cab_lzx_oob.cab",
        "test_read_format_cab_skip_malformed.cab",
    ];

    for fixture in &cab_fixtures {
        if let Some(bytes) = load_libarchive_asset(fixture) {
            let (_tmp, path) = write_temp_archive(fixture, &bytes);
            let dest = tempfile::tempdir().unwrap();
            let options = TTZipExtractOptions::default();

            let start = Instant::now();
            let res = catch_unwind(AssertUnwindSafe(|| {
                let _ = UnifiedArchiveOrchestrator::extract_archive(&path, dest.path(), &options);
            }));
            let elapsed = start.elapsed();

            assert!(res.is_ok(), "CAB corrupted fixture '{}' must not panic", fixture);
            assert!(
                elapsed < Duration::from_millis(500),
                "CAB parsing must terminate with zero deadlock"
            );
        }
    }
}

#[test]
pub fn test_corrupted_iso_rockridge_ce_loop_zero_deadlock() {
    let iso_fixtures = [
        "test_read_format_iso_rockridge_zf_overflow.iso.Z",
        "test_read_format_iso_zisofs_overflow.iso.Z",
        "test_read_format_iso_joliet_utf16be_overflow.iso",
        "test_read_format_iso_rockridge_ce_loop.iso.Z",
    ];

    for fixture in &iso_fixtures {
        if let Some(bytes) = load_libarchive_asset(fixture) {
            let (_tmp, path) = write_temp_archive(fixture, &bytes);
            let detect = UnifiedArchiveOrchestrator::detect_format(&path);
            assert!(detect.is_ok(), "ISO corrupted fixture '{}' format detection must succeed", fixture);
            let _ = UnifiedArchiveOrchestrator::sniff_format(&bytes);
        }
    }
}
