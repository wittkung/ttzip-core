// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! C-ABI Archive Endpoints aligned with `sdk/include/ttzip.h`.
//!
//! Provides memory-safe, panic-guarded lifecycle functions for archive creation,
//! extraction, inspection, verification, and repair via UnifiedArchiveOrchestrator.

use super::helpers::{safe_cstr, safe_cstr_opt};
use crate::archive::unified::UnifiedArchiveOrchestrator;
use crate::types::{
    set_last_error, write_error_info, TTZipCreateOptions, TTZipErrorInfo, TTZipExtractOptions,
    TTZipInspectCallback, TTZipProgressCallback, TTZipStatus, TTZIP_ABI_VERSION_2,
};
use libc::{c_char, c_void};
use std::ffi::CString;
use std::panic::catch_unwind;
use std::path::{Path, PathBuf};

// MARK: - 1. Create Archive

/// High-level archive creation C-ABI entry point.
///
/// # Safety
/// - `source_paths` must point to `source_count` valid null-terminated C string pointers.
/// - `destination_path` must point to a valid null-terminated C string.
/// - `options` may be null or point to a valid `TTZipCreateOptions` struct.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_create_archive(
    source_paths: *const *const c_char,
    source_count: usize,
    destination_path: *const c_char,
    options: *const TTZipCreateOptions,
) -> TTZipStatus {
    ttzip_rust_archive_create_unified(source_paths, source_count, destination_path, options, 0)
}

/// Unified multi-format archive creation C-ABI entry point with volume split support.
///
/// # Safety
/// - `source_paths` must point to `source_count` valid null-terminated C string pointers.
/// - `destination_path` must point to a valid null-terminated C string.
/// - `options` may be null or point to a valid `TTZipCreateOptions` struct.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_archive_create_unified(
    source_paths: *const *const c_char,
    source_count: usize,
    destination_path: *const c_char,
    options: *const TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if source_paths.is_null() || source_count == 0 || destination_path.is_null() {
            set_last_error(TTZipStatus::ErrInvalidParam, "Null source or destination pointer", None, 0);
            return TTZipStatus::ErrInvalidParam;
        }

        let dest_str = match safe_cstr(destination_path) {
            Ok(s) => s,
            Err(st) => {
                set_last_error(st, "Invalid UTF-8 in destination path", None, 0);
                return st;
            }
        };
        let dest_path = Path::new(dest_str);

        let mut paths = Vec::with_capacity(source_count);
        for i in 0..source_count {
            let src_c = *source_paths.add(i);
            if src_c.is_null() {
                continue;
            }
            if let Ok(src_str) = safe_cstr(src_c) {
                paths.push(PathBuf::from(src_str));
            }
        }

        if paths.is_empty() {
            set_last_error(TTZipStatus::ErrInvalidParam, "No valid source paths provided", None, 0);
            return TTZipStatus::ErrInvalidParam;
        }

        let default_opt = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: TTZIP_ABI_VERSION_2,
            format: crate::types::TTZipArchiveFormat::Zip,
            level: crate::types::TTZipCompressionLevel::Normal,
            encryption: crate::types::TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 0,
            solid_block_size_mb: 64,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        let opt_ref = if !options.is_null() {
            &*options
        } else {
            &default_opt
        };

        match UnifiedArchiveOrchestrator::create_archive(
            &paths,
            dest_path,
            opt_ref,
            split_volume_size_bytes,
        ) {
            Ok(()) => TTZipStatus::Ok,
            Err(st) => {
                set_last_error(st, st.as_str(), dest_path.to_str(), 0);
                st
            }
        }
    });

    result.unwrap_or_else(|_| {
        set_last_error(TTZipStatus::ErrPanicCaught, "Panic caught in create_archive C-ABI", None, 0);
        TTZipStatus::ErrPanicCaught
    })
}

// MARK: - 2. Extract Archive

/// High-level archive extraction C-ABI entry point.
///
/// # Safety
/// - `archive_path` must point to a valid null-terminated C string.
/// - `destination_path` must point to a valid null-terminated C string.
/// - `options` may be null or point to a valid `TTZipExtractOptions` struct.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_extract_archive(
    archive_path: *const c_char,
    destination_path: *const c_char,
    options: *const TTZipExtractOptions,
) -> TTZipStatus {
    ttzip_rust_archive_extract_unified(archive_path, destination_path, options)
}

/// Unified multi-format archive extraction C-ABI entry point.
///
/// # Safety
/// - `archive_path` must point to a valid null-terminated C string.
/// - `destination_path` must point to a valid null-terminated C string.
/// - `options` may be null or point to a valid `TTZipExtractOptions` struct.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_archive_extract_unified(
    archive_path: *const c_char,
    destination_path: *const c_char,
    options: *const TTZipExtractOptions,
) -> TTZipStatus {
    ttzip_rust_archive_extract_unified_v2(
        archive_path,
        destination_path,
        options,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    )
}

/// Unified archive extraction C-ABI entry point v2 with metrics and structured error return.
///
/// # Safety
/// - `archive_path` must point to a valid null-terminated C string.
/// - `destination_path` must point to a valid null-terminated C string (or options.destination_path).
/// - `options` may be null or point to a valid `TTZipExtractOptions` struct.
/// - `out_extracted_bytes` if non-null receives total decompressed byte count.
/// - `out_error` if non-null receives structured diagnostic error information.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_archive_extract_unified_v2(
    archive_path: *const c_char,
    destination_path: *const c_char,
    options: *const TTZipExtractOptions,
    out_extracted_bytes: *mut u64,
    out_error: *mut TTZipErrorInfo,
) -> TTZipStatus {
    if !out_error.is_null() {
        *out_error = TTZipErrorInfo::empty();
    }

    let result = catch_unwind(|| {
        if archive_path.is_null() {
            write_error_info(out_error, TTZipStatus::ErrInvalidParam, "Archive path pointer is null", None, 0);
            set_last_error(TTZipStatus::ErrInvalidParam, "Archive path pointer is null", None, 0);
            return TTZipStatus::ErrInvalidParam;
        }

        let dest_c = if !destination_path.is_null() {
            destination_path
        } else if !options.is_null() && !(*options).destination_path.is_null() {
            (*options).destination_path
        } else {
            write_error_info(out_error, TTZipStatus::ErrInvalidParam, "Destination path pointer is null", None, 0);
            set_last_error(TTZipStatus::ErrInvalidParam, "Destination path pointer is null", None, 0);
            return TTZipStatus::ErrInvalidParam;
        };

        let archive_str = match safe_cstr(archive_path) {
            Ok(s) => s,
            Err(_) => {
                write_error_info(out_error, TTZipStatus::ErrInvalidParam, "Invalid UTF-8 in archive path", None, 0);
                set_last_error(TTZipStatus::ErrInvalidParam, "Invalid UTF-8 in archive path", None, 0);
                return TTZipStatus::ErrInvalidParam;
            }
        };

        let dest_str = match safe_cstr(dest_c) {
            Ok(s) => s,
            Err(_) => {
                write_error_info(out_error, TTZipStatus::ErrInvalidParam, "Invalid UTF-8 in destination path", None, 0);
                set_last_error(TTZipStatus::ErrInvalidParam, "Invalid UTF-8 in destination path", None, 0);
                return TTZipStatus::ErrInvalidParam;
            }
        };

        let archive_p = Path::new(archive_str);
        let dest_p = Path::new(dest_str);

        let default_opt = TTZipExtractOptions {
            struct_size: std::mem::size_of::<TTZipExtractOptions>() as u32,
            abi_version: TTZIP_ABI_VERSION_2,
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
            Err(st) => {
                write_error_info(out_error, st, st.as_str(), archive_p.to_str(), 0);
                set_last_error(st, st.as_str(), archive_p.to_str(), 0);
                st
            }
        }
    });

    result.unwrap_or_else(|_| {
        write_error_info(out_error, TTZipStatus::ErrPanicCaught, "Panic caught in extract_unified_v2 C-ABI", None, 0);
        set_last_error(TTZipStatus::ErrPanicCaught, "Panic caught in extract_unified_v2 C-ABI", None, 0);
        TTZipStatus::ErrPanicCaught
    })
}

// MARK: - 3. Inspect Archive

/// High-level archive inspection C-ABI entry point.
///
/// # Safety
/// - `archive_path` must point to a valid null-terminated C string.
/// - `password` if non-null must point to a valid null-terminated C string.
/// - `callback` must be a valid function pointer.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_inspect_archive(
    archive_path: *const c_char,
    password: *const c_char,
    detect_encoding: bool,
    callback: TTZipInspectCallback,
    user_data: *mut c_void,
) -> TTZipStatus {
    ttzip_rust_archive_inspect_unified(archive_path, password, detect_encoding, callback, user_data)
}

/// Unified multi-format archive inspection C-ABI entry point.
///
/// # Safety
/// - `archive_path` must point to a valid null-terminated C string.
/// - `password` if non-null must point to a valid null-terminated C string.
/// - `callback` must be a valid function pointer.
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
            set_last_error(TTZipStatus::ErrInvalidParam, "Null archive path or callback", None, 0);
            return TTZipStatus::ErrInvalidParam;
        }

        let arch_str = match safe_cstr(archive_path) {
            Ok(s) => s,
            Err(st) => {
                set_last_error(st, "Invalid UTF-8 in archive path", None, 0);
                return st;
            }
        };
        let arch_p = Path::new(arch_str);

        let pwd_opt = match safe_cstr_opt(password) {
            Ok(p) => p,
            Err(st) => {
                set_last_error(st, "Invalid UTF-8 in password", None, 0);
                return st;
            }
        };

        match UnifiedArchiveOrchestrator::inspect_archive(
            arch_p,
            pwd_opt,
            detect_encoding,
            callback,
            user_data,
        ) {
            Ok(_) => TTZipStatus::Ok,
            Err(st) => {
                set_last_error(st, st.as_str(), arch_p.to_str(), 0);
                st
            }
        }
    });

    result.unwrap_or_else(|_| {
        set_last_error(TTZipStatus::ErrPanicCaught, "Panic caught in inspect_unified C-ABI", None, 0);
        TTZipStatus::ErrPanicCaught
    })
}

// MARK: - 4. In-Memory Extraction & Selective Extraction

/// Extracts a single entry directly into a caller-supplied memory buffer without disk I/O.
///
/// # Safety
/// - `archive_path` must point to a valid null-terminated C string.
/// - `entry_path` if non-null must point to a valid null-terminated C string.
/// - `password` if non-null must point to a valid null-terminated C string.
/// - `out_buffer` if non-null must point to `buffer_capacity` writable bytes.
/// - `out_extracted_len` must be a valid writable pointer to `usize`.
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
            set_last_error(TTZipStatus::ErrInvalidParam, "Null archive path or output length pointer", None, 0);
            return TTZipStatus::ErrInvalidParam;
        }

        let arch_str = match safe_cstr(archive_path) {
            Ok(s) => s,
            Err(st) => return st,
        };
        let entry_str = safe_cstr_opt(entry_path).unwrap_or(None);
        let pwd_str = safe_cstr_opt(password).unwrap_or(None);

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
            Err(st) => {
                set_last_error(st, st.as_str(), Some(arch_str), 0);
                st
            }
        }
    });

    result.unwrap_or_else(|_| {
        set_last_error(TTZipStatus::ErrPanicCaught, "Panic caught in extract_single_entry_memory", None, 0);
        TTZipStatus::ErrPanicCaught
    })
}

/// Batch selective extraction of specified entries in a single pass.
///
/// # Safety
/// - `archive_path` must point to a valid null-terminated C string.
/// - `target_paths` must point to `target_count` valid null-terminated C string pointers.
/// - `destination_dir` must point to a valid null-terminated C string.
/// - `options` may be null or point to `TTZipExtractOptions`.
/// - `out_extracted_count` if non-null receives the count of successfully extracted entries.
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
            set_last_error(TTZipStatus::ErrInvalidParam, "Null parameter in extract_selected", None, 0);
            return TTZipStatus::ErrInvalidParam;
        }

        let arch_str = match safe_cstr(archive_path) {
            Ok(s) => s,
            Err(st) => return st,
        };
        let dest_str = match safe_cstr(destination_dir) {
            Ok(s) => s,
            Err(st) => return st,
        };

        let mut targets = Vec::with_capacity(target_count);
        for i in 0..target_count {
            let t_c = *target_paths.add(i);
            if !t_c.is_null() {
                if let Ok(t_s) = safe_cstr(t_c) {
                    targets.push(t_s.to_string());
                }
            }
        }

        let default_opt = TTZipExtractOptions {
            struct_size: std::mem::size_of::<TTZipExtractOptions>() as u32,
            abi_version: TTZIP_ABI_VERSION_2,
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
            Err(st) => {
                set_last_error(st, st.as_str(), Some(arch_str), 0);
                st
            }
        }
    });

    result.unwrap_or_else(|_| {
        set_last_error(TTZipStatus::ErrPanicCaught, "Panic caught in extract_selected", None, 0);
        TTZipStatus::ErrPanicCaught
    })
}

// MARK: - 5. Stream Verification & Archive Repair

/// Stream-discarding archive integrity verification endpoint returning JSON report.
///
/// # Safety
/// - `archive_path` must point to a valid null-terminated C string.
/// - `password` if non-null must point to a valid null-terminated C string.
/// - `out_report_json` must be a valid pointer to receive a heap-allocated C string.
///   The caller is responsible for freeing it via `ttzip_rust_free_string`.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_archive_verify_stream(
    archive_path: *const c_char,
    password: *const c_char,
    progress_callback: TTZipProgressCallback,
    user_data: *mut c_void,
    out_report_json: *mut *mut c_char,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if archive_path.is_null() || out_report_json.is_null() {
            set_last_error(TTZipStatus::ErrInvalidParam, "Null archive path or out_report_json pointer", None, 0);
            return TTZipStatus::ErrInvalidParam;
        }

        let arch_str = match safe_cstr(archive_path) {
            Ok(s) => s,
            Err(st) => return st,
        };
        let pwd_str = safe_cstr_opt(password).unwrap_or(None);

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
                let c_json = match CString::new(json_str) {
                    Ok(c) => c,
                    Err(_) => return TTZipStatus::ErrOutOfMemory,
                };
                *out_report_json = c_json.into_raw();
                TTZipStatus::Ok
            }
            Err(st) => {
                set_last_error(st, st.as_str(), Some(arch_str), 0);
                st
            }
        }
    });

    result.unwrap_or_else(|_| {
        set_last_error(TTZipStatus::ErrPanicCaught, "Panic caught in verify_stream", None, 0);
        TTZipStatus::ErrPanicCaught
    })
}

/// Unified archive salvage and repair endpoint.
///
/// # Safety
/// - `damaged_path` must point to a valid null-terminated C string.
/// - `repaired_path` must point to a valid null-terminated C string.
/// - `out_salvaged_count` if non-null receives the count of salvaged entries.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_archive_repair_unified(
    damaged_path: *const c_char,
    repaired_path: *const c_char,
    out_salvaged_count: *mut usize,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if damaged_path.is_null() || repaired_path.is_null() {
            set_last_error(TTZipStatus::ErrInvalidParam, "Null damaged or repaired path pointer", None, 0);
            return TTZipStatus::ErrInvalidParam;
        }

        let damaged_str = match safe_cstr(damaged_path) {
            Ok(s) => s,
            Err(st) => return st,
        };
        let repaired_str = match safe_cstr(repaired_path) {
            Ok(s) => s,
            Err(st) => return st,
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
            Err(st) => {
                set_last_error(st, st.as_str(), Some(damaged_str), 0);
                st
            }
        }
    });

    result.unwrap_or_else(|_| {
        set_last_error(TTZipStatus::ErrPanicCaught, "Panic caught in repair_unified", None, 0);
        TTZipStatus::ErrPanicCaught
    })
}

// MARK: - 5. In-Place Mutation Sessions

use crate::archive::in_place_edit::InPlaceArchiveSession;
use crate::types::TTZipArchiveFormat;
use std::panic::AssertUnwindSafe;

/// Opaque C-ABI wrapper handle around transactional in-place archive mutation session.
pub struct TTZipInPlaceSession {
    pub inner: InPlaceArchiveSession,
}

impl std::panic::RefUnwindSafe for TTZipInPlaceSession {}
impl std::panic::UnwindSafe for TTZipInPlaceSession {}

/// Begins a new transactional in-place archive mutation session.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_inplace_session_begin(
    archive_path: *const c_char,
    format: i32,
    out_session: *mut *mut TTZipInPlaceSession,
) -> TTZipStatus {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if archive_path.is_null() || out_session.is_null() {
            set_last_error(TTZipStatus::ErrInvalidParam, "Null archive_path or out_session", None, 0);
            return TTZipStatus::ErrInvalidParam;
        }

        let path_str = match safe_cstr(archive_path) {
            Ok(s) => s,
            Err(st) => {
                set_last_error(st, "Invalid archive_path C-string", None, 0);
                return st;
            }
        };

        let fmt = match format {
            1 => Some(TTZipArchiveFormat::Zip),
            2 => Some(TTZipArchiveFormat::SevenZip),
            3 => Some(TTZipArchiveFormat::Tar),
            _ => None,
        };

        match InPlaceArchiveSession::begin(Path::new(path_str), fmt) {
            Ok(session) => {
                *out_session = Box::into_raw(Box::new(TTZipInPlaceSession { inner: session }));
                TTZipStatus::Ok
            }
            Err(st) => {
                set_last_error(st, st.as_str(), Some(path_str), 0);
                st
            }
        }
    }));
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

/// Appends a new file entry into the in-place editing session.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_inplace_session_append(
    session: *mut TTZipInPlaceSession,
    entry_path: *const c_char,
    source_file_path: *const c_char,
) -> TTZipStatus {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if session.is_null() || entry_path.is_null() || source_file_path.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }

        let entry_str = match safe_cstr(entry_path) {
            Ok(s) => s,
            Err(st) => return st,
        };

        let src_str = match safe_cstr(source_file_path) {
            Ok(s) => s,
            Err(st) => return st,
        };

        let s = &mut (*session).inner;
        match s.append(entry_str, Path::new(src_str)) {
            Ok(()) => TTZipStatus::Ok,
            Err(st) => st,
        }
    }));
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

/// Replaces an existing entry inside the in-place editing session.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_inplace_session_replace(
    session: *mut TTZipInPlaceSession,
    entry_path: *const c_char,
    source_file_path: *const c_char,
) -> TTZipStatus {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if session.is_null() || entry_path.is_null() || source_file_path.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }

        let entry_str = match safe_cstr(entry_path) {
            Ok(s) => s,
            Err(st) => return st,
        };

        let src_str = match safe_cstr(source_file_path) {
            Ok(s) => s,
            Err(st) => return st,
        };

        let s = &mut (*session).inner;
        match s.replace(entry_str, Path::new(src_str)) {
            Ok(()) => TTZipStatus::Ok,
            Err(st) => st,
        }
    }));
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

/// Deletes an entry inside the in-place editing session.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_inplace_session_delete(
    session: *mut TTZipInPlaceSession,
    entry_path: *const c_char,
) -> TTZipStatus {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if session.is_null() || entry_path.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }

        let entry_str = match safe_cstr(entry_path) {
            Ok(s) => s,
            Err(st) => return st,
        };

        let s = &mut (*session).inner;
        match s.delete(entry_str) {
            Ok(()) => TTZipStatus::Ok,
            Err(st) => st,
        }
    }));
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

/// Commits the in-place editing session atomically into the original archive.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_inplace_session_commit(
    session: *mut TTZipInPlaceSession,
) -> TTZipStatus {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if session.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }

        let s = &mut (*session).inner;
        match s.commit() {
            Ok(()) => TTZipStatus::Ok,
            Err(st) => st,
        }
    }));
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

/// Cancels the session, rolling back all uncommitted mutations and removing shadow files.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_inplace_session_cancel(
    session: *mut TTZipInPlaceSession,
) -> TTZipStatus {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if session.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }

        let s = &mut (*session).inner;
        match s.cancel() {
            Ok(()) => TTZipStatus::Ok,
            Err(st) => st,
        }
    }));
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

/// Frees an in-place session handle and releases resources.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_inplace_session_free(session: *mut TTZipInPlaceSession) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !session.is_null() {
            drop(Box::from_raw(session));
        }
    }));
}

