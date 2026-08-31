// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit, edge-case, differential, and fuzz tests for 7z Varint (`Real_UINT64`) codec.

use std::io::Cursor;
use ttzip_engine::sevenz::varint::{
    decode_7z_varint, encode_7z_varint, encode_7z_varint_vec, read_variable_u64,
    read_variable_usize, try_encode_7z_varint, varint_size_7z, write_variable_u64,
    VarintError, K_ADDITIONAL_STREAMS_INFO, K_ANTI, K_ARCHIVE_PROPERTIES, K_ATIME,
    K_CODERS_UNPACK_SIZE, K_COMMENT, K_CRC, K_CTIME, K_DUMMY, K_EMPTY_FILE,
    K_EMPTY_STREAM, K_ENCODED_HEADER, K_END, K_FILES_INFO, K_FOLDER, K_HEADER,
    K_MAIN_STREAMS_INFO, K_MTIME, K_NAME, K_NUM_UNPACK_STREAM, K_PACK_INFO, K_SIZE,
    K_START_EDIT_HEADER, K_SUB_STREAMS_INFO, K_UNPACK_INFO, K_WIN_ATTRIBUTES,
    MAX_VARINT_LEN_7Z,
};
use ttzip_engine::sevenz::{read_varint, write_varint};

#[test]
fn test_specific_edge_boundaries_and_exact_encodings() {
    let cases: &[(u64, usize, &[u8])] = &[
        (0, 1, &[0x00]),
        (1, 1, &[0x01]),
        (127, 1, &[0x7F]),
        (128, 2, &[0x80, 0x80]),
        (255, 2, &[0x80, 0xFF]),
        (256, 2, &[0x81, 0x00]),
        (16383, 2, &[0xBF, 0xFF]),
        (16384, 3, &[0xC0, 0x00, 0x40]),
        (0x1FFFFF, 3, &[0xDF, 0xFF, 0xFF]),
        (0x200000, 4, &[0xE0, 0x00, 0x00, 0x20]),
        (0x3FFFFF, 4, &[0xE0, 0xFF, 0xFF, 0x3F]),
        (0x0FFFFFFF, 4, &[0xEF, 0xFF, 0xFF, 0xFF]),
        (0x10000000, 5, &[0xF0, 0x00, 0x00, 0x00, 0x10]),
        (0x1FFFFFFF, 5, &[0xF0, 0xFF, 0xFF, 0xFF, 0x1F]),
        (0x00000007_FFFFFFFF, 5, &[0xF7, 0xFF, 0xFF, 0xFF, 0xFF]),
        (0x00000008_00000000, 6, &[0xF8, 0x00, 0x00, 0x00, 0x00, 0x08]),
        (0x000003FF_FFFFFFFF, 6, &[0xFB, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
        (0x00000400_00000000, 7, &[0xFC, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04]),
        (0x0001FFFF_FFFFFFFF, 7, &[0xFD, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
        (0x00020000_00000000, 8, &[0xFE, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02]),
        (0x00FFFFFF_FFFFFFFF, 8, &[0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
        (0x01000000_00000000, 9, &[0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
        (u64::MAX, 9, &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
    ];

    for &(val, expected_len, expected_bytes) in cases {
        assert_eq!(varint_size_7z(val), expected_len, "varint_size_7z for {}", val);

        let mut buf = [0u8; MAX_VARINT_LEN_7Z];
        let written = encode_7z_varint(val, &mut buf);
        assert_eq!(written, expected_len, "written len for {}", val);
        assert_eq!(&buf[..written], expected_bytes, "byte pattern mismatch for {}", val);

        let (decoded, consumed) = decode_7z_varint(&buf[..written]).expect("decode must succeed");
        assert_eq!(decoded, val, "decoded value mismatch");
        assert_eq!(consumed, expected_len, "consumed bytes mismatch");

        // Format bridge verification
        let (bridge_decoded, bridge_consumed) = read_varint(&buf[..written]).expect("read_varint must succeed");
        assert_eq!(bridge_decoded, val);
        assert_eq!(bridge_consumed, expected_len);

        let mut vec_buf = Vec::new();
        write_varint(val, &mut vec_buf);
        assert_eq!(&vec_buf, expected_bytes);
    }
}

#[test]
fn test_all_bit_transition_boundaries() {
    // Test (1 << b) - 1, 1 << b, (1 << b) + 1 for each bit from 0 to 63
    for bit in 0..64 {
        let base = 1u64 << bit;
        let candidates = [
            base.saturating_sub(1),
            base,
            base.saturating_add(1),
        ];

        for &val in &candidates {
            let size = varint_size_7z(val);
            let mut buf = [0u8; MAX_VARINT_LEN_7Z];
            let written = encode_7z_varint(val, &mut buf);
            assert_eq!(written, size);

            let (decoded, consumed) = decode_7z_varint(&buf[..written]).expect("decode must succeed");
            assert_eq!(decoded, val);
            assert_eq!(consumed, size);
        }
    }
}

#[test]
fn test_10000_pseudorandom_fuzz_roundtrip() {
    // Use an LCG PRNG for determinism without external crate requirements
    let mut state: u64 = 0xDEADBEEFCAFEBABE;
    let next_u64 = |s: &mut u64| -> u64 {
        *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *s
    };

    let mut buf = [0u8; MAX_VARINT_LEN_7Z];
    let extra_trailing = [0u8; 16];

    for _ in 0..10_000 {
        let val = next_u64(&mut state);
        let expected_size = varint_size_7z(val);

        let written = encode_7z_varint(val, &mut buf);
        assert_eq!(written, expected_size);

        // Decode exact slice
        let (decoded, consumed) = decode_7z_varint(&buf[..written]).expect("decode exact slice");
        assert_eq!(decoded, val);
        assert_eq!(consumed, written);

        // Decode slice with trailing garbage
        let mut stream = Vec::with_capacity(written + extra_trailing.len());
        stream.extend_from_slice(&buf[..written]);
        stream.extend_from_slice(&extra_trailing);

        let (decoded_stream, consumed_stream) = decode_7z_varint(&stream).expect("decode stream with trailing");
        assert_eq!(decoded_stream, val);
        assert_eq!(consumed_stream, written);
    }
}

#[test]
fn test_truncated_buffer_defensive_errors() {
    let test_values = [
        0u64,
        128,
        16384,
        0x200000,
        0x10000000,
        0x00000008_00000000,
        0x00000400_00000000,
        0x00020000_00000000,
        u64::MAX,
    ];

    for &val in &test_values {
        let mut buf = [0u8; MAX_VARINT_LEN_7Z];
        let written = encode_7z_varint(val, &mut buf);

        // Test all truncated slice lengths from 0 up to written - 1
        for len in 0..written {
            let res = decode_7z_varint(&buf[..len]);
            match res {
                Err(VarintError::UnexpectedEof { needed, available }) => {
                    assert_eq!(available, len);
                    let expected_needed = if len == 0 { 1 } else { written };
                    assert_eq!(needed, expected_needed);
                }
                other => panic!("expected UnexpectedEof for val={}, len={}, got {:?}", val, len, other),
            }
        }
    }
}

#[test]
fn test_buffer_capacity_errors() {
    let mut small_buf = [0u8; 4];
    // 5-byte varint into 4-byte buffer
    let res = try_encode_7z_varint(0x10000000, &mut small_buf);
    assert_eq!(
        res,
        Err(VarintError::BufferTooSmall {
            needed: 5,
            available: 4,
        })
    );

    // 9-byte varint into 4-byte buffer
    let res9 = try_encode_7z_varint(u64::MAX, &mut small_buf);
    assert_eq!(
        res9,
        Err(VarintError::BufferTooSmall {
            needed: 9,
            available: 4,
        })
    );
}

#[test]
fn test_sequential_varint_stream_decoding() {
    let values = [
        42u64,
        999,
        1234567,
        0xDEADBEEF,
        0xCAFEBABEDEADBEEF,
        0,
        u64::MAX,
        16383,
    ];

    let mut stream = Vec::new();
    for &v in &values {
        let n = encode_7z_varint_vec(v, &mut stream);
        assert_eq!(n, varint_size_7z(v));
    }

    let mut cursor = 0;
    for &expected in &values {
        let (val, consumed) = decode_7z_varint(&stream[cursor..]).expect("sequential decode");
        assert_eq!(val, expected);
        cursor += consumed;
    }
    assert_eq!(cursor, stream.len());
}

#[test]
fn test_streaming_varint_all_byte_tiers_roundtrip() {
    // Tiers 1 through 9 bytes
    let tier_samples = [
        // 1-byte tier: 0..=127
        0u64, 1, 42, 63, 126, 127,
        // 2-byte tier: 128..=16383
        128, 255, 256, 1000, 8192, 16383,
        // 3-byte tier: 16384..=0x1FFFFF
        16384, 65535, 65536, 0x1FFFFF,
        // 4-byte tier: 0x200000..=0x0FFFFFFF
        0x200000, 0x3FFFFF, 0x0FFFFFFF,
        // 5-byte tier: 0x10000000..=0x00000007_FFFFFFFF
        0x10000000, 0x1FFFFFFF, 0x00000007_FFFFFFFF,
        // 6-byte tier: 0x00000008_00000000..=0x000003FF_FFFFFFFF
        0x00000008_00000000, 0x000003FF_FFFFFFFF,
        // 7-byte tier: 0x00000400_00000000..=0x0001FFFF_FFFFFFFF
        0x00000400_00000000, 0x0001FFFF_FFFFFFFF,
        // 8-byte tier: 0x00020000_00000000..=0x00FFFFFF_FFFFFFFF
        0x00020000_00000000, 0x00FFFFFF_FFFFFFFF,
        // 9-byte tier: 0x01000000_00000000..=u64::MAX
        0x01000000_00000000, 0x7FFFFFFF_FFFFFFFF, 0x80000000_00000000, u64::MAX,
    ];

    for &val in &tier_samples {
        let expected_len = varint_size_7z(val);
        let mut out = Vec::new();
        let written = write_variable_u64(&mut out, val).expect("write_variable_u64 must succeed");
        assert_eq!(written, expected_len, "written length mismatch for val={}", val);
        assert_eq!(out.len(), expected_len);

        let mut cursor = Cursor::new(&out);
        let decoded = read_variable_u64(&mut cursor).expect("read_variable_u64 must succeed");
        assert_eq!(decoded, val, "decoded value mismatch for val={}", val);
        assert_eq!(cursor.position() as usize, expected_len);

        // Test read_variable_usize
        let mut cursor_usize = Cursor::new(&out);
        let decoded_usize = read_variable_usize(&mut cursor_usize).expect("read_variable_usize must succeed");
        let expected_usize = val.min(usize::MAX as u64) as usize;
        assert_eq!(decoded_usize, expected_usize);
    }
}

#[test]
fn test_streaming_varint_exact_boundaries_no_overflow() {
    let boundary_values = [
        0u64,
        1,
        0x7F,
        0x80,
        0x3FFF,
        0x4000,
        0x1FFFFF,
        0x200000,
        0x0FFFFFFF,
        0x10000000,
        u32::MAX as u64,
        (u32::MAX as u64) + 1,
        0x00000007_FFFFFFFF,
        0x00000008_00000000,
        0x000003FF_FFFFFFFF,
        0x00000400_00000000,
        0x0001FFFF_FFFFFFFF,
        0x00020000_00000000,
        0x00FFFFFF_FFFFFFFF,
        0x01000000_00000000,
        0x7FFFFFFF_FFFFFFFF,
        0x80000000_00000000,
        u64::MAX,
    ];

    for &val in &boundary_values {
        let mut stream = Vec::new();
        let written = write_variable_u64(&mut stream, val).expect("write boundary value");
        assert!((1..=9).contains(&written));

        let mut cursor = Cursor::new(&stream);
        let decoded = read_variable_u64(&mut cursor).expect("read boundary value");
        assert_eq!(decoded, val);
        assert_eq!(cursor.position() as usize, written);

        let mut cursor_usize = Cursor::new(&stream);
        let decoded_usize = read_variable_usize(&mut cursor_usize).expect("read boundary usize");
        let clamped = val.min(usize::MAX as u64) as usize;
        assert_eq!(decoded_usize, clamped);
    }
}

#[test]
fn test_streaming_varint_unexpected_eof() {
    let test_cases = [
        128u64,              // 2 bytes
        16384,               // 3 bytes
        0x200000,            // 4 bytes
        0x10000000,          // 5 bytes
        0x00000008_00000000, // 6 bytes
        0x00000400_00000000, // 7 bytes
        0x00020000_00000000, // 8 bytes
        u64::MAX,            // 9 bytes
    ];

    for &val in &test_cases {
        let mut full_bytes = Vec::new();
        let written = write_variable_u64(&mut full_bytes, val).expect("write valid varint");

        // Verify truncated streams from 0 to written - 1 bytes return UnexpectedEof
        for len in 0..written {
            let truncated = &full_bytes[..len];
            let mut cursor = Cursor::new(truncated);
            let res = read_variable_u64(&mut cursor);
            assert!(res.is_err(), "expected error for len={} on val={}", len, val);
            assert_eq!(
                res.unwrap_err().kind(),
                std::io::ErrorKind::UnexpectedEof,
                "expected UnexpectedEof for len={} on val={}",
                len,
                val
            );

            let mut cursor_usize = Cursor::new(truncated);
            let res_usize = read_variable_usize(&mut cursor_usize);
            assert!(res_usize.is_err());
            assert_eq!(
                res_usize.unwrap_err().kind(),
                std::io::ErrorKind::UnexpectedEof
            );
        }
    }
}

#[test]
fn test_all_nid_namespace_constants() {
    let nids = [
        (K_END, 0x00u8),
        (K_HEADER, 0x01),
        (K_ARCHIVE_PROPERTIES, 0x02),
        (K_ADDITIONAL_STREAMS_INFO, 0x03),
        (K_MAIN_STREAMS_INFO, 0x04),
        (K_FILES_INFO, 0x05),
        (K_PACK_INFO, 0x06),
        (K_UNPACK_INFO, 0x07),
        (K_SUB_STREAMS_INFO, 0x08),
        (K_SIZE, 0x09),
        (K_CRC, 0x0A),
        (K_FOLDER, 0x0B),
        (K_CODERS_UNPACK_SIZE, 0x0C),
        (K_NUM_UNPACK_STREAM, 0x0D),
        (K_EMPTY_STREAM, 0x0E),
        (K_EMPTY_FILE, 0x0F),
        (K_ANTI, 0x10),
        (K_NAME, 0x11),
        (K_CTIME, 0x12),
        (K_ATIME, 0x13),
        (K_MTIME, 0x14),
        (K_WIN_ATTRIBUTES, 0x15),
        (K_COMMENT, 0x16),
        (K_ENCODED_HEADER, 0x17),
        (K_START_EDIT_HEADER, 0x18),
        (K_DUMMY, 0x19),
    ];

    for (i, &(nid, expected)) in nids.iter().enumerate() {
        assert_eq!(nid, expected);
        assert_eq!(nid as usize, i);
    }
}
