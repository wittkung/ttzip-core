// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration and property test suite for Libdeflate Zlib and Gzip container state machines.
//!
//! Validates:
//! 1. 100% Roundtrip fidelity for RFC 1950 (Zlib) across multiple sizes (0B, 1B, 100B, 64KB, 1MB) and compression levels (0..=12).
//! 2. 100% Roundtrip fidelity for RFC 1952 (Gzip) across multiple sizes (0B, 1B, 100B, 64KB, 1MB) and compression levels (0..=12).
//! 3. Adler-32 and CRC-32 checksum tampering and mismatch detection with strict error interception.
//! 4. Gzip ISIZE payload length tampering detection.
//! 5. Zero-Panic defense against malformed headers, illegal CMF/FLG bits, corrupted magic, and truncated extension fields.
//! 6. Gzip variable-length extension segments (`FEXTRA`, `FNAME`, `FCOMMENT`, `FHCRC`) decoding compliance and header CRC16 verification.
//! 7. Unified [`ContainerFormat`] generic dispatcher roundtrip consistency.

use ttzip_engine::codecs::libdeflate::checksum::{adler32_compute, crc32_compute};
use ttzip_engine::codecs::libdeflate::container::{
    compress_container, decompress_container, gzip_compress, gzip_compress_bound,
    gzip_compress_to_slice, gzip_decompress, zlib_compress, zlib_compress_bound,
    zlib_compress_to_slice, zlib_decompress, ContainerFormat, GZIP_CM_DEFLATE, GZIP_FCOMMENT,
    GZIP_FEXTRA, GZIP_FHCRC, GZIP_FNAME, GZIP_ID1, GZIP_ID2, GZIP_MIN_OVERHEAD,
    ZLIB_MIN_OVERHEAD,
};
use ttzip_engine::types::TTZipStatus;

/// Deterministic pseudo-random bytes generator using Linear Congruential Generator.
fn generate_deterministic_payload(len: usize, seed: u64) -> Vec<u8> {
    let mut state = seed;
    let mut buf = Vec::with_capacity(len);
    for _ in 0..len {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        buf.push((state >> 33) as u8);
    }
    buf
}

// ============================================================================
// 1. Zlib (RFC 1950) Roundtrip Fidelity Tests
// ============================================================================

#[test]
fn test_zlib_roundtrip_boundary_sizes() {
    let test_sizes = [
        0,                      // 0 Bytes (empty)
        1,                      // 1 Byte
        100,                    // 100 Bytes
        64 * 1024,              // 64 KB
        1024 * 1024,            // 1 MB
    ];

    let levels = [0, 1, 6, 9, 12];

    for &size in &test_sizes {
        let original = generate_deterministic_payload(size, 0x1950_ADE3_2000u64 ^ (size as u64));

        for &level in &levels {
            let compressed = zlib_compress(&original, level)
                .unwrap_or_else(|e| panic!("zlib_compress failed on size {} level {}: {:?}", size, level, e));

            assert!(
                compressed.len() >= ZLIB_MIN_OVERHEAD,
                "Compressed zlib stream smaller than min overhead ({} < {})",
                compressed.len(),
                ZLIB_MIN_OVERHEAD
            );

            // Verify header CMF byte: 0x78 (32K window, DEFLATE)
            assert_eq!(compressed[0], 0x78);

            // Verify FCHECK rule: (CMF * 256 + FLG) % 31 == 0
            let hdr = u16::from_be_bytes([compressed[0], compressed[1]]);
            assert_eq!(hdr % 31, 0, "Zlib FCHECK violation on level {}", level);

            // Verify footer Adler-32 in big-endian
            let expected_adler = adler32_compute(&original);
            let footer_start = compressed.len() - 4;
            let stored_adler = u32::from_be_bytes([
                compressed[footer_start],
                compressed[footer_start + 1],
                compressed[footer_start + 2],
                compressed[footer_start + 3],
            ]);
            assert_eq!(stored_adler, expected_adler, "Stored Adler-32 mismatch on size {}", size);

            // Decompress and verify bit-for-bit fidelity
            let mut decompressed = vec![0u8; original.len()];
            let decomp_len = zlib_decompress(&compressed, &mut decompressed)
                .unwrap_or_else(|e| panic!("zlib_decompress failed on size {} level {}: {:?}", size, level, e));

            assert_eq!(decomp_len, original.len());
            assert_eq!(decompressed, original, "Zlib roundtrip payload mismatch on size {}", size);
        }
    }
}

#[test]
fn test_zlib_compress_to_slice_buffer() {
    let original = b"High-throughput zero-copy zlib slice compression test buffer for TTZip.";
    let bound = zlib_compress_bound(original.len(), 6);
    let mut comp_buf = vec![0u8; bound];

    let written = zlib_compress_to_slice(original, &mut comp_buf, 6)
        .expect("zlib_compress_to_slice failed");
    assert!(written <= bound);

    let mut decomp_buf = vec![0u8; original.len()];
    let decomp_len = zlib_decompress(&comp_buf[..written], &mut decomp_buf)
        .expect("zlib_decompress failed");
    assert_eq!(decomp_len, original.len());
    assert_eq!(&decomp_buf[..decomp_len], original);
}

// ============================================================================
// 2. Gzip (RFC 1952) Roundtrip Fidelity Tests
// ============================================================================

#[test]
fn test_gzip_roundtrip_boundary_sizes() {
    let test_sizes = [
        0,                      // 0 Bytes (empty)
        1,                      // 1 Byte
        100,                    // 100 Bytes
        64 * 1024,              // 64 KB
        1024 * 1024,            // 1 MB
    ];

    let levels = [0, 1, 6, 9, 12];

    for &size in &test_sizes {
        let original = generate_deterministic_payload(size, 0x1952_6219_0000u64 ^ (size as u64));

        for &level in &levels {
            let compressed = gzip_compress(&original, level)
                .unwrap_or_else(|e| panic!("gzip_compress failed on size {} level {}: {:?}", size, level, e));

            assert!(
                compressed.len() >= GZIP_MIN_OVERHEAD,
                "Compressed gzip stream smaller than min overhead ({} < {})",
                compressed.len(),
                GZIP_MIN_OVERHEAD
            );

            // Validate fixed header
            assert_eq!(compressed[0], GZIP_ID1, "GZIP ID1 mismatch");
            assert_eq!(compressed[1], GZIP_ID2, "GZIP ID2 mismatch");
            assert_eq!(compressed[2], GZIP_CM_DEFLATE, "GZIP CM mismatch");
            assert_eq!(compressed[3], 0, "GZIP default FLG should be 0");

            // Validate CRC-32 (little-endian)
            let expected_crc = crc32_compute(&original);
            let footer_start = compressed.len() - 8;
            let stored_crc = u32::from_le_bytes([
                compressed[footer_start],
                compressed[footer_start + 1],
                compressed[footer_start + 2],
                compressed[footer_start + 3],
            ]);
            assert_eq!(stored_crc, expected_crc, "Stored CRC-32 mismatch on size {}", size);

            // Validate ISIZE (little-endian modulo 2^32)
            let stored_isize = u32::from_le_bytes([
                compressed[footer_start + 4],
                compressed[footer_start + 5],
                compressed[footer_start + 6],
                compressed[footer_start + 7],
            ]);
            assert_eq!(stored_isize, (original.len() as u32), "Stored ISIZE mismatch on size {}", size);

            // Decompress and verify bit-for-bit fidelity
            let mut decompressed = vec![0u8; original.len()];
            let decomp_len = gzip_decompress(&compressed, &mut decompressed)
                .unwrap_or_else(|e| panic!("gzip_decompress failed on size {} level {}: {:?}", size, level, e));

            assert_eq!(decomp_len, original.len());
            assert_eq!(decompressed, original, "Gzip roundtrip payload mismatch on size {}", size);
        }
    }
}

#[test]
fn test_gzip_compress_to_slice_buffer() {
    let original = b"RFC 1952 Gzip zero-copy slice stream testing payload.";
    let bound = gzip_compress_bound(original.len(), 6);
    let mut comp_buf = vec![0u8; bound];

    let written = gzip_compress_to_slice(original, &mut comp_buf, 6)
        .expect("gzip_compress_to_slice failed");
    assert!(written <= bound);

    let mut decomp_buf = vec![0u8; original.len()];
    let decomp_len = gzip_decompress(&comp_buf[..written], &mut decomp_buf)
        .expect("gzip_decompress failed");
    assert_eq!(decomp_len, original.len());
    assert_eq!(&decomp_buf[..decomp_len], original);
}

// ============================================================================
// 3. Checksum Tampering & Interception Tests
// ============================================================================

#[test]
fn test_zlib_adler32_tamper_interception() {
    let original = b"The quick brown fox jumps over the lazy dog";
    let mut compressed = zlib_compress(original, 6).unwrap();

    // Tamper with Adler-32 checksum in footer
    let last_idx = compressed.len() - 1;
    compressed[last_idx] ^= 0xFF;

    let mut decomp = vec![0u8; original.len()];
    let res = zlib_decompress(&compressed, &mut decomp);
    assert_eq!(res, Err(TTZipStatus::ErrCorruptHeader));
}

#[test]
fn test_gzip_crc32_tamper_interception() {
    let original = b"The quick brown fox jumps over the lazy dog";
    let mut compressed = gzip_compress(original, 6).unwrap();

    // Tamper with CRC-32 in footer (offset len - 8)
    let crc_idx = compressed.len() - 8;
    compressed[crc_idx] ^= 0xFF;

    let mut decomp = vec![0u8; original.len()];
    let res = gzip_decompress(&compressed, &mut decomp);
    assert_eq!(res, Err(TTZipStatus::ErrCorruptHeader));
}

#[test]
fn test_gzip_isize_tamper_interception() {
    let original = b"Testing GZIP ISIZE tampering rejection";
    let mut compressed = gzip_compress(original, 6).unwrap();

    // Tamper with ISIZE in footer (offset len - 4)
    let isize_idx = compressed.len() - 4;
    compressed[isize_idx] ^= 0x01;

    let mut decomp = vec![0u8; original.len()];
    let res = gzip_decompress(&compressed, &mut decomp);
    assert_eq!(res, Err(TTZipStatus::ErrCorruptHeader));
}

// ============================================================================
// 4. Corrupted Header & Fuzzing Resilience Tests (0 Panic)
// ============================================================================

#[test]
fn test_zlib_corrupted_headers() {
    let original = b"Header corruption test data for zlib";
    let valid_compressed = zlib_compress(original, 6).unwrap();

    // Truncated streams
    let mut decomp = vec![0u8; 100];
    assert_eq!(zlib_decompress(&[], &mut decomp), Err(TTZipStatus::ErrCorruptHeader));
    assert_eq!(zlib_decompress(&valid_compressed[..3], &mut decomp), Err(TTZipStatus::ErrCorruptHeader));
    assert_eq!(zlib_decompress(&valid_compressed[..5], &mut decomp), Err(TTZipStatus::ErrCorruptHeader));

    // Invalid CMF compression method (not DEFLATE 8)
    let mut bad_cm = valid_compressed.clone();
    bad_cm[0] = 0x79; // CM = 9
    assert_eq!(zlib_decompress(&bad_cm, &mut decomp), Err(TTZipStatus::ErrCorruptHeader));

    // Invalid CINFO window size (> 7)
    let mut bad_cinfo = valid_compressed.clone();
    bad_cinfo[0] = 0x88; // CINFO = 8
    assert_eq!(zlib_decompress(&bad_cinfo, &mut decomp), Err(TTZipStatus::ErrCorruptHeader));

    // Invalid FCHECK (tampered FLG without % 31 alignment)
    let mut bad_fcheck = valid_compressed.clone();
    bad_fcheck[1] ^= 0x01;
    assert_eq!(zlib_decompress(&bad_fcheck, &mut decomp), Err(TTZipStatus::ErrCorruptHeader));

    // FDICT flag set (unsupported preset dict: CMF=0x78, FLG=0x20 => 0x7820 % 31 == 0, FDICT=1)
    let mut fdict = valid_compressed.clone();
    fdict[0] = 0x78;
    fdict[1] = 0x20;
    assert_eq!(zlib_decompress(&fdict, &mut decomp), Err(TTZipStatus::ErrCorruptHeader));
}

#[test]
fn test_gzip_corrupted_headers() {
    let original = b"Header corruption test data for gzip";
    let valid_compressed = gzip_compress(original, 6).unwrap();

    // Truncated streams
    let mut decomp = vec![0u8; 100];
    assert_eq!(gzip_decompress(&[], &mut decomp), Err(TTZipStatus::ErrCorruptHeader));
    assert_eq!(gzip_decompress(&valid_compressed[..9], &mut decomp), Err(TTZipStatus::ErrCorruptHeader));
    assert_eq!(gzip_decompress(&valid_compressed[..17], &mut decomp), Err(TTZipStatus::ErrCorruptHeader));

    // Invalid Magic ID1/ID2
    let mut bad_id1 = valid_compressed.clone();
    bad_id1[0] = 0x00;
    assert_eq!(gzip_decompress(&bad_id1, &mut decomp), Err(TTZipStatus::ErrCorruptHeader));

    let mut bad_id2 = valid_compressed.clone();
    bad_id2[1] = 0x00;
    assert_eq!(gzip_decompress(&bad_id2, &mut decomp), Err(TTZipStatus::ErrCorruptHeader));

    // Invalid Compression Method (not 8)
    let mut bad_cm = valid_compressed.clone();
    bad_cm[2] = 0x07;
    assert_eq!(gzip_decompress(&bad_cm, &mut decomp), Err(TTZipStatus::ErrCorruptHeader));

    // Reserved flags set (bits 5..7)
    let mut bad_flg = valid_compressed.clone();
    bad_flg[3] = 0x80;
    assert_eq!(gzip_decompress(&bad_flg, &mut decomp), Err(TTZipStatus::ErrCorruptHeader));
}

// ============================================================================
// 5. Gzip Variable-Length Extensions Compliance Tests
// ============================================================================

#[test]
fn test_gzip_variable_length_extensions() {
    let original = b"Comprehensive gzip header extensions testing payload.";
    let base_compressed = gzip_compress(original, 6).unwrap();
    let deflate_and_footer = &base_compressed[10..];

    // Construct custom Gzip stream with FEXTRA + FNAME + FCOMMENT + FHCRC
    let mut custom_gzip = Vec::new();
    custom_gzip.push(GZIP_ID1);
    custom_gzip.push(GZIP_ID2);
    custom_gzip.push(GZIP_CM_DEFLATE);

    let flg = GZIP_FEXTRA | GZIP_FNAME | GZIP_FCOMMENT | GZIP_FHCRC;
    custom_gzip.push(flg);
    custom_gzip.extend_from_slice(&[0u8; 4]); // MTIME = 0
    custom_gzip.push(0); // XFL = 0
    custom_gzip.push(255); // OS = 255

    // 1. FEXTRA: 4 bytes extra field
    let extra_data = b"ZIP6";
    let xlen = extra_data.len() as u16;
    custom_gzip.extend_from_slice(&xlen.to_le_bytes());
    custom_gzip.extend_from_slice(extra_data);

    // 2. FNAME: "archive.tar\0"
    let filename = b"archive.tar\0";
    custom_gzip.extend_from_slice(filename);

    // 3. FCOMMENT: "Created by TTZip engine\0"
    let comment = b"Created by TTZip engine\0";
    custom_gzip.extend_from_slice(comment);

    // 4. FHCRC: 16-bit CRC of header bytes up to this point
    let header_crc32 = crc32_compute(&custom_gzip);
    let header_crc16 = (header_crc32 & 0xFFFF) as u16;
    custom_gzip.extend_from_slice(&header_crc16.to_le_bytes());

    // Append Deflate payload and 8-byte footer
    custom_gzip.extend_from_slice(deflate_and_footer);

    // Decompress and verify full compliance
    let mut decomp = vec![0u8; original.len()];
    let decomp_len = gzip_decompress(&custom_gzip, &mut decomp)
        .expect("gzip_decompress failed with valid header extensions");
    assert_eq!(decomp_len, original.len());
    assert_eq!(&decomp[..decomp_len], original);

    // Test malformed FHCRC mismatch detection
    let mut bad_fhcrc_stream = custom_gzip.clone();
    let fhcrc_offset = 10 + 2 + extra_data.len() + filename.len() + comment.len();
    bad_fhcrc_stream[fhcrc_offset] ^= 0xFF;
    assert_eq!(
        gzip_decompress(&bad_fhcrc_stream, &mut decomp),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // Test truncated filename (missing null terminator)
    let mut unterminated_name_stream = custom_gzip.clone();
    let name_null_offset = 10 + 2 + extra_data.len() + filename.len() - 1;
    unterminated_name_stream[name_null_offset] = b'X';
    assert_eq!(
        gzip_decompress(&unterminated_name_stream, &mut decomp),
        Err(TTZipStatus::ErrCorruptHeader)
    );
}

// ============================================================================
// 6. Generic Container Dispatcher Tests
// ============================================================================

#[test]
fn test_generic_container_dispatch() {
    let original = b"Unified ContainerFormat generic dispatcher roundtrip verification.";
    let formats = [ContainerFormat::Raw, ContainerFormat::Zlib, ContainerFormat::Gzip];

    for &fmt in &formats {
        let compressed = compress_container(original, fmt, 6)
            .unwrap_or_else(|e| panic!("compress_container failed on format {:?}: {:?}", fmt, e));

        let mut decomp = vec![0u8; original.len()];
        let decomp_len = decompress_container(&compressed, &mut decomp, fmt)
            .unwrap_or_else(|e| panic!("decompress_container failed on format {:?}: {:?}", fmt, e));

        assert_eq!(decomp_len, original.len());
        assert_eq!(&decomp[..decomp_len], original);
    }
}
