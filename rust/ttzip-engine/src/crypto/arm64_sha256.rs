// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! ARM64 hardware-accelerated SHA-256 vector pipeline and 7z KDF engine.
//!
//! Inspired by 7-Zip `Sha256Opt.c`, this module implements ARMv8-A Cryptography
//! Extensions vector instructions (`SHA256H`, `SHA256H2`, `SHA256SU0`, `SHA256SU1`)
//! for high-throughput 4-round interleaved pipeline compression, cross-platform
//! safe dynamic dispatch with scalar fallback, and 64-way pre-expanded batching
//! for 7z 524,288-cycle password stretching KDF.

use parking_lot::Mutex;
use std::sync::LazyLock;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Standard SHA-256 round constant table (K256).
pub const K256: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// Initial SHA-256 hash state (H0..H7).
pub const INITIAL_H: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
    0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

// ============================================================================
// 1. ARM64 Crypto Extension Hardware Vector Pipeline
// ============================================================================

#[cfg(target_arch = "aarch64")]
pub mod arm64 {
    use super::*;
    use core::arch::aarch64::*;
    use core::arch::asm;

    #[inline(always)]
    pub unsafe fn vsha256hq_u32(mut hash_abcd: uint32x4_t, hash_efgh: uint32x4_t, wk: uint32x4_t) -> uint32x4_t {
        asm!(
            "SHA256H {hash_abcd:q}, {hash_efgh:q}, {wk:v}.4S",
            hash_abcd = inout(vreg) hash_abcd,
            hash_efgh = in(vreg) hash_efgh,
            wk = in(vreg) wk,
            options(pure, nomem, nostack, preserves_flags)
        );
        hash_abcd
    }

    #[inline(always)]
    pub unsafe fn vsha256h2q_u32(mut hash_efgh: uint32x4_t, hash_abcd: uint32x4_t, wk: uint32x4_t) -> uint32x4_t {
        asm!(
            "SHA256H2 {hash_efgh:q}, {hash_abcd:q}, {wk:v}.4S",
            hash_efgh = inout(vreg) hash_efgh,
            hash_abcd = in(vreg) hash_abcd,
            wk = in(vreg) wk,
            options(pure, nomem, nostack, preserves_flags)
        );
        hash_efgh
    }

    #[inline(always)]
    pub unsafe fn vsha256su0q_u32(mut w0_3: uint32x4_t, w4_7: uint32x4_t) -> uint32x4_t {
        asm!(
            "SHA256SU0 {w0_3:v}.4S, {w4_7:v}.4S",
            w0_3 = inout(vreg) w0_3,
            w4_7 = in(vreg) w4_7,
            options(pure, nomem, nostack, preserves_flags)
        );
        w0_3
    }

    #[inline(always)]
    pub unsafe fn vsha256su1q_u32(mut tw0_3: uint32x4_t, w8_11: uint32x4_t, w12_15: uint32x4_t) -> uint32x4_t {
        asm!(
            "SHA256SU1 {tw0_3:v}.4S, {w8_11:v}.4S, {w12_15:v}.4S",
            tw0_3 = inout(vreg) tw0_3,
            w8_11 = in(vreg) w8_11,
            w12_15 = in(vreg) w12_15,
            options(pure, nomem, nostack, preserves_flags)
        );
        tw0_3
    }

    /// Compresses contiguous 64-byte blocks into state using ARMv8 SHA-256 crypto instructions.
    ///
    /// # Safety
    /// `data` must point to `blocks * 64` valid readable bytes.
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

            // Rounds 0-15 (Initial schedule)
            round_quad!(s0, 0);
            round_quad!(s1, 4);
            round_quad!(s2, 8);
            round_quad!(s3, 12);

            // Rounds 16-63 (Message schedule expansion + interleaved rounds)
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

/// Direct exported entry point for ARM64 crypto hardware block compression.
///
/// # Safety
/// Caller must ensure target architecture supports ARMv8 Cryptography Extensions
/// and `data` points to `blocks * 64` valid readable bytes.
#[cfg(target_arch = "aarch64")]
#[inline]
pub unsafe fn sha256_compress_arm64_crypto(state: &mut [u32; 8], data: *const u8, blocks: usize) {
    arm64::compress_blocks_arm64(state, data, blocks);
}

// ============================================================================
// 2. High-Quality Scalar Reference Block Compression
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

    /// Compresses contiguous 64-byte blocks into state using pure scalar fallback.
    ///
    /// # Safety
    /// `data` must point to `blocks * 64` valid readable bytes.
    pub fn compress_blocks_scalar(state: &mut [u32; 8], mut data: *const u8, blocks: usize) {
        for _ in 0..blocks {
            let mut w = [0u32; 64];
            for i in 0..16 {
                w[i] = unsafe { u32::from_be(std::ptr::read_unaligned(data.add(i * 4) as *const u32)) };
            }
            for i in 16..64 {
                w[i] = gamma1(w[i - 2])
                    .wrapping_add(w[i - 7])
                    .wrapping_add(gamma0(w[i - 15]))
                    .wrapping_add(w[i - 16]);
            }

            let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h) =
                (state[0], state[1], state[2], state[3], state[4], state[5], state[6], state[7]);

            for i in 0..64 {
                let t1 = h
                    .wrapping_add(sigma1(e))
                    .wrapping_add(ch(e, f, g))
                    .wrapping_add(K256[i])
                    .wrapping_add(w[i]);
                let t2 = sigma0(a).wrapping_add(maj(a, b, c));
                h = g;
                g = f;
                f = e;
                e = d.wrapping_add(t1);
                d = c;
                c = b;
                b = a;
                a = t1.wrapping_add(t2);
            }

            state[0] = state[0].wrapping_add(a);
            state[1] = state[1].wrapping_add(b);
            state[2] = state[2].wrapping_add(c);
            state[3] = state[3].wrapping_add(d);
            state[4] = state[4].wrapping_add(e);
            state[5] = state[5].wrapping_add(f);
            state[6] = state[6].wrapping_add(g);
            state[7] = state[7].wrapping_add(h);

            unsafe {
                data = data.add(64);
            }
        }
    }
}

// ============================================================================
// 3. Cross-Platform Dynamic Dispatch Entry Point
// ============================================================================

/// Safe cross-platform dynamic dispatch entry point to compress 64-byte blocks into state.
///
/// Automatically dispatches to ARM64 Crypto Extension SIMD instructions when available,
/// or falls back to standard pure scalar arithmetic.
pub fn sha256_compress_blocks(state: &mut [u32; 8], blocks: &[[u8; 64]]) {
    if blocks.is_empty() {
        return;
    }
    let data_ptr = blocks.as_ptr() as *const u8;
    let num_blocks = blocks.len();

    #[cfg(target_arch = "aarch64")]
    {
        #[cfg(target_os = "macos")]
        let has_crypto = true;
        #[cfg(not(target_os = "macos"))]
        let has_crypto = std::arch::is_aarch64_feature_detected!("sha2");

        if has_crypto {
            unsafe {
                arm64::compress_blocks_arm64(state, data_ptr, num_blocks);
            }
            return;
        }
    }

    scalar::compress_blocks_scalar(state, data_ptr, num_blocks);
}

// ============================================================================
// 4. Zero-Heap Streaming Hasher
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

    /// Returns the internal 8-word hash state.
    pub fn current_state(&self) -> [u32; 8] {
        self.state
    }

    /// Returns the total processed byte length.
    pub fn total_len(&self) -> u64 {
        self.total_len
    }

    /// Checks if internal state is fully zeroized.
    pub fn is_zeroized(&self) -> bool {
        self.state == [0u32; 8] && self.buffer == [0u8; 64] && self.buf_len == 0 && self.total_len == 0
    }

    #[inline(always)]
    fn compress(&mut self, data: *const u8, blocks: usize) {
        #[cfg(target_arch = "aarch64")]
        {
            #[cfg(target_os = "macos")]
            let has_crypto = true;
            #[cfg(not(target_os = "macos"))]
            let has_crypto = std::arch::is_aarch64_feature_detected!("sha2");

            if has_crypto {
                unsafe {
                    arm64::compress_blocks_arm64(&mut self.state, data, blocks);
                }
                return;
            }
        }

        scalar::compress_blocks_scalar(&mut self.state, data, blocks);
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
// 5. Thread-Safe Global LRU Key Cache
// ============================================================================

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
struct CachedKeyEntry {
    password: String,
    salt: Vec<u8>,
    num_cycles_power: u32,
    derived_key: [u8; 32],
}

struct KeyCacheInner {
    entries: Vec<CachedKeyEntry>,
    capacity: usize,
}

impl KeyCacheInner {
    fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
        }
    }

    fn get(&mut self, password: &str, salt: &[u8], num_cycles_power: u32) -> Option<[u8; 32]> {
        if let Some(pos) = self.entries.iter().position(|e| {
            e.num_cycles_power == num_cycles_power
                && e.salt.as_slice() == salt
                && e.password.as_str() == password
        }) {
            let entry = self.entries.remove(pos);
            let key = entry.derived_key;
            self.entries.insert(0, entry);
            Some(key)
        } else {
            None
        }
    }

    fn insert(&mut self, password: &str, salt: &[u8], num_cycles_power: u32, key: [u8; 32]) {
        if let Some(pos) = self.entries.iter().position(|e| {
            e.num_cycles_power == num_cycles_power
                && e.salt.as_slice() == salt
                && e.password.as_str() == password
        }) {
            self.entries.remove(pos);
        } else if self.entries.len() >= self.capacity && !self.entries.is_empty() {
            let mut evicted = self.entries.pop().unwrap();
            evicted.zeroize();
        }

        self.entries.insert(
            0,
            CachedKeyEntry {
                password: password.to_string(),
                salt: salt.to_vec(),
                num_cycles_power,
                derived_key: key,
            },
        );
    }

    fn clear(&mut self) {
        for entry in self.entries.iter_mut() {
            entry.zeroize();
        }
        self.entries.clear();
    }
}

/// Thread-safe LRU singleton cache for derived 7z AES encryption keys.
pub struct SevenZKeyCache {
    inner: Mutex<KeyCacheInner>,
}

impl SevenZKeyCache {
    /// Default maximum cache entries.
    pub const DEFAULT_CAPACITY: usize = 64;

    /// Creates a new key cache with specified capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(KeyCacheInner::new(capacity)),
        }
    }

    /// Accesses the global singleton key cache instance.
    pub fn global() -> &'static Self {
        static INSTANCE: LazyLock<SevenZKeyCache> =
            LazyLock::new(|| SevenZKeyCache::new(SevenZKeyCache::DEFAULT_CAPACITY));
        &INSTANCE
    }

    /// Queries the cache for a precomputed 32-byte key.
    pub fn get(&self, password: &str, salt: &[u8], num_cycles_power: u32) -> Option<[u8; 32]> {
        self.inner.lock().get(password, salt, num_cycles_power)
    }

    /// Inserts a newly computed 32-byte key into the cache.
    pub fn insert(&self, password: &str, salt: &[u8], num_cycles_power: u32, key: [u8; 32]) {
        self.inner.lock().insert(password, salt, num_cycles_power, key);
    }

    /// Clears and zeroizes all cached keys and credentials.
    pub fn clear(&self) {
        self.inner.lock().clear();
    }

    /// Returns the current number of cached entries.
    pub fn len(&self) -> usize {
        self.inner.lock().entries.len()
    }

    /// Checks if the key cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ============================================================================
// 6. 7z 524,288 Cycles KDF Key Derivation (64-way Pre-Expanded Batching)
// ============================================================================

/// Derives a 32-byte AES key for 7z archives using hardware-accelerated ARM64 SHA-256.
///
/// Features:
/// - 64-way pre-expanded batch buffering to fully saturate hardware SIMD vector pipelines.
/// - Zero heap allocation during key derivation iterations.
/// - Full `Zeroize` erasure of sensitive plaintext and intermediate stack buffers.
/// - Fast LRU singleton cache lookup for instant repeated access.
pub fn derive_7z_key_arm64(password: &str, salt: &[u8], num_cycles_power: u32) -> [u8; 32] {
    if let Some(cached) = SevenZKeyCache::global().get(password, salt, num_cycles_power) {
        return cached;
    }

    let mut hasher = HardwareSha256::new();
    let num_cycles = 1u64 << num_cycles_power;

    let mut utf16_buf = [0u8; 1024];
    let mut utf16_len = 0;

    for val in password.encode_utf16() {
        if utf16_len + 2 <= utf16_buf.len() {
            let bytes = val.to_le_bytes();
            utf16_buf[utf16_len] = bytes[0];
            utf16_buf[utf16_len + 1] = bytes[1];
            utf16_len += 2;
        }
    }

    let pass_bytes = &utf16_buf[..utf16_len];
    let prefix_len = salt.len() + pass_bytes.len();
    let step_len = prefix_len + 8;

    if step_len <= 1024 {
        let mut iter_buf = [0u8; 1024];
        if !salt.is_empty() {
            iter_buf[..salt.len()].copy_from_slice(salt);
        }
        if !pass_bytes.is_empty() {
            iter_buf[salt.len()..prefix_len].copy_from_slice(pass_bytes);
        }

        const BATCH_BUF_SIZE: usize = 8192;
        let batch_count = (BATCH_BUF_SIZE / step_len).clamp(1, 64);
        let mut batch_buf = [0u8; BATCH_BUF_SIZE];
        let batch_total_bytes = batch_count * step_len;

        for step_idx in 0..batch_count {
            let offset = step_idx * step_len;
            batch_buf[offset..offset + prefix_len].copy_from_slice(&iter_buf[..prefix_len]);
        }

        let mut i = 0u64;
        while i + (batch_count as u64) <= num_cycles {
            for step_idx in 0..batch_count {
                let current_cycle = i + (step_idx as u64);
                let counter_offset = step_idx * step_len + prefix_len;
                batch_buf[counter_offset..counter_offset + 8].copy_from_slice(&current_cycle.to_le_bytes());
            }
            hasher.update(&batch_buf[..batch_total_bytes]);
            i += batch_count as u64;
        }

        while i < num_cycles {
            iter_buf[prefix_len..step_len].copy_from_slice(&i.to_le_bytes());
            hasher.update(&iter_buf[..step_len]);
            i += 1;
        }

        iter_buf.zeroize();
        batch_buf.zeroize();
    } else {
        for i in 0..num_cycles {
            if !salt.is_empty() {
                hasher.update(salt);
            }
            if !pass_bytes.is_empty() {
                hasher.update(pass_bytes);
            }
            hasher.update(&i.to_le_bytes());
        }
    }

    utf16_buf.zeroize();

    let key = hasher.finalize();
    SevenZKeyCache::global().insert(password, salt, num_cycles_power, key);
    key
}
