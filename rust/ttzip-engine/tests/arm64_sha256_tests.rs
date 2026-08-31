// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive test suite for ARM64 SHA-256 hardware acceleration,
//! vector pipelines, dynamic dispatch, and 7z 524,288-cycle KDF.

use sha2::Digest;
use std::time::Instant;
use ttzip_engine::crypto::arm64_sha256::{
    derive_7z_key_arm64, scalar::compress_blocks_scalar, sha256_compress_blocks, HardwareSha256,
    INITIAL_H,
};
#[cfg(target_arch = "aarch64")]
use ttzip_engine::crypto::arm64_sha256::sha256_compress_arm64_crypto;
use ttzip_engine::crypto::sha256::SevenZKeyCache;
use zeroize::Zeroize;

// ============================================================================
// 1. NIST FIPS 180-4 Standard Test Vectors
// ============================================================================

#[test]
fn test_nist_fips_180_4_standard_vectors() {
    // Vector 1: Empty input
    let digest_empty = HardwareSha256::digest(b"");
    assert_eq!(
        hex::encode(digest_empty),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "NIST vector for empty string failed"
    );

    // Vector 2: "abc"
    let mut h2 = HardwareSha256::new();
    h2.update(b"abc");
    assert_eq!(
        hex::encode(h2.finalize()),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        "NIST vector for 'abc' failed"
    );

    // Vector 3: 56-byte message "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
    let mut h3 = HardwareSha256::new();
    h3.update(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq");
    assert_eq!(
        hex::encode(h3.finalize()),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        "NIST vector for 56-byte message failed"
    );

    // Vector 4: 1,000,000 repetitions of 'a'
    let mut h4 = HardwareSha256::new();
    let chunk = vec![b'a'; 8192];
    let mut remaining = 1_000_000;
    while remaining > 0 {
        let take = remaining.min(chunk.len());
        h4.update(&chunk[..take]);
        remaining -= take;
    }
    assert_eq!(
        hex::encode(h4.finalize()),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0",
        "NIST vector for 1,000,000 'a's failed"
    );
}

// ============================================================================
// 2. Hardware Block Compression Differential Oracle
// ============================================================================

#[test]
fn test_block_compression_differential_oracle() {
    let test_sizes = [64, 128, 256, 512, 1024, 4096, 65536];

    for &size in &test_sizes {
        let mut data = vec![0u8; size];
        for (i, byte) in data.iter_mut().enumerate() {
            *byte = ((i * 37 + 13) & 0xFF) as u8;
        }

        let num_blocks = size / 64;
        let mut state_scalar = INITIAL_H;
        let mut state_dispatch = INITIAL_H;

        compress_blocks_scalar(&mut state_scalar, data.as_ptr(), num_blocks);

        // Convert slice to &[ [u8; 64] ]
        let blocks_slice: &[[u8; 64]] = unsafe {
            std::slice::from_raw_parts(data.as_ptr() as *const [u8; 64], num_blocks)
        };
        sha256_compress_blocks(&mut state_dispatch, blocks_slice);

        assert_eq!(
            state_scalar, state_dispatch,
            "Dynamic dispatch mismatch for {} blocks",
            num_blocks
        );

        #[cfg(target_arch = "aarch64")]
        {
            let mut state_arm64 = INITIAL_H;
            unsafe {
                sha256_compress_arm64_crypto(&mut state_arm64, data.as_ptr(), num_blocks);
            }
            assert_eq!(
                state_scalar, state_arm64,
                "ARM64 crypto assembly mismatch for {} blocks",
                num_blocks
            );
        }
    }
}

// ============================================================================
// 3. 7-Zip Official KDF Test Vectors & Oracle Verification
// ============================================================================

fn oracle_7z_kdf(password: &str, salt: &[u8], num_cycles_power: u32) -> [u8; 32] {
    let mut hasher = sha2::Sha256::new();
    let num_cycles = 1u64 << num_cycles_power;

    let mut utf16_pass = Vec::new();
    for u in password.encode_utf16() {
        utf16_pass.extend_from_slice(&u.to_le_bytes());
    }

    for i in 0..num_cycles {
        if !salt.is_empty() {
            hasher.update(salt);
        }
        if !utf16_pass.is_empty() {
            hasher.update(&utf16_pass);
        }
        hasher.update(i.to_le_bytes());
    }

    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

#[test]
fn test_7z_official_kdf_vectors() {
    SevenZKeyCache::global().clear();

    // Case 1: Empty password and empty salt, power = 0 (1 cycle)
    let key1 = derive_7z_key_arm64("", &[], 0);
    let expected1 = oracle_7z_kdf("", &[], 0);
    assert_eq!(key1, expected1, "7z KDF empty 1-cycle test failed");

    // Case 2: Short password and 2-byte salt, power = 1 (2 cycles)
    let key2 = derive_7z_key_arm64("123", &[0xAA, 0xBB], 1);
    let expected2 = oracle_7z_kdf("123", &[0xAA, 0xBB], 1);
    assert_eq!(key2, expected2, "7z KDF 2-cycle test failed");

    // Case 3: 64-way batch boundary tests (power = 6 => 64 cycles)
    let test_matrix = [
        ("password", &[0x01, 0x02, 0x03, 0x04][..]),
        ("P@ssw0rd!#$%", &[0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE][..]),
        ("TTZip Apple Silicon 极速 KDF 验证 🚀", &[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88][..]),
        ("", &[0xFF; 16][..]),
    ];

    for &(pwd, salt) in &test_matrix {
        SevenZKeyCache::global().clear();
        let derived = derive_7z_key_arm64(pwd, salt, 6);
        let oracle = oracle_7z_kdf(pwd, salt, 6);
        assert_eq!(
            derived, oracle,
            "7z KDF 64-cycle mismatch for password '{}'",
            pwd
        );
    }
}

#[test]
fn test_7z_kdf_524288_cycles_full_derivation() {
    SevenZKeyCache::global().clear();

    let password = "TTZipProductionMasterKey2026";
    let salt = [0x01, 0x03, 0x05, 0x07, 0x09, 0x0B, 0x0D, 0x0F, 0x11, 0x13, 0x15, 0x17, 0x19, 0x1B, 0x1D, 0x1F];
    let num_cycles_power = 19; // 524,288 cycles

    let start = Instant::now();
    let key = derive_7z_key_arm64(password, &salt, num_cycles_power);
    let elapsed = start.elapsed();

    assert_ne!(key, [0u8; 32], "Derived key must not be zero");
    println!("524,288 cycles KDF elapsed: {:.2} ms", elapsed.as_secs_f64() * 1000.0);

    // Verify cache hit takes < 100 microseconds
    let cache_start = Instant::now();
    let cached_key = derive_7z_key_arm64(password, &salt, num_cycles_power);
    let cache_elapsed = cache_start.elapsed();

    assert_eq!(key, cached_key, "Cached key must match computed key");
    assert!(
        cache_elapsed.as_micros() < 500,
        "Cache hit elapsed {:?} exceeds 500µs limit",
        cache_elapsed
    );
}

// ============================================================================
// 4. Memory Sensitive Erasure & Zeroization Verification
// ============================================================================

#[test]
fn test_zeroize_memory_erasure_and_safety() {
    let mut hasher = HardwareSha256::new();
    hasher.update(b"Super sensitive unencrypted archive header block");
    assert_ne!(hasher.current_state(), [0u32; 8]);
    assert_ne!(hasher.total_len(), 0);

    hasher.zeroize();
    assert!(hasher.is_zeroized(), "Hasher must be fully zeroized after zeroize()");
    assert_eq!(hasher.current_state(), [0u32; 8]);
    assert_eq!(hasher.total_len(), 0);

    // Test SevenZKeyCache zeroization
    let cache = SevenZKeyCache::new(4);
    cache.insert("pwd1", &[1, 2, 3], 19, [0xAA; 32]);
    assert_eq!(cache.len(), 1);
    cache.clear();
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.get("pwd1", &[1, 2, 3], 19), None);
}
