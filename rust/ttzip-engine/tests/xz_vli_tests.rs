// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration tests for XZ Variable-Length Integer (VLI) codec,
//! testing 1..=9 byte ladder boundaries, non-canonical encoding rejection,
//! overflow circuit breakers, and streaming/slice parity.

use std::io::Cursor;
use ttzip_engine::xz::vli::{
    decode_vli, decode_vli_stream, encode_vli, encode_vli_stream, encode_vli_vec, vli_size,
    XzVliError, XZ_VLI_BYTES_MAX, XZ_VLI_MAX, XZ_VLI_UNKNOWN,
};

#[test]
fn test_ladder_boundaries_roundtrip() {
    let test_matrix: &[(u64, usize)] = &[
        // 1 byte boundaries (0 .. 127)
        (0, 1),
        (1, 1),
        (63, 1),
        (127, 1),
        // 2 byte boundaries (128 .. 16383)
        (128, 2),
        (255, 2),
        (256, 2),
        (16383, 2),
        // 3 byte boundaries (16384 .. 2097151)
        (16384, 3),
        (65535, 3),
        (2097151, 3),
        // 4 byte boundaries (2097152 .. 268435455)
        (2097152, 4),
        (16777215, 4),
        (268435455, 4),
        // 5 byte boundaries (268435456 .. 34359738367)
        (268435456, 5),
        (4294967295, 5),
        (34359738367, 5),
        // 6 byte boundaries (34359738368 .. 4398046511103)
        (34359738368, 6),
        (4398046511103, 6),
        // 7 byte boundaries (4398046511104 .. 562949953421311)
        (4398046511104, 7),
        (562949953421311, 7),
        // 8 byte boundaries (562949953421312 .. 72057594037927935)
        (562949953421312, 8),
        (72057594037927935, 8),
        // 9 byte boundaries (72057594037927936 .. 2^63 - 1)
        (72057594037927936, 9),
        (0x0100_0000_0000_0000, 9),
        (0x4000_0000_0000_0000, 9),
        (XZ_VLI_MAX, 9),
    ];

    for &(val, expected_len) in test_matrix {
        // 1. Check vli_size fast calculation
        let calculated_size = vli_size(val).expect("valid VLI value must compute size");
        assert_eq!(
            calculated_size, expected_len,
            "vli_size mismatch for value {val:#X} ({val})"
        );

        // 2. Encode to slice
        let mut buf = [0u8; XZ_VLI_BYTES_MAX + 4];
        let mut pos = 0;
        let written = encode_vli(val, &mut buf, &mut pos).expect("encode_vli must succeed");
        assert_eq!(written, expected_len);
        assert_eq!(pos, expected_len);

        // 3. Encode to Vec
        let vec_buf = encode_vli_vec(val).expect("encode_vli_vec must succeed");
        assert_eq!(vec_buf.len(), expected_len);
        assert_eq!(&buf[..written], &vec_buf[..]);

        // 4. Decode from slice
        let mut decode_pos = 0;
        let decoded = decode_vli(&buf[..written], &mut decode_pos).expect("decode_vli must succeed");
        assert_eq!(decoded, val);
        assert_eq!(decode_pos, written);

        // 5. Decode from stream
        let mut cursor = Cursor::new(&vec_buf);
        let stream_decoded =
            decode_vli_stream(&mut cursor).expect("decode_vli_stream must succeed");
        assert_eq!(stream_decoded, val);
        assert_eq!(cursor.position() as usize, written);

        // 6. Encode to stream
        let mut stream_out = Vec::new();
        let stream_written =
            encode_vli_stream(val, &mut stream_out).expect("encode_vli_stream must succeed");
        assert_eq!(stream_written, expected_len);
        assert_eq!(stream_out, vec_buf);
    }
}

#[test]
fn test_vli_size_exhaustive_consistency() {
    // Test power of two steps and offsets around boundaries
    for shift in 0..63 {
        let base = 1u64 << shift;
        let candidates = [
            base.saturating_sub(2),
            base.saturating_sub(1),
            base,
            base.saturating_add(1),
        ];

        for &val in &candidates {
            if val <= XZ_VLI_MAX {
                let size = vli_size(val).unwrap();
                let encoded = encode_vli_vec(val).unwrap();
                assert_eq!(
                    size,
                    encoded.len(),
                    "vli_size ({size}) differs from actual encoded length ({}) for {val:#X}",
                    encoded.len()
                );
            }
        }
    }
}

#[test]
fn test_vli_overflow_rejection() {
    let overflow_values = [
        XZ_VLI_MAX + 1, // 0x8000_0000_0000_0000 (bit 63 set)
        0x8000_0000_0000_0001,
        0xFFFF_FFFF_0000_0000,
        XZ_VLI_UNKNOWN, // u64::MAX
    ];

    for &val in &overflow_values {
        assert_eq!(
            vli_size(val),
            Err(XzVliError::ValueTooLarge { val }),
            "vli_size must reject overflow value {val:#X}"
        );

        let mut buf = [0u8; 16];
        let mut pos = 0;
        assert_eq!(
            encode_vli(val, &mut buf, &mut pos),
            Err(XzVliError::ValueTooLarge { val }),
            "encode_vli must reject overflow value {val:#X}"
        );
        assert_eq!(pos, 0, "pos must remain 0 on error");

        assert_eq!(
            encode_vli_vec(val),
            Err(XzVliError::ValueTooLarge { val }),
            "encode_vli_vec must reject overflow value {val:#X}"
        );

        let mut out = Vec::new();
        assert_eq!(
            encode_vli_stream(val, &mut out),
            Err(XzVliError::ValueTooLarge { val }),
            "encode_vli_stream must reject overflow value {val:#X}"
        );
    }
}

#[test]
fn test_non_canonical_encoding_rejection() {
    // Non-canonical sequences have value 0x00 in the final terminating byte of a multi-byte sequence
    let non_canonical_cases: &[(&[u8], usize)] = &[
        // 2-byte zero: [0x80, 0x00] -> decoded 0, but must be [0x00]
        (&[0x80, 0x00], 1),
        // 2-byte one with zero high byte: [0x81, 0x00] -> decoded 1, but must be [0x01]
        (&[0x81, 0x00], 1),
        // 3-byte zero: [0x80, 0x80, 0x00]
        (&[0x80, 0x80, 0x00], 2),
        // 3-byte with zero top byte: [0xFF, 0x80, 0x00]
        (&[0xFF, 0x80, 0x00], 2),
        // 9-byte sequence with zero terminating byte
        (
            &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x00],
            8,
        ),
    ];

    for &(raw_bytes, expected_byte_index) in non_canonical_cases {
        let mut pos = 0;
        let res = decode_vli(raw_bytes, &mut pos);
        assert_eq!(
            res,
            Err(XzVliError::NonCanonical {
                byte_index: expected_byte_index
            }),
            "decode_vli should reject non-canonical bytes: {raw_bytes:02X?}"
        );
        assert_eq!(pos, 0, "pos should not be mutated on failure");

        let mut cursor = Cursor::new(raw_bytes);
        let stream_res = decode_vli_stream(&mut cursor);
        assert_eq!(
            stream_res,
            Err(XzVliError::NonCanonical {
                byte_index: expected_byte_index
            }),
            "decode_vli_stream should reject non-canonical bytes: {raw_bytes:02X?}"
        );
    }
}

#[test]
fn test_sequence_too_long_circuit_breaker() {
    // 9 bytes all with 0x80 bit set -> indicates 10th byte needed -> SequenceTooLong
    let nine_all_cont = [0x80; 9];
    let mut pos = 0;
    assert_eq!(
        decode_vli(&nine_all_cont, &mut pos),
        Err(XzVliError::SequenceTooLong),
        "decode_vli must reject 9 bytes with high bit set"
    );
    assert_eq!(pos, 0);

    let mut cursor = Cursor::new(&nine_all_cont);
    assert_eq!(
        decode_vli_stream(&mut cursor),
        Err(XzVliError::SequenceTooLong),
        "decode_vli_stream must reject 9 bytes with high bit set"
    );

    // 10-byte valid-looking sequence
    let ten_bytes = [0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01];
    let mut pos = 0;
    assert_eq!(
        decode_vli(&ten_bytes, &mut pos),
        Err(XzVliError::SequenceTooLong),
        "decode_vli must reject 10-byte sequence"
    );
    assert_eq!(pos, 0);

    let mut cursor = Cursor::new(&ten_bytes);
    assert_eq!(
        decode_vli_stream(&mut cursor),
        Err(XzVliError::SequenceTooLong),
        "decode_vli_stream must reject 10-byte sequence"
    );
}

#[test]
fn test_unexpected_eof_on_truncated_buffers() {
    let truncated_cases: &[&[u8]] = &[
        &[],
        &[0x80],
        &[0x80, 0x80],
        &[0xFF, 0xFF, 0xFF],
        &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80], // 8 bytes with continuation bits
    ];

    for &buf in truncated_cases {
        let mut pos = 0;
        let res = decode_vli(buf, &mut pos);
        assert!(
            matches!(res, Err(XzVliError::UnexpectedEof { .. })),
            "Expected UnexpectedEof for buffer {buf:02X?}, got {res:?}"
        );
        assert_eq!(pos, 0);

        let mut cursor = Cursor::new(buf);
        let stream_res = decode_vli_stream(&mut cursor);
        assert!(
            matches!(stream_res, Err(XzVliError::UnexpectedEof { .. })),
            "Expected UnexpectedEof for stream {buf:02X?}, got {stream_res:?}"
        );
    }
}

#[test]
fn test_buffer_too_small_on_encode() {
    let mut small_buf = [0u8; 2];
    let mut pos = 0;
    let res = encode_vli(16384, &mut small_buf, &mut pos); // 16384 requires 3 bytes
    assert_eq!(
        res,
        Err(XzVliError::BufferTooSmall {
            needed: 3,
            available: 2
        })
    );
    assert_eq!(pos, 0);
}

#[test]
fn test_sequential_concatenated_vlis() {
    let values = [
        0u64,
        127,
        128,
        16383,
        16384,
        2097151,
        2097152,
        XZ_VLI_MAX,
        42,
    ];

    let mut stream_buf = Vec::new();
    for &val in &values {
        encode_vli_stream(val, &mut stream_buf).expect("encode stream must succeed");
    }

    // Decode sequentially from slice
    let mut pos = 0;
    for &expected in &values {
        let decoded = decode_vli(&stream_buf, &mut pos).expect("sequential decode_vli failed");
        assert_eq!(decoded, expected);
    }
    assert_eq!(pos, stream_buf.len());

    // Decode sequentially from stream
    let mut cursor = Cursor::new(&stream_buf);
    for &expected in &values {
        let decoded =
            decode_vli_stream(&mut cursor).expect("sequential decode_vli_stream failed");
        assert_eq!(decoded, expected);
    }
    assert_eq!(cursor.position() as usize, stream_buf.len());
}
