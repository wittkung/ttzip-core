// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Thread-local staging slots, progress bridging, and option builders for UniFFI archive creation.

use std::sync::Arc;
use super::{UniFFICreateArchiveConfig, UniFFICreateArchiveExtraOptions};
use super::super::types::{
    ArchiveFormat, CancellationToken, CompressionReport, ProgressHandler, TTZipError,
    UniFFICreateArchiveTuningOptions,
};

pub(super) struct CreateProgressContext {
    pub handler: Option<Box<dyn ProgressHandler>>,
    pub token: Option<Arc<CancellationToken>>,
    pub override_total_bytes: Option<u64>,
}

pub(super) struct CreateProgressGuard(pub *mut libc::c_void);
impl Drop for CreateProgressGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                drop(Arc::from_raw(self.0 as *const CreateProgressContext));
            }
        }
    }
}

pub(super) unsafe extern "C" fn create_progress_cb(
    processed: u64,
    total: u64,
    entry_name: *const libc::c_char,
    user_data: *mut libc::c_void,
) -> bool {
    if user_data.is_null() {
        return true;
    }
    let p = unsafe { &*(user_data as *const CreateProgressContext) };
    if p.token.as_ref().is_some_and(|t| t.is_cancelled()) {
        return false;
    }
    if let Some(ref h) = p.handler {
        let name = (!entry_name.is_null())
            .then(|| unsafe { std::ffi::CStr::from_ptr(entry_name) }.to_string_lossy().into_owned());
        if !h.on_progress(processed, p.override_total_bytes.unwrap_or(total), name) {
            return false;
        }
    }
    !p.token.as_ref().is_some_and(|t| t.is_cancelled())
}

pub(super) fn map_create_level(level: i32) -> crate::types::TTZipCompressionLevel {
    match level {
        0 => crate::types::TTZipCompressionLevel::Store,
        1 => crate::types::TTZipCompressionLevel::Fastest,
        2..=4 => crate::types::TTZipCompressionLevel::Fast,
        5..=6 => crate::types::TTZipCompressionLevel::Normal,
        7..=9 => crate::types::TTZipCompressionLevel::Maximum,
        10..=22 => crate::types::TTZipCompressionLevel::Ultra,
        _ => crate::types::TTZipCompressionLevel::Normal,
    }
}

pub(super) fn calculate_paths_uncompressed_bytes(paths: &[std::path::PathBuf]) -> u64 {
    fn calculate_single(path: &std::path::Path) -> u64 {
        if path.is_file() {
            std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
        } else if path.is_dir() {
            std::fs::read_dir(path)
                .ok()
                .into_iter()
                .flatten()
                .flatten()
                .map(|entry| calculate_single(&entry.path()))
                .sum()
        } else {
            0
        }
    }
    paths.iter().map(|p| calculate_single(p)).sum()
}

pub(super) fn finalize_compression_report(
    uncompressed_bytes: u64,
    comp_size: u64,
    start: std::time::Instant,
    provenance: &str,
    outcome: Option<super::super::extraction_session::ExtractionOutcome>,
) -> CompressionReport {
    let elapsed = start.elapsed();
    let elapsed_nanos = elapsed.as_nanos() as u64;
    let elapsed_secs = elapsed.as_secs_f64().max(0.000001);
    let benchmark_bytes = if uncompressed_bytes > 0 { uncompressed_bytes } else { comp_size };
    let throughput_mbs = (benchmark_bytes as f64 / (1024.0 * 1024.0)) / elapsed_secs;
    let space_savings_pct = if uncompressed_bytes > 0 && comp_size <= uncompressed_bytes {
        ((uncompressed_bytes - comp_size) as f64 / uncompressed_bytes as f64) * 100.0
    } else {
        0.0
    };
    CompressionReport {
        uncompressed_bytes,
        compressed_bytes: comp_size,
        elapsed_nanos,
        throughput_mbs,
        space_savings_pct,
        engine_provenance: provenance.to_string(),
        outcome,
    }
}

std::thread_local! {
    static STAGED_NEXT_CREATE_EXTRA_OPTIONS: std::cell::Cell<Option<UniFFICreateArchiveExtraOptions>> =
        const { std::cell::Cell::new(None) };
    static STAGED_NEXT_CREATE_RECOVERY_PERCENT: std::cell::Cell<u8> =
        const { std::cell::Cell::new(0) };
    static STAGED_NEXT_CREATE_TUNING_OPTIONS: std::cell::RefCell<Option<UniFFICreateArchiveTuningOptions>> =
        const { std::cell::RefCell::new(None) };
}

pub(super) fn stage_extra_options(extra_options: UniFFICreateArchiveExtraOptions) {
    STAGED_NEXT_CREATE_EXTRA_OPTIONS.with(|slot| {
        slot.set(Some(extra_options));
    });
}

pub(super) fn take_staged_create_options() -> (
    UniFFICreateArchiveExtraOptions,
    u8,
    UniFFICreateArchiveTuningOptions,
) {
    let extra = STAGED_NEXT_CREATE_EXTRA_OPTIONS
        .with(|slot| slot.take())
        .unwrap_or_default();
    let recovery_pct = STAGED_NEXT_CREATE_RECOVERY_PERCENT.with(|slot| slot.replace(0));
    let tuning = STAGED_NEXT_CREATE_TUNING_OPTIONS
        .with(|slot| slot.borrow_mut().take())
        .unwrap_or_default();
    (extra, recovery_pct, tuning)
}

/// Stages typed advanced tuning options for the next `create_archive_advanced` invocation on the calling thread.
pub fn stage_next_create_tuning_options(tuning: UniFFICreateArchiveTuningOptions) {
    STAGED_NEXT_CREATE_TUNING_OPTIONS.with(|slot| {
        *slot.borrow_mut() = Some(tuning);
    });
}

/// C-ABI bridge allowing `ArchiveWriter.swift` to stage typed extra options for the next
/// `create_archive_advanced` invocation on the calling thread without mutating frozen `repr(C)` layouts.
/// `encrypt_file_names_mode`: `1` => `Some(true)`, `0` => `Some(false)`, `< 0` => `None`.
#[unsafe(no_mangle)]
pub extern "C" fn ttzip_uniffi_stage_next_create_extra_options(encrypt_file_names_mode: i32) {
    let encrypt_file_names = match encrypt_file_names_mode {
        1 => Some(true),
        0 => Some(false),
        _ => None,
    };
    stage_extra_options(UniFFICreateArchiveExtraOptions { encrypt_file_names });
}

/// C-ABI bridge allowing `ArchiveWriter.swift` to stage the recovery record redundancy percentage
/// (`0` = disabled, `1..=50` = enabled) for the next `create_archive_advanced` invocation on the calling thread.
#[unsafe(no_mangle)]
pub extern "C" fn ttzip_uniffi_stage_next_create_recovery_record(redundancy_percent: u8) {
    STAGED_NEXT_CREATE_RECOVERY_PERCENT.with(|slot| slot.set(redundancy_percent));
}

/// C-ABI bridge allowing `ArchiveWriter.swift` to stage advanced codec/container tuning options
/// for the next `create_archive_advanced` invocation on the calling thread without mutating frozen UniFFI checksums.
#[unsafe(no_mangle)]
pub extern "C" fn ttzip_uniffi_stage_next_create_tuning_options(
    dictionary_size_mb: u32,
    enable_solid_archive: bool,
    use_zip_crypto: bool,
    zstd_level: u8,
    enable_ldm: bool,
) {
    let zip_encryption_method = if use_zip_crypto {
        "ZipCrypto".to_string()
    } else {
        "AES-256".to_string()
    };
    stage_next_create_tuning_options(UniFFICreateArchiveTuningOptions {
        dictionary_size_mb,
        enable_solid_archive,
        zip_encryption_method,
        zstd_level: (zstd_level > 0).then_some(zstd_level),
        enable_ldm,
    });
}

pub(super) fn plan_advanced_create_entries(
    config: &UniFFICreateArchiveConfig,
    token: Option<&Arc<CancellationToken>>,
) -> Result<(Vec<(std::path::PathBuf, String)>, u64), TTZipError> {
    let exc_refs: Vec<&str> = config.exclude_patterns.iter().map(|s| s.as_str()).collect();
    let inc_refs: Vec<&str> = config.include_patterns.iter().map(|s| s.as_str()).collect();
    let filter = crate::fs::filter::pattern::PathPatternFilter::new(
        &inc_refs,
        &exc_refs,
        config.exclude_vcs,
        config.no_mac_metadata,
    )
    .map_err(|e| TTZipError::IoError { message: e })?;

    let mut planned_entries: Vec<(std::path::PathBuf, String)> = Vec::new();
    let mut total_uncompressed_bytes: u64 = 0;

    for src_str in &config.source_paths {
        if token.is_some_and(|t| t.is_cancelled()) {
            return Err(TTZipError::Cancelled);
        }
        let src_path = std::path::Path::new(src_str);
        let sym_meta = std::fs::symlink_metadata(src_path)
            .map_err(|_| TTZipError::FileNotFound { path: src_str.clone() })?;
        let (effective_src, meta) = if sym_meta.file_type().is_symlink() {
            (src_path.to_path_buf(), sym_meta)
        } else {
            let canonical_src =
                std::fs::canonicalize(src_path).unwrap_or_else(|_| src_path.to_path_buf());
            let m = std::fs::metadata(&canonical_src).unwrap_or(sym_meta);
            (canonical_src, m)
        };

        if meta.is_dir() && !meta.file_type().is_symlink() {
            let scan_opts = crate::fs::scanner::ScanOptions {
                include_hidden: true,
                skip_mac_junk: config.no_mac_metadata,
                max_depth: 0,
                thread_budget: config.thread_budget,
            };
            for item in crate::fs::scanner::scan_directory_parallel(&effective_src, &scan_opts) {
                if filter.should_include(&item.rel_path) {
                    total_uncompressed_bytes =
                        total_uncompressed_bytes.saturating_add(item.file_size);
                    planned_entries.push((std::path::PathBuf::from(item.src_path), item.rel_path));
                }
            }
        } else {
            let file_name = src_path.file_name().and_then(|n| n.to_str()).unwrap_or(src_str);
            if filter.should_include(file_name) {
                let size = if meta.file_type().is_symlink() { 0 } else { meta.len() };
                total_uncompressed_bytes = total_uncompressed_bytes.saturating_add(size);
                planned_entries.push((effective_src, file_name.to_string()));
            }
        }
    }
    if planned_entries.is_empty() {
        return Err(TTZipError::EngineError { code: -404 });
    }
    Ok((planned_entries, total_uncompressed_bytes))
}

pub(super) fn build_advanced_create_options(
    config: &UniFFICreateArchiveConfig,
    tuning: &UniFFICreateArchiveTuningOptions,
    pwd_ptr: Option<*const libc::c_char>,
    raw_ctx: *mut libc::c_void,
) -> (crate::types::TTZipCreateOptions, crate::types::TTZipExtendedCreateOptions) {
    let raw_lvl = match config.format {
        ArchiveFormat::TarZstd | ArchiveFormat::Zstd => tuning
            .zstd_level
            .filter(|&z| z != 3 || config.level == 3)
            .or_else(|| u8::try_from(config.level.clamp(0, 22)).ok()),
        _ => u8::try_from(config.level.clamp(0, 22)).ok(),
    };
    let effective_i32_level = raw_lvl.map(i32::from).unwrap_or(config.level);
    let encryption = if pwd_ptr.is_some() {
        if tuning.zip_encryption_method.eq_ignore_ascii_case("ZipCrypto") {
            crate::types::TTZipEncryptionMethod::ZipCrypto
        } else {
            crate::types::TTZipEncryptionMethod::Aes256
        }
    } else {
        crate::types::TTZipEncryptionMethod::None
    };
    let options = crate::types::TTZipCreateOptions {
        struct_size: std::mem::size_of::<crate::types::TTZipCreateOptions>() as u32,
        abi_version: crate::types::TTZIP_ABI_VERSION_3,
        format: config.format.into(),
        level: map_create_level(effective_i32_level),
        encryption,
        password: pwd_ptr.unwrap_or(std::ptr::null()),
        thread_budget: config.thread_budget,
        solid_block_size_mb: if tuning.enable_solid_archive { 64 } else { 0 },
        progress_callback: Some(create_progress_cb),
        user_data: raw_ctx,
    };
    let ext_opts = crate::types::TTZipExtendedCreateOptions {
        dictionary_size_mb: tuning.dictionary_size_mb,
        enable_ldm: tuning.enable_ldm,
        raw_level: raw_lvl,
    };
    (options, ext_opts)
}
