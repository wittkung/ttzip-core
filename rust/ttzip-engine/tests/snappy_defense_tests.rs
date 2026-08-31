// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration tests for Snappy 6-Layer Defense-in-Depth and Circuit Breakers.

use ttzip_engine::security::snappy_defense::{
    SnappyDefenseConfig, SnappyDefenseGuard, SNAPPY_MAX_ALLOWED_BLOCK_SIZE,
};
use ttzip_engine::types::TTZipStatus;

#[test]
fn test_snappy_defense_default_config_invariants() {
    let cfg = SnappyDefenseConfig::default();
    assert_eq!(cfg.max_output_limit, 512 * 1024 * 1024);
    assert_eq!(cfg.max_chunk_size, 65536);
    assert_eq!(cfg.max_expansion_ratio, 100);
    assert_eq!(SNAPPY_MAX_ALLOWED_BLOCK_SIZE, 512 * 1024 * 1024);

    let guard = SnappyDefenseGuard::default();
    assert_eq!(guard.bytes_read(), 0);
    assert_eq!(guard.bytes_written(), 0);
    assert_eq!(guard.chunk_count(), 0);
}

#[test]
fn test_snappy_defense_raw_uncompressed_length_bounds() {
    let cfg = SnappyDefenseConfig::new(10 * 1024 * 1024, 65536, 100);
    let guard = SnappyDefenseGuard::new(cfg);

    // Valid size within budget
    assert!(guard.validate_raw_uncompressed_length(5 * 1024 * 1024).is_ok());
    assert!(guard.validate_raw_uncompressed_length(10 * 1024 * 1024).is_ok());

    // Exceeds policy budget -> ErrSecurityViolation
    assert_eq!(
        guard.validate_raw_uncompressed_length(10 * 1024 * 1024 + 1),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        guard.validate_raw_uncompressed_length(usize::MAX),
        Err(TTZipStatus::ErrSecurityViolation)
    );
}

#[test]
fn test_snappy_defense_chunk_size_bounds() {
    let guard = SnappyDefenseGuard::default();

    // Valid chunk size <= 64KB
    assert!(guard.validate_chunk_size(0).is_ok());
    assert!(guard.validate_chunk_size(1024).is_ok());
    assert!(guard.validate_chunk_size(65536).is_ok());

    // Exceeds 64KB spec limit -> ErrSecurityViolation
    assert_eq!(
        guard.validate_chunk_size(65537),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        guard.validate_chunk_size(1024 * 1024),
        Err(TTZipStatus::ErrSecurityViolation)
    );
}

#[test]
fn test_snappy_defense_copy_offset_validation() {
    let guard = SnappyDefenseGuard::default();

    // 1. Zero offset MUST be rejected
    assert!(guard.validate_copy_offset(0, 100).is_err());

    // 2. Offset within current decompressed boundary
    assert!(guard.validate_copy_offset(1, 100).is_ok());
    assert!(guard.validate_copy_offset(50, 100).is_ok());
    assert!(guard.validate_copy_offset(100, 100).is_ok());

    // 3. Offset beyond current decompressed position (underflow exploit)
    assert!(guard.validate_copy_offset(101, 100).is_err());
    assert!(guard.validate_copy_offset(1000, 100).is_err());
}

#[test]
fn test_snappy_defense_decompression_bomb_budget_breaker() {
    let cfg = SnappyDefenseConfig::new(2 * 1024 * 1024, 65536, 100); // 2MB budget
    let mut guard = SnappyDefenseGuard::new(cfg);

    // 1st chunk: 1MB decompressed from 50KB compressed -> OK
    assert!(guard.track_decompression(50 * 1024, 1024 * 1024).is_ok());
    assert_eq!(guard.bytes_written(), 1024 * 1024);
    assert_eq!(guard.chunk_count(), 1);

    // 2nd chunk: 1MB decompressed -> total 2MB -> OK
    assert!(guard.track_decompression(50 * 1024, 1024 * 1024).is_ok());
    assert_eq!(guard.bytes_written(), 2 * 1024 * 1024);

    // 3rd chunk: 1 byte additional -> exceeds 2MB limit -> ErrSecurityViolation
    assert_eq!(
        guard.track_decompression(10, 1),
        Err(TTZipStatus::ErrSecurityViolation)
    );
}

#[test]
fn test_snappy_defense_expansion_ratio_breaker() {
    let cfg = SnappyDefenseConfig::new(500 * 1024 * 1024, 65536, 10); // 10:1 ratio limit
    let mut guard = SnappyDefenseGuard::new(cfg);

    // Initial small chunks under threshold (1MB) do not trigger ratio breaker
    assert!(guard.track_decompression(100, 50 * 1024).is_ok());

    // Exceeds threshold with excessive expansion ratio: 100 bytes input -> 5MB output (50000:1)
    assert_eq!(
        guard.track_decompression(100, 5 * 1024 * 1024),
        Err(TTZipStatus::ErrSecurityViolation)
    );
}
