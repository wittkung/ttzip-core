// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! De Bruijn 64-bit multiplication constants and common prefix match length counting primitives.
//!
//! Provides ultra-fast matching word length discovery for the LZMA2 match-finder pipeline:
//! - Hardware-accelerated count-trailing-zeros (`trailing_zeros >> 3`) on AArch64/x86_64.
//! - Deterministic De Bruijn multiplication table lookup fallback for generic targets.
//! - 64-bit word chunk scanning with sub-word and scalar tail reduction.

/// 64-bit De Bruijn sequence multiplier for counting trailing zero bytes.
pub const DE_BRUIJN_64: u64 = 0x0218_A392_CDAB_BD3F;

/// 32-bit De Bruijn sequence multiplier for 32-bit architectures.
pub const DE_BRUIJN_32: u32 = 0x077C_B531;

/// 64-byte lookup table mapping `((isolated_lsb * DE_BRUIJN_64) >> 58)` to byte position (0..7).
pub const DE_BRUIJN_BYTE_POS_64: [u8; 64] = [
    0, 0, 0, 0, 0, 1, 1, 2,
    0, 3, 1, 3, 1, 4, 2, 7,
    0, 2, 3, 6, 1, 5, 3, 5,
    1, 3, 4, 4, 2, 5, 6, 7,
    7, 0, 1, 2, 3, 3, 4, 6,
    2, 6, 5, 5, 3, 4, 5, 6,
    7, 1, 2, 4, 6, 4, 4, 5,
    7, 2, 6, 5, 7, 6, 7, 7,
];

/// 32-byte lookup table mapping `((isolated_lsb * DE_BRUIJN_32) >> 27)` to byte position (0..3).
pub const DE_BRUIJN_BYTE_POS_32: [u8; 32] = [
    0, 0, 3, 0, 3, 1, 3, 0,
    3, 2, 2, 1, 3, 2, 0, 1,
    3, 3, 1, 2, 2, 2, 2, 0,
    3, 1, 2, 0, 1, 0, 1, 1,
];

/// Counts the number of matching common leading bytes (0..8) between two 64-bit words
/// given their bitwise XOR difference `val = word1 ^ word2`.
///
/// If `val == 0`, all 8 bytes match, returning `8`.
///
/// On ARM64 (AArch64) and x86/x86_64, uses hardware `trailing_zeros` (RBIT+CLZ or BSF/TZCNT).
/// On other generic architectures, falls back to the De Bruijn multiplication table lookup.
#[inline(always)]
pub fn count_common_bytes_64(val: u64) -> usize {
    if val == 0 {
        return 8;
    }
    #[cfg(any(
        target_arch = "aarch64",
        target_arch = "x86_64",
        target_arch = "x86",
        target_arch = "arm"
    ))]
    {
        (val.to_le().trailing_zeros() >> 3) as usize
    }
    #[cfg(not(any(
        target_arch = "aarch64",
        target_arch = "x86_64",
        target_arch = "x86",
        target_arch = "arm"
    )))]
    {
        count_common_bytes_64_debruijn(val)
    }
}

/// Counts the number of matching common leading bytes (0..8) using pure 64-bit De Bruijn multiplication.
///
/// This implementation provides deterministic bit-exact parity with hardware `trailing_zeros`
/// across all architectures without relying on target CPU intrinsics.
#[inline(always)]
pub fn count_common_bytes_64_debruijn(val: u64) -> usize {
    if val == 0 {
        return 8;
    }
    let val_le = val.to_le();
    let isolated_lsb = val_le & val_le.wrapping_neg();
    let index = (isolated_lsb.wrapping_mul(DE_BRUIJN_64) >> 58) as usize;
    DE_BRUIJN_BYTE_POS_64[index] as usize
}

/// Counts the number of matching common leading bytes (0..4) using 32-bit De Bruijn multiplication.
#[inline(always)]
pub fn count_common_bytes_32_debruijn(val: u32) -> usize {
    if val == 0 {
        return 4;
    }
    let val_le = val.to_le();
    let isolated_lsb = val_le & val_le.wrapping_neg();
    let index = (isolated_lsb.wrapping_mul(DE_BRUIJN_32) >> 27) as usize;
    DE_BRUIJN_BYTE_POS_32[index] as usize
}

/// Counts the common match length between `src` and `matched` up to `max_len` bytes.
///
/// Scans in 8-byte (64-bit) unaligned word chunks using `count_common_bytes_64` for maximum
/// throughput, then finishes with a scalar tail loop for the remaining 0..7 bytes.
///
/// # Arguments
/// * `src` - Current buffer position slice.
/// * `matched` - Reference or dictionary match slice.
/// * `max_len` - Maximum comparison length limit.
///
/// # Returns
/// Number of contiguous identical bytes between `src` and `matched`, capped at
/// `min(max_len, src.len(), matched.len())`.
#[inline(always)]
pub fn count_match_length(src: &[u8], matched: &[u8], max_len: usize) -> usize {
    let limit = max_len.min(src.len()).min(matched.len());
    if limit == 0 {
        return 0;
    }
    unsafe { count_match_length_raw(src.as_ptr(), matched.as_ptr(), limit) }
}

/// Raw pointer match length counter for high-frequency inner loops.
///
/// # Safety
/// Caller must ensure both `src` and `matched` are valid for reads of up to `max_len` bytes.
#[inline(always)]
pub unsafe fn count_match_length_raw(
    src: *const u8,
    matched: *const u8,
    max_len: usize,
) -> usize {
    let mut len = 0usize;

    // Fast-check first byte to immediately exit on mismatch (0 ns overhead)
    if max_len > 0 && *src != *matched {
        return 0;
    }

    // 8-byte 64-bit word chunk scanning
    if max_len >= 8 {
        let loop_limit = max_len - 8;
        while len <= loop_limit {
            let s = (src.add(len) as *const u64).read_unaligned();
            let m = (matched.add(len) as *const u64).read_unaligned();
            let diff = s ^ m;
            if diff != 0 {
                return len + count_common_bytes_64(diff);
            }
            len += 8;
        }
    }

    // 4-byte sub-word chunk acceleration for remaining tail
    if max_len - len >= 4 {
        let s = (src.add(len) as *const u32).read_unaligned();
        let m = (matched.add(len) as *const u32).read_unaligned();
        let diff = s ^ m;
        if diff != 0 {
            #[cfg(any(
                target_arch = "aarch64",
                target_arch = "x86_64",
                target_arch = "x86",
                target_arch = "arm"
            ))]
            {
                return len + ((diff.to_le().trailing_zeros() >> 3) as usize);
            }
            #[cfg(not(any(
                target_arch = "aarch64",
                target_arch = "x86_64",
                target_arch = "x86",
                target_arch = "arm"
            )))]
            {
                return len + count_common_bytes_32_debruijn(diff);
            }
        }
        len += 4;
    }

    // Remaining scalar tail 0..3 bytes
    while len < max_len && *src.add(len) == *matched.add(len) {
        len += 1;
    }

    len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debruijn_parity_all_bit_positions() {
        for bit in 0..64 {
            let val = 1u64 << bit;
            let expected_byte = bit / 8;
            let debruijn_result = count_common_bytes_64_debruijn(val);
            let hw_result = (val.trailing_zeros() >> 3) as usize;
            assert_eq!(
                debruijn_result, expected_byte,
                "De Bruijn failed at bit position {bit}"
            );
            assert_eq!(
                debruijn_result, hw_result,
                "Parity mismatch at bit position {bit}"
            );
        }
    }

    #[test]
    fn test_debruijn_zero_input() {
        assert_eq!(count_common_bytes_64(0), 8);
        assert_eq!(count_common_bytes_64_debruijn(0), 8);
        assert_eq!(count_common_bytes_32_debruijn(0), 4);
    }
}
