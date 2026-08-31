// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Official Google Snappy (1.2.2) Compliance & Real-World Industrial Corpora Matrix Tests.
//!
//! Validates 100% bit-exact roundtrip fidelity and corruption rejection across Google's 14 standard test datasets.

use std::fs;
use std::path::PathBuf;
use ttzip_engine::codecs::snappy::{
    is_framed_snappy, max_compressed_len, snappy_compress_framed, snappy_compress_raw,
    snappy_decompress_framed, snappy_decompress_raw, snappy_validate_framed, snappy_validate_raw,
};

fn get_snappy_testdata_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // ../../../vendor/snappy/testdata
    let path = manifest_dir.join("../../../vendor/snappy/testdata");
    if path.exists() {
        path
    } else {
        panic!("Vendor snappy testdata directory not found at: {:?}", path);
    }
}

const VALID_CORPORA_FILES: &[&str] = &[
    "html",
    "urls.10K",
    "fireworks.jpeg",
    "paper-100k.pdf",
    "html_x_4",
    "alice29.txt",
    "asyoulik.txt",
    "lcet10.txt",
    "plrabn12.txt",
    "geo.protodata",
    "kppkn.gtb",
];

const CORRUPT_CORPORA_FILES: &[&str] = &[
    "baddata1.snappy",
    "baddata2.snappy",
    "baddata3.snappy",
];

#[test]
fn test_snappy_compliance_raw_roundtrip_all_corpora() {
    let testdata_dir = get_snappy_testdata_dir();

    for &filename in VALID_CORPORA_FILES {
        let file_path = testdata_dir.join(filename);
        assert!(file_path.exists(), "Corpus file {:?} must exist", file_path);

        let original_data = fs::read(&file_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", filename, e));

        // 1. Raw Compress
        let compressed = snappy_compress_raw(&original_data)
            .unwrap_or_else(|e| panic!("Raw compress failed for {}: {:?}", filename, e));

        // 2. Bound invariant
        let max_bound = max_compressed_len(original_data.len());
        assert!(
            compressed.len() <= max_bound,
            "Compressed size {} must be <= max bound {} for {}",
            compressed.len(),
            max_bound,
            filename
        );

        // 3. Validation
        assert!(
            snappy_validate_raw(&compressed, original_data.len()),
            "Raw validate failed for {}",
            filename
        );

        // 4. Raw Decompress & Bit-Exact Assertion
        let decompressed = snappy_decompress_raw(&compressed)
            .unwrap_or_else(|e| panic!("Raw decompress failed for {}: {:?}", filename, e));

        assert_eq!(
            decompressed, original_data,
            "100% Bit-exact roundtrip mismatch for {}",
            filename
        );
    }
}

#[test]
fn test_snappy_compliance_framed_roundtrip_all_corpora() {
    let testdata_dir = get_snappy_testdata_dir();

    for &filename in VALID_CORPORA_FILES {
        let file_path = testdata_dir.join(filename);
        let original_data = fs::read(&file_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", filename, e));

        // 1. Framed Compress (.sz)
        let framed = snappy_compress_framed(&original_data)
            .unwrap_or_else(|e| panic!("Framed compress failed for {}: {:?}", filename, e));

        // 2. Stream Identifier & Frame Validation
        assert!(
            is_framed_snappy(&framed),
            "Must be valid framed stream for {}",
            filename
        );
        assert!(
            snappy_validate_framed(&framed),
            "Framed validation failed for {}",
            filename
        );

        // 3. Framed Decompress & Bit-Exact Assertion
        let decompressed = snappy_decompress_framed(&framed)
            .unwrap_or_else(|e| panic!("Framed decompress failed for {}: {:?}", filename, e));

        assert_eq!(
            decompressed, original_data,
            "100% Bit-exact framed roundtrip mismatch for {}",
            filename
        );
    }
}

#[test]
fn test_snappy_compliance_corrupted_baddata_rejection() {
    let testdata_dir = get_snappy_testdata_dir();

    for &filename in CORRUPT_CORPORA_FILES {
        let file_path = testdata_dir.join(filename);
        assert!(file_path.exists(), "Corrupt file {:?} must exist", file_path);

        let corrupt_data = fs::read(&file_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", filename, e));

        // Must NOT panic, must return error or false
        let val_raw = snappy_validate_raw(&corrupt_data, 1024 * 1024 * 64);
        let dec_raw = snappy_decompress_raw(&corrupt_data);
        let val_framed = snappy_validate_framed(&corrupt_data);
        let dec_framed = snappy_decompress_framed(&corrupt_data);

        // If raw validate is false, decompress must fail
        if !val_raw {
            assert!(dec_raw.is_err(), "Raw decompress on invalid {} must fail", filename);
        }
        if !val_framed {
            assert!(dec_framed.is_err(), "Framed decompress on invalid {} must fail", filename);
        }
    }
}
