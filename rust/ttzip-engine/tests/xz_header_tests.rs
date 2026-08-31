// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit and integration test suite for XZ Stream Header, Stream Footer,
//! Stream Flags, 12-byte fixed memory geometry, and error resilience.

use std::mem::size_of;
use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::xz::{
    compare_stream_flags, XzCheckType, XzError, XzRawStreamFooter, XzRawStreamHeader,
    XzStreamFlags, XzStreamFooter, XzStreamHeader, LEN_FOOTER_BACKWARD_SIZE, LEN_FOOTER_CRC,
    LEN_FOOTER_FLAGS, LEN_FOOTER_MAGIC, LEN_HEADER_CRC, LEN_HEADER_FLAGS, LEN_HEADER_MAGIC,
    OFFSET_FOOTER_BACKWARD_SIZE, OFFSET_FOOTER_CRC, OFFSET_FOOTER_FLAGS, OFFSET_FOOTER_MAGIC,
    OFFSET_HEADER_CRC, OFFSET_HEADER_FLAGS, OFFSET_HEADER_MAGIC, XZ_BACKWARD_SIZE_UNIT,
    XZ_FOOTER_MAGIC, XZ_HEADER_MAGIC, XZ_MAX_BACKWARD_SIZE, XZ_MIN_BACKWARD_SIZE,
    XZ_STREAM_FOOTER_SIZE, XZ_STREAM_HEADER_SIZE,
};

#[test]
fn test_xz_stream_geometry_and_offsets() {
    assert_eq!(size_of::<XzRawStreamHeader>(), 12);
    assert_eq!(size_of::<XzRawStreamFooter>(), 12);
    assert_eq!(XZ_STREAM_HEADER_SIZE, 12);
    assert_eq!(XZ_STREAM_FOOTER_SIZE, 12);

    // Stream Header offsets and lengths
    assert_eq!(OFFSET_HEADER_MAGIC, 0);
    assert_eq!(LEN_HEADER_MAGIC, 6);
    assert_eq!(OFFSET_HEADER_FLAGS, 6);
    assert_eq!(LEN_HEADER_FLAGS, 2);
    assert_eq!(OFFSET_HEADER_CRC, 8);
    assert_eq!(LEN_HEADER_CRC, 4);

    assert_eq!(OFFSET_HEADER_MAGIC + LEN_HEADER_MAGIC, OFFSET_HEADER_FLAGS);
    assert_eq!(OFFSET_HEADER_FLAGS + LEN_HEADER_FLAGS, OFFSET_HEADER_CRC);
    assert_eq!(OFFSET_HEADER_CRC + LEN_HEADER_CRC, XZ_STREAM_HEADER_SIZE);

    // Stream Footer offsets and lengths
    assert_eq!(OFFSET_FOOTER_CRC, 0);
    assert_eq!(LEN_FOOTER_CRC, 4);
    assert_eq!(OFFSET_FOOTER_BACKWARD_SIZE, 4);
    assert_eq!(LEN_FOOTER_BACKWARD_SIZE, 4);
    assert_eq!(OFFSET_FOOTER_FLAGS, 8);
    assert_eq!(LEN_FOOTER_FLAGS, 2);
    assert_eq!(OFFSET_FOOTER_MAGIC, 10);
    assert_eq!(LEN_FOOTER_MAGIC, 2);

    assert_eq!(OFFSET_FOOTER_CRC + LEN_FOOTER_CRC, OFFSET_FOOTER_BACKWARD_SIZE);
    assert_eq!(
        OFFSET_FOOTER_BACKWARD_SIZE + LEN_FOOTER_BACKWARD_SIZE,
        OFFSET_FOOTER_FLAGS
    );
    assert_eq!(OFFSET_FOOTER_FLAGS + LEN_FOOTER_FLAGS, OFFSET_FOOTER_MAGIC);
    assert_eq!(OFFSET_FOOTER_MAGIC + LEN_FOOTER_MAGIC, XZ_STREAM_FOOTER_SIZE);
}

#[test]
fn test_xz_check_type_spec_conformance() {
    let check_types = [
        (XzCheckType::None, 0x00u8, 0usize),
        (XzCheckType::Crc32, 0x01u8, 4usize),
        (XzCheckType::Crc64, 0x04u8, 8usize),
        (XzCheckType::Sha256, 0x0Au8, 32usize),
    ];

    for (check_type, expected_id, expected_size) in check_types {
        assert_eq!(check_type.id(), expected_id);
        assert_eq!(check_type.check_size(), expected_size);
        assert_eq!(XzCheckType::from_id(expected_id).unwrap(), check_type);
    }

    // Reserved and unallocated IDs should be rejected with UnsupportedCheckType
    let unsupported_ids = [
        0x02, 0x03, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x7F,
        0xFF,
    ];
    for &id in &unsupported_ids {
        assert_eq!(
            XzCheckType::from_id(id),
            Err(XzError::UnsupportedCheckType(id))
        );
    }
}

#[test]
fn test_xz_stream_header_roundtrip_all_check_types() {
    let check_types = [
        XzCheckType::None,
        XzCheckType::Crc32,
        XzCheckType::Crc64,
        XzCheckType::Sha256,
    ];

    for check_type in check_types {
        let flags = XzStreamFlags::new(check_type);
        let header = XzStreamHeader::new(flags);

        let encoded = header.encode();
        assert_eq!(encoded.len(), XZ_STREAM_HEADER_SIZE);

        // Verify magic bytes
        assert_eq!(&encoded[0..6], &XZ_HEADER_MAGIC);

        // Verify stream flags bytes
        assert_eq!(encoded[6], 0x00);
        assert_eq!(encoded[7], check_type.id());

        // Verify CRC32
        let expected_crc = crc32_fast(0, &encoded[6..8]);
        let actual_crc = u32::from_le_bytes(encoded[8..12].try_into().unwrap());
        assert_eq!(expected_crc, actual_crc);

        // Parse and verify 100% roundtrip fidelity
        let parsed = XzStreamHeader::parse(&encoded).expect("Valid header must parse");
        assert_eq!(parsed, header);
        assert_eq!(parsed.flags.check_type, check_type);
    }
}

#[test]
fn test_xz_stream_footer_roundtrip_and_backward_size() {
    let check_types = [
        XzCheckType::None,
        XzCheckType::Crc32,
        XzCheckType::Crc64,
        XzCheckType::Sha256,
    ];

    let test_sizes: &[u64] = &[
        XZ_MIN_BACKWARD_SIZE,
        8,
        12,
        16,
        1024,
        65536,
        1048576,
        1_073_741_824,
        XZ_MAX_BACKWARD_SIZE,
    ];

    for check_type in check_types {
        for &size in test_sizes {
            let flags = XzStreamFlags::new(check_type);
            let footer = XzStreamFooter::new(flags, size);

            let encoded = footer.encode(size).expect("Valid size must encode");
            assert_eq!(encoded.len(), XZ_STREAM_FOOTER_SIZE);

            // Verify footer magic
            assert_eq!(&encoded[10..12], &XZ_FOOTER_MAGIC);

            // Verify stored backward size calculation: (real_size / 4) - 1
            let expected_stored_size = ((size / XZ_BACKWARD_SIZE_UNIT) - 1) as u32;
            let actual_stored_size = u32::from_le_bytes(encoded[4..8].try_into().unwrap());
            assert_eq!(expected_stored_size, actual_stored_size);

            // Verify stream flags
            assert_eq!(encoded[8], 0x00);
            assert_eq!(encoded[9], check_type.id());

            // Verify CRC32 is calculated across Backward Size + Stream Flags (6 bytes)
            let expected_crc = crc32_fast(0, &encoded[4..10]);
            let actual_crc = u32::from_le_bytes(encoded[0..4].try_into().unwrap());
            assert_eq!(expected_crc, actual_crc);

            // Parse and verify 100% roundtrip fidelity
            let parsed = XzStreamFooter::parse(&encoded).expect("Valid footer must parse");
            assert_eq!(parsed.flags, footer.flags);
            assert_eq!(parsed.backward_size, size);

            // Verify encode_self matches
            let self_encoded = footer
                .encode_self()
                .expect("encode_self on valid instance must succeed");
            assert_eq!(self_encoded, encoded);
        }
    }
}

#[test]
fn test_stream_flags_reserved_bits_rejection() {
    // 1. Byte 0 is non-zero
    let bad_byte0_flags = [[0x01, 0x01], [0x80, 0x04], [0xFF, 0x0A], [0x42, 0x00]];
    for raw in bad_byte0_flags {
        let err = XzStreamFlags::parse(raw).unwrap_err();
        assert!(
            matches!(err, XzError::ReservedFlagsNonZero { byte0, .. } if byte0 == raw[0]),
            "Byte0 non-zero must trigger ReservedFlagsNonZero: {:?}",
            raw
        );
    }

    // 2. Byte 1 upper 4 bits are non-zero
    let bad_reserved_bits = [
        [0x00, 0x10],
        [0x00, 0x24],
        [0x00, 0xF0],
        [0x00, 0x81],
        [0x00, 0xA0],
    ];
    for raw in bad_reserved_bits {
        let err = XzStreamFlags::parse(raw).unwrap_err();
        assert!(
            matches!(err, XzError::ReservedFlagsNonZero { reserved_bits, .. } if reserved_bits == (raw[1] & 0xF0)),
            "Byte1 reserved bits non-zero must trigger ReservedFlagsNonZero: {:?}",
            raw
        );
    }
}

#[test]
fn test_stream_header_bad_magic_and_crc_rejection() {
    let header = XzStreamHeader::new(XzStreamFlags::new(XzCheckType::Crc32));
    let valid_bytes = header.encode();

    // 1. Mutate magic bytes
    for i in 0..6 {
        let mut corrupted = valid_bytes;
        corrupted[i] ^= 0xFF;
        // Even with CRC matching the corrupted slice, magic check MUST fail first
        let err = XzStreamHeader::parse(&corrupted).unwrap_err();
        assert!(
            matches!(err, XzError::InvalidHeaderMagic { .. }),
            "Corrupted magic byte at index {} must fail with InvalidHeaderMagic",
            i
        );
    }

    // 2. Mutate CRC bytes
    for i in 8..12 {
        let mut corrupted = valid_bytes;
        corrupted[i] ^= 0x55;
        let err = XzStreamHeader::parse(&corrupted).unwrap_err();
        assert!(
            matches!(err, XzError::HeaderCrcMismatch { .. }),
            "Corrupted CRC byte at index {} must fail with HeaderCrcMismatch",
            i
        );
    }

    // 3. Mutate flags without updating CRC
    let mut corrupted_flags = valid_bytes;
    corrupted_flags[7] = XzCheckType::Sha256.id(); // CRC not updated
    let err = XzStreamHeader::parse(&corrupted_flags).unwrap_err();
    assert!(
        matches!(err, XzError::HeaderCrcMismatch { .. }),
        "Mismatching CRC after flag mutation must be caught"
    );
}

#[test]
fn test_stream_footer_bad_magic_and_crc_rejection() {
    let footer = XzStreamFooter::new(XzStreamFlags::new(XzCheckType::Crc64), 1024);
    let valid_bytes = footer.encode_self().unwrap();

    // 1. Mutate footer magic bytes
    for i in 10..12 {
        let mut corrupted = valid_bytes;
        corrupted[i] ^= 0xAA;
        let err = XzStreamFooter::parse(&corrupted).unwrap_err();
        assert!(
            matches!(err, XzError::InvalidFooterMagic { .. }),
            "Corrupted footer magic byte at index {} must fail with InvalidFooterMagic",
            i
        );
    }

    // 2. Mutate footer CRC bytes
    for i in 0..4 {
        let mut corrupted = valid_bytes;
        corrupted[i] ^= 0x33;
        let err = XzStreamFooter::parse(&corrupted).unwrap_err();
        assert!(
            matches!(err, XzError::FooterCrcMismatch { .. }),
            "Corrupted footer CRC byte at index {} must fail with FooterCrcMismatch",
            i
        );
    }

    // 3. Mutate backward size without updating CRC
    let mut corrupted_size = valid_bytes;
    corrupted_size[4] ^= 0x01;
    let err = XzStreamFooter::parse(&corrupted_size).unwrap_err();
    assert!(
        matches!(err, XzError::FooterCrcMismatch { .. }),
        "Mismatching footer CRC after size mutation must be caught"
    );
}

#[test]
fn test_invalid_backward_size_bounds_and_alignment() {
    let footer = XzStreamFooter::new(XzStreamFlags::new(XzCheckType::Crc32), 4);

    // Size < 4
    assert_eq!(
        footer.encode(0),
        Err(XzError::InvalidBackwardSize(0))
    );
    assert_eq!(
        footer.encode(1),
        Err(XzError::InvalidBackwardSize(1))
    );
    assert_eq!(
        footer.encode(2),
        Err(XzError::InvalidBackwardSize(2))
    );
    assert_eq!(
        footer.encode(3),
        Err(XzError::InvalidBackwardSize(3))
    );

    // Non-multiples of 4
    assert_eq!(
        footer.encode(5),
        Err(XzError::InvalidBackwardSize(5))
    );
    assert_eq!(
        footer.encode(6),
        Err(XzError::InvalidBackwardSize(6))
    );
    assert_eq!(
        footer.encode(7),
        Err(XzError::InvalidBackwardSize(7))
    );
    assert_eq!(
        footer.encode(1025),
        Err(XzError::InvalidBackwardSize(1025))
    );

    // Size > XZ_MAX_BACKWARD_SIZE
    let overflow_size = XZ_MAX_BACKWARD_SIZE + 4;
    assert_eq!(
        footer.encode(overflow_size),
        Err(XzError::InvalidBackwardSize(overflow_size))
    );
}

#[test]
fn test_stream_flags_parity_and_comparison() {
    let flags_crc32 = XzStreamFlags::new(XzCheckType::Crc32);
    let flags_sha256 = XzStreamFlags::new(XzCheckType::Sha256);
    let flags_crc32_clone = XzStreamFlags::new(XzCheckType::Crc32);

    assert!(compare_stream_flags(&flags_crc32, &flags_crc32_clone));
    assert!(!compare_stream_flags(&flags_crc32, &flags_sha256));

    let header = XzStreamHeader::new(flags_crc32);
    let footer_matching = XzStreamFooter::new(flags_crc32, 1024);
    let footer_mismatching = XzStreamFooter::new(flags_sha256, 1024);

    assert!(footer_matching.verify_flags(&header.flags).is_ok());
    let mismatch_err = footer_mismatching
        .verify_flags(&header.flags)
        .unwrap_err();
    assert_eq!(
        mismatch_err,
        XzError::FlagsMismatch {
            header: flags_crc32,
            footer: flags_sha256,
        }
    );

    let footer_matching_bytes = footer_matching.encode_self().unwrap();
    let footer_mismatching_bytes = footer_mismatching.encode_self().unwrap();

    let verified =
        XzStreamFooter::parse_and_verify_header(&footer_matching_bytes, &header.flags)
            .expect("Matching flags must parse and verify");
    assert_eq!(verified, footer_matching);

    let failed =
        XzStreamFooter::parse_and_verify_header(&footer_mismatching_bytes, &header.flags)
            .unwrap_err();
    assert_eq!(
        failed,
        XzError::FlagsMismatch {
            header: flags_crc32,
            footer: flags_sha256,
        }
    );
}

#[test]
fn test_adversarial_arbitrary_bytes_zero_panic() {
    let test_patterns: Vec<[u8; 12]> = vec![
        [0u8; 12],
        [0xFFu8; 12],
        [0xAAu8; 12],
        [0x55u8; 12],
        [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C],
    ];

    for pattern in test_patterns {
        let _ = XzStreamHeader::parse(&pattern);
        let _ = XzStreamFooter::parse(&pattern);
    }
}
