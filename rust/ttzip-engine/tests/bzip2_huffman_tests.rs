// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit tests for Bzip2 canonical Huffman code length generation and decoding.

use ttzip_engine::codecs::bzip2::huffman::{
    hb_assign_codes, hb_create_decode_tables, hb_make_code_lengths, huffman_decode_symbol,
    BitReader, BZ_MAX_CODE_LEN,
};
use ttzip_engine::codecs::bzip2::block::BitWriter;

#[test]
fn test_huffman_kraft_inequality_and_depth() {
    let alpha_size = 10;
    let freq = [1000, 500, 250, 125, 60, 30, 15, 8, 4, 1];
    let mut lengths = vec![0u8; alpha_size];
    hb_make_code_lengths(&mut lengths, &freq, alpha_size, 17);

    for &l in &lengths {
        assert!((1..=17).contains(&l), "Length {} out of bounds", l);
    }

    // Verify Kraft inequality: sum(2^(-l)) <= 1.0
    let mut kraft_sum = 0.0f64;
    for &l in &lengths {
        kraft_sum += 2.0f64.powi(-(l as i32));
    }
    assert!(kraft_sum <= 1.0000001, "Kraft inequality violated: {}", kraft_sum);
}

#[test]
fn test_huffman_encode_decode_roundtrip() {
    let alpha_size = 6;
    let freq = [500, 200, 100, 50, 25, 10];
    let mut lengths = vec![0u8; alpha_size];
    hb_make_code_lengths(&mut lengths, &freq, alpha_size, 17);

    let mut min_len = 20;
    let mut max_len = 1;
    for &l in &lengths {
        min_len = min_len.min(l as usize);
        max_len = max_len.max(l as usize);
    }

    let mut codes = vec![0i32; alpha_size];
    hb_assign_codes(&mut codes, &lengths, min_len, max_len, alpha_size);

    let mut limit = vec![0i32; BZ_MAX_CODE_LEN + 2];
    let mut base = vec![0i32; BZ_MAX_CODE_LEN + 2];
    let mut perm = vec![0i32; alpha_size];
    hb_create_decode_tables(&mut limit, &mut base, &mut perm, &lengths, min_len, max_len, alpha_size);

    // Encode a sequence of symbols
    let symbols = [0u16, 1, 2, 3, 4, 5, 0, 0, 1, 4, 3, 2];
    let mut writer = BitWriter::new();
    for &s in &symbols {
        let sym_idx = s as usize;
        writer.write_bits(codes[sym_idx] as u32, lengths[sym_idx] as u32);
    }
    writer.flush_to_byte_boundary();

    // Decode with BitReader
    let mut reader = BitReader::new(&writer.buf);
    let mut decoded = Vec::new();
    for _ in 0..symbols.len() {
        let sym = huffman_decode_symbol(&mut reader, &limit, &base, &perm, min_len).unwrap();
        decoded.push(sym);
    }
    assert_eq!(&decoded, &symbols);
}
