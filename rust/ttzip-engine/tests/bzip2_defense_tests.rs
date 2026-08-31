// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit tests for Bzip2 6-layer security defense and quota bounds.

use ttzip_engine::security::bzip2_defense::{Bzip2DefenseGuard, Bzip2SecurityLimits};
use ttzip_engine::types::TTZipStatus;

#[test]
fn test_security_limits_defaults() {
    let limits = Bzip2SecurityLimits::default();
    assert_eq!(limits.max_block_size, 900_000);
    assert_eq!(limits.max_huffman_depth, 20);
    assert_eq!(limits.max_expansion_ratio, 250);
}

#[test]
fn test_security_header_validation() {
    let guard = Bzip2DefenseGuard::new(Bzip2SecurityLimits::default());
    assert_eq!(guard.verify_stream_header(b"BZh1").unwrap(), 1);
    assert_eq!(guard.verify_stream_header(b"BZh9").unwrap(), 9);

    assert_eq!(
        guard.verify_stream_header(b"BZh0").unwrap_err(),
        TTZipStatus::ErrCorruptHeader
    );
    assert_eq!(
        guard.verify_stream_header(b"BZha").unwrap_err(),
        TTZipStatus::ErrCorruptHeader
    );
    assert_eq!(
        guard.verify_stream_header(b"PK\x03\x04").unwrap_err(),
        TTZipStatus::ErrCorruptHeader
    );
}

#[test]
fn test_security_bwt_invariant_checks() {
    let guard = Bzip2DefenseGuard::new(Bzip2SecurityLimits::default());
    assert!(guard.validate_bwt_invariants(0, 100).is_ok());
    assert!(guard.validate_bwt_invariants(99, 100).is_ok());

    // orig_ptr >= nblock
    assert_eq!(
        guard.validate_bwt_invariants(100, 100).unwrap_err(),
        TTZipStatus::ErrCorruptHeader
    );
    // nblock > 900KB
    assert_eq!(
        guard.validate_bwt_invariants(0, 1_000_000).unwrap_err(),
        TTZipStatus::ErrSecurityViolation
    );
}
