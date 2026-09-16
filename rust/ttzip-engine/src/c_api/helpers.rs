// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe C-ABI memory slice, pointer validation, and string bridge helpers.
//!
//! Enforces null-pointer defenses, non-zero slice bounds checks, and UTF-8 verification.

use crate::types::TTZipStatus;
use libc::c_char;
use std::ffi::CStr;
use std::slice;

/// Safely constructs an immutable byte slice from a C raw pointer and length.
///
/// # Safety
/// If `len > 0`, `ptr` must point to at least `len` valid, initialized bytes.
#[inline(always)]
pub unsafe fn safe_slice<'a>(ptr: *const u8, len: usize) -> Result<&'a [u8], TTZipStatus> {
    if len == 0 {
        Ok(&[])
    } else if ptr.is_null() {
        Err(TTZipStatus::ErrInvalidParam)
    } else {
        // SAFETY: Caller ensures ptr is valid for len bytes; len > 0 and ptr != null
        Ok(unsafe { slice::from_raw_parts(ptr, len) })
    }
}

/// Safely constructs a mutable byte slice from a C raw pointer and length.
///
/// # Safety
/// If `len > 0`, `ptr` must point to at least `len` valid, writable bytes.
#[inline(always)]
pub unsafe fn safe_slice_mut<'a>(ptr: *mut u8, len: usize) -> Result<&'a mut [u8], TTZipStatus> {
    if len == 0 {
        Ok(&mut [])
    } else if ptr.is_null() {
        Err(TTZipStatus::ErrInvalidParam)
    } else {
        // SAFETY: Caller ensures ptr is valid for len writable bytes; len > 0 and ptr != null
        Ok(unsafe { slice::from_raw_parts_mut(ptr, len) })
    }
}

/// Safely dereferences and converts a non-null C null-terminated string into a UTF-8 `&str`.
///
/// # Safety
/// `ptr` must point to a valid null-terminated C string.
#[inline(always)]
pub unsafe fn safe_cstr<'a>(ptr: *const c_char) -> Result<&'a str, TTZipStatus> {
    if ptr.is_null() {
        Err(TTZipStatus::ErrInvalidParam)
    } else {
        // SAFETY: Caller ensures ptr points to a valid null-terminated C string
        let cstr = unsafe { CStr::from_ptr(ptr) };
        cstr.to_str().map_err(|_| TTZipStatus::ErrInvalidParam)
    }
}

/// Safely converts an optional C null-terminated string pointer into an `Option<&str>`.
///
/// Returns `Ok(None)` if `ptr` is null, or `Ok(Some(&str))` if valid UTF-8.
///
/// # Safety
/// If non-null, `ptr` must point to a valid null-terminated C string.
#[inline(always)]
pub unsafe fn safe_cstr_opt<'a>(ptr: *const c_char) -> Result<Option<&'a str>, TTZipStatus> {
    if ptr.is_null() {
        Ok(None)
    } else {
        // SAFETY: Caller ensures ptr points to a valid null-terminated C string
        let cstr = unsafe { CStr::from_ptr(ptr) };
        cstr.to_str().map(Some).map_err(|_| TTZipStatus::ErrInvalidParam)
    }
}

/// Executes a generic buffer compression or decompression operation with standard C-ABI safety guards.
///
/// Handles null-pointer checking, slice reconstruction, panic catching, and writing `out_len`.
///
/// # Safety
/// Caller must ensure `src` points to `src_len` readable bytes, and `dst` points to `dst_capacity` writable bytes.
#[inline]
pub unsafe fn run_codec_op<F>(
    src: *const u8,
    src_len: libc::size_t,
    dst: *mut u8,
    dst_capacity: libc::size_t,
    out_len: *mut libc::size_t,
    op: F,
) -> TTZipStatus
where
    F: FnOnce(&[u8], &mut [u8]) -> Result<usize, TTZipStatus>,
{
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if out_len.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }
        let in_slice = match safe_slice(src, src_len) {
            Ok(s) => s,
            Err(st) => return st,
        };
        let out_slice = match safe_slice_mut(dst, dst_capacity) {
            Ok(s) => s,
            Err(st) => return st,
        };

        match op(in_slice, out_slice) {
            Ok(written) => {
                *out_len = written;
                TTZipStatus::Ok
            }
            Err(st) => st,
        }
    }));
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

