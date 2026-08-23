// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

//! Unified C-ABI FFI Archive Interface.
//!
//! Exposes unified archive lifecycle endpoints with complete panic safety,
//! multi-volume support, ZipSlip protection, and 17-format auto-dispatch.

use crate::archive::unified::UnifiedArchiveOrchestrator;
use crate::types::{
    TTZipCreateOptions, TTZipExtractOptions, TTZipInspectCallback, TTZipStatus,
};
use libc::{c_char, c_void};
use std::ffi::CStr;
use std::panic::catch_unwind;
use std::path::{Path, PathBuf};

/// C-ABI unified archive creation endpoint.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_archive_create_unified(
    source_paths: *const *const c_char,
    source_count: usize,
    destination_path: *const c_char,
    options: *const TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if source_paths.is_null()
            || source_count == 0
            || destination_path.is_null()
            || options.is_null()
        {
            return TTZipStatus::ErrInvalidParam;
        }

        let dest_str = match CStr::from_ptr(destination_path).to_str() {
            Ok(s) => s,
            Err(_) => return TTZipStatus::ErrInvalidParam,
        };
        let dest_path = Path::new(dest_str);

        let mut paths = Vec::with_capacity(source_count);
        for i in 0..source_count {
            let src_c = *source_paths.add(i);
            if src_c.is_null() {
                continue;
            }
            if let Ok(src_str) = CStr::from_ptr(src_c).to_str() {
                paths.push(PathBuf::from(src_str));
            }
        }

        if paths.is_empty() {
            return TTZipStatus::ErrInvalidParam;
        }

        match UnifiedArchiveOrchestrator::create_archive(
            &paths,
            dest_path,
            &*options,
            split_volume_size_bytes,
        ) {
            Ok(()) => TTZipStatus::Ok,
            Err(status) => {
                crate::types::set_last_error(status, status.as_str(), dest_path.to_str(), 0);
                status
            }
        }
    });

    result.unwrap_or_else(|_| {
        crate::types::set_last_error(TTZipStatus::ErrPanicCaught, "Panic caught in archive creation FFI boundary", None, 0);
        TTZipStatus::ErrPanicCaught
    })
}

/// C-ABI unified archive extraction endpoint.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_archive_extract_unified(
    archive_path: *const c_char,
    destination_path: *const c_char,
    options: *const TTZipExtractOptions,
) -> TTZipStatus {
    ttzip_rust_archive_extract_unified_v2(archive_path, destination_path, options, std::ptr::null_mut(), std::ptr::null_mut())
}

/// C-ABI unified archive extraction endpoint v2 with direct bytes count & structured error envelope.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_archive_extract_unified_v2(
    archive_path: *const c_char,
    destination_path: *const c_char,
    options: *const TTZipExtractOptions,
    out_extracted_bytes: *mut u64,
    out_error: *mut crate::types::TTZipErrorInfo,
) -> TTZipStatus {
    if !out_error.is_null() {
        *out_error = crate::types::TTZipErrorInfo::empty();
    }

    let result = catch_unwind(|| {
        if archive_path.is_null() {
            crate::types::write_error_info(out_error, TTZipStatus::ErrInvalidParam, "Archive path pointer is null", None, 0);
            crate::types::set_last_error(TTZipStatus::ErrInvalidParam, "Archive path pointer is null", None, 0);
            return TTZipStatus::ErrInvalidParam;
        }

        let dest_c = if !destination_path.is_null() {
            destination_path
        } else if !options.is_null() && !(*options).destination_path.is_null() {
            (*options).destination_path
        } else {
            crate::types::write_error_info(out_error, TTZipStatus::ErrInvalidParam, "Destination path pointer is null", None, 0);
            crate::types::set_last_error(TTZipStatus::ErrInvalidParam, "Destination path pointer is null", None, 0);
            return TTZipStatus::ErrInvalidParam;
        };

        let archive_str = match CStr::from_ptr(archive_path).to_str() {
            Ok(s) => s,
            Err(_) => {
                crate::types::write_error_info(out_error, TTZipStatus::ErrInvalidParam, "Invalid UTF-8 in archive path", None, 0);
                crate::types::set_last_error(TTZipStatus::ErrInvalidParam, "Invalid UTF-8 in archive path", None, 0);
                return TTZipStatus::ErrInvalidParam;
            }
        };
        let dest_str = match CStr::from_ptr(dest_c).to_str() {
            Ok(s) => s,
            Err(_) => {
                crate::types::write_error_info(out_error, TTZipStatus::ErrInvalidParam, "Invalid UTF-8 in destination path", None, 0);
                crate::types::set_last_error(TTZipStatus::ErrInvalidParam, "Invalid UTF-8 in destination path", None, 0);
                return TTZipStatus::ErrInvalidParam;
            }
        };

        let archive_p = Path::new(archive_str);
        let dest_p = Path::new(dest_str);

        let default_opt = TTZipExtractOptions {
            destination_path: dest_c,
            password: std::ptr::null(),
            thread_budget: 0,
            overwrite_existing: true,
            preserve_permissions: true,
            dry_run: false,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        let opt_ref = if !options.is_null() {
            &*options
        } else {
            &default_opt
        };

        match UnifiedArchiveOrchestrator::extract_archive_with_metrics(archive_p, dest_p, opt_ref) {
            Ok(bytes) => {
                if !out_extracted_bytes.is_null() {
                    *out_extracted_bytes = bytes;
                }
                TTZipStatus::Ok
            }
            Err(status) => {
                crate::types::write_error_info(out_error, status, status.as_str(), archive_p.to_str(), 0);
                crate::types::set_last_error(status, status.as_str(), archive_p.to_str(), 0);
                status
            }
        }
    });

    result.unwrap_or_else(|_| {
        crate::types::write_error_info(out_error, TTZipStatus::ErrPanicCaught, "Panic caught in archive extraction FFI boundary", None, 0);
        crate::types::set_last_error(TTZipStatus::ErrPanicCaught, "Panic caught in archive extraction FFI boundary", None, 0);
        TTZipStatus::ErrPanicCaught
    })
}

/// C-ABI unified archive inspection endpoint.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_archive_inspect_unified(
    archive_path: *const c_char,
    password: *const c_char,
    detect_encoding: bool,
    callback: TTZipInspectCallback,
    user_data: *mut c_void,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if archive_path.is_null() || callback.is_none() {
            return TTZipStatus::ErrInvalidParam;
        }

        let arch_str = match CStr::from_ptr(archive_path).to_str() {
            Ok(s) => s,
            Err(_) => return TTZipStatus::ErrInvalidParam,
        };
        let arch_p = Path::new(arch_str);

        let pwd_opt = if !password.is_null() {
            CStr::from_ptr(password).to_str().ok()
        } else {
            None
        };

        match UnifiedArchiveOrchestrator::inspect_archive(
            arch_p,
            pwd_opt,
            detect_encoding,
            callback,
            user_data,
        ) {
            Ok(_) => TTZipStatus::Ok,
            Err(status) => status,
        }
    });

    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

/// C-ABI unified archive repair endpoint.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_archive_repair_unified(
    damaged_path: *const c_char,
    repaired_path: *const c_char,
    out_salvaged_count: *mut usize,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if damaged_path.is_null() || repaired_path.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }

        let damaged_str = match CStr::from_ptr(damaged_path).to_str() {
            Ok(s) => s,
            Err(_) => return TTZipStatus::ErrInvalidParam,
        };
        let repaired_str = match CStr::from_ptr(repaired_path).to_str() {
            Ok(s) => s,
            Err(_) => return TTZipStatus::ErrInvalidParam,
        };

        match UnifiedArchiveOrchestrator::repair_archive(
            Path::new(damaged_str),
            Path::new(repaired_str),
        ) {
            Ok(count) => {
                if !out_salvaged_count.is_null() {
                    *out_salvaged_count = count;
                }
                TTZipStatus::Ok
            }
            Err(status) => status,
        }
    });

    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

/// C-ABI single entry in-memory extraction endpoint (zero disk I/O).
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_archive_extract_single_entry_memory(
    archive_path: *const c_char,
    entry_path: *const c_char,
    entry_index: i64,
    password: *const c_char,
    out_buffer: *mut u8,
    buffer_capacity: usize,
    out_extracted_len: *mut usize,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if archive_path.is_null() || out_extracted_len.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }

        let arch_str = match CStr::from_ptr(archive_path).to_str() {
            Ok(s) => s,
            Err(_) => return TTZipStatus::ErrInvalidParam,
        };

        let entry_str = if !entry_path.is_null() {
            CStr::from_ptr(entry_path).to_str().ok()
        } else {
            None
        };

        let pwd_str = if !password.is_null() {
            CStr::from_ptr(password).to_str().ok()
        } else {
            None
        };

        match crate::archive::unified::extract_single::extract_single_entry_memory(
            Path::new(arch_str),
            entry_str,
            entry_index,
            pwd_str,
        ) {
            Ok(data) => {
                *out_extracted_len = data.len();
                if !out_buffer.is_null() && buffer_capacity >= data.len() {
                    std::ptr::copy_nonoverlapping(data.as_ptr(), out_buffer, data.len());
                }
                TTZipStatus::Ok
            }
            Err(status) => status,
        }
    });

    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

/// C-ABI batch selective extraction endpoint (Single-pass O(N) stream scan).
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_archive_extract_selected(
    archive_path: *const c_char,
    target_paths: *const *const c_char,
    target_count: usize,
    destination_dir: *const c_char,
    options: *const TTZipExtractOptions,
    out_extracted_count: *mut usize,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if archive_path.is_null()
            || target_paths.is_null()
            || target_count == 0
            || destination_dir.is_null()
        {
            return TTZipStatus::ErrInvalidParam;
        }

        let arch_str = match CStr::from_ptr(archive_path).to_str() {
            Ok(s) => s,
            Err(_) => return TTZipStatus::ErrInvalidParam,
        };
        let dest_str = match CStr::from_ptr(destination_dir).to_str() {
            Ok(s) => s,
            Err(_) => return TTZipStatus::ErrInvalidParam,
        };

        let mut targets = Vec::with_capacity(target_count);
        for i in 0..target_count {
            let t_c = *target_paths.add(i);
            if !t_c.is_null() {
                if let Ok(t_s) = CStr::from_ptr(t_c).to_str() {
                    targets.push(t_s.to_string());
                }
            }
        }

        let default_opt = TTZipExtractOptions {
            destination_path: destination_dir,
            password: std::ptr::null(),
            thread_budget: 0,
            overwrite_existing: true,
            preserve_permissions: true,
            dry_run: false,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        let opt_ref = if !options.is_null() {
            &*options
        } else {
            &default_opt
        };

        match crate::archive::unified::extract_single::extract_selected_entries(
            Path::new(arch_str),
            &targets,
            Path::new(dest_str),
            opt_ref,
        ) {
            Ok(count) => {
                if !out_extracted_count.is_null() {
                    *out_extracted_count = count;
                }
                TTZipStatus::Ok
            }
            Err(status) => status,
        }
    });

    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

/// C-ABI stream-discarding archive integrity verification endpoint.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_archive_verify_stream(
    archive_path: *const c_char,
    password: *const c_char,
    progress_callback: crate::types::TTZipProgressCallback,
    user_data: *mut c_void,
    out_report_json: *mut *mut c_char,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if archive_path.is_null() || out_report_json.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }

        let arch_str = match CStr::from_ptr(archive_path).to_str() {
            Ok(s) => s,
            Err(_) => return TTZipStatus::ErrInvalidParam,
        };

        let pwd_str = if !password.is_null() {
            CStr::from_ptr(password).to_str().ok()
        } else {
            None
        };

        match crate::archive::unified::verify::verify_archive_stream(
            Path::new(arch_str),
            pwd_str,
            progress_callback,
            user_data,
        ) {
            Ok(report) => {
                let json_str = match serde_json::to_string(&report) {
                    Ok(s) => s,
                    Err(_) => return TTZipStatus::ErrOutOfMemory,
                };
                let c_json = match std::ffi::CString::new(json_str) {
                    Ok(c) => c,
                    Err(_) => return TTZipStatus::ErrOutOfMemory,
                };
                *out_report_json = c_json.into_raw();
                TTZipStatus::Ok
            }
            Err(status) => status,
        }
    });

    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

/// C-ABI unified string deallocator for strings allocated by Rust FFI.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(std::ffi::CString::from_raw(ptr));
    }
}

/// Returns thread-local diagnostic error message or NULL if previous operation succeeded.
#[no_mangle]
pub extern "C" fn ttzip_rust_last_error_message() -> *const c_char {
    crate::types::get_last_error_message()
}

/// Clears thread-local diagnostic error message.
#[no_mangle]
pub extern "C" fn ttzip_rust_clear_last_error() {
    crate::types::clear_last_error();
}
