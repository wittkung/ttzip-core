// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Hardware-accelerated 7z AES-256 Key Derivation Function (KDF) and Sensitive Memory Zeroization.
//!
//! Provides high-throughput SHA-256 key derivation conforming to the official 7-Zip AES
//! specification, integrating ARM64 Crypto Extensions (`SHA256H`) and x86 SHA-NI hardware
//! acceleration via the `sha2` crate, DoS exhaustion prevention via strict cycle upper limits,
//! raw key (`0x3F`) pass-through mode, thread-safe LRU caching, and sensitive memory erasure.

use parking_lot::{Mutex, MutexGuard};
use std::sync::LazyLock;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::arm64_sha256::HardwareSha256;

/// Errors that can occur during 7z AES key derivation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SevenZKdfError {
    /// AES-256 KDF cycles power exceeds maximum security threshold (DoS / CPU exhaustion defense).
    #[error("7z AES-256 KDF cycles power exceeds maximum security threshold (max 24)")]
    CryptoExhaustion,
}

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
// 1. Thread-Safe Unified LRU Key Cache Pool
// ============================================================================

#[derive(Clone)]
struct CachedKeyEntry {
    password_bytes: Zeroizing<Vec<u8>>,
    salt: Vec<u8>,
    cycles_power: u32,
    derived_key: Zeroizing<[u8; 32]>,
}

impl Zeroize for CachedKeyEntry {
    #[inline]
    fn zeroize(&mut self) {
        self.password_bytes.zeroize();
        self.salt.zeroize();
        self.cycles_power.zeroize();
        self.derived_key.zeroize();
    }
}

pub(crate) struct AesKdfCacheInner {
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

    fn get(
        &mut self,
        password_utf16le: &[u8],
        salt: &[u8],
        cycles_power: u32,
    ) -> Option<Zeroizing<[u8; 32]>> {
        if let Some(pos) = self.entries.iter().position(|e| {
            e.cycles_power == cycles_power
                && e.salt.as_slice() == salt
                && e.password_bytes.as_slice() == password_utf16le
        }) {
            let entry = self.entries.remove(pos);
            let key = entry.derived_key.clone();
            self.entries.insert(0, entry);
            Some(key)
        } else {
            None
        }
    }

    fn insert(
        &mut self,
        password_utf16le: &[u8],
        salt: &[u8],
        cycles_power: u32,
        key: Zeroizing<[u8; 32]>,
    ) {
        if let Some(pos) = self.entries.iter().position(|e| {
            e.cycles_power == cycles_power
                && e.salt.as_slice() == salt
                && e.password_bytes.as_slice() == password_utf16le
        }) {
            let mut removed = self.entries.remove(pos);
            removed.zeroize();
        } else if self.entries.len() >= self.capacity
            && let Some(mut evicted) = self.entries.pop()
        {
            evicted.zeroize();
        }

        self.entries.insert(
            0,
            CachedKeyEntry {
                password_bytes: Zeroizing::new(password_utf16le.to_vec()),
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

static GLOBAL_KDF_CACHE_INNER: LazyLock<Mutex<AesKdfCacheInner>> =
    LazyLock::new(|| Mutex::new(AesKdfCacheInner::new(AesKdfCache::DEFAULT_CAPACITY)));

enum KdfCacheBackend {
    Global,
    Local(Mutex<AesKdfCacheInner>),
}

impl KdfCacheBackend {
    #[inline]
    fn lock(&self) -> MutexGuard<'_, AesKdfCacheInner> {
        match self {
            Self::Global => GLOBAL_KDF_CACHE_INNER.lock(),
            Self::Local(mutex) => mutex.lock(),
        }
    }
}

/// Thread-safe LRU cache pool for 7z AES-256 derived keys to prevent duplicate block derivations.
pub struct AesKdfCache {
    backend: KdfCacheBackend,
}

impl AesKdfCache {
    /// Default capacity for the global KDF cache.
    pub const DEFAULT_CAPACITY: usize = 64;

    /// Creates a new `AesKdfCache` with the specified entry capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            backend: KdfCacheBackend::Local(Mutex::new(AesKdfCacheInner::new(capacity))),
        }
    }

    pub(crate) const fn new_global() -> Self {
        Self {
            backend: KdfCacheBackend::Global,
        }
    }

    /// Returns the global singleton instance of `AesKdfCache`.
    pub fn global() -> &'static Self {
        static INSTANCE: AesKdfCache = AesKdfCache::new_global();
        &INSTANCE
    }

    /// Retrieves a cached 32-byte AES key wrapped in `Zeroizing` if available.
    pub fn get_zeroizing(
        &self,
        password_utf16le: &[u8],
        salt: &[u8],
        cycles_power: u32,
    ) -> Option<Zeroizing<[u8; 32]>> {
        self.backend.lock().get(password_utf16le, salt, cycles_power)
    }

    /// Retrieves a cached 32-byte AES key if available.
    pub fn get(&self, password_utf16le: &[u8], salt: &[u8], cycles_power: u8) -> Option<[u8; 32]> {
        self.get_zeroizing(password_utf16le, salt, cycles_power as u32)
            .map(|k| *k)
    }

    /// Inserts a newly derived 32-byte `Zeroizing` AES key into the cache.
    pub fn insert_zeroizing(
        &self,
        password_utf16le: &[u8],
        salt: &[u8],
        cycles_power: u32,
        key: Zeroizing<[u8; 32]>,
    ) {
        self.backend
            .lock()
            .insert(password_utf16le, salt, cycles_power, key);
    }

    /// Inserts a newly derived 32-byte AES key into the cache.
    pub fn insert(&self, password_utf16le: &[u8], salt: &[u8], cycles_power: u8, key: [u8; 32]) {
        self.insert_zeroizing(
            password_utf16le,
            salt,
            cycles_power as u32,
            Zeroizing::new(key),
        );
    }

    /// Clears and zeroizes all cached keys and credentials.
    pub fn clear(&self) {
        self.backend.lock().clear();
    }

    /// Returns the current number of cached entries.
    pub fn len(&self) -> usize {
        self.backend.lock().entries.len()
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

fn hash_7z_batched_rounds(
    hasher: &mut HardwareSha256,
    password_utf16le: &[u8],
    salt: &[u8],
    num_cycles: u64,
) {
    let prefix_len = salt.len() + password_utf16le.len();
    let step_len = prefix_len + 8;
    let mut iter_buf = [0u8; 1024];
    iter_buf[..salt.len()].copy_from_slice(salt);
    iter_buf[salt.len()..prefix_len].copy_from_slice(password_utf16le);

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
}

/// Computes the canonical 7z SHA-256 stretching rounds without consulting the cache.
pub fn compute_7z_sha256_rounds(
    password_utf16le: &[u8],
    salt: &[u8],
    cycles_power: u32,
) -> Zeroizing<[u8; 32]> {
    let num_cycles = 1u64.checked_shl(cycles_power).unwrap_or(0);
    let step_len = salt.len() + password_utf16le.len() + 8;
    let mut hasher = HardwareSha256::new();

    if step_len <= 1024 {
        hash_7z_batched_rounds(&mut hasher, password_utf16le, salt, num_cycles);
    } else {
        for i in 0..num_cycles {
            hasher.update(salt);
            hasher.update(password_utf16le);
            hasher.update(&i.to_le_bytes());
        }
    }

    Zeroizing::new(hasher.finalize())
}

/// Derives a 32-byte 7z AES key from UTF-16LE password bytes via the unified global cache.
pub fn derive_7z_key_utf16le(
    password_utf16le: &[u8],
    salt: &[u8],
    cycles_power: u32,
) -> Zeroizing<[u8; 32]> {
    if let Some(cached_key) =
        AesKdfCache::global().get_zeroizing(password_utf16le, salt, cycles_power)
    {
        return cached_key;
    }

    let key = compute_7z_sha256_rounds(password_utf16le, salt, cycles_power);
    AesKdfCache::global().insert_zeroizing(password_utf16le, salt, cycles_power, key.clone());
    key
}

/// Derives the 32-byte AES-256 key and 16-byte IV for 7z archives using hardware-accelerated SHA-256.
pub fn derive_7z_aes_key(
    password_utf16le: &[u8],
    salt: &[u8],
    cycles_power: u8,
    raw_iv: &[u8],
) -> Result<DerivedKey, SevenZKdfError> {
    let mut iv = [0u8; 16];
    let copy_iv_len = raw_iv.len().min(16);
    iv[..copy_iv_len].copy_from_slice(&raw_iv[..copy_iv_len]);

    if cycles_power == RAW_KEY_CYCLES_POWER {
        let mut key = [0u8; 32];
        let copy_key_len = password_utf16le.len().min(32);
        key[..copy_key_len].copy_from_slice(&password_utf16le[..copy_key_len]);
        return Ok(DerivedKey { key, iv });
    }

    if cycles_power > MAX_AES_CYCLES_POWER {
        return Err(SevenZKdfError::CryptoExhaustion);
    }

    let key = derive_7z_key_utf16le(password_utf16le, salt, cycles_power as u32);
    Ok(DerivedKey { key: *key, iv })
}

#[cfg(test)]
pub(crate) static GLOBAL_CACHE_TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arm64_sha256::{derive_7z_key_arm64, SevenZKeyCache};
    use crate::sha256::sha256_7z_kdf;

    #[test]
    fn test_unified_7z_kdf_cache_sharing_and_clear() {
        let _guard = GLOBAL_CACHE_TEST_LOCK.lock();
        AesKdfCache::global().clear();
        assert!(AesKdfCache::global().is_empty());
        assert!(SevenZKeyCache::global().is_empty());

        let password = "UnifiedCacheTestPassword!2026";
        let pw_utf16 = password_to_utf16le(password);
        let salt = [0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80];
        let iv = [0xAA; 16];
        let cycles_power = 8u8;

        let derived =
            derive_7z_aes_key(&pw_utf16, &salt, cycles_power, &iv).expect("derive_7z_aes_key");
        assert_eq!(
            SevenZKeyCache::global().get(password, &salt, cycles_power as u32),
            Some(derived.key)
        );
        assert_eq!(
            AesKdfCache::global().get(&pw_utf16, &salt, cycles_power),
            Some(derived.key)
        );

        let key_via_arm64 = derive_7z_key_arm64(password, &salt, cycles_power as u32);
        let key_via_sha256 = sha256_7z_kdf(password, &salt, cycles_power as u32);
        assert_eq!(key_via_arm64, derived.key);
        assert_eq!(key_via_sha256, derived.key);

        SevenZKeyCache::global().clear();
        assert_eq!(AesKdfCache::global().get(&pw_utf16, &salt, cycles_power), None);

        let key2 = sha256_7z_kdf("SecondUnifiedEntry", &salt, 6);
        let pw2_utf16 = password_to_utf16le("SecondUnifiedEntry");
        assert_eq!(AesKdfCache::global().get(&pw2_utf16, &salt, 6), Some(key2));

        AesKdfCache::global().clear();
        assert_eq!(SevenZKeyCache::global().get("SecondUnifiedEntry", &salt, 6), None);
    }
}
