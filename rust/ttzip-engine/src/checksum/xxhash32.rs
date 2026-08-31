// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, high-throughput pure Rust XXH32 (32-bit xxHash) implementation.
//!
//! Provides zero-allocation block hashing and streaming hasher conforming to the
//! official xxHash specification (Yann Collet).

use core::hash::Hasher;

// MARK: - XXH32 Primes

const PRIME32_1: u32 = 0x9E37_79B1;
const PRIME32_2: u32 = 0x85EB_CA77;
const PRIME32_3: u32 = 0xC2B2_AE3D;
const PRIME32_4: u32 = 0x27D4_EB2F;
const PRIME32_5: u32 = 0x1656_67B1;

// MARK: - Core XXH32 Functions

#[inline(always)]
const fn round(acc: u32, input: u32) -> u32 {
    acc.wrapping_add(input.wrapping_mul(PRIME32_2))
        .rotate_left(13)
        .wrapping_mul(PRIME32_1)
}

#[inline(always)]
const fn avalanche(mut h32: u32) -> u32 {
    h32 ^= h32 >> 15;
    h32 = h32.wrapping_mul(PRIME32_2);
    h32 ^= h32 >> 13;
    h32 = h32.wrapping_mul(PRIME32_3);
    h32 ^= h32 >> 16;
    h32
}

/// Computes the XXH32 32-bit hash of `data` using the specified `seed`.
///
/// Fully deterministic, zero heap allocation, and pure safe Rust.
pub fn xxh32(data: &[u8], seed: u32) -> u32 {
    let len = data.len();
    let mut h32: u32;
    let mut remaining = data;

    if len >= 16 {
        let mut v1 = seed.wrapping_add(PRIME32_1).wrapping_add(PRIME32_2);
        let mut v2 = seed.wrapping_add(PRIME32_2);
        let mut v3 = seed;
        let mut v4 = seed.wrapping_sub(PRIME32_1);

        while remaining.len() >= 16 {
            let chunk: &[u8; 16] = match remaining[..16].try_into() {
                Ok(c) => c,
                Err(_) => break,
            };
            let w0 = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            let w1 = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
            let w2 = u32::from_le_bytes([chunk[8], chunk[9], chunk[10], chunk[11]]);
            let w3 = u32::from_le_bytes([chunk[12], chunk[13], chunk[14], chunk[15]]);

            v1 = round(v1, w0);
            v2 = round(v2, w1);
            v3 = round(v3, w2);
            v4 = round(v4, w3);

            remaining = &remaining[16..];
        }

        h32 = v1.rotate_left(1)
            .wrapping_add(v2.rotate_left(7))
            .wrapping_add(v3.rotate_left(12))
            .wrapping_add(v4.rotate_left(18));
    } else {
        h32 = seed.wrapping_add(PRIME32_5);
    }

    h32 = h32.wrapping_add(len as u32);

    while remaining.len() >= 4 {
        let chunk: &[u8; 4] = match remaining[..4].try_into() {
            Ok(c) => c,
            Err(_) => break,
        };
        let val = u32::from_le_bytes(*chunk);
        h32 = h32
            .wrapping_add(val.wrapping_mul(PRIME32_3))
            .rotate_left(17)
            .wrapping_mul(PRIME32_4);
        remaining = &remaining[4..];
    }

    for &byte in remaining {
        h32 = h32
            .wrapping_add((byte as u32).wrapping_mul(PRIME32_5))
            .rotate_left(11)
            .wrapping_mul(PRIME32_1);
    }

    avalanche(h32)
}

// MARK: - Streaming Hasher

/// Streaming XXH32 hasher with internal 16-byte staging buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Xxh32Hasher {
    seed: u32,
    v1: u32,
    v2: u32,
    v3: u32,
    v4: u32,
    total_len: u64,
    buffer: [u8; 16],
    buffer_len: usize,
}

impl Default for Xxh32Hasher {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Xxh32Hasher {
    /// Creates a new XXH32 hasher initialized with standard seed `0`.
    #[inline]
    pub const fn new() -> Self {
        Self::with_seed(0)
    }

    /// Creates an XXH32 hasher with a custom starting seed.
    #[inline]
    pub const fn with_seed(seed: u32) -> Self {
        Self {
            seed,
            v1: seed.wrapping_add(PRIME32_1).wrapping_add(PRIME32_2),
            v2: seed.wrapping_add(PRIME32_2),
            v3: seed,
            v4: seed.wrapping_sub(PRIME32_1),
            total_len: 0,
            buffer: [0u8; 16],
            buffer_len: 0,
        }
    }

    /// Resets the internal state to seed `0`.
    #[inline]
    pub fn reset(&mut self) {
        self.reset_with_seed(0);
    }

    /// Resets the internal state with a specific seed.
    #[inline]
    pub fn reset_with_seed(&mut self, seed: u32) {
        self.seed = seed;
        self.v1 = seed.wrapping_add(PRIME32_1).wrapping_add(PRIME32_2);
        self.v2 = seed.wrapping_add(PRIME32_2);
        self.v3 = seed;
        self.v4 = seed.wrapping_sub(PRIME32_1);
        self.total_len = 0;
        self.buffer_len = 0;
    }

    /// Computes and returns the 32-bit xxHash value from current state.
    pub fn digest(&self) -> u32 {
        let mut h32: u32;

        if self.total_len >= 16 {
            h32 = self.v1.rotate_left(1)
                .wrapping_add(self.v2.rotate_left(7))
                .wrapping_add(self.v3.rotate_left(12))
                .wrapping_add(self.v4.rotate_left(18));
        } else {
            h32 = self.seed.wrapping_add(PRIME32_5);
        }

        h32 = h32.wrapping_add(self.total_len as u32);

        let mut remaining = &self.buffer[..self.buffer_len];

        while remaining.len() >= 4 {
            let chunk: &[u8; 4] = match remaining[..4].try_into() {
                Ok(c) => c,
                Err(_) => break,
            };
            let val = u32::from_le_bytes(*chunk);
            h32 = h32
                .wrapping_add(val.wrapping_mul(PRIME32_3))
                .rotate_left(17)
                .wrapping_mul(PRIME32_4);
            remaining = &remaining[4..];
        }

        for &byte in remaining {
            h32 = h32
                .wrapping_add((byte as u32).wrapping_mul(PRIME32_5))
                .rotate_left(11)
                .wrapping_mul(PRIME32_1);
        }

        avalanche(h32)
    }

    /// Feeds input bytes incrementally into the streaming hasher.
    pub fn update(&mut self, input: &[u8]) {
        <Self as Hasher>::write(self, input);
    }
}

impl Hasher for Xxh32Hasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.digest() as u64
    }

    fn write(&mut self, mut input: &[u8]) {
        self.total_len = self.total_len.saturating_add(input.len() as u64);

        if self.buffer_len > 0 && self.buffer_len + input.len() >= 16 {
            let needed = 16 - self.buffer_len;
            self.buffer[self.buffer_len..16].copy_from_slice(&input[..needed]);

            let w0 = u32::from_le_bytes([self.buffer[0], self.buffer[1], self.buffer[2], self.buffer[3]]);
            let w1 = u32::from_le_bytes([self.buffer[4], self.buffer[5], self.buffer[6], self.buffer[7]]);
            let w2 = u32::from_le_bytes([self.buffer[8], self.buffer[9], self.buffer[10], self.buffer[11]]);
            let w3 = u32::from_le_bytes([self.buffer[12], self.buffer[13], self.buffer[14], self.buffer[15]]);

            self.v1 = round(self.v1, w0);
            self.v2 = round(self.v2, w1);
            self.v3 = round(self.v3, w2);
            self.v4 = round(self.v4, w3);

            self.buffer_len = 0;
            input = &input[needed..];
        }

        while input.len() >= 16 {
            let chunk: &[u8; 16] = match input[..16].try_into() {
                Ok(c) => c,
                Err(_) => break,
            };
            let w0 = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            let w1 = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
            let w2 = u32::from_le_bytes([chunk[8], chunk[9], chunk[10], chunk[11]]);
            let w3 = u32::from_le_bytes([chunk[12], chunk[13], chunk[14], chunk[15]]);

            self.v1 = round(self.v1, w0);
            self.v2 = round(self.v2, w1);
            self.v3 = round(self.v3, w2);
            self.v4 = round(self.v4, w3);

            input = &input[16..];
        }

        if !input.is_empty() {
            self.buffer[self.buffer_len..self.buffer_len + input.len()].copy_from_slice(input);
            self.buffer_len += input.len();
        }
    }
}

// MARK: - Unit Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    extern "C" {
        fn XXH32(input: *const libc::c_void, len: usize, seed: libc::c_uint) -> libc::c_uint;
    }

    #[test]
    fn test_xxh32_matches_c_reference() {
        let test_cases: &[&[u8]] = &[
            b"",
            b"a",
            b"ab",
            b"abc",
            b"abcd",
            b"12345678",
            b"123456789012345",
            b"1234567890123456",
            b"12345678901234567",
            b"1234567890123456789012345678901234567890",
            b"The quick brown fox jumps over the lazy dog",
            b"TTZip high-performance archiving and compression microkernel 2026",
        ];

        for &data in test_cases {
            for seed in [0u32, 1, 42, 0x12345678, 0xDEADBEEF] {
                let rust_hash = xxh32(data, seed);
                let c_hash = unsafe {
                    XXH32(data.as_ptr() as *const libc::c_void, data.len(), seed as libc::c_uint)
                } as u32;

                assert_eq!(
                    rust_hash, c_hash,
                    "Hash mismatch for data len {} with seed {}: rust=0x{:08X}, c=0x{:08X}",
                    data.len(), seed, rust_hash, c_hash
                );
            }
        }
    }

    #[test]
    fn test_xxh32_streaming_matches_block() {
        let payload = b"TTZip high-performance native compression engine XXH32 streaming verification.";
        let mut hasher = Xxh32Hasher::new();
        hasher.write(&payload[..10]);
        hasher.write(&payload[10..25]);
        hasher.write(&payload[25..55]);
        hasher.write(&payload[55..]);

        let direct = xxh32(payload, 0);
        assert_eq!(hasher.digest(), direct);
        assert_eq!(hasher.finish() as u32, direct);
    }

    #[test]
    fn test_xxh32_custom_seeds() {
        let payload = b"Custom seed verification payload.";
        for seed in [0u32, 1, 42, 0x12345678, 0xDEADBEEF] {
            let direct = xxh32(payload, seed);
            let mut hasher = Xxh32Hasher::with_seed(seed);
            hasher.write(payload);
            assert_eq!(hasher.digest(), direct);
        }
    }
}
