// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Exhaustive verification suite for LZ4 Knuth golden ratio hashing, prime multipliers,
//! small-offset lookup tables, broadcast replication, and in-place decompression safety margins.

use std::collections::HashSet;
use ttzip_engine::codecs::lz4::{
    copy_small_offset, lz4_decompress_inplace_buffer_size, lz4_decompress_inplace_margin,
    lz4_hash4, lz4_hash4_bytes, lz4_hash5, lz4_hash5_slice, lz4_hash8, DEC64_TABLE,
    INC32_TABLE, KNUTH_GOLDEN_RATIO_32, PRIME_5BYTES_64, PRIME_8BYTES_64,
};

// MARK: - 1. Hash Primitives, Constants & Distribution Tests

#[test]
fn test_hash_constants_exact_values() {
    assert_eq!(KNUTH_GOLDEN_RATIO_32, 2654435761u32);
    assert_eq!(KNUTH_GOLDEN_RATIO_32, 0x9E3779B1u32);

    assert_eq!(PRIME_5BYTES_64, 889523592379u64);
    assert_eq!(PRIME_5BYTES_64, 0x0000_00CF_1BBC_DCBBu64);

    assert_eq!(PRIME_8BYTES_64, 11400714785074694791u64);
    assert_eq!(PRIME_8BYTES_64, 0x9E37_79B1_85EB_CA87u64);
}

#[test]
fn test_hash_determinism_and_range_bounds() {
    for log in 10..=16 {
        let max_bound = 1u32 << log;

        for seq32 in [
            0x00000000u32,
            0x00000001,
            0x12345678,
            0x55555555,
            0xAAAAAAAA,
            0xFFFFFFFF,
            0xDEADBEEF,
            0xCAFEBABE,
        ] {
            let h1 = lz4_hash4(seq32, log);
            let h2 = lz4_hash4(seq32, log);
            assert_eq!(h1, h2, "lz4_hash4 must be deterministic");
            assert!(
                h1 < max_bound,
                "lz4_hash4({seq32:#x}, {log}) = {h1} exceeded max bound {max_bound}"
            );
        }

        for seq64 in [
            0x0000000000000000u64,
            0x0000000000000001,
            0x000000123456789A,
            0x0000005555555555,
            0x000000AAAAAAAAAA,
            0x000000FFFFFFFFFF,
            0x000000DEADBEEF01,
        ] {
            let h1 = lz4_hash5(seq64, log);
            let h2 = lz4_hash5(seq64, log);
            assert_eq!(h1, h2, "lz4_hash5 must be deterministic");
            assert!(
                h1 < max_bound,
                "lz4_hash5({seq64:#x}, {log}) = {h1} exceeded max bound {max_bound}"
            );
        }

        for seq64 in [
            0x0000000000000000u64,
            0x0123456789ABCDEF,
            0xFFFFFFFFFFFFFFFF,
            0x9E3779B97F4A7C15,
        ] {
            let h = lz4_hash8(seq64, log);
            assert!(
                h < max_bound,
                "lz4_hash8({seq64:#x}, {log}) = {h} exceeded max bound {max_bound}"
            );
        }
    }
}

#[test]
fn test_hash4_distribution_and_avalanche() {
    let hash_log = 12u32;
    let table_size = 1usize << hash_log; // 4096 buckets
    let num_samples = 10_000usize;

    let mut unique_hashes = HashSet::with_capacity(num_samples);
    let mut bucket_counts = vec![0usize; table_size];

    for i in 0..num_samples {
        // High variation pseudo-linear sequence
        let seq = (i as u32)
            .wrapping_mul(0x45D9F3B)
            .wrapping_add(0x11223344);
        let h = lz4_hash4(seq, hash_log) as usize;
        bucket_counts[h] += 1;
        unique_hashes.insert(h);
    }

    // Measure table occupancy (at 10000 items in 4096 buckets, > 85% buckets should be occupied)
    let occupied_buckets = bucket_counts.iter().filter(|&&c| c > 0).count();
    let occupancy_rate = occupied_buckets as f64 / table_size as f64;
    assert!(
        occupancy_rate > 0.85,
        "lz4_hash4 bucket occupancy too low: {occupancy_rate:.4} (expected > 0.85)"
    );

    // Avalanche bit-flip sensitivity test
    let base_seq = 0x12345678u32;
    let base_hash = lz4_hash4(base_seq, 16);
    let mut bit_changes = 0usize;

    for bit in 0..32 {
        let flipped_seq = base_seq ^ (1u32 << bit);
        let flipped_hash = lz4_hash4(flipped_seq, 16);
        let diff_bits = (base_hash ^ flipped_hash).count_ones();
        bit_changes += diff_bits as usize;
    }

    let avg_bit_flips = bit_changes as f64 / 32.0;
    assert!(
        avg_bit_flips >= 4.0,
        "lz4_hash4 avalanche effect insufficient: avg flipped bits = {avg_bit_flips:.2} (expected >= 4.0)"
    );
}

#[test]
fn test_hash5_distribution_uniformity() {
    let hash_log = 14u32;
    let table_size = 1usize << hash_log; // 16384 buckets
    let num_samples = 20_000usize;

    let mut bucket_counts = vec![0usize; table_size];

    for i in 0..num_samples {
        // Generate diverse 5-byte sequences
        let low32 = (i as u32).wrapping_mul(0x9E3779B9);
        let high8 = ((i >> 16) & 0xFF) as u64;
        let seq5 = (high8 << 32) | (low32 as u64);

        let h = lz4_hash5(seq5, hash_log) as usize;
        bucket_counts[h] += 1;
    }

    let occupied = bucket_counts.iter().filter(|&&c| c > 0).count();
    let occupancy = occupied as f64 / table_size as f64;
    assert!(
        occupancy > 0.65,
        "lz4_hash5 occupancy too low: {occupancy:.4} (expected > 0.65)"
    );
}

#[test]
fn test_hash_helpers_byte_compatibility() {
    let raw4: [u8; 4] = [0x78, 0x56, 0x34, 0x12];
    let seq32 = u32::from_le_bytes(raw4);
    assert_eq!(lz4_hash4_bytes(&raw4, 12), lz4_hash4(seq32, 12));

    let raw8: [u8; 8] = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
    let h5 = lz4_hash5_slice(&raw8[..5], 14);
    let mut buf5 = [0u8; 8];
    buf5[..5].copy_from_slice(&raw8[..5]);
    assert_eq!(h5, lz4_hash5(u64::from_le_bytes(buf5), 14));
}

// MARK: - 2. Small Offset LUT & Broadcast Replication Tests

#[test]
fn test_lut_tables_exact_values() {
    assert_eq!(INC32_TABLE, [0, 1, 2, 1, 0, 4, 4, 4]);
    assert_eq!(DEC64_TABLE, [0, 0, 0, -1, -4, 1, 2, 3]);
}

fn reference_copy_pattern(dst: &mut [u8], src: &[u8], offset: usize, length: usize) {
    if length == 0 || dst.is_empty() || src.is_empty() || offset == 0 {
        return;
    }
    let count = length.min(dst.len());
    let pat_len = offset.min(src.len());
    for i in 0..count {
        dst[i] = src[i % pat_len];
    }
}

#[test]
fn test_copy_small_offset_1_byte_fill_matrix() {
    let src = [0x7E];
    for len in [0, 1, 2, 3, 7, 8, 9, 15, 16, 17, 31, 32, 63, 64, 128, 1024] {
        let mut actual = vec![0u8; len];
        let mut expected = vec![0u8; len];

        copy_small_offset(&mut actual, &src, 1, len);
        reference_copy_pattern(&mut expected, &src, 1, len);

        assert_eq!(actual, expected, "Mismatch at offset 1 for length {len}");
    }
}

#[test]
fn test_copy_small_offset_2_bytes_broadcast_matrix() {
    let src = [0x55, 0xAA];
    for len in [0, 1, 2, 3, 4, 7, 8, 9, 15, 16, 17, 23, 24, 31, 32, 64, 100] {
        let mut actual = vec![0u8; len];
        let mut expected = vec![0u8; len];

        copy_small_offset(&mut actual, &src, 2, len);
        reference_copy_pattern(&mut expected, &src, 2, len);

        assert_eq!(actual, expected, "Mismatch at offset 2 for length {len}");
    }
}

#[test]
fn test_copy_small_offset_4_bytes_broadcast_matrix() {
    let src = [0x10, 0x20, 0x30, 0x40];
    for len in [0, 1, 2, 3, 4, 5, 7, 8, 9, 12, 15, 16, 17, 28, 32, 64, 127, 256] {
        let mut actual = vec![0u8; len];
        let mut expected = vec![0u8; len];

        copy_small_offset(&mut actual, &src, 4, len);
        reference_copy_pattern(&mut expected, &src, 4, len);

        assert_eq!(actual, expected, "Mismatch at offset 4 for length {len}");
    }
}

#[test]
fn test_copy_small_offset_all_offsets_1_to_7_differential() {
    let patterns: [&[u8]; 7] = [
        &[0xA1],
        &[0xB1, 0xB2],
        &[0xC1, 0xC2, 0xC3],
        &[0xD1, 0xD2, 0xD3, 0xD4],
        &[0xE1, 0xE2, 0xE3, 0xE4, 0xE5],
        &[0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6],
        &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07],
    ];

    for (idx, &pat) in patterns.iter().enumerate() {
        let offset = idx + 1;
        for len in 0..=80 {
            let mut actual = vec![0u8; len];
            let mut expected = vec![0u8; len];

            copy_small_offset(&mut actual, pat, offset, len);
            reference_copy_pattern(&mut expected, pat, offset, len);

            assert_eq!(
                actual, expected,
                "Differential failure for offset {offset}, length {len}"
            );
        }
    }
}

#[test]
fn test_copy_small_offset_edge_cases() {
    // 1. Zero length
    let mut dst = [0xFF; 8];
    copy_small_offset(&mut dst, &[1, 2, 3], 3, 0);
    assert_eq!(dst, [0xFF; 8]);

    // 2. Zero offset
    copy_small_offset(&mut dst, &[1, 2, 3], 0, 8);
    assert_eq!(dst, [0xFF; 8]);

    // 3. Empty source
    copy_small_offset(&mut dst, &[], 2, 8);
    assert_eq!(dst, [0xFF; 8]);

    // 4. Empty destination
    let mut empty_dst: [u8; 0] = [];
    copy_small_offset(&mut empty_dst, &[1, 2], 2, 10);
    assert!(empty_dst.is_empty());

    // 5. Length exceeds destination capacity: should safely clamp to dst.len()
    let mut small_dst = [0u8; 4];
    copy_small_offset(&mut small_dst, &[0x42], 1, 100);
    assert_eq!(small_dst, [0x42; 4]);
}

// MARK: - 3. In-place Decompression Safety Margins & Bounds

#[test]
fn test_inplace_margin_exact_values() {
    // 0 Bytes
    assert_eq!(lz4_decompress_inplace_margin(0), 32);
    assert_eq!(lz4_decompress_inplace_buffer_size(0), 32);

    // 1 KB (1024 Bytes)
    assert_eq!(lz4_decompress_inplace_margin(1024), 36);
    assert_eq!(lz4_decompress_inplace_buffer_size(1024), 1060);

    // 64 KB (65536 Bytes)
    assert_eq!(lz4_decompress_inplace_margin(65536), 288);
    assert_eq!(lz4_decompress_inplace_buffer_size(65536), 65824);

    // 1 MB (1048576 Bytes)
    assert_eq!(lz4_decompress_inplace_margin(1048576), 4128);
    assert_eq!(lz4_decompress_inplace_buffer_size(1048576), 1052704);
}

#[test]
fn test_inplace_margin_monotonicity_and_saturation() {
    let mut prev_margin = 0usize;
    for &size in &[0, 255, 256, 512, 1024, 4096, 65536, 1048576, 16777216] {
        let margin = lz4_decompress_inplace_margin(size);
        let buf_size = lz4_decompress_inplace_buffer_size(size);

        assert!(
            margin >= prev_margin,
            "Margin must be monotonic: size {size} has margin {margin} < {prev_margin}"
        );
        assert_eq!(buf_size, size + margin);
        prev_margin = margin;
    }

    // Overflow protection with saturating arithmetic
    let huge_size = usize::MAX - 10;
    let huge_buf = lz4_decompress_inplace_buffer_size(huge_size);
    assert_eq!(huge_buf, usize::MAX);
}
