// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Lookup tables and small offset broadcast unrolling for ultra-fast LZ4 decompression.
//!
//! When decoding matches with `offset < 8`, standard block copies cannot be performed
//! naively due to byte overlaps between source and destination. This module provides
//! standard step tables (`INC32_TABLE`, `DEC64_TABLE`), vectorized/unrolled small offset
//! pattern broadcasters, and in-place decompression margin calculators.

/// Step table for 32-bit pointer advance adjustments on small match offsets (0..=7).
pub const INC32_TABLE: [u32; 8] = [0, 1, 2, 1, 0, 4, 4, 4];

/// Step table for 64-bit match pointer decrements on small match offsets (0..=7).
pub const DEC64_TABLE: [i32; 8] = [0, 0, 0, -1, -4, 1, 2, 3];

/// Copies overlapping match patterns with small offset (`offset < 8`) using safe byte broadcasting.
///
/// In LZ4 match replication:
/// - Offset 1: Broadcasts single byte across the entire destination slice (run-length fill).
/// - Offset 2: Broadcasts 16-bit word `[A, B]` in 64-bit chunks `[A, B, A, B, A, B, A, B]`.
/// - Offset 4: Broadcasts 32-bit dword `[A, B, C, D]` in 64-bit chunks.
/// - Offset 3, 5, 6, 7: Broadcasts modulo-indexed patterns across the destination.
///
/// # Arguments
///
/// * `dst` - Destination slice to write decompressed match sequence into.
/// * `src` - Source buffer containing the initial match pattern.
/// * `offset` - Match distance / period (typically 1..7).
/// * `length` - Number of bytes to copy.
pub fn copy_small_offset(dst: &mut [u8], src: &[u8], offset: usize, length: usize) {
    if length == 0 || dst.is_empty() || src.is_empty() || offset == 0 {
        return;
    }

    let count = length.min(dst.len());
    let pattern_len = offset.min(src.len());

    match offset {
        1 => {
            let byte = src[0];
            dst[..count].fill(byte);
        }
        2 if pattern_len >= 2 => {
            let b0 = src[0];
            let b1 = src[1];
            let pat8 = [b0, b1, b0, b1, b0, b1, b0, b1];
            let mut chunks = dst[..count].chunks_exact_mut(8);
            for chunk in &mut chunks {
                chunk.copy_from_slice(&pat8);
            }
            let rem = chunks.into_remainder();
            if !rem.is_empty() {
                rem.copy_from_slice(&pat8[..rem.len()]);
            }
        }
        4 if pattern_len >= 4 => {
            let b0 = src[0];
            let b1 = src[1];
            let b2 = src[2];
            let b3 = src[3];
            let pat8 = [b0, b1, b2, b3, b0, b1, b2, b3];
            let mut chunks = dst[..count].chunks_exact_mut(8);
            for chunk in &mut chunks {
                chunk.copy_from_slice(&pat8);
            }
            let rem = chunks.into_remainder();
            if !rem.is_empty() {
                rem.copy_from_slice(&pat8[..rem.len()]);
            }
        }
        _ => {
            let pat = &src[..pattern_len];
            for i in 0..count {
                dst[i] = pat[i % pattern_len];
            }
        }
    }
}

/// Computes the safety margin in bytes required for in-place decompression of an LZ4 block.
///
/// In-place decompression requires placing the compressed stream at the end of the buffer.
/// To guarantee that the decompressor never overwrites unread compressed data during
/// forward linear decoding, the buffer must include a safety margin of `(compressed_size >> 8) + 32`.
///
/// # Arguments
///
/// * `compressed_size` - Size of compressed payload in bytes.
///
/// # Returns
///
/// Additional margin size in bytes.
#[inline(always)]
pub const fn lz4_decompress_inplace_margin(compressed_size: usize) -> usize {
    (compressed_size >> 8) + 32
}

/// Computes the total buffer size required to decompress an LZ4 block in-place.
///
/// Total capacity is `decompressed_size + lz4_decompress_inplace_margin(decompressed_size)`.
///
/// # Arguments
///
/// * `decompressed_size` - Expected uncompressed size in bytes.
///
/// # Returns
///
/// Total allocated buffer size in bytes.
#[inline(always)]
pub const fn lz4_decompress_inplace_buffer_size(decompressed_size: usize) -> usize {
    decompressed_size.saturating_add(lz4_decompress_inplace_margin(decompressed_size))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inc32_and_dec64_tables() {
        assert_eq!(INC32_TABLE, [0, 1, 2, 1, 0, 4, 4, 4]);
        assert_eq!(DEC64_TABLE, [0, 0, 0, -1, -4, 1, 2, 3]);
    }

    #[test]
    fn test_copy_small_offset_1_byte_fill() {
        let src = [0x5A];
        let mut dst = [0u8; 17];
        copy_small_offset(&mut dst, &src, 1, 17);
        assert_eq!(dst, [0x5A; 17]);
    }

    #[test]
    fn test_copy_small_offset_2_bytes() {
        let src = [0xAB, 0xCD];
        let mut dst = [0u8; 9];
        copy_small_offset(&mut dst, &src, 2, 9);
        assert_eq!(dst, [0xAB, 0xCD, 0xAB, 0xCD, 0xAB, 0xCD, 0xAB, 0xCD, 0xAB]);
    }

    #[test]
    fn test_copy_small_offset_4_bytes() {
        let src = [1, 2, 3, 4];
        let mut dst = [0u8; 10];
        copy_small_offset(&mut dst, &src, 4, 10);
        assert_eq!(dst, [1, 2, 3, 4, 1, 2, 3, 4, 1, 2]);
    }

    #[test]
    fn test_copy_small_offset_3_bytes() {
        let src = [10, 20, 30];
        let mut dst = [0u8; 8];
        copy_small_offset(&mut dst, &src, 3, 8);
        assert_eq!(dst, [10, 20, 30, 10, 20, 30, 10, 20]);
    }

    #[test]
    fn test_inplace_margin_bounds() {
        assert_eq!(lz4_decompress_inplace_margin(0), 32);
        assert_eq!(lz4_decompress_inplace_margin(1024), 36);
        assert_eq!(lz4_decompress_inplace_margin(65536), 288);
        assert_eq!(lz4_decompress_inplace_margin(1048576), 4128);

        assert_eq!(lz4_decompress_inplace_buffer_size(0), 32);
        assert_eq!(lz4_decompress_inplace_buffer_size(1024), 1060);
        assert_eq!(lz4_decompress_inplace_buffer_size(65536), 65824);
        assert_eq!(lz4_decompress_inplace_buffer_size(1048576), 1052704);
    }
}
