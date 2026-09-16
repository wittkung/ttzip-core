// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! C-ABI Cryptographic and Checksum Endpoints aligned with `sdk/include/ttzip.h`.
//!
//! Provides SIMD/hardware-accelerated hashing, symmetric ciphers, key derivation,
//! and authenticated encryption routines.

use super::helpers::{safe_cstr, safe_slice, safe_slice_mut};
use crate::crypto::{adler32, aes256, blake3, chacha20poly1305, crc32, crc64, md5, sha1, sha256, vault, xxh3, zipcrypto};
use crate::types::TTZipStatus;
use libc::c_char;
use std::ffi::CStr;
use std::panic::catch_unwind;

// MARK: - 1. Checksums & Fast Hashes

/// Hardware-accelerated CRC-32 (Castagnoli / IEEE 802.3).
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_crc32(crc: u32, data: *const u8, len: usize) -> u32 {
    match safe_slice(data, len) {
        Ok(slice) => crc32::crc32_fast(crc, slice),
        Err(_) => crc,
    }
}

/// Hardware-accelerated Adler-32 checksum.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_adler32(adler: u32, data: *const u8, len: usize) -> u32 {
    if data.is_null() && len == 0 {
        return 1;
    }
    match safe_slice(data, len) {
        Ok(slice) => adler32::adler32_fast(adler, slice),
        Err(_) => adler,
    }
}

/// Hardware-accelerated CRC-64 (ECMA-182).
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_crc64(seed: u64, data: *const u8, len: usize) -> u64 {
    match safe_slice(data, len) {
        Ok(slice) => crc64::crc64(slice, seed),
        Err(_) => seed,
    }
}

/// Hardware-accelerated XXH3 64-bit SIMD checksum.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_xxh3_64(data: *const u8, len: usize, seed: u64) -> u64 {
    match safe_slice(data, len) {
        Ok(slice) => xxh3::xxh3_64_with_seed(slice, seed),
        Err(_) => 0,
    }
}

/// Hardware-accelerated XXH3 128-bit SIMD checksum.
///
/// # Safety
/// - `out_16_bytes` must point to at least 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_xxh3_128(
    data: *const u8,
    len: usize,
    seed: u64,
    out_16_bytes: *mut u8,
) -> i32 {
    if out_16_bytes.is_null() {
        return TTZipStatus::ErrInvalidParam.to_i32();
    }
    let slice = match safe_slice(data, len) {
        Ok(s) => s,
        Err(st) => return st.to_i32(),
    };
    let (low, high) = xxh3::xxh3_128_with_seed(slice, seed);
    let mut hash = [0u8; 16];
    hash[..8].copy_from_slice(&low.to_le_bytes());
    hash[8..].copy_from_slice(&high.to_le_bytes());
    std::ptr::copy_nonoverlapping(hash.as_ptr(), out_16_bytes, 16);
    TTZipStatus::Ok.to_i32()
}

/// BLAKE3 256-bit cryptographic hash.
///
/// # Safety
/// - `out_32_bytes` must point to at least 32 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_blake3(
    data: *const u8,
    len: usize,
    out_32_bytes: *mut u8,
) -> i32 {
    if out_32_bytes.is_null() {
        return TTZipStatus::ErrInvalidParam.to_i32();
    }
    let slice = match safe_slice(data, len) {
        Ok(s) => s,
        Err(st) => return st.to_i32(),
    };
    let hash = blake3::blake3(slice);
    std::ptr::copy_nonoverlapping(hash.as_ptr(), out_32_bytes, 32);
    TTZipStatus::Ok.to_i32()
}

/// BLAKE3 keyed 256-bit cryptographic hash.
///
/// # Safety
/// - `key_32_bytes` must point to 32 readable bytes.
/// - `out_32_bytes` must point to at least 32 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_blake3_keyed(
    key_32_bytes: *const u8,
    data: *const u8,
    len: usize,
    out_32_bytes: *mut u8,
) -> i32 {
    if key_32_bytes.is_null() || out_32_bytes.is_null() {
        return TTZipStatus::ErrInvalidParam.to_i32();
    }
    let key_ref = &*(key_32_bytes as *const [u8; 32]);
    let slice = match safe_slice(data, len) {
        Ok(s) => s,
        Err(st) => return st.to_i32(),
    };
    let mut hasher = blake3::Blake3::new_keyed(key_ref);
    hasher.update(slice);
    let hash = hasher.finalize();
    std::ptr::copy_nonoverlapping(hash.as_ptr(), out_32_bytes, 32);
    TTZipStatus::Ok.to_i32()
}

/// MD5 cryptographic hash (16 bytes).
///
/// # Safety
/// - `out_16_bytes` must point to at least 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_md5(
    data: *const u8,
    len: usize,
    out_16_bytes: *mut u8,
) -> i32 {
    if out_16_bytes.is_null() {
        return TTZipStatus::ErrInvalidParam.to_i32();
    }
    let slice = match safe_slice(data, len) {
        Ok(s) => s,
        Err(st) => return st.to_i32(),
    };
    let hash = md5::md5(slice);
    std::ptr::copy_nonoverlapping(hash.as_ptr(), out_16_bytes, 16);
    TTZipStatus::Ok.to_i32()
}

/// SHA-1 cryptographic hash (20 bytes).
///
/// # Safety
/// - `out_20_bytes` must point to at least 20 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_sha1(
    data: *const u8,
    len: usize,
    out_20_bytes: *mut u8,
) -> i32 {
    if out_20_bytes.is_null() {
        return TTZipStatus::ErrInvalidParam.to_i32();
    }
    let slice = match safe_slice(data, len) {
        Ok(s) => s,
        Err(st) => return st.to_i32(),
    };
    let hash = sha1::sha1(slice);
    std::ptr::copy_nonoverlapping(hash.as_ptr(), out_20_bytes, 20);
    TTZipStatus::Ok.to_i32()
}

/// Hardware-accelerated SHA-256 cryptographic hash (32 bytes).
///
/// # Safety
/// - `out_32_bytes` must point to at least 32 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_sha256(
    data: *const u8,
    len: usize,
    out_32_bytes: *mut u8,
) -> i32 {
    if out_32_bytes.is_null() {
        return TTZipStatus::ErrInvalidParam.to_i32();
    }
    let slice = match safe_slice(data, len) {
        Ok(s) => s,
        Err(st) => return st.to_i32(),
    };
    let hash = sha256::HardwareSha256::digest(slice);
    std::ptr::copy_nonoverlapping(hash.as_ptr(), out_32_bytes, 32);
    TTZipStatus::Ok.to_i32()
}

// MARK: - 2. AES-256 (CTR & CBC)

/// Hardware AES-256-CTR encryption / decryption.
///
/// # Safety
/// - `key` must point to 32 readable bytes.
/// - `src` must point to `len` readable bytes.
/// - `dst` must point to `len` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_aes256_ctr(
    key: *const u8,
    initial_counter: u64,
    src: *const u8,
    len: usize,
    dst: *mut u8,
) -> i32 {
    let result = catch_unwind(|| {
        if key.is_null() {
            return TTZipStatus::ErrInvalidParam.to_i32();
        }
        let src_slice = match safe_slice(src, len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };
        let dst_slice = match safe_slice_mut(dst, len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };
        if len == 0 {
            return TTZipStatus::Ok.to_i32();
        }

        let key_ref = &*(key as *const [u8; 32]);
        match aes256::aes256_ctr_crypt(key_ref, initial_counter, src_slice, dst_slice) {
            Ok(()) => TTZipStatus::Ok.to_i32(),
            Err(_) => TTZipStatus::ErrInvalidParam.to_i32(),
        }
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught.to_i32())
}

/// Hardware AES-256-CBC encryption.
///
/// # Safety
/// - `key` must point to 32 readable bytes.
/// - `iv` must point to 16 readable bytes.
/// - `len` must be a multiple of 16.
/// - `src` must point to `len` readable bytes.
/// - `dst` must point to `len` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_aes256_cbc_encrypt(
    key: *const u8,
    iv: *const u8,
    src: *const u8,
    len: usize,
    dst: *mut u8,
) -> i32 {
    let result = catch_unwind(|| {
        if key.is_null() || iv.is_null() || !len.is_multiple_of(16) {
            return TTZipStatus::ErrInvalidParam.to_i32();
        }
        let src_slice = match safe_slice(src, len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };
        let dst_slice = match safe_slice_mut(dst, len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };
        if len == 0 {
            return TTZipStatus::Ok.to_i32();
        }

        let key_ref = &*(key as *const [u8; 32]);
        let iv_ref = &*(iv as *const [u8; 16]);
        match aes256::aes256_cbc_encrypt(key_ref, iv_ref, src_slice, dst_slice) {
            Ok(()) => TTZipStatus::Ok.to_i32(),
            Err(_) => TTZipStatus::ErrInvalidParam.to_i32(),
        }
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught.to_i32())
}

/// Hardware AES-256-CBC decryption.
///
/// # Safety
/// - `key` must point to 32 readable bytes.
/// - `iv` must point to 16 readable bytes.
/// - `len` must be a multiple of 16.
/// - `src` must point to `len` readable bytes.
/// - `dst` must point to `len` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_aes256_cbc_decrypt(
    key: *const u8,
    iv: *const u8,
    src: *const u8,
    len: usize,
    dst: *mut u8,
) -> i32 {
    let result = catch_unwind(|| {
        if key.is_null() || iv.is_null() || !len.is_multiple_of(16) {
            return TTZipStatus::ErrInvalidParam.to_i32();
        }
        let src_slice = match safe_slice(src, len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };
        let dst_slice = match safe_slice_mut(dst, len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };
        if len == 0 {
            return TTZipStatus::Ok.to_i32();
        }

        let key_ref = &*(key as *const [u8; 32]);
        let iv_ref = &*(iv as *const [u8; 16]);
        match aes256::aes256_cbc_decrypt(key_ref, iv_ref, src_slice, dst_slice) {
            Ok(()) => TTZipStatus::Ok.to_i32(),
            Err(_) => TTZipStatus::ErrInvalidParam.to_i32(),
        }
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught.to_i32())
}

// MARK: - 3. 7z KDF & Password Vault (AES-256-GCM)

/// 7z SHA-256 KDF key derivation.
///
/// # Safety
/// - `password` must be a valid null-terminated C string.
/// - `salt` must point to `salt_len` readable bytes.
/// - `out_key` must point to at least 32 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_7z_kdf_sha256(
    password: *const c_char,
    salt: *const u8,
    salt_len: usize,
    num_cycles_power: u32,
    out_key: *mut u8,
) -> i32 {
    let result = catch_unwind(|| {
        if out_key.is_null() {
            return TTZipStatus::ErrInvalidParam.to_i32();
        }
        let pass_str = match safe_cstr(password) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };
        let salt_slice = match safe_slice(salt, salt_len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };

        let derived = sha256::sha256_7z_kdf(pass_str, salt_slice, num_cycles_power);
        std::ptr::copy_nonoverlapping(derived.as_ptr(), out_key, 32);
        TTZipStatus::Ok.to_i32()
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught.to_i32())
}

/// AES-256-GCM authenticated password vault key encryption.
///
/// # Safety
/// - `key` must point to 32 readable bytes.
/// - `iv` must point to 12 readable bytes.
/// - `src` must point to `src_len` readable bytes.
/// - `aad` must point to `aad_len` readable bytes.
/// - `out_cipher` must point to `src_len` writable bytes.
/// - `out_tag` must point to 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_vault_encrypt_key(
    key: *const u8,
    iv: *const u8,
    src: *const u8,
    src_len: usize,
    aad: *const u8,
    aad_len: usize,
    out_cipher: *mut u8,
    out_tag: *mut u8,
) -> i32 {
    let result = catch_unwind(|| {
        if key.is_null() || iv.is_null() || out_tag.is_null() {
            return TTZipStatus::ErrInvalidParam.to_i32();
        }
        let src_slice = match safe_slice(src, src_len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };
        let aad_slice = match safe_slice(aad, aad_len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };
        let cipher_slice = match safe_slice_mut(out_cipher, src_len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };

        let key_ref = &*(key as *const [u8; 32]);
        let iv_ref = &*(iv as *const [u8; 12]);
        let tag_ref = &mut *(out_tag as *mut [u8; 16]);

        match vault::aes256_gcm_encrypt(key_ref, iv_ref, src_slice, aad_slice, cipher_slice, tag_ref) {
            Ok(()) => TTZipStatus::Ok.to_i32(),
            Err(e) => e.to_i32(),
        }
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught.to_i32())
}

/// AES-256-GCM authenticated password vault key decryption.
///
/// # Safety
/// - `key` must point to 32 readable bytes.
/// - `iv` must point to 12 readable bytes.
/// - `cipher` must point to `cipher_len` readable bytes.
/// - `aad` must point to `aad_len` readable bytes.
/// - `tag` must point to 16 readable bytes.
/// - `out_plain` must point to `cipher_len` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_vault_decrypt_key(
    key: *const u8,
    iv: *const u8,
    cipher: *const u8,
    cipher_len: usize,
    aad: *const u8,
    aad_len: usize,
    tag: *const u8,
    out_plain: *mut u8,
) -> i32 {
    let result = catch_unwind(|| {
        if key.is_null() || iv.is_null() || tag.is_null() {
            return TTZipStatus::ErrInvalidParam.to_i32();
        }
        let cipher_slice = match safe_slice(cipher, cipher_len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };
        let aad_slice = match safe_slice(aad, aad_len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };
        let plain_slice = match safe_slice_mut(out_plain, cipher_len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };

        let key_ref = &*(key as *const [u8; 32]);
        let iv_ref = &*(iv as *const [u8; 12]);
        let tag_ref = &*(tag as *const [u8; 16]);

        match vault::aes256_gcm_decrypt(key_ref, iv_ref, cipher_slice, aad_slice, tag_ref, plain_slice) {
            Ok(()) => TTZipStatus::Ok.to_i32(),
            Err(e) => e.to_i32(),
        }
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught.to_i32())
}

/// Securely zeroizes memory at the given pointer.
///
/// # Safety
/// If `ptr` is non-null, it must point to at least `len` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_vault_wipe(ptr: *mut u8, len: usize) {
    let _ = catch_unwind(|| {
        vault::secure_wipe(ptr, len);
    });
}

// MARK: - 4. ChaCha20-Poly1305 AEAD

/// ChaCha20-Poly1305 AEAD encryption.
///
/// # Safety
/// - `key` must point to 32 readable bytes.
/// - `nonce` must point to 12 readable bytes.
/// - `src` must point to `len` readable bytes.
/// - `aad` must point to `aad_len` readable bytes.
/// - `dst` must point to `len` writable bytes.
/// - `out_tag` must point to 16 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_chacha20_poly1305_encrypt(
    key: *const u8,
    nonce: *const u8,
    src: *const u8,
    len: usize,
    aad: *const u8,
    aad_len: usize,
    dst: *mut u8,
    out_tag: *mut u8,
) -> i32 {
    let result = catch_unwind(|| {
        if key.is_null() || nonce.is_null() || out_tag.is_null() {
            return TTZipStatus::ErrInvalidParam.to_i32();
        }
        let src_slice = match safe_slice(src, len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };
        let aad_slice = match safe_slice(aad, aad_len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };
        let dst_slice = match safe_slice_mut(dst, len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };

        let key_ref = &*(key as *const [u8; 32]);
        let nonce_ref = &*(nonce as *const [u8; 12]);
        let tag_ref = &mut *(out_tag as *mut [u8; 16]);

        match chacha20poly1305::chacha20_poly1305_encrypt(
            key_ref, nonce_ref, src_slice, aad_slice, dst_slice, tag_ref,
        ) {
            Ok(()) => TTZipStatus::Ok.to_i32(),
            Err(e) => e.to_i32(),
        }
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught.to_i32())
}

/// ChaCha20-Poly1305 AEAD decryption.
///
/// # Safety
/// - `key` must point to 32 readable bytes.
/// - `nonce` must point to 12 readable bytes.
/// - `src` must point to `len` readable bytes.
/// - `aad` must point to `aad_len` readable bytes.
/// - `tag` must point to 16 readable bytes.
/// - `dst` must point to `len` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_chacha20_poly1305_decrypt(
    key: *const u8,
    nonce: *const u8,
    src: *const u8,
    len: usize,
    aad: *const u8,
    aad_len: usize,
    tag: *const u8,
    dst: *mut u8,
) -> i32 {
    let result = catch_unwind(|| {
        if key.is_null() || nonce.is_null() || tag.is_null() {
            return TTZipStatus::ErrInvalidParam.to_i32();
        }
        let src_slice = match safe_slice(src, len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };
        let aad_slice = match safe_slice(aad, aad_len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };
        let dst_slice = match safe_slice_mut(dst, len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };

        let key_ref = &*(key as *const [u8; 32]);
        let nonce_ref = &*(nonce as *const [u8; 12]);
        let tag_ref = &*(tag as *const [u8; 16]);

        match chacha20poly1305::chacha20_poly1305_decrypt(
            key_ref, nonce_ref, src_slice, aad_slice, tag_ref, dst_slice,
        ) {
            Ok(()) => TTZipStatus::Ok.to_i32(),
            Err(e) => e.to_i32(),
        }
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught.to_i32())
}

// MARK: - 5. PKZIP Traditional ZipCrypto

/// Decrypts a buffer in-place using PKZIP ZipCrypto.
///
/// # Safety
/// - `password` must point to `password_len` readable bytes.
/// - `data` must point to `len` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_zipcrypto_decrypt(
    password: *const u8,
    password_len: usize,
    data: *mut u8,
    len: usize,
) -> i32 {
    let result = catch_unwind(|| {
        if password.is_null() && password_len > 0 {
            return TTZipStatus::ErrInvalidParam.to_i32();
        }
        let pwd_slice = match safe_slice(password, password_len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };
        let data_slice = match safe_slice_mut(data, len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };
        if len == 0 {
            return TTZipStatus::Ok.to_i32();
        }
        let mut keys = zipcrypto::ZipCryptoKeys::from_password(pwd_slice);
        zipcrypto::decrypt_stream_fast(&mut keys.key0, &mut keys.key1, &mut keys.key2, data_slice);
        TTZipStatus::Ok.to_i32()
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught.to_i32())
}

/// Encrypts a buffer in-place using PKZIP ZipCrypto.
///
/// # Safety
/// - `password` must point to `password_len` readable bytes.
/// - `data` must point to `len` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_zipcrypto_encrypt(
    password: *const u8,
    password_len: usize,
    data: *mut u8,
    len: usize,
) -> i32 {
    let result = catch_unwind(|| {
        if password.is_null() && password_len > 0 {
            return TTZipStatus::ErrInvalidParam.to_i32();
        }
        let pwd_slice = match safe_slice(password, password_len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };
        let data_slice = match safe_slice_mut(data, len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };
        if len == 0 {
            return TTZipStatus::Ok.to_i32();
        }
        let mut keys = zipcrypto::ZipCryptoKeys::from_password(pwd_slice);
        zipcrypto::encrypt_stream_fast(&mut keys.key0, &mut keys.key1, &mut keys.key2, data_slice);
        TTZipStatus::Ok.to_i32()
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught.to_i32())
}

/// Derives initial 3-key state for PKZIP stream cipher from a null-terminated password string.
///
/// # Safety
/// - `password` must point to a valid null-terminated C string.
/// - `key0`, `key1`, `key2` must point to writable `u32` variables.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_zipcrypto_init_keys(
    password: *const c_char,
    key0: *mut u32,
    key1: *mut u32,
    key2: *mut u32,
) -> i32 {
    let result = catch_unwind(|| {
        if password.is_null() || key0.is_null() || key1.is_null() || key2.is_null() {
            return TTZipStatus::ErrInvalidParam.to_i32();
        }
        let c_str = CStr::from_ptr(password);
        let keys = zipcrypto::ZipCryptoKeys::from_password(c_str.to_bytes());
        *key0 = keys.key0;
        *key1 = keys.key1;
        *key2 = keys.key2;
        TTZipStatus::Ok.to_i32()
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught.to_i32())
}

/// Streams ZipCrypto decryption using caller-managed state.
///
/// # Safety
/// - `key0`, `key1`, `key2` must point to valid readable/writable `u32` variables.
/// - `src` must point to `len` readable bytes.
/// - `dst` must point to `len` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_zipcrypto_decrypt_stream(
    key0: *mut u32,
    key1: *mut u32,
    key2: *mut u32,
    src: *const u8,
    len: usize,
    dst: *mut u8,
) -> i32 {
    let result = catch_unwind(|| {
        if key0.is_null() || key1.is_null() || key2.is_null() {
            return TTZipStatus::ErrInvalidParam.to_i32();
        }
        if len == 0 {
            return TTZipStatus::Ok.to_i32();
        }
        let dst_slice = match safe_slice_mut(dst, len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };

        if !std::ptr::eq(src, dst) {
            std::ptr::copy(src, dst, len);
        }

        zipcrypto::decrypt_stream_fast(&mut *key0, &mut *key1, &mut *key2, dst_slice);
        TTZipStatus::Ok.to_i32()
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught.to_i32())
}

/// Streams ZipCrypto encryption using caller-managed state.
///
/// # Safety
/// - `key0`, `key1`, `key2` must point to valid readable/writable `u32` variables.
/// - `src` must point to `len` readable bytes.
/// - `dst` must point to `len` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_zipcrypto_encrypt_stream(
    key0: *mut u32,
    key1: *mut u32,
    key2: *mut u32,
    src: *const u8,
    len: usize,
    dst: *mut u8,
) -> i32 {
    let result = catch_unwind(|| {
        if key0.is_null() || key1.is_null() || key2.is_null() {
            return TTZipStatus::ErrInvalidParam.to_i32();
        }
        if len == 0 {
            return TTZipStatus::Ok.to_i32();
        }
        let dst_slice = match safe_slice_mut(dst, len) {
            Ok(s) => s,
            Err(st) => return st.to_i32(),
        };

        if !std::ptr::eq(src, dst) {
            std::ptr::copy(src, dst, len);
        }

        zipcrypto::encrypt_stream_fast(&mut *key0, &mut *key1, &mut *key2, dst_slice);
        TTZipStatus::Ok.to_i32()
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught.to_i32())
}
