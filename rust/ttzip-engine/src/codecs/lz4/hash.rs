// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Knuth golden ratio and prime hash primitives for high-speed LZ4 sequence matching.
//!
//! Provides deterministic multiplicative hashing for 4-byte, 5-byte, and 8-byte
//! sequences, mapping raw byte windows into hash table buckets with high entropy
//! and minimal collision overhead.

/// Knuth 32-bit golden ratio multiplicative constant: `2^32 * (sqrt(5) - 1) / 2`.
///
/// In hexadecimal: `0x9E3779B1`.
pub const KNUTH_GOLDEN_RATIO_32: u32 = 2654435761;

/// Standard LZ4 64-bit prime multiplier for 5-byte sequence hashing.
///
/// In hexadecimal: `0x0000_00CF_1BBC_DCBB`.
pub const PRIME_5BYTES_64: u64 = 889523592379;

/// Knuth 64-bit golden ratio prime multiplier for 8-byte sequence hashing: `2^64 * (sqrt(5) - 1) / 2`.
///
/// In hexadecimal: `0x9E37_79B1_85EB_CA87`.
pub const PRIME_8BYTES_64: u64 = 11400714785074694791;

/// Computes the 32-bit Knuth multiplicative hash of a 4-byte sequence down to `hash_log` bits.
///
/// # Arguments
///
/// * `sequence` - 4-byte sequence encoded in a 32-bit unsigned integer (native/little-endian).
/// * `hash_log` - Hash table address bit width, typically in range `10..=16` (table size `2^hash_log`).
///
/// # Returns
///
/// A hash index within `0..(1 << hash_log)`.
#[inline(always)]
pub fn lz4_hash4(sequence: u32, hash_log: u32) -> u32 {
    let shift = 32u32.saturating_sub(hash_log.min(32));
    sequence.wrapping_mul(KNUTH_GOLDEN_RATIO_32) >> shift
}

/// Computes the 64-bit prime hash of a 5-byte sequence down to `hash_log` bits.
///
/// In standard LZ4, the lower 5 bytes of `sequence` represent the candidate sequence.
/// Left-shifting by 24 aligns the 5 bytes (40 bits) to the top of the 64-bit register,
/// multiplying by [`PRIME_5BYTES_64`], and right-shifting down to `hash_log` bits.
///
/// # Arguments
///
/// * `sequence` - 5-byte sequence contained in the lower 40 bits of a 64-bit unsigned integer.
/// * `hash_log` - Hash table address bit width, typically in range `10..=16`.
///
/// # Returns
///
/// A hash index within `0..(1 << hash_log)`.
#[inline(always)]
pub fn lz4_hash5(sequence: u64, hash_log: u32) -> u32 {
    let shift = 64u32.saturating_sub(hash_log.min(64));
    ((sequence << 24).wrapping_mul(PRIME_5BYTES_64) >> shift) as u32
}

/// Computes the 64-bit golden ratio prime hash of an 8-byte sequence down to `hash_log` bits.
///
/// # Arguments
///
/// * `sequence` - 8-byte word.
/// * `hash_log` - Hash table address bit width.
///
/// # Returns
///
/// A hash index within `0..(1 << hash_log)`.
#[inline(always)]
pub fn lz4_hash8(sequence: u64, hash_log: u32) -> u32 {
    let shift = 64u32.saturating_sub(hash_log.min(64));
    (sequence.wrapping_mul(PRIME_8BYTES_64) >> shift) as u32
}

/// Convenience helper to compute 4-byte hash from a 4-byte slice or array.
#[inline(always)]
pub fn lz4_hash4_bytes(bytes: &[u8; 4], hash_log: u32) -> u32 {
    lz4_hash4(u32::from_le_bytes(*bytes), hash_log)
}

/// Convenience helper to compute 5-byte hash from a slice of at least 5 bytes.
#[inline(always)]
pub fn lz4_hash5_slice(slice: &[u8], hash_log: u32) -> u32 {
    debug_assert!(slice.len() >= 5, "lz4_hash5_slice requires at least 5 bytes");
    if slice.len() < 5 {
        return 0;
    }
    let mut buf = [0u8; 8];
    buf[..5].copy_from_slice(&slice[..5]);
    let seq = u64::from_le_bytes(buf);
    lz4_hash5(seq, hash_log)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lz4_hash_constants_exact() {
        assert_eq!(KNUTH_GOLDEN_RATIO_32, 2654435761);
        assert_eq!(PRIME_5BYTES_64, 889523592379);
        assert_eq!(PRIME_8BYTES_64, 11400714785074694791);
    }

    #[test]
    fn test_lz4_hash4_bounded() {
        for log in 10..=16 {
            let limit = 1u32 << log;
            for seq in [0u32, 1, 0x12345678, 0xFFFFFFFF, 0x9E3779B1] {
                let h = lz4_hash4(seq, log);
                assert!(h < limit, "hash {h} exceeds limit {limit} for log {log}");
            }
        }
    }

    #[test]
    fn test_lz4_hash5_bounded() {
        for log in 10..=16 {
            let limit = 1u32 << log;
            for seq in [0u64, 1, 0x000000FFFFFFFFFF, 0x123456789A, 0xCAFEBABE01] {
                let h = lz4_hash5(seq, log);
                assert!(h < limit, "hash {h} exceeds limit {limit} for log {log}");
            }
        }
    }

    #[test]
    fn test_lz4_hash4_and_hash5_helpers() {
        let bytes4 = [0x12, 0x34, 0x56, 0x78];
        let h4 = lz4_hash4_bytes(&bytes4, 12);
        assert_eq!(h4, lz4_hash4(u32::from_le_bytes(bytes4), 12));

        let bytes5 = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let h5 = lz4_hash5_slice(&bytes5, 14);
        assert!(h5 < (1 << 14));
    }
}
