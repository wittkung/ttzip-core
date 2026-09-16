// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! C-ABI Split Multi-Volume Endpoints aligned with `sdk/include/ttzip.h`.

use crate::archive::split::{
    SplitVolumeWriter, VirtualMultiVolumeReader, VolumeNamingScheme,
};
use crate::types::{TTZipProgressCallback, TTZipStatus};
use libc::{c_char, c_void};
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::panic::catch_unwind;
use std::path::Path;

// MARK: - Split Writer Handle

/// Opaque handle wrapping `SplitVolumeWriter`.
pub struct TTZipSplitWriterHandle {
    pub(crate) writer: SplitVolumeWriter,
    pub(crate) cached_volumes: Vec<CString>,
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_split_writer_new(
    base_path: *const c_char,
    volume_size_bytes: u64,
    naming_scheme: i32,
    clean_on_failure: bool,
) -> *mut TTZipSplitWriterHandle {
    let res = catch_unwind(|| {
        if base_path.is_null() || volume_size_bytes == 0 {
            return std::ptr::null_mut();
        }
        let p_str = match CStr::from_ptr(base_path).to_str() {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        };
        let scheme = VolumeNamingScheme::from(naming_scheme);
        let writer = match SplitVolumeWriter::new(p_str, volume_size_bytes, scheme) {
            Ok(w) => w.with_clean_on_failure(clean_on_failure),
            Err(_) => return std::ptr::null_mut(),
        };
        Box::into_raw(Box::new(TTZipSplitWriterHandle {
            writer,
            cached_volumes: Vec::new(),
        }))
    });
    res.unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_split_writer_write(
    handle: *mut TTZipSplitWriterHandle,
    data: *const u8,
    len: usize,
) -> i32 {
    let res = catch_unwind(|| {
        if handle.is_null() || (data.is_null() && len > 0) {
            return -1;
        }
        let h = &mut *handle;
        let slice = if len > 0 {
            std::slice::from_raw_parts(data, len)
        } else {
            &[]
        };
        match h.writer.write_all(slice) {
            Ok(_) => 0,
            Err(_) => -1,
        }
    });
    res.unwrap_or(-1)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_split_writer_flush(
    handle: *mut TTZipSplitWriterHandle,
) -> TTZipStatus {
    let res = catch_unwind(|| {
        if handle.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }
        let h = &mut *handle;
        match h.writer.flush() {
            Ok(_) => TTZipStatus::Ok,
            Err(_) => TTZipStatus::ErrCompressionFailed,
        }
    });
    res.unwrap_or(TTZipStatus::ErrPanicCaught)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_split_writer_close(
    handle: *mut TTZipSplitWriterHandle,
) -> TTZipStatus {
    let res = catch_unwind(|| {
        if handle.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }
        let h = &mut *handle;
        match h.writer.close() {
            Ok(volumes) => {
                h.cached_volumes = volumes
                    .into_iter()
                    .filter_map(|p| CString::new(p.to_string_lossy().as_bytes()).ok())
                    .collect();
                TTZipStatus::Ok
            }
            Err(_) => TTZipStatus::ErrCompressionFailed,
        }
    });
    res.unwrap_or(TTZipStatus::ErrPanicCaught)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_split_writer_cancel(handle: *mut TTZipSplitWriterHandle) {
    let _ = catch_unwind(|| {
        if !handle.is_null() {
            let h = &mut *handle;
            h.writer.cancel_and_cleanup();
        }
    });
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_split_writer_get_total_bytes(
    handle: *const TTZipSplitWriterHandle,
) -> u64 {
    let res = catch_unwind(|| {
        if handle.is_null() {
            return 0;
        }
        (*handle).writer.total_bytes()
    });
    res.unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_split_writer_get_volume_count(
    handle: *mut TTZipSplitWriterHandle,
) -> usize {
    let res = catch_unwind(|| {
        if handle.is_null() {
            return 0;
        }
        let h = &mut *handle;
        if h.cached_volumes.is_empty() {
            h.cached_volumes = h
                .writer
                .generated_volumes()
                .iter()
                .filter_map(|p| CString::new(p.to_string_lossy().as_bytes()).ok())
                .collect();
        }
        h.cached_volumes.len()
    });
    res.unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_split_writer_get_volume_path(
    handle: *mut TTZipSplitWriterHandle,
    index: usize,
    out_buf: *mut c_char,
    buf_capacity: usize,
) -> TTZipStatus {
    let res = catch_unwind(|| {
        if handle.is_null() || out_buf.is_null() || buf_capacity == 0 {
            return TTZipStatus::ErrInvalidParam;
        }
        let h = &mut *handle;
        if h.cached_volumes.is_empty() {
            h.cached_volumes = h
                .writer
                .generated_volumes()
                .iter()
                .filter_map(|p| CString::new(p.to_string_lossy().as_bytes()).ok())
                .collect();
        }
        if index >= h.cached_volumes.len() {
            return TTZipStatus::ErrInvalidParam;
        }
        let c_str = &h.cached_volumes[index];
        let bytes = c_str.as_bytes_with_nul();
        if bytes.len() > buf_capacity {
            return TTZipStatus::ErrPathTooLong;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_char, out_buf, bytes.len());
        TTZipStatus::Ok
    });
    res.unwrap_or(TTZipStatus::ErrPanicCaught)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_split_writer_free(handle: *mut TTZipSplitWriterHandle) {
    let _ = catch_unwind(|| {
        if !handle.is_null() {
            drop(Box::from_raw(handle));
        }
    });
}

// MARK: - Split Reader Handle

/// Opaque handle wrapping `VirtualMultiVolumeReader`.
pub struct TTZipSplitReaderHandle {
    pub(crate) reader: VirtualMultiVolumeReader,
    pub(crate) cached_volumes: Vec<CString>,
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_split_reader_open(
    seed_path: *const c_char,
) -> *mut TTZipSplitReaderHandle {
    let res = catch_unwind(|| {
        if seed_path.is_null() {
            return std::ptr::null_mut();
        }
        let p_str = match CStr::from_ptr(seed_path).to_str() {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        };
        let reader = match VirtualMultiVolumeReader::open_from_any_volume(p_str) {
            Ok(r) => r,
            Err(_) => return std::ptr::null_mut(),
        };
        let cached_volumes = reader
            .volume_paths()
            .into_iter()
            .filter_map(|p| CString::new(p.to_string_lossy().as_bytes()).ok())
            .collect();
        Box::into_raw(Box::new(TTZipSplitReaderHandle {
            reader,
            cached_volumes,
        }))
    });
    res.unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_split_reader_read(
    handle: *mut TTZipSplitReaderHandle,
    buf: *mut u8,
    len: usize,
    out_bytes_read: *mut usize,
) -> TTZipStatus {
    let res = catch_unwind(|| {
        if handle.is_null() || (buf.is_null() && len > 0) || out_bytes_read.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }
        let h = &mut *handle;
        let slice = if len > 0 {
            std::slice::from_raw_parts_mut(buf, len)
        } else {
            &mut []
        };
        match h.reader.read(slice) {
            Ok(n) => {
                *out_bytes_read = n;
                if n == 0 && len > 0 {
                    TTZipStatus::Eof
                } else {
                    TTZipStatus::Ok
                }
            }
            Err(_) => TTZipStatus::ErrOpenFailed,
        }
    });
    res.unwrap_or(TTZipStatus::ErrPanicCaught)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_split_reader_seek(
    handle: *mut TTZipSplitReaderHandle,
    offset: i64,
    whence: i32,
    out_new_offset: *mut u64,
) -> TTZipStatus {
    let res = catch_unwind(|| {
        if handle.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }
        let seek_from = match whence {
            0 => {
                if offset < 0 {
                    return TTZipStatus::ErrInvalidOffset;
                }
                SeekFrom::Start(offset as u64)
            }
            1 => SeekFrom::Current(offset),
            2 => SeekFrom::End(offset),
            _ => return TTZipStatus::ErrInvalidParam,
        };
        let h = &mut *handle;
        match h.reader.seek(seek_from) {
            Ok(pos) => {
                if !out_new_offset.is_null() {
                    *out_new_offset = pos;
                }
                TTZipStatus::Ok
            }
            Err(_) => TTZipStatus::ErrInvalidOffset,
        }
    });
    res.unwrap_or(TTZipStatus::ErrPanicCaught)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_split_reader_get_total_size(
    handle: *const TTZipSplitReaderHandle,
) -> u64 {
    let res = catch_unwind(|| {
        if handle.is_null() {
            return 0;
        }
        (*handle).reader.total_size()
    });
    res.unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_split_reader_get_volume_count(
    handle: *const TTZipSplitReaderHandle,
) -> usize {
    let res = catch_unwind(|| {
        if handle.is_null() {
            return 0;
        }
        (*handle).cached_volumes.len()
    });
    res.unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_split_reader_get_volume_path(
    handle: *const TTZipSplitReaderHandle,
    index: usize,
    out_buf: *mut c_char,
    buf_capacity: usize,
) -> TTZipStatus {
    let res = catch_unwind(|| {
        if handle.is_null() || out_buf.is_null() || buf_capacity == 0 {
            return TTZipStatus::ErrInvalidParam;
        }
        let h = &*handle;
        if index >= h.cached_volumes.len() {
            return TTZipStatus::ErrInvalidParam;
        }
        let c_str = &h.cached_volumes[index];
        let bytes = c_str.as_bytes_with_nul();
        if bytes.len() > buf_capacity {
            return TTZipStatus::ErrPathTooLong;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_char, out_buf, bytes.len());
        TTZipStatus::Ok
    });
    res.unwrap_or(TTZipStatus::ErrPanicCaught)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_split_reader_free(handle: *mut TTZipSplitReaderHandle) {
    let _ = catch_unwind(|| {
        if !handle.is_null() {
            drop(Box::from_raw(handle));
        }
    });
}

// MARK: - Split File & Join Operations

/// Slices an existing monolithic archive file into multi-volume segments in-process.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_split_file(
    src_path: *const c_char,
    dst_base_path: *const c_char,
    volume_size_bytes: u64,
    naming_scheme: i32,
    clean_on_failure: bool,
) -> TTZipStatus {
    let res = catch_unwind(|| {
        if src_path.is_null() || dst_base_path.is_null() || volume_size_bytes == 0 {
            return TTZipStatus::ErrInvalidParam;
        }
        let src_str = match CStr::from_ptr(src_path).to_str() {
            Ok(s) => s,
            Err(_) => return TTZipStatus::ErrInvalidParam,
        };
        let dst_str = match CStr::from_ptr(dst_base_path).to_str() {
            Ok(s) => s,
            Err(_) => return TTZipStatus::ErrInvalidParam,
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
        let mut writer = match SplitVolumeWriter::new(dst_str, volume_size_bytes, scheme) {
            Ok(w) => w.with_clean_on_failure(clean_on_failure),
            Err(_) => return TTZipStatus::ErrCompressionFailed,
        };

        let mut buffer = vec![0u8; 4 * 1024 * 1024]; // 4 MB buffer
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
    });
    res.unwrap_or(TTZipStatus::ErrPanicCaught)
}

/// Reassembles / joins multi-volume split files into a single unified output archive file.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_join_split_volumes(
    first_volume_path: *const c_char,
    output_path: *const c_char,
    progress_callback: TTZipProgressCallback,
    user_data: *mut c_void,
) -> TTZipStatus {
    let res = catch_unwind(|| {
        if first_volume_path.is_null() || output_path.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }
        let first_str = match CStr::from_ptr(first_volume_path).to_str() {
            Ok(s) => s,
            Err(_) => return TTZipStatus::ErrInvalidParam,
        };
        let out_str = match CStr::from_ptr(output_path).to_str() {
            Ok(s) => s,
            Err(_) => return TTZipStatus::ErrInvalidParam,
        };

        let mut reader = match VirtualMultiVolumeReader::open_from_any_volume(first_str) {
            Ok(r) => r,
            Err(_) => return TTZipStatus::ErrFileNotFound,
        };

        let total_size = reader.total_size();
        let mut out_file = match File::create(out_str) {
            Ok(f) => f,
            Err(_) => return TTZipStatus::ErrOpenFailed,
        };

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
                let should_continue = cb(total_copied, total_size, first_volume_path, user_data);
                if !should_continue {
                    return TTZipStatus::Cancelled;
                }
            }
        }

        let _ = out_file.flush();
        TTZipStatus::Ok
    });
    res.unwrap_or(TTZipStatus::ErrPanicCaught)
}
