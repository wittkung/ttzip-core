// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration tests for Libdeflate 6-Layer Defense Guard and Decompression Bomb Circuit Breakers.
//!
//! Validates:
//! 1. Decompression bomb cumulative output budget and 1032:1 expansion ratio circuit breakers.
//! 2. Backward reference distance underflow and maximum window boundary defense.
//! 3. Kraft-McMillan inequality and malformed Huffman codespace overload defense.
//! 4. Bitstream overread boundary guard (`overread_count > 8`).
//! 5. RFC 1951 uncompressed block inverted length integrity (`LEN == !NLEN`).
//! 6. Sensitive memory scrubbing via Zeroize and zeroize-on-drop invariants.
//! 7. Container stream header validation and guarded decompressor behavior across Raw, Zlib, and Gzip.

use zeroize::Zeroize;

use ttzip_engine::codecs::libdeflate::container::{
    compress_container, ContainerFormat, GZIP_ID1, GZIP_ID2,
};
use ttzip_engine::security::libdeflate_defense::{
    guarded_decompress, validate_stream_header, LibdeflateDefenseConfig, LibdeflateSecurityGuard,
    LIBDEFLATE_DEFAULT_MAX_OUTPUT_LIMIT, LIBDEFLATE_DEFAULT_THRESHOLD_BYTES,
    LIBDEFLATE_MAX_ALLOWED_DISTANCE, LIBDEFLATE_MAX_EXPANSION_RATIO,
    LIBDEFLATE_MAX_OVERREAD_BYTES,
};
use ttzip_engine::types::TTZipStatus;

// MARK: - Configuration & Invariants Tests

#[test]
fn test_libdeflate_defense_default_config_invariants() {
    let cfg = LibdeflateDefenseConfig::default();
    assert_eq!(cfg.max_output_limit, LIBDEFLATE_DEFAULT_MAX_OUTPUT_LIMIT);
    assert_eq!(cfg.max_expansion_ratio, LIBDEFLATE_MAX_EXPANSION_RATIO);
    assert_eq!(cfg.threshold_bytes, LIBDEFLATE_DEFAULT_THRESHOLD_BYTES);
    assert_eq!(cfg.max_distance, LIBDEFLATE_MAX_ALLOWED_DISTANCE);

    let guard = LibdeflateSecurityGuard::default();
    assert_eq!(guard.bytes_read(), 0);
    assert_eq!(guard.bytes_written(), 0);
    assert_eq!(guard.current_ratio(), 0.0);
}

#[test]
fn test_libdeflate_defense_config_builders() {
    let cfg = LibdeflateDefenseConfig::new(10 * 1024 * 1024, 500)
        .with_max_output_limit(20 * 1024 * 1024)
        .with_max_expansion_ratio(250)
        .with_threshold_bytes(512 * 1024)
        .with_max_distance(16384);

    assert_eq!(cfg.max_output_limit, 20 * 1024 * 1024);
    assert_eq!(cfg.max_expansion_ratio, 250);
    assert_eq!(cfg.threshold_bytes, 512 * 1024);
    assert_eq!(cfg.max_distance, 16384);

    let guard = LibdeflateSecurityGuard::with_output_limit(1024 * 1024);
    assert_eq!(guard.config().max_output_limit, 1024 * 1024);
}

// MARK: - Layer 1: Output Quota & Decompression Bomb Tests

#[test]
fn test_libdeflate_defense_quota_circuit_breaker() {
    let cfg = LibdeflateDefenseConfig::new(2 * 1024 * 1024, 1032)
        .with_threshold_bytes(1024 * 1024);
    let mut guard = LibdeflateSecurityGuard::new(cfg);

    // 1st chunk: 1 MiB decompressed from 10 KiB compressed -> OK
    assert_eq!(guard.track_decompression(10 * 1024, 1024 * 1024), Ok(()));
    assert_eq!(guard.bytes_written(), 1024 * 1024);
    assert_eq!(guard.bytes_read(), 10 * 1024);

    // 2nd chunk: 1 MiB decompressed -> total 2 MiB -> OK (exactly at limit)
    assert_eq!(guard.track_decompression(10 * 1024, 1024 * 1024), Ok(()));
    assert_eq!(guard.bytes_written(), 2 * 1024 * 1024);

    // 3rd chunk: 1 byte additional -> exceeds 2 MiB limit -> ErrSecurityViolation
    assert_eq!(
        guard.track_decompression(100, 1),
        Err(TTZipStatus::ErrSecurityViolation)
    );
}

#[test]
fn test_libdeflate_defense_expansion_ratio_circuit_breaker() {
    let cfg = LibdeflateDefenseConfig::new(500 * 1024 * 1024, 1032)
        .with_threshold_bytes(1024 * 1024);
    let mut guard = LibdeflateSecurityGuard::new(cfg);

    // Normal ratio (100:1) past threshold (1.5 MiB out / 15 KiB in) -> OK
    assert_eq!(
        guard.track_decompression(15 * 1024, 1536 * 1024),
        Ok(())
    );

    // Exorbitant expansion bomb ratio: 2000x past threshold (100 B in / 2 MiB out) -> ErrSecurityViolation
    let mut bomb_guard = LibdeflateSecurityGuard::new(
        LibdeflateDefenseConfig::new(500 * 1024 * 1024, 1032).with_threshold_bytes(1024 * 1024),
    );
    assert_eq!(
        bomb_guard.track_decompression(100, 2 * 1024 * 1024),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // Standalone ratio validation
    assert_eq!(
        guard.validate_expansion_ratio(100, 2 * 1024 * 1024),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        guard.validate_expansion_ratio(100 * 1024, 1024 * 1024),
        Ok(())
    );
}

// MARK: - Layer 2: Match Distance Underflow Tests

#[test]
fn test_libdeflate_defense_distance_underflow_defense() {
    let guard = LibdeflateSecurityGuard::default();

    // 1. Zero distance is strictly invalid in RFC 1951 Deflate
    assert_eq!(
        guard.validate_distance(0, 100),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 2. Valid backward reference within written buffer boundary
    assert_eq!(guard.validate_distance(1, 100), Ok(()));
    assert_eq!(guard.validate_distance(50, 100), Ok(()));
    assert_eq!(guard.validate_distance(100, 100), Ok(()));

    // 3. Backward distance exceeds cursor (buffer underflow attack)
    assert_eq!(
        guard.validate_distance(101, 100),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        guard.validate_distance(5000, 100),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 4. Backward distance exceeds 32 KiB maximum sliding window
    assert_eq!(
        guard.validate_distance(32769, 100_000),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(guard.validate_distance(32768, 100_000), Ok(()));
}

// MARK: - Layer 3: Kraft Inequality & Codespace Tests

#[test]
fn test_libdeflate_defense_kraft_inequality_and_codespace() {
    let guard = LibdeflateSecurityGuard::default();

    // 1. Valid full binary tree: 2 codes of length 1 (Kraft sum = 1/2 + 1/2 = 1.0)
    let valid_tree_2 = [1u8, 1u8];
    assert_eq!(guard.validate_huffman_codespace(&valid_tree_2, 1), Ok(()));

    // 2. Valid complete tree: 4 codes of length 2 (Kraft sum = 4 * 1/4 = 1.0)
    let valid_tree_4 = [2u8, 2u8, 2u8, 2u8];
    assert_eq!(guard.validate_huffman_codespace(&valid_tree_4, 2), Ok(()));

    // 3. Over-subscribed tree: 3 codes of length 1 (Kraft sum = 3/2 = 1.5 > 1.0) -> ErrSecurityViolation
    let oversubscribed_tree = [1u8, 1u8, 1u8];
    assert_eq!(
        guard.validate_huffman_codespace(&oversubscribed_tree, 1),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 4. Over-subscribed tree with mixed lengths: 2 of len 1 + 1 of len 2 (Kraft sum = 1 + 0.25 = 1.25 > 1.0)
    let oversubscribed_mixed = [1u8, 1u8, 2u8];
    assert_eq!(
        guard.validate_huffman_codespace(&oversubscribed_mixed, 2),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 5. Valid single-symbol incomplete code (1 symbol with len 1)
    let single_symbol_code = [1u8, 0u8, 0u8];
    assert_eq!(guard.validate_huffman_codespace(&single_symbol_code, 1), Ok(()));

    // 6. Invalid incomplete code (2 symbols with len 2 -> Kraft sum = 0.5 < 1.0)
    let invalid_incomplete = [2u8, 2u8, 0u8];
    assert_eq!(
        guard.validate_huffman_codespace(&invalid_incomplete, 2),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // 7. Invalid parameters
    assert_eq!(
        guard.validate_huffman_codespace(&[], 0),
        Err(TTZipStatus::ErrCorruptHeader)
    );
    assert_eq!(
        guard.validate_huffman_codespace(&[1u8], 16),
        Err(TTZipStatus::ErrCorruptHeader)
    );
}

// MARK: - Layer 4: Bitstream Overread Guard Tests

#[test]
fn test_libdeflate_defense_overread_guard() {
    let guard = LibdeflateSecurityGuard::default();

    // 1. Overread count within 8 bytes is allowable during bitbuffer refill near EOF
    for count in 0..=LIBDEFLATE_MAX_OVERREAD_BYTES {
        assert_eq!(guard.validate_overread(count), Ok(()));
    }

    // 2. Overread count exceeding 8 bytes indicates bitstream runaway/truncation
    assert_eq!(
        guard.validate_overread(LIBDEFLATE_MAX_OVERREAD_BYTES + 1),
        Err(TTZipStatus::ErrCorruptHeader)
    );
    assert_eq!(
        guard.validate_overread(100),
        Err(TTZipStatus::ErrCorruptHeader)
    );
}

// MARK: - Layer 5: Uncompressed Block Inverted Length Invariant Tests

#[test]
fn test_libdeflate_defense_uncompressed_block_invariant() {
    let guard = LibdeflateSecurityGuard::default();

    // 1. Valid RFC 1951 pairs where len == !nlen
    assert_eq!(guard.validate_uncompressed_block(0, 0xFFFF), Ok(()));
    assert_eq!(guard.validate_uncompressed_block(1, 0xFFFE), Ok(()));
    assert_eq!(guard.validate_uncompressed_block(1024, !1024), Ok(()));
    assert_eq!(guard.validate_uncompressed_block(65535, 0), Ok(()));

    // 2. Malformed pairs where len != !nlen
    assert_eq!(
        guard.validate_uncompressed_block(0, 0),
        Err(TTZipStatus::ErrCorruptHeader)
    );
    assert_eq!(
        guard.validate_uncompressed_block(100, 100),
        Err(TTZipStatus::ErrCorruptHeader)
    );
    assert_eq!(
        guard.validate_uncompressed_block(1024, !1025),
        Err(TTZipStatus::ErrCorruptHeader)
    );
}

// MARK: - Layer 6: Sensitive Memory Zeroize Protection Tests

#[test]
fn test_libdeflate_defense_zeroize_protection() {
    let mut guard = LibdeflateSecurityGuard::default();
    assert_eq!(
        guard.track_decompression(1000, 5000),
        Ok(())
    );
    guard.secure_scratch[0] = 0xAA;
    guard.secure_scratch[63] = 0xBB;

    assert_eq!(guard.bytes_read(), 1000);
    assert_eq!(guard.bytes_written(), 5000);

    // Explicit reset wipes counters and scratchpad
    guard.reset();
    assert_eq!(guard.bytes_read(), 0);
    assert_eq!(guard.bytes_written(), 0);
    assert_eq!(guard.secure_scratch, [0u8; 64]);

    // Zeroize trait implementation
    guard.secure_scratch[10] = 0xFF;
    guard.zeroize();
    assert_eq!(guard.secure_scratch, [0u8; 64]);
}

// MARK: - Stream Header Validation Tests

#[test]
fn test_libdeflate_defense_stream_header_validation() {
    // 1. Raw Deflate: always passes header validation
    assert_eq!(validate_stream_header(&[], ContainerFormat::Raw), Ok(()));
    assert_eq!(validate_stream_header(&[0x01, 0x02], ContainerFormat::Raw), Ok(()));

    // 2. Zlib header validation
    // Valid Zlib header: CMF = 0x78 (Deflate, 32KB window), FLG = 0x01 (FCHECK = 1, (0x7801) % 31 == 0)
    let valid_zlib = [0x78, 0x01];
    assert_eq!(validate_stream_header(&valid_zlib, ContainerFormat::Zlib), Ok(()));

    // Truncated Zlib header (< 2 bytes)
    assert_eq!(
        validate_stream_header(&[0x78], ContainerFormat::Zlib),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // Corrupted FCHECK ((0x7800) % 31 != 0)
    let invalid_fcheck = [0x78, 0x00];
    assert_eq!(
        validate_stream_header(&invalid_fcheck, ContainerFormat::Zlib),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // Invalid Compression Method (CM != 8)
    let invalid_cm = [0x79, 0x00];
    assert_eq!(
        validate_stream_header(&invalid_cm, ContainerFormat::Zlib),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // Preset dictionary set (FDICT != 0)
    let fdict_set = [0x78, 0x20];
    assert_eq!(
        validate_stream_header(&fdict_set, ContainerFormat::Zlib),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // 3. Gzip header validation
    // Valid Gzip header: 10 bytes with ID1=0x1F, ID2=0x8B, CM=8, FLG=0
    let valid_gzip = [GZIP_ID1, GZIP_ID2, 8, 0, 0, 0, 0, 0, 0, 255];
    assert_eq!(validate_stream_header(&valid_gzip, ContainerFormat::Gzip), Ok(()));

    // Truncated Gzip header (< 10 bytes)
    assert_eq!(
        validate_stream_header(&valid_gzip[..9], ContainerFormat::Gzip),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // Corrupted Gzip magic
    let invalid_magic = [0x1F, 0x99, 8, 0, 0, 0, 0, 0, 0, 255];
    assert_eq!(
        validate_stream_header(&invalid_magic, ContainerFormat::Gzip),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // Gzip reserved flags set
    let reserved_flags = [GZIP_ID1, GZIP_ID2, 8, 0xE0, 0, 0, 0, 0, 0, 255];
    assert_eq!(
        validate_stream_header(&reserved_flags, ContainerFormat::Gzip),
        Err(TTZipStatus::ErrCorruptHeader)
    );
}

// MARK: - Guarded Decompression End-to-End Tests

#[test]
fn test_libdeflate_guarded_decompress_roundtrip_all_formats() {
    let payload = b"TTZip Libdeflate 6-Layer Security Guard and Circuit Breaker Invariants!";

    for format in [ContainerFormat::Raw, ContainerFormat::Zlib, ContainerFormat::Gzip] {
        let compressed = compress_container(payload, format, 6)
            .expect("Compression must succeed");

        let mut dst = vec![0u8; 1024];
        let decompressed_len = guarded_decompress(&compressed, &mut dst, format, 1024)
            .expect("Guarded decompression must succeed");

        assert_eq!(decompressed_len, payload.len());
        assert_eq!(&dst[..decompressed_len], payload);
    }
}

#[test]
fn test_libdeflate_guarded_decompress_zero_limit_rejection() {
    let payload = b"Decompression Bomb Prevention";
    let compressed = compress_container(payload, ContainerFormat::Raw, 6)
        .expect("Compression must succeed");

    let mut dst = vec![0u8; 1024];
    let res = guarded_decompress(&compressed, &mut dst, ContainerFormat::Raw, 0);
    assert_eq!(res, Err(TTZipStatus::ErrSecurityViolation));
}

#[test]
fn test_libdeflate_guarded_decompress_quota_exceeded_rejection() {
    let payload = vec![0x42u8; 10_000];
    let compressed = compress_container(&payload, ContainerFormat::Zlib, 6)
        .expect("Compression must succeed");

    // Output quota set to 5,000 bytes (lower than 10,000 payload)
    let mut dst = vec![0u8; 10_000];
    let res = guarded_decompress(&compressed, &mut dst, ContainerFormat::Zlib, 5_000);
    assert!(res.is_err());
}

#[test]
fn test_libdeflate_guarded_decompress_zero_panic_on_corrupted_stream() {
    let corrupted_payload = vec![0xFFu8; 256];
    let mut dst = vec![0u8; 1024];

    for format in [ContainerFormat::Raw, ContainerFormat::Zlib, ContainerFormat::Gzip] {
        let res = guarded_decompress(&corrupted_payload, &mut dst, format, 1024);
        assert!(res.is_err(), "Corrupted payload must return Err gracefully without panicking");
    }
}
