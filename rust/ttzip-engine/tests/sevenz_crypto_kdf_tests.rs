// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit and integration test suite for 7z AES-256 SHA-256 KDF,
//! hardware acceleration, sensitive memory zeroization, and LRU cache concurrency.

use std::sync::Arc;
use std::thread;
use std::time::Instant;
use zeroize::Zeroize;

use sha2::{Digest, Sha256};
use ttzip_engine::crypto::sevenz_kdf::{
    derive_7z_aes_key, password_to_utf16le, AesKdfCache, DerivedKey, MAX_AES_CYCLES_POWER,
    RAW_KEY_CYCLES_POWER,
};
use ttzip_engine::sevenz::dag::SevenZError;

// ============================================================================
// 1. Official Standard Test Vectors & Output Precision Tests
// ============================================================================

#[test]
fn test_7z_kdf_official_vectors_precision() {
    AesKdfCache::global().clear();

    // Vector 1: Empty password, empty salt, cycles_power = 0 (1 cycle: hashes [0u8; 8])
    let pw_empty: [u8; 0] = [];
    let salt_empty: [u8; 0] = [];
    let raw_iv_empty: [u8; 0] = [];

    let derived = derive_7z_aes_key(&pw_empty, &salt_empty, 0, &raw_iv_empty)
        .expect("KDF derivation with 1 cycle must succeed");

    let mut expected_h = Sha256::new();
    expected_h.update(0u64.to_le_bytes());
    let expected_key_bytes: [u8; 32] = expected_h.finalize().into();

    assert_eq!(derived.key, expected_key_bytes);
    assert_eq!(derived.iv, [0u8; 16]);

    // Vector 2: Password "123" (UTF-16LE), Salt [0xAA, 0xBB], cycles_power = 1 (2 cycles), raw_iv 4 bytes
    let pw_123 = password_to_utf16le("123");
    let salt_123 = [0xAA, 0xBB];
    let raw_iv_4 = [0x10, 0x20, 0x30, 0x40];

    let derived_123 = derive_7z_aes_key(&pw_123, &salt_123, 1, &raw_iv_4)
        .expect("KDF derivation with 2 cycles must succeed");

    let mut exp_123 = Sha256::new();
    // Round 0
    exp_123.update(salt_123);
    exp_123.update(&pw_123);
    exp_123.update(0u64.to_le_bytes());
    // Round 1
    exp_123.update(salt_123);
    exp_123.update(&pw_123);
    exp_123.update(1u64.to_le_bytes());
    let exp_key_123: [u8; 32] = exp_123.finalize().into();

    assert_eq!(derived_123.key, exp_key_123);
    let mut expected_iv_4 = [0u8; 16];
    expected_iv_4[..4].copy_from_slice(&raw_iv_4);
    assert_eq!(derived_123.iv, expected_iv_4);

    // Vector 3: Standard 64-cycle vector (cycles_power = 6), 16-byte raw IV
    let pw_str = "TestPassword2026!#$";
    let pw_utf16 = password_to_utf16le(pw_str);
    let salt_64 = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
    let raw_iv_16 = [
        0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF,
        0x00,
    ];

    let derived_64 = derive_7z_aes_key(&pw_utf16, &salt_64, 6, &raw_iv_16)
        .expect("KDF derivation with 64 cycles must succeed");

    let mut exp_64 = Sha256::new();
    for cycle in 0..64u64 {
        exp_64.update(salt_64);
        exp_64.update(&pw_utf16);
        exp_64.update(cycle.to_le_bytes());
    }
    let exp_key_64: [u8; 32] = exp_64.finalize().into();

    assert_eq!(derived_64.key, exp_key_64);
    assert_eq!(derived_64.iv, raw_iv_16);
}

#[test]
fn test_7z_kdf_iv_truncation_and_padding() {
    let pw = password_to_utf16le("iv_test");
    let salt = [0x55; 8];

    // Raw IV shorter than 16 bytes: zero-padded
    let raw_iv_short = [0xAA, 0xBB, 0xCC];
    let derived_short = derive_7z_aes_key(&pw, &salt, 2, &raw_iv_short).expect("success");
    assert_eq!(
        derived_short.iv,
        [0xAA, 0xBB, 0xCC, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );

    // Raw IV longer than 16 bytes: clamped to 16 bytes
    let raw_iv_long = [0x33; 24];
    let derived_long = derive_7z_aes_key(&pw, &salt, 2, &raw_iv_long).expect("success");
    assert_eq!(derived_long.iv, [0x33; 16]);
}

// ============================================================================
// 2. DoS Exhaustion Defense (cycles_power > 24 Hard Upper Bound)
// ============================================================================

#[test]
fn test_cycles_power_exhaustion_interception() {
    let pw = password_to_utf16le("AttackPassword");
    let salt = [0xDE, 0xAD, 0xBE, 0xEF];
    let raw_iv = [0x01; 16];

    // Boundary: 24 is allowed
    assert_eq!(MAX_AES_CYCLES_POWER, 24);

    // Over limits: 25, 26, 30, 62 must all fail immediately with CryptoExhaustion
    let test_powers = [25, 26, 30, 32, 50, 62];
    for &power in &test_powers {
        let start = Instant::now();
        let result = derive_7z_aes_key(&pw, &salt, power, &raw_iv);
        let elapsed = start.elapsed();

        assert_eq!(
            result,
            Err(SevenZError::CryptoExhaustion),
            "cycles_power = {power} must return SevenZError::CryptoExhaustion"
        );
        assert!(
            elapsed.as_micros() < 50,
            "Exhaustion interception must take < 50µs (0 CPU waste), took {:?}",
            elapsed
        );
    }
}

// ============================================================================
// 3. 0x3F Raw Key Pass-Through Mode Tests
// ============================================================================

#[test]
fn test_0x3f_raw_key_passthrough_mode() {
    assert_eq!(RAW_KEY_CYCLES_POWER, 0x3F);

    let raw_iv = [0x77; 16];

    // Case A: Exact 32-byte raw key
    let raw_key_32 = [0x42u8; 32];
    let derived_32 = derive_7z_aes_key(&raw_key_32, &[], RAW_KEY_CYCLES_POWER, &raw_iv)
        .expect("Raw key 32 bytes must succeed");
    assert_eq!(derived_32.key, raw_key_32);
    assert_eq!(derived_32.iv, raw_iv);

    // Case B: Raw key shorter than 32 bytes (16 bytes, zero-padded)
    let raw_key_16 = [0x99u8; 16];
    let derived_16 = derive_7z_aes_key(&raw_key_16, &[], RAW_KEY_CYCLES_POWER, &raw_iv)
        .expect("Raw key 16 bytes must succeed");
    let mut expected_key_16 = [0u8; 32];
    expected_key_16[..16].copy_from_slice(&raw_key_16);
    assert_eq!(derived_16.key, expected_key_16);
    assert_eq!(derived_16.iv, raw_iv);

    // Case C: Raw key longer than 32 bytes (clamped to first 32 bytes)
    let raw_key_48 = [0x5Au8; 48];
    let derived_48 = derive_7z_aes_key(&raw_key_48, &[], RAW_KEY_CYCLES_POWER, &raw_iv)
        .expect("Raw key 48 bytes must succeed");
    assert_eq!(derived_48.key, [0x5Au8; 32]);
    assert_eq!(derived_48.iv, raw_iv);
}

// ============================================================================
// 4. Sensitive Memory Zeroization Tests
// ============================================================================

#[test]
fn test_derived_key_sensitive_zeroize() {
    let mut key = DerivedKey::new([0xAA; 32], [0xBB; 16]);
    assert_eq!(key.key(), &[0xAA; 32]);
    assert_eq!(key.iv(), &[0xBB; 16]);

    // Explicit Zeroize execution
    key.zeroize();
    assert_eq!(key.key, [0u8; 32]);
    assert_eq!(key.iv, [0u8; 16]);

    // Verify debug formatting redacts key material
    let secret_key = DerivedKey::new([0xFE; 32], [0x12; 16]);
    let debug_repr = format!("{:?}", secret_key);
    assert!(debug_repr.contains("[REDACTED 32-BYTES]"));
    assert!(!debug_repr.contains("254")); // 0xFE = 254
}

// ============================================================================
// 5. AesKdfCache Concurrency & LRU Tests
// ============================================================================

#[test]
fn test_aes_kdf_cache_lru_behavior() {
    let cache = AesKdfCache::new(2);
    let salt = [0x11, 0x22, 0x33, 0x44];
    let key1 = [0x01u8; 32];
    let key2 = [0x02u8; 32];
    let key3 = [0x03u8; 32];

    let pw1 = password_to_utf16le("p1");
    let pw2 = password_to_utf16le("p2");
    let pw3 = password_to_utf16le("p3");

    cache.insert(&pw1, &salt, 19, key1);
    cache.insert(&pw2, &salt, 19, key2);
    assert_eq!(cache.get(&pw1, &salt, 19), Some(key1));
    assert_eq!(cache.get(&pw2, &salt, 19), Some(key2));

    // Access pw1 to make pw2 the LRU candidate
    let _ = cache.get(&pw1, &salt, 19);
    cache.insert(&pw3, &salt, 19, key3);

    // pw1 and pw3 must remain; pw2 must have been evicted
    assert_eq!(cache.get(&pw1, &salt, 19), Some(key1));
    assert_eq!(cache.get(&pw3, &salt, 19), Some(key3));
    assert_eq!(cache.get(&pw2, &salt, 19), None);

    cache.clear();
    assert_eq!(cache.len(), 0);
    assert!(cache.is_empty());
}

#[test]
fn test_aes_kdf_cache_multithreaded_concurrency() {
    let cache = Arc::new(AesKdfCache::new(32));
    let num_threads = 16;
    let iterations_per_thread = 100;
    let mut handles = Vec::with_capacity(num_threads);

    for thread_idx in 0..num_threads {
        let cache_clone = Arc::clone(&cache);
        let handle = thread::spawn(move || {
            for iter in 0..iterations_per_thread {
                let pw_str = format!("thread_user_{}_{}", thread_idx % 4, iter % 8);
                let pw_bytes = password_to_utf16le(&pw_str);
                let salt = [(thread_idx as u8), (iter as u8), 0x55, 0xAA];
                let cycles_power = (iter % 3) as u8;

                // Concurrent read / write
                if let Some(cached_key) = cache_clone.get(&pw_bytes, &salt, cycles_power) {
                    assert_ne!(cached_key, [0u8; 32]);
                } else {
                    let mut key = [0u8; 32];
                    key[0] = thread_idx as u8;
                    key[1] = iter as u8;
                    cache_clone.insert(&pw_bytes, &salt, cycles_power, key);
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Worker thread must not panic");
    }

    assert!(cache.len() <= 32);
}

#[test]
fn test_524288_cycles_kdf_performance_and_cache_hit() {
    AesKdfCache::global().clear();

    let password = password_to_utf16le("SuperMasterKey2026!");
    let salt = [0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04];
    let cycles_power = 19; // 524,288 rounds (2^19)
    let raw_iv = [0x88; 16];

    // First computation (populates cache)
    let start = Instant::now();
    let derived1 = derive_7z_aes_key(&password, &salt, cycles_power, &raw_iv)
        .expect("524,288 cycles KDF must succeed");
    let elapsed = start.elapsed();

    assert_ne!(derived1.key, [0u8; 32]);
    assert_eq!(derived1.iv, raw_iv);
    assert!(
        elapsed.as_millis() <= 35,
        "524,288 cycles KDF took {:?}, exceeding 35ms threshold",
        elapsed
    );

    // Second computation (instant cache hit)
    let cache_start = Instant::now();
    let derived2 = derive_7z_aes_key(&password, &salt, cycles_power, &raw_iv)
        .expect("Cached lookup must succeed");
    let cache_elapsed = cache_start.elapsed();

    assert_eq!(derived1, derived2);
    assert!(
        cache_elapsed.as_micros() < 500,
        "Cache hit took {:?}, exceeding 500µs threshold",
        cache_elapsed
    );
}
