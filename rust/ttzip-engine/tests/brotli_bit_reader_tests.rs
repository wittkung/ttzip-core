// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit and invariant tests for RFC 7932 64-bit BitReader & WBITS window parser.

use ttzip_engine::codecs::brotli::{
    BrotliBitReader, BrotliError, BrotliWindow, BROTLI_LARGE_MAX_WINDOW_BITS,
    BROTLI_MAX_WINDOW_BITS, BROTLI_MIN_WINDOW_BITS, BROTLI_WINDOW_GAP,
};

/// Helper to pack LSB-first bit sequences into a byte vector.
fn pack_lsb_bits(chunks: &[(u32, u32)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut acc = 0u64;
    let mut count = 0u32;
    for &(val, len) in chunks {
        let mask = if len >= 32 { 0xFFFF_FFFF } else { (1u32 << len) - 1 };
        acc |= ((val & mask) as u64) << count;
        count += len;
        while count >= 8 {
            out.push((acc & 0xFF) as u8);
            acc >>= 8;
            count -= 8;
        }
    }
    if count > 0 {
        out.push((acc & 0xFF) as u8);
    }
    out
}

#[test]
fn test_bit_reader_basic_creation_and_fill() {
    let data = [0xAA, 0x55, 0xCC, 0x33, 0x0F, 0xF0];
    let mut br = BrotliBitReader::new(&data);

    assert_eq!(br.peek_bits(8), 0xAA);
    assert_eq!(br.read_bits(8).expect("read 8"), 0xAA);
    assert_eq!(br.peek_bits(8), 0x55);
    assert_eq!(br.read_bits(8).expect("read 8"), 0x55);
    assert_eq!(br.read_byte().expect("read byte"), 0xCC);
    assert_eq!(br.read_byte().expect("read byte"), 0x33);
}

#[test]
fn test_bit_reader_fill_and_read_1_to_32_bits() {
    // Generate a known bit sequence and read variable length bits (1..=32)
    let payload = vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11, 0x22];
    let mut br = BrotliBitReader::new(&payload);

    // Read 1 bit (0x12 & 1 == 0)
    assert_eq!(br.read_bits(1).expect("1 bit"), 0);
    // Read 3 bits (0x12 >> 1 & 7 == 1)
    assert_eq!(br.read_bits(3).expect("3 bits"), 1);
    // Read 4 bits (0x12 >> 4 & 0xF == 1)
    assert_eq!(br.read_bits(4).expect("4 bits"), 1);

    // Now aligned to byte 1 (0x34)
    assert_eq!(br.read_bits(16).expect("16 bits"), 0x5634);

    // Read 32 bits across remaining bytes
    let val32 = br.read_bits(32).expect("32 bits");
    let expected32 = u32::from_le_bytes([0x78, 0x9A, 0xBC, 0xDE]);
    assert_eq!(val32, expected32);

    // Read 0 bits
    assert_eq!(br.read_bits(0).expect("0 bits"), 0);
}

#[test]
fn test_bit_reader_byte_boundary_jump_valid_zero_padding() {
    // Byte 0: 0b0000_0101 (value 5 in low 3 bits, remaining 5 padding bits are 0)
    // Byte 1: 0x42
    let data = [0b0000_0101, 0x42];
    let mut br = BrotliBitReader::new(&data);

    assert_eq!(br.read_bits(3).expect("read 3 bits"), 5);
    assert_eq!(br.bits_remaining_in_byte(), 5);

    // Jump to byte boundary should succeed since padding is 0
    br.jump_to_byte_boundary().expect("valid zero padding");

    // Next read should be byte 1 (0x42)
    assert_eq!(br.read_byte().expect("read byte 1"), 0x42);
}

#[test]
fn test_bit_reader_byte_boundary_jump_invalid_nonzero_padding() {
    // Byte 0: 0b0010_0101 (value 5 in low 3 bits, but padding contains a 1 at bit 5)
    let data = [0b0010_0101, 0x42];
    let mut br = BrotliBitReader::new(&data);

    assert_eq!(br.read_bits(3).expect("read 3 bits"), 5);
    // Non-zero padding must trigger InvalidPadding error
    let err = br.jump_to_byte_boundary().expect_err("must reject non-zero padding");
    assert_eq!(err, BrotliError::InvalidPadding);
}

#[test]
fn test_bit_reader_byte_boundary_jump_when_already_aligned() {
    let data = [0x11, 0x22];
    let mut br = BrotliBitReader::new(&data);

    assert_eq!(br.read_bits(8).expect("read 8 bits"), 0x11);
    // Already aligned
    assert_eq!(br.bits_remaining_in_byte(), 0);
    br.jump_to_byte_boundary().expect("already aligned jump");
    assert_eq!(br.read_bits(8).expect("read next byte"), 0x22);
}

#[test]
fn test_bit_reader_unload_exact_byte_rewind() {
    let data = [10, 20, 30, 40, 50, 60, 70, 80];
    let mut br = BrotliBitReader::new(&data);

    // Initial prefill loaded 4 bytes (10, 20, 30, 40) into 64-bit accumulator.
    // Read only 8 bits (byte 0: 10)
    assert_eq!(br.read_bits(8).expect("read byte 0"), 10);

    // Unload should rewind unconsumed bytes (20, 30, 40) back to input cursor
    let consumed = br.unload();
    assert_eq!(consumed, 1);
    assert_eq!(br.pos, 1);
    assert_eq!(br.val, 0);
    assert_eq!(br.bit_pos, 0);

    // Subsequent reader starting from data[consumed..] should see byte 20 next
    let mut br2 = BrotliBitReader::new(&data[consumed..]);
    assert_eq!(br2.read_byte().expect("read byte 1"), 20);
    assert_eq!(br2.read_byte().expect("read byte 2"), 30);
}

#[test]
fn test_bit_reader_eof_error() {
    let data = [0x01];
    let mut br = BrotliBitReader::new(&data);

    assert_eq!(br.read_bits(8).expect("read 8 bits"), 1);
    let err = br.read_bits(1).expect_err("should encounter EOF");
    assert_eq!(err, BrotliError::UnexpectedEof);
    assert!(br.is_empty());
}

#[test]
fn test_brotli_window_all_standard_wbits_10_to_24() {
    // Test all 15 standard WBITS encodings from RFC 7932 Section 9.1
    let test_cases: &[(u8, &[(u32, u32)])] = &[
        // WBITS 16: 1 bit '0'
        (16, &[(0, 1)]),
        // WBITS 17: 7 bits '1' + '000' + '000'
        (17, &[(1, 1), (0, 3), (0, 3)]),
        // WBITS 18..=24: 4 bits '1' + n (1..=7)
        (18, &[(1, 1), (1, 3)]),
        (19, &[(1, 1), (2, 3)]),
        (20, &[(1, 1), (3, 3)]),
        (21, &[(1, 1), (4, 3)]),
        (22, &[(1, 1), (5, 3)]),
        (23, &[(1, 1), (6, 3)]),
        (24, &[(1, 1), (7, 3)]),
        // WBITS 10..=15: 7 bits '1' + '000' + m (2..=7)
        (10, &[(1, 1), (0, 3), (2, 3)]),
        (11, &[(1, 1), (0, 3), (3, 3)]),
        (12, &[(1, 1), (0, 3), (4, 3)]),
        (13, &[(1, 1), (0, 3), (5, 3)]),
        (14, &[(1, 1), (0, 3), (6, 3)]),
        (15, &[(1, 1), (0, 3), (7, 3)]),
    ];

    for &(expected_wbits, bit_pattern) in test_cases {
        let stream = pack_lsb_bits(bit_pattern);
        let mut br = BrotliBitReader::new(&stream);
        let window = BrotliWindow::parse_window_bits(&mut br, false)
            .unwrap_or_else(|e| panic!("Failed to parse standard WBITS {}: {:?}", expected_wbits, e));

        assert_eq!(
            window.window_bits, expected_wbits,
            "WBITS mismatch for pattern {:?}",
            bit_pattern
        );
        assert_eq!(
            window.max_distance,
            (1usize << expected_wbits) - BROTLI_WINDOW_GAP
        );
    }
}

#[test]
fn test_brotli_window_max_distance_calculation() {
    for bits in BROTLI_MIN_WINDOW_BITS..=BROTLI_MAX_WINDOW_BITS {
        let win = BrotliWindow::new(bits, false).expect("valid window");
        assert_eq!(win.max_distance, (1usize << bits) - 16);
    }
}

#[test]
fn test_brotli_large_window_parsing_valid_25_to_30() {
    for wbits in 25..=BROTLI_LARGE_MAX_WINDOW_BITS {
        // Large Window 14-bit pattern: 1 + 000 + 001 + 0 + 6-bit wbits
        let pattern: &[(u32, u32)] = &[
            (1, 1),           // bit0 = 1
            (0, 3),           // n = 0
            (1, 3),           // m = 1 (large window signal)
            (0, 1),           // extra bit = 0
            (wbits as u32, 6) // 6-bit WBITS
        ];
        let stream = pack_lsb_bits(pattern);
        let mut br = BrotliBitReader::new(&stream);

        let window = BrotliWindow::parse_window_bits(&mut br, true)
            .unwrap_or_else(|e| panic!("Failed to parse Large Window WBITS {}: {:?}", wbits, e));

        assert_eq!(window.window_bits, wbits);
        assert_eq!(window.max_distance, (1usize << wbits) - BROTLI_WINDOW_GAP);
    }
}

#[test]
fn test_brotli_large_window_unauthorized_rejection() {
    // Large Window pattern with allow_large_window = false
    let pattern: &[(u32, u32)] = &[
        (1, 1),
        (0, 3),
        (1, 3),
        (0, 1),
        (26, 6),
    ];
    let stream = pack_lsb_bits(pattern);
    let mut br = BrotliBitReader::new(&stream);

    let err = BrotliWindow::parse_window_bits(&mut br, false)
        .expect_err("must reject large window when unauthorized");
    assert_eq!(err, BrotliError::InvalidWindowBits(1));
}

#[test]
fn test_brotli_large_window_invalid_extra_bit_or_out_of_range() {
    // Test non-zero extra bit (8th bit = 1)
    let invalid_extra_bit: &[(u32, u32)] = &[
        (1, 1),
        (0, 3),
        (1, 3),
        (1, 1), // Corrupted 8th bit
        (26, 6),
    ];
    let stream = pack_lsb_bits(invalid_extra_bit);
    let mut br = BrotliBitReader::new(&stream);
    let err = BrotliWindow::parse_window_bits(&mut br, true)
        .expect_err("must reject non-zero extra bit");
    assert_eq!(err, BrotliError::InvalidWindowBits(0));

    // Test out of range WBITS (wbits = 31 > 30)
    let out_of_range_wbits: &[(u32, u32)] = &[
        (1, 1),
        (0, 3),
        (1, 3),
        (0, 1),
        (31, 6), // 31 is > 30
    ];
    let stream2 = pack_lsb_bits(out_of_range_wbits);
    let mut br2 = BrotliBitReader::new(&stream2);
    let err2 = BrotliWindow::parse_window_bits(&mut br2, true)
        .expect_err("must reject wbits > 30");
    assert_eq!(err2, BrotliError::InvalidWindowBits(31));

    // Test out of range WBITS (wbits = 9 < 10)
    let underflow_wbits: &[(u32, u32)] = &[
        (1, 1),
        (0, 3),
        (1, 3),
        (0, 1),
        (9, 6), // 9 is < 10
    ];
    let stream3 = pack_lsb_bits(underflow_wbits);
    let mut br3 = BrotliBitReader::new(&stream3);
    let err3 = BrotliWindow::parse_window_bits(&mut br3, true)
        .expect_err("must reject wbits < 10");
    assert_eq!(err3, BrotliError::InvalidWindowBits(9));
}
