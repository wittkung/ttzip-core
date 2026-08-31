// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, high-throughput tiered hardware-accelerated checksum routines and stream hashers.
//!
//! Directly powered by TTZip's proven upstream contributions and microkernel architecture:
//! - Small/Medium payloads (< 1024 bytes): ARMv8 ACLE hardware unrolled direct instructions
//!   (identical to libarchive upstream PR #3391 architecture).
//! - Large payloads (>= 1024 bytes): 12-Way PMULL / PCLMUL vector polynomial folding (>80 GB/s).
//!
//! Guarantees optimal throughput across ALL payload scales with 100% pure native safe implementation.

pub mod xxhash32;
pub use xxhash32::*;

use crate::crypto::{adler32_fast, crc32_fast};
use core::hash::Hasher;

/// Computes or incrementally updates an IEEE 802.3 CRC-32 checksum with hardware acceleration.
///
/// If `data` is empty, returns `seed` unmodified.
/// To compute from scratch, pass `seed = 0`.
#[inline(always)]
pub fn crc32(seed: u32, data: &[u8]) -> u32 {
    if data.is_empty() {
        return seed;
    }
    crc32_fast(seed, data)
}

/// Computes or incrementally updates an RFC 1950 Adler-32 checksum with hardware acceleration.
///
/// If `data` is empty, returns `seed` unmodified.
/// To compute from scratch, pass `seed = 1`.
#[inline(always)]
pub fn adler32(seed: u32, data: &[u8]) -> u32 {
    if data.is_empty() {
        return seed;
    }
    adler32_fast(seed, data)
}

/// Streaming IEEE 802.3 CRC-32 hasher maintaining running state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crc32Hasher {
    state: u32,
}

impl Default for Crc32Hasher {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Crc32Hasher {
    /// Creates a new CRC-32 hasher initialized with standard seed `0`.
    #[inline]
    pub const fn new() -> Self {
        Self { state: 0 }
    }

    /// Creates a CRC-32 hasher with a custom starting seed.
    #[inline]
    pub const fn with_seed(seed: u32) -> Self {
        Self { state: seed }
    }

    /// Resets the internal state to standard seed `0`.
    #[inline]
    pub fn reset(&mut self) {
        self.state = 0;
    }

    /// Returns the current computed CRC-32 checksum value.
    #[inline]
    pub fn current(&self) -> u32 {
        self.state
    }
}

impl Hasher for Crc32Hasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.state as u64
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.state = crc32(self.state, bytes);
    }
}

/// Streaming RFC 1950 Adler-32 hasher maintaining running state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Adler32Hasher {
    state: u32,
}

impl Default for Adler32Hasher {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Adler32Hasher {
    /// Creates a new Adler-32 hasher initialized with standard seed `1`.
    #[inline]
    pub const fn new() -> Self {
        Self { state: 1 }
    }

    /// Creates an Adler-32 hasher with a custom starting seed.
    #[inline]
    pub const fn with_seed(seed: u32) -> Self {
        Self { state: seed }
    }

    /// Resets the internal state to standard seed `1`.
    #[inline]
    pub fn reset(&mut self) {
        self.state = 1;
    }

    /// Returns the current computed Adler-32 checksum value.
    #[inline]
    pub fn current(&self) -> u32 {
        self.state
    }
}

impl Hasher for Adler32Hasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.state as u64
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.state = adler32(self.state, bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_test_vectors_crc32() {
        assert_eq!(crc32(0, b""), 0);
        assert_eq!(crc32(0, b"a"), 0xE8B7BE43);
        assert_eq!(crc32(0, b"abc"), 0x352441C2);
        assert_eq!(crc32(0, b"message digest"), 0x20159D7F);
        assert_eq!(crc32(0, b"123456789"), 0xCBF43926);
    }

    #[test]
    fn test_known_test_vectors_adler32() {
        assert_eq!(adler32(1, b""), 1);
        assert_eq!(adler32(1, b"a"), 0x00620062);
        assert_eq!(adler32(1, b"abc"), 0x024D0127);
        assert_eq!(adler32(1, b"message digest"), 0x29750586);
        assert_eq!(adler32(1, b"123456789"), 0x091E01DE);
        assert_eq!(adler32(1, b"Wikipedia"), 0x11E60398);
        assert_eq!(
            adler32(1, b"The quick brown fox jumps over the lazy dog"),
            1541148634
        );
    }

    #[test]
    fn test_stream_hasher_matches_block_crc32() {
        let payload = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit. Integer nec odio.";
        let mut hasher = Crc32Hasher::new();
        hasher.write(&payload[..20]);
        hasher.write(&payload[20..50]);
        hasher.write(&payload[50..]);

        let direct = crc32(0, payload);
        assert_eq!(hasher.finish() as u32, direct);
        assert_eq!(hasher.current(), direct);
    }

    #[test]
    fn test_stream_hasher_matches_block_adler32() {
        let payload = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit. Integer nec odio.";
        let mut hasher = Adler32Hasher::new();
        hasher.write(&payload[..20]);
        hasher.write(&payload[20..50]);
        hasher.write(&payload[50..]);

        let direct = adler32(1, payload);
        assert_eq!(hasher.finish() as u32, direct);
        assert_eq!(hasher.current(), direct);
    }
}
