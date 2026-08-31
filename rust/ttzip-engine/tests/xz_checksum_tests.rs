// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Exhaustive test suite for XZ CRC32, CRC64 ECMA-182, and SHA-256 checksum engine.

use ttzip_engine::xz::checksum::{
    crc64_xz, crc64_xz_update, XzChecksumEngine, XzChecksumError, XzChecksumType, XzCrc64,
};

#[test]
fn test_xz_crc64_official_test_vectors() {
    // Standard XZ / 7-Zip test vector: "123456789" -> 0x995DC9BBDF1939FA
    let input = b"123456789";
    let crc = crc64_xz(input);
    assert_eq!(crc, 0x995DC9BBDF1939FA);

    // Empty input -> 0
    assert_eq!(crc64_xz(b""), 0);

    // Single character vectors
    let single_a = crc64_xz(b"a");
    assert_ne!(single_a, 0);

    // Verify incremental update with seed
    let part1 = b"12345";
    let part2 = b"6789";
    let crc_part1 = crc64_xz(part1);
    let crc_combined = crc64_xz_update(crc_part1, part2);
    assert_eq!(crc_combined, 0x995DC9BBDF1939FA);
}

#[test]
fn test_streaming_chunking_equivalence_crc64() {
    let payload = b"TTZip high-performance archiving engine: testing CRC-64 ECMA-182 streaming equivalence!";
    let direct = crc64_xz(payload);

    // Test various arbitrary chunk sizes: 1, 3, 7, 8, 9, 16, 32
    for chunk_size in [1, 2, 3, 7, 8, 9, 11, 16, 23, 32, 64] {
        let mut hasher = XzCrc64::new();
        for chunk in payload.chunks(chunk_size) {
            hasher.update(chunk);
        }
        assert_eq!(
            hasher.finish(),
            direct,
            "Failed equivalence for chunk_size {}",
            chunk_size
        );
        assert_eq!(
            hasher.digest_bytes(),
            direct.to_le_bytes(),
            "Failed digest_bytes for chunk_size {}",
            chunk_size
        );
    }
}

#[test]
fn test_streaming_chunking_equivalence_all_types() {
    let payload = b"Comprehensive multi-megabyte payload simulation for all supported XZ checksum types."
        .repeat(50);

    let types = [
        XzChecksumType::None,
        XzChecksumType::Crc32,
        XzChecksumType::Crc64,
        XzChecksumType::Sha256,
    ];

    for check_type in types {
        let mut direct_engine = XzChecksumEngine::new(check_type);
        direct_engine.update(&payload);
        let expected_digest = direct_engine.digest();

        // 1. Verify verify() passes on correct digest
        assert!(
            direct_engine.verify(&expected_digest).is_ok(),
            "Engine verify failed for {:?}",
            check_type
        );

        // 2. Verify streaming chunked updates yield identical digest
        for chunk_size in [1, 7, 8, 15, 64, 256, 1024] {
            let mut chunked_engine = XzChecksumEngine::new(check_type);
            for chunk in payload.chunks(chunk_size) {
                chunked_engine.update(chunk);
            }
            let chunked_digest = chunked_engine.digest();
            assert_eq!(
                chunked_digest, expected_digest,
                "Chunked digest mismatch for {:?} with chunk size {}",
                check_type, chunk_size
            );
            assert!(chunked_engine.verify(&expected_digest).is_ok());
        }
    }
}

#[test]
fn test_checksum_mismatch_immediate_interception() {
    let payload = b"Safe error intercept verification without panics.";

    let types = [
        XzChecksumType::Crc32,
        XzChecksumType::Crc64,
        XzChecksumType::Sha256,
    ];

    for check_type in types {
        let mut engine = XzChecksumEngine::new(check_type);
        engine.update(payload);
        let mut valid_digest = engine.digest();

        // Corrupt first byte
        valid_digest[0] ^= 0x55;
        let err = engine.verify(&valid_digest);
        assert!(
            matches!(err, Err(XzChecksumError::ChecksumMismatch { .. })),
            "Expected ChecksumMismatch for {:?}, got {:?}",
            check_type,
            err
        );

        // Invalid length
        let mut short_digest = valid_digest.clone();
        short_digest.pop();
        let len_err = engine.verify(&short_digest);
        assert!(
            matches!(len_err, Err(XzChecksumError::InvalidDigestLength { .. })),
            "Expected InvalidDigestLength for {:?}, got {:?}",
            check_type,
            len_err
        );

        let mut long_digest = valid_digest.clone();
        long_digest.push(0x00);
        let long_len_err = engine.verify(&long_digest);
        assert!(
            matches!(
                long_len_err,
                Err(XzChecksumError::InvalidDigestLength { .. })
            ),
            "Expected InvalidDigestLength for {:?}, got {:?}",
            check_type,
            long_len_err
        );
    }
}

#[test]
fn test_check_type_id_parsing() {
    assert_eq!(XzChecksumType::from_id(0x00).unwrap(), XzChecksumType::None);
    assert_eq!(XzChecksumType::from_id(0x01).unwrap(), XzChecksumType::Crc32);
    assert_eq!(XzChecksumType::from_id(0x04).unwrap(), XzChecksumType::Crc64);
    assert_eq!(XzChecksumType::from_id(0x0A).unwrap(), XzChecksumType::Sha256);

    // Reserved / Unsupported IDs
    for id in [0x02, 0x03, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0xFF] {
        assert!(matches!(
            XzChecksumType::from_id(id),
            Err(XzChecksumError::UnsupportedCheckType(_))
        ));
    }
}

#[test]
fn test_engine_reset_and_zero_alloc_digest_into() {
    let payload_a = b"First batch of data";
    let payload_b = b"Second batch of data";

    let mut engine = XzChecksumEngine::crc64();
    engine.update(payload_a);
    let digest_a = engine.digest();

    engine.reset();
    engine.update(payload_b);
    let digest_b = engine.digest();

    assert_ne!(digest_a, digest_b);

    // Test zero-alloc digest_into
    let mut buf = [0u8; 8];
    let written = engine.digest_into(&mut buf).expect("digest_into");
    assert_eq!(written, 8);
    assert_eq!(&buf[..], &digest_b[..]);

    // Test short buffer in digest_into
    let mut small_buf = [0u8; 4];
    assert!(matches!(
        engine.digest_into(&mut small_buf),
        Err(XzChecksumError::InvalidDigestLength { .. })
    ));
}

#[test]
fn test_none_engine_behavior() {
    let mut engine = XzChecksumEngine::none();
    engine.update(b"Data does not affect None check");
    assert_eq!(engine.check_size(), 0);
    assert_eq!(engine.digest(), Vec::<u8>::new());
    assert!(engine.verify(&[]).is_ok());
    assert!(matches!(
        engine.verify(b"non-empty"),
        Err(XzChecksumError::InvalidDigestLength { .. })
    ));
}
