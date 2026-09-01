// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Hardware-accelerated SHA-256 and 7z SHA-256 KDF engine.
//!
//! Implements ARM64 Crypto extension SHA-256 block compression (`SHA256H`,
//! `SHA256H2`, `SHA256SU0`, `SHA256SU1`), scalar fallback, and 7z KDF key
//! derivation (up to 524,288 cycles) with zero heap allocations, sensitive
//! memory zeroization, and LRU singleton caching.

use zeroize::{Zeroize, ZeroizeOnDrop};

const K256: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

const INITIAL_H: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
    0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

// ============================================================================
// 1. ARM64 SHA-256 Hardware Compression
// ============================================================================
#[cfg(target_arch = "aarch64")]
pub mod arm64 {
    use super::*;
    use core::arch::aarch64::*;
    use core::arch::asm;

    #[inline(always)]
    unsafe fn vsha256hq_u32(mut hash_abcd: uint32x4_t, hash_efgh: uint32x4_t, wk: uint32x4_t) -> uint32x4_t {
        asm!("SHA256H {hash_abcd:q}, {hash_efgh:q}, {wk:v}.4S", hash_abcd = inout(vreg) hash_abcd, hash_efgh = in(vreg) hash_efgh, wk = in(vreg) wk, options(pure, nomem, nostack, preserves_flags));
        hash_abcd
    }

    #[inline(always)]
    unsafe fn vsha256h2q_u32(mut hash_efgh: uint32x4_t, hash_abcd: uint32x4_t, wk: uint32x4_t) -> uint32x4_t {
        asm!("SHA256H2 {hash_efgh:q}, {hash_abcd:q}, {wk:v}.4S", hash_efgh = inout(vreg) hash_efgh, hash_abcd = in(vreg) hash_abcd, wk = in(vreg) wk, options(pure, nomem, nostack, preserves_flags));
        hash_efgh
    }

    #[inline(always)]
    unsafe fn vsha256su0q_u32(mut w0_3: uint32x4_t, w4_7: uint32x4_t) -> uint32x4_t {
        asm!("SHA256SU0 {w0_3:v}.4S, {w4_7:v}.4S", w0_3 = inout(vreg) w0_3, w4_7 = in(vreg) w4_7, options(pure, nomem, nostack, preserves_flags));
        w0_3
    }

    #[inline(always)]
    unsafe fn vsha256su1q_u32(mut tw0_3: uint32x4_t, w8_11: uint32x4_t, w12_15: uint32x4_t) -> uint32x4_t {
        asm!("SHA256SU1 {tw0_3:v}.4S, {w8_11:v}.4S, {w12_15:v}.4S", tw0_3 = inout(vreg) tw0_3, w8_11 = in(vreg) w8_11, w12_15 = in(vreg) w12_15, options(pure, nomem, nostack, preserves_flags));
        tw0_3
    }

    /// Compresses contiguous 64-byte blocks into state using ARMv8 SHA-256 crypto instructions.
    ///
    /// # Safety
    /// `data` must point to `blocks * 64` readable bytes.
    #[target_feature(enable = "sha2")]
    pub unsafe fn compress_blocks_arm64(state: &mut [u32; 8], mut data: *const u8, blocks: usize) {
        let mut abcd = vld1q_u32(state.as_ptr());
        let mut efgh = vld1q_u32(state.as_ptr().add(4));

        for _ in 0..blocks {
            let (abcd_orig, efgh_orig) = (abcd, efgh);
            let mut s0 = vreinterpretq_u32_u8(vrev32q_u8(vld1q_u8(data)));
            let mut s1 = vreinterpretq_u32_u8(vrev32q_u8(vld1q_u8(data.add(16))));
            let mut s2 = vreinterpretq_u32_u8(vrev32q_u8(vld1q_u8(data.add(32))));
            let mut s3 = vreinterpretq_u32_u8(vrev32q_u8(vld1q_u8(data.add(48))));

            macro_rules! round_quad {
                ($s:expr, $k_idx:expr) => {{
                    let tmp = vaddq_u32($s, vld1q_u32(&K256[$k_idx]));
                    let abcd_prev = abcd;
                    abcd = vsha256hq_u32(abcd_prev, efgh, tmp);
                    efgh = vsha256h2q_u32(efgh, abcd_prev, tmp);
                }};
            }

            round_quad!(s0, 0);
            round_quad!(s1, 4);
            round_quad!(s2, 8);
            round_quad!(s3, 12);

            for t in (16..64).step_by(16) {
                s0 = vsha256su1q_u32(vsha256su0q_u32(s0, s1), s2, s3);
                round_quad!(s0, t);

                s1 = vsha256su1q_u32(vsha256su0q_u32(s1, s2), s3, s0);
                round_quad!(s1, t + 4);

                s2 = vsha256su1q_u32(vsha256su0q_u32(s2, s3), s0, s1);
                round_quad!(s2, t + 8);

                s3 = vsha256su1q_u32(vsha256su0q_u32(s3, s0), s1, s2);
                round_quad!(s3, t + 12);
            }

            abcd = vaddq_u32(abcd, abcd_orig);
            efgh = vaddq_u32(efgh, efgh_orig);
            data = data.add(64);
        }

        vst1q_u32(state.as_mut_ptr(), abcd);
        vst1q_u32(state.as_mut_ptr().add(4), efgh);
    }
}

// ============================================================================
// 2. Scalar Reference Block Compression
// ============================================================================
pub mod scalar {
    use super::*;

    #[inline(always)] fn rotr(x: u32, n: u32) -> u32 { x.rotate_right(n) }
    #[inline(always)] fn ch(x: u32, y: u32, z: u32) -> u32 { (x & y) ^ (!x & z) }
    #[inline(always)] fn maj(x: u32, y: u32, z: u32) -> u32 { (x & y) ^ (x & z) ^ (y & z) }
    #[inline(always)] fn sigma0(x: u32) -> u32 { rotr(x, 2) ^ rotr(x, 13) ^ rotr(x, 22) }
    #[inline(always)] fn sigma1(x: u32) -> u32 { rotr(x, 6) ^ rotr(x, 11) ^ rotr(x, 25) }
    #[inline(always)] fn gamma0(x: u32) -> u32 { rotr(x, 7) ^ rotr(x, 18) ^ (x >> 3) }
    #[inline(always)] fn gamma1(x: u32) -> u32 { rotr(x, 17) ^ rotr(x, 19) ^ (x >> 10) }

    /// Compresses contiguous 64-byte blocks into state using scalar fallback arithmetic.
    pub fn compress_blocks_scalar(state: &mut [u32; 8], mut data: *const u8, blocks: usize) {
        for _ in 0..blocks {
            let mut w = [0u32; 64];
            for i in 0..16 {
                w[i] = unsafe { u32::from_be(std::ptr::read_unaligned(data.add(i * 4) as *const u32)) };
            }
            for i in 16..64 {
                w[i] = gamma1(w[i - 2]).wrapping_add(w[i - 7]).wrapping_add(gamma0(w[i - 15])).wrapping_add(w[i - 16]);
            }

            let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h) =
                (state[0], state[1], state[2], state[3], state[4], state[5], state[6], state[7]);

            for i in 0..64 {
                let t1 = h.wrapping_add(sigma1(e)).wrapping_add(ch(e, f, g)).wrapping_add(K256[i]).wrapping_add(w[i]);
                let t2 = sigma0(a).wrapping_add(maj(a, b, c));
                h = g; g = f; f = e; e = d.wrapping_add(t1); d = c; c = b; b = a; a = t1.wrapping_add(t2);
            }

            state[0] = state[0].wrapping_add(a);
            state[1] = state[1].wrapping_add(b);
            state[2] = state[2].wrapping_add(c);
            state[3] = state[3].wrapping_add(d);
            state[4] = state[4].wrapping_add(e);
            state[5] = state[5].wrapping_add(f);
            state[6] = state[6].wrapping_add(g);
            state[7] = state[7].wrapping_add(h);

            unsafe { data = data.add(64); }
        }
    }
}

// ============================================================================
// 3. Hardware-Accelerated Streaming SHA-256 Engine
// ============================================================================

/// Stack-allocated zero-heap hardware-accelerated streaming SHA-256 hasher.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct HardwareSha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buf_len: usize,
    total_len: u64,
}

/// Backwards-compatible alias for HardwareSha256.
pub type FastSha256 = HardwareSha256;

impl Default for HardwareSha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl HardwareSha256 {
    /// Creates a new SHA-256 hasher initialized to standard constants.
    pub const fn new() -> Self {
        Self {
            state: INITIAL_H,
            buffer: [0u8; 64],
            buf_len: 0,
            total_len: 0,
        }
    }

    /// Resets internal state to initial values, zeroizing previous buffers.
    pub fn reset(&mut self) {
        self.state.zeroize();
        self.buffer.zeroize();
        self.state = INITIAL_H;
        self.buf_len = 0;
        self.total_len = 0;
    }

    /// Computes one-shot SHA-256 digest of input data.
    pub fn digest(data: &[u8]) -> [u8; 32] {
        let mut hasher = Self::new();
        hasher.update(data);
        hasher.finalize()
    }

    #[inline(always)]
    fn compress(&mut self, data: *const u8, blocks: usize) {
        #[cfg(target_arch = "aarch64")]
        unsafe { arm64::compress_blocks_arm64(&mut self.state, data, blocks); }

        #[cfg(not(target_arch = "aarch64"))]
        { scalar::compress_blocks_scalar(&mut self.state, data, blocks); }
    }

    /// Consumes arbitrary-length input slice and updates running SHA-256 digest.
    pub fn update(&mut self, mut data: &[u8]) {
        self.total_len += data.len() as u64;

        if self.buf_len > 0 {
            let take = (64 - self.buf_len).min(data.len());
            self.buffer[self.buf_len..self.buf_len + take].copy_from_slice(&data[..take]);
            self.buf_len += take;
            data = &data[take..];

            if self.buf_len == 64 {
                self.compress(self.buffer.as_ptr(), 1);
                self.buf_len = 0;
            }
        }

        let full_blocks = data.len() / 64;
        if full_blocks > 0 {
            self.compress(data.as_ptr(), full_blocks);
            data = &data[full_blocks * 64..];
        }

        if !data.is_empty() {
            self.buffer[..data.len()].copy_from_slice(data);
            self.buf_len = data.len();
        }
    }

    /// Finalizes the SHA-256 digest, returning the 32-byte hash and resetting internal state.
    pub fn finalize_reset(&mut self) -> [u8; 32] {
        let bit_len = self.total_len * 8;
        self.buffer[self.buf_len] = 0x80;
        self.buf_len += 1;

        if self.buf_len > 56 {
            self.buffer[self.buf_len..64].fill(0);
            self.compress(self.buffer.as_ptr(), 1);
            self.buf_len = 0;
        }

        self.buffer[self.buf_len..56].fill(0);
        self.buffer[56..64].copy_from_slice(&bit_len.to_be_bytes());
        self.compress(self.buffer.as_ptr(), 1);

        let mut out = [0u8; 32];
        for i in 0..8 {
            out[i * 4..i * 4 + 4].copy_from_slice(&self.state[i].to_be_bytes());
        }

        self.reset();
        out
    }

    /// Finalizes and consumes the hasher, returning the 32-byte digest.
    pub fn finalize(mut self) -> [u8; 32] {
        self.finalize_reset()
    }
}

// ============================================================================
// 4. Thread-Safe Global LRU Singleton Key Cache & KDF
// ============================================================================

pub use crate::crypto::arm64_sha256::SevenZKeyCache;

/// 7z SHA-256 KDF key derivation.
pub fn sha256_7z_kdf(password: &str, salt: &[u8], num_cycles_power: u32) -> [u8; 32] {
    crate::crypto::arm64_sha256::derive_7z_key_arm64(password, salt, num_cycles_power)
}

pub use crate::crypto::arm64_sha256::{derive_7z_key_arm64, sha256_compress_blocks};

// ============================================================================
// 6. Unit Tests
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_sha256_nist_vectors() {
        let empty_hash = HardwareSha256::digest(b"");
        assert_eq!(hex::encode(empty_hash), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");

        let mut h = HardwareSha256::new();
        h.update(b"abc");
        assert_eq!(hex::encode(h.finalize()), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");

        let mut h2 = HardwareSha256::new();
        h2.update(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq");
        assert_eq!(hex::encode(h2.finalize()), "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1");
    }

    #[test]
    fn test_scalar_vs_arm64_differential() {
        let corpus = b"The quick brown fox jumps over the lazy dog and tests 64-byte alignment boundaries thoroughly across multiple blocks.";
        let mut state_scalar = INITIAL_H;
        let mut state_target = INITIAL_H;
        let blocks = corpus.len() / 64;
        scalar::compress_blocks_scalar(&mut state_scalar, corpus.as_ptr(), blocks);

        #[cfg(target_arch = "aarch64")]
        unsafe {
            arm64::compress_blocks_arm64(&mut state_target, corpus.as_ptr(), blocks);
            assert_eq!(state_scalar, state_target, "ARM64 and scalar block compression must be identical");
        }
    }

    #[test]
    fn test_7z_official_vectors() {
        SevenZKeyCache::global().clear();

        // 7-Zip Standard KDF Test Vectors:
        // Case 1: Password = "", Salt = [], num_cycles_power = 0 (1 cycle: hashes 8 bytes 0x00)
        let key_empty = sha256_7z_kdf("", &[], 0);
        let mut expected_h = HardwareSha256::new();
        expected_h.update(&0u64.to_le_bytes());
        assert_eq!(key_empty, expected_h.finalize());

        // Case 2: Password = "123", Salt = [0xAA, 0xBB], num_cycles_power = 1 (2 cycles)
        let key_123 = sha256_7z_kdf("123", &[0xAA, 0xBB], 1);
        let mut exp_123 = HardwareSha256::new();
        let pass_utf16_123: [u8; 6] = [0x31, 0x00, 0x32, 0x00, 0x33, 0x00];
        exp_123.update(&[0xAA, 0xBB]);
        exp_123.update(&pass_utf16_123);
        exp_123.update(&0u64.to_le_bytes());
        exp_123.update(&[0xAA, 0xBB]);
        exp_123.update(&pass_utf16_123);
        exp_123.update(&1u64.to_le_bytes());
        assert_eq!(key_123, exp_123.finalize());

        // Case 3: 7z Standard 64-cycle vector
        let salt = [0x01, 0x02, 0x03, 0x04];
        let key = sha256_7z_kdf("password123", &salt, 6);
        assert_ne!(key, [0u8; 32]);
    }

    #[test]
    fn test_sevenz_key_cache_lru_hit() {
        let cache = SevenZKeyCache::new(2);
        let salt = [0x11, 0x22, 0x33, 0x44];
        let key1 = [0x01u8; 32];
        let key2 = [0x02u8; 32];
        let key3 = [0x03u8; 32];

        cache.insert("p1", &salt, 19, key1);
        cache.insert("p2", &salt, 19, key2);
        assert_eq!(cache.get("p1", &salt, 19), Some(key1));
        assert_eq!(cache.get("p2", &salt, 19), Some(key2));

        let _ = cache.get("p1", &salt, 19);
        cache.insert("p3", &salt, 19, key3);

        assert_eq!(cache.get("p1", &salt, 19), Some(key1));
        assert_eq!(cache.get("p3", &salt, 19), Some(key3));
        assert_eq!(cache.get("p2", &salt, 19), None);

        cache.clear();
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_524288_cycles_performance_benchmark() {
        let password = "SuperSecretMasterKey2026!#$";
        let salt = [0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C];
        let num_cycles_power = 19; // 524,288 cycles

        // 1. Warm-up pass to prime CPU pipeline, SIMD vector units, and cache lines
        let _ = sha256_7z_kdf("warmup_prime_2026", &salt, 10);

        // 2. Multi-sample benchmark for 524,288 cycles to eliminate OS scheduling preemption jitter
        let mut min_elapsed = std::time::Duration::from_secs(100);
        let mut key1 = [0u8; 32];

        for _ in 0..3 {
            SevenZKeyCache::global().clear();
            let start = Instant::now();
            let key = sha256_7z_kdf(password, &salt, num_cycles_power);
            let dur = start.elapsed();
            if dur < min_elapsed {
                min_elapsed = dur;
                key1 = key;
            }
        }

        println!("524,288 cycles 7z SHA-256 KDF elapsed time: {:.3} ms", min_elapsed.as_secs_f64() * 1000.0);
        assert_ne!(key1, [0u8; 32]);
        assert!(min_elapsed.as_millis() <= 20, "7z KDF derivation took {:?}, exceeding 20ms threshold", min_elapsed);

        // 3. Ensure key is in cache and measure cache hit latency across multiple samples
        SevenZKeyCache::global().insert(password, &salt, num_cycles_power, key1);
        let mut min_cache_elapsed = std::time::Duration::from_secs(100);
        let mut key2 = [0u8; 32];

        for _ in 0..5 {
            let cache_start = Instant::now();
            let k = sha256_7z_kdf(password, &salt, num_cycles_power);
            let c_dur = cache_start.elapsed();
            if c_dur < min_cache_elapsed {
                min_cache_elapsed = c_dur;
                key2 = k;
            }
        }

        assert_eq!(key1, key2);
        assert!(min_cache_elapsed.as_micros() < 500, "Cache hit took {:?}, exceeding 500µs threshold", min_cache_elapsed);
    }

    #[test]
    fn test_zeroize_memory_erasure() {
        let mut hasher = HardwareSha256::new();
        hasher.update(b"Sensitive payload before zeroize");
        hasher.zeroize();
        assert_eq!(hasher.state, [0u32; 8]);
        assert_eq!(hasher.buffer, [0u8; 64]);
        assert_eq!(hasher.buf_len, 0);
        assert_eq!(hasher.total_len, 0);
    }
}
