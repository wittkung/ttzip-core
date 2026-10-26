// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Hardware-accelerated CRC-32 instructions and multi-stream SIMD batch processing for ZipCrypto.

use super::scalar::decrypt_byte_key;

#[cfg(target_arch = "aarch64")]
use super::scalar::ZIPCRYPTO_MULT;

use super::scalar::update_keys;

#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::*;

/// Updates `key0`, `key1`, `key2` using hardware ARM64 `__crc32b` instructions where available.
#[inline(always)]
pub fn update_keys_fast(key0: &mut u32, key1: &mut u32, key2: &mut u32, plain_byte: u8) {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        // Bit-reflected CRC32 with polynomial 0xEDB88320 (IEEE 802.3)
        *key0 = __crc32b(*key0, plain_byte);
        *key1 = (*key1).wrapping_add(*key0 & 0xFF).wrapping_mul(ZIPCRYPTO_MULT).wrapping_add(1);
        *key2 = __crc32b(*key2, (*key1 >> 24) as u8);
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        update_keys(key0, key1, key2, plain_byte);
    }
}

/// Decrypts a contiguous buffer in-place with loop unrolling.
#[inline]
pub fn decrypt_stream_fast(key0: &mut u32, key1: &mut u32, key2: &mut u32, data: &mut [u8]) {
    let mut chunks = data.chunks_exact_mut(8);
    for chunk in chunks.by_ref() {
        for b in chunk.iter_mut() {
            let k = decrypt_byte_key(*key2);
            let p = *b ^ k;
            *b = p;
            update_keys_fast(key0, key1, key2, p);
        }
    }
    for b in chunks.into_remainder().iter_mut() {
        let k = decrypt_byte_key(*key2);
        let p = *b ^ k;
        *b = p;
        update_keys_fast(key0, key1, key2, p);
    }
}

/// Encrypts a contiguous buffer in-place with loop unrolling.
#[inline]
pub fn encrypt_stream_fast(key0: &mut u32, key1: &mut u32, key2: &mut u32, data: &mut [u8]) {
    let mut chunks = data.chunks_exact_mut(8);
    for chunk in chunks.by_ref() {
        for b in chunk.iter_mut() {
            let p = *b;
            let k = decrypt_byte_key(*key2);
            *b = p ^ k;
            update_keys_fast(key0, key1, key2, p);
        }
    }
    for b in chunks.into_remainder().iter_mut() {
        let p = *b;
        let k = decrypt_byte_key(*key2);
        *b = p ^ k;
        update_keys_fast(key0, key1, key2, p);
    }
}

/// 4-Way SIMD vertical batch state for processing 4 streams / blocks in parallel.
#[derive(Clone, Copy, Debug)]
#[repr(C, align(16))]
pub struct ZipCryptoBatch4 {
    pub key0: [u32; 4],
    pub key1: [u32; 4],
    pub key2: [u32; 4],
}

impl Default for ZipCryptoBatch4 {
    fn default() -> Self {
        Self::new()
    }
}

impl ZipCryptoBatch4 {
    /// Creates a new batch initialized with default PKZIP keys for all 4 slots.
    pub fn new() -> Self {
        Self {
            key0: [super::scalar::ZIPCRYPTO_KEY0_INIT; 4],
            key1: [super::scalar::ZIPCRYPTO_KEY1_INIT; 4],
            key2: [super::scalar::ZIPCRYPTO_KEY2_INIT; 4],
        }
    }

    /// Initializes batch from 4 distinct password byte slices.
    pub fn from_passwords(passwords: [&[u8]; 4]) -> Self {
        let mut batch = Self::new();
        for (lane, pwd) in passwords.iter().enumerate() {
            for &b in *pwd {
                update_keys_fast(
                    &mut batch.key0[lane],
                    &mut batch.key1[lane],
                    &mut batch.key2[lane],
                    b,
                );
            }
        }
        batch
    }

    /// Updates all 4 key states with a 4-byte plaintext batch.
    #[inline(always)]
    pub fn update_keys_4way(&mut self, plain_bytes: [u8; 4]) {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            self.update_keys_4way_neon(plain_bytes);
        }

        #[cfg(not(target_arch = "aarch64"))]
        {
            self.update_keys_4way_scalar(plain_bytes);
        }
    }

    /// AArch64 hardware NEON and CRC32b accelerated 4-lane state update kernel.
    #[cfg(target_arch = "aarch64")]
    #[inline(always)]
    pub unsafe fn update_keys_4way_neon(&mut self, plain: [u8; 4]) {
        // 1. Parallel CRC32b updates for key0 across all 4 independent lanes
        self.key0[0] = __crc32b(self.key0[0], plain[0]);
        self.key0[1] = __crc32b(self.key0[1], plain[1]);
        self.key0[2] = __crc32b(self.key0[2], plain[2]);
        self.key0[3] = __crc32b(self.key0[3], plain[3]);

        // 2. Vectorized 4-lane LCG update for key1:
        // key1 = (key1 + (key0 & 0xFF)) * 134775813 + 1 (mod 2^32)
        let v_key0 = vld1q_u32(self.key0.as_ptr());
        let v_key1 = vld1q_u32(self.key1.as_ptr());
        let v_k0_low = vandq_u32(v_key0, vdupq_n_u32(0xFF));
        let sum = vaddq_u32(v_key1, v_k0_low);
        let v_key1_new = vmlaq_u32(vdupq_n_u32(1), sum, vdupq_n_u32(134775813));
        vst1q_u32(self.key1.as_mut_ptr(), v_key1_new);

        // 3. Extract high bytes (bits 24..31) from key1 via NEON shift and update key2 using __crc32b
        let v_b2 = vshrq_n_u32(v_key1_new, 24);
        self.key2[0] = __crc32b(self.key2[0], vgetq_lane_u32::<0>(v_b2) as u8);
        self.key2[1] = __crc32b(self.key2[1], vgetq_lane_u32::<1>(v_b2) as u8);
        self.key2[2] = __crc32b(self.key2[2], vgetq_lane_u32::<2>(v_b2) as u8);
        self.key2[3] = __crc32b(self.key2[3], vgetq_lane_u32::<3>(v_b2) as u8);
    }

    /// Scalar fallback 4-lane state update.
    #[inline(always)]
    pub fn update_keys_4way_scalar(&mut self, plain: [u8; 4]) {
        for lane in 0..4 {
            update_keys(
                &mut self.key0[lane],
                &mut self.key1[lane],
                &mut self.key2[lane],
                plain[lane],
            );
        }
    }

    /// Decrypts one byte across all 4 lanes simultaneously.
    #[inline(always)]
    pub fn decrypt_bytes_4way(&mut self, cipher_bytes: [u8; 4]) -> [u8; 4] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            // Keystream derivation via 4-way NEON vector arithmetic
            let v_key2 = vld1q_u32(self.key2.as_ptr());
            let temp = vorrq_u32(vandq_u32(v_key2, vdupq_n_u32(0xFFFF)), vdupq_n_u32(2));
            let temp_xor_1 = veorq_u32(temp, vdupq_n_u32(1));
            let prod = vmulq_u32(temp, temp_xor_1);
            let k_vec = vandq_u32(vshrq_n_u32(prod, 8), vdupq_n_u32(0xFF));

            let cipher_u32 = [
                cipher_bytes[0] as u32,
                cipher_bytes[1] as u32,
                cipher_bytes[2] as u32,
                cipher_bytes[3] as u32,
            ];
            let v_cipher = vld1q_u32(cipher_u32.as_ptr());
            let v_plain = veorq_u32(v_cipher, k_vec);

            let plain = [
                vgetq_lane_u32::<0>(v_plain) as u8,
                vgetq_lane_u32::<1>(v_plain) as u8,
                vgetq_lane_u32::<2>(v_plain) as u8,
                vgetq_lane_u32::<3>(v_plain) as u8,
            ];

            self.update_keys_4way_neon(plain);
            plain
        }

        #[cfg(not(target_arch = "aarch64"))]
        {
            let mut plain = [0u8; 4];
            for lane in 0..4 {
                let k = decrypt_byte_key(self.key2[lane]);
                let p = cipher_bytes[lane] ^ k;
                plain[lane] = p;
                update_keys(
                    &mut self.key0[lane],
                    &mut self.key1[lane],
                    &mut self.key2[lane],
                    p,
                );
            }
            plain
        }
    }

    /// Encrypts one byte across all 4 lanes simultaneously.
    #[inline(always)]
    pub fn encrypt_bytes_4way(&mut self, plain_bytes: [u8; 4]) -> [u8; 4] {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            // Keystream derivation via 4-way NEON vector arithmetic
            let v_key2 = vld1q_u32(self.key2.as_ptr());
            let temp = vorrq_u32(vandq_u32(v_key2, vdupq_n_u32(0xFFFF)), vdupq_n_u32(2));
            let temp_xor_1 = veorq_u32(temp, vdupq_n_u32(1));
            let prod = vmulq_u32(temp, temp_xor_1);
            let k_vec = vandq_u32(vshrq_n_u32(prod, 8), vdupq_n_u32(0xFF));

            let plain_u32 = [
                plain_bytes[0] as u32,
                plain_bytes[1] as u32,
                plain_bytes[2] as u32,
                plain_bytes[3] as u32,
            ];
            let v_plain = vld1q_u32(plain_u32.as_ptr());
            let v_cipher = veorq_u32(v_plain, k_vec);

            let cipher = [
                vgetq_lane_u32::<0>(v_cipher) as u8,
                vgetq_lane_u32::<1>(v_cipher) as u8,
                vgetq_lane_u32::<2>(v_cipher) as u8,
                vgetq_lane_u32::<3>(v_cipher) as u8,
            ];

            self.update_keys_4way_neon(plain_bytes);
            cipher
        }

        #[cfg(not(target_arch = "aarch64"))]
        {
            let mut cipher = [0u8; 4];
            for lane in 0..4 {
                let p = plain_bytes[lane];
                let k = decrypt_byte_key(self.key2[lane]);
                cipher[lane] = p ^ k;
                update_keys(
                    &mut self.key0[lane],
                    &mut self.key1[lane],
                    &mut self.key2[lane],
                    p,
                );
            }
            cipher
        }
    }
}
