// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit & boundary validation tests for Snappy Varint-32 and Tag Bytecode codecs.

use ttzip_engine::codecs::snappy::*;
use ttzip_engine::types::TTZipStatus;

#[test]
fn test_varint32_boundary_and_canonical_vectors() {
    let test_vectors: &[(u32, &[u8])] = &[
        (0, &[0x00]),
        (1, &[0x01]),
        (64, &[0x40]),
        (127, &[0x7F]),
        (128, &[0x80, 0x01]),
        (255, &[0xFF, 0x01]),
        (256, &[0x80, 0x02]),
        (16383, &[0xFF, 0x7F]),
        (16384, &[0x80, 0x80, 0x01]),
        (65535, &[0xFF, 0xFF, 0x03]),
        (65536, &[0x80, 0x80, 0x04]),
        (2097150, &[0xFE, 0xFF, 0x7F]), // 0x1FFFFE from Snappy format_description.txt
        (2097151, &[0xFF, 0xFF, 0x7F]),
        (2097152, &[0x80, 0x80, 0x80, 0x01]),
        (268435455, &[0xFF, 0xFF, 0xFF, 0x7F]),
        (268435456, &[0x80, 0x80, 0x80, 0x80, 0x01]),
        (0x7FFFFFFF, &[0xFF, 0xFF, 0xFF, 0xFF, 0x07]),
        (0xFFFFFFFF, &[0xFF, 0xFF, 0xFF, 0xFF, 0x0F]),
    ];

    for &(val, expected_bytes) in test_vectors {
        // 1. Validate length helper
        let needed_len = varint32_len(val);
        assert_eq!(
            needed_len,
            expected_bytes.len(),
            "varint32_len mismatch for {val:#X}"
        );

        // 2. Validate encoding
        let mut buf = [0u8; MAX_VARINT32_BYTES];
        let written = encode_varint32(val, &mut buf);
        assert_eq!(written, expected_bytes.len());
        assert_eq!(&buf[..written], expected_bytes);

        // 3. Validate decoding
        let (decoded, consumed) = decode_varint32(&buf[..written]).expect("decode_varint32 failed");
        assert_eq!(decoded, val, "Decoded value mismatch for {val:#X}");
        assert_eq!(consumed, written);

        // 4. Validate decoding with trailing garbage padding
        let mut padded = buf.to_vec();
        padded.extend_from_slice(&[0xAA, 0xBB, 0xCC]);
        let (decoded_pad, consumed_pad) =
            decode_varint32(&padded).expect("decode with trailing padding failed");
        assert_eq!(decoded_pad, val);
        assert_eq!(consumed_pad, written);
    }
}

#[test]
fn test_varint32_malformed_and_overflow_attacks() {
    // 1. Empty slice
    assert_eq!(decode_varint32(&[]), Err(SnappyError::UnexpectedEof));

    // 2. Truncated non-terminating LEB128 sequences
    assert_eq!(decode_varint32(&[0x80]), Err(SnappyError::UnexpectedEof));
    assert_eq!(
        decode_varint32(&[0x80, 0x80]),
        Err(SnappyError::UnexpectedEof)
    );
    assert_eq!(
        decode_varint32(&[0x80, 0x80, 0x80]),
        Err(SnappyError::UnexpectedEof)
    );
    assert_eq!(
        decode_varint32(&[0x80, 0x80, 0x80, 0x80]),
        Err(SnappyError::UnexpectedEof)
    );

    // 3. 5th byte with bit 7 set (continuation bit set in 5th byte -> overflow beyond 32 bits)
    assert_eq!(
        decode_varint32(&[0x80, 0x80, 0x80, 0x80, 0x80]),
        Err(SnappyError::VarintOverflow)
    );

    // 4. 5th byte data overflow (bits 4..6 non-zero)
    // 0x10 has bit 4 set (> 0x0F)
    assert_eq!(
        decode_varint32(&[0xFF, 0xFF, 0xFF, 0xFF, 0x10]),
        Err(SnappyError::VarintOverflow)
    );
    // 0x20 has bit 5 set
    assert_eq!(
        decode_varint32(&[0xFF, 0xFF, 0xFF, 0xFF, 0x20]),
        Err(SnappyError::VarintOverflow)
    );
    // 0x40 has bit 6 set
    assert_eq!(
        decode_varint32(&[0xFF, 0xFF, 0xFF, 0xFF, 0x40]),
        Err(SnappyError::VarintOverflow)
    );
    // 0x80 has bit 7 set
    assert_eq!(
        decode_varint32(&[0xFF, 0xFF, 0xFF, 0xFF, 0x80]),
        Err(SnappyError::VarintOverflow)
    );

    // 5. 64-bit varint overflow (more than 5 bytes)
    let malicious_64bit = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01];
    assert_eq!(
        decode_varint32(&malicious_64bit),
        Err(SnappyError::VarintOverflow)
    );
}

#[test]
fn test_literal_tag_encoding_and_roundtrip() {
    let lengths: &[usize] = &[
        1, 2, 10, 59, 60, // 1-byte header
        61, 100, 255, 256, // 2-byte header (extra 1 byte)
        257, 1000, 65535, 65536, // 3-byte header (extra 2 bytes)
        65537, 100000, 16777216, // 4-byte header (extra 3 bytes)
        16777217, 50000000, // 5-byte header (extra 4 bytes)
    ];

    let mut buf = [0u8; 16];
    for &len in lengths {
        let written = emit_literal_tag(len, &mut buf).expect("emit literal tag failed");
        let (header, consumed) = parse_tag_header(&buf[..written]).expect("parse literal tag header failed");
        assert_eq!(consumed, written);
        assert_eq!(header.tag_type, SnappyTagType::Literal);
        assert_eq!(header.length as usize, len);
        assert_eq!(header.offset, 0);

        let expected_header_len = match len {
            1..=60 => 1,
            61..=256 => 2,
            257..=65536 => 3,
            65537..=16777216 => 4,
            _ => 5,
        };
        assert_eq!(written, expected_header_len);
    }

    // Zero length must error
    assert_eq!(
        emit_literal_tag(0, &mut buf),
        Err(SnappyError::InvalidParam(
            "Literal length must be at least 1 byte".to_string()
        ))
    );

    // Buffer too small checks
    let mut small_buf = [0u8; 1];
    assert!(matches!(
        emit_literal_tag(61, &mut small_buf),
        Err(SnappyError::BufferTooSmall { required: 2, .. })
    ));
}

#[test]
fn test_literal_full_element_parsing() {
    let payload = b"Hello Snappy Pure Rust Zero-Copy Bytecode!";
    let mut stream = Vec::new();

    let mut tag_buf = [0u8; 8];
    let tag_len = emit_literal_tag(payload.len(), &mut tag_buf).expect("emit literal");
    stream.extend_from_slice(&tag_buf[..tag_len]);
    stream.extend_from_slice(payload);

    let (element, consumed) = parse_element(&stream).expect("parse_element literal");
    assert_eq!(consumed, stream.len());
    assert!(element.is_literal());
    assert!(!element.is_copy());
    assert_eq!(element.length(), payload.len());
    assert_eq!(element.offset(), None);

    match element {
        SnappyElement::Literal { data } => assert_eq!(data, payload),
        _ => panic!("Expected SnappyElement::Literal"),
    }

    // Truncated payload check
    assert_eq!(
        parse_element(&stream[..stream.len() - 1]),
        Err(SnappyError::UnexpectedEof)
    );
}

#[test]
fn test_copy1_tag_encoding_and_roundtrip() {
    let lengths: &[usize] = &[4, 5, 6, 7, 8, 9, 10, 11];
    let offsets: &[u32] = &[1, 2, 64, 255, 256, 1024, 2047];

    let mut buf = [0u8; 8];
    for &len in lengths {
        for &offset in offsets {
            let written = emit_copy1_tag(len, offset, &mut buf).expect("emit copy1 tag");
            assert_eq!(written, 2);

            let (header, consumed) = parse_tag_header(&buf[..written]).expect("parse copy1 header");
            assert_eq!(consumed, 2);
            assert_eq!(header.tag_type, SnappyTagType::Copy1Byte);
            assert_eq!(header.length as usize, len);
            assert_eq!(header.offset, offset);

            let (element, el_consumed) = parse_element(&buf[..written]).expect("parse_element copy1");
            assert_eq!(el_consumed, 2);
            assert!(element.is_copy());
            assert_eq!(element.length(), len);
            assert_eq!(element.offset(), Some(offset));
        }
    }

    // Out of bounds checks
    assert!(emit_copy1_tag(3, 100, &mut buf).is_err());
    assert!(emit_copy1_tag(12, 100, &mut buf).is_err());
    assert!(emit_copy1_tag(4, 0, &mut buf).is_err());
    assert!(emit_copy1_tag(4, 2048, &mut buf).is_err());

    let mut small_buf = [0u8; 1];
    assert!(matches!(
        emit_copy1_tag(4, 100, &mut small_buf),
        Err(SnappyError::BufferTooSmall { required: 2, .. })
    ));
}

#[test]
fn test_copy2_tag_encoding_and_roundtrip() {
    let lengths: &[usize] = &[1, 2, 16, 32, 63, 64];
    let offsets: &[u32] = &[1, 100, 2048, 32768, 65535];

    let mut buf = [0u8; 8];
    for &len in lengths {
        for &offset in offsets {
            let written = emit_copy2_tag(len, offset, &mut buf).expect("emit copy2 tag");
            assert_eq!(written, 3);

            let (header, consumed) = parse_tag_header(&buf[..written]).expect("parse copy2 header");
            assert_eq!(consumed, 3);
            assert_eq!(header.tag_type, SnappyTagType::Copy2Byte);
            assert_eq!(header.length as usize, len);
            assert_eq!(header.offset, offset);

            let (element, el_consumed) = parse_element(&buf[..written]).expect("parse_element copy2");
            assert_eq!(el_consumed, 3);
            assert!(element.is_copy());
            assert_eq!(element.length(), len);
            assert_eq!(element.offset(), Some(offset));
        }
    }

    // Out of bounds checks
    assert!(emit_copy2_tag(0, 100, &mut buf).is_err());
    assert!(emit_copy2_tag(65, 100, &mut buf).is_err());
    assert!(emit_copy2_tag(10, 0, &mut buf).is_err());
    assert!(emit_copy2_tag(10, 65536, &mut buf).is_err());

    let mut small_buf = [0u8; 2];
    assert!(matches!(
        emit_copy2_tag(10, 100, &mut small_buf),
        Err(SnappyError::BufferTooSmall { required: 3, .. })
    ));
}

#[test]
fn test_copy4_tag_encoding_and_roundtrip() {
    let lengths: &[usize] = &[1, 2, 16, 32, 63, 64];
    let offsets: &[u32] = &[1, 65536, 1000000, 0x12345678, u32::MAX];

    let mut buf = [0u8; 8];
    for &len in lengths {
        for &offset in offsets {
            let written = emit_copy4_tag(len, offset, &mut buf).expect("emit copy4 tag");
            assert_eq!(written, 5);

            let (header, consumed) = parse_tag_header(&buf[..written]).expect("parse copy4 header");
            assert_eq!(consumed, 5);
            assert_eq!(header.tag_type, SnappyTagType::Copy4Byte);
            assert_eq!(header.length as usize, len);
            assert_eq!(header.offset, offset);

            let (element, el_consumed) = parse_element(&buf[..written]).expect("parse_element copy4");
            assert_eq!(el_consumed, 5);
            assert!(element.is_copy());
            assert_eq!(element.length(), len);
            assert_eq!(element.offset(), Some(offset));
        }
    }

    // Out of bounds checks
    assert!(emit_copy4_tag(0, 100, &mut buf).is_err());
    assert!(emit_copy4_tag(65, 100, &mut buf).is_err());
    assert!(emit_copy4_tag(10, 0, &mut buf).is_err());

    let mut small_buf = [0u8; 4];
    assert!(matches!(
        emit_copy4_tag(10, 100, &mut small_buf),
        Err(SnappyError::BufferTooSmall { required: 5, .. })
    ));
}

#[test]
fn test_length_minus_offset_table_correctness_and_branchless_properties() {
    assert_eq!(LENGTH_MINUS_OFFSET_TABLE.len(), 256);

    for tag in 0..=255u8 {
        let entry = LENGTH_MINUS_OFFSET_TABLE[tag as usize];
        let tag_type = tag & 0x03;
        let data = (tag >> 2) as i16;

        match tag_type {
            0b11 => {
                // Copy-4
                assert_eq!(entry, 0x00FF);
            }
            0b10 => {
                // Copy-2: length = data + 1, offset = 0 in tag
                let expected = data + 1;
                assert_eq!(entry, expected);
            }
            0b01 => {
                // Copy-1: length = (data & 7) + 4, offset_hi = data >> 3
                let len = (data & 7) + 4;
                let offset_hi = data >> 3;
                let expected = len - (offset_hi << 8);
                assert_eq!(entry, expected);
            }
            0b00 => {
                // Literal: data < 60 => spurious offset 1 (256), data >= 60 => 0x00FF
                if data < 60 {
                    let len = data + 1;
                    let expected = len - 256;
                    assert_eq!(entry, expected);
                } else {
                    assert_eq!(entry, 0x00FF);
                }
            }
            _ => unreachable!(),
        }
    }

    // Verify fast branchless reconstruction on Copy-1 sample:
    // Tag with length 7, offset 1000 (0x3E8 = 0b00000011_11101000)
    // tag_bits: Copy1 (0b01) | (7 - 4 = 3 << 2 = 0b00001100) | (offset_hi = 3 << 5 = 0b01100000) = 0x6D
    // trailer = offset_lo = 0xE8 (232)
    let tag: u8 = 0x6D;
    let entry = LENGTH_MINUS_OFFSET_TABLE[tag as usize]; // len 7 - (3 << 8) = 7 - 768 = -761
    let length = (entry & 0xFF) as u32;
    assert_eq!(length, 7);
    let trailer: u32 = 0xE8;
    // copy_offset = trailer - entry + length
    let reconstructed_offset = trailer as i32 - entry as i32 + length as i32;
    assert_eq!(reconstructed_offset, 1000);
}

#[test]
fn test_snappy_error_and_ttzip_status_interop() {
    let pairs: &[(SnappyError, TTZipStatus)] = &[
        (SnappyError::VarintOverflow, TTZipStatus::ErrCorruptHeader),
        (SnappyError::UnexpectedEof, TTZipStatus::ErrCorruptHeader),
        (
            SnappyError::CorruptHeader("bad magic".to_string()),
            TTZipStatus::ErrCorruptHeader,
        ),
        (SnappyError::InvalidTag(0xFF), TTZipStatus::ErrCorruptHeader),
        (
            SnappyError::InvalidOffset {
                offset: 50,
                position: 10,
            },
            TTZipStatus::ErrInvalidOffset,
        ),
        (
            SnappyError::BufferTooSmall {
                required: 10,
                available: 2,
            },
            TTZipStatus::ErrInvalidParam,
        ),
        (
            SnappyError::LiteralLengthExceeded {
                length: 100,
                max: 50,
            },
            TTZipStatus::ErrInvalidParam,
        ),
        (SnappyError::CompressionFailed, TTZipStatus::ErrCompressionFailed),
        (
            SnappyError::DecompressionFailed("bad block".to_string()),
            TTZipStatus::ErrExtractionFailed,
        ),
    ];

    for (err, expected_status) in pairs {
        let status: TTZipStatus = err.clone().into();
        assert_eq!(status, *expected_status);

        let roundtrip_err: SnappyError = status.into();
        let roundtrip_status: TTZipStatus = roundtrip_err.into();
        assert_eq!(roundtrip_status, *expected_status);
    }
}
