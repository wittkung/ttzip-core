// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Integration and verification suite for Mozilla UniFFI 0.28 Crypto & Hash API exports.
//!
//! Covers hardware-accelerated cryptographic primitives, hashing, and cipher roundtrips:
//! Adler32, CRC32, CRC64, XXH3 (64/128), BLAKE3, WinZip AES-256, 7z AES-256, ZipCrypto,
//! Vault AES-GCM, Vault ChaCha20-Poly1305, MD5, SHA1, SHA256, AES-256-CTR, and AES-256-CBC.

use ttzip_engine::uniffi_api::crypto::*;

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

#[test]
fn test_md5_sha1_sha256_exports() {
    let text = b"TTZip Cryptographic Hash Verification 2026".to_vec();
    let m = uniffi_md5(text.clone());
    assert_eq!(m.len(), 16);
    assert_eq!(m, ttzip_engine::crypto::md5::md5(&text).to_vec());

    let s1 = uniffi_sha1(text.clone());
    assert_eq!(s1.len(), 20);
    assert_eq!(s1, ttzip_engine::crypto::sha1::sha1(&text).to_vec());

    let s256 = uniffi_sha256(text.clone());
    assert_eq!(s256.len(), 32);
    assert_eq!(s256, ttzip_engine::crypto::sha256::HardwareSha256::digest(&text).to_vec());
}

#[test]
fn test_aes256_ctr_and_cbc_raw_roundtrip() {
    let key = vec![0x42u8; 32];
    let iv = vec![0x24u8; 16];
    let plaintext = b"Block Aligned 16Bytes x2 Block!!".to_vec();

    // AES-256-CTR
    let ctr_enc = uniffi_aes256_ctr(key.clone(), 100, plaintext.clone()).expect("ctr enc");
    assert_eq!(ctr_enc.len(), plaintext.len());
    assert_ne!(ctr_enc, plaintext);
    let ctr_dec = uniffi_aes256_ctr(key.clone(), 100, ctr_enc).expect("ctr dec");
    assert_eq!(ctr_dec, plaintext);

    // AES-256-CBC Raw
    let cbc_enc = uniffi_aes256_cbc_raw_encrypt(key.clone(), iv.clone(), plaintext.clone()).expect("cbc enc");
    assert_eq!(cbc_enc.len(), plaintext.len());
    assert_ne!(cbc_enc, plaintext);
    let cbc_dec = uniffi_aes256_cbc_raw_decrypt(key, iv, cbc_enc).expect("cbc dec");
    assert_eq!(cbc_dec, plaintext);
}
