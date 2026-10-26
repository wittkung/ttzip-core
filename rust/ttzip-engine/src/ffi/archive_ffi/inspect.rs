// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Archive inspection C-ABI FFI entry implementation.

use crate::ffi::helpers::safe_cstr;
use crate::types::{TTZipInspectCallback, TTZipStatus};
use libc::{c_char, c_void};
use std::ffi::CStr;
use std::panic::catch_unwind;
use std::path::Path;

/// C-ABI exported unified archive inspection.
///
/// Iterates over all headers in `archive_path` and delivers `TTZipEntryMetadata`
/// to the caller callback. Returning `false` from `callback` halts traversal.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_inspect_archive(
    archive_path: *const c_char,
    password: *const c_char,
    detect_encoding: bool,
    callback: TTZipInspectCallback,
    user_data: *mut c_void,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        let path_str = match unsafe { safe_cstr(archive_path) } {
            Ok(s) => s,
            Err(st) => return st,
        };
        let pwd_str = if password.is_null() {
            None
        } else {
            match unsafe { CStr::from_ptr(password) }.to_str() {
                Ok(s) if !s.is_empty() => Some(s),
                _ => None,
            }
        };

        match crate::archive::unified::inspect::inspect_archive_c_compat(
            Path::new(path_str),
            pwd_str,
            detect_encoding,
            callback,
            user_data,
        ) {
            Ok(_) => TTZipStatus::Ok,
            Err(st) => st,
        }
    });

    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

