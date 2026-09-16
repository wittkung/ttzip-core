// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Bzip2 C-ABI FFI exports.

use crate::codecs::bzip2::{bzip2_compress, bzip2_compress_bound, bzip2_decompress};
use crate::ffi::helpers::{safe_slice, safe_slice_mut};
use crate::types::TTZipStatus;
use libc::size_t;
use std::panic::catch_unwind;

// MARK: - Bzip2 C-ABI

#[no_mangle]
pub extern "C" fn ttzip_rust_bzip2_compress(
    src: *const u8,
    src_len: size_t,
    dst: *mut u8,
    dst_capacity: size_t,
    level: i32,
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

        match bzip2_compress(in_slice, out_slice, level) {
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
pub extern "C" fn ttzip_rust_bzip2_decompress(
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

        match bzip2_decompress(in_slice, out_slice) {
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
pub extern "C" fn ttzip_rust_bzip2_compress_bound(src_len: size_t) -> size_t {
    bzip2_compress_bound(src_len)
}
