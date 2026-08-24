// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS, Linux, and Windows.

//! Audio waveform C-ABI FFI exports.

use crate::audio::{extract_waveform_from_bytes, extract_waveform_from_file};
use crate::types::TTZipStatus;
use libc::{c_char, size_t};
use std::ffi::CStr;
use std::panic::catch_unwind;

#[no_mangle]
pub extern "C" fn ttzip_extract_audio_waveform(
    file_path: *const c_char,
    bucket_count: size_t,
    out_amplitudes: *mut f32,
    out_count: *mut size_t,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if file_path.is_null() || out_amplitudes.is_null() || bucket_count == 0 {
            return TTZipStatus::ErrInvalidParam;
        }

        let c_str = unsafe { CStr::from_ptr(file_path) };
        let path_str = match c_str.to_str() {
            Ok(s) => s,
            Err(_) => return TTZipStatus::ErrInvalidParam,
        };

        let waveform = match extract_waveform_from_file(path_str, bucket_count) {
            Ok(wf) => wf,
            Err(_) => return TTZipStatus::ErrOpenFailed,
        };

        let count = waveform.len().min(bucket_count);
        unsafe {
            std::ptr::copy_nonoverlapping(waveform.as_ptr(), out_amplitudes, count);
            if !out_count.is_null() {
                *out_count = count;
            }
        }

        TTZipStatus::Ok
    });

    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

#[no_mangle]
pub extern "C" fn ttzip_extract_audio_waveform_from_memory(
    data: *const u8,
    data_len: size_t,
    bucket_count: size_t,
    out_amplitudes: *mut f32,
    out_count: *mut size_t,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if data.is_null() || data_len == 0 || out_amplitudes.is_null() || bucket_count == 0 {
            return TTZipStatus::ErrInvalidParam;
        }

        let slice = unsafe { std::slice::from_raw_parts(data, data_len) };
        let waveform = extract_waveform_from_bytes(slice, bucket_count);
        let count = waveform.len().min(bucket_count);

        unsafe {
            std::ptr::copy_nonoverlapping(waveform.as_ptr(), out_amplitudes, count);
            if !out_count.is_null() {
                *out_count = count;
            }
        }

        TTZipStatus::Ok
    });

    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}
