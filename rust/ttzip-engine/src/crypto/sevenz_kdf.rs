// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Hardware-accelerated 7z AES-256 Key Derivation Function (KDF) and Sensitive Memory Zeroization.
//!
//! Provides high-throughput SHA-256 key derivation conforming to the official 7-Zip AES
//! specification, integrating ARM64 Crypto Extensions (`SHA256H`) and x86 SHA-NI hardware
//! acceleration via the `sha2` crate, DoS exhaustion prevention via strict cycle upper limits,
//! raw key (`0x3F`) pass-through mode, thread-safe LRU caching, and sensitive memory erasure.

use parking_lot::Mutex;
use std::sync::LazyLock;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::crypto::sha256::HardwareSha256;
use crate::sevenz::dag::SevenZError;

/// Maximum allowed AES KDF cycle power (2^24 iterations) for DoS defense.
pub const MAX_AES_CYCLES_POWER: u8 = 24;

/// Special cycle power value indicating raw key pass-through mode in 7z format.
pub const RAW_KEY_CYCLES_POWER: u8 = 0x3F;

/// Secure container holding the 32-byte AES-256 key and 16-byte CBC initialization vector.
///
#[derive(Clone, PartialEq, Eq)]
pub struct DerivedKey {
    /// 256-bit AES encryption/decryption key.
    pub key: [u8; 32],
    /// 128-bit AES CBC initialization vector.
    pub iv: [u8; 16],
}

impl Zeroize for DerivedKey {
    #[inline]
    fn zeroize(&mut self) {
        self.key.zeroize();
        self.iv.zeroize();
    }
}

impl Drop for DerivedKey {
    #[inline]
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for DerivedKey {}


impl DerivedKey {
    /// Creates a new `DerivedKey` with the specified key and IV.
    #[inline]
    pub const fn new(key: [u8; 32], iv: [u8; 16]) -> Self {
        Self { key, iv }
    }

    /// Returns a reference to the 32-byte AES key.
    #[inline]
    pub fn key(&self) -> &[u8; 32] {
        &self.key
    }

    /// Returns a reference to the 16-byte CBC initialization vector.
    #[inline]
    pub fn iv(&self) -> &[u8; 16] {
        &self.iv
    }
}

impl core::fmt::Debug for DerivedKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DerivedKey")
            .field("key", &"[REDACTED 32-BYTES]")
            .field("iv", &self.iv)
            .finish()
    }
}

// ============================================================================
// 1. Thread-Safe LRU Key Cache Pool
// ============================================================================

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
struct CachedKeyEntry {
    password_bytes: Vec<u8>,
    salt: Vec<u8>,
    cycles_power: u8,
    derived_key: [u8; 32],
}

struct AesKdfCacheInner {
    entries: Vec<CachedKeyEntry>,
    capacity: usize,
}

impl AesKdfCacheInner {
    fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
        }
    }

    fn get(&mut self, password_utf16le: &[u8], salt: &[u8], cycles_power: u8) -> Option<[u8; 32]> {
        if let Some(pos) = self.entries.iter().position(|e| {
            e.cycles_power == cycles_power
                && e.salt.as_slice() == salt
                && e.password_bytes.as_slice() == password_utf16le
        }) {
            let entry = self.entries.remove(pos);
            let key = entry.derived_key;
            self.entries.insert(0, entry);
            Some(key)
        } else {
            None
        }
    }

    fn insert(&mut self, password_utf16le: &[u8], salt: &[u8], cycles_power: u8, key: [u8; 32]) {
        if let Some(pos) = self.entries.iter().position(|e| {
            e.cycles_power == cycles_power
                && e.salt.as_slice() == salt
                && e.password_bytes.as_slice() == password_utf16le
        }) {
            self.entries.remove(pos);
        } else if self.entries.len() >= self.capacity && !self.entries.is_empty() {
            let mut evicted = self.entries.pop().unwrap();
            evicted.zeroize();
        }

        self.entries.insert(
            0,
            CachedKeyEntry {
                password_bytes: password_utf16le.to_vec(),
                salt: salt.to_vec(),
                cycles_power,
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

/// Thread-safe LRU cache pool for 7z AES-256 derived keys to prevent duplicate block derivations.
pub struct AesKdfCache {
    inner: Mutex<AesKdfCacheInner>,
}

impl AesKdfCache {
    /// Default capacity for the global KDF cache.
    pub const DEFAULT_CAPACITY: usize = 64;

    /// Creates a new `AesKdfCache` with the specified entry capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(AesKdfCacheInner::new(capacity)),
        }
    }

    /// Returns the global singleton instance of `AesKdfCache`.
    pub fn global() -> &'static Self {
        static INSTANCE: LazyLock<AesKdfCache> =
            LazyLock::new(|| AesKdfCache::new(AesKdfCache::DEFAULT_CAPACITY));
        &INSTANCE
    }

    /// Retrieves a cached 32-byte AES key if available.
    pub fn get(&self, password_utf16le: &[u8], salt: &[u8], cycles_power: u8) -> Option<[u8; 32]> {
        self.inner.lock().get(password_utf16le, salt, cycles_power)
    }

    /// Inserts a newly derived 32-byte AES key into the cache.
    pub fn insert(&self, password_utf16le: &[u8], salt: &[u8], cycles_power: u8, key: [u8; 32]) {
        self.inner.lock().insert(password_utf16le, salt, cycles_power, key);
    }

    /// Clears and zeroizes all cached keys and credentials.
    pub fn clear(&self) {
        self.inner.lock().clear();
    }

    /// Returns the current number of cached entries.
    pub fn len(&self) -> usize {
        self.inner.lock().entries.len()
    }

    /// Returns `true` if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ============================================================================
// 2. 7z AES-256 Key Derivation Function (KDF)
// ============================================================================

/// Encodes a UTF-8 password string into Little-Endian UTF-16 byte representation.
pub fn password_to_utf16le(password: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(password.len() * 2);
    for code_unit in password.encode_utf16() {
        out.extend_from_slice(&code_unit.to_le_bytes());
    }
    out
}

/// Derives the 32-byte AES-256 key and 16-byte IV for 7z archives using hardware-accelerated SHA-256.
///
/// # Parameters
/// - `password_utf16le`: Plaintext password encoded as Little-Endian UTF-16 bytes.
/// - `salt`: Cryptographic salt bytes (up to 16 bytes).
/// - `cycles_power`: Iteration count power ($2^{\text{cycles\_power}}$ rounds), or `0x3F` for raw key mode.
/// - `raw_iv`: Archive-provided IV bytes (0 to 16 bytes).
///
/// # Returns
/// - `Ok(DerivedKey)` containing 32-byte key and 16-byte IV on success.
/// - `Err(SevenZError::CryptoExhaustion)` if `cycles_power > MAX_AES_CYCLES_POWER` (24) and not `0x3F`.
pub fn derive_7z_aes_key(
    password_utf16le: &[u8],
    salt: &[u8],
    cycles_power: u8,
    raw_iv: &[u8],
) -> Result<DerivedKey, SevenZError> {
    // 1. Resolve 16-byte initialization vector (zero-padded)
    let mut iv = [0u8; 16];
    let copy_iv_len = raw_iv.len().min(16);
    if copy_iv_len > 0 {
        iv[..copy_iv_len].copy_from_slice(&raw_iv[..copy_iv_len]);
    }

    // 2. Check 0x3F Raw Key pass-through mode
    if cycles_power == RAW_KEY_CYCLES_POWER {
        let mut key = [0u8; 32];
        let copy_key_len = password_utf16le.len().min(32);
        if copy_key_len > 0 {
            key[..copy_key_len].copy_from_slice(&password_utf16le[..copy_key_len]);
        }
        return Ok(DerivedKey { key, iv });
    }

    // 3. Security threshold check for DoS / CPU exhaustion prevention
    if cycles_power > MAX_AES_CYCLES_POWER {
        return Err(SevenZError::CryptoExhaustion);
    }

    // 4. Fast LRU singleton cache lookup
    if let Some(cached_key) = AesKdfCache::global().get(password_utf16le, salt, cycles_power) {
        return Ok(DerivedKey { key: cached_key, iv });
    }

    // 5. Hardware-accelerated multi-round SHA-256 iteration
    let num_cycles = 1u64 << (cycles_power as u32);
    let prefix_len = salt.len() + password_utf16le.len();
    let step_len = prefix_len + 8;
    let mut hasher = HardwareSha256::new();

    if step_len <= 1024 {
        let mut iter_buf = [0u8; 1024];
        if !salt.is_empty() {
            iter_buf[..salt.len()].copy_from_slice(salt);
        }
        if !password_utf16le.is_empty() {
            iter_buf[salt.len()..prefix_len].copy_from_slice(password_utf16le);
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
                batch_buf[counter_offset..counter_offset + 8]
                    .copy_from_slice(&current_cycle.to_le_bytes());
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
            if !password_utf16le.is_empty() {
                hasher.update(password_utf16le);
            }
            hasher.update(&i.to_le_bytes());
        }
    }

    let digest_output = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&digest_output);

    // 6. Cache computed key for subsequent blocks
    AesKdfCache::global().insert(password_utf16le, salt, cycles_power, key);

    Ok(DerivedKey { key, iv })
}
