// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Archive extraction C-ABI FFI entry implementation.

use crate::ffi::helpers::safe_cstr;
use crate::types::{TTZipExtractOptions, TTZipStatus};
use libc::c_char;
use std::ffi::CStr;
use std::fs;
use std::panic::catch_unwind;
use std::path::Path;

/// C-ABI exported unified archive extraction.
///
/// Implements two-stage safe extraction with `O_NOFOLLOW`, ZipSlip validation,
/// micro-buffering, and bottom-up permission/mtime application.
///
/// Result contract (spec 229 FR-023): `Ok` only when every staged item was committed;
/// `Cancelled` for policy Stop with conflicts (destination unchanged); `ErrExtractionFailed`
/// when at least one item failed to commit; decode errors keep their own status.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_extract_archive(
    archive_path: *const c_char,
    destination_path: *const c_char,
    options: *const TTZipExtractOptions,
) -> TTZipStatus {
    catch_unwind(|| extract_archive_inner(archive_path, destination_path, options))
        .unwrap_or(TTZipStatus::ErrPanicCaught)
}

unsafe fn extract_archive_inner(
    archive_path: *const c_char,
    destination_path: *const c_char,
    options: *const TTZipExtractOptions,
) -> TTZipStatus {
    let dest_c = if !destination_path.is_null() {
        destination_path
    } else if !options.is_null() && !(*options).destination_path.is_null() {
        (*options).destination_path
    } else {
        return TTZipStatus::ErrInvalidParam;
    };

    let archive_str = match safe_cstr(archive_path) {
        Ok(s) => s,
        Err(st) => return st,
    };
    let dest_str = match safe_cstr(dest_c) {
        Ok(s) => s,
        Err(st) => return st,
    };

    let archive_p = Path::new(archive_str);
    if !archive_p.exists() {
        return TTZipStatus::ErrFileNotFound;
    }
    let dest_p = Path::new(dest_str);

    let default_opt = TTZipExtractOptions {
        struct_size: std::mem::size_of::<TTZipExtractOptions>() as u32,
        abi_version: crate::types::TTZIP_ABI_VERSION_3,
        destination_path: dest_c,
        password: std::ptr::null(),
        thread_budget: 0,
        collision_policy: crate::types::TTZIP_COLLISION_POLICY_KEEP_BOTH,
        preserve_permissions: true,
        dry_run: false,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };
    let opt_ref = if !options.is_null() { &*options } else { &default_opt };

    if opt_ref.abi_version != crate::types::TTZIP_ABI_VERSION_3 {
        return TTZipStatus::ErrAbiMismatch;
    }
    if opt_ref.collision_policy > 3 {
        return TTZipStatus::ErrInvalidParam;
    }

    execute_staged_extraction(archive_p, dest_p, opt_ref)
}

fn execute_staged_extraction(archive_p: &Path, dest_p: &Path, opt_ref: &TTZipExtractOptions) -> TTZipStatus {
    use ttzip_archive::fs::collision::ExtractionSession;

    let staged = match ExtractionSession::new(dest_p).stage() {
        Ok(s) => s,
        Err(_) => return TTZipStatus::ErrExtractionFailed,
    };

    if let Err(status) = ttzip_archive::archive::unified::UnifiedArchiveOrchestrator::extract_archive_with_metrics(
        archive_p,
        &staged.staging().payload_dir(),
        opt_ref,
    ) {
        return status;
    }

    if opt_ref.dry_run {
        return TTZipStatus::Ok;
    }

    super::staged_commit::commit_staged_with_policy(staged, opt_ref.collision_policy).status
}

/// C-ABI exported 7z single entry extraction into an in-memory buffer with Early Termination.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_7z_extract_entry_memory(
    archive_path: *const c_char,
    entry_path: *const c_char,
    entry_index: i64,
    password: *const c_char,
    out_buffer: *mut u8,
    buffer_capacity: usize,
    out_extracted_len: *mut usize,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if archive_path.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }

        let arch_str = match CStr::from_ptr(archive_path).to_str() {
            Ok(s) => s,
            Err(_) => return TTZipStatus::ErrInvalidParam,
        };

        let p = Path::new(arch_str);
        if !p.exists() {
            return TTZipStatus::ErrFileNotFound;
        }

        let file = match fs::File::open(p) {
            Ok(f) => f,
            Err(_) => return TTZipStatus::ErrFileNotFound,
        };
        let file_meta = match file.metadata() {
            Ok(m) => m,
            Err(_) => return TTZipStatus::ErrOpenFailed,
        };
        if file_meta.len() == 0 {
            return TTZipStatus::ErrCorruptHeader;
        }
        let mmap = match unsafe { memmap2::MmapOptions::new().map(&file) } {
            Ok(m) => m,
            Err(_) => return TTZipStatus::ErrMmapFailed,
        };

        let archive = match crate::sevenz::SevenZArchive::open_slice(&mmap) {
            Ok(a) => a,
            Err(e) => return e,
        };

        let target_idx = if entry_index >= 0 {
            entry_index as usize
        } else if !entry_path.is_null() {
            let ep_str = match CStr::from_ptr(entry_path).to_str() {
                Ok(s) => s,
                Err(_) => return TTZipStatus::ErrInvalidParam,
            };
            match archive.seek_index().get_by_path(ep_str) {
                Some(loc) => loc.file_index,
                None => return TTZipStatus::ErrFileNotFound,
            }
        } else {
            return TTZipStatus::ErrInvalidParam;
        };

        let pass_str = if !password.is_null() {
            CStr::from_ptr(password).to_str().ok()
        } else {
            None
        };

        let payload = match archive.extract_entry_bytes_stream(target_idx, pass_str) {
            Ok(bytes) => bytes,
            Err(e) => return e,
        };

        if !out_extracted_len.is_null() {
            *out_extracted_len = payload.len();
        }

        if !out_buffer.is_null() {
            if buffer_capacity < payload.len() {
                return TTZipStatus::ErrOutOfMemory;
            }
            if !payload.is_empty() {
                std::ptr::copy_nonoverlapping(payload.as_ptr(), out_buffer, payload.len());
            }
        }

        TTZipStatus::Ok
    });

    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}
