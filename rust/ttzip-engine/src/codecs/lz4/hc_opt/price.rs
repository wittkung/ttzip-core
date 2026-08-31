// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Bit-Cost Price Functions for LZ4 sequence encoding and rate-distortion modeling.
//!
//! Provides exact byte/bit cost evaluations for:
//! - Literals lengths (token header + variable byte extensions + payload)
//! - Match sequences (token + literals payload + offset + match length extensions)
//! - Decompression-speed biased penalties for small offsets

/// Calculates exact bit-cost price in bytes for encoding a raw literals block.
#[inline(always)]
pub const fn price_literals(length: usize) -> usize {
    let extra_len_bytes = if length >= 15 {
        1 + (length - 15) / 255
    } else {
        0
    };
    1 + extra_len_bytes + length
}

/// Calculates exact bit-cost price in bytes for encoding an LZ4 sequence (literals + match).
#[inline(always)]
pub const fn price_sequence(lit_len: usize, match_len: usize) -> usize {
    let extra_lit_bytes = if lit_len >= 15 {
        1 + (lit_len - 15) / 255
    } else {
        0
    };
    let extra_match_bytes = if match_len >= 19 {
        1 + (match_len - 19) / 255
    } else {
        0
    };
    1 + extra_lit_bytes + lit_len + 2 + extra_match_bytes
}

/// Calculates bit-cost price with optional decompression speed bias penalty.
#[inline(always)]
pub const fn price_sequence_speed(
    lit_len: usize,
    match_len: usize,
    offset: u16,
    favor_dec_speed: bool,
) -> usize {
    let base = price_sequence(lit_len, match_len);
    if favor_dec_speed && (offset < 8) && (match_len < 18) {
        base + 3
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_literals_bounds() {
        assert_eq!(price_literals(0), 1);
        assert_eq!(price_literals(1), 2);
        assert_eq!(price_literals(14), 15);
        assert_eq!(price_literals(15), 17);
        assert_eq!(price_literals(269), 271);
        assert_eq!(price_literals(270), 273);
    }

    #[test]
    fn test_price_sequence_bounds() {
        assert_eq!(price_sequence(0, 4), 3);
        assert_eq!(price_sequence(5, 4), 8);
        assert_eq!(price_sequence(14, 4), 17);
        assert_eq!(price_sequence(15, 4), 19);
        assert_eq!(price_sequence(0, 18), 3);
        assert_eq!(price_sequence(0, 19), 4);
    }

    #[test]
    fn test_price_sequence_speed_penalty() {
        let p_normal = price_sequence_speed(0, 4, 4, false);
        assert_eq!(p_normal, 3);

        let p_slow = price_sequence_speed(0, 4, 4, true);
        assert_eq!(p_slow, 6);

        let p_fast_aligned = price_sequence_speed(0, 18, 4, true);
        assert_eq!(p_fast_aligned, price_sequence(0, 18));
    }
}
