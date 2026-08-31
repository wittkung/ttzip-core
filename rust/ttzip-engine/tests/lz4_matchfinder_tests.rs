// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Exhaustive verification suite for LZ4 `byU16`/`byU32` dual hash table matchfinder,
//! adaptive stepping acceleration, and Catch-Up backward match extension.

use ttzip_engine::codecs::lz4::{
    lz4_compress_bound, lz4_compress_fast_rust, lz4_compress_fast_rust_to_vec, lz4_decompress,
    lz4_decompress_safe_custom, Lz4FastCompressor, TableType, LASTLITERALS, LZ4_64K_LIMIT,
    LZ4_DISTANCE_MAX, LZ4_HASH_LOG, LZ4_HASH_SIZE, MFLIMIT, MINMATCH,
};
use ttzip_engine::types::TTZipStatus;

// MARK: - Helper Functions

/// Generates pseudo-random deterministic high-entropy byte sequence.
fn generate_high_entropy(len: usize, seed: u32) -> Vec<u8> {
    let mut state = seed;
    let mut data = Vec::with_capacity(len);
    for _ in 0..len {
        // LCG multiplier & increment (Knuth)
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        data.push((state >> 24) as u8);
    }
    data
}

/// Generates structured low-entropy repetitive byte sequence.
fn generate_low_entropy_repeats(pattern: &[u8], repeats: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(pattern.len() * repeats);
    for _ in 0..repeats {
        data.extend_from_slice(pattern);
    }
    data
}

// MARK: - 1. TableType & Constant Invariants

#[test]
fn test_table_type_constants_and_properties() {
    assert_eq!(MINMATCH, 4);
    assert_eq!(MFLIMIT, 12);
    assert_eq!(LASTLITERALS, 5);
    assert_eq!(LZ4_64K_LIMIT, 65536);
    assert_eq!(LZ4_DISTANCE_MAX, 65535);
    assert_eq!(LZ4_HASH_LOG, 15);
    assert_eq!(LZ4_HASH_SIZE, 32768);

    assert_eq!(TableType::ByU16.hash_log(), 15);
    assert_eq!(TableType::ByU32.hash_log(), 15);
    assert_eq!(TableType::ByU16.table_size(), 32768);
    assert_eq!(TableType::ByU32.table_size(), 32768);

    // Auto-select threshold tests
    assert_eq!(TableType::auto_select(0), TableType::ByU16);
    assert_eq!(TableType::auto_select(1), TableType::ByU16);
    assert_eq!(TableType::auto_select(65535), TableType::ByU16);
    assert_eq!(TableType::auto_select(65536), TableType::ByU32);
    assert_eq!(TableType::auto_select(1024 * 1024), TableType::ByU32);
}

// MARK: - 2. Basic Roundtrip & Bit-Exact Consistency

#[test]
fn test_empty_and_small_inputs() {
    let compressor = Lz4FastCompressor::new();

    // 0 bytes
    let empty_res = compressor.compress_to_vec(&[]).expect("empty compress");
    assert!(empty_res.is_empty());

    // 1..11 bytes (< MFLIMIT = 12)
    for size in 1..12 {
        let input = generate_high_entropy(size, 42 + size as u32);
        let compressed = compressor.compress_to_vec(&input).expect("small compress");
        assert!(!compressed.is_empty());

        let mut decomp = vec![0u8; size];
        let dlen = lz4_decompress(&compressed, &mut decomp).expect("decompress");
        assert_eq!(dlen, size);
        assert_eq!(&decomp, &input);

        let mut custom_decomp = vec![0u8; size];
        let cdlen = lz4_decompress_safe_custom(&compressed, &mut custom_decomp).expect("custom decompress");
        assert_eq!(cdlen, size);
        assert_eq!(&custom_decomp, &input);
    }
}

#[test]
fn test_by_u16_and_by_u32_roundtrips_across_sizes() {
    let sizes = [
        12, 13, 16, 32, 64, 128, 256, 512, 1024, 4096, 16384, 32768, 65535, 65536, 65537,
        131072, 262144, 524288,
    ];

    for &size in &sizes {
        let pattern = b"TTZip High-Performance Fast LZ4 Matchfinder & Catch-Up backward Extension Test Block.";
        let repeats = (size / pattern.len()) + 1;
        let mut raw = generate_low_entropy_repeats(pattern, repeats);
        raw.truncate(size);

        // Test ByU16 explicitly (for sizes < 64KB)
        if size < LZ4_64K_LIMIT {
            let comp_u16 = Lz4FastCompressor::new()
                .with_table_type(TableType::ByU16)
                .compress_to_vec(&raw)
                .expect("compress byU16");
            assert!(!comp_u16.is_empty());

            let mut decomp_c = vec![0u8; size];
            let dlen_c = lz4_decompress(&comp_u16, &mut decomp_c).expect("lz4_decompress");
            assert_eq!(dlen_c, size);
            assert_eq!(&decomp_c, &raw);

            let mut decomp_rust = vec![0u8; size];
            let dlen_r = lz4_decompress_safe_custom(&comp_u16, &mut decomp_rust).expect("lz4_decompress_safe_custom");
            assert_eq!(dlen_r, size);
            assert_eq!(&decomp_rust, &raw);
        }

        // Test ByU32 explicitly (for all sizes)
        let comp_u32 = Lz4FastCompressor::new()
            .with_table_type(TableType::ByU32)
            .compress_to_vec(&raw)
            .expect("compress byU32");
        assert!(!comp_u32.is_empty());

        let mut decomp_c = vec![0u8; size];
        let dlen_c = lz4_decompress(&comp_u32, &mut decomp_c).expect("lz4_decompress");
        assert_eq!(dlen_c, size);
        assert_eq!(&decomp_c, &raw);

        let mut decomp_rust = vec![0u8; size];
        let dlen_r = lz4_decompress_safe_custom(&comp_u32, &mut decomp_rust).expect("lz4_decompress_safe_custom");
        assert_eq!(dlen_r, size);
        assert_eq!(&decomp_rust, &raw);
    }
}

// MARK: - 3. Acceleration 1..=10 Ladder Tests

#[test]
fn test_acceleration_ladder_on_repetitive_data() {
    let raw = generate_low_entropy_repeats(b"ABCDEFGHIJKLMN0123456789_Repetitive_Pattern_For_Acceleration_Test_", 500);

    for accel in 1..=10 {
        let compressor = Lz4FastCompressor::new().with_acceleration(accel);
        let compressed = compressor.compress_to_vec(&raw).expect("compress accel");
        assert!(!compressed.is_empty());
        assert!(compressed.len() < raw.len(), "repetitive data must compress");

        let mut decomp = vec![0u8; raw.len()];
        let dlen = lz4_decompress(&compressed, &mut decomp).expect("decompress");
        assert_eq!(dlen, raw.len());
        assert_eq!(&decomp, &raw);
    }
}

#[test]
fn test_acceleration_ladder_on_high_entropy_data() {
    let raw = generate_high_entropy(65536, 123456);

    for accel in 1..=10 {
        let compressed = lz4_compress_fast_rust_to_vec(&raw, accel).expect("compress high entropy");
        assert!(!compressed.is_empty());

        let mut decomp = vec![0u8; raw.len()];
        let dlen = lz4_decompress(&compressed, &mut decomp).expect("decompress");
        assert_eq!(dlen, raw.len());
        assert_eq!(&decomp, &raw);
    }
}

// MARK: - 4. Catch-Up Backward Match Extension Tests

#[test]
fn test_catch_up_backward_match_extension_boundary() {
    // Construct a specific scenario where stepping lands slightly after the start of a repeated block:
    // [Prefix: 32 bytes "ABC..."][Match Block: "COMMON_LONG_SHARED_STRING_1234567890"]
    // [Intermediary: "XYZ..."][Match Block: "COMMON_LONG_SHARED_STRING_1234567890"]
    let shared = b"COMMON_LONG_SHARED_STRING_1234567890_XYZ_ABC_TEST_PATTERN_PAYLOAD";
    let mut data = Vec::new();
    data.extend_from_slice(b"NON_MATCHING_PREFIX_INITIAL_LITERALS_0123456789_");
    data.extend_from_slice(shared);
    data.extend_from_slice(b"_RANDOM_INTERMEDIARY_LITERALS_9876543210_");
    data.extend_from_slice(shared);
    data.extend_from_slice(b"_TRAILING_LITERALS_SAFETY_ZONE_END_");

    let compressed = Lz4FastCompressor::new()
        .with_acceleration(1)
        .compress_to_vec(&data)
        .expect("compress catch-up");

    let mut decomp = vec![0u8; data.len()];
    let dlen = lz4_decompress(&compressed, &mut decomp).expect("decompress");
    assert_eq!(dlen, data.len());
    assert_eq!(&decomp, &data);
}

#[test]
fn test_catch_up_with_varying_prefix_overlaps() {
    for overlap_len in [1, 2, 3, 4, 7, 8, 15, 16, 31, 32] {
        let mut data = Vec::new();
        let overlap = vec![b'Q'; overlap_len];
        let base_pattern = b"CORE_TEST_PATTERN_SEQUENCE_PAYLOAD_FOR_OVERLAP_";

        data.extend_from_slice(b"INIT_HEADER_123_");
        data.extend_from_slice(&overlap);
        data.extend_from_slice(base_pattern);
        data.extend_from_slice(b"MIDDLE_GAP_456_");
        data.extend_from_slice(&overlap);
        data.extend_from_slice(base_pattern);
        data.extend_from_slice(b"FINAL_FOOTER_789_");

        let compressed = lz4_compress_fast_rust_to_vec(&data, 1).expect("compress overlap");

        let mut decomp = vec![0u8; data.len()];
        let dlen = lz4_decompress(&compressed, &mut decomp).expect("decompress");
        assert_eq!(dlen, data.len());
        assert_eq!(&decomp, &data);
    }
}

// MARK: - 5. Long Runs, Large Matches & Edge Cases

#[test]
fn test_single_byte_run_length_match() {
    // 32 KB of identical bytes:
    // Requires (32768 - 4 - 15) / 255 = 128 bytes of 0xFF for extra match length encoding.
    let raw = vec![0x5A; 32768];
    let compressed = lz4_compress_fast_rust_to_vec(&raw, 1).expect("compress RLE");
    assert!(compressed.len() < 200, "32KB RLE stream must compress into ~135 bytes (was {})", compressed.len());

    let mut decomp = vec![0u8; raw.len()];
    let dlen = lz4_decompress(&compressed, &mut decomp).expect("decompress RLE");
    assert_eq!(dlen, raw.len());
    assert_eq!(&decomp, &raw);

    // Also test 4KB RLE
    let raw_4k = vec![0xA5; 4096];
    let comp_4k = lz4_compress_fast_rust_to_vec(&raw_4k, 1).expect("compress 4K RLE");
    assert!(comp_4k.len() < 30, "4KB RLE stream must compress into ~20 bytes (was {})", comp_4k.len());

    let mut decomp_4k = vec![0u8; raw_4k.len()];
    let dlen_4k = lz4_decompress(&comp_4k, &mut decomp_4k).expect("decompress 4k RLE");
    assert_eq!(dlen_4k, raw_4k.len());
    assert_eq!(&decomp_4k, &raw_4k);
}

#[test]
fn test_long_literal_runs_exceeding_255_bytes() {
    // 2000 bytes of strictly distinct / high-entropy literals
    let raw = generate_high_entropy(2000, 9999);
    let compressed = lz4_compress_fast_rust_to_vec(&raw, 1).expect("compress long literals");

    let mut decomp = vec![0u8; raw.len()];
    let dlen = lz4_decompress(&compressed, &mut decomp).expect("decompress long literals");
    assert_eq!(dlen, raw.len());
    assert_eq!(&decomp, &raw);
}

#[test]
fn test_buffer_too_small_error() {
    let raw = b"Small payload test for buffer too small failure.";
    let mut small_dst = [0u8; 2]; // Insufficient capacity
    let res = Lz4FastCompressor::new().compress(raw, &mut small_dst);
    assert_eq!(res, Err(TTZipStatus::ErrCompressionFailed));
}

#[test]
fn test_max_distance_64kb_window() {
    // Create a 64KB payload with match at the very beginning and very end
    let mut raw = vec![0u8; 65536];
    let tag = b"MAX_WINDOW_MATCH_TAG_1234";
    raw[0..tag.len()].copy_from_slice(tag);
    let end_idx = 65536 - tag.len() - LASTLITERALS;
    raw[end_idx..end_idx + tag.len()].copy_from_slice(tag);

    let compressed = Lz4FastCompressor::compress_by_u32(&raw, &mut vec![0u8; lz4_compress_bound(raw.len())], 1)
        .expect("compress 64k window");
    assert!(compressed > 0);
}

#[test]
fn test_builder_and_convenience_apis() {
    let raw = b"TTZip Fast API Verification Slice Payload.";
    let mut bound_buf = vec![0u8; lz4_compress_bound(raw.len())];

    let w1 = Lz4FastCompressor::compress_by_u16(raw, &mut bound_buf, 1).expect("by_u16");
    assert!(w1 > 0);

    let w2 = Lz4FastCompressor::compress_by_u32(raw, &mut bound_buf, 2).expect("by_u32");
    assert!(w2 > 0);

    let w3 = lz4_compress_fast_rust(raw, &mut bound_buf, 3).expect("fast_rust");
    assert!(w3 > 0);

    let v = lz4_compress_fast_rust_to_vec(raw, 4).expect("to_vec");
    assert!(!v.is_empty());
}
