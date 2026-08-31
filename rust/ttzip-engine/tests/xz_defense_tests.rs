// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration tests for XzDefense memory quota budget breaker,
//! decompression bomb expansion ratio defenses, Index memory exhaustion guards,
//! and malformed Block Header filter chain security interception.

use ttzip_engine::security::xz_defense::{
    XzDefenseConfig, XzDefenseGuard, LZMA2_DECODER_OVERHEAD_BYTES,
};
use ttzip_engine::types::TTZipStatus;
use ttzip_engine::xz::block::{
    XzBlockHeader, XzFilterConfig, FILTER_ID_ARM, FILTER_ID_ARM64, FILTER_ID_DELTA,
    FILTER_ID_LZMA2, FILTER_ID_X86,
};
use ttzip_engine::xz::header::XzStreamFlags;
use ttzip_engine::xz::payload::lzma2_dict_size_from_prop;
use ttzip_engine::xz::types::XzCheckType;

#[test]
fn test_large_dictionary_oom_circuit_breaker() {
    let guard = XzDefenseGuard::with_memlimit(256 * 1024 * 1024); // 256 MiB limit

    // 1. Valid 64 MiB dictionary (prop = 28)
    // 64 MiB + 2 MiB overhead = 66 MiB <= 256 MiB
    let valid_filter = XzFilterConfig::new(FILTER_ID_LZMA2, vec![28]);
    let valid_header = XzBlockHeader {
        header_size: 8,
        compressed_size: None,
        uncompressed_size: None,
        filters: vec![valid_filter],
        check_type: XzCheckType::Crc32,
    };
    let est = guard.estimate_block_memory(&valid_header);
    assert!(est.is_ok(), "64 MiB dictionary should fit in 256 MiB quota");
    let needed = est.unwrap();
    let expected_dict = 64 * 1024 * 1024;
    assert_eq!(needed, expected_dict + LZMA2_DECODER_OVERHEAD_BYTES);

    // 2. 512 MiB dictionary (prop = 34) -> should exceed 256 MiB and trigger ErrOutOfMemory
    let large_filter_512m = XzFilterConfig::new(FILTER_ID_LZMA2, vec![34]);
    let large_header_512m = XzBlockHeader {
        header_size: 8,
        compressed_size: None,
        uncompressed_size: None,
        filters: vec![large_filter_512m],
        check_type: XzCheckType::Crc32,
    };
    let res_512m = guard.estimate_block_memory(&large_header_512m);
    assert_eq!(
        res_512m,
        Err(TTZipStatus::ErrOutOfMemory),
        "512 MiB dictionary must trip OOM breaker against 256 MiB quota"
    );

    // 3. 1 GiB dictionary (prop = 36) -> should immediately trigger ErrOutOfMemory
    let large_filter_1g = XzFilterConfig::new(FILTER_ID_LZMA2, vec![36]);
    let large_header_1g = XzBlockHeader {
        header_size: 8,
        compressed_size: None,
        uncompressed_size: None,
        filters: vec![large_filter_1g],
        check_type: XzCheckType::Crc32,
    };
    let res_1g = guard.estimate_block_memory(&large_header_1g);
    assert_eq!(
        res_1g,
        Err(TTZipStatus::ErrOutOfMemory),
        "1 GiB dictionary must immediately trip OOM breaker against 256 MiB quota"
    );

    // 4. Test exact dictionary size decoding formulas
    assert_eq!(lzma2_dict_size_from_prop(0), 4096);
    assert_eq!(lzma2_dict_size_from_prop(20), 4 * 1024 * 1024);
    assert_eq!(lzma2_dict_size_from_prop(28), 64 * 1024 * 1024);
    assert_eq!(lzma2_dict_size_from_prop(36), 1024 * 1024 * 1024);
}

#[test]
fn test_zip_bomb_expansion_ratio_breaker() {
    let mut guard = XzDefenseGuard::new(
        XzDefenseConfig::default_limits()
            .with_max_decompressed_size(10 * 1024 * 1024 * 1024) // 10 GiB
            .with_max_ratio(100) // 100:1 ratio limit
            .with_threshold_bytes(1024 * 1024), // 1 MiB threshold
    );

    // 1. Below warmup threshold (500 KiB uncompressed from 50 bytes -> 10,000:1 ratio, but < 1 MiB)
    assert_eq!(
        guard.track_decompression(50, 500 * 1024),
        Ok(()),
        "Warmup below threshold must not prematurely trip ratio guard"
    );

    // 2. Normal reasonable decompression (20 MiB compressed, 100 MiB uncompressed -> 5:1 ratio)
    assert_eq!(
        guard.track_decompression(20 * 1024 * 1024, 100 * 1024 * 1024),
        Ok(()),
        "Normal compression ratio must pass"
    );

    // 3. High expansion bomb attack: reset and inject 10 KiB compressed producing 100 MiB uncompressed
    guard.reset();
    let bomb_res = guard.track_decompression(10 * 1024, 100 * 1024 * 1024);
    assert_eq!(
        bomb_res,
        Err(TTZipStatus::ErrSecurityViolation),
        "10,000:1 expansion ratio beyond threshold must trip ErrSecurityViolation"
    );

    // 4. Hard total uncompressed size limit enforcement
    let mut quota_guard = XzDefenseGuard::new(
        XzDefenseConfig::default_limits().with_max_decompressed_size(100 * 1024 * 1024), // 100 MiB limit
    );
    // Extract 50 MiB
    assert_eq!(quota_guard.track_decompression(25 * 1024 * 1024, 50 * 1024 * 1024), Ok(()));
    // Extract another 60 MiB (total 110 MiB > 100 MiB)
    assert_eq!(
        quota_guard.track_decompression(30 * 1024 * 1024, 60 * 1024 * 1024),
        Err(TTZipStatus::ErrSecurityViolation),
        "Exceeding max_decompressed_size quota must trip ErrSecurityViolation"
    );
}

#[test]
fn test_index_record_count_and_memory_exhaustion() {
    let guard = XzDefenseGuard::new(
        XzDefenseConfig::default_limits()
            .with_memlimit(256 * 1024 * 1024)
            .with_max_index_records(1_000_000),
    );

    // 1. Valid record count
    assert_eq!(
        guard.validate_index_memory(5_000),
        Ok(()),
        "5,000 index records must pass"
    );
    assert_eq!(
        guard.validate_index_memory(1_000_000),
        Ok(()),
        "1,000,000 index records at exact limit must pass"
    );

    // 2. 10,000,000 index records exceeding max_index_records limit
    assert_eq!(
        guard.validate_index_memory(10_000_000),
        Err(TTZipStatus::ErrSecurityViolation),
        "10,000,000 records exceeding 1,000,000 must trip ErrSecurityViolation"
    );

    // 3. Memory exhaustion test when max_index_records is relaxed but memory exceeds 256 MiB
    // 6,000,000 records * 48 bytes = 288 MiB > 256 MiB
    let relaxed_guard = XzDefenseGuard::new(
        XzDefenseConfig::default_limits()
            .with_memlimit(256 * 1024 * 1024)
            .with_max_index_records(100_000_000),
    );
    assert_eq!(
        relaxed_guard.validate_index_memory(6_000_000),
        Err(TTZipStatus::ErrOutOfMemory),
        "Index memory exceeding 256 MiB must trip ErrOutOfMemory"
    );
}

#[test]
fn test_reserved_flags_and_malformed_filter_chains() {
    // 1. Raw Stream Flags Validation
    // Valid flags: None, Crc32, Crc64, Sha256
    assert!(XzDefenseGuard::validate_raw_flags([0x00, 0x00]).is_ok());
    assert!(XzDefenseGuard::validate_raw_flags([0x00, 0x01]).is_ok());
    assert!(XzDefenseGuard::validate_raw_flags([0x00, 0x04]).is_ok());
    assert!(XzDefenseGuard::validate_raw_flags([0x00, 0x0A]).is_ok());

    // Reserved byte0 != 0
    assert_eq!(
        XzDefenseGuard::validate_raw_flags([0x01, 0x01]),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // Reserved high bits in byte1 != 0
    assert_eq!(
        XzDefenseGuard::validate_raw_flags([0x00, 0x11]),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        XzDefenseGuard::validate_raw_flags([0x00, 0xF0]),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // Unsupported check type ID
    assert_eq!(
        XzDefenseGuard::validate_raw_flags([0x00, 0x02]),
        Err(TTZipStatus::ErrUnsupportedFeature)
    );
    assert_eq!(
        XzDefenseGuard::validate_raw_flags([0x00, 0x0F]),
        Err(TTZipStatus::ErrUnsupportedFeature)
    );

    // 2. Typed Stream Flags validation
    let valid_flags = XzStreamFlags::new(XzCheckType::Crc32);
    assert_eq!(XzDefenseGuard::validate_header_flags(&valid_flags), Ok(()));

    // 3. Filter Chain Validation
    let guard = XzDefenseGuard::default();

    // Empty filter chain
    assert_eq!(
        guard.validate_filter_chain(&[]),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // Filter chain with > 4 filters
    let too_many_filters = vec![
        XzFilterConfig::new(FILTER_ID_X86, vec![]),
        XzFilterConfig::new(FILTER_ID_ARM, vec![]),
        XzFilterConfig::new(FILTER_ID_ARM64, vec![]),
        XzFilterConfig::new(FILTER_ID_DELTA, vec![0]),
        XzFilterConfig::new(FILTER_ID_LZMA2, vec![20]),
    ];
    assert_eq!(
        guard.validate_filter_chain(&too_many_filters),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // Last filter is NOT LZMA2 (e.g. only X86 BCJ)
    let non_lzma2_last = vec![XzFilterConfig::new(FILTER_ID_X86, vec![])];
    assert_eq!(
        guard.validate_filter_chain(&non_lzma2_last),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // Multiple LZMA2 filters in chain
    let double_lzma2 = vec![
        XzFilterConfig::new(FILTER_ID_LZMA2, vec![20]),
        XzFilterConfig::new(FILTER_ID_LZMA2, vec![20]),
    ];
    assert_eq!(
        guard.validate_filter_chain(&double_lzma2),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // Multiple Delta filters in chain
    let double_delta = vec![
        XzFilterConfig::new(FILTER_ID_DELTA, vec![0]),
        XzFilterConfig::new(FILTER_ID_DELTA, vec![0]),
        XzFilterConfig::new(FILTER_ID_LZMA2, vec![20]),
    ];
    assert_eq!(
        guard.validate_filter_chain(&double_delta),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // Malformed Delta property (length != 1)
    let bad_delta_prop = vec![
        XzFilterConfig::new(FILTER_ID_DELTA, vec![0, 1]),
        XzFilterConfig::new(FILTER_ID_LZMA2, vec![20]),
    ];
    assert_eq!(
        guard.validate_filter_chain(&bad_delta_prop),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // Malformed LZMA2 property (length != 1)
    let bad_lzma2_prop_len = vec![XzFilterConfig::new(FILTER_ID_LZMA2, vec![])];
    assert_eq!(
        guard.validate_filter_chain(&bad_lzma2_prop_len),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // Malformed LZMA2 property (property value > 39 is reserved)
    let bad_lzma2_prop_val = vec![XzFilterConfig::new(FILTER_ID_LZMA2, vec![40])];
    assert_eq!(
        guard.validate_filter_chain(&bad_lzma2_prop_val),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // Malformed BCJ property (property length not 0 and not 4)
    let bad_bcj_prop = vec![
        XzFilterConfig::new(FILTER_ID_X86, vec![1, 2]),
        XzFilterConfig::new(FILTER_ID_LZMA2, vec![20]),
    ];
    assert_eq!(
        guard.validate_filter_chain(&bad_bcj_prop),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // Unsupported Filter ID (e.g. 0xFE)
    let unknown_filter = vec![
        XzFilterConfig::new(0xFE, vec![]),
        XzFilterConfig::new(FILTER_ID_LZMA2, vec![20]),
    ];
    assert_eq!(
        guard.validate_filter_chain(&unknown_filter),
        Err(TTZipStatus::ErrUnsupportedFeature)
    );

    // Valid composite filter chain: X86 BCJ + Delta + LZMA2
    let valid_composite = vec![
        XzFilterConfig::new(FILTER_ID_X86, vec![]),
        XzFilterConfig::new(FILTER_ID_DELTA, vec![0]),
        XzFilterConfig::new(FILTER_ID_LZMA2, vec![20]),
    ];
    assert_eq!(guard.validate_filter_chain(&valid_composite), Ok(()));
}
