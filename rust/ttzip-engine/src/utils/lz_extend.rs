// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! SWAR (SIMD Within A Register) 64-bit wide-word XOR + CTZ longest match extension primitive.
//!
//! Provides ultra-fast byte match extension for LZ-family compression algorithms (LZ4,
//! Deflate, Snappy, Zstandard, Fast-LZMA2). Reads 8 bytes at a time into 64-bit integer registers,
//! performs bitwise XOR, and extracts the exact match boundary in a single instruction cycle
//! via count-trailing-zeros (`trailing_zeros() / 8`).

/// Extends a byte match between `src` and `match_slice` starting from `start_len`.
///
/// Compares 8 bytes per iteration using 64-bit little-endian XOR. When a mismatch
/// is encountered, computes the exact mismatch offset using `(diff.trailing_zeros() / 8)`.
/// Handles remaining trailing bytes (0..7) individually.
///
/// # Arguments
/// * `src` - Input buffer slice at the current coding position.
/// * `match_slice` - Dictionary or reference slice to match against.
/// * `start_len` - Number of initial bytes already known to match.
///
/// # Returns
/// Total length of contiguous matching bytes between `src` and `match_slice`,
/// bounded by `min(src.len(), match_slice.len())`.
#[inline]
pub fn lz_extend(src: &[u8], match_slice: &[u8], start_len: usize) -> usize {
    let max_len = src.len().min(match_slice.len());
    if start_len >= max_len {
        return max_len;
    }

    if src[start_len] != match_slice[start_len] {
        return start_len;
    }

    let mut len = start_len;

    // Fast SWAR 8-byte comparison loop with single margin guard
    if max_len >= 8 {
        let limit = max_len - 8;
        let s_ptr = src.as_ptr();
        let m_ptr = match_slice.as_ptr();
        while len <= limit {
            let s = unsafe { (s_ptr.add(len) as *const u64).read_unaligned() }.to_le();
            let m = unsafe { (m_ptr.add(len) as *const u64).read_unaligned() }.to_le();
            let diff = s ^ m;
            if diff != 0 {
                let matched_bytes = (diff.trailing_zeros() / 8) as usize;
                return len + matched_bytes;
            }
            len += 8;
        }
    }

    // Scalar tail loop for remaining 0..7 bytes
    while len < max_len && src[len] == match_slice[len] {
        len += 1;
    }

    len
}

/// Raw-pointer fast-path variant of `lz_extend` for high-throughput inner loops.
///
/// # Safety
/// The caller must ensure that `src_ptr` and `match_ptr` are valid for reads
/// up to `max_len` bytes.
#[inline]
pub unsafe fn lz_extend_raw(
    src_ptr: *const u8,
    match_ptr: *const u8,
    start_len: usize,
    max_len: usize,
) -> usize {
    if start_len >= max_len {
        return max_len;
    }

    let mut len = start_len;

    if max_len >= 8 {
        let limit = max_len - 8;
        while len <= limit {
            let s = (src_ptr.add(len) as *const u64).read_unaligned().to_le();
            let m = (match_ptr.add(len) as *const u64).read_unaligned().to_le();
            let diff = s ^ m;
            if diff != 0 {
                let matched_bytes = (diff.trailing_zeros() / 8) as usize;
                return len + matched_bytes;
            }
            len += 8;
        }
    }

    while len < max_len && *src_ptr.add(len) == *match_ptr.add(len) {
        len += 1;
    }

    len
}

#[cfg(test)]
mod tests {
    use super::*;

    fn naive_extend(src: &[u8], match_slice: &[u8], start_len: usize) -> usize {
        let max_len = src.len().min(match_slice.len());
        let mut len = start_len.min(max_len);
        while len < max_len && src[len] == match_slice[len] {
            len += 1;
        }
        len
    }

    #[test]
    fn test_empty_and_out_of_bounds() {
        assert_eq!(lz_extend(&[], &[], 0), 0);
        assert_eq!(lz_extend(b"abc", b"abc", 3), 3);
        assert_eq!(lz_extend(b"abc", b"abc", 5), 3);
        assert_eq!(lz_extend(b"abc", &[], 0), 0);
        assert_eq!(lz_extend(&[], b"abc", 0), 0);
    }

    #[test]
    fn test_identical_buffers_various_lengths() {
        for len in [0, 1, 2, 7, 8, 9, 15, 16, 17, 31, 32, 33, 63, 64, 65, 128, 256, 1000] {
            let buf1 = vec![0x5Au8; len];
            let buf2 = vec![0x5Au8; len];

            for start_len in [0, 1, 4, 8, len / 2, len] {
                if start_len <= len {
                    let res = lz_extend(&buf1, &buf2, start_len);
                    assert_eq!(res, len, "Failed on identical buffer of len {len}, start_len {start_len}");
                }
            }
        }
    }

    #[test]
    fn test_completely_different() {
        let buf1 = b"abcdefghijklmnop";
        let buf2 = b"1234567890123456";
        assert_eq!(lz_extend(buf1, buf2, 0), 0);
    }

    #[test]
    fn test_mismatch_at_all_byte_offsets_within_word() {
        // Test mismatches at byte offsets 0..7 within 8-byte chunk
        for mismatch_pos in 0..64 {
            let buf1 = vec![0xAAu8; 80];
            let mut buf2 = vec![0xAAu8; 80];
            buf2[mismatch_pos] = 0xBB;

            let expected = mismatch_pos;
            let actual = lz_extend(&buf1, &buf2, 0);
            assert_eq!(
                actual, expected,
                "Mismatch detection failed for mismatch_pos {mismatch_pos}"
            );

            unsafe {
                let actual_raw = lz_extend_raw(buf1.as_ptr(), buf2.as_ptr(), 0, buf1.len());
                assert_eq!(actual_raw, expected, "Raw mismatch detection failed at {mismatch_pos}");
            }
        }
    }

    #[test]
    fn test_overlapping_match_rle() {
        // Overlapping match: "AAAA..." with offset 1
        let data = vec![b'A'; 200];
        let src = &data[1..];
        let match_slice = &data[..199];

        assert_eq!(lz_extend(src, match_slice, 0), 199);
        assert_eq!(lz_extend(src, match_slice, 4), 199);
    }

    #[test]
    fn test_tail_remainders() {
        // Remainder 0..7 bytes after multiple 8-byte blocks
        for tail in 0..8 {
            let total_len = 24 + tail;
            let buf1 = vec![b'X'; total_len];
            let mut buf2 = vec![b'X'; total_len];

            assert_eq!(lz_extend(&buf1, &buf2, 0), total_len);

            if tail > 0 {
                // Break match in the tail
                buf2[24 + tail - 1] = b'Y';
                assert_eq!(lz_extend(&buf1, &buf2, 0), 24 + tail - 1);
            }
        }
    }

    #[test]
    fn test_differential_randomized_parity() {
        let mut buffer = vec![0u8; 512];
        for (i, b) in buffer.iter_mut().enumerate() {
            *b = ((i * 43 + 7) & 0xFF) as u8;
        }

        for offset_a in 0..64 {
            for offset_b in 0..64 {
                let slice_a = &buffer[offset_a..offset_a + 256];
                let slice_b = &buffer[offset_b..offset_b + 256];

                for start in [0, 1, 3, 7, 8, 15, 16, 32] {
                    let expected = naive_extend(slice_a, slice_b, start);
                    let actual = lz_extend(slice_a, slice_b, start);
                    assert_eq!(
                        actual, expected,
                        "Differential mismatch for offset_a={offset_a}, offset_b={offset_b}, start={start}"
                    );
                }
            }
        }
    }
}
