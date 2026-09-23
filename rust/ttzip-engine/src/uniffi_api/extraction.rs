// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Mozilla UniFFI Archive Extraction and In-Place Mutation Scaffolding.

use std::sync::Arc;
use super::types::{
    CancellationToken, CompressionReport, InPlaceMutationAction, ProgressHandler, TTZipError,
    UniFFIExtractArchiveConfig,
};

/// Resolves effective thread budget adaptively based on hardware topology and QoS.
#[inline]
#[must_use]
pub fn resolve_adaptive_extraction_threads(requested: u32) -> u32 {
    if requested > 0 {
        requested.clamp(1, 64)
    } else {
        let caps = crate::platform::cpu::CpuCapabilities::get();
        if caps.p_cores > 0 {
            caps.p_cores.clamp(1, 64)
        } else {
            crate::types::resolve_thread_budget(0) as u32
        }
    }
}

/// Extracts full archive with progress reporting.
#[uniffi::export]
pub fn extract_archive_stream(
    archive_path: String,
    destination_dir: String,
    password: Option<String>,
    progress: Option<Box<dyn ProgressHandler>>,
    token: Option<Arc<CancellationToken>>,
) -> Result<CompressionReport, TTZipError> {
    let secure_pwd = password.map(crate::crypto::ZeroizingString::new);
    let src = std::path::Path::new(&archive_path);
    let dst = std::path::Path::new(&destination_dir);
    if !src.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

    let start = std::time::Instant::now();
    let pwd_cstr = secure_pwd.as_ref().and_then(|p| p.to_c_string().ok());

    struct ProgressContext {
        handler: Option<Box<dyn ProgressHandler>>,
        token: Option<Arc<CancellationToken>>,
    }
    let progress_ctx = Arc::new(ProgressContext {
        handler: progress,
        token,
    });
    let raw_ctx = Arc::into_raw(progress_ctx) as *mut libc::c_void;

    struct ContextGuard(*mut libc::c_void);
    impl Drop for ContextGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    drop(Arc::from_raw(self.0 as *const ProgressContext));
                }
            }
        }
    }
    let _guard = ContextGuard(raw_ctx);

    unsafe extern "C" fn progress_cb(
        processed: u64,
        total: u64,
        entry_name: *const libc::c_char,
        user_data: *mut libc::c_void,
    ) -> bool {
        if user_data.is_null() {
            return true;
        }
        let unwind_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let p = &*(user_data as *const ProgressContext);
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
        }));

        unwind_res.unwrap_or(false)
    }

    let effective_thread_budget = resolve_adaptive_extraction_threads(0);
    let options = crate::types::TTZipExtractOptions {
        struct_size: std::mem::size_of::<crate::types::TTZipExtractOptions>() as u32,
        abi_version: crate::types::TTZIP_ABI_VERSION_2,
        destination_path: std::ptr::null(),
        password: pwd_cstr.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null()),
        thread_budget: effective_thread_budget,
        overwrite_existing: true,
        preserve_permissions: true,
        dry_run: false,
        progress_callback: Some(progress_cb),
        user_data: raw_ctx,
    };

    let bytes = crate::archive::unified::extract::extract_archive_with_metrics(src, dst, &options)
        .map_err(|s| match s {
            crate::types::TTZipStatus::Cancelled => TTZipError::Cancelled,
            crate::types::TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
            crate::types::TTZipStatus::ErrOutOfMemory => TTZipError::OutOfMemory {
                details: "Extraction memory allocation limit exceeded".to_string(),
            },
            crate::types::TTZipStatus::ErrSecurityViolation => TTZipError::DecompressionBombDetected {
                reason: "Decompression ratio threshold exceeded or overlapping entry detected".to_string(),
            },
            crate::types::TTZipStatus::ErrCorruptHeader => TTZipError::CorruptHeader {
                details: "Corrupted archive header or checksum mismatch".to_string(),
                offset: 0,
            },
            _ => TTZipError::EngineError { code: s as i32 },
        })?;

    let elapsed = start.elapsed();
    let elapsed_nanos = elapsed.as_nanos() as u64;
    let elapsed_secs = elapsed.as_secs_f64().max(0.000001);
    let throughput_mbs = (bytes as f64 / (1024.0 * 1024.0)) / elapsed_secs;
    let comp_bytes = std::fs::metadata(src).map(|m| m.len()).unwrap_or(bytes);
    let space_savings_pct = if bytes > 0 && comp_bytes <= bytes {
        ((bytes - comp_bytes) as f64 / bytes as f64) * 100.0
    } else {
        0.0
    };

    Ok(CompressionReport {
        uncompressed_bytes: bytes,
        compressed_bytes: comp_bytes,
        elapsed_nanos,
        throughput_mbs,
        space_savings_pct,
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
    let secure_pwd = password.map(crate::crypto::ZeroizingString::new);
    let src = std::path::Path::new(&archive_path);
    let dst = std::path::Path::new(&destination_dir);
    if !src.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

    let pwd_cstr = secure_pwd.as_ref().and_then(|p| p.to_c_string().ok());

    struct ProgressContextSelected {
        handler: Option<Box<dyn ProgressHandler>>,
        token: Option<Arc<CancellationToken>>,
    }
    let progress_ctx = Arc::new(ProgressContextSelected {
        handler: progress,
        token,
    });
    let raw_ctx = Arc::into_raw(progress_ctx) as *mut libc::c_void;

    struct ContextGuardSelected(*mut libc::c_void);
    impl Drop for ContextGuardSelected {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    drop(Arc::from_raw(self.0 as *const ProgressContextSelected));
                }
            }
        }
    }
    let _guard = ContextGuardSelected(raw_ctx);

    unsafe extern "C" fn progress_cb(
        processed: u64,
        total: u64,
        entry_name: *const libc::c_char,
        user_data: *mut libc::c_void,
    ) -> bool {
        if user_data.is_null() {
            return true;
        }
        let unwind_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let p = &*(user_data as *const ProgressContextSelected);
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
        }));

        unwind_res.unwrap_or(false)
    }

    let effective_thread_budget = resolve_adaptive_extraction_threads(0);
    let options = crate::types::TTZipExtractOptions {
        struct_size: std::mem::size_of::<crate::types::TTZipExtractOptions>() as u32,
        abi_version: crate::types::TTZIP_ABI_VERSION_2,
        destination_path: std::ptr::null(),
        password: pwd_cstr.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null()),
        thread_budget: effective_thread_budget,
        overwrite_existing: true,
        preserve_permissions: true,
        dry_run: false,
        progress_callback: Some(progress_cb),
        user_data: raw_ctx,
    };

    crate::archive::unified::extract_single::extract_selected_entries(
        src,
        &target_entries,
        dst,
        &options,
    )
    .map(|count| count as u64)
    .map_err(|s| match s {
        crate::types::TTZipStatus::Cancelled => TTZipError::Cancelled,
        crate::types::TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
        crate::types::TTZipStatus::ErrOutOfMemory => TTZipError::OutOfMemory {
            details: "Extraction memory allocation limit exceeded".to_string(),
        },
        crate::types::TTZipStatus::ErrSecurityViolation => TTZipError::DecompressionBombDetected {
            reason: "Decompression ratio threshold exceeded or overlapping entry detected".to_string(),
        },
        crate::types::TTZipStatus::ErrCorruptHeader => TTZipError::CorruptHeader {
            details: "Corrupted archive header or checksum mismatch".to_string(),
            offset: 0,
        },
        _ => TTZipError::EngineError { code: s as i32 },
    })
}

/// Extracts full archive with explicit configuration (including thread budget) and progress reporting.
#[uniffi::export]
pub fn extract_archive_stream_advanced(
    config: UniFFIExtractArchiveConfig,
    progress: Option<Box<dyn ProgressHandler>>,
    token: Option<Arc<CancellationToken>>,
) -> Result<CompressionReport, TTZipError> {
    let secure_pwd = config.password.map(crate::crypto::ZeroizingString::new);
    let src = std::path::Path::new(&config.archive_path);
    let dst = std::path::Path::new(&config.destination_dir);
    if !src.exists() {
        return Err(TTZipError::FileNotFound { path: config.archive_path });
    }

    let start = std::time::Instant::now();
    let pwd_cstr = secure_pwd.as_ref().and_then(|p| p.to_c_string().ok());

    struct ProgressContextAdvanced {
        handler: Option<Box<dyn ProgressHandler>>,
        token: Option<Arc<CancellationToken>>,
    }
    let progress_ctx = Arc::new(ProgressContextAdvanced {
        handler: progress,
        token,
    });
    let raw_ctx = Arc::into_raw(progress_ctx) as *mut libc::c_void;

    struct ContextGuardAdvanced(*mut libc::c_void);
    impl Drop for ContextGuardAdvanced {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    drop(Arc::from_raw(self.0 as *const ProgressContextAdvanced));
                }
            }
        }
    }
    let _guard = ContextGuardAdvanced(raw_ctx);

    unsafe extern "C" fn progress_cb(
        processed: u64,
        total: u64,
        entry_name: *const libc::c_char,
        user_data: *mut libc::c_void,
    ) -> bool {
        if user_data.is_null() {
            return true;
        }
        let unwind_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let p = &*(user_data as *const ProgressContextAdvanced);
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
        }));

        unwind_res.unwrap_or(false)
    }

    let effective_thread_budget = resolve_adaptive_extraction_threads(config.thread_budget);
    let options = crate::types::TTZipExtractOptions {
        struct_size: std::mem::size_of::<crate::types::TTZipExtractOptions>() as u32,
        abi_version: crate::types::TTZIP_ABI_VERSION_2,
        destination_path: std::ptr::null(),
        password: pwd_cstr.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null()),
        thread_budget: effective_thread_budget,
        overwrite_existing: config.overwrite_existing,
        preserve_permissions: config.preserve_permissions,
        dry_run: false,
        progress_callback: Some(progress_cb),
        user_data: raw_ctx,
    };

    let bytes = crate::archive::unified::extract::extract_archive_with_metrics(src, dst, &options)
        .map_err(|s| match s {
            crate::types::TTZipStatus::Cancelled => TTZipError::Cancelled,
            crate::types::TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
            crate::types::TTZipStatus::ErrOutOfMemory => TTZipError::OutOfMemory {
                details: "Extraction memory allocation limit exceeded".to_string(),
            },
            crate::types::TTZipStatus::ErrSecurityViolation => TTZipError::DecompressionBombDetected {
                reason: "Decompression ratio threshold exceeded or overlapping entry detected".to_string(),
            },
            crate::types::TTZipStatus::ErrCorruptHeader => TTZipError::CorruptHeader {
                details: "Corrupted archive header or checksum mismatch".to_string(),
                offset: 0,
            },
            _ => TTZipError::EngineError { code: s as i32 },
        })?;

    let elapsed = start.elapsed();
    let elapsed_nanos = elapsed.as_nanos() as u64;
    let elapsed_secs = elapsed.as_secs_f64().max(0.000001);
    let throughput_mbs = (bytes as f64 / (1024.0 * 1024.0)) / elapsed_secs;
    let comp_bytes = std::fs::metadata(src).map(|m| m.len()).unwrap_or(bytes);
    let space_savings_pct = if bytes > 0 && comp_bytes <= bytes {
        ((bytes - comp_bytes) as f64 / bytes as f64) * 100.0
    } else {
        0.0
    };

    Ok(CompressionReport {
        uncompressed_bytes: bytes,
        compressed_bytes: comp_bytes,
        elapsed_nanos,
        throughput_mbs,
        space_savings_pct,
        engine_provenance: "Mozilla UniFFI Native Core Pipeline".to_string(),
    })
}

/// Extracts a single entry stream preview directly by relative entry path.
#[uniffi::export]
pub fn extract_single_entry_by_path(
    archive_path: String,
    entry_path: String,
    password: Option<String>,
) -> Result<Vec<u8>, TTZipError> {
    let secure_pwd = password.map(crate::crypto::ZeroizingString::new);
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
    const DEFAULT_EXTRACTION_BUDGET_BYTES: u64 = 64 * 1024 * 1024; // Strict 64MB limit

    if mapped.starts_with(b"7z\xBC\xAF\x27\x1C") {
        let arch = crate::sevenz::decoder::SevenZArchive::open_slice(mapped)
            .map_err(|_| TTZipError::CorruptHeader { details: "Invalid 7z header".to_string(), offset: 0 })?;
        
        let found_idx = arch.files().iter().position(|f| {
            f.rel_path == clean_target || f.rel_path.ends_with(&format!("/{}", clean_target))
        }).ok_or_else(|| TTZipError::FileNotFound { path: entry_path.clone() })?;

        if let Some(loc) = arch.seek_index().entries.get(found_idx) {
            if loc.uncompressed_size > DEFAULT_EXTRACTION_BUDGET_BYTES {
                return Err(TTZipError::OutOfMemory {
                    details: format!(
                        "Entry uncompressed size ({} bytes) exceeds safety budget ({} bytes)",
                        loc.uncompressed_size, DEFAULT_EXTRACTION_BUDGET_BYTES
                    ),
                });
            }
        }

        let budget_bytes = DEFAULT_EXTRACTION_BUDGET_BYTES;
        crate::sevenz::decoder::stream::extract_entry_bytes_stream_bounded(
            mapped,
            arch.info(),
            arch.seek_index(),
            found_idx,
            secure_pwd.as_deref(),
            budget_bytes,
        ).map_err(|status| match status {
            crate::types::TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
            crate::types::TTZipStatus::ErrSolidBudgetExceeded => TTZipError::DecompressionBombDetected {
                reason: "7z solid decompression budget exceeded".to_string(),
            },
            crate::types::TTZipStatus::ErrOutOfMemory => TTZipError::OutOfMemory {
                details: "Memory allocation failed during 7z decompression".to_string(),
            },
            crate::types::TTZipStatus::ErrCorruptHeader => TTZipError::CorruptHeader {
                details: "Corrupted 7z payload or CRC mismatch".to_string(),
                offset: 0,
            },
            crate::types::TTZipStatus::ErrSecurityViolation => TTZipError::DecompressionBombDetected {
                reason: "Security policy boundary violation".to_string(),
            },
            _ => TTZipError::EngineError { code: status as i32 },
        })
    } else if let Ok(zip_archive) = crate::zip::reader::ZipArchive::open_slice(mapped) {
        let found_idx = zip_archive.entries().iter().position(|e| {
            e.rel_path == clean_target || e.rel_path.ends_with(&format!("/{}", clean_target))
        }).ok_or_else(|| TTZipError::FileNotFound { path: entry_path.clone() })?;

        let entry = &zip_archive.entries()[found_idx];
        if entry.uncompressed_size > DEFAULT_EXTRACTION_BUDGET_BYTES {
            return Err(TTZipError::OutOfMemory {
                details: format!(
                    "Entry uncompressed size ({} bytes) exceeds safety budget ({} bytes)",
                    entry.uncompressed_size, DEFAULT_EXTRACTION_BUDGET_BYTES
                ),
            });
        }

        zip_archive.extract_entry_bytes(found_idx, secure_pwd.as_deref())
            .map_err(|status| match status {
                crate::types::TTZipStatus::ErrCorruptHeader => {
                    TTZipError::CorruptHeader { details: "Corrupted entry CRC or payload".to_string(), offset: 0 }
                }
                crate::types::TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
                crate::types::TTZipStatus::ErrOutOfMemory => TTZipError::OutOfMemory {
                    details: "Memory allocation failed during ZIP decompression".to_string(),
                },
                crate::types::TTZipStatus::ErrSecurityViolation => TTZipError::DecompressionBombDetected {
                    reason: "Decompression ratio or overlap violation detected".to_string(),
                },
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
