// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Official Zopfli RFC 1951 / RFC 1950 / RFC 1952 Compliance and Defense Test Suite.
//!
//! Validates:
//! 1. 100% Bit-exact roundtrip fidelity across Raw Deflate, Zlib, and Gzip containers.
//! 2. Google C Zopfli adversarial compression ratio benchmarks against standard Deflate.
//! 3. Omnidirectional differential decompression oracle across `flate2`, `libdeflate`, and `ttzip_engine`.
//! 4. 6-layer defense-in-depth security invariants and circuit breaker bounds.

use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use flate2::read::{DeflateDecoder, GzDecoder, ZlibDecoder};

use ttzip_engine::codecs::deflate::{deflate_compress, deflate_decompress, gzip_decompress, zlib_decompress};
use ttzip_engine::codecs::libdeflate::container::{decompress_container, ContainerFormat};
use ttzip_engine::codecs::zopfli::{
    zopfli_compress, zopfli_compress_deflate, zopfli_compress_gzip, zopfli_compress_zlib,
    ZopfliFormat, ZopfliOptions,
};
use ttzip_engine::security::zopfli_defense::{
    sanitize_zopfli_entry_path, BlockSplitRecursionGuard, DagRecursionGuard, SqueezeIterationGuard,
    ZopfliCancellationGuard, ZopfliDecompressionBombGuard, ZopfliDefenseGuard,
    ZopfliZeroizeScratchpad, ZOPFLI_DEFAULT_MAX_BLOCK_SIZE, ZOPFLI_DEFAULT_NUM_ITERATIONS,
    ZOPFLI_MAX_BLOCK_SPLIT_DEPTH, ZOPFLI_MAX_EXPANSION_RATIO, ZOPFLI_MAX_NUM_ITERATIONS,
};
use ttzip_engine::types::TTZipStatus;

// MARK: - Canonical Corpora Generators

/// Generates synthetic Canterbury corpus blend (HTML, C code, English prose, grammar patterns).
fn generate_canterbury_corpus(size: usize) -> Vec<u8> {
    if size == 0 {
        return Vec::new();
    }
    const SNIPPETS: &[&[u8]] = &[
        b"<!DOCTYPE html><html><head><title>Zopfli Test</title></head><body>\n",
        b"<p>Zopfli is an optimal Deflate compressor with dynamic programming shortest-path.</p>\n",
        b"for (int i = 0; i < n; i++) { cost[i] = calculate_shannon_entropy(i); }\n",
        b"Whan that Aprill with his shoures soote / The droghte of March hath perced to the roote,\n",
        b"{\n  \"algorithm\": \"zopfli\",\n  \"iterations\": 15,\n  \"block_splitting\": true\n}\n",
    ];
    let mut buf = Vec::with_capacity(size);
    let mut idx = 0;
    while buf.len() < size {
        let snippet = SNIPPETS[idx % SNIPPETS.len()];
        let rem = size - buf.len();
        let take = rem.min(snippet.len());
        buf.extend_from_slice(&snippet[..take]);
        idx += 1;
    }
    buf
}

/// Generates Fibonacci skewed distribution byte sequences.
fn generate_fibonacci_corpus(size: usize) -> Vec<u8> {
    if size == 0 {
        return Vec::new();
    }
    let fib_weights = [1u32, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233, 377];
    let sum_weights: u32 = fib_weights.iter().sum();
    let mut buf = Vec::with_capacity(size);
    let mut state: u32 = 0x1123_5813;

    for _ in 0..size {
        state = state.wrapping_mul(1103515245).wrapping_add(12345);
        let roll = (state >> 16) % sum_weights;
        let mut acc = 0;
        let mut chosen_symbol = 0u8;
        for (i, &w) in fib_weights.iter().enumerate() {
            acc += w;
            if roll < acc {
                chosen_symbol = ((i as u32 * 19 + 65) % 256) as u8;
                break;
            }
        }
        buf.push(chosen_symbol);
    }
    buf
}

/// Generates alternating pattern buffer.
fn generate_alternating_corpus(size: usize) -> Vec<u8> {
    let mut buf = Vec::with_capacity(size);
    for i in 0..size {
        buf.push(if i % 2 == 0 { 0x55 } else { 0xAA });
    }
    buf
}

// MARK: - Test Suite 1: Container Roundtrip Fidelity

#[test]
fn test_zopfli_raw_deflate_roundtrip_fidelity() {
    let corpora = vec![
        ("empty", Vec::new()),
        ("single_byte", vec![0x42]),
        ("small_text", b"Hello, World of Zopfli Deflate!".to_vec()),
        ("homogeneous", vec![b'Z'; 4096]),
        ("alternating", generate_alternating_corpus(2048)),
        ("fibonacci", generate_fibonacci_corpus(4096)),
        ("canterbury", generate_canterbury_corpus(8192)),
    ];

    let options = ZopfliOptions::default();

    for (name, raw) in corpora {
        let compressed = zopfli_compress_deflate(&raw, &options)
            .unwrap_or_else(|e| panic!("Failed to compress raw Deflate for '{name}': {e:?}"));

        // 1. Decompress with flate2 DeflateDecoder
        let mut decoder = DeflateDecoder::new(&compressed[..]);
        let mut decompressed_flate2 = Vec::new();
        decoder
            .read_to_end(&mut decompressed_flate2)
            .unwrap_or_else(|e| panic!("flate2 failed on '{name}': {e}"));
        assert_eq!(raw, decompressed_flate2, "flate2 mismatch on '{name}'");

        // 2. Decompress with libdeflate container helper
        let mut decompressed_libdef = vec![0u8; raw.len().max(1)];
        let libdef_out = decompress_container(
            &compressed,
            &mut decompressed_libdef,
            ContainerFormat::Raw,
        )
        .unwrap_or_else(|e| panic!("libdeflate container failed on '{name}': {e:?}"));
        assert_eq!(raw.len(), libdef_out);
        assert_eq!(raw, &decompressed_libdef[..libdef_out], "libdeflate mismatch on '{name}'");

        // 3. Decompress with ttzip-engine zero-copy decompressor
        let mut decompressed_engine = vec![0u8; raw.len().max(1)];
        let dec_size = deflate_decompress(&compressed, &mut decompressed_engine)
            .unwrap_or_else(|e| panic!("ttzip_engine failed on '{name}': {e:?}"));
        assert_eq!(raw.len(), dec_size);
        assert_eq!(raw, &decompressed_engine[..dec_size], "ttzip_engine mismatch on '{name}'");
    }
}

#[test]
fn test_zopfli_zlib_roundtrip_fidelity() {
    let corpora = vec![
        ("empty", Vec::new()),
        ("small_text", b"Zlib container wrapping test string.".to_vec()),
        ("homogeneous", vec![0x33; 2048]),
        ("fibonacci", generate_fibonacci_corpus(3000)),
        ("canterbury", generate_canterbury_corpus(4096)),
    ];

    let options = ZopfliOptions::default();

    for (name, raw) in corpora {
        let compressed = zopfli_compress_zlib(&raw, &options)
            .unwrap_or_else(|e| panic!("Failed to compress Zlib for '{name}': {e:?}"));

        // Verify RFC 1950 Zlib header: CM=8, CINFO=7 (32K window) -> 0x78
        if !compressed.is_empty() {
            assert_eq!(compressed[0], 0x78, "Invalid Zlib CM/CINFO header on '{name}'");
        }

        // 1. Decompress with flate2 ZlibDecoder
        let mut decoder = ZlibDecoder::new(&compressed[..]);
        let mut decompressed_flate2 = Vec::new();
        decoder
            .read_to_end(&mut decompressed_flate2)
            .unwrap_or_else(|e| panic!("flate2 Zlib failed on '{name}': {e}"));
        assert_eq!(raw, decompressed_flate2, "flate2 Zlib mismatch on '{name}'");

        // 2. Decompress with libdeflate zlib
        let mut decompressed_libdef = vec![0u8; raw.len().max(1)];
        let libdef_out = decompress_container(
            &compressed,
            &mut decompressed_libdef,
            ContainerFormat::Zlib,
        )
        .unwrap_or_else(|e| panic!("libdeflate zlib failed on '{name}': {e:?}"));
        assert_eq!(raw.len(), libdef_out);
        assert_eq!(raw, &decompressed_libdef[..libdef_out], "libdeflate zlib mismatch on '{name}'");

        // 3. Decompress with ttzip-engine zlib
        let mut decompressed_engine = vec![0u8; raw.len().max(1)];
        let dec_size = zlib_decompress(&compressed, &mut decompressed_engine)
            .unwrap_or_else(|e| panic!("ttzip_engine zlib failed on '{name}': {e:?}"));
        assert_eq!(raw.len(), dec_size);
        assert_eq!(raw, &decompressed_engine[..dec_size], "ttzip_engine zlib mismatch on '{name}'");
    }
}

#[test]
fn test_zopfli_gzip_roundtrip_fidelity() {
    let corpora = vec![
        ("empty", Vec::new()),
        ("small_text", b"Gzip container wrapping test stream.".to_vec()),
        ("homogeneous", vec![0xAA; 1500]),
        ("canterbury", generate_canterbury_corpus(5000)),
    ];

    let options = ZopfliOptions::default();

    for (name, raw) in corpora {
        let compressed = zopfli_compress_gzip(&raw, &options)
            .unwrap_or_else(|e| panic!("Failed to compress Gzip for '{name}': {e:?}"));

        // Verify RFC 1952 Gzip magic header: 0x1F, 0x8B
        assert!(compressed.len() >= 10, "Gzip header too short on '{name}'");
        assert_eq!(compressed[0], 0x1F, "Invalid Gzip ID1 on '{name}'");
        assert_eq!(compressed[1], 0x8B, "Invalid Gzip ID2 on '{name}'");
        assert_eq!(compressed[2], 0x08, "Invalid Gzip CM (expected DEFLATE) on '{name}'");

        // 1. Decompress with flate2 GzDecoder
        let mut decoder = GzDecoder::new(&compressed[..]);
        let mut decompressed_flate2 = Vec::new();
        decoder
            .read_to_end(&mut decompressed_flate2)
            .unwrap_or_else(|e| panic!("flate2 Gzip failed on '{name}': {e}"));
        assert_eq!(raw, decompressed_flate2, "flate2 Gzip mismatch on '{name}'");

        // 2. Decompress with ttzip-engine gzip
        let mut decompressed_engine = vec![0u8; raw.len().max(1)];
        let dec_size = gzip_decompress(&compressed, &mut decompressed_engine)
            .unwrap_or_else(|e| panic!("ttzip_engine gzip failed on '{name}': {e:?}"));
        assert_eq!(raw.len(), dec_size);
        assert_eq!(raw, &decompressed_engine[..dec_size], "ttzip_engine gzip mismatch on '{name}'");
    }
}

// MARK: - Test Suite 2: Google C Zopfli Compression Efficiency

#[test]
fn test_zopfli_compression_gain_against_standard_deflate() {
    let input = generate_canterbury_corpus(16384);

    // Standard Fast Deflate (level 1)
    let mut std_deflate_buf = vec![0u8; input.len() + 1024];
    let std_len = deflate_compress(&input, &mut std_deflate_buf, 1).unwrap();

    // Zopfli Deflate (15 iterations)
    let options = ZopfliOptions::default();
    let zopfli_compressed = zopfli_compress_deflate(&input, &options).unwrap();

    assert!(
        zopfli_compressed.len() <= std_len,
        "Zopfli compressed size ({} bytes) must be <= standard Deflate Level 1 ({} bytes)",
        zopfli_compressed.len(),
        std_len
    );

    // Multi-iteration comparison: 1 vs 15 iterations
    let opt_1 = ZopfliOptions { num_iterations: 1, ..Default::default() };
    let zopfli_1 = zopfli_compress_deflate(&input, &opt_1).unwrap();

    assert!(
        zopfli_compressed.len() <= zopfli_1.len(),
        "15 iterations ({} B) must be <= 1 iteration ({} B)",
        zopfli_compressed.len(),
        zopfli_1.len()
    );
}

// MARK: - Test Suite 3: Omnidirectional Differential Oracle

#[test]
fn test_omnidirectional_differential_oracle() {
    let mut test_cases = Vec::new();
    for size in [0, 1, 7, 16, 255, 512, 1024, 4096] {
        test_cases.push(generate_canterbury_corpus(size));
        test_cases.push(generate_fibonacci_corpus(size));
        test_cases.push(generate_alternating_corpus(size));
    }

    let options = ZopfliOptions { num_iterations: 3, ..Default::default() };

    for (idx, raw) in test_cases.iter().enumerate() {
        let compressed = zopfli_compress(raw, ZopfliFormat::Deflate, &options)
            .unwrap_or_else(|e| panic!("Case {idx} compression failed: {e:?}"));

        let mut flate2_out = Vec::new();
        DeflateDecoder::new(&compressed[..]).read_to_end(&mut flate2_out).unwrap();

        let mut engine_out = vec![0u8; raw.len().max(1)];
        let engine_len = deflate_decompress(&compressed, &mut engine_out).unwrap();

        assert_eq!(raw, &flate2_out, "Differential mismatch case {idx} on flate2");
        assert_eq!(raw.len(), engine_len, "Differential length mismatch case {idx} on engine");
        assert_eq!(raw, &engine_out[..engine_len], "Differential mismatch case {idx} on engine");
    }
}

// MARK: - Test Suite 4: 6-Layer Security Invariants

#[test]
fn test_zopfli_dag_recursion_guard_invariants() {
    let mut guard = DagRecursionGuard::new(1024, 5);
    assert!(guard.begin_block(512).is_ok());
    assert_eq!(guard.begin_block(2048), Err(TTZipStatus::ErrSecurityViolation));

    assert!(guard.begin_block(512).is_ok());
    assert!(guard.validate_transition(0, 10).is_ok());
    assert!(guard.validate_transition(10, 20).is_ok());
    assert_eq!(guard.validate_transition(20, 10), Err(TTZipStatus::ErrSecurityViolation));
    assert_eq!(guard.validate_transition(20, 20), Err(TTZipStatus::ErrSecurityViolation));
    assert_eq!(guard.validate_transition(20, 600), Err(TTZipStatus::ErrSecurityViolation));

    // Relaxation budget exhaust
    let mut step_guard = DagRecursionGuard::new(1024, 2);
    assert!(step_guard.begin_block(100).is_ok());
    assert!(step_guard.validate_transition(0, 1).is_ok());
    assert!(step_guard.validate_transition(1, 2).is_ok());
    assert_eq!(step_guard.validate_transition(2, 3), Err(TTZipStatus::ErrSecurityViolation));

    // Trace backtrack validation
    assert!(guard.validate_path_trace(&[0, 10, 50, 100], 100).is_ok());
    assert_eq!(guard.validate_path_trace(&[1, 10, 50, 100], 100), Err(TTZipStatus::ErrSecurityViolation));
    assert_eq!(guard.validate_path_trace(&[0, 10, 50, 99], 100), Err(TTZipStatus::ErrSecurityViolation));
}

#[test]
fn test_zopfli_squeeze_iteration_guard_invariants() {
    assert_eq!(SqueezeIterationGuard::new(0, None).unwrap_err(), TTZipStatus::ErrInvalidParam);
    assert_eq!(SqueezeIterationGuard::new(ZOPFLI_MAX_NUM_ITERATIONS + 1, None).unwrap_err(), TTZipStatus::ErrInvalidParam);

    let mut guard = SqueezeIterationGuard::new(3, None).unwrap();
    guard.begin_squeeze();
    assert_eq!(guard.record_iteration(100.0), Ok(true));
    assert_eq!(guard.record_iteration(90.0), Ok(true));
    assert_eq!(guard.record_iteration(80.0), Ok(false)); // reached max_iterations 3
    assert_eq!(guard.record_iteration(70.0), Err(TTZipStatus::ErrSecurityViolation));

    // Stagnant early convergence
    let mut stag_guard = SqueezeIterationGuard::new(50, None).unwrap();
    stag_guard.begin_squeeze();
    for _ in 0..5 {
        assert_eq!(stag_guard.record_iteration(100.0), Ok(true));
    }
    assert_eq!(stag_guard.record_iteration(100.0), Ok(false));
}

#[test]
fn test_zopfli_block_split_recursion_guard_invariants() {
    assert_eq!(BlockSplitRecursionGuard::new(0, 100), Err(TTZipStatus::ErrInvalidParam));
    assert_eq!(BlockSplitRecursionGuard::new(ZOPFLI_MAX_BLOCK_SPLIT_DEPTH + 1, 100), Err(TTZipStatus::ErrInvalidParam));

    let mut guard = BlockSplitRecursionGuard::new(2, 3).unwrap();
    assert!(guard.enter_depth().is_ok());
    assert!(guard.enter_depth().is_ok());
    assert_eq!(guard.enter_depth(), Err(TTZipStatus::ErrSecurityViolation));
    guard.leave_depth();
    assert_eq!(guard.current_depth(), 1);

    assert!(guard.validate_split_point(0, 50, 100).is_ok());
    assert!(guard.validate_split_point(50, 75, 100).is_ok());
    assert_eq!(guard.validate_split_point(75, 90, 100), Err(TTZipStatus::ErrSecurityViolation));
    assert_eq!(guard.validate_split_point(0, 0, 100), Err(TTZipStatus::ErrSecurityViolation));
}

#[test]
fn test_zopfli_decompression_bomb_guard_invariants() {
    let mut guard = ZopfliDecompressionBombGuard::new(1024 * 1024, ZOPFLI_MAX_EXPANSION_RATIO, 1024);
    assert!(guard.track_progress(100, 500).is_ok());
    assert_eq!(guard.track_progress(100, 2 * 1024 * 1024), Err(TTZipStatus::ErrSecurityViolation));

    let mut ratio_guard = ZopfliDecompressionBombGuard::new(10 * 1024 * 1024, 10, 100);
    assert_eq!(ratio_guard.track_progress(10, 200), Err(TTZipStatus::ErrSecurityViolation));
}

#[test]
fn test_zopfli_zeroize_and_cancellation_guards() {
    let mut scratch = ZopfliZeroizeScratchpad::new(128);
    scratch.symbol_scratch.push(0x55AA);
    scratch.cost_scratch.push(std::f64::consts::PI);
    scratch.hash_scratch.push(0xDEAD_BEEF);
    scratch.secure_salt[0] = 0xFF;
    scratch.reset();
    assert!(scratch.symbol_scratch.is_empty());
    assert!(scratch.cost_scratch.is_empty());
    assert!(scratch.hash_scratch.is_empty());
    assert_eq!(scratch.secure_salt[0], 0);

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let mut cancel_guard = ZopfliCancellationGuard::new(Some(cancel_flag.clone()), 2);
    assert!(cancel_guard.check_cancelled().is_ok());
    assert!(cancel_guard.tick_check().is_ok());
    cancel_flag.store(true, Ordering::SeqCst);
    assert_eq!(cancel_guard.check_cancelled(), Err(TTZipStatus::Cancelled));
    assert_eq!(cancel_guard.tick_check(), Err(TTZipStatus::Cancelled));
}

#[test]
fn test_zopfli_composite_guard_and_path_sanitizer() {
    let guard = ZopfliDefenseGuard::new();
    assert_eq!(guard.dag_guard.max_block_size(), ZOPFLI_DEFAULT_MAX_BLOCK_SIZE);
    assert_eq!(guard.squeeze_guard.max_iterations(), ZOPFLI_DEFAULT_NUM_ITERATIONS);

    let res = sanitize_zopfli_entry_path("../../etc/shadow");
    assert_eq!(res.normalized_path, "etc/shadow");
    assert!(res.has_traversal_attack);
}
