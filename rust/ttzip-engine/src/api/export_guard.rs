// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Dynamic library symbol export hard isolation, ABI barrier, and panic protection.
//!
//! Enforces C-ABI 2.0 symbol visibility isolation:
//! - Whitelist validation ensuring only authorized `ttzip_*` C-ABI entrypoints are exposed.
//! - Panic catching boundary wrapping unhandled Rust unwinds into deterministic error statuses.
//! - Runtime ABI version compatibility checks preventing cross-version structure misalignment.

use std::ffi::CStr;
use std::panic::{catch_unwind, UnwindSafe};
use libc::c_char;

use crate::types::{set_last_error, TTZIP_ABI_VERSION_2, TTZipStatus};

/// Authoritative canonical whitelist of exported C-ABI symbols in libttzip.
pub const TTZIP_EXPORTED_SYMBOLS: &[&str] = &[
    // Checksum & Hashes
    "ttzip_rust_crc32",
    "ttzip_rust_adler32",
    "ttzip_rust_crc64",
    "ttzip_rust_xxh3_64",
    "ttzip_rust_xxh3_128",
    "ttzip_rust_blake3",
    "ttzip_rust_blake3_keyed",
    "ttzip_rust_md5",
    "ttzip_rust_sha1",
    "ttzip_rust_sha256",
    // Ciphers & Security
    "ttzip_rust_aes256_ctr",
    "ttzip_rust_aes256_cbc_decrypt",
    "ttzip_rust_aes256_cbc_encrypt",
    "ttzip_rust_vault_encrypt_key",
    "ttzip_rust_vault_decrypt_key",
    "ttzip_rust_chacha20_poly1305_encrypt",
    "ttzip_rust_chacha20_poly1305_decrypt",
    "ttzip_rust_zipcrypto_decrypt",
    "ttzip_rust_zipcrypto_encrypt",
    // Single-Format Compression Codecs
    "ttzip_rust_deflate_compress",
    "ttzip_rust_deflate_decompress",
    "ttzip_rust_zlib_compress",
    "ttzip_rust_zlib_decompress",
    "ttzip_rust_gzip_compress",
    "ttzip_rust_gzip_decompress",
    "ttzip_rust_deflate_compress_bound",
    "ttzip_rust_zstd_compress",
    "ttzip_rust_zstd_compress_advanced",
    "ttzip_rust_zstd_decompress",
    "ttzip_rust_zstd_compress_bound",
    "ttzip_rust_zstd_get_decompressed_size",
    "ttzip_rust_zstd_train_dict",
    "ttzip_rust_zstd_dict_compress",
    "ttzip_rust_zstd_dict_decompress",
    "ttzip_rust_lz4_compress",
    "ttzip_rust_lz4_decompress",
    "ttzip_rust_lz4_compress_bound",
    "ttzip_rust_snappy_compress",
    "ttzip_rust_snappy_decompress",
    "ttzip_rust_snappy_max_compressed_length",
    "ttzip_rust_snappy_uncompressed_length",
    "ttzip_rust_snappy_frame_encode",
    "ttzip_rust_snappy_frame_decode",
    "ttzip_rust_snappy_frame_max_encoded_length",
    "ttzip_rust_lzfse_compress",
    "ttzip_rust_lzfse_decompress",
    "ttzip_rust_lzfse_compress_bound",
    "ttzip_rust_brotli_compress",
    "ttzip_rust_brotli_decompress",
    "ttzip_rust_brotli_compress_bound",
    "ttzip_rust_fl2_compress",
    "ttzip_rust_fl2_decompress",
    "ttzip_rust_fl2_compress_bound",
    "ttzip_rust_fl2_find_decompressed_size",
    "ttzip_rust_bzip2_compress",
    "ttzip_rust_bzip2_decompress",
    "ttzip_rust_bzip2_compress_bound",
    // Diagnostics & Memory Management
    "ttzip_rust_get_last_error_info",
    "ttzip_rust_get_last_error_message_owned",
    "ttzip_free",
    // ABI Barrier & Export Verification
    "ttzip_abi_version",
    "ttzip_abi_is_compatible",
    "ttzip_abi_symbol_count",
    "ttzip_symbol_is_whitelisted",
];

/// Checks if a given symbol name exists within the authoritative export whitelist.
#[must_use]
pub fn is_symbol_in_whitelist(name: &str) -> bool {
    TTZIP_EXPORTED_SYMBOLS.contains(&name)
}

/// Verifies that a collection of candidate symbols strictly conforms to the whitelist.
/// Returns `Err` with all unauthorized / rogue symbols detected.
pub fn verify_symbols_whitelist<'a>(symbols: &[&'a str]) -> Result<(), Vec<&'a str>> {
    let mut rogue = Vec::new();
    for &sym in symbols {
        if !is_symbol_in_whitelist(sym) {
            rogue.push(sym);
        }
    }
    if rogue.is_empty() {
        Ok(())
    } else {
        Err(rogue)
    }
}

/// Executes a closure inside a panic barrier, catching any unwinds and returning a default value.
pub fn catch_ffi_boundary<F, R>(default_value: R, op: F) -> R
where
    F: FnOnce() -> R + UnwindSafe,
{
    match catch_unwind(op) {
        Ok(result) => result,
        Err(payload) => {
            let panic_msg = if let Some(s) = payload.downcast_ref::<&str>() {
                *s
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.as_str()
            } else {
                "Internal engine panic caught at ABI boundary"
            };
            set_last_error(TTZipStatus::ErrPanicCaught, panic_msg, None, 0);
            default_value
        }
    }
}

/// Executes an operation inside a panic barrier, translating results to C `int32_t` status codes.
pub fn catch_ffi_status<F>(op: F) -> i32
where
    F: FnOnce() -> Result<TTZipStatus, TTZipStatus> + UnwindSafe,
{
    catch_ffi_boundary(TTZipStatus::ErrPanicCaught.to_i32(), || match op() {
        Ok(status) => status.to_i32(),
        Err(err) => err.to_i32(),
    })
}

// =============================================================================
// Exported C-ABI Guard Endpoints
// =============================================================================

/// Returns the active C-ABI version identifier.
#[no_mangle]
pub unsafe extern "C" fn ttzip_abi_version() -> u32 {
    TTZIP_ABI_VERSION_2
}

/// Validates whether a client requested ABI version is backwards-compatible.
#[no_mangle]
pub unsafe extern "C" fn ttzip_abi_is_compatible(requested_version: u32) -> bool {
    requested_version == TTZIP_ABI_VERSION_2
}

/// Returns the total count of officially registered public C-ABI exported symbols.
#[no_mangle]
pub unsafe extern "C" fn ttzip_abi_symbol_count() -> usize {
    TTZIP_EXPORTED_SYMBOLS.len()
}

/// Queries whether the given null-terminated C symbol name is in the export whitelist.
#[no_mangle]
pub unsafe extern "C" fn ttzip_symbol_is_whitelisted(symbol_name: *const c_char) -> bool {
    if symbol_name.is_null() {
        return false;
    }
    match CStr::from_ptr(symbol_name).to_str() {
        Ok(name) => is_symbol_in_whitelist(name),
        Err(_) => false,
    }
}
