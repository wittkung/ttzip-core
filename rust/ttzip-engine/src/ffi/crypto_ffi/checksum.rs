// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! C-ABI / FFI export functions for CRC-32 and Adler-32 checksums.

use crate::crypto::{adler32, crc32};
use crate::ffi::helpers::safe_slice;

/// C-ABI exported fast CRC-32 calculator.
///
/// # Safety
/// If `data` is not null and `len > 0`, `data` must point to at least `len` valid readable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_crc32(crc: u32, data: *const u8, len: usize) -> u32 {
    match unsafe { safe_slice(data, len) } {
        Ok(slice) => crc32::crc32_fast(crc, slice),
        Err(_) => crc,
    }
}

/// C-ABI exported fast Adler-32 calculator.
///
/// # Safety
/// If `data` is not null and `len > 0`, `data` must point to at least `len` valid readable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_adler32(adler: u32, data: *const u8, len: usize) -> u32 {
    if data.is_null() && len == 0 {
        return 1;
    }
    match unsafe { safe_slice(data, len) } {
        Ok(slice) => adler32::adler32_fast(adler, slice),
        Err(_) => adler,
    }
}

/// C-ABI exported fast CRC-64 ECMA calculator.
///
/// # Safety
/// If `data` is not null and `len > 0`, `data` must point to at least `len` valid readable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_crc64(seed: u64, data: *const u8, len: usize) -> u64 {
    match unsafe { safe_slice(data, len) } {
        Ok(slice) => crate::crypto::crc64::crc64(slice, seed),
        Err(_) => seed,
    }
}

/// C-ABI exported XXH3 64-bit SIMD checksum.
///
/// # Safety
/// If `data` is not null and `len > 0`, `data` must point to at least `len` valid readable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_xxh3_64(data: *const u8, len: usize, seed: u64) -> u64 {
    match unsafe { safe_slice(data, len) } {
        Ok(slice) => crate::crypto::xxh3::xxh3_64_with_seed(slice, seed),
        Err(_) => 0,
    }
}

/// C-ABI exported XXH3 128-bit SIMD checksum.
///
/// # Safety
/// - `out_16_bytes` must point to at least 16 writable bytes.
/// - If `data` is not null and `len > 0`, `data` must point to at least `len` valid readable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_xxh3_128(
    data: *const u8,
    len: usize,
    seed: u64,
    out_16_bytes: *mut u8,
) -> i32 {
    if out_16_bytes.is_null() {
        return crate::types::TTZipStatus::ErrInvalidParam.to_i32();
    }
    let slice = match unsafe { safe_slice(data, len) } {
        Ok(s) => s,
        Err(st) => return st.to_i32(),
    };
    let (low, high) = crate::crypto::xxh3::xxh3_128_with_seed(slice, seed);
    let mut hash = [0u8; 16];
    hash[..8].copy_from_slice(&low.to_le_bytes());
    hash[8..].copy_from_slice(&high.to_le_bytes());
    unsafe {
        std::ptr::copy_nonoverlapping(hash.as_ptr(), out_16_bytes, 16);
    }
    crate::types::TTZipStatus::Ok.to_i32()
}

/// C-ABI exported BLAKE3 256-bit cryptographic hash.
///
/// # Safety
/// - `out_32_bytes` must point to at least 32 writable bytes.
/// - If `data` is not null and `len > 0`, `data` must point to at least `len` valid readable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_blake3(
    data: *const u8,
    len: usize,
    out_32_bytes: *mut u8,
) -> i32 {
    if out_32_bytes.is_null() {
        return crate::types::TTZipStatus::ErrInvalidParam.to_i32();
    }
    let slice = match unsafe { safe_slice(data, len) } {
        Ok(s) => s,
        Err(st) => return st.to_i32(),
    };
    let hash = crate::crypto::blake3::blake3(slice);
    unsafe {
        std::ptr::copy_nonoverlapping(hash.as_ptr(), out_32_bytes, 32);
    }
    crate::types::TTZipStatus::Ok.to_i32()
}

/// C-ABI exported BLAKE3 keyed 256-bit cryptographic hash.
///
/// # Safety
/// - `key_32_bytes` must point to 32 readable bytes.
/// - `out_32_bytes` must point to at least 32 writable bytes.
/// - If `data` is not null and `len > 0`, `data` must point to at least `len` valid readable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_blake3_keyed(
    key_32_bytes: *const u8,
    data: *const u8,
    len: usize,
    out_32_bytes: *mut u8,
) -> i32 {
    if key_32_bytes.is_null() || out_32_bytes.is_null() {
        return crate::types::TTZipStatus::ErrInvalidParam.to_i32();
    }
    let key_ref = unsafe { &*(key_32_bytes as *const [u8; 32]) };
    let slice = match unsafe { safe_slice(data, len) } {
        Ok(s) => s,
        Err(st) => return st.to_i32(),
    };
    let mut hasher = crate::crypto::blake3::Blake3::new_keyed(key_ref);
    hasher.update(slice);
    let hash = hasher.finalize();
    unsafe {
        std::ptr::copy_nonoverlapping(hash.as_ptr(), out_32_bytes, 32);
    }
    crate::types::TTZipStatus::Ok.to_i32()
}

/// C-ABI exported MD5 checksum (16 bytes).
///
/// # Safety
/// - `out_16_bytes` must point to at least 16 writable bytes.
/// - If `data` is not null and `len > 0`, `data` must point to at least `len` valid readable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_md5(
    data: *const u8,
    len: usize,
    out_16_bytes: *mut u8,
) -> i32 {
    if out_16_bytes.is_null() {
        return crate::types::TTZipStatus::ErrInvalidParam.to_i32();
    }
    let slice = match unsafe { safe_slice(data, len) } {
        Ok(s) => s,
        Err(st) => return st.to_i32(),
    };
    let hash = crate::crypto::md5::md5(slice);
    unsafe {
        std::ptr::copy_nonoverlapping(hash.as_ptr(), out_16_bytes, 16);
    }
    crate::types::TTZipStatus::Ok.to_i32()
}

/// C-ABI exported SHA-1 hash (20 bytes).
///
/// # Safety
/// - `out_20_bytes` must point to at least 20 writable bytes.
/// - If `data` is not null and `len > 0`, `data` must point to at least `len` valid readable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_sha1(
    data: *const u8,
    len: usize,
    out_20_bytes: *mut u8,
) -> i32 {
    if out_20_bytes.is_null() {
        return crate::types::TTZipStatus::ErrInvalidParam.to_i32();
    }
    let slice = match unsafe { safe_slice(data, len) } {
        Ok(s) => s,
        Err(st) => return st.to_i32(),
    };
    let hash = crate::crypto::sha1::sha1(slice);
    unsafe {
        std::ptr::copy_nonoverlapping(hash.as_ptr(), out_20_bytes, 20);
    }
    crate::types::TTZipStatus::Ok.to_i32()
}

/// C-ABI exported SHA-256 hash (32 bytes).
///
/// # Safety
/// - `out_32_bytes` must point to at least 32 writable bytes.
/// - If `data` is not null and `len > 0`, `data` must point to at least `len` valid readable bytes.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_sha256(
    data: *const u8,
    len: usize,
    out_32_bytes: *mut u8,
) -> i32 {
    if out_32_bytes.is_null() {
        return crate::types::TTZipStatus::ErrInvalidParam.to_i32();
    }
    let slice = match unsafe { safe_slice(data, len) } {
        Ok(s) => s,
        Err(st) => return st.to_i32(),
    };
    let hash = crate::crypto::sha256::HardwareSha256::digest(slice);
    unsafe {
        std::ptr::copy_nonoverlapping(hash.as_ptr(), out_32_bytes, 32);
    }
    crate::types::TTZipStatus::Ok.to_i32()
}
