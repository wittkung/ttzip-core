// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Hardware-accelerated cryptographic primitives and checksum routines.

pub mod adler32;
pub mod aes256;
pub mod arm64_sha256;
pub mod blake3;
pub mod chacha20poly1305;
pub mod crc32;
pub mod crc64;
pub mod ed25519;
pub mod md5;
pub mod password_recovery;
pub mod recovery;
pub mod rs_fec;
pub mod sevenz_kdf;
pub mod sha1;
pub mod sha256;
pub mod vault;
pub mod winzip_aes;
pub mod xxh3;
pub mod zipcrypto;

pub use adler32::{adler32, adler32_fast};
pub use aes256::{aes256_cbc_decrypt, aes256_cbc_encrypt, aes256_ctr_crypt, Aes256Context};
pub use arm64_sha256::{
    derive_7z_key_arm64, sha256_compress_blocks,
};
#[cfg(target_arch = "aarch64")]
pub use arm64_sha256::sha256_compress_arm64_crypto;
pub use blake3::{
    blake3, derive_key, hash, hash_xof, keyed_hash, Blake3, Blake3Hasher, ChunkState, Hasher,
    Output, OutputReader,
};
pub use chacha20poly1305::{chacha20_poly1305_decrypt, chacha20_poly1305_encrypt, Poly1305};
pub use crc32::{crc32, crc32_combine, crc32_fast};
pub use crc64::{crc64, crc64_fast};
pub use ed25519::*;
pub use md5::{md5, FastMd5};
pub use password_recovery::*;
pub use recovery::*;
pub use rs_fec::{
    cauchy, gf8, recovery_record, ReedSolomonEngine, RecoveryRecordInfo,
};
pub use sevenz_kdf::{
    derive_7z_aes_key, password_to_utf16le, AesKdfCache, DerivedKey, MAX_AES_CYCLES_POWER,
    RAW_KEY_CYCLES_POWER,
};
pub use sha1::{
    hmac_sha1, hmac_sha1_10, pbkdf2_sha1, sha1, winzip_aes256_decrypt_and_verify,
    winzip_aes256_derive_keys, winzip_aes256_encrypt_and_tag, FastSha1, WinZipAes256Keys,
};
pub use sha256::{
    sha256_7z_kdf, FastSha256, HardwareSha256, SevenZKeyCache,
};
pub use vault::{
    aes256_gcm_decrypt, aes256_gcm_encrypt, constant_time_eq_16, get_random_bytes, hmac_sha256,
    pbkdf2_hmac_sha256, secure_wipe, secure_wipe_slice, GHash,
};
pub use winzip_aes::{
    winzip_aes_decrypt_payload, winzip_aes_encrypt_payload, WinZipAesCtr, WinZipAesDecrypter,
    WinZipAesDerivedKeys, WinZipAesEncrypter, WinZipAesHmac, WinZipAesKdf, WinZipAesKeyStrength,
    WinZipAesVersion, WINZIP_AES_AUTH_TAG_LEN, WINZIP_AES_PBKDF2_ROUNDS, WINZIP_AES_PVV_LEN,
};
pub use xxh3::{xxh3_128, xxh3_128_bytes, xxh3_64, Xxh3_128, Xxh3_64};
pub use zipcrypto::{
    zipcrypto_decrypt_slice, zipcrypto_encrypt_slice, ZipCryptoBatch4, ZipCryptoEngine,
    ZipCryptoKeys,
};



