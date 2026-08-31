// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration tests for XZ Block Header variable-length header parsing,
//! Filter Flag chain, 4-byte alignment padding, and adversarial corruption defense.

use ttzip_engine::crypto::crc32_fast;
use ttzip_engine::xz::block::{
    aligned_block_size, pad_to_4, XzBlockError, XzBlockHeader, XzFilterConfig, FILTER_ID_ARM,
    FILTER_ID_LZMA2, FILTER_ID_X86, MAX_BLOCK_HEADER_SIZE, MIN_BLOCK_HEADER_SIZE,
};
use ttzip_engine::xz::types::XzCheckType;
use ttzip_engine::xz::vli::VLI_MAX;


#[test]
fn test_4byte_padding_and_alignment_mathematical_invariants() {
    // 1. Exhaustive verification for small offsets
    for size in 0..=64u64 {
        let pad = pad_to_4(size);
        let aligned = aligned_block_size(size);

        assert!(pad < 4, "Padding must strictly be in [0, 3]");
        assert_eq!(
            (size + pad as u64) % 4,
            0,
            "size + pad must be a multiple of 4"
        );
        assert_eq!(
            aligned,
            size + pad as u64,
            "aligned_block_size must equal size + pad"
        );
        assert_eq!(
            aligned % 4,
            0,
            "aligned_block_size must be a multiple of 4"
        );
    }

    // 2. Exact boundary values
    assert_eq!(pad_to_4(0), 0);
    assert_eq!(pad_to_4(1), 3);
    assert_eq!(pad_to_4(2), 2);
    assert_eq!(pad_to_4(3), 1);
    assert_eq!(pad_to_4(4), 0);

    assert_eq!(aligned_block_size(0), 0);
    assert_eq!(aligned_block_size(1), 4);
    assert_eq!(aligned_block_size(2), 4);
    assert_eq!(aligned_block_size(3), 4);
    assert_eq!(aligned_block_size(4), 4);
    assert_eq!(aligned_block_size(5), 8);

    // 3. Large integer boundaries
    let large_size = 1_073_741_825u64; // 1 GB + 1 byte
    assert_eq!(pad_to_4(large_size), 3);
    assert_eq!(aligned_block_size(large_size), 1_073_741_828);

    let power_of_two = 1u64 << 30; // 1 GB
    assert_eq!(pad_to_4(power_of_two), 0);
    assert_eq!(aligned_block_size(power_of_two), power_of_two);
}

#[test]
fn test_block_header_single_filter_roundtrip() {
    let filter = XzFilterConfig::lzma2(8 * 1024 * 1024);
    let header = XzBlockHeader::new(vec![filter], XzCheckType::Crc32)
        .expect("create header")
        .with_sizes(Some(65536), Some(131072))
        .expect("with sizes");

    let encoded = header.encode().expect("encode");
    assert_eq!(encoded.len() % 4, 0);
    assert!(encoded.len() >= MIN_BLOCK_HEADER_SIZE);
    assert!(encoded.len() <= MAX_BLOCK_HEADER_SIZE);

    let parsed = XzBlockHeader::parse(&encoded, XzCheckType::Crc32).expect("parse header");
    assert_eq!(header.header_size, parsed.header_size);
    assert_eq!(header.compressed_size, parsed.compressed_size);
    assert_eq!(header.uncompressed_size, parsed.uncompressed_size);
    assert_eq!(header.filters, parsed.filters);
    assert_eq!(header.check_type, parsed.check_type);

    // Verify unpadded and total size computations
    assert_eq!(
        parsed.unpadded_size(),
        Some(parsed.header_size as u64 + 65536 + 4)
    );
    assert_eq!(
        parsed.total_block_size(),
        Some(parsed.header_size as u64 + aligned_block_size(65536) + 4)
    );
    assert_eq!(parsed.block_padding_size(), Some(0));
}

#[test]
fn test_block_header_bcj_lzma2_dual_filter_roundtrip() {
    let bcj = XzFilterConfig::bcj_x86(Some(0x1000));
    let lzma2 = XzFilterConfig::lzma2(16 * 1024 * 1024);

    let header = XzBlockHeader::new(vec![bcj, lzma2], XzCheckType::Crc64)
        .expect("create dual filter header")
        .with_sizes(Some(1001), Some(4000))
        .expect("with sizes");

    let encoded = header.encode().expect("encode");
    assert_eq!(encoded.len() % 4, 0);

    let parsed = XzBlockHeader::parse(&encoded, XzCheckType::Crc64).expect("parse dual filter");
    assert_eq!(parsed.filters.len(), 2);
    assert_eq!(parsed.filters[0].filter_id, FILTER_ID_X86);
    assert_eq!(parsed.filters[1].filter_id, FILTER_ID_LZMA2);
    assert_eq!(parsed.compressed_size, Some(1001));
    assert_eq!(parsed.uncompressed_size, Some(4000));
    assert_eq!(parsed.check_type, XzCheckType::Crc64);
    assert_eq!(parsed.check_size(), 8);

    // Padding for 1001 compressed bytes: 1001 % 4 = 1 -> pad = 3
    assert_eq!(parsed.block_padding_size(), Some(3));
    assert_eq!(
        parsed.total_block_size(),
        Some(parsed.header_size as u64 + 1004 + 8)
    );
}

#[test]
fn test_block_header_tri_and_quad_filter_roundtrip() {
    // 3 Filters: Delta + ARM + LZMA2
    let delta = XzFilterConfig::delta(4);
    let arm = XzFilterConfig::new(FILTER_ID_ARM, vec![]);
    let lzma2 = XzFilterConfig::lzma2(4 * 1024 * 1024);

    let header3 = XzBlockHeader::new(vec![delta.clone(), arm.clone(), lzma2.clone()], XzCheckType::Sha256)
        .expect("create tri filter header");
    let encoded3 = header3.encode().expect("encode tri filter");
    let parsed3 = XzBlockHeader::parse(&encoded3, XzCheckType::Sha256).expect("parse tri filter");
    assert_eq!(parsed3.filters.len(), 3);
    assert_eq!(parsed3.check_size(), 32);

    // 4 Filters (Maximum allowable in XZ specification)
    let x86 = XzFilterConfig::bcj_x86(None);
    let header4 = XzBlockHeader::new(vec![delta, arm, x86, lzma2], XzCheckType::None)
        .expect("create quad filter header")
        .with_sizes(Some(9999), None)
        .expect("with compressed size only");

    let encoded4 = header4.encode().expect("encode quad filter");
    let parsed4 = XzBlockHeader::parse(&encoded4, XzCheckType::None).expect("parse quad filter");
    assert_eq!(parsed4.filters.len(), 4);
    assert_eq!(parsed4.compressed_size, Some(9999));
    assert_eq!(parsed4.uncompressed_size, None);
    assert_eq!(parsed4.check_size(), 0);
}

#[test]
fn test_block_header_unknown_sizes_roundtrip() {
    let filter = XzFilterConfig::lzma2(2 * 1024 * 1024);
    let header = XzBlockHeader::new(vec![filter], XzCheckType::Crc32)
        .expect("create header with unknown sizes");

    assert_eq!(header.compressed_size, None);
    assert_eq!(header.uncompressed_size, None);

    let encoded = header.encode().expect("encode unknown sizes");
    let parsed = XzBlockHeader::parse(&encoded, XzCheckType::Crc32).expect("parse unknown sizes");

    assert_eq!(parsed.compressed_size, None);
    assert_eq!(parsed.uncompressed_size, None);
    assert_eq!(parsed.unpadded_size(), None);
    assert_eq!(parsed.total_block_size(), None);
    assert_eq!(parsed.block_padding_size(), None);
}

#[test]
fn test_block_header_nonzero_padding_defense() {
    let filter = XzFilterConfig::lzma2(8 * 1024 * 1024);
    let header = XzBlockHeader::new(vec![filter], XzCheckType::Crc32).expect("create header");

    let mut encoded = header.encode().expect("encode");
    let payload_len = encoded.len() - 4;

    // Find a padding byte in the payload
    // In minimal header (8 bytes total, 4 bytes payload + 4 bytes CRC):
    // Byte 0: Header Size (0x01 -> 8 bytes)
    // Byte 1: Block Flags (0x00 -> 1 filter, no sizes)
    // Byte 2: Filter ID (0x21)
    // Byte 3: Props Size (0x01)
    // Byte 4: Prop byte (e.g. dict size)
    // Real minimal header might need 12 bytes or padding. Let's inspect where padding is located.
    // Let's create a header with larger padding by specifying a larger header size if needed,
    // or locate the padding slice directly before CRC32.
    
    // We modify the last byte before CRC32 (which is padding) to 0x01
    let padding_idx = payload_len - 1;
    encoded[padding_idx] = 0x01;

    // 1. Direct parse without CRC update should fail with Crc32Mismatch
    let err = XzBlockHeader::parse(&encoded, XzCheckType::Crc32).unwrap_err();
    assert!(
        matches!(err, XzBlockError::Crc32Mismatch { .. }),
        "Tampered payload must trigger CRC mismatch, got: {:?}",
        err
    );

    // 2. Update CRC to match tampered payload: parser MUST detect NonZeroHeaderPadding!
    let new_crc = crc32_fast(0, &encoded[..payload_len]);
    encoded[payload_len..payload_len + 4].copy_from_slice(&new_crc.to_le_bytes());

    let err2 = XzBlockHeader::parse(&encoded, XzCheckType::Crc32).unwrap_err();
    assert_eq!(
        err2,
        XzBlockError::NonZeroHeaderPadding,
        "Header with non-zero padding byte must trigger NonZeroHeaderPadding error"
    );
}

#[test]
fn test_block_header_crc32_corruption_detection() {
    let filter = XzFilterConfig::lzma2(4 * 1024 * 1024);
    let header = XzBlockHeader::new(vec![filter], XzCheckType::Crc32).expect("header");
    let mut encoded = header.encode().expect("encode");

    // Corrupt one bit in the CRC field
    let len = encoded.len();
    encoded[len - 1] ^= 0x01;

    let err = XzBlockHeader::parse(&encoded, XzCheckType::Crc32).unwrap_err();
    match err {
        XzBlockError::Crc32Mismatch { expected, computed } => {
            assert_ne!(expected, computed);
        }
        other => panic!("Expected Crc32Mismatch, got {:?}", other),
    }
}

#[test]
fn test_block_header_reserved_flags_rejection() {
    let filter = XzFilterConfig::lzma2(4 * 1024 * 1024);
    let header = XzBlockHeader::new(vec![filter], XzCheckType::Crc32).expect("header");
    let mut encoded = header.encode().expect("encode");

    // Set bit 2 (reserved bit) in Block Flags (byte 1)
    encoded[1] |= 0x04;

    // Recalculate CRC to bypass CRC check and reach flag validation
    let payload_len = encoded.len() - 4;
    let new_crc = crc32_fast(0, &encoded[..payload_len]);
    encoded[payload_len..payload_len + 4].copy_from_slice(&new_crc.to_le_bytes());

    let err = XzBlockHeader::parse(&encoded, XzCheckType::Crc32).unwrap_err();
    assert_eq!(err, XzBlockError::ReservedFlagsSet(encoded[1]));
}

#[test]
fn test_block_header_truncated_input_handling() {
    assert_eq!(
        XzBlockHeader::parse(&[], XzCheckType::Crc32),
        Err(XzBlockError::UnexpectedEof)
    );
    assert_eq!(
        XzBlockHeader::parse(&[0x01, 0x00], XzCheckType::Crc32),
        Err(XzBlockError::UnexpectedEof)
    );
    assert_eq!(
        XzBlockHeader::parse(&[0x00; 7], XzCheckType::Crc32),
        Err(XzBlockError::UnexpectedEof)
    );
}

#[test]
fn test_block_header_invalid_filter_count() {
    // 0 filters
    let err0 = XzBlockHeader::new(vec![], XzCheckType::Crc32).unwrap_err();
    assert_eq!(err0, XzBlockError::InvalidFilterCount(0));

    // 5 filters (exceeds MAX_FILTER_COUNT = 4)
    let f = XzFilterConfig::lzma2(1024 * 1024);
    let err5 = XzBlockHeader::new(vec![f.clone(), f.clone(), f.clone(), f.clone(), f], XzCheckType::Crc32).unwrap_err();
    assert_eq!(err5, XzBlockError::InvalidFilterCount(5));
}

#[test]
fn test_block_header_large_vli_sizes() {
    let filter = XzFilterConfig::lzma2(32 * 1024 * 1024);
    let large_size = VLI_MAX - 1000;
    let header = XzBlockHeader::new(vec![filter], XzCheckType::Crc64)
        .expect("header")
        .with_sizes(Some(large_size), Some(large_size))
        .expect("with large sizes");

    let encoded = header.encode().expect("encode large sizes");
    let parsed = XzBlockHeader::parse(&encoded, XzCheckType::Crc64).expect("parse large sizes");

    assert_eq!(parsed.compressed_size, Some(large_size));
    assert_eq!(parsed.uncompressed_size, Some(large_size));
}
