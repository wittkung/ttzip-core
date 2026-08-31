// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit tests for Libdeflate 1-Pass Huffman tree builder,
//! 19-symbol Precode RLE encoder, and 64-bit fast bitstream emitter.

use ttzip_engine::codecs::libdeflate::{
    compute_num_explicit_precode_lens, compute_precode_items, deflate_make_huffman_code,
    reverse_codeword, FastBitWriter, FastBitWriterError, FastBitWriterVec, PrecodeEncoder,
    DEFLATE_EXTRA_PRECODE_BITS, DEFLATE_MAX_LITLEN_CODEWORD_LEN, DEFLATE_NUM_LITLEN_SYMS,
    DEFLATE_NUM_OFFSET_SYMS, DEFLATE_NUM_PRECODE_SYMS, DEFLATE_PRECODE_LENS_PERMUTATION,
    MAX_LITLEN_CODEWORD_LEN, MAX_OFFSET_CODEWORD_LEN, MAX_PRE_CODEWORD_LEN,
};

/// Helper: calculates the Kraft sum for a given set of codeword lengths.
/// Sum must be <= 1.0 (or <= 1 << 15 when scaled by 2^15).
fn compute_kraft_scaled(lens: &[u8]) -> u32 {
    let mut sum: u32 = 0;
    for &len in lens {
        if len > 0 {
            assert!(len <= 15, "codeword length exceeds max 15");
            sum += 1 << (15 - len);
        }
    }
    sum
}

/// Helper: validates that no two canonical codewords create a prefix collision.
fn assert_no_prefix_collisions(lens: &[u8], codewords: &[u32]) {
    let n = lens.len();
    for i in 0..n {
        let len_i = lens[i];
        if len_i == 0 {
            continue;
        }
        let code_i = codewords[i];
        for j in (i + 1)..n {
            let len_j = lens[j];
            if len_j == 0 {
                continue;
            }
            let code_j = codewords[j];

            let min_len = len_i.min(len_j);
            let mask = (1u32 << min_len) - 1;
            let prefix_i = code_i & mask;
            let prefix_j = code_j & mask;

            assert!(
                prefix_i != prefix_j || len_i == len_j,
                "Prefix collision between sym {i} (len {len_i}, code {code_i:#b}) and sym {j} (len {len_j}, code {code_j:#b})"
            );
        }
    }
}

/// Helper: simple bit reader for bit-exact verification.
struct TestBitReader<'a> {
    data: &'a [u8],
    bit_pos: usize,
}

impl<'a> TestBitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, bit_pos: 0 }
    }

    fn read_bits(&mut self, n: u32) -> u64 {
        if n == 0 {
            return 0;
        }
        let mut val = 0u64;
        for i in 0..n {
            let byte_idx = (self.bit_pos + i as usize) / 8;
            let bit_idx = (self.bit_pos + i as usize) % 8;
            if byte_idx < self.data.len() {
                let bit = ((self.data[byte_idx] >> bit_idx) & 1) as u64;
                val |= bit << i;
            }
        }
        self.bit_pos += n as usize;
        val
    }
}

// MARK: - Huffman Tree Builder Tests

#[test]
fn test_huffman_kraft_inequality_and_max_depth_bound_14() {
    let mut freqs = [0u32; DEFLATE_NUM_LITLEN_SYMS];
    for i in 0..DEFLATE_NUM_LITLEN_SYMS {
        freqs[i] = ((i * 37 + 13) % 500 + 1) as u32;
    }

    let mut lens = [0u8; DEFLATE_NUM_LITLEN_SYMS];
    let mut codewords = [0u32; DEFLATE_NUM_LITLEN_SYMS];

    deflate_make_huffman_code(
        DEFLATE_NUM_LITLEN_SYMS,
        MAX_LITLEN_CODEWORD_LEN,
        &freqs,
        &mut lens,
        &mut codewords,
    );

    for &len in &lens {
        assert!(
            len <= MAX_LITLEN_CODEWORD_LEN as u8,
            "codeword length {len} exceeds 14"
        );
        assert!(len > 0, "used symbol must have non-zero length");
    }

    let kraft = compute_kraft_scaled(&lens);
    assert!(
        kraft <= (1 << 15),
        "Kraft inequality violated: {kraft} > 32768"
    );
    assert_no_prefix_collisions(&lens, &codewords);
}

#[test]
fn test_huffman_kraft_inequality_and_max_depth_bound_15() {
    let mut freqs = [0u32; DEFLATE_NUM_LITLEN_SYMS];
    for i in 0..DEFLATE_NUM_LITLEN_SYMS {
        freqs[i] = (i as u32 + 1).pow(2);
    }

    let mut lens = [0u8; DEFLATE_NUM_LITLEN_SYMS];
    let mut codewords = [0u32; DEFLATE_NUM_LITLEN_SYMS];

    deflate_make_huffman_code(
        DEFLATE_NUM_LITLEN_SYMS,
        DEFLATE_MAX_LITLEN_CODEWORD_LEN,
        &freqs,
        &mut lens,
        &mut codewords,
    );

    for &len in &lens {
        assert!(
            len <= DEFLATE_MAX_LITLEN_CODEWORD_LEN as u8,
            "codeword length {len} exceeds 15"
        );
        assert!(len > 0);
    }

    let kraft = compute_kraft_scaled(&lens);
    assert!(kraft <= (1 << 15), "Kraft inequality violated: {kraft}");
    assert_no_prefix_collisions(&lens, &codewords);
}

#[test]
fn test_huffman_precode_max_depth_bound_7() {
    let mut freqs = [0u32; DEFLATE_NUM_PRECODE_SYMS];
    for i in 0..DEFLATE_NUM_PRECODE_SYMS {
        freqs[i] = (i as u32 + 1) * 10;
    }

    let mut lens = [0u8; DEFLATE_NUM_PRECODE_SYMS];
    let mut codewords = [0u32; DEFLATE_NUM_PRECODE_SYMS];

    deflate_make_huffman_code(
        DEFLATE_NUM_PRECODE_SYMS,
        MAX_PRE_CODEWORD_LEN,
        &freqs,
        &mut lens,
        &mut codewords,
    );

    for &len in &lens {
        assert!(
            len <= MAX_PRE_CODEWORD_LEN as u8,
            "precode length {len} exceeds 7"
        );
        assert!(len > 0);
    }

    let kraft = compute_kraft_scaled(&lens);
    assert!(kraft <= (1 << 15), "Kraft inequality violated: {kraft}");
    assert_no_prefix_collisions(&lens, &codewords);
}

#[test]
fn test_huffman_degenerate_single_symbol() {
    let mut freqs = [0u32; DEFLATE_NUM_OFFSET_SYMS];
    freqs[17] = 100;

    let mut lens = [0u8; DEFLATE_NUM_OFFSET_SYMS];
    let mut codewords = [0u32; DEFLATE_NUM_OFFSET_SYMS];

    deflate_make_huffman_code(
        DEFLATE_NUM_OFFSET_SYMS,
        MAX_OFFSET_CODEWORD_LEN,
        &freqs,
        &mut lens,
        &mut codewords,
    );

    assert_eq!(lens[0], 1);
    assert_eq!(codewords[0], 0);
    assert_eq!(lens[17], 1);
    assert_eq!(codewords[17], 1);

    for i in 1..DEFLATE_NUM_OFFSET_SYMS {
        if i != 17 {
            assert_eq!(lens[i], 0);
        }
    }
}

#[test]
fn test_huffman_degenerate_zero_symbols() {
    let freqs = [0u32; DEFLATE_NUM_OFFSET_SYMS];
    let mut lens = [0u8; DEFLATE_NUM_OFFSET_SYMS];
    let mut codewords = [0u32; DEFLATE_NUM_OFFSET_SYMS];

    deflate_make_huffman_code(
        DEFLATE_NUM_OFFSET_SYMS,
        MAX_OFFSET_CODEWORD_LEN,
        &freqs,
        &mut lens,
        &mut codewords,
    );

    assert_eq!(lens[0], 1);
    assert_eq!(codewords[0], 0);
    assert_eq!(lens[1], 1);
    assert_eq!(codewords[1], 1);

    for i in 2..DEFLATE_NUM_OFFSET_SYMS {
        assert_eq!(lens[i], 0);
    }
}

#[test]
fn test_huffman_uniform_distribution() {
    let freqs = [1000u32; DEFLATE_NUM_LITLEN_SYMS];
    let mut lens = [0u8; DEFLATE_NUM_LITLEN_SYMS];
    let mut codewords = [0u32; DEFLATE_NUM_LITLEN_SYMS];

    deflate_make_huffman_code(
        DEFLATE_NUM_LITLEN_SYMS,
        MAX_LITLEN_CODEWORD_LEN,
        &freqs,
        &mut lens,
        &mut codewords,
    );

    for &len in &lens {
        assert!((8..=10).contains(&len), "uniform tree should be balanced");
    }

    let kraft = compute_kraft_scaled(&lens);
    assert!(kraft <= (1 << 15));
    assert_no_prefix_collisions(&lens, &codewords);
}

#[test]
fn test_huffman_extreme_skewed_fibonacci_distribution() {
    let mut freqs = [0u32; DEFLATE_NUM_LITLEN_SYMS];
    let mut a: u64 = 1;
    let mut b: u64 = 1;
    for i in 0..DEFLATE_NUM_LITLEN_SYMS {
        freqs[i] = (a % 100_000 + 1) as u32;
        let c = (a + b) % 1_000_000;
        a = b;
        b = c;
    }

    let mut lens = [0u8; DEFLATE_NUM_LITLEN_SYMS];
    let mut codewords = [0u32; DEFLATE_NUM_LITLEN_SYMS];

    deflate_make_huffman_code(
        DEFLATE_NUM_LITLEN_SYMS,
        MAX_LITLEN_CODEWORD_LEN,
        &freqs,
        &mut lens,
        &mut codewords,
    );

    for &len in &lens {
        assert!(len <= 14);
        assert!(len > 0);
    }
    assert_no_prefix_collisions(&lens, &codewords);
}

// MARK: - Precode RLE Tests

#[test]
fn test_precode_rle_roundtrip_reconstruction() {
    let mut test_lens = vec![0u8; 2];
    test_lens.extend(std::iter::repeat_n(0u8, 5));
    test_lens.extend(std::iter::repeat_n(0u8, 50));
    test_lens.extend(std::iter::repeat_n(8u8, 6));
    test_lens.extend(std::iter::repeat_n(12u8, 1));
    test_lens.extend(std::iter::repeat_n(4u8, 15));
    test_lens.extend(std::iter::repeat_n(0u8, 150));

    let mut precode_freqs = [0u32; DEFLATE_NUM_PRECODE_SYMS];
    let mut items = Vec::new();
    let num_items = compute_precode_items(&test_lens, &mut precode_freqs, &mut items);
    assert_eq!(num_items, items.len());

    let mut decoded_lens = Vec::new();
    let mut prev_len = 0u8;

    for &item in &items {
        let sym = (item & 0x1F) as usize;
        let extra = item >> 5;

        match sym {
            0..=15 => {
                let len = sym as u8;
                decoded_lens.push(len);
                prev_len = len;
            }
            16 => {
                let count = 3 + extra as usize;
                for _ in 0..count {
                    decoded_lens.push(prev_len);
                }
            }
            17 => {
                let count = 3 + extra as usize;
                decoded_lens.extend(std::iter::repeat_n(0u8, count));
                prev_len = 0;
            }
            18 => {
                let count = 11 + extra as usize;
                decoded_lens.extend(std::iter::repeat_n(0u8, count));
                prev_len = 0;
            }
            _ => panic!("invalid precode symbol {sym}"),
        }
    }

    assert_eq!(
        decoded_lens, test_lens,
        "Precode RLE decompression must match input exactly"
    );
}

#[test]
fn test_precode_encoder_header_pipeline() {
    let mut litlen_lens = [0u8; DEFLATE_NUM_LITLEN_SYMS];
    for i in 0..260 {
        litlen_lens[i] = if i % 2 == 0 { 5 } else { 8 };
    }

    let mut offset_lens = [0u8; DEFLATE_NUM_OFFSET_SYMS];
    for i in 0..10 {
        offset_lens[i] = 4;
    }

    let header = PrecodeEncoder::encode_header(&litlen_lens, &offset_lens);

    assert_eq!(header.num_litlen_syms, 260);
    assert_eq!(header.num_offset_syms, 10);
    assert!(header.num_explicit_lens >= 4 && header.num_explicit_lens <= 19);
    assert!(!header.items.is_empty());

    for &len in &header.precode_lens {
        assert!(len <= MAX_PRE_CODEWORD_LEN as u8);
    }
}

#[test]
fn test_compute_num_explicit_precode_lens() {
    let mut precode_lens = [0u8; DEFLATE_NUM_PRECODE_SYMS];
    assert_eq!(compute_num_explicit_precode_lens(&precode_lens), 4);

    let last_perm_sym = DEFLATE_PRECODE_LENS_PERMUTATION[18] as usize;
    precode_lens[last_perm_sym] = 3;
    assert_eq!(compute_num_explicit_precode_lens(&precode_lens), 19);

    let seventh_sym = DEFLATE_PRECODE_LENS_PERMUTATION[6] as usize;
    precode_lens[last_perm_sym] = 0;
    precode_lens[seventh_sym] = 2;
    assert_eq!(compute_num_explicit_precode_lens(&precode_lens), 7);
}

#[test]
fn test_precode_constants_and_tables() {
    assert_eq!(DEFLATE_PRECODE_LENS_PERMUTATION.len(), 19);
    assert_eq!(DEFLATE_EXTRA_PRECODE_BITS.len(), 19);
    assert_eq!(DEFLATE_EXTRA_PRECODE_BITS[16], 2);
    assert_eq!(DEFLATE_EXTRA_PRECODE_BITS[17], 3);
    assert_eq!(DEFLATE_EXTRA_PRECODE_BITS[18], 7);
    for i in 0..16 {
        assert_eq!(DEFLATE_EXTRA_PRECODE_BITS[i], 0);
    }
}


// MARK: - FastBitWriter Tests

#[test]
fn test_fast_bit_writer_exact_readback() {
    let mut buf = [0u8; 128];
    let mut writer = FastBitWriter::new(&mut buf);

    let test_patterns: Vec<(u64, u32)> = vec![
        (0b101, 3),
        (0b11, 2),
        (0b0, 1),
        (0b11110000, 8),
        (0xDEADBEEF, 32),
        (0b10101, 5),
        (0x123456789ABCDEF0, 60),
    ];

    for &(val, bits) in &test_patterns {
        writer.add_bits(val, bits);
        writer.flush_bits();
    }

    let bytes_written = writer.finish().expect("write finish failed");
    assert!(bytes_written > 0);

    let mut reader = TestBitReader::new(&buf[..bytes_written]);
    for &(val, bits) in &test_patterns {
        let mask = if bits == 64 { !0 } else { (1u64 << bits) - 1 };
        let expected = val & mask;
        let actual = reader.read_bits(bits);
        assert_eq!(
            actual, expected,
            "Bit pattern mismatch for bit length {bits}: expected {expected:#b}, got {actual:#b}"
        );
    }
}

#[test]
fn test_fast_bit_writer_emit_literals_4x() {
    let lits = *b"ABCD";
    let mut codewords = [0u32; 256];
    let mut lens = [0u8; 256];

    codewords[b'A' as usize] = 0b001;
    lens[b'A' as usize] = 3;

    codewords[b'B' as usize] = 0b1010;
    lens[b'B' as usize] = 4;

    codewords[b'C' as usize] = 0b11;
    lens[b'C' as usize] = 2;

    codewords[b'D' as usize] = 0b01111;
    lens[b'D' as usize] = 5;

    let mut buf = [0u8; 32];
    let mut writer = FastBitWriter::new(&mut buf);
    writer.emit_literals_4x(lits, &codewords, &lens);
    let bytes_written = writer.finish().expect("finish failed");

    let mut reader = TestBitReader::new(&buf[..bytes_written]);
    assert_eq!(reader.read_bits(3), 0b001);
    assert_eq!(reader.read_bits(4), 0b1010);
    assert_eq!(reader.read_bits(2), 0b11);
    assert_eq!(reader.read_bits(5), 0b01111);
}

#[test]
fn test_fast_bit_writer_byte_align_and_overflow() {
    let mut buf = [0u8; 32];
    let mut writer = FastBitWriter::new(&mut buf);
    writer.add_bits(0b101, 3);
    writer.align_to_byte();
    writer.add_bits(0xFF, 8);
    let bytes_written = writer.finish().expect("finish failed");
    assert_eq!(bytes_written, 2);
    assert_eq!(buf[0], 0b00000101);
    assert_eq!(buf[1], 0xFF);

    let mut small_buf = [0u8; 1];
    let mut overflow_writer = FastBitWriter::new(&mut small_buf);
    overflow_writer.add_bits(0xDEADBEEF, 32);
    overflow_writer.flush_bits();
    overflow_writer.add_bits(0xCAFEBABE, 32);
    let res = overflow_writer.finish();
    assert_eq!(res, Err(FastBitWriterError::BufferOverflow));
}

#[test]
fn test_fast_bit_writer_vec() {
    let mut writer = FastBitWriterVec::new();
    writer.add_bits(0b1101, 4);
    writer.add_bits(0b0010, 4);
    writer.add_bits(0xAB, 8);
    let out = writer.finish();
    assert_eq!(out, vec![0b00101101, 0xAB]);
}

#[test]
fn test_reverse_codeword() {
    assert_eq!(reverse_codeword(0b0, 1), 0b0);
    assert_eq!(reverse_codeword(0b1, 1), 0b1);
    assert_eq!(reverse_codeword(0b100, 3), 0b001);
    assert_eq!(reverse_codeword(0b1101, 4), 0b1011);
    assert_eq!(reverse_codeword(0b10101010, 8), 0b01010101);
    assert_eq!(reverse_codeword(0, 0), 0);
}
