// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive test suite for Libdeflate compression pipelines, container formats,
//! and high-level facade APIs.
//!
//! Validates:
//! 1. Level 0..12 roundtrip fidelity across DEFLATE, Zlib, and Gzip containers.
//! 2. Multiple data corpora: empty, single-byte, repeated text, random binary, and structured logs.
//! 3. Format validation (`libdeflate_validate`) against valid, corrupt, and cross-format streams.
//! 4. Deterministic error mapping and boundary handling (level clamping, buffer limits).

use ttzip_engine::codecs::libdeflate::{
    deflate_compress, libdeflate_deflate_compress, libdeflate_deflate_decompress,
    libdeflate_gzip_compress, libdeflate_gzip_decompress, libdeflate_validate,
    libdeflate_zlib_compress, libdeflate_zlib_decompress, ContainerFormat,
};

// MARK: - Test Helpers

/// Generates pseudo-random byte buffer with deterministic seed.
fn generate_pseudo_random(len: usize, seed: u32) -> Vec<u8> {
    let mut state = seed;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        out.push((state >> 24) as u8);
    }
    out
}

/// Generates repeating log messages for compression benchmarking.
fn generate_mock_log(lines: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(lines * 80);
    for i in 0..lines {
        let line = format!(
            "[2026-08-31 01:00:{:02}.{:03}] [INFO] [ttzip-engine::worker-{}] Connection established successfully from 192.168.1.{}\n",
            i % 60,
            (i * 17) % 1000,
            i % 8,
            (i * 31) % 254 + 1
        );
        out.extend_from_slice(line.as_bytes());
    }
    out
}

// MARK: - Level 0: Pure Store Tests

#[test]
fn test_level_0_store_roundtrip() {
    let test_corpora: Vec<(&str, Vec<u8>)> = vec![
        ("empty", vec![]),
        ("single_byte", vec![b'A']),
        ("short_text", b"Hello, Libdeflate Store Level 0!".to_vec()),
        ("random_4k", generate_pseudo_random(4096, 42)),
        ("log_32k", generate_mock_log(400)),
    ];

    for (name, data) in test_corpora {
        // Raw DEFLATE
        let compressed = deflate_compress(&data, 0).expect("Level 0 deflate compression failed");
        let mut decompressed = vec![0u8; data.len() + 64];
        let n = libdeflate_deflate_decompress(&compressed, &mut decompressed)
            .expect("Level 0 deflate decompression failed");
        assert_eq!(&decompressed[..n], &data[..], "Mismatch in Level 0 deflate for {name}");

        // Zlib
        let zlib_c = libdeflate_zlib_compress(&data, 0).expect("Level 0 zlib compression failed");
        let mut zlib_d = vec![0u8; data.len() + 64];
        let n_z = libdeflate_zlib_decompress(&zlib_c, &mut zlib_d)
            .expect("Level 0 zlib decompression failed");
        assert_eq!(&zlib_d[..n_z], &data[..], "Mismatch in Level 0 zlib for {name}");

        // Gzip
        let gzip_c = libdeflate_gzip_compress(&data, 0).expect("Level 0 gzip compression failed");
        let mut gzip_d = vec![0u8; data.len() + 64];
        let n_g = libdeflate_gzip_decompress(&gzip_c, &mut gzip_d)
            .expect("Level 0 gzip decompression failed");
        assert_eq!(&gzip_d[..n_g], &data[..], "Mismatch in Level 0 gzip for {name}");
    }
}

// MARK: - Levels 1..3: HtMatchfinder Greedy Pipeline Tests

#[test]
fn test_levels_1_to_3_ht_roundtrip() {
    let data = generate_mock_log(300);

    for level in 1..=3 {
        let comp = libdeflate_deflate_compress(&data, level)
            .unwrap_or_else(|_| panic!("Level {level} compression failed"));
        assert!(comp.len() < data.len(), "Level {level} should compress text data");

        let mut decomp = vec![0u8; data.len() + 128];
        let n = libdeflate_deflate_decompress(&comp, &mut decomp)
            .unwrap_or_else(|_| panic!("Level {level} decompression failed"));
        assert_eq!(&decomp[..n], &data[..], "Roundtrip mismatch at Level {level}");
    }
}

// MARK: - Levels 4..9: HcMatchfinder Lazy Pipeline Tests

#[test]
fn test_levels_4_to_9_hc_roundtrip() {
    let data = generate_mock_log(300);

    for level in 4..=9 {
        let comp = libdeflate_deflate_compress(&data, level)
            .unwrap_or_else(|_| panic!("Level {level} compression failed"));
        assert!(comp.len() < data.len(), "Level {level} should compress text data");

        let mut decomp = vec![0u8; data.len() + 128];
        let n = libdeflate_deflate_decompress(&comp, &mut decomp)
            .unwrap_or_else(|_| panic!("Level {level} decompression failed"));
        assert_eq!(&decomp[..n], &data[..], "Roundtrip mismatch at Level {level}");
    }
}

// MARK: - Levels 10..12: BtMatchfinder Near-Optimal DP Pipeline Tests

#[test]
fn test_levels_10_to_12_bt_roundtrip() {
    let data = generate_mock_log(200);

    for level in 10..=12 {
        let comp = libdeflate_deflate_compress(&data, level)
            .unwrap_or_else(|_| panic!("Level {level} compression failed"));
        assert!(comp.len() < data.len(), "Level {level} should compress text data");

        let mut decomp = vec![0u8; data.len() + 128];
        let n = libdeflate_deflate_decompress(&comp, &mut decomp)
            .unwrap_or_else(|_| panic!("Level {level} decompression failed"));
        assert_eq!(&decomp[..n], &data[..], "Roundtrip mismatch at Level {level}");
    }
}

// MARK: - Comprehensive Full-Matrix Level Matrix Tests

#[test]
fn test_full_level_matrix_roundtrip_all_formats() {
    let corpora = [
        ("empty", vec![]),
        ("single_byte", vec![b'Z']),
        ("repetitive_text", b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_vec()),
        ("json_payload", br#"{"id":1001,"status":"ok","items":["apple","orange","banana"],"nested":{"enabled":true}}"#.to_vec()),
        ("random_binary", generate_pseudo_random(2048, 12345)),
        ("mock_log", generate_mock_log(150)),
    ];

    for level in 0..=12 {
        for (name, data) in &corpora {
            // Raw Deflate
            let raw_c = libdeflate_deflate_compress(data, level)
                .unwrap_or_else(|_| panic!("Level {level} raw compress failed for {name}"));
            let mut raw_d = vec![0u8; data.len() + 64];
            let n_raw = libdeflate_deflate_decompress(&raw_c, &mut raw_d)
                .unwrap_or_else(|_| panic!("Level {level} raw decompress failed for {name}"));
            assert_eq!(&raw_d[..n_raw], &data[..], "Raw deflate mismatch at level {level} on {name}");

            // Zlib
            let zlib_c = libdeflate_zlib_compress(data, level)
                .unwrap_or_else(|_| panic!("Level {level} zlib compress failed for {name}"));
            let mut zlib_d = vec![0u8; data.len() + 64];
            let n_zlib = libdeflate_zlib_decompress(&zlib_c, &mut zlib_d)
                .unwrap_or_else(|_| panic!("Level {level} zlib decompress failed for {name}"));
            assert_eq!(&zlib_d[..n_zlib], &data[..], "Zlib mismatch at level {level} on {name}");

            // Gzip
            let gzip_c = libdeflate_gzip_compress(data, level)
                .unwrap_or_else(|_| panic!("Level {level} gzip compress failed for {name}"));
            let mut gzip_d = vec![0u8; data.len() + 64];
            let n_gzip = libdeflate_gzip_decompress(&gzip_c, &mut gzip_d)
                .unwrap_or_else(|_| panic!("Level {level} gzip decompress failed for {name}"));
            assert_eq!(&gzip_d[..n_gzip], &data[..], "Gzip mismatch at level {level} on {name}");
        }
    }
}

// MARK: - Stream Validation Tests

#[test]
fn test_libdeflate_validate_valid_and_invalid_data() {
    let data = b"Testing libdeflate stream validation with multiple container formats.";

    let raw_valid = libdeflate_deflate_compress(data, 6).unwrap();
    let zlib_valid = libdeflate_zlib_compress(data, 6).unwrap();
    let gzip_valid = libdeflate_gzip_compress(data, 6).unwrap();

    // 1. Valid streams match their respective formats
    assert!(libdeflate_validate(&raw_valid, ContainerFormat::Deflate).unwrap());
    assert!(libdeflate_validate(&raw_valid, ContainerFormat::Raw).unwrap());
    assert!(libdeflate_validate(&zlib_valid, ContainerFormat::Zlib).unwrap());
    assert!(libdeflate_validate(&gzip_valid, ContainerFormat::Gzip).unwrap());

    // 2. Cross-format rejection
    assert!(!libdeflate_validate(&zlib_valid, ContainerFormat::Gzip).unwrap());
    assert!(!libdeflate_validate(&gzip_valid, ContainerFormat::Zlib).unwrap());

    // 3. Empty input rejection
    assert!(!libdeflate_validate(&[], ContainerFormat::Deflate).unwrap());
    assert!(!libdeflate_validate(&[], ContainerFormat::Zlib).unwrap());
    assert!(!libdeflate_validate(&[], ContainerFormat::Gzip).unwrap());

    // 4. Corrupted header in Zlib
    let mut corrupt_zlib = zlib_valid.clone();
    corrupt_zlib[0] ^= 0xFF; // Invalidate CMF
    assert!(!libdeflate_validate(&corrupt_zlib, ContainerFormat::Zlib).unwrap());

    // 5. Corrupted header in Gzip
    let mut corrupt_gzip = gzip_valid.clone();
    corrupt_gzip[0] = 0x00; // Invalidate ID1 magic
    assert!(!libdeflate_validate(&corrupt_gzip, ContainerFormat::Gzip).unwrap());

    // 6. Truncated stream
    assert!(!libdeflate_validate(&raw_valid[..raw_valid.len() / 2], ContainerFormat::Deflate).unwrap());
    assert!(!libdeflate_validate(&zlib_valid[..zlib_valid.len() / 2], ContainerFormat::Zlib).unwrap());
    assert!(!libdeflate_validate(&gzip_valid[..gzip_valid.len() / 2], ContainerFormat::Gzip).unwrap());

    // 7. Pure random garbage
    let garbage = generate_pseudo_random(128, 999);
    assert!(!libdeflate_validate(&garbage, ContainerFormat::Deflate).unwrap());
    assert!(!libdeflate_validate(&garbage, ContainerFormat::Zlib).unwrap());
    assert!(!libdeflate_validate(&garbage, ContainerFormat::Gzip).unwrap());
}

// MARK: - Level Clamping & Error Handling Tests

#[test]
fn test_libdeflate_level_clamping() {
    let data = b"Level clamping and boundary parameter verification.";

    // Negative level (-1) defaults to 6
    let comp_neg = libdeflate_deflate_compress(data, -1).expect("Negative level should clamp to default");
    let mut decomp_neg = vec![0u8; data.len() + 32];
    let n = libdeflate_deflate_decompress(&comp_neg, &mut decomp_neg).expect("Decompression failed");
    assert_eq!(&decomp_neg[..n], data);

    // Out-of-bounds high level (99) clamps to 12
    let comp_high = libdeflate_deflate_compress(data, 99).expect("High level should clamp to 12");
    let mut decomp_high = vec![0u8; data.len() + 32];
    let n_high = libdeflate_deflate_decompress(&comp_high, &mut decomp_high).expect("Decompression failed");
    assert_eq!(&decomp_high[..n_high], data);
}

#[test]
fn test_libdeflate_decompression_insufficient_buffer() {
    let data = b"Decompression into undersized destination buffer should fail cleanly.";
    let comp = libdeflate_deflate_compress(data, 6).unwrap();

    let mut too_small = vec![0u8; 5];
    let res = libdeflate_deflate_decompress(&comp, &mut too_small);
    assert!(res.is_err(), "Decompressing into insufficient buffer must return Err");
}
