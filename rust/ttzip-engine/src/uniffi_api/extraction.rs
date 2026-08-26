// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI Archive Extraction and In-Place Mutation Scaffolding.

use std::sync::Arc;
use super::types::{CancellationToken, CompressionReport, InPlaceMutationAction, ProgressHandler, TTZipError};

/// Extracts full archive with progress reporting.
#[uniffi::export]
pub fn extract_archive_stream(
    archive_path: String,
    destination_dir: String,
    password: Option<String>,
    progress: Option<Box<dyn ProgressHandler>>,
    token: Option<Arc<CancellationToken>>,
) -> Result<CompressionReport, TTZipError> {
    let src = std::path::Path::new(&archive_path);
    let dst = std::path::Path::new(&destination_dir);
    if !src.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

    let start = std::time::Instant::now();
    let pwd_cstr = password.as_deref().and_then(|p| std::ffi::CString::new(p).ok());

    struct ProgressBox {
        handler: Option<Box<dyn ProgressHandler>>,
        token: Option<Arc<CancellationToken>>,
    }
    let mut pbox = ProgressBox { handler: progress, token };

    unsafe extern "C" fn progress_cb(
        processed: u64,
        total: u64,
        entry_name: *const libc::c_char,
        user_data: *mut libc::c_void,
    ) -> bool {
        if user_data.is_null() {
            return true;
        }
        let p = &*(user_data as *const ProgressBox);
        if let Some(ref t) = p.token {
            if t.is_cancelled() {
                return false;
            }
        }
        if let Some(ref h) = p.handler {
            let name = if !entry_name.is_null() {
                Some(std::ffi::CStr::from_ptr(entry_name).to_string_lossy().into_owned())
            } else {
                None
            };
            return h.on_progress(processed, total, name);
        }
        true
    }

    let options = crate::types::TTZipExtractOptions {
        struct_size: std::mem::size_of::<crate::types::TTZipExtractOptions>() as u32,
        abi_version: crate::types::TTZIP_ABI_VERSION_2,
        destination_path: std::ptr::null(),
        password: pwd_cstr.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null()),
        thread_budget: 0,
        overwrite_existing: true,
        preserve_permissions: true,
        dry_run: false,
        progress_callback: Some(progress_cb),
        user_data: &mut pbox as *mut ProgressBox as *mut libc::c_void,
    };

    let bytes = crate::archive::unified::extract::extract_archive_with_metrics(src, dst, &options)
        .map_err(|s| {
            if s == crate::types::TTZipStatus::Cancelled {
                TTZipError::Cancelled
            } else if s == crate::types::TTZipStatus::ErrInvalidPassword {
                TTZipError::InvalidPassword
            } else {
                TTZipError::EngineError { code: s as i32 }
            }
        })?;

    let elapsed = start.elapsed();
    let elapsed_nanos = elapsed.as_nanos() as u64;
    let elapsed_secs = elapsed.as_secs_f64().max(0.000001);
    let throughput_mbs = (bytes as f64 / (1024.0 * 1024.0)) / elapsed_secs;

    Ok(CompressionReport {
        uncompressed_bytes: bytes,
        compressed_bytes: std::fs::metadata(src).map(|m| m.len()).unwrap_or(bytes),
        elapsed_nanos,
        throughput_mbs,
        space_savings_pct: 0.0,
        engine_provenance: "Mozilla UniFFI Native Core Pipeline".to_string(),
    })
}

/// Extracts selected subset of entries from an archive into destination directory.
#[uniffi::export]
pub fn extract_selected_entries(
    archive_path: String,
    target_entries: Vec<String>,
    destination_dir: String,
    password: Option<String>,
    progress: Option<Box<dyn ProgressHandler>>,
    token: Option<Arc<CancellationToken>>,
) -> Result<u64, TTZipError> {
    let src = std::path::Path::new(&archive_path);
    let dst = std::path::Path::new(&destination_dir);
    if !src.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

    let pwd_cstr = password.as_deref().and_then(|p| std::ffi::CString::new(p).ok());

    struct ProgressBox {
        handler: Option<Box<dyn ProgressHandler>>,
        token: Option<Arc<CancellationToken>>,
    }
    let mut pbox = ProgressBox { handler: progress, token };

    unsafe extern "C" fn progress_cb(
        processed: u64,
        total: u64,
        entry_name: *const libc::c_char,
        user_data: *mut libc::c_void,
    ) -> bool {
        if user_data.is_null() {
            return true;
        }
        let p = &*(user_data as *const ProgressBox);
        if let Some(ref t) = p.token {
            if t.is_cancelled() {
                return false;
            }
        }
        if let Some(ref h) = p.handler {
            let name = if !entry_name.is_null() {
                Some(std::ffi::CStr::from_ptr(entry_name).to_string_lossy().into_owned())
            } else {
                None
            };
            return h.on_progress(processed, total, name);
        }
        true
    }

    let options = crate::types::TTZipExtractOptions {
        struct_size: std::mem::size_of::<crate::types::TTZipExtractOptions>() as u32,
        abi_version: crate::types::TTZIP_ABI_VERSION_2,
        destination_path: std::ptr::null(),
        password: pwd_cstr.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null()),
        thread_budget: 0,
        overwrite_existing: true,
        preserve_permissions: true,
        dry_run: false,
        progress_callback: Some(progress_cb),
        user_data: &mut pbox as *mut ProgressBox as *mut libc::c_void,
    };

    crate::archive::unified::extract_single::extract_selected_entries(
        src,
        &target_entries,
        dst,
        &options,
    )
    .map(|count| count as u64)
    .map_err(|s| {
        if s == crate::types::TTZipStatus::Cancelled {
            TTZipError::Cancelled
        } else if s == crate::types::TTZipStatus::ErrInvalidPassword {
            TTZipError::InvalidPassword
        } else {
            TTZipError::EngineError { code: s as i32 }
        }
    })
}

/// Extracts a single entry stream preview directly by relative entry path.
#[uniffi::export]
pub fn extract_single_entry_by_path(
    archive_path: String,
    entry_path: String,
    password: Option<String>,
) -> Result<Vec<u8>, TTZipError> {
    let p = std::path::Path::new(&archive_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

    let source = crate::archive::source::open_archive_source(p)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    let mapped = source.as_slice().ok_or_else(|| TTZipError::IoError {
        message: "Failed to map archive bytes".to_string(),
    })?;

    let clean_target = entry_path.trim_start_matches('/');

    if mapped.starts_with(b"7z\xBC\xAF\x27\x1C") {
        let arch = crate::sevenz::decoder::SevenZArchive::open_slice(mapped)
            .map_err(|_| TTZipError::CorruptHeader { details: "Invalid 7z header".to_string(), offset: 0 })?;
        
        let found_idx = arch.files().iter().position(|f| {
            f.rel_path == clean_target || f.rel_path.ends_with(&format!("/{}", clean_target))
        }).ok_or_else(|| TTZipError::FileNotFound { path: entry_path.clone() })?;

        let budget_bytes = 100 * 1024 * 1024;
        crate::sevenz::decoder::stream::extract_entry_bytes_stream_bounded(
            mapped,
            arch.info(),
            arch.seek_index(),
            found_idx,
            password.as_deref(),
            budget_bytes,
        ).map_err(|status| match status {
            crate::types::TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
            crate::types::TTZipStatus::ErrSolidBudgetExceeded => TTZipError::EngineError { code: -24 },
            _ => TTZipError::EngineError { code: status as i32 },
        })
    } else if let Ok(zip_archive) = crate::zip::reader::ZipArchive::open_slice(mapped) {
        let found_idx = zip_archive.entries().iter().position(|e| {
            e.rel_path == clean_target || e.rel_path.ends_with(&format!("/{}", clean_target))
        }).ok_or_else(|| TTZipError::FileNotFound { path: entry_path.clone() })?;

        zip_archive.extract_entry_bytes(found_idx, password.as_deref())
            .map_err(|status| match status {
                crate::types::TTZipStatus::ErrCorruptHeader => {
                    TTZipError::CorruptHeader { details: "Corrupted entry CRC or payload".to_string(), offset: 0 }
                }
                crate::types::TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
                _ => TTZipError::EngineError { code: status as i32 },
            })
    } else {
        Err(TTZipError::EngineError { code: -1 })
    }
}

/// Repairs damaged archive file and writes to output destination.
#[uniffi::export]
pub fn repair_archive_file(damaged_path: String, output_path: String) -> Result<u64, TTZipError> {
    let damaged = std::path::Path::new(&damaged_path);
    let output = std::path::Path::new(&output_path);
    if !damaged.exists() {
        return Err(TTZipError::FileNotFound { path: damaged_path });
    }
    crate::archive::unified::repair::repair_archive(damaged, output)
        .map(|count| count as u64)
        .map_err(|s| TTZipError::EngineError { code: s as i32 })
}

/// Atomically mutates archive in-place (append, replace, delete) without full recompression.
#[uniffi::export]
pub fn in_place_mutate_archive(
    archive_path: String,
    actions: Vec<InPlaceMutationAction>,
) -> Result<(), TTZipError> {
    let p = std::path::Path::new(&archive_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

    let mut session = crate::archive::in_place_edit::InPlaceArchiveSession::begin(p, None)
        .map_err(|s| TTZipError::EngineError { code: s as i32 })?;

    for act in actions {
        if act.is_delete {
            session.delete(&act.entry_path)
                .map_err(|s| TTZipError::EngineError { code: s as i32 })?;
        } else if let Some(ref src) = act.source_path {
            let src_path = std::path::Path::new(src);
            session.replace(&act.entry_path, src_path)
                .map_err(|s| TTZipError::EngineError { code: s as i32 })?;
        }
    }

    session.commit()
        .map_err(|s| TTZipError::EngineError { code: s as i32 })?;

    Ok(())
}
