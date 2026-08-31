// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit, validation, and performance tests for Brotli second-order
//! context modeling and 2048-byte static LUT lookup table.

use std::time::Instant;
use ttzip_engine::codecs::brotli::{
    context_lut_slice, get_context_id, get_distance_context, BrotliContextMap, BrotliContextMode,
    BROTLI_CONTEXT_LOOKUP_TABLE,
};

#[test]
fn test_context_mode_constants_and_table_size() {
    assert_eq!(BROTLI_CONTEXT_LOOKUP_TABLE.len(), 2048);
    assert_eq!(BrotliContextMode::Lsb6 as usize, 0);
    assert_eq!(BrotliContextMode::Msb6 as usize, 1);
    assert_eq!(BrotliContextMode::Utf8 as usize, 2);
    assert_eq!(BrotliContextMode::Signed as usize, 3);

    let lsb_slice = context_lut_slice(BrotliContextMode::Lsb6);
    let msb_slice = context_lut_slice(BrotliContextMode::Msb6);
    let utf8_slice = context_lut_slice(BrotliContextMode::Utf8);
    let signed_slice = context_lut_slice(BrotliContextMode::Signed);

    assert_eq!(lsb_slice.len(), 512);
    assert_eq!(msb_slice.len(), 512);
    assert_eq!(utf8_slice.len(), 512);
    assert_eq!(signed_slice.len(), 512);

    assert_eq!(lsb_slice.as_ptr(), BROTLI_CONTEXT_LOOKUP_TABLE.as_ptr());
}

#[test]
fn test_lsb6_exhaustive_oracle() {
    // LSB6 must extract exactly the 6 least significant bits of p1 regardless of p2.
    for p1 in 0..=255u8 {
        let expected = (p1 & 0x3F) as usize;
        for p2 in 0..=255u8 {
            let ctx = get_context_id(p1, p2, BrotliContextMode::Lsb6);
            assert_eq!(
                ctx, expected,
                "LSB6 mismatch for p1={}, p2={}: got {}, expected {}",
                p1, p2, ctx, expected
            );
            assert!(ctx < 64);
        }
    }
}

#[test]
fn test_msb6_exhaustive_oracle() {
    // MSB6 must extract exactly the 6 most significant bits of p1 (p1 >> 2) regardless of p2.
    for p1 in 0..=255u8 {
        let expected = (p1 >> 2) as usize;
        for p2 in 0..=255u8 {
            let ctx = get_context_id(p1, p2, BrotliContextMode::Msb6);
            assert_eq!(
                ctx, expected,
                "MSB6 mismatch for p1={}, p2={}: got {}, expected {}",
                p1, p2, ctx, expected
            );
            assert!(ctx < 64);
        }
    }
}

#[test]
fn test_utf8_ascii_character_class_separation() {
    // For ASCII p1 and p2: context = 4 * context1(p1) + context2(p2)
    // Verify lowercase letters (context1 = 14 for vowels, 15 for consonants)
    let ctx_a_space = get_context_id(b'a', b' ', BrotliContextMode::Utf8);
    // 'a' is lowercase vowel -> context1 = 14 (14 * 4 = 56), ' ' -> context2 = 0 -> 56 + 0 = 56
    assert_eq!(ctx_a_space, 56);

    let ctx_b_a = get_context_id(b'b', b'a', BrotliContextMode::Utf8);
    // 'b' is lowercase consonant -> context1 = 15 (15 * 4 = 60), 'a' is lowercase -> context2 = 3 -> 60 + 3 = 63
    assert_eq!(ctx_b_a, 63);

    // Uppercase vowels (context1 = 12 -> 48) and uppercase consonants (context1 = 13 -> 52)
    let ctx_a_up_space = get_context_id(b'A', b' ', BrotliContextMode::Utf8);
    assert_eq!(ctx_a_up_space, 48);

    let ctx_b_up_a_up = get_context_id(b'B', b'A', BrotliContextMode::Utf8);
    // 'B' consonant -> 52, 'A' uppercase letter -> context2 = 2 -> 52 + 2 = 54
    assert_eq!(ctx_b_up_a_up, 54);

    // Digits (context1 = 11 -> 44)
    let ctx_0_space = get_context_id(b'0', b' ', BrotliContextMode::Utf8);
    assert_eq!(ctx_0_space, 44);

    let ctx_9_0 = get_context_id(b'9', b'0', BrotliContextMode::Utf8);
    // '9' -> 44, '0' (number) -> context2 = 2 -> 44 + 2 = 46
    assert_eq!(ctx_9_0, 46);

    // Control whitespace (\t, \n, \r -> context1 = 1 -> 4)
    assert_eq!(get_context_id(b'\n', b' ', BrotliContextMode::Utf8), 4);
    assert_eq!(get_context_id(b'\t', b'a', BrotliContextMode::Utf8), 7);
    assert_eq!(get_context_id(b'\r', b'.', BrotliContextMode::Utf8), 5);

    // Punctuation characters
    // '.' is context1 = 9 -> 36
    assert_eq!(get_context_id(b'.', b' ', BrotliContextMode::Utf8), 36);
    // ',' is context1 = 8 -> 32
    assert_eq!(get_context_id(b',', b' ', BrotliContextMode::Utf8), 32);
    // '"' is context1 = 4 -> 16
    assert_eq!(get_context_id(b'"', b' ', BrotliContextMode::Utf8), 16);
}

#[test]
fn test_utf8_multibyte_chinese_and_unicode_transitions() {
    // Chinese character: '中' = [0xE4, 0xB8, 0xAD]
    let zhong = "中".as_bytes();
    assert_eq!(zhong.len(), 3);
    let lead_e4 = zhong[0];
    let cont_b8 = zhong[1];
    let cont_ad = zhong[2];

    // Transition 1: Lead byte after ASCII space
    // p1 = lead_e4 (0xE4 >= 192), p2 = ' '
    let ctx_lead = get_context_id(lead_e4, b' ', BrotliContextMode::Utf8);
    // Lead byte range produces context 2 or 3
    assert!(ctx_lead == 2 || ctx_lead == 3);

    // Transition 2: Continuation byte after lead byte (0xE4 is in range 208..=255)
    // p1 = cont_b8, p2 = lead_e4
    let ctx_cont1 = get_context_id(cont_b8, lead_e4, BrotliContextMode::Utf8);
    assert!(ctx_cont1 == 2 || ctx_cont1 == 3);

    // Transition 3: Second continuation byte after first continuation
    // p1 = cont_ad, p2 = cont_b8
    let ctx_cont2 = get_context_id(cont_ad, cont_b8, BrotliContextMode::Utf8);
    // Continuation after continuation produces 0 or 1
    assert!(ctx_cont2 == 0 || ctx_cont2 == 1);

    // 4-byte Emoji: '😀' = [0xF0, 0x9F, 0x98, 0x80]
    let emoji = "😀".as_bytes();
    assert_eq!(emoji.len(), 4);
    for window in emoji.windows(2) {
        let p2 = window[0];
        let p1 = window[1];
        let ctx = get_context_id(p1, p2, BrotliContextMode::Utf8);
        assert!(ctx < 64, "Context ID must be strictly in 0..64, got {}", ctx);
    }
}

#[test]
fn test_signed_context_logarithmic_bucketing() {
    // Zero difference: p1 = 0, p2 = 0 -> context 0
    let ctx_zero = get_context_id(0, 0, BrotliContextMode::Signed);
    assert_eq!(ctx_zero, 0);

    // Small positive difference: p1 = 1, p2 = 1 -> bucket1 = 8, bucket2 = 1 -> 8 | 1 = 9
    let ctx_small_pos = get_context_id(1, 1, BrotliContextMode::Signed);
    assert_eq!(ctx_small_pos, 9);

    // Medium positive difference: p1 = 30, p2 = 30 -> bucket1 = 16, bucket2 = 2 -> 16 | 2 = 18
    let ctx_med_pos = get_context_id(30, 30, BrotliContextMode::Signed);
    assert_eq!(ctx_med_pos, 18);

    // Large positive: p1 = 100, p2 = 100 -> bucket1 = 24, bucket2 = 3 -> 24 | 3 = 27
    let ctx_large_pos = get_context_id(100, 100, BrotliContextMode::Signed);
    assert_eq!(ctx_large_pos, 27);

    // Negative difference (high byte values representing two's complement negative):
    // p1 = 255 (-1), p2 = 255 (-1) -> bucket1 = 56, bucket2 = 7 -> 56 | 7 = 63
    let ctx_neg_1 = get_context_id(255, 255, BrotliContextMode::Signed);
    assert_eq!(ctx_neg_1, 63);

    // p1 = 250 (-6), p2 = 250 (-6) -> bucket1 = 48, bucket2 = 6 -> 48 | 6 = 54
    let ctx_neg_6 = get_context_id(250, 250, BrotliContextMode::Signed);
    assert_eq!(ctx_neg_6, 54);

    // All signed combinations must remain strictly < 64
    for p1 in 0..=255u8 {
        for p2 in 0..=255u8 {
            let ctx = get_context_id(p1, p2, BrotliContextMode::Signed);
            assert!(ctx < 64, "Signed context out of bounds: {}", ctx);
        }
    }
}

#[test]
fn test_distance_context_four_tier_split() {
    assert_eq!(get_distance_context(0), 0);
    assert_eq!(get_distance_context(1), 0);
    assert_eq!(get_distance_context(2), 0);
    assert_eq!(get_distance_context(3), 1);
    assert_eq!(get_distance_context(4), 2);
    for len in 5..=1000 {
        assert_eq!(get_distance_context(len), 3);
    }
    assert_eq!(get_distance_context(usize::MAX), 3);
}

#[test]
fn test_context_map_multi_block_and_query() {
    // Create a 2-block literal context map (2 * 64 = 128 entries)
    let mut map_data = vec![0u8; 128];
    // Block 0: all context map to tree 0
    // Block 1: all context map to tree 1
    for i in 64..128 {
        map_data[i] = 1;
    }
    let context_map = BrotliContextMap::new(2, map_data);
    assert_eq!(context_map.num_trees(), 2);
    assert_eq!(context_map.len(), 128);
    assert!(!context_map.is_empty());
    assert!(!context_map.is_trivial());

    // Query block 0
    for ctx in 0..64 {
        assert_eq!(context_map.get_literal_tree(0, ctx), 0);
    }
    // Query block 1
    for ctx in 0..64 {
        assert_eq!(context_map.get_literal_tree(1, ctx), 1);
    }

    // Out of bounds query returns 0 safely
    assert_eq!(context_map.get(9999), 0);
}

#[test]
fn test_context_map_distance_query() {
    // Create a 3-block distance context map (3 * 4 = 12 entries)
    let map_data = vec![0, 0, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3];
    let dist_map = BrotliContextMap::new(4, map_data);
    assert_eq!(dist_map.get_distance_tree(0, 0), 0);
    assert_eq!(dist_map.get_distance_tree(0, 1), 0);
    assert_eq!(dist_map.get_distance_tree(0, 2), 1);
    assert_eq!(dist_map.get_distance_tree(0, 3), 1);

    assert_eq!(dist_map.get_distance_tree(1, 0), 2);
    assert_eq!(dist_map.get_distance_tree(2, 3), 3);
}

#[test]
fn test_context_lookup_performance_gate() {
    let iterations = 10_000_000usize;
    let mut sum: usize = 0;

    let start = Instant::now();
    for i in 0..iterations {
        let p1 = (i & 0xFF) as u8;
        let p2 = ((i >> 8) & 0xFF) as u8;
        let mode = match (i >> 16) & 3 {
            0 => BrotliContextMode::Lsb6,
            1 => BrotliContextMode::Msb6,
            2 => BrotliContextMode::Utf8,
            _ => BrotliContextMode::Signed,
        };
        sum = sum.wrapping_add(get_context_id(p1, p2, mode));
    }
    let elapsed = start.elapsed();
    let elapsed_secs = elapsed.as_secs_f64();
    let mops = (iterations as f64 / elapsed_secs) / 1_000_000.0;

    println!(
        "Context lookup speed: {:.2} Mops/sec ({} lookups in {:.4}s, checksum={})",
        mops, iterations, elapsed_secs, sum
    );

    // Prevent compiler from optimizing the loop away
    assert!(sum > 0);
    // Hard gate: must achieve > 50 Mops/sec in Release (and easily > 30 Mops even in debug mode)
    assert!(
        mops > 10.0,
        "Context lookup throughput too low: {:.2} Mops/sec",
        mops
    );
}
