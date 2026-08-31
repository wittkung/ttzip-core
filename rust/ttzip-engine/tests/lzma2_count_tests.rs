// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration and differential tests for LZMA2 De Bruijn constants
//! and common prefix match length counting primitives.

use ttzip_engine::codecs::lzma2::count::{
    count_common_bytes_32_debruijn, count_common_bytes_64, count_common_bytes_64_debruijn,
    count_match_length, count_match_length_raw, DE_BRUIJN_32, DE_BRUIJN_64,
    DE_BRUIJN_BYTE_POS_32, DE_BRUIJN_BYTE_POS_64,
};

/// Reference naive byte-by-byte match counter.
fn naive_match_count(src: &[u8], matched: &[u8], max_len: usize) -> usize {
    let limit = max_len.min(src.len()).min(matched.len());
    let mut len = 0;
    while len < limit && src[len] == matched[len] {
        len += 1;
    }
    len
}

#[test]
fn test_debruijn_constants_structure() {
    assert_eq!(DE_BRUIJN_64, 0x0218_A392_CDAB_BD3F);
    assert_eq!(DE_BRUIJN_32, 0x077C_B531);
    assert_eq!(DE_BRUIJN_BYTE_POS_64.len(), 64);
    assert_eq!(DE_BRUIJN_BYTE_POS_32.len(), 32);

    // Verify lookup table bounds
    for (i, &pos) in DE_BRUIJN_BYTE_POS_64.iter().enumerate() {
        assert!(pos <= 7, "LUT index {i} out of bounds: {pos}");
    }
    for (i, &pos) in DE_BRUIJN_BYTE_POS_32.iter().enumerate() {
        assert!(pos <= 3, "32-bit LUT index {i} out of bounds: {pos}");
    }
}

#[test]
fn test_count_common_bytes_64_mathematical_determinism() {
    // Zero diff means all 8 bytes match
    assert_eq!(count_common_bytes_64(0), 8);
    assert_eq!(count_common_bytes_64_debruijn(0), 8);

    // Verify every byte boundary 0..7
    for byte_pos in 0..8 {
        for bit_offset in 0..8 {
            let bit = byte_pos * 8 + bit_offset;
            let val = 1u64 << bit;

            let count_hw = count_common_bytes_64(val);
            let count_debruijn = count_common_bytes_64_debruijn(val);

            assert_eq!(
                count_hw, byte_pos,
                "Hardware count failed at bit {bit} (expected byte {byte_pos})"
            );
            assert_eq!(
                count_debruijn, byte_pos,
                "De Bruijn count failed at bit {bit} (expected byte {byte_pos})"
            );
            assert_eq!(
                count_hw, count_debruijn,
                "Parity mismatch at bit {bit}"
            );
        }
    }
}

#[test]
fn test_debruijn_with_noisy_high_bits() {
    // Ensure that having random set bits above the lowest set bit does not affect results
    let mut rng_state: u64 = 0x1234_5678_9ABC_DEF0;

    for bit in 0..64 {
        let expected_byte = bit / 8;
        let lsb_mask = 1u64 << bit;

        for _ in 0..100 {
            // LCG pseudo-random step
            rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);

            // Generate noise in higher bits only
            let high_noise = if bit < 63 {
                rng_state << (bit + 1)
            } else {
                0
            };
            let test_val = lsb_mask | high_noise;

            let hw_res = count_common_bytes_64(test_val);
            let debruijn_res = count_common_bytes_64_debruijn(test_val);

            assert_eq!(
                hw_res, expected_byte,
                "HW mismatch with high noise: bit={bit}, val={test_val:#018x}"
            );
            assert_eq!(
                debruijn_res, expected_byte,
                "De Bruijn mismatch with high noise: bit={bit}, val={test_val:#018x}"
            );
            assert_eq!(hw_res, debruijn_res);
        }
    }
}

#[test]
fn test_count_common_bytes_32_determinism() {
    assert_eq!(count_common_bytes_32_debruijn(0), 4);

    for bit in 0..32 {
        let expected_byte = bit / 8;
        let val = 1u32 << bit;
        let res = count_common_bytes_32_debruijn(val);
        assert_eq!(
            res, expected_byte,
            "32-bit De Bruijn failed at bit {bit}"
        );
    }
}

#[test]
fn test_count_match_length_fixed_sizes() {
    let sizes = [0, 1, 7, 8, 15, 64, 256, 1024];

    for &size in &sizes {
        let buf_a = vec![0x42u8; size];
        let mut buf_b = vec![0x42u8; size];

        // 1. Identical buffers
        assert_eq!(
            count_match_length(&buf_a, &buf_b, size),
            size,
            "Failed identical match for size {size}"
        );
        assert_eq!(
            count_match_length(&buf_a, &buf_b, size + 100),
            size,
            "Failed identical match with oversized max_len for size {size}"
        );

        // 2. Mismatch at every byte position
        if size > 0 {
            for mismatch_pos in 0..size {
                buf_b[mismatch_pos] = 0x99;

                let expected = mismatch_pos;
                let actual = count_match_length(&buf_a, &buf_b, size);
                assert_eq!(
                    actual, expected,
                    "Mismatch detection failed at pos {mismatch_pos} in size {size}"
                );

                unsafe {
                    let actual_raw = count_match_length_raw(
                        buf_a.as_ptr(),
                        buf_b.as_ptr(),
                        size,
                    );
                    assert_eq!(
                        actual_raw, expected,
                        "Raw mismatch detection failed at pos {mismatch_pos} in size {size}"
                    );
                }

                // Restore buffer
                buf_b[mismatch_pos] = 0x42;
            }
        }
    }
}

#[test]
fn test_count_match_length_unaligned_slices() {
    // Generate deterministic test pattern
    let total_len = 2048;
    let mut source = vec![0u8; total_len];
    for (i, b) in source.iter_mut().enumerate() {
        *b = ((i * 37 + 13) & 0xFF) as u8;
    }

    let alignments = [0, 1, 2, 3, 4, 5, 6, 7];
    let test_lengths = [0, 1, 3, 7, 8, 9, 15, 16, 31, 32, 64, 127, 256, 512, 1024];

    for &align_a in &alignments {
        for &align_b in &alignments {
            for &len in &test_lengths {
                if align_a + len > total_len || align_b + len > total_len {
                    continue;
                }

                let slice_a = &source[align_a..align_a + len];
                let slice_b = &source[align_b..align_b + len];

                for &max_len in &[0, 1, len / 2, len, len + 10] {
                    let expected = naive_match_count(slice_a, slice_b, max_len);
                    let actual = count_match_length(slice_a, slice_b, max_len);

                    assert_eq!(
                        actual, expected,
                        "Unaligned mismatch at align_a={align_a}, align_b={align_b}, len={len}, max_len={max_len}"
                    );
                }
            }
        }
    }
}

#[test]
fn test_count_match_length_boundary_conditions() {
    let empty: [u8; 0] = [];
    let non_empty = [1, 2, 3, 4];

    assert_eq!(count_match_length(&empty, &empty, 0), 0);
    assert_eq!(count_match_length(&empty, &empty, 10), 0);
    assert_eq!(count_match_length(&empty, &non_empty, 10), 0);
    assert_eq!(count_match_length(&non_empty, &empty, 10), 0);
    assert_eq!(count_match_length(&non_empty, &non_empty, 0), 0);
    assert_eq!(count_match_length(&non_empty, &non_empty, 2), 2);
    assert_eq!(count_match_length(&non_empty, &non_empty, 4), 4);
    assert_eq!(count_match_length(&non_empty, &non_empty, 100), 4);
}
