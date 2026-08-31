// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Cross-Language Safe Export Layer for Cryptographic Primitives and Hashes.
//!
//! Provides typed, memory-sanitized, and Swift 6 Sendable bindings for 11 algorithms:
//! - Adler-32 Checksum (UDOT NEON / Slicing-by-4)
//! - CRC-32 (IEEE 802.3 ARM64 PMULL / Slicing-by-8 + CRC combine)
//! - CRC-64 (ECMA-182)
//! - XXH3-64 (xxHash3 64-bit)
//! - XXH3-128 (xxHash3 128-bit)
//! - BLAKE3 (Keyed & Unkeyed tree hashing)
//! - WinZip AES-256 (PBKDF2-HMAC-SHA1 + CTR + HMAC-SHA1-10)
//! - 7z AES-256 (Multi-round SHA-256 KDF + CBC)
//! - Traditional ZipCrypto (PKWARE 3-key cipher)
//! - TTZip Vault AES-256-GCM (NIST SP 800-38D AEAD)
//! - TTZip Vault ChaCha20-Poly1305 (RFC 8439 IETF AEAD)
//!
//! Enforces Zeroize memory scrubbing and compiler fences on all key material.

use super::types::TTZipError;
use std::sync::atomic::{compiler_fence, Ordering};
use zeroize::Zeroize;

/// Structure representing a 128-bit XXH3 hash digest.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIXxh3128Digest {
    pub low: u64,
    pub high: u64,
}

/// Derived WinZip AES-256 key material.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIWinZipKeys {
    pub enc_key: Vec<u8>,
    pub auth_key: Vec<u8>,
    pub pvv: Vec<u8>,
}

/// Authenticated Encryption with Associated Data (AEAD) encryption result.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIAeadResult {
    pub ciphertext: Vec<u8>,
    pub tag: Vec<u8>,
}

// ============================================================================
// 1. Adler-32 Checksum Exports
// ============================================================================

/// Computes Adler-32 checksum starting from standard initial state 1.
#[uniffi::export]
pub fn uniffi_adler32(data: Vec<u8>) -> u32 {
    crate::crypto::adler32::adler32(&data)
}

/// Computes rolling Adler-32 checksum starting from an existing accumulator.
#[uniffi::export]
pub fn uniffi_adler32_rolling(initial: u32, data: Vec<u8>) -> u32 {
    crate::crypto::adler32::adler32_fast(initial, &data)
}

// ============================================================================
// 2. CRC-32 (IEEE 802.3) Checksum Exports
// ============================================================================

/// Computes CRC-32 (IEEE 802.3) checksum.
#[uniffi::export]
pub fn uniffi_crc32(data: Vec<u8>) -> u32 {
    crate::crypto::crc32::crc32(&data)
}

/// Computes rolling CRC-32 checksum starting from an existing accumulator.
#[uniffi::export]
pub fn uniffi_crc32_rolling(initial: u32, data: Vec<u8>) -> u32 {
    crate::crypto::crc32::crc32_fast(initial, &data)
}

/// Combines two separate CRC-32 checksums in O(log N) time using GF(2) matrix multiplication.
#[uniffi::export]
pub fn uniffi_crc32_combine(crc1: u32, crc2: u32, len2: u64) -> u32 {
    crate::crypto::crc32::crc32_combine(crc1, crc2, len2)
}

// ============================================================================
// 3. CRC-64 (ECMA-182) Checksum Exports
// ============================================================================

/// Computes CRC-64 (ECMA-182) checksum with optional seed (defaults to 0).
#[uniffi::export]
pub fn uniffi_crc64(data: Vec<u8>, seed: Option<u64>) -> u64 {
    match seed {
        Some(s) => crate::crypto::crc64::crc64(&data, s),
        None => crate::crypto::crc64::crc64_fast(&data),
    }
}

// ============================================================================
// 4. XXH3-64 & XXH3-128 Hash Exports
// ============================================================================

/// Computes XXH3 64-bit hash with optional 64-bit seed.
#[uniffi::export]
pub fn uniffi_xxh3_64(data: Vec<u8>, seed: Option<u64>) -> u64 {
    match seed {
        Some(s) => crate::crypto::xxh3::Xxh3_64::with_seed(s).finalize_with_data(&data),
        None => crate::crypto::xxh3::xxh3_64(&data),
    }
}

/// Computes XXH3 128-bit hash returning raw 16-byte array.
#[uniffi::export]
pub fn uniffi_xxh3_128(data: Vec<u8>, seed: Option<u64>) -> Vec<u8> {
    let (low, high) = match seed {
        Some(s) => crate::crypto::xxh3::Xxh3_128::with_seed(s).finalize_with_data(&data),
        None => crate::crypto::xxh3::xxh3_128(&data),
    };
    let mut out = vec![0u8; 16];
    out[..8].copy_from_slice(&low.to_le_bytes());
    out[8..].copy_from_slice(&high.to_le_bytes());
    out
}

/// Computes XXH3 128-bit hash returning structured record (low, high).
#[uniffi::export]
pub fn uniffi_xxh3_128_digest(data: Vec<u8>, seed: Option<u64>) -> UniFFIXxh3128Digest {
    let (low, high) = match seed {
        Some(s) => crate::crypto::xxh3::Xxh3_128::with_seed(s).finalize_with_data(&data),
        None => crate::crypto::xxh3::xxh3_128(&data),
    };
    UniFFIXxh3128Digest { low, high }
}

trait Xxh3DataExt {
    fn finalize_with_data(self, data: &[u8]) -> Self::Output;
    type Output;
}

impl Xxh3DataExt for crate::crypto::xxh3::Xxh3_64 {
    type Output = u64;
    fn finalize_with_data(mut self, data: &[u8]) -> u64 {
        self.update(data);
        self.finalize()
    }
}

impl Xxh3DataExt for crate::crypto::xxh3::Xxh3_128 {
    type Output = (u64, u64);
    fn finalize_with_data(mut self, data: &[u8]) -> (u64, u64) {
        self.update(data);
        self.finalize()
    }
}

// ============================================================================
// 5. BLAKE3 Cryptographic Hash Exports
// ============================================================================

/// Computes unkeyed 256-bit BLAKE3 hash returning 32-byte digest.
#[uniffi::export]
pub fn uniffi_blake3(data: Vec<u8>) -> Vec<u8> {
    crate::crypto::blake3::blake3(&data).to_vec()
}

/// Computes keyed 256-bit BLAKE3 hash with 32-byte secret key (scrubs key upon return).
#[uniffi::export]
pub fn uniffi_blake3_keyed(data: Vec<u8>, mut key: Vec<u8>) -> Result<Vec<u8>, TTZipError> {
    if key.len() != 32 {
        key.zeroize();
        compiler_fence(Ordering::SeqCst);
        return Err(TTZipError::EngineError { code: -1 });
    }

    let mut key_arr = [0u8; 32];
    key_arr.copy_from_slice(&key);
    key.zeroize();
    compiler_fence(Ordering::SeqCst);

    let mut hasher = crate::crypto::blake3::Blake3::new_keyed(&key_arr);
    key_arr.zeroize();
    compiler_fence(Ordering::SeqCst);

    hasher.update(&data);
    let digest = hasher.finalize();
    Ok(digest.to_vec())
}

// ============================================================================
// 6. WinZip AES-256 Encryption & Key Derivation Exports
// ============================================================================

/// Derives WinZip AES-256 keys (1000 rounds PBKDF2-HMAC-SHA1).
#[uniffi::export]
pub fn uniffi_winzip_aes256_derive_keys(
    password: String,
    salt: Vec<u8>,
) -> Result<UniFFIWinZipKeys, TTZipError> {
    if salt.len() != 16 {
        return Err(TTZipError::EngineError { code: -1 });
    }
    let mut salt_arr = [0u8; 16];
    salt_arr.copy_from_slice(&salt);

    let keys = crate::crypto::sha1::winzip_aes256_derive_keys(&password, &salt_arr)
        .map_err(|s| TTZipError::EngineError { code: s as i32 })?;

    Ok(UniFFIWinZipKeys {
        enc_key: keys.enc_key.to_vec(),
        auth_key: keys.auth_key.to_vec(),
        pvv: keys.pvv.to_vec(),
    })
}

/// Encrypts plaintext into a complete WinZip AES-256 authenticated payload.
/// Format: `[16-byte salt] || [2-byte PVV] || [Ciphertext] || [10-byte HMAC-SHA1]`
#[uniffi::export]
pub fn uniffi_winzip_aes256_encrypt(
    password: String,
    salt: Vec<u8>,
    plaintext: Vec<u8>,
) -> Result<Vec<u8>, TTZipError> {
    if salt.len() != 16 {
        return Err(TTZipError::EngineError { code: -1 });
    }
    let mut salt_arr = [0u8; 16];
    salt_arr.copy_from_slice(&salt);

    let mut out_payload = Vec::new();
    crate::crypto::sha1::winzip_aes256_encrypt_and_tag(
        &password,
        &salt_arr,
        &plaintext,
        &mut out_payload,
    )
    .map_err(|s| TTZipError::EngineError { code: s as i32 })?;

    Ok(out_payload)
}

/// Decrypts and authenticates a complete WinZip AES-256 payload.
#[uniffi::export]
pub fn uniffi_winzip_aes256_decrypt(
    password: String,
    enc_payload: Vec<u8>,
) -> Result<Vec<u8>, TTZipError> {
    if enc_payload.len() < 28 {
        return Err(TTZipError::CorruptHeader {
            details: "WinZip AES payload too short".to_string(),
            offset: 0,
        });
    }

    let cipher_len = enc_payload.len() - 28;
    let mut dst = vec![0u8; cipher_len];

    let written = crate::crypto::sha1::winzip_aes256_decrypt_and_verify(
        &password,
        &enc_payload,
        &mut dst,
    )
    .map_err(|s| match s {
        crate::types::TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
        crate::types::TTZipStatus::ErrCorruptHeader => TTZipError::CorruptHeader {
            details: "Corrupted WinZip payload header".to_string(),
            offset: 0,
        },
        other => TTZipError::EngineError { code: other as i32 },
    })?;

    dst.truncate(written);
    Ok(dst)
}

// ============================================================================
// 7. 7z AES-256 Encryption & KDF Exports
// ============================================================================

/// Derives 7z 256-bit AES key using multi-round SHA-256 KDF (up to 2^19 cycles).
#[uniffi::export]
pub fn uniffi_7z_aes256_derive_key(
    password: String,
    salt: Vec<u8>,
    num_cycles_power: u32,
) -> Vec<u8> {
    let key = crate::crypto::arm64_sha256::derive_7z_key_arm64(&password, &salt, num_cycles_power);
    key.to_vec()
}

/// Encrypts plaintext with AES-256-CBC with PKCS#7 block padding.
#[uniffi::export]
pub fn uniffi_7z_aes256_encrypt(
    mut key: Vec<u8>,
    iv: Vec<u8>,
    plaintext: Vec<u8>,
) -> Result<Vec<u8>, TTZipError> {
    if key.len() != 32 || iv.len() != 16 {
        key.zeroize();
        compiler_fence(Ordering::SeqCst);
        return Err(TTZipError::EngineError { code: -1 });
    }

    let mut key_arr = [0u8; 32];
    key_arr.copy_from_slice(&key);
    key.zeroize();

    let mut iv_arr = [0u8; 16];
    iv_arr.copy_from_slice(&iv);

    let pad_len = 16 - (plaintext.len() % 16);
    let mut padded = Vec::with_capacity(plaintext.len() + pad_len);
    padded.extend_from_slice(&plaintext);
    padded.resize(plaintext.len() + pad_len, pad_len as u8);

    let mut cipher = vec![0u8; padded.len()];
    let res = crate::crypto::aes256::aes256_cbc_encrypt(&key_arr, &iv_arr, &padded, &mut cipher);

    key_arr.zeroize();
    padded.zeroize();
    compiler_fence(Ordering::SeqCst);

    res.map_err(|_| TTZipError::EngineError { code: -2 })?;
    Ok(cipher)
}

/// Decrypts AES-256-CBC ciphertext and validates PKCS#7 block padding.
#[uniffi::export]
pub fn uniffi_7z_aes256_decrypt(
    mut key: Vec<u8>,
    iv: Vec<u8>,
    ciphertext: Vec<u8>,
) -> Result<Vec<u8>, TTZipError> {
    if key.len() != 32 || iv.len() != 16 || !ciphertext.len().is_multiple_of(16) || ciphertext.is_empty() {
        key.zeroize();
        compiler_fence(Ordering::SeqCst);
        return Err(TTZipError::EngineError { code: -1 });
    }

    let mut key_arr = [0u8; 32];
    key_arr.copy_from_slice(&key);
    key.zeroize();

    let mut iv_arr = [0u8; 16];
    iv_arr.copy_from_slice(&iv);

    let mut plain_padded = vec![0u8; ciphertext.len()];
    let res = crate::crypto::aes256::aes256_cbc_decrypt(&key_arr, &iv_arr, &ciphertext, &mut plain_padded);

    key_arr.zeroize();
    compiler_fence(Ordering::SeqCst);

    if res.is_err() {
        plain_padded.zeroize();
        return Err(TTZipError::EngineError { code: -2 });
    }

    let pad_byte = plain_padded[plain_padded.len() - 1];
    let pad_len = pad_byte as usize;
    if pad_len == 0 || pad_len > 16 || pad_len > plain_padded.len() {
        plain_padded.zeroize();
        return Err(TTZipError::InvalidPassword);
    }
    for &b in &plain_padded[plain_padded.len() - pad_len..] {
        if b != pad_byte {
            plain_padded.zeroize();
            return Err(TTZipError::InvalidPassword);
        }
    }

    plain_padded.truncate(plain_padded.len() - pad_len);
    Ok(plain_padded)
}

// ============================================================================
// 8. Traditional ZipCrypto (PKWARE) Exports
// ============================================================================

/// Encrypts plaintext buffer in-place using traditional PKZIP 3-key stream cipher.
#[uniffi::export]
pub fn uniffi_zipcrypto_encrypt(mut password: Vec<u8>, plaintext: Vec<u8>) -> Vec<u8> {
    let mut out = plaintext;
    crate::crypto::zipcrypto::zipcrypto_encrypt_slice(&password, &mut out);
    password.zeroize();
    compiler_fence(Ordering::SeqCst);
    out
}

/// Decrypts ciphertext buffer in-place using traditional PKZIP 3-key stream cipher.
#[uniffi::export]
pub fn uniffi_zipcrypto_decrypt(mut password: Vec<u8>, ciphertext: Vec<u8>) -> Vec<u8> {
    let mut out = ciphertext;
    crate::crypto::zipcrypto::zipcrypto_decrypt_slice(&password, &mut out);
    password.zeroize();
    compiler_fence(Ordering::SeqCst);
    out
}

// ============================================================================
// 9. TTZip Vault AES-256-GCM AEAD Exports
// ============================================================================

/// Authenticated encryption with AES-256-GCM (NIST SP 800-38D).
#[uniffi::export]
pub fn uniffi_vault_aes_gcm_encrypt(
    mut key: Vec<u8>,
    iv: Vec<u8>,
    plaintext: Vec<u8>,
    aad: Vec<u8>,
) -> Result<UniFFIAeadResult, TTZipError> {
    if key.len() != 32 || iv.len() != 12 {
        key.zeroize();
        compiler_fence(Ordering::SeqCst);
        return Err(TTZipError::EngineError { code: -1 });
    }

    let mut key_arr = [0u8; 32];
    key_arr.copy_from_slice(&key);
    key.zeroize();

    let mut iv_arr = [0u8; 12];
    iv_arr.copy_from_slice(&iv);

    let mut ciphertext = vec![0u8; plaintext.len()];
    let mut tag = [0u8; 16];

    let res = crate::crypto::vault::aes256_gcm_encrypt(
        &key_arr,
        &iv_arr,
        &plaintext,
        &aad,
        &mut ciphertext,
        &mut tag,
    );

    key_arr.zeroize();
    compiler_fence(Ordering::SeqCst);

    res.map_err(|s| TTZipError::EngineError { code: s as i32 })?;

    Ok(UniFFIAeadResult {
        ciphertext,
        tag: tag.to_vec(),
    })
}

/// Authenticated decryption with AES-256-GCM (NIST SP 800-38D).
#[uniffi::export]
pub fn uniffi_vault_aes_gcm_decrypt(
    mut key: Vec<u8>,
    iv: Vec<u8>,
    ciphertext: Vec<u8>,
    aad: Vec<u8>,
    tag: Vec<u8>,
) -> Result<Vec<u8>, TTZipError> {
    if key.len() != 32 || iv.len() != 12 || tag.len() != 16 {
        key.zeroize();
        compiler_fence(Ordering::SeqCst);
        return Err(TTZipError::EngineError { code: -1 });
    }

    let mut key_arr = [0u8; 32];
    key_arr.copy_from_slice(&key);
    key.zeroize();

    let mut iv_arr = [0u8; 12];
    iv_arr.copy_from_slice(&iv);

    let mut tag_arr = [0u8; 16];
    tag_arr.copy_from_slice(&tag);

    let mut plaintext = vec![0u8; ciphertext.len()];

    let res = crate::crypto::vault::aes256_gcm_decrypt(
        &key_arr,
        &iv_arr,
        &ciphertext,
        &aad,
        &tag_arr,
        &mut plaintext,
    );

    key_arr.zeroize();
    tag_arr.zeroize();
    compiler_fence(Ordering::SeqCst);

    res.map_err(|s| match s {
        crate::types::TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
        other => TTZipError::EngineError { code: other as i32 },
    })?;

    Ok(plaintext)
}

// ============================================================================
// 10. TTZip Vault ChaCha20-Poly1305 AEAD Exports
// ============================================================================

/// Authenticated encryption with ChaCha20-Poly1305 (RFC 8439).
#[uniffi::export]
pub fn uniffi_vault_chacha20_poly1305_encrypt(
    mut key: Vec<u8>,
    nonce: Vec<u8>,
    plaintext: Vec<u8>,
    aad: Vec<u8>,
) -> Result<UniFFIAeadResult, TTZipError> {
    if key.len() != 32 || nonce.len() != 12 {
        key.zeroize();
        compiler_fence(Ordering::SeqCst);
        return Err(TTZipError::EngineError { code: -1 });
    }

    let mut key_arr = [0u8; 32];
    key_arr.copy_from_slice(&key);
    key.zeroize();

    let mut nonce_arr = [0u8; 12];
    nonce_arr.copy_from_slice(&nonce);

    let mut ciphertext = vec![0u8; plaintext.len()];
    let mut tag = [0u8; 16];

    let res = crate::crypto::chacha20poly1305::chacha20_poly1305_encrypt(
        &key_arr,
        &nonce_arr,
        &plaintext,
        &aad,
        &mut ciphertext,
        &mut tag,
    );

    key_arr.zeroize();
    compiler_fence(Ordering::SeqCst);

    res.map_err(|s| TTZipError::EngineError { code: s as i32 })?;

    Ok(UniFFIAeadResult {
        ciphertext,
        tag: tag.to_vec(),
    })
}

/// Authenticated decryption with ChaCha20-Poly1305 (RFC 8439).
#[uniffi::export]
pub fn uniffi_vault_chacha20_poly1305_decrypt(
    mut key: Vec<u8>,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
    aad: Vec<u8>,
    tag: Vec<u8>,
) -> Result<Vec<u8>, TTZipError> {
    if key.len() != 32 || nonce.len() != 12 || tag.len() != 16 {
        key.zeroize();
        compiler_fence(Ordering::SeqCst);
        return Err(TTZipError::EngineError { code: -1 });
    }

    let mut key_arr = [0u8; 32];
    key_arr.copy_from_slice(&key);
    key.zeroize();

    let mut nonce_arr = [0u8; 12];
    nonce_arr.copy_from_slice(&nonce);

    let mut tag_arr = [0u8; 16];
    tag_arr.copy_from_slice(&tag);

    let mut plaintext = vec![0u8; ciphertext.len()];

    let res = crate::crypto::chacha20poly1305::chacha20_poly1305_decrypt(
        &key_arr,
        &nonce_arr,
        &ciphertext,
        &aad,
        &tag_arr,
        &mut plaintext,
    );

    key_arr.zeroize();
    tag_arr.zeroize();
    compiler_fence(Ordering::SeqCst);

    res.map_err(|s| match s {
        crate::types::TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
        other => TTZipError::EngineError { code: other as i32 },
    })?;

    Ok(plaintext)
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adler32_and_crc32_and_crc64() {
        let text = b"123456789".to_vec();
        assert_eq!(uniffi_adler32(text.clone()), 0x091E01DE);
        assert_eq!(uniffi_crc32(text.clone()), 0xCBF43926);
        assert_eq!(uniffi_crc64(text.clone(), None), 13288015728624077471u64);

        // CRC combine test
        let part1 = b"12345".to_vec();
        let part2 = b"6789".to_vec();
        let c1 = uniffi_crc32(part1);
        let c2 = uniffi_crc32(part2);
        let combined = uniffi_crc32_combine(c1, c2, 4);
        assert_eq!(combined, 0xCBF43926);
    }

    #[test]
    fn test_xxh3_and_blake3_exports() {
        let text = b"TTZip Engine Cross-Language Hashes 2026".to_vec();

        let h64 = uniffi_xxh3_64(text.clone(), None);
        assert_ne!(h64, 0);

        let h128 = uniffi_xxh3_128(text.clone(), None);
        assert_eq!(h128.len(), 16);

        let digest = uniffi_xxh3_128_digest(text.clone(), None);
        assert_ne!(digest.low, 0);

        let b3 = uniffi_blake3(text.clone());
        assert_eq!(b3.len(), 32);

        let key = vec![0x42u8; 32];
        let b3_keyed = uniffi_blake3_keyed(text.clone(), key).expect("keyed blake3");
        assert_eq!(b3_keyed.len(), 32);
        assert_ne!(b3, b3_keyed);
    }

    #[test]
    fn test_winzip_aes256_roundtrip() {
        let password = "SuperSecretWinZipPassword2026!".to_string();
        let salt = vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00];
        let plaintext = b"Confidential payload protected by WinZip AES-256 standard".to_vec();

        let keys = uniffi_winzip_aes256_derive_keys(password.clone(), salt.clone()).expect("derive keys");
        assert_eq!(keys.enc_key.len(), 32);
        assert_eq!(keys.auth_key.len(), 32);
        assert_eq!(keys.pvv.len(), 2);

        let enc_payload = uniffi_winzip_aes256_encrypt(password.clone(), salt, plaintext.clone()).expect("winzip encrypt");
        assert!(enc_payload.len() > plaintext.len());

        let dec = uniffi_winzip_aes256_decrypt(password.clone(), enc_payload).expect("winzip decrypt");
        assert_eq!(dec, plaintext);

        // Test wrong password
        let bad_res = uniffi_winzip_aes256_decrypt("WrongPass".to_string(), plaintext);
        assert!(bad_res.is_err());
    }

    #[test]
    fn test_7z_aes256_and_zipcrypto_roundtrip() {
        // 7z KDF & CBC
        let password = "ArchivePassword7z".to_string();
        let salt = vec![0x01, 0x02, 0x03, 0x04];
        let key = uniffi_7z_aes256_derive_key(password, salt, 2);
        assert_eq!(key.len(), 32);

        let iv = vec![0x99u8; 16];
        let plaintext = b"Arbitrary plaintext length for 7z AES-256 CBC verification test!".to_vec();

        let cipher = uniffi_7z_aes256_encrypt(key.clone(), iv.clone(), plaintext.clone()).expect("7z encrypt");
        assert_eq!(cipher.len() % 16, 0);

        let decrypted = uniffi_7z_aes256_decrypt(key, iv, cipher).expect("7z decrypt");
        assert_eq!(decrypted, plaintext);

        // ZipCrypto
        let zpass = b"ZipPass".to_vec();
        let zplain = b"Traditional ZipCrypto Payload 12345".to_vec();
        let zenc = uniffi_zipcrypto_encrypt(zpass.clone(), zplain.clone());
        assert_ne!(zenc, zplain);
        let zdec = uniffi_zipcrypto_decrypt(zpass, zenc);
        assert_eq!(zdec, zplain);
    }

    #[test]
    fn test_vault_aead_gcm_and_chacha20_roundtrip() {
        let key = vec![0x33u8; 32];
        let iv = vec![0x77u8; 12];
        let plaintext = b"TTZip Vault Authenticated Confidential Record 2026".to_vec();
        let aad = b"HeaderMetadataV1".to_vec();

        // AES-GCM
        let gcm_res = uniffi_vault_aes_gcm_encrypt(key.clone(), iv.clone(), plaintext.clone(), aad.clone()).expect("gcm enc");
        assert_eq!(gcm_res.ciphertext.len(), plaintext.len());
        assert_eq!(gcm_res.tag.len(), 16);

        let gcm_plain = uniffi_vault_aes_gcm_decrypt(key.clone(), iv.clone(), gcm_res.ciphertext.clone(), aad.clone(), gcm_res.tag.clone()).expect("gcm dec");
        assert_eq!(gcm_plain, plaintext);

        // ChaCha20-Poly1305
        let chacha_res = uniffi_vault_chacha20_poly1305_encrypt(key.clone(), iv.clone(), plaintext.clone(), aad.clone()).expect("chacha enc");
        assert_eq!(chacha_res.ciphertext.len(), plaintext.len());
        assert_eq!(chacha_res.tag.len(), 16);

        let chacha_plain = uniffi_vault_chacha20_poly1305_decrypt(key, iv, chacha_res.ciphertext.clone(), aad, chacha_res.tag).expect("chacha dec");
        assert_eq!(chacha_plain, plaintext);
    }
}
