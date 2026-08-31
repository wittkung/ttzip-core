// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Integration tests for Brotli 7-layer defense guard and decompression bomb circuit breaker.
//!
//! Validates:
//! 1. Unauthorized Large Window (1 GiB) interception and WBITS boundary validation.
//! 2. Decompression bomb cumulative output budget and expansion ratio circuit breakers.
//! 3. Maximum distance ceiling (`0x7FFFFFFC`) backward reference injection defense.
//! 4. Exuberant Nibble & Exuberant Meta Nibble protocol violations with zero panics.
//! 5. Non-zero byte-boundary alignment padding rejection.
//! 6. Authorized Large Window extension workflows and state lifecycle resets.

use ttzip_engine::security::brotli_defense::{
    BrotliDefenseConfig, BrotliDefenseGuard, BROTLI_LARGE_MAX_WINDOW_BITS,
    BROTLI_MAX_ALLOWED_DISTANCE, BROTLI_MAX_WINDOW_BITS, BROTLI_MIN_WINDOW_BITS,
};
use ttzip_engine::types::TTZipStatus;

#[test]
fn test_unauthorized_large_window_rejection() {
    // Default guard has allow_large_window = false, max_window_bits = 24
    let guard = BrotliDefenseGuard::default();

    // 1. Standard valid window bits (10..=24) must pass
    for wbits in BROTLI_MIN_WINDOW_BITS..=BROTLI_MAX_WINDOW_BITS {
        assert_eq!(
            guard.validate_window_bits(wbits),
            Ok(()),
            "Valid window bits {wbits} must succeed"
        );
    }

    // 2. Out-of-bounds lower bits (< 10) must be rejected
    for wbits in 0..10 {
        assert_eq!(
            guard.validate_window_bits(wbits),
            Err(TTZipStatus::ErrSecurityViolation),
            "Window bits {wbits} < 10 must trigger ErrSecurityViolation"
        );
    }

    // 3. Unauthorized Large Window bits (25..=30, e.g. 1 GiB window at wbits = 30) must be rejected
    for wbits in 25..=BROTLI_LARGE_MAX_WINDOW_BITS {
        assert_eq!(
            guard.validate_window_bits(wbits),
            Err(TTZipStatus::ErrSecurityViolation),
            "Unauthorized large window bits {wbits} must trigger ErrSecurityViolation"
        );
    }

    // 4. Over-the-top bits (> 30) must be rejected
    for wbits in 31..=64 {
        assert_eq!(
            guard.validate_window_bits(wbits),
            Err(TTZipStatus::ErrSecurityViolation),
            "Window bits {wbits} > 30 must trigger ErrSecurityViolation"
        );
    }
}

#[test]
fn test_authorized_large_window_validation() {
    // Config with Large Window authorized (up to 30 bits)
    let config = BrotliDefenseConfig::default_limits()
        .with_allow_large_window(true)
        .with_max_window_bits(30);
    let guard = BrotliDefenseGuard::new(config);

    // 1. Valid range 10..=30 must all pass
    for wbits in 10..=30 {
        assert_eq!(
            guard.validate_window_bits(wbits),
            Ok(()),
            "Authorized large window bits {wbits} must succeed"
        );
    }

    // 2. Exceeding 30 must be rejected
    assert_eq!(
        guard.validate_window_bits(31),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 3. Config with Large Window authorized but constrained to 27 bits
    let constrained_guard = BrotliDefenseGuard::new(
        BrotliDefenseConfig::default_limits()
            .with_allow_large_window(true)
            .with_max_window_bits(27),
    );
    assert_eq!(constrained_guard.validate_window_bits(27), Ok(()));
    assert_eq!(
        constrained_guard.validate_window_bits(28),
        Err(TTZipStatus::ErrSecurityViolation),
        "Window bits exceeding max_window_bits policy must trigger ErrSecurityViolation"
    );
}

#[test]
fn test_decompression_bomb_expansion_ratio_circuit_breaker() {
    let mut guard = BrotliDefenseGuard::new(
        BrotliDefenseConfig::default_limits()
            .with_max_output_limit(512 * 1024 * 1024) // 512 MiB limit
            .with_max_expansion_ratio(100) // 100:1 ratio limit
            .with_threshold_bytes(1024 * 1024), // 1 MiB threshold
    );

    // 1. Below warmup threshold (50 bytes compressed -> 500 KiB decompressed = 10,000:1 ratio)
    // Should NOT trip because decompressed output (500 KiB) <= threshold (1 MiB)
    assert_eq!(
        guard.track_decompression(50, 500 * 1024),
        Ok(()),
        "Decompression below threshold must not prematurely trip ratio breaker"
    );

    // 2. Normal reasonable decompression (1 MiB compressed -> 5 MiB decompressed)
    // Cumulative: 1,048,626 compressed, 5,742,080 decompressed -> ratio ~5.47:1 <= 100:1
    assert_eq!(
        guard.track_decompression(1024 * 1024, 5 * 1024 * 1024),
        Ok(()),
        "Normal compression ratio must pass"
    );
    assert!(guard.current_ratio() < 10.0);

    // 3. Malicious payload chunk triggering explosive expansion ratio beyond 100:1
    // Add 100 bytes compressed -> 200 MiB decompressed
    // Cumulative: ~1 MiB compressed, ~205 MiB decompressed -> ratio ~205:1 > 100:1
    let bomb_res = guard.track_decompression(100, 200 * 1024 * 1024);
    assert_eq!(
        bomb_res,
        Err(TTZipStatus::ErrSecurityViolation),
        "Explosive expansion ratio exceeding 100:1 past threshold must trip ErrSecurityViolation"
    );
}

#[test]
fn test_decompression_bomb_total_output_quota_breaker() {
    let mut guard = BrotliDefenseGuard::with_output_limit(10 * 1024 * 1024); // 10 MiB hard limit

    // 1. Decompress 8 MiB with 1:1 ratio
    assert_eq!(guard.track_decompression(8 * 1024 * 1024, 8 * 1024 * 1024), Ok(()));
    assert_eq!(guard.bytes_written(), 8 * 1024 * 1024);

    // 2. Decompress another 3 MiB -> total 11 MiB > 10 MiB limit
    let quota_res = guard.track_decompression(3 * 1024 * 1024, 3 * 1024 * 1024);
    assert_eq!(
        quota_res,
        Err(TTZipStatus::ErrSecurityViolation),
        "Exceeding max_output_limit must immediately trip ErrSecurityViolation"
    );
}

#[test]
fn test_max_distance_ceiling_overflow_guard() {
    let guard = BrotliDefenseGuard::default();

    // 1. Distance = 0 is invalid
    assert_eq!(
        guard.validate_distance(0),
        Err(TTZipStatus::ErrSecurityViolation),
        "Zero distance must trigger ErrSecurityViolation"
    );

    // 2. Valid standard and large distances up to 0x7FFFFFFC
    assert_eq!(guard.validate_distance(1), Ok(()));
    assert_eq!(guard.validate_distance(64 * 1024 * 1024), Ok(()));
    assert_eq!(guard.validate_distance(BROTLI_MAX_ALLOWED_DISTANCE), Ok(()));

    // 3. Distances strictly greater than 0x7FFFFFFC must be intercepted
    assert_eq!(
        guard.validate_distance(BROTLI_MAX_ALLOWED_DISTANCE + 1),
        Err(TTZipStatus::ErrSecurityViolation),
        "Distance exceeding 0x7FFFFFFC must trigger ErrSecurityViolation"
    );
    assert_eq!(
        guard.validate_distance(0x8000_0000),
        Err(TTZipStatus::ErrSecurityViolation),
        "32-bit signed overflow distance must trigger ErrSecurityViolation"
    );
    assert_eq!(
        guard.validate_distance(usize::MAX),
        Err(TTZipStatus::ErrSecurityViolation),
        "usize::MAX distance must trigger ErrSecurityViolation"
    );
}

#[test]
fn test_exuberant_nibbles_rejection() {
    let guard = BrotliDefenseGuard::default();

    // 1. Valid meta-block nibbles:
    // size_nibbles = 4 can have high_nibble == 0 (representing small meta-block lengths)
    assert_eq!(guard.validate_meta_block_nibbles(4, 0), Ok(()));
    assert_eq!(guard.validate_meta_block_nibbles(4, 5), Ok(()));
    assert_eq!(guard.validate_meta_block_nibbles(4, 15), Ok(()));

    // size_nibbles in 5..=7 with non-zero high_nibble must pass
    for nibbles in 5..=7 {
        for high in 1..=15 {
            assert_eq!(
                guard.validate_meta_block_nibbles(nibbles, high),
                Ok(()),
                "Valid nibbles {nibbles} with high {high} must pass"
            );
        }
    }

    // 2. Exuberant nibbles: size_nibbles > 4 with high_nibble == 0 MUST be rejected
    for nibbles in 5..=7 {
        assert_eq!(
            guard.validate_meta_block_nibbles(nibbles, 0),
            Err(TTZipStatus::ErrSecurityViolation),
            "Exuberant nibble with size_nibbles={nibbles} and high_nibble=0 must trigger ErrSecurityViolation"
        );
    }

    // 3. Out-of-spec size_nibbles (< 4 or > 7) or high_nibble > 15 must be rejected
    assert_eq!(guard.validate_meta_block_nibbles(0, 1), Err(TTZipStatus::ErrSecurityViolation));
    assert_eq!(guard.validate_meta_block_nibbles(3, 1), Err(TTZipStatus::ErrSecurityViolation));
    assert_eq!(guard.validate_meta_block_nibbles(8, 1), Err(TTZipStatus::ErrSecurityViolation));
    assert_eq!(guard.validate_meta_block_nibbles(4, 16), Err(TTZipStatus::ErrSecurityViolation));

    // 4. Metadata block exuberant nibbles
    assert_eq!(guard.validate_metadata_nibbles(1, 0), Ok(()));
    assert_eq!(guard.validate_metadata_nibbles(2, 5), Ok(()));
    assert_eq!(guard.validate_metadata_nibbles(4, 128), Ok(()));

    // size_bytes > 1 with high_byte == 0 must be rejected
    for size_bytes in 2..=4 {
        assert_eq!(
            guard.validate_metadata_nibbles(size_bytes, 0),
            Err(TTZipStatus::ErrSecurityViolation),
            "Exuberant meta nibble with size_bytes={size_bytes} and high_byte=0 must trigger ErrSecurityViolation"
        );
    }
    assert_eq!(guard.validate_metadata_nibbles(0, 1), Err(TTZipStatus::ErrSecurityViolation));
    assert_eq!(guard.validate_metadata_nibbles(5, 1), Err(TTZipStatus::ErrSecurityViolation));
}

#[test]
fn test_byte_boundary_padding_validation() {
    let guard = BrotliDefenseGuard::default();

    // 1. Valid zero padding across 0..=7 bits
    for bits in 0..=7 {
        assert_eq!(
            guard.validate_padding(bits, 0),
            Ok(()),
            "Zero padding of {bits} bits must succeed"
        );
    }

    // 2. Non-zero padding values must be intercepted
    for pad_val in 1..=255 {
        assert_eq!(
            guard.validate_padding(3, pad_val),
            Err(TTZipStatus::ErrSecurityViolation),
            "Non-zero padding value {pad_val} must trigger ErrSecurityViolation"
        );
    }

    // 3. Padding bits > 7 is invalid
    assert_eq!(
        guard.validate_padding(8, 0),
        Err(TTZipStatus::ErrSecurityViolation)
    );
}

#[test]
fn test_guard_reset_and_lifecycle() {
    let mut guard = BrotliDefenseGuard::default();

    assert_eq!(guard.track_decompression(1000, 2000), Ok(()));
    assert_eq!(guard.bytes_read(), 1000);
    assert_eq!(guard.bytes_written(), 2000);
    assert_eq!(guard.current_ratio(), 2.0);

    guard.reset();
    assert_eq!(guard.bytes_read(), 0);
    assert_eq!(guard.bytes_written(), 0);
    assert_eq!(guard.current_ratio(), 0.0);
}
