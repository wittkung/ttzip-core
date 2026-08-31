// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration tests for Apple LZFSE & LZVN 6-Layer Defense-in-Depth and Circuit Breakers.

use ttzip_engine::codecs::lzfse::block::{
    BvxMagic, LzfseBlockHeader, LzfseFreqTables, LZFSE_ENCODE_D_STATES,
    LZFSE_ENCODE_LITERAL_STATES, LZFSE_ENCODE_L_STATES, LZFSE_ENCODE_M_STATES,
    LZFSE_LITERALS_PER_BLOCK, LZFSE_MATCHES_PER_BLOCK,
};
use ttzip_engine::codecs::lzfse::{lzfse_decompress_stream, lzvn_decompress_raw};
use ttzip_engine::security::lzfse_defense::{
    LzfseDefenseConfig, LzfseDefenseGuard, LzfseSecurityLimits,
    LZFSE_DEFAULT_MAX_BLOCK_UNCOMPRESSED_SIZE, LZFSE_DEFAULT_MAX_EXPANSION_RATIO,
    LZFSE_DEFAULT_MAX_OUTPUT_LIMIT, LZFSE_DEFAULT_THRESHOLD_BYTES,
    LZFSE_MAX_BACKWARD_DISTANCE,
};
use ttzip_engine::types::TTZipStatus;


#[test]
fn test_lzfse_defense_default_config_invariants() {
    let cfg = LzfseDefenseConfig::default();
    assert_eq!(cfg.max_output_limit, LZFSE_DEFAULT_MAX_OUTPUT_LIMIT);
    assert_eq!(cfg.max_expansion_ratio, LZFSE_DEFAULT_MAX_EXPANSION_RATIO);
    assert_eq!(
        cfg.max_block_uncompressed_size,
        LZFSE_DEFAULT_MAX_BLOCK_UNCOMPRESSED_SIZE
    );
    assert_eq!(cfg.threshold_bytes, LZFSE_DEFAULT_THRESHOLD_BYTES);

    let guard = LzfseDefenseGuard::default();
    assert_eq!(guard.bytes_read(), 0);
    assert_eq!(guard.bytes_written(), 0);
    assert_eq!(guard.blocks_processed(), 0);
    assert_eq!(guard.current_ratio(), 0.0);
}


#[test]
fn test_lzfse_defense_magic_validation_layer1() {
    // Valid magics
    assert_eq!(
        LzfseDefenseGuard::validate_magic(0x2d78_7662),
        Ok(BvxMagic::RawUncompressed)
    );
    assert_eq!(
        LzfseDefenseGuard::validate_magic(0x3178_7662),
        Ok(BvxMagic::CompressedV1)
    );
    assert_eq!(
        LzfseDefenseGuard::validate_magic(0x3278_7662),
        Ok(BvxMagic::CompressedV2)
    );
    assert_eq!(
        LzfseDefenseGuard::validate_magic(0x6e78_7662),
        Ok(BvxMagic::CompressedLZVN)
    );
    assert_eq!(
        LzfseDefenseGuard::validate_magic(0x2478_7662),
        Ok(BvxMagic::EndOfStream)
    );

    // Invalid magics
    assert_eq!(
        LzfseDefenseGuard::validate_magic(0x0000_0000),
        Err(TTZipStatus::ErrCorruptHeader)
    );
    assert_eq!(
        LzfseDefenseGuard::validate_magic(0xDEAD_BEEF),
        Err(TTZipStatus::ErrCorruptHeader)
    );
}

#[test]
fn test_lzfse_defense_block_size_bounds_layer1() {
    let guard = LzfseDefenseGuard::default();

    // Valid size <= 1 MiB
    assert!(guard.validate_raw_block_size(0).is_ok());
    assert!(guard.validate_raw_block_size(256 * 1024).is_ok());
    assert!(guard.validate_raw_block_size(1024 * 1024).is_ok());

    // Exceeds 1 MiB block size ceiling -> ErrSecurityViolation
    assert_eq!(
        guard.validate_raw_block_size(1024 * 1024 + 1),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        guard.validate_raw_block_size(usize::MAX),
        Err(TTZipStatus::ErrSecurityViolation)
    );
}

#[test]
fn test_lzfse_defense_decompression_bomb_budget_breaker_layer2() {
    let limits = LzfseSecurityLimits::new(4 * 1024 * 1024, 100, 1024 * 1024); // 4 MiB budget
    let mut guard = LzfseDefenseGuard::new(limits);

    // 1st block: 1 MiB decompressed from 20 KiB compressed -> OK
    assert!(guard.track_decompression(20 * 1024, 1024 * 1024).is_ok());
    assert_eq!(guard.bytes_written(), 1024 * 1024);

    // 2nd block: 2 MiB decompressed -> total 3 MiB -> OK
    assert!(guard
        .track_decompression(40 * 1024, 2 * 1024 * 1024)
        .is_ok());
    assert_eq!(guard.bytes_written(), 3 * 1024 * 1024);

    // 3rd block: 1 MiB decompressed -> total 4 MiB -> OK (exact budget limit)
    assert!(guard.track_decompression(20 * 1024, 1024 * 1024).is_ok());
    assert_eq!(guard.bytes_written(), 4 * 1024 * 1024);

    // 4th block: 1 byte additional -> exceeds 4 MiB budget -> ErrSecurityViolation
    assert_eq!(
        guard.track_decompression(10, 1),
        Err(TTZipStatus::ErrSecurityViolation)
    );
}

#[test]
fn test_lzfse_defense_expansion_ratio_breaker_layer2() {
    let limits = LzfseSecurityLimits::new(500 * 1024 * 1024, 50, 1024 * 1024) // 50:1 ratio limit
        .with_threshold_bytes(512 * 1024); // 512 KiB warmup threshold
    let mut guard = LzfseDefenseGuard::new(limits);

    // Small warmup block under threshold does not trigger breaker
    assert!(guard.track_decompression(100, 256 * 1024).is_ok());

    // Exceeds threshold with excessive expansion ratio (100 bytes -> 10 MiB, ratio 100,000:1 > 50:1)
    assert_eq!(
        guard.track_decompression(100, 10 * 1024 * 1024),
        Err(TTZipStatus::ErrSecurityViolation)
    );
}

#[test]
fn test_lzfse_defense_match_distance_underflow_defense_layer3() {
    // 1. Zero distance MUST be rejected
    assert_eq!(
        LzfseDefenseGuard::validate_match_distance(0, 100),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        LzfseDefenseGuard::validate_lzvn_distance(0, 100),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 2. Distance within written destination boundary
    assert!(LzfseDefenseGuard::validate_match_distance(1, 100).is_ok());
    assert!(LzfseDefenseGuard::validate_match_distance(50, 100).is_ok());
    assert!(LzfseDefenseGuard::validate_match_distance(100, 100).is_ok());

    // 3. Distance exceeding current destination boundary (underflow attack)
    assert_eq!(
        LzfseDefenseGuard::validate_match_distance(101, 100),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        LzfseDefenseGuard::validate_match_distance(1000, 100),
        Err(TTZipStatus::ErrSecurityViolation)
    );
}

#[test]
fn test_layer4_fse_frequency_conservation() {
    // 1. Valid L table (sum <= 64)
    let mut l_freq = [0u16; 20];
    l_freq[0] = 32;
    l_freq[1] = 32;
    assert!(LzfseDefenseGuard::validate_fse_freq_table(&l_freq, LZFSE_ENCODE_L_STATES).is_ok());

    // 2. Invalid L table (sum 65 > 64) -> ErrCorruptHeader
    l_freq[2] = 1;
    assert_eq!(
        LzfseDefenseGuard::validate_fse_freq_table(&l_freq, LZFSE_ENCODE_L_STATES),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // 3. Invalid M table (sum 70 > 64)
    let mut m_freq = [0u16; 20];
    m_freq[0] = 70;
    assert_eq!(
        LzfseDefenseGuard::validate_fse_freq_table(&m_freq, LZFSE_ENCODE_M_STATES),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // 4. Invalid D table (sum 300 > 256)
    let mut d_freq = [0u16; 64];
    d_freq[0] = 300;
    assert_eq!(
        LzfseDefenseGuard::validate_fse_freq_table(&d_freq, LZFSE_ENCODE_D_STATES),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // 5. Invalid Literal table (sum 1025 > 1024)
    let mut lit_freq = [0u16; 256];
    lit_freq[0] = 1025;
    assert_eq!(
        LzfseDefenseGuard::validate_fse_freq_table(&lit_freq, LZFSE_ENCODE_LITERAL_STATES),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // 6. Full valid LzfseFreqTables validation
    let valid_tables = LzfseFreqTables::default();
    assert!(LzfseDefenseGuard::validate_fse_freq_tables(&valid_tables).is_ok());
}


#[test]
fn test_lzfse_defense_fse_state_bounds_layer4() {
    // Valid states within bounds
    assert!(LzfseDefenseGuard::validate_fse_states(
        (LZFSE_ENCODE_L_STATES - 1) as u16,
        (LZFSE_ENCODE_M_STATES - 1) as u16,
        (LZFSE_ENCODE_D_STATES - 1) as u16,
        &[0, 100, 500, (LZFSE_ENCODE_LITERAL_STATES - 1) as u16],
    )
    .is_ok());

    // L state out of range
    assert_eq!(
        LzfseDefenseGuard::validate_fse_states(64, 0, 0, &[0; 4]),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // M state out of range
    assert_eq!(
        LzfseDefenseGuard::validate_fse_states(0, 64, 0, &[0; 4]),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // D state out of range
    assert_eq!(
        LzfseDefenseGuard::validate_fse_states(0, 0, 256, &[0; 4]),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // Literal state out of range
    assert_eq!(
        LzfseDefenseGuard::validate_fse_states(0, 0, 0, &[0, 0, 1024, 0]),
        Err(TTZipStatus::ErrCorruptHeader)
    );
}

#[test]
fn test_lzfse_defense_lifo_bitstream_bounds_layer5() {
    // 1. Initial bits outside range [-7, 0]
    assert_eq!(
        LzfseDefenseGuard::validate_lifo_stream_bounds(64, 1, 8),
        Err(TTZipStatus::ErrCorruptHeader)
    );
    assert_eq!(
        LzfseDefenseGuard::validate_lifo_stream_bounds(64, -8, 8),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // 2. Cursor retreat underflow
    assert_eq!(
        LzfseDefenseGuard::validate_lifo_cursor_retreat(10, 5),
        Ok(5)
    );
    assert_eq!(
        LzfseDefenseGuard::validate_lifo_cursor_retreat(10, 10),
        Ok(0)
    );
    assert_eq!(
        LzfseDefenseGuard::validate_lifo_cursor_retreat(10, 11),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 3. Accumulator dirty upper bits
    assert!(LzfseDefenseGuard::validate_accumulator_state(0x00FF_FFFF_FFFF_FFFF, 56).is_ok());
    assert_eq!(
        LzfseDefenseGuard::validate_accumulator_state(0x01FF_FFFF_FFFF_FFFF, 56),
        Err(TTZipStatus::ErrCorruptHeader)
    );
}

#[test]
fn test_lzfse_defense_zero_panic_guarantee_layer6() {
    // Normal Ok closure
    let ok_res = LzfseDefenseGuard::guarantee_zero_panic(|| Ok::<u32, TTZipStatus>(42));
    assert_eq!(ok_res, Ok(42));

    // Error returning closure
    let err_res = LzfseDefenseGuard::guarantee_zero_panic(|| {
        Err::<u32, TTZipStatus>(TTZipStatus::ErrCorruptHeader)
    });
    assert_eq!(err_res, Err(TTZipStatus::ErrCorruptHeader));

    // Panicking closure intercepted and mapped to ErrSecurityViolation
    let panic_res = LzfseDefenseGuard::guarantee_zero_panic(|| {
        if bool::default() {
            Ok::<u32, TTZipStatus>(0)
        } else {
            panic!("Intentional panic test inside sandbox");
        }
    });
    assert_eq!(panic_res, Err(TTZipStatus::ErrSecurityViolation));
}

#[test]
fn test_layer1_block_header_constraints() {
    let guard = LzfseDefenseGuard::default();

    // 1. Valid raw uncompressed header
    let raw_hdr = LzfseBlockHeader::new_uncompressed(64 * 1024);
    assert!(guard.validate_block_header(&raw_hdr).is_ok());

    // 2. Valid LZVN compressed header
    let lzvn_hdr = LzfseBlockHeader::new_lzvn(64 * 1024, 20 * 1024);
    assert!(guard.validate_block_header(&lzvn_hdr).is_ok());

    // 3. Valid End-of-stream header
    let eos_hdr = LzfseBlockHeader::new_end_of_stream();
    assert!(guard.validate_block_header(&eos_hdr).is_ok());

    // 4. Exceeding per-block uncompressed size limit (> 1 MiB)
    let huge_hdr = LzfseBlockHeader::new_uncompressed(2 * 1024 * 1024);
    assert_eq!(
        guard.validate_block_header(&huge_hdr),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 5. Exceeding matches per block limit
    let mut v2_hdr = LzfseBlockHeader {
        magic: BvxMagic::CompressedV2,
        n_raw_bytes: 64 * 1024,
        n_matches: (LZFSE_MATCHES_PER_BLOCK + 1) as u32,
        n_literals: 1000,
        header_size: 32,
        ..Default::default()
    };
    assert_eq!(
        guard.validate_block_header(&v2_hdr),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 6. Exceeding literals per block limit
    v2_hdr.n_matches = 1000;
    v2_hdr.n_literals = (LZFSE_LITERALS_PER_BLOCK + 1) as u32;
    assert_eq!(
        guard.validate_block_header(&v2_hdr),
        Err(TTZipStatus::ErrSecurityViolation)
    );
}

#[test]
fn test_layer2_declared_size_preflight_check() {
    let guard = LzfseDefenseGuard::default();

    // 1. Normal declared size
    assert!(guard
        .validate_declared_decompressed_size(10 * 1024 * 1024, 2 * 1024 * 1024)
        .is_ok());

    // 2. Declared size exceeds max output limit (512MB default)
    assert_eq!(
        guard.validate_declared_decompressed_size(600 * 1024 * 1024, 10 * 1024 * 1024),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 3. Declared size exceeds expansion ratio past threshold (e.g. 1KB -> 50MB)
    assert_eq!(
        guard.validate_declared_decompressed_size(50 * 1024 * 1024, 1024),
        Err(TTZipStatus::ErrSecurityViolation)
    );
}

#[test]
fn test_layer3_backward_distance_ceiling() {
    assert!(LzfseDefenseGuard::validate_match_distance(LZFSE_MAX_BACKWARD_DISTANCE, usize::MAX).is_ok());
    assert_eq!(
        LzfseDefenseGuard::validate_match_distance(LZFSE_MAX_BACKWARD_DISTANCE + 1, usize::MAX),
        Err(TTZipStatus::ErrSecurityViolation)
    );
}

#[test]
fn test_layer6_corrupted_payload_zero_panic() {
    // 1. Empty payload returns empty buffer cleanly
    assert_eq!(lzfse_decompress_stream(b""), Ok(Vec::new()));

    // 2. Malformed non-empty streams must return Err without panicking
    let malformed_payloads: Vec<&[u8]> = vec![
        b"bvx",
        b"bvx-",
        b"bvx1\x00\x00\x00\x00",
        b"bvx2\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF",
        b"bvxn\x00\x00\x00\x00\x00\x00\x00\x00",
        &[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01, 0x02, 0x03],
    ];

    for (idx, payload) in malformed_payloads.iter().enumerate() {
        let stream_res = lzfse_decompress_stream(payload);
        assert!(
            stream_res.is_err(),
            "Malformed payload #{idx} should return deterministic error without panic"
        );

        let lzvn_res = lzvn_decompress_raw(payload, 1024);
        assert!(
            lzvn_res.is_err(),
            "Malformed LZVN payload #{idx} should return deterministic error without panic"
        );
    }
}

