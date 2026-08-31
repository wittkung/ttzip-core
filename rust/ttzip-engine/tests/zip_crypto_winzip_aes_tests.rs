// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit and integration tests for WinZip AES-128/192/256 authenticated encryption,
//! PBKDF2-HMAC-SHA1 1000-round key derivation, AES-CTR stream cipher, and AE-1/AE-2 state machine.

use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::crypto::winzip_aes::{
    winzip_aes_decrypt_payload, winzip_aes_encrypt_payload, WinZipAesCtr, WinZipAesDecrypter,
    WinZipAesHmac, WinZipAesKdf, WinZipAesKeyStrength, WinZipAesVersion, WINZIP_AES_AUTH_TAG_LEN,
    WINZIP_AES_PVV_LEN,
};
use ttzip_engine::types::TTZipStatus;
use zeroize::Zeroize;

// ============================================================================
// 1. Key Strength & Version Enum Properties & Roundtrip
// ============================================================================

#[test]
fn test_winzip_aes_key_strength_properties() {
    let s128 = WinZipAesKeyStrength::Aes128;
    assert_eq!(s128.key_len(), 16);
    assert_eq!(s128.salt_len(), 8);
    assert_eq!(s128.total_derived_len(), 34);
    assert_eq!(s128.code(), 1);
    assert_eq!(WinZipAesKeyStrength::from_code(1).unwrap(), s128);

    let s192 = WinZipAesKeyStrength::Aes192;
    assert_eq!(s192.key_len(), 24);
    assert_eq!(s192.salt_len(), 12);
    assert_eq!(s192.total_derived_len(), 50);
    assert_eq!(s192.code(), 2);
    assert_eq!(WinZipAesKeyStrength::from_code(2).unwrap(), s192);

    let s256 = WinZipAesKeyStrength::Aes256;
    assert_eq!(s256.key_len(), 32);
    assert_eq!(s256.salt_len(), 16);
    assert_eq!(s256.total_derived_len(), 66);
    assert_eq!(s256.code(), 3);
    assert_eq!(WinZipAesKeyStrength::from_code(3).unwrap(), s256);

    assert_eq!(
        WinZipAesKeyStrength::from_code(0),
        Err(TTZipStatus::ErrUnsupportedFeature)
    );
    assert_eq!(
        WinZipAesKeyStrength::from_code(4),
        Err(TTZipStatus::ErrUnsupportedFeature)
    );
}

#[test]
fn test_winzip_aes_version_properties() {
    let ae1 = WinZipAesVersion::AE1;
    assert_eq!(ae1.code(), 0x0001);
    assert!(!ae1.suppresses_crc());
    assert_eq!(WinZipAesVersion::from_code(0x0001).unwrap(), ae1);

    let ae2 = WinZipAesVersion::AE2;
    assert_eq!(ae2.code(), 0x0002);
    assert!(ae2.suppresses_crc());
    assert_eq!(WinZipAesVersion::from_code(0x0002).unwrap(), ae2);

    assert_eq!(
        WinZipAesVersion::from_code(0x0003),
        Err(TTZipStatus::ErrUnsupportedFeature)
    );
}

// ============================================================================
// 2. PBKDF2-HMAC-SHA1 1000-Round Key Derivation
// ============================================================================

#[test]
fn test_winzip_aes_kdf_derivation_all_strengths() {
    let password = b"TestWinZipPassword2026";
    let salt8 = [0x11u8; 8];
    let salt12 = [0x22u8; 12];
    let salt16 = [0x33u8; 16];

    // AES-128
    let keys128 = WinZipAesKdf::derive(WinZipAesKeyStrength::Aes128, password, &salt8).unwrap();
    assert_eq!(keys128.strength, WinZipAesKeyStrength::Aes128);
    assert_eq!(keys128.enc_key_slice().len(), 16);
    assert_eq!(keys128.auth_key_slice().len(), 16);
    assert_eq!(keys128.pwd_verify_2b.len(), WINZIP_AES_PVV_LEN);
    assert_ne!(keys128.enc_key_slice(), keys128.auth_key_slice());

    // AES-192
    let keys192 = WinZipAesKdf::derive(WinZipAesKeyStrength::Aes192, password, &salt12).unwrap();
    assert_eq!(keys192.strength, WinZipAesKeyStrength::Aes192);
    assert_eq!(keys192.enc_key_slice().len(), 24);
    assert_eq!(keys192.auth_key_slice().len(), 24);
    assert_eq!(keys192.pwd_verify_2b.len(), WINZIP_AES_PVV_LEN);
    assert_ne!(keys192.enc_key_slice(), keys192.auth_key_slice());

    // AES-256
    let keys256 = WinZipAesKdf::derive(WinZipAesKeyStrength::Aes256, password, &salt16).unwrap();
    assert_eq!(keys256.strength, WinZipAesKeyStrength::Aes256);
    assert_eq!(keys256.enc_key_slice().len(), 32);
    assert_eq!(keys256.auth_key_slice().len(), 32);
    assert_eq!(keys256.pwd_verify_2b.len(), WINZIP_AES_PVV_LEN);
    assert_ne!(keys256.enc_key_slice(), keys256.auth_key_slice());

    // Invalid salt lengths must fail
    assert_eq!(
        WinZipAesKdf::derive(WinZipAesKeyStrength::Aes128, password, &salt16).err(),
        Some(TTZipStatus::ErrInvalidParam)
    );
    assert_eq!(
        WinZipAesKdf::derive(WinZipAesKeyStrength::Aes256, password, &salt8).err(),
        Some(TTZipStatus::ErrInvalidParam)
    );
}

// ============================================================================
// 3. AES-CTR Little-Endian 128-bit Counter Stream Cipher
// ============================================================================

#[test]
fn test_winzip_aes_ctr_counter_increment_and_symmetry() {
    let key128 = [0x42u8; 16];
    let mut ctr_enc = WinZipAesCtr::new(WinZipAesKeyStrength::Aes128, &key128).unwrap();
    assert_eq!(ctr_enc.counter(), 1);

    let plaintext = b"0123456789ABCDEF0123456789ABCDEF Extra Tail Bytes";
    let mut ciphertext = plaintext.to_vec();
    ctr_enc.apply_keystream(&mut ciphertext);

    assert_ne!(&ciphertext, plaintext);
    assert_eq!(ciphertext.len(), plaintext.len());

    let mut ctr_dec = WinZipAesCtr::new(WinZipAesKeyStrength::Aes128, &key128).unwrap();
    let mut decrypted = ciphertext.clone();
    ctr_dec.apply_keystream(&mut decrypted);

    assert_eq!(&decrypted, plaintext);
}

#[test]
fn test_winzip_aes_ctr_chunked_streaming_equivalence() {
    for strength in [
        WinZipAesKeyStrength::Aes128,
        WinZipAesKeyStrength::Aes192,
        WinZipAesKeyStrength::Aes256,
    ] {
        let key = [0x5Au8; 32];
        let key_slice = &key[..strength.key_len()];

        let mut data = vec![0u8; 1000];
        for (i, b) in data.iter_mut().enumerate() {
            *b = (i * 37 + 13) as u8;
        }

        // 1. One-pass encryption
        let mut single_pass_cipher = data.clone();
        let mut ctr_single = WinZipAesCtr::new(strength, key_slice).unwrap();
        ctr_single.apply_keystream(&mut single_pass_cipher);

        // 2. Micro-chunked encryption (1-byte, 7-byte, 19-byte irregular chunks)
        let mut chunked_cipher = data.clone();
        let mut ctr_chunked = WinZipAesCtr::new(strength, key_slice).unwrap();
        let chunk_sizes = [1, 7, 13, 16, 23, 64, 128, 5, 2];
        let mut offset = 0;
        let mut idx = 0;

        while offset < chunked_cipher.len() {
            let chunk_sz = chunk_sizes[idx % chunk_sizes.len()];
            let end = (offset + chunk_sz).min(chunked_cipher.len());
            ctr_chunked.apply_keystream(&mut chunked_cipher[offset..end]);
            offset = end;
            idx += 1;
        }

        assert_eq!(
            single_pass_cipher, chunked_cipher,
            "Chunked AES-CTR keystream mismatch for {:?}",
            strength
        );

        // 3. Chunked decryption
        let mut decrypted = chunked_cipher.clone();
        let mut ctr_dec = WinZipAesCtr::new(strength, key_slice).unwrap();
        offset = 0;
        idx = 0;
        while offset < decrypted.len() {
            let chunk_sz = chunk_sizes[(idx + 3) % chunk_sizes.len()];
            let end = (offset + chunk_sz).min(decrypted.len());
            ctr_dec.apply_keystream(&mut decrypted[offset..end]);
            offset = end;
            idx += 1;
        }

        assert_eq!(decrypted, data, "Decryption mismatch for {:?}", strength);
    }
}

// ============================================================================
// 4. HMAC-SHA1-80 Authentication Tag Calculation & Constant Time Check
// ============================================================================

#[test]
fn test_winzip_aes_hmac_streaming_and_constant_time() {
    let auth_key = [0x77u8; 32];
    let data = b"Sensitive ciphertext payload stream for HMAC-SHA1-80 authentication";

    // 1. One-pass HMAC
    let mut hmac1 = WinZipAesHmac::new(&auth_key);
    hmac1.update(data);
    let tag1 = hmac1.finalize();
    assert_eq!(tag1.len(), WINZIP_AES_AUTH_TAG_LEN);

    // 2. Chunked HMAC
    let mut hmac2 = WinZipAesHmac::new(&auth_key);
    hmac2.update(&data[..15]);
    hmac2.update(&data[15..35]);
    hmac2.update(&data[35..]);
    let tag2 = hmac2.finalize();

    assert_eq!(tag1, tag2);
    assert!(WinZipAesHmac::verify_tag(&tag1, &tag2));

    // 3. Corrupted tag constant-time check
    let mut corrupted_tag = tag1;
    corrupted_tag[0] ^= 0x01;
    assert!(!WinZipAesHmac::verify_tag(&tag1, &corrupted_tag));
}

// ============================================================================
// 5. Encrypter & Decrypter State Machine (AE-1 & AE-2)
// ============================================================================

#[test]
fn test_winzip_aes_ae1_and_ae2_roundtrip_all_strengths() {
    let test_payloads: Vec<Vec<u8>> = vec![
        vec![],                                              // 0 bytes (Empty)
        vec![0x42],                                          // 1 byte
        vec![0xAA; 15],                                      // 15 bytes (Partial block)
        vec![0xBB; 16],                                      // 16 bytes (Exact block)
        vec![0xCC; 17],                                      // 17 bytes (Block + 1)
        b"The quick brown fox jumps over the lazy dog.".to_vec(), // 44 bytes
        vec![0x55; 4096],                                    // 4 KB
        vec![0x77; 65536],                                   // 64 KB
    ];

    let passwords = ["SimplePass", "P@ssw0rd2026!#$%", "TTZip Extreme Archive"];

    for &version in &[WinZipAesVersion::AE1, WinZipAesVersion::AE2] {
        for &strength in &[
            WinZipAesKeyStrength::Aes128,
            WinZipAesKeyStrength::Aes192,
            WinZipAesKeyStrength::Aes256,
        ] {
            let salt = vec![0x33u8; strength.salt_len()];

            for &pwd in &passwords {
                for plaintext in &test_payloads {
                    let expected_crc = crc32_fast(0, plaintext);

                    // 1. Encrypt payload
                    let enc_payload = winzip_aes_encrypt_payload(
                        version, strength, pwd, &salt, plaintext,
                    )
                    .unwrap();

                    assert_eq!(
                        enc_payload.len(),
                        strength.salt_len()
                            + WINZIP_AES_PVV_LEN
                            + plaintext.len()
                            + WINZIP_AES_AUTH_TAG_LEN
                    );

                    // 2. Decrypt payload
                    let (decrypted, crc) = winzip_aes_decrypt_payload(
                        version,
                        strength,
                        pwd,
                        &enc_payload,
                        if version.suppresses_crc() {
                            None
                        } else {
                            Some(expected_crc)
                        },
                    )
                    .unwrap();

                    assert_eq!(&decrypted, plaintext);

                    if version.suppresses_crc() {
                        assert_eq!(crc, 0, "AE-2 must return CRC = 0");
                    } else {
                        assert_eq!(crc, expected_crc, "AE-1 must return computed CRC32");
                    }
                }
            }
        }
    }
}

// ============================================================================
// 6. Security Interceptions: 2B PVV Fast Intercept & Tampering Interception
// ============================================================================

#[test]
fn test_winzip_aes_pvv_instant_wrong_password_interception() {
    let password = "CorrectPassword123";
    let wrong_password = "WrongPassword456";
    let salt = [0x55u8; 16];
    let plaintext = b"Top secret classified archive contents";

    let enc_payload = winzip_aes_encrypt_payload(
        WinZipAesVersion::AE2,
        WinZipAesKeyStrength::Aes256,
        password,
        &salt,
        plaintext,
    )
    .unwrap();

    let mut stored_pvv = [0u8; WINZIP_AES_PVV_LEN];
    stored_pvv.copy_from_slice(&enc_payload[16..18]);

    // Decrypter initialization with wrong password must immediately fail on PVV
    let dec_res = WinZipAesDecrypter::new(
        WinZipAesVersion::AE2,
        WinZipAesKeyStrength::Aes256,
        wrong_password,
        &salt,
        stored_pvv,
    );

    assert_eq!(
        dec_res.err(),
        Some(TTZipStatus::ErrInvalidPassword),
        "Must intercept wrong password instantly via 2-byte PVV"
    );
}

#[test]
fn test_winzip_aes_tampered_ciphertext_and_auth_tag_interception() {
    let password = "SecurePassword2026";
    let salt = [0x88u8; 16];
    let plaintext = b"High security payload for integrity tampering resistance test";

    let enc_payload = winzip_aes_encrypt_payload(
        WinZipAesVersion::AE2,
        WinZipAesKeyStrength::Aes256,
        password,
        &salt,
        plaintext,
    )
    .unwrap();

    // 1. Tamper single bit in ciphertext
    let mut tampered_cipher = enc_payload.clone();
    tampered_cipher[20] ^= 0x01; // In ciphertext region
    let res1 = winzip_aes_decrypt_payload(
        WinZipAesVersion::AE2,
        WinZipAesKeyStrength::Aes256,
        password,
        &tampered_cipher,
        None,
    );
    assert_eq!(res1.err(), Some(TTZipStatus::ErrInvalidPassword));

    // 2. Tamper single bit in 10-byte AuthTag
    let mut tampered_tag = enc_payload.clone();
    let last_byte = tampered_tag.len() - 1;
    tampered_tag[last_byte] ^= 0x80;
    let res2 = winzip_aes_decrypt_payload(
        WinZipAesVersion::AE2,
        WinZipAesKeyStrength::Aes256,
        password,
        &tampered_tag,
        None,
    );
    assert_eq!(res2.err(), Some(TTZipStatus::ErrInvalidPassword));

    // 3. Truncated header
    let truncated = &enc_payload[..15];
    let res3 = winzip_aes_decrypt_payload(
        WinZipAesVersion::AE2,
        WinZipAesKeyStrength::Aes256,
        password,
        truncated,
        None,
    );
    assert_eq!(res3.err(), Some(TTZipStatus::ErrCorruptHeader));
}

#[test]
fn test_winzip_aes_ae1_crc_mismatch_interception() {
    let password = "IntegrityPassword";
    let salt = [0x99u8; 16];
    let plaintext = b"Data integrity verification for AE-1 CRC32 checking";

    let enc_payload = winzip_aes_encrypt_payload(
        WinZipAesVersion::AE1,
        WinZipAesKeyStrength::Aes256,
        password,
        &salt,
        plaintext,
    )
    .unwrap();

    let invalid_expected_crc = 0xDEADBEEFu32;
    let res = winzip_aes_decrypt_payload(
        WinZipAesVersion::AE1,
        WinZipAesKeyStrength::Aes256,
        password,
        &enc_payload,
        Some(invalid_expected_crc),
    );

    assert_eq!(
        res.err(),
        Some(TTZipStatus::ErrCorruptHeader),
        "AE-1 must report ErrCorruptHeader on CRC-32 mismatch"
    );
}

// ============================================================================
// 7. Zeroize Sensitive Key Material Check
// ============================================================================

#[test]
fn test_winzip_aes_derived_keys_zeroize_on_drop() {
    let password = b"ZeroizeVerificationPass";
    let salt = [0x12u8; 16];
    let mut derived =
        WinZipAesKdf::derive(WinZipAesKeyStrength::Aes256, password, &salt).unwrap();

    assert_ne!(derived.enc_key, [0u8; 32]);
    assert_ne!(derived.auth_key, [0u8; 32]);
    assert_ne!(derived.pwd_verify_2b, [0u8; 2]);

    derived.zeroize();
    assert_eq!(derived.enc_key, [0u8; 32]);
    assert_eq!(derived.auth_key, [0u8; 32]);
    assert_eq!(derived.pwd_verify_2b, [0u8; 2]);
}
