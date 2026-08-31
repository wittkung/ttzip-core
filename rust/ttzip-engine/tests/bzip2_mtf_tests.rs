// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit tests for Bzip2 RLE1, MTF, and RLE2 codecs.

use ttzip_engine::codecs::bzip2::mtf::{
    generate_mtf_values, rle1_compress, rle1_decompress, rle2_decode_and_inverse_mtf,
    MAX_ALPHA_SIZE,
};

#[test]
fn test_rle1_short_and_run_lengths() {
    let input = b"AAAAAAABBBCCCCCCCCDD";
    let mut compressed = Vec::new();
    rle1_compress(input, &mut compressed);

    let mut decompressed = Vec::new();
    rle1_decompress(&compressed, &mut decompressed).unwrap();
    assert_eq!(&decompressed, input);

    // 255 run of same byte
    let long_run = vec![0xFE; 255];
    let mut comp_long = Vec::new();
    rle1_compress(&long_run, &mut comp_long);
    let mut decomp_long = Vec::new();
    rle1_decompress(&comp_long, &mut decomp_long).unwrap();
    assert_eq!(&decomp_long, &long_run);
}

#[test]
fn test_mtf_and_rle2_roundtrip_fidelity() {
    let raw = b"nnbaaaannbaaa";
    let mut in_use = [false; 256];
    for &b in raw {
        in_use[b as usize] = true;
    }

    let mut mtf_symbols = Vec::new();
    let mut mtf_freq = [0u32; MAX_ALPHA_SIZE];
    generate_mtf_values(raw, &in_use, &mut mtf_symbols, &mut mtf_freq);

    let mut restored = Vec::new();
    rle2_decode_and_inverse_mtf(&mtf_symbols, &in_use, &mut restored).unwrap();
    assert_eq!(&restored, raw);
}
