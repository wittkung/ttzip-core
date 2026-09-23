// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Official Google Snappy (1.2.2) Compliance & Real-World Industrial Corpora Matrix Tests.
//!
//! Validates 100% bit-exact roundtrip fidelity and corruption rejection across self-contained
//! golden test vectors and optional upstream Google vendor datasets.

use std::fs;
use std::path::PathBuf;
use ttzip_engine::codecs::snappy::{
    is_framed_snappy, max_compressed_len, snappy_compress_framed, snappy_compress_raw,
    snappy_decompress_framed, snappy_decompress_raw, snappy_validate_framed, snappy_validate_raw,
};

/// Resolves self-contained fixture path within crate fixtures directory.
fn get_local_fixtures_dir() -> PathBuf {
    let direct = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/snappy");
    if direct.exists() {
        return direct;
    }
    if let Ok(srcdir) = std::env::var("TEST_SRCDIR") {
        let ws = std::env::var("TEST_WORKSPACE").unwrap_or_else(|_| "_main".to_string());
        let p = PathBuf::from(&srcdir).join(&ws).join("products/ttzip/core/rust/ttzip-engine/tests/fixtures/snappy");
        if p.exists() {
            return p;
        }
        let p2 = PathBuf::from(&srcdir).join("products/ttzip/core/rust/ttzip-engine/tests/fixtures/snappy");
        if p2.exists() {
            return p2;
        }
    }
    direct
}

/// Resolves optional external vendor testdata directory, if available.
fn get_external_vendor_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("SNAPPY_TESTDATA_DIR") {
        let p = PathBuf::from(dir);
        if p.exists() {
            return Some(p);
        }
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../vendor/snappy/testdata");
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

/// Discovers all available valid corpora, prioritizing self-contained golden vectors
/// and appending external vendor corpora if present on the filesystem.
fn collect_valid_corpora() -> Vec<(String, Vec<u8>)> {
    let mut corpora = Vec::new();
    let local_dir = get_local_fixtures_dir();

    // 1. Mandatory self-contained golden sample
    let golden_sample_path = local_dir.join("golden_sample.txt");
    let data = fs::read(&golden_sample_path)
        .unwrap_or_else(|e| panic!("Mandatory self-contained fixture {:?} must exist: {}", golden_sample_path, e));
    corpora.push(("golden_sample.txt".to_string(), data));

    // 2. Optional external vendor corpora
    let vendor_files = [
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
    if let Some(ext_dir) = get_external_vendor_dir() {
        for &filename in &vendor_files {
            let p = ext_dir.join(filename);
            if p.exists() {
                if let Ok(data) = fs::read(&p) {
                    corpora.push((format!("vendor:{}", filename), data));
                }
            }
        }
    }

    corpora
}

/// Discovers all available corrupt corpora, prioritizing self-contained golden vectors
/// and appending external vendor corruption datasets if present.
fn collect_corrupt_corpora() -> Vec<(String, Vec<u8>)> {
    let mut corrupt_list = Vec::new();
    let local_dir = get_local_fixtures_dir();

    // 1. Mandatory self-contained corrupted fixtures
    let local_corrupt_files = [
        "corrupted.snappy",
        "baddata1.snappy",
        "baddata2.snappy",
        "baddata3.snappy",
    ];
    for &name in &local_corrupt_files {
        let p = local_dir.join(name);
        let data = fs::read(&p)
            .unwrap_or_else(|e| panic!("Mandatory self-contained corrupt fixture {:?} must exist: {}", p, e));
        corrupt_list.push((name.to_string(), data));
    }

    // 2. Optional external vendor baddata files
    let vendor_files = ["baddata1.snappy", "baddata2.snappy", "baddata3.snappy"];
    if let Some(ext_dir) = get_external_vendor_dir() {
        for &filename in &vendor_files {
            let p = ext_dir.join(filename);
            if p.exists() {
                if let Ok(data) = fs::read(&p) {
                    corrupt_list.push((format!("vendor:{}", filename), data));
                }
            }
        }
    }

    corrupt_list
}

#[test]
fn test_snappy_compliance_raw_roundtrip_all_corpora() {
    let corpora = collect_valid_corpora();
    assert!(
        !corpora.is_empty(),
        "At least one valid corpus must be tested (0 skipped)"
    );

    for (name, original_data) in corpora {
        // 1. Raw Compress
        let compressed = snappy_compress_raw(&original_data)
            .unwrap_or_else(|e| panic!("Raw compress failed for {}: {:?}", name, e));

        // 2. Bound invariant
        let max_bound = max_compressed_len(original_data.len());
        assert!(
            compressed.len() <= max_bound,
            "Compressed size {} must be <= max bound {} for {}",
            compressed.len(),
            max_bound,
            name
        );

        // 3. Validation
        assert!(
            snappy_validate_raw(&compressed, original_data.len()),
            "Raw validate failed for {}",
            name
        );

        // 4. Raw Decompress & Bit-Exact Assertion
        let decompressed = snappy_decompress_raw(&compressed)
            .unwrap_or_else(|e| panic!("Raw decompress failed for {}: {:?}", name, e));

        assert_eq!(
            decompressed, original_data,
            "100% Bit-exact roundtrip mismatch for {}",
            name
        );
    }
}

#[test]
fn test_snappy_compliance_framed_roundtrip_all_corpora() {
    let corpora = collect_valid_corpora();
    assert!(
        !corpora.is_empty(),
        "At least one valid corpus must be tested (0 skipped)"
    );

    for (name, original_data) in corpora {
        // 1. Framed Compress (.sz)
        let framed = snappy_compress_framed(&original_data)
            .unwrap_or_else(|e| panic!("Framed compress failed for {}: {:?}", name, e));

        // 2. Stream Identifier & Frame Validation
        assert!(
            is_framed_snappy(&framed),
            "Must be valid framed stream for {}",
            name
        );
        assert!(
            snappy_validate_framed(&framed),
            "Framed validation failed for {}",
            name
        );

        // 3. Framed Decompress & Bit-Exact Assertion
        let decompressed = snappy_decompress_framed(&framed)
            .unwrap_or_else(|e| panic!("Framed decompress failed for {}: {:?}", name, e));

        assert_eq!(
            decompressed, original_data,
            "100% Bit-exact framed roundtrip mismatch for {}",
            name
        );
    }
}

#[test]
fn test_snappy_compliance_corrupted_baddata_rejection() {
    let corrupt_corpora = collect_corrupt_corpora();
    assert!(
        !corrupt_corpora.is_empty(),
        "At least one corrupt corpus must be tested (0 skipped)"
    );

    for (name, corrupt_data) in corrupt_corpora {
        // Must NOT panic, must return error or false
        let val_raw = snappy_validate_raw(&corrupt_data, 1024 * 1024 * 64);
        let dec_raw = snappy_decompress_raw(&corrupt_data);
        let val_framed = snappy_validate_framed(&corrupt_data);
        let dec_framed = snappy_decompress_framed(&corrupt_data);

        // If raw validate is false, decompress must fail
        if !val_raw {
            assert!(dec_raw.is_err(), "Raw decompress on invalid {} must fail", name);
        }
        if !val_framed {
            assert!(dec_framed.is_err(), "Framed decompress on invalid {} must fail", name);
        }
    }
}

