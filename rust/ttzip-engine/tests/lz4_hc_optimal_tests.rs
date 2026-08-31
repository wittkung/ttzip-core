// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Exhaustive verification suite for LZ4HC 256KB dual-table and DP optimal parser.
//!
//! Validates:
//! 1. Bit-Exact round-trip consistency across all compression levels (1..=12).
//! 2. Measurably superior compression ratio over LZ4 Fast mode on compressible corpuses.
//! 3. Favor Decompression Speed (`favor_dec_speed`) mode correctness and collision-free alignment.
//! 4. Zero-allocation stability on edge cases, single-byte blocks, and 64KB+ streaming payloads.

use ttzip_engine::codecs::lz4::{
    lz4_compress_fast, lz4_compress_hc_opt_custom_to_vec, lz4_compress_hc_opt_to_vec,
    lz4_decompress, lz4_decompress_to_vec, price_literals, price_sequence,
    price_sequence_speed, Lz4HcDualTable, Lz4HcParams, LZ4HC_CHAIN_SIZE, LZ4HC_HASH_SIZE,
    LZ4_OPT_NUM,
};


// MARK: - Helper Payloads

fn generate_prose_corpus() -> Vec<u8> {
    let base = b"In computer science, LZ4 is a lossless data compression algorithm that is focused on compression and decompression speed. It belongs to the LZ77 family of byte-oriented compression schemes. The algorithm gives a slightly worse compression ratio than the LZO algorithm, but gives very high compression speeds and even higher decompression speeds.";
    let mut data = Vec::with_capacity(base.len() * 40);
    for _ in 0..40 {
        data.extend_from_slice(base);
    }
    data
}

fn generate_json_structured_corpus() -> Vec<u8> {
    let mut data = Vec::new();
    for i in 0..200 {
        let record = format!(
            "{{\"id\": {}, \"uuid\": \"a1b2c3d4-e5f6-7890-abcd-123456789{:03}\", \"type\": \"compression_benchmark\", \"status\": \"success\", \"timestamp\": 1772345678, \"tags\": [\"lz4\", \"hc\", \"optimal\", \"ttzip\"]}}\n",
            i, i
        );
        data.extend_from_slice(record.as_bytes());
    }
    data
}

fn generate_runlength_corpus() -> Vec<u8> {
    let mut data = Vec::new();
    for byte in b"ABCDEFGHIJKLMNOPQRSTUVWXYZ" {
        for _ in 0..300 {
            data.push(*byte);
        }
    }
    data
}

// MARK: - 1. Bit-Exact Round-Trip Across All Levels 1..=12

#[test]
fn test_lz4_hc_all_levels_prose_roundtrip() {
    let src = generate_prose_corpus();

    for level in 1..=12 {
        let compressed = lz4_compress_hc_opt_to_vec(&src, level)
            .unwrap_or_else(|e| panic!("level {level} compression failed: {e:?}"));

        assert!(!compressed.is_empty(), "level {level} output was empty");
        assert!(
            compressed.len() < src.len(),
            "level {level} failed to compress prose (raw: {}, comp: {})",
            src.len(),
            compressed.len()
        );

        let decompressed = lz4_decompress_to_vec(&compressed, src.len())
            .unwrap_or_else(|e| panic!("level {level} decompression failed: {e:?}"));

        assert_eq!(
            decompressed.as_slice(),
            src.as_slice(),
            "level {level} roundtrip mismatch"
        );
    }
}

#[test]
fn test_lz4_hc_all_levels_json_roundtrip() {
    let src = generate_json_structured_corpus();

    for level in 1..=12 {
        let compressed = lz4_compress_hc_opt_to_vec(&src, level)
            .unwrap_or_else(|e| panic!("level {level} JSON compression failed: {e:?}"));

        let decompressed = lz4_decompress_to_vec(&compressed, src.len())
            .unwrap_or_else(|e| panic!("level {level} JSON decompression failed: {e:?}"));

        assert_eq!(
            decompressed.as_slice(),
            src.as_slice(),
            "level {level} JSON mismatch"
        );
    }
}

#[test]
fn test_lz4_hc_all_levels_runlength_roundtrip() {
    let src = generate_runlength_corpus();

    for level in 1..=12 {
        let compressed = lz4_compress_hc_opt_to_vec(&src, level)
            .unwrap_or_else(|e| panic!("level {level} RLE compression failed: {e:?}"));

        let decompressed = lz4_decompress_to_vec(&compressed, src.len())
            .unwrap_or_else(|e| panic!("level {level} RLE decompression failed: {e:?}"));

        assert_eq!(
            decompressed.as_slice(),
            src.as_slice(),
            "level {level} RLE mismatch"
        );
    }
}

// MARK: - 2. Compression Ratio Superiority vs Fast Mode

#[test]
fn test_lz4_hc_optimal_ratio_vs_fast() {
    let src = generate_json_structured_corpus();

    // Fast Mode (accel = 1)
    let mut fast_dst = vec![0u8; src.len() * 2];
    let fast_len = lz4_compress_fast(&src, &mut fast_dst, 1).expect("fast compress");

    // LZ4HC Mode (Level 9 / DP Optimal)
    let hc_comp = lz4_compress_hc_opt_to_vec(&src, 9).expect("hc opt compress");

    // LZ4HC Level 12 (Maximum Search Depth)
    let hc12_comp = lz4_compress_hc_opt_to_vec(&src, 12).expect("hc 12 compress");

    assert!(
        hc_comp.len() <= fast_len,
        "LZ4HC level 9 ({} bytes) should achieve equal or better ratio than Fast ({} bytes)",
        hc_comp.len(),
        fast_len
    );

    assert!(
        hc12_comp.len() <= hc_comp.len(),
        "LZ4HC level 12 ({} bytes) should be <= level 9 ({} bytes)",
        hc12_comp.len(),
        hc_comp.len()
    );
}

// MARK: - 3. Favor Decompression Speed Mode Verification

#[test]
fn test_lz4_hc_favor_dec_speed_all_strategies() {
    let src = generate_prose_corpus();

    for level in [1, 4, 7, 9, 12] {
        let params = Lz4HcParams::for_level(level).with_favor_dec_speed(true);
        assert!(params.favor_dec_speed);

        let compressed = lz4_compress_hc_opt_custom_to_vec(&src, &params)
            .unwrap_or_else(|e| panic!("favor_dec_speed level {level} failed: {e:?}"));

        let decompressed = lz4_decompress_to_vec(&compressed, src.len())
            .unwrap_or_else(|e| panic!("favor_dec_speed decompress level {level} failed: {e:?}"));

        assert_eq!(
            decompressed.as_slice(),
            src.as_slice(),
            "favor_dec_speed level {level} data mismatch"
        );
    }
}

// MARK: - 4. Edge Cases & Boundary Conditions

#[test]
fn test_lz4_hc_empty_and_sub_mflimit_inputs() {
    // Empty
    let empty: [u8; 0] = [];
    let comp_empty = lz4_compress_hc_opt_to_vec(&empty, 9).expect("empty compress");
    assert!(comp_empty.is_empty());
    let mut decomp_empty = [0u8; 4];
    let d_len = lz4_decompress(&comp_empty, &mut decomp_empty).expect("empty decompress");
    assert_eq!(d_len, 0);

    // Sub-MFLIMIT (e.g. 1..=11 bytes)
    for len in 1..12 {
        let input = vec![0xABu8; len];
        let comp = lz4_compress_hc_opt_to_vec(&input, 9).expect("sub-mflimit compress");
        let decomp = lz4_decompress_to_vec(&comp, len).expect("sub-mflimit decompress");
        assert_eq!(decomp.as_slice(), input.as_slice());
    }
}

#[test]
fn test_lz4_hc_large_window_span() {
    // Large payload (> 128 KB) exceeding the 64KB distance window to stress circular table
    let mut data = Vec::with_capacity(160 * 1024);
    for chunk in 0..160 {
        for byte_idx in 0..1024 {
            data.push(((chunk * 7 + byte_idx * 13) & 0xFF) as u8);
        }
    }

    let comp = lz4_compress_hc_opt_to_vec(&data, 9).expect("large compress");
    assert!(!comp.is_empty());

    let decomp = lz4_decompress_to_vec(&comp, data.len()).expect("large decompress");
    assert_eq!(decomp.as_slice(), data.as_slice());
}

#[test]
fn test_lz4_hc_incompressible_random_data() {
    // High-entropy random-like payload
    let mut random_data = Vec::with_capacity(4096);
    let mut seed = 0x12345678u64;
    for _ in 0..4096 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        random_data.push((seed >> 32) as u8);
    }

    let comp = lz4_compress_hc_opt_to_vec(&random_data, 9).expect("incompressible compress");
    let decomp = lz4_decompress_to_vec(&comp, random_data.len()).expect("incompressible decompress");
    assert_eq!(decomp.as_slice(), random_data.as_slice());
}

// MARK: - 5. Price Model and Table Architecture Invariants

#[test]
fn test_price_model_invariants() {
    // Literal prices
    assert_eq!(price_literals(0), 1);
    assert_eq!(price_literals(14), 15);
    assert_eq!(price_literals(15), 17); // 1 token + 1 extra len byte + 15 lit bytes
    assert_eq!(price_literals(269), 271);
    assert_eq!(price_literals(270), 273); // 1 token + 2 extra len bytes + 270 lit bytes

    // Sequence prices
    assert_eq!(price_sequence(0, 4), 3); // 1 token + 2 offset + 0 extra
    assert_eq!(price_sequence(15, 4), 19); // 1 token + 1 extra + 15 lits + 2 offset
    assert_eq!(price_sequence(0, 19), 4); // 1 token + 2 offset + 1 extra match len

    // Speed bias penalties
    assert_eq!(price_sequence_speed(0, 4, 2, false), 3);
    assert_eq!(price_sequence_speed(0, 4, 2, true), 6);
    assert_eq!(price_sequence_speed(0, 18, 2, true), 3);
}

#[test]
fn test_dual_table_constants_and_geometry() {
    assert_eq!(LZ4HC_HASH_SIZE, 32768);
    assert_eq!(LZ4HC_CHAIN_SIZE, 65536);
    assert_eq!(LZ4_OPT_NUM, 4100);

    let mut dt = Lz4HcDualTable::new();
    dt.reset();
    assert_eq!(dt.hash_table.len(), 32768);
    assert_eq!(dt.chain_table.len(), 65536);
}
