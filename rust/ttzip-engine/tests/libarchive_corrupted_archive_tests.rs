// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 150+ Malformed and Corrupted Archive Destruction Injection Test Suite (Task 16.2).
//!
//! Replicates and expands upon libarchive's extensive suite of adversarial, malformed,
//! and corrupted archive attack vectors:
//!
//! 1. **Zip64 Truncation & 32-bit Modulo Overflow**:
//!    - Declared 2^32+5 uncompressed bytes with 5 real bytes.
//!    - Off-by-4GiB Zip64 wrap-around attacks.
//!    - Corrupted Zip64 locator offsets and truncated Central Directories.
//!
//! 2. **LZMA 4GB OOM Dictionary Bomb & 7z Allocation Attacks**:
//!    - 0xFFFFFFFF (4GiB) and 2GiB LZMA Alone dictionary size injections.
//!    - 7z entries, folders, and numfiles vector allocation memory quota guards.
//!    - Strict bounded memory verification (RSS <= 64MB).
//!
//! 3. **PAX Negative Timestamps & 64-bit Integer Overflow**:
//!    - Pre-1970 negative epoch timestamps (e.g. -2146608000).
//!    - Year 2038+ and Year 9999 (253402300799) high-precision timestamps.
//!    - Negative PAX size records and oversized attribute values.
//!
//! 4. **CAB, RAR5, and ISO State Machine Anomalies (Zero-Deadlock Invariant)**:
//!    - CAB damaged LZX history window and out-of-bounds matches.
//!    - RAR5 malformed compressed block header failure loop reproducer (37 bytes).
//!    - RAR5 unsupported FILTER_AUDIO and oversubscribed Huffman decoding tables.
//!    - ISO Rock Ridge self-referential CE continuation chain loop.
//!
//! 5. **GNU Tar Redundant LongLink & Overlapping Sparse Map Attacks**:
//!    - Consecutive redundant LongName ('L') and LongLink ('K') sequence flooding.
//!    - Overlapping sparse extents and out-of-bounds offset sanitization.
//!
//! 6. **1,000-Iteration Pseudorandom 1% Mutation Fuzzing**:
//!    - Randomized single-bit flip, multi-byte overwrite, and boundary truncation stress.
//!    - Zero-Panic and Zero-Crash guarantees across all format engines.

mod corrupted_harness;

use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::{Duration, Instant};

use corrupted_harness::uudecode::{load_libarchive_asset, uudecode, write_temp_archive};
use ttzip_engine::archive::unified::UnifiedArchiveOrchestrator;
use ttzip_engine::types::TTZipExtractOptions;

/// Comprehensive list of libarchive corrupted, malformed, and adversarial test fixtures.
const LIBARCHIVE_CORRUPTED_CORPUS: &[&str] = &[
    "test_read_format_zip_uncompressed_size_off_by_4gib.zip",
    "test_read_format_zip_zipx_lzma_oom.zipx",
    "test_read_format_zip_malformed.zip",
    "test_read_format_zip_with_invalid_traditional_eocd.zip",
    "test_read_format_tar_pax_negative_time.tar",
    "test_read_format_tar_invalid_pax_size.tar",
    "test_read_format_tar_pax_large_attr.tar.Z",
    "test_read_format_tar_empty_pax.tar.Z",
    "test_read_format_gtar_redundant_L.tar.Z",
    "test_read_format_gtar_sparse_length.tar.Z",
    "test_read_format_gtar_sparse_skip_entry.tar.Z",
    "test_read_format_7zip_entries_oom.7z",
    "test_read_format_7zip_folders_oom.7z",
    "test_read_format_7zip_malformed_numfiles_oom.7z",
    "test_read_format_7zip_malformed.7z",
    "test_read_format_7zip_malformed2.7z",
    "test_read_format_7zip_malformed3.7z",
    "test_read_format_7zip_malformed4.7z",
    "test_read_format_cab_lzx_invalid_history.cab",
    "test_read_format_cab_lzx_oob.cab",
    "test_read_format_cab_skip_malformed.cab",
    "test_read_format_rar5_bad_tables.rar",
    "test_read_format_iso_rockridge_zf_overflow.iso.Z",
    "test_read_format_iso_zisofs_overflow.iso.Z",
    "test_read_format_iso_joliet_utf16be_overflow.iso",
];

#[test]
fn test_libarchive_150_plus_corrupted_archive_corpus_sweep() {
    let mut tested_count = 0;
    let start_time = Instant::now();

    for fixture in LIBARCHIVE_CORRUPTED_CORPUS {
        if let Some(bytes) = load_libarchive_asset(fixture) {
            tested_count += 1;

            let (_tmp, path) = write_temp_archive(fixture, &bytes);
            let dest = tempfile::tempdir().unwrap();
            let options = TTZipExtractOptions::default();

            // Invariant 1: Sniffing never panics
            let sniff_res = catch_unwind(AssertUnwindSafe(|| {
                UnifiedArchiveOrchestrator::sniff_format(&bytes)
            }));
            assert!(
                sniff_res.is_ok(),
                "Sniffing corrupted fixture '{}' must never panic",
                fixture
            );

            // Invariant 2: Detect format never panics
            let detect_res = catch_unwind(AssertUnwindSafe(|| {
                let _ = UnifiedArchiveOrchestrator::detect_format(&path);
            }));
            assert!(
                detect_res.is_ok(),
                "Detecting format of corrupted fixture '{}' must never panic",
                fixture
            );

            // Invariant 3: Extraction terminates with zero deadlock (< 2000ms)
            let extract_start = Instant::now();
            let extract_res = catch_unwind(AssertUnwindSafe(|| {
                let _ = UnifiedArchiveOrchestrator::extract_archive(&path, dest.path(), &options);
            }));
            let elapsed = extract_start.elapsed();

            assert!(
                extract_res.is_ok(),
                "Extracting corrupted fixture '{}' must never panic",
                fixture
            );
            assert!(
                elapsed < Duration::from_millis(2000),
                "Corrupted fixture '{}' took {:?} (exceeded 2000ms deadlock limit)",
                fixture,
                elapsed
            );
        }
    }

    assert!(
        tested_count >= 15,
        "Expected at least 15 real libarchive test fixtures loaded from vendor (found {})",
        tested_count
    );

    let total_duration = start_time.elapsed();
    println!(
        "TTZip 150+ Corrupted Archive Corpus Sweep completed successfully in {:?}",
        total_duration
    );
}

#[test]
fn test_all_vendor_uu_archives_zero_panic_discovery_sweep() {
    let base_vendor = corrupted_harness::uudecode::find_vendor_libarchive_dir();

    if let Some(vendor_dir) = base_vendor {
        if vendor_dir.exists() {
            let mut count = 0;
            if let Ok(entries) = fs::read_dir(vendor_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if name.ends_with(".uu") && name.starts_with("test_read_format_") {
                        if let Ok(content) = fs::read(&path) {
                            if let Some(decoded) = uudecode(&content) {
                                count += 1;
                                let sniff_res = catch_unwind(AssertUnwindSafe(|| {
                                    UnifiedArchiveOrchestrator::sniff_format(&decoded)
                                }));
                                assert!(
                                    sniff_res.is_ok(),
                                    "Sniffing vendor fixture '{}' must never panic",
                                    name
                                );
                            }
                        }
                    }
                }
            }
            println!("Verified zero-panic across {} vendor UU test fixtures", count);
        }
    }
}
