// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust Apple LZFSE and LZVN C-ABI FFI exports.

use crate::codecs::lzfse::{
    lzfse_compress, lzfse_compress_bound, lzfse_compress_stream, lzfse_decompress,
    lzfse_decompress_raw, lzfse_decompress_stream, lzfse_validate, lzvn_compress,
    lzvn_compress_bound, lzvn_decompress, lzvn_decompress_raw, lzvn_validate,
};
use crate::ffi::helpers::{safe_slice, safe_slice_mut};
use crate::types::TTZipStatus;
use libc::size_t;
use std::panic::catch_unwind;

// MARK: - LZFSE Compression & Decompression C-ABI

#[no_mangle]
pub extern "C" fn ttzip_rust_lzfse_compress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if out_len.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }
        let in_slice = match unsafe { safe_slice(src, src_len) } {
            Ok(s) => s,
            Err(st) => return st,
        };
        let out_slice = match unsafe { safe_slice_mut(dst, dst_capacity) } {
            Ok(s) => s,
            Err(st) => return st,
        };

        match lzfse_compress(in_slice, out_slice) {
            Ok(written) => {
                unsafe { *out_len = written };
                TTZipStatus::Ok
            }
            Err(status) => status,
        }
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

#[no_mangle]
pub extern "C" fn ttzip_rust_lzfse_decompress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if out_len.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }
        let in_slice = match unsafe { safe_slice(src, src_len) } {
            Ok(s) => s,
            Err(st) => return st,
        };
        let out_slice = match unsafe { safe_slice_mut(dst, dst_capacity) } {
            Ok(s) => s,
            Err(st) => return st,
        };

        match lzfse_decompress(in_slice, out_slice) {
            Ok(written) => {
                unsafe { *out_len = written };
                TTZipStatus::Ok
            }
            Err(status) => status,
        }
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

#[no_mangle]
pub extern "C" fn ttzip_rust_lzfse_compress_raw(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    ttzip_rust_lzfse_compress(src, src_len, dst, dst_capacity, out_len)
}

#[no_mangle]
pub extern "C" fn ttzip_rust_lzfse_decompress_raw(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    uncompressed_len: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if out_len.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }
        let in_slice = match unsafe { safe_slice(src, src_len) } {
            Ok(s) => s,
            Err(st) => return st,
        };
        let out_slice = match unsafe { safe_slice_mut(dst, dst_capacity) } {
            Ok(s) => s,
            Err(st) => return st,
        };

        if out_slice.len() < uncompressed_len {
            return TTZipStatus::ErrInvalidParam;
        }

        match lzfse_decompress_raw(in_slice, uncompressed_len) {
            Ok(bytes) => {
                out_slice[..bytes.len()].copy_from_slice(&bytes);
                unsafe { *out_len = bytes.len() };
                TTZipStatus::Ok
            }
            Err(status) => status,
        }
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

#[no_mangle]
pub extern "C" fn ttzip_rust_lzfse_compress_stream(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if out_len.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }
        let in_slice = match unsafe { safe_slice(src, src_len) } {
            Ok(s) => s,
            Err(st) => return st,
        };
        let out_slice = match unsafe { safe_slice_mut(dst, dst_capacity) } {
            Ok(s) => s,
            Err(st) => return st,
        };

        match lzfse_compress_stream(in_slice) {
            Ok(bytes) => {
                if out_slice.len() < bytes.len() {
                    return TTZipStatus::ErrInvalidParam;
                }
                out_slice[..bytes.len()].copy_from_slice(&bytes);
                unsafe { *out_len = bytes.len() };
                TTZipStatus::Ok
            }
            Err(status) => status,
        }
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

#[no_mangle]
pub extern "C" fn ttzip_rust_lzfse_decompress_stream(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if out_len.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }
        let in_slice = match unsafe { safe_slice(src, src_len) } {
            Ok(s) => s,
            Err(st) => return st,
        };
        let out_slice = match unsafe { safe_slice_mut(dst, dst_capacity) } {
            Ok(s) => s,
            Err(st) => return st,
        };

        match lzfse_decompress_stream(in_slice) {
            Ok(bytes) => {
                if out_slice.len() < bytes.len() {
                    return TTZipStatus::ErrInvalidParam;
                }
                out_slice[..bytes.len()].copy_from_slice(&bytes);
                unsafe { *out_len = bytes.len() };
                TTZipStatus::Ok
            }
            Err(status) => status,
        }
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

#[no_mangle]
pub extern "C" fn ttzip_rust_lzfse_compress_bound(src_len: size_t) -> size_t {
    lzfse_compress_bound(src_len)
}

#[no_mangle]
pub extern "C" fn ttzip_rust_lzfse_validate(src: *const u8, src_len: size_t) -> bool {
    match unsafe { safe_slice(src, src_len) } {
        Ok(slice) => lzfse_validate(slice),
        Err(_) => false,
    }
}

// MARK: - Apple LZVN Compression & Decompression C-ABI

#[no_mangle]
pub extern "C" fn ttzip_rust_lzvn_compress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if out_len.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }
        let in_slice = match unsafe { safe_slice(src, src_len) } {
            Ok(s) => s,
            Err(st) => return st,
        };
        let out_slice = match unsafe { safe_slice_mut(dst, dst_capacity) } {
            Ok(s) => s,
            Err(st) => return st,
        };

        match lzvn_compress(in_slice, out_slice) {
            Ok(written) => {
                unsafe { *out_len = written };
                TTZipStatus::Ok
            }
            Err(status) => status,
        }
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

#[no_mangle]
pub extern "C" fn ttzip_rust_lzvn_decompress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if out_len.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }
        let in_slice = match unsafe { safe_slice(src, src_len) } {
            Ok(s) => s,
            Err(st) => return st,
        };
        let out_slice = match unsafe { safe_slice_mut(dst, dst_capacity) } {
            Ok(s) => s,
            Err(st) => return st,
        };

        match lzvn_decompress(in_slice, out_slice) {
            Ok(written) => {
                unsafe { *out_len = written };
                TTZipStatus::Ok
            }
            Err(status) => status,
        }
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

#[no_mangle]
pub extern "C" fn ttzip_rust_lzvn_compress_raw(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    ttzip_rust_lzvn_compress(src, src_len, dst, dst_capacity, out_len)
}

#[no_mangle]
pub extern "C" fn ttzip_rust_lzvn_decompress_raw(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    uncompressed_len: size_t,
    out_len: *mut size_t,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if out_len.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }
        let in_slice = match unsafe { safe_slice(src, src_len) } {
            Ok(s) => s,
            Err(st) => return st,
        };
        let out_slice = match unsafe { safe_slice_mut(dst, dst_capacity) } {
            Ok(s) => s,
            Err(st) => return st,
        };

        if out_slice.len() < uncompressed_len {
            return TTZipStatus::ErrInvalidParam;
        }

        match lzvn_decompress_raw(in_slice, uncompressed_len) {
            Ok(bytes) => {
                out_slice[..bytes.len()].copy_from_slice(&bytes);
                unsafe { *out_len = bytes.len() };
                TTZipStatus::Ok
            }
            Err(status) => status,
        }
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

#[no_mangle]
pub extern "C" fn ttzip_rust_lzvn_compress_bound(src_len: size_t) -> size_t {
    lzvn_compress_bound(src_len)
}

#[no_mangle]
pub extern "C" fn ttzip_rust_lzvn_validate(src: *const u8, src_len: size_t) -> bool {
    match unsafe { safe_slice(src, src_len) } {
        Ok(slice) => lzvn_validate(slice),
        Err(_) => false,
    }
}
