// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

use super::*;

#[test]
fn test_aes256_gcm_basic_roundtrip() {
    let key = [0x42u8; 32];
    let iv = [0x12u8; 12];
    let plaintext = b"VaultCredentialsPayload2026";
    let aad = b"VaultHeader";
    let mut cipher = vec![0u8; plaintext.len()];
    let mut tag = [0u8; 16];

    aes256_gcm_encrypt(&key, &iv, plaintext, aad, &mut cipher, &mut tag).unwrap();
    assert_ne!(&cipher[..], plaintext);

    let mut decrypted = vec![0u8; cipher.len()];
    aes256_gcm_decrypt(&key, &iv, &cipher, aad, &tag, &mut decrypted).unwrap();
    assert_eq!(&decrypted[..], plaintext);
}

#[test]
fn test_secure_wipe_compiler_fence() {
    let mut sensitive = [0xAAu8; 64];
    secure_wipe(sensitive.as_mut_ptr(), sensitive.len());
    assert_eq!(sensitive, [0u8; 64]);
}

#[test]
fn test_hmac_sha256_rfc4231() {
    let key = [0x0bu8; 20];
    let data = b"Hi There";
    let result = hmac_sha256(&key, data);
    assert_eq!(
        hex::encode(result),
        "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
    );
}

#[test]
fn test_pbkdf2_hmac_sha256_rfc6070() {
    let password = b"password";
    let salt = b"salt";
    let mut key = [0u8; 32];

    pbkdf2_hmac_sha256(password, salt, 1, &mut key).unwrap();
    assert_eq!(
        hex::encode(key),
        "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b"
    );

    pbkdf2_hmac_sha256(password, salt, 2, &mut key).unwrap();
    assert_eq!(
        hex::encode(key),
        "ae4d0c95af6b46d32d0adff928f06dd02a303f8ef3c251dfd6e2d85a95474c43"
    );

    pbkdf2_hmac_sha256(password, salt, 4096, &mut key).unwrap();
    assert_eq!(
        hex::encode(key),
        "c5e478d59288c841aa530db6845c4c8d962893a001ce4e11a4963873aa98134a"
    );
}

#[test]
fn test_ghash_mul_h_matches_scalar() {
    let x_vecs: [[u8; 16]; 6] = [
        [0; 16],
        [0xff; 16],
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        [0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01],
        [0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x0f, 0xed, 0xcb, 0xa9, 0x87, 0x65, 0x43, 0x21],
        [0x42; 16],
    ];

    let h_vecs: [[u8; 16]; 6] = [
        [0; 16],
        [0xff; 16],
        [16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1],
        [0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x80],
        [0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef],
        [0x99; 16],
    ];

    for (i, x) in x_vecs.iter().enumerate() {
        for (j, h) in h_vecs.iter().enumerate() {
            let scalar_res = GHash::mul_h_scalar(x, h);
            let pmull_res = GHash::mul_h(x, h);
            assert_eq!(
                pmull_res, scalar_res,
                "Mismatch for vector pair ({}, {})",
                i, j
            );
        }
    }
}

#[test]
fn test_aes256_gcm_variable_lengths_and_multiblock() {
    let key = [0x5au8; 32];
    let iv = [0x3cu8; 12];
    let aad = b"AuthenticatedHeaderData";

    for len in [0, 1, 15, 16, 17, 31, 32, 63, 64, 127, 128, 129, 255, 256, 257, 512, 1000] {
        let plaintext: Vec<u8> = (0..len).map(|i| (i * 17 + 5) as u8).collect();
        let mut ciphertext = vec![0u8; len];
        let mut tag = [0u8; 16];

        aes256_gcm_encrypt(&key, &iv, &plaintext, aad, &mut ciphertext, &mut tag).unwrap();
        let mut decrypted = vec![0u8; len];
        aes256_gcm_decrypt(&key, &iv, &ciphertext, aad, &tag, &mut decrypted).unwrap();
        assert_eq!(decrypted, plaintext, "AES-GCM mismatch at length {}", len);
    }
}
