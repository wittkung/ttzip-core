// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! C-ABI FFI entries for one-shot multi-volume slicing and reassembly.

use crate::archive::split::{
    SplitVolumeWriter, VirtualMultiVolumeReader, VolumeNamingScheme,
};
use crate::types::{TTZipProgressCallback, TTZipStatus};
use libc::{c_char, c_void};
use std::ffi::CStr;
use std::fs::File;
use std::io::{Read, Write};
use std::panic::catch_unwind;
use std::path::Path;

unsafe fn parse_cstr_pair<'a>(
    first: *const c_char,
    second: *const c_char,
) -> Result<(&'a str, &'a str), TTZipStatus> {
    if first.is_null() || second.is_null() {
        return Err(TTZipStatus::ErrInvalidParam);
    }
    let a = unsafe { CStr::from_ptr(first) }
        .to_str()
        .map_err(|_| TTZipStatus::ErrInvalidParam)?;
    let b = unsafe { CStr::from_ptr(second) }
        .to_str()
        .map_err(|_| TTZipStatus::ErrInvalidParam)?;
    Ok((a, b))
}

fn stream_into_split_writer(file: &mut File, mut writer: SplitVolumeWriter) -> TTZipStatus {
    let mut buffer = vec![0u8; 4 * 1024 * 1024];
    loop {
        let n = match file.read(&mut buffer) {
            Ok(n) => n,
            Err(_) => {
                writer.cancel_and_cleanup();
                return TTZipStatus::ErrOpenFailed;
            }
        };
        if n == 0 {
            break;
        }
        if writer.write_all(&buffer[..n]).is_err() {
            writer.cancel_and_cleanup();
            return TTZipStatus::ErrCompressionFailed;
        }
    }
    if writer.close().is_err() {
        return TTZipStatus::ErrCompressionFailed;
    }
    TTZipStatus::Ok
}

/// Slices an existing monolithic archive file into multi-volume segments in-process.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ttzip_rust_split_file(
    src_path: *const c_char,
    dst_base_path: *const c_char,
    volume_size_bytes: u64,
    naming_scheme: i32,
    clean_on_failure: bool,
) -> TTZipStatus {
    unsafe {
        let res = catch_unwind(|| {
            if volume_size_bytes == 0 {
                return TTZipStatus::ErrInvalidParam;
            }
            let (src_str, dst_str) = match parse_cstr_pair(src_path, dst_base_path) {
                Ok(pair) => pair,
                Err(st) => return st,
            };
            let src_p = Path::new(src_str);
            if !src_p.exists() {
                return TTZipStatus::ErrFileNotFound;
            }
            let mut file = match File::open(src_p) {
                Ok(f) => f,
                Err(_) => return TTZipStatus::ErrOpenFailed,
            };
            let scheme = VolumeNamingScheme::from(naming_scheme);
            let writer = match SplitVolumeWriter::new(dst_str, volume_size_bytes, scheme) {
                Ok(w) => w.with_clean_on_failure(clean_on_failure),
                Err(_) => return TTZipStatus::ErrCompressionFailed,
            };
            stream_into_split_writer(&mut file, writer)
        });
        res.unwrap_or(TTZipStatus::ErrPanicCaught)
    }
}

fn stream_joined_volumes(
    reader: &mut VirtualMultiVolumeReader,
    out_file: &mut File,
    first_volume_path: *const c_char,
    progress_callback: TTZipProgressCallback,
    user_data: *mut c_void,
) -> TTZipStatus {
    let total_size = reader.total_size();
    let mut buffer = vec![0u8; 4 * 1024 * 1024];
    let mut total_copied = 0u64;

    loop {
        let n = match reader.read(&mut buffer) {
            Ok(n) => n,
            Err(_) => return TTZipStatus::ErrOpenFailed,
        };
        if n == 0 {
            break;
        }
        if out_file.write_all(&buffer[..n]).is_err() {
            return TTZipStatus::ErrCompressionFailed;
        }
        total_copied += n as u64;

        if let Some(cb) = progress_callback {
            let should_continue =
                unsafe { cb(total_copied, total_size, first_volume_path, user_data) };
            if !should_continue {
                return TTZipStatus::Cancelled;
            }
        }
    }

    if out_file.flush().is_err() {
        return TTZipStatus::ErrCompressionFailed;
    }
    TTZipStatus::Ok
}

/// Reassembles / joins multi-volume split files into a single unified output archive file.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ttzip_rust_join_split_volumes(
    first_volume_path: *const c_char,
    output_path: *const c_char,
    progress_callback: TTZipProgressCallback,
    user_data: *mut c_void,
) -> TTZipStatus {
    unsafe {
        let res = catch_unwind(|| {
            let (first_str, out_str) = match parse_cstr_pair(first_volume_path, output_path) {
                Ok(pair) => pair,
                Err(st) => return st,
            };
            let mut reader = match VirtualMultiVolumeReader::open_from_any_volume(first_str) {
                Ok(r) => r,
                Err(_) => return TTZipStatus::ErrFileNotFound,
            };
            let mut out_file = match File::create(out_str) {
                Ok(f) => f,
                Err(_) => return TTZipStatus::ErrOpenFailed,
            };
            stream_joined_volumes(
                &mut reader,
                &mut out_file,
                first_volume_path,
                progress_callback,
                user_data,
            )
        });
        res.unwrap_or(TTZipStatus::ErrPanicCaught)
    }
}
