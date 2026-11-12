// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Mozilla UniFFI Archive Creation, Inspection, and Format Detection Scaffolding.

#![allow(
    deprecated,
    reason = "UniFFI scaffolding exports deprecated v1 compatibility endpoints"
)]

use std::sync::Arc;
use super::types::{
    ArchiveFormat, CancellationToken, CompressionReport, PasswordRecoveryOutcome, ProgressHandler,
    SniffMetadata, TTZipArchiveFormat, TTZipError, UniFFIEntryMetadata,
};

#[path = "archive_staging.rs"]
mod archive_staging;
pub use archive_staging::{
    stage_next_create_tuning_options, ttzip_uniffi_stage_next_create_extra_options,
    ttzip_uniffi_stage_next_create_recovery_record, ttzip_uniffi_stage_next_create_tuning_options,
};
use archive_staging::{
    build_advanced_create_options, calculate_paths_uncompressed_bytes, create_progress_cb,
    finalize_compression_report, map_create_level, plan_advanced_create_entries,
    take_staged_create_options, CreateProgressContext, CreateProgressGuard,
};

/// Detects archive format from file using the full 16-format magic and SFX sniffer.
#[uniffi::export]
pub fn detect_archive_format(path: String) -> Result<ArchiveFormat, TTZipError> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path });
    }

    let sniff = crate::standards::detect_format_file(p)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;

    let core_fmt = sniff.compound_format.unwrap_or(sniff.format);
    Ok(ArchiveFormat::from(core_fmt))
}

/// Sniffs format and metadata from in-memory byte buffer.
#[uniffi::export]
pub fn sniff_format_buffer(data: Vec<u8>, filename_hint: Option<String>) -> SniffMetadata {
    let sniff = crate::standards::detect_format_buffer(&data, filename_hint.as_deref());
    let is_archive = sniff.format != crate::standards::signatures::DetectedFormat::Unknown;
    SniffMetadata {
        format_name: sniff.description.to_string(),
        mime_type: sniff.mime_type.to_string(),
        is_archive,
        is_sfx: sniff.is_sfx,
        sfx_offset: sniff.sfx_offset as u64,
        confidence: sniff.confidence as u32,
    }
}

/// Sniffs format and metadata from file on disk.
#[uniffi::export]
pub fn sniff_format_file(path: String) -> Result<SniffMetadata, TTZipError> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path });
    }
    let sniff = crate::standards::detect_format_file(p)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    let is_archive = sniff.format != crate::standards::signatures::DetectedFormat::Unknown;
    Ok(SniffMetadata {
        format_name: sniff.description.to_string(),
        mime_type: sniff.mime_type.to_string(),
        is_archive,
        is_sfx: sniff.is_sfx,
        sfx_offset: sniff.sfx_offset as u64,
        confidence: sniff.confidence as u32,
    })
}

/// Measures Shannon entropy of byte data using SIMD.
#[uniffi::export]
pub fn estimate_shannon_entropy(data: Vec<u8>) -> f64 {
    crate::analytics::compute_shannon_entropy(&data)
}

/// Recommends codec algorithm name based on entropy and scenario.
#[uniffi::export]
pub fn recommend_codec(data: Vec<u8>, scenario: i32) -> String {
    let sc = match scenario {
        0 => crate::analytics::Scenario::InstantTransfer,
        2 => crate::analytics::Scenario::ColdStorage,
        _ => crate::analytics::Scenario::BalancedDaily,
    };
    crate::analytics::CascadedCodecSelector::recommend(&data, sc).recommended_algorithm.to_string()
}

/// Inspects all archive entry metadata items.
///
/// # Deprecated
/// Legacy monolithic entry inspection allocating intermediate structs for all entries.
/// For UI tree construction and navigation, use `VfsSession` / `load_archive_vfs_tree`.
#[allow(deprecated, reason = "Retained for UniFFI backward compatibility")]
#[deprecated(
    since = "0.2.0",
    note = "Monolithic entry inspection is deprecated. Use VfsSession or load_archive_vfs_tree for windowed VFS navigation."
)]
#[uniffi::export]
pub fn inspect_archive_entries(
    archive_path: String,
    password: Option<String>,
) -> Result<Vec<UniFFIEntryMetadata>, TTZipError> {
    let secure_pwd = password.map(crate::crypto::ZeroizingString::new);
    let p = std::path::Path::new(&archive_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

    let mut entries = Vec::new();

    let res = crate::archive::unified::inspect::inspect_archive(
        p,
        secure_pwd.as_deref(),
        true,
        |entry_ref| {
            let method = match entry_ref.compression_method {
                0 => "store",
                8 => "deflate",
                12 => "bzip2",
                14 => "lzma",
                93 | 20 => "zstd",
                95 => "xz",
                _ => "unknown",
            }.to_string();

            entries.push(UniFFIEntryMetadata {
                path: entry_ref.path.to_string(),
                uncompressed_size: entry_ref.uncompressed_size,
                compressed_size: entry_ref.compressed_size,
                crc32: entry_ref.crc32,
                mtime_epoch_secs: entry_ref.mtime_epoch_secs,
                mode: entry_ref.mode,
                is_directory: entry_ref.is_directory,
                is_encrypted: entry_ref.is_encrypted,
                compression_method: method,
                detected_encoding: entry_ref.detected_encoding.map(|s| s.to_string()),
            });
            true
        },
    );

    match res {
        Ok(_) => Ok(entries),
        Err(status) => {
            if status == crate::types::TTZipStatus::ErrInvalidPassword {
                Err(TTZipError::InvalidPassword)
            } else {
                Err(TTZipError::EngineError { code: status as i32 })
            }
        }
    }
}

/// Extracts a single entry stream preview from a solid or non-solid archive.
#[uniffi::export]
pub fn extract_single_entry_stream(
    archive_path: String,
    entry_index: u64,
    password: Option<String>,
) -> Result<Vec<u8>, TTZipError> {
    extract_single_entry_stream_guarded(archive_path, entry_index, password, 64)
}

/// Extracts a single entry stream preview with configurable preceding solid budget in MB.
#[uniffi::export]
pub fn extract_single_entry_stream_guarded(
    archive_path: String,
    entry_index: u64,
    password: Option<String>,
    max_preceding_budget_mb: u32,
) -> Result<Vec<u8>, TTZipError> {
    let secure_pwd = password.map(crate::crypto::ZeroizingString::new);
    let p = std::path::Path::new(&archive_path);
    let volume_chain = crate::archive::split::detect_volume_chain(p)
        .map_err(|_| TTZipError::FileNotFound {
            path: archive_path.clone(),
        })?;
    if volume_chain.len() > 1 {
        return super::extraction::extract_single_entry_split_guarded(
            &volume_chain,
            p,
            None,
            entry_index as i64,
            secure_pwd.as_deref(),
            max_preceding_budget_mb as u64,
        );
    }
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

    let source = crate::archive::source::open_archive_source(p)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    let mapped = source.as_slice().ok_or_else(|| TTZipError::IoError {
        message: "Failed to map archive bytes".to_string(),
    })?;

    let budget_bytes = (max_preceding_budget_mb as u64) * 1024 * 1024;
    let idx = entry_index as usize;
    let missing_label = format!("entry #{entry_index}");

    if mapped.starts_with(b"7z\xBC\xAF\x27\x1C") {
        let arch = super::extraction::open_sevenz_slice_for_preview(mapped, secure_pwd.as_deref())?;
        if idx >= arch.files().len() {
            return Err(TTZipError::FileNotFound { path: missing_label });
        }
        if let Some(loc) = arch.seek_index().entries.get(idx) {
            super::extraction::enforce_7z_entry_budget(loc, budget_bytes)?;
        }
        crate::sevenz::decoder::stream::extract_entry_bytes_stream_bounded(
            mapped,
            arch.info(),
            arch.seek_index(),
            idx,
            secure_pwd.as_deref(),
            budget_bytes,
        )
        .map_err(|s| super::extraction::map_single_entry_status(s, &missing_label))
    } else if let Ok(zip_archive) = crate::zip::reader::ZipArchive::open_slice(mapped) {
        if idx >= zip_archive.entries().len() {
            return Err(TTZipError::FileNotFound { path: missing_label });
        }
        let entry = &zip_archive.entries()[idx];
        if entry.uncompressed_size > budget_bytes {
            return Err(TTZipError::EngineError {
                code: crate::types::TTZipStatus::ErrOutOfMemory as i32,
            });
        }
        zip_archive
            .extract_entry_bytes(idx, secure_pwd.as_deref())
            .map_err(|s| super::extraction::map_single_entry_status(s, &missing_label))
    } else {
        crate::archive::unified::extract_single::extract_single_entry_memory(
            p,
            None,
            entry_index as i64,
            secure_pwd.as_deref(),
        )
        .map_err(|s| super::extraction::map_single_entry_status(s, &missing_label))
    }
}

fn validate_create_archive_inputs(
    source_paths: &[String],
    password: Option<&str>,
    token: Option<&Arc<CancellationToken>>,
) -> Result<Vec<std::path::PathBuf>, TTZipError> {
    if token.is_some_and(|t| t.is_cancelled()) {
        return Err(TTZipError::Cancelled);
    }
    if source_paths.is_empty() {
        return Err(TTZipError::EngineError { code: -404 });
    }
    for src_str in source_paths {
        let src_p = std::path::Path::new(src_str);
        if !src_p.exists() && std::fs::symlink_metadata(src_p).is_err() {
            return Err(TTZipError::FileNotFound {
                path: src_str.clone(),
            });
        }
    }
    if password == Some("") {
        return Err(TTZipError::InvalidPassword);
    }
    Ok(source_paths.iter().map(std::path::PathBuf::from).collect())
}

/// Compresses source paths into destination archive.
#[uniffi::export]
pub fn create_archive_stream(
    source_paths: Vec<String>,
    output_path: String,
    format: ArchiveFormat,
    level: i32,
    password: Option<String>,
    progress: Option<Box<dyn ProgressHandler>>,
    token: Option<Arc<CancellationToken>>,
) -> Result<CompressionReport, TTZipError> {
    let paths =
        validate_create_archive_inputs(&source_paths, password.as_deref(), token.as_ref())?;
    let out_p = std::path::Path::new(&output_path);
    let secure_pwd = password.map(crate::crypto::ZeroizingString::new);
    let start = std::time::Instant::now();
    let pwd_cstr = secure_pwd.as_ref().and_then(|p| p.to_c_string().ok());
    let token_for_probe = token.clone();
    let raw_ctx = Arc::into_raw(Arc::new(CreateProgressContext {
        handler: progress,
        token,
        override_total_bytes: None,
    })) as *mut libc::c_void;
    let _guard = CreateProgressGuard(raw_ctx);

    let options = crate::types::TTZipCreateOptions {
        struct_size: std::mem::size_of::<crate::types::TTZipCreateOptions>() as u32,
        abi_version: crate::types::TTZIP_ABI_VERSION_3,
        format: format.into(),
        level: map_create_level(level),
        encryption: if pwd_cstr.is_some() {
            crate::types::TTZipEncryptionMethod::Aes256
        } else {
            crate::types::TTZipEncryptionMethod::None
        },
        password: pwd_cstr.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null()),
        thread_budget: 0,
        solid_block_size_mb: 64,
        progress_callback: Some(create_progress_cb),
        user_data: raw_ctx,
    };

    let ext_opts = crate::types::TTZipExtendedCreateOptions {
        dictionary_size_mb: 0,
        enable_ldm: false,
        raw_level: u8::try_from(level.clamp(0, 22)).ok(),
    };
    let commit_out = crate::types::with_extended_create_options(ext_opts, || {
        crate::archive::unified::create::create_archive_detailed_outcome_with_cancel(
            &paths,
            out_p,
            &options,
            0,
            move || token_for_probe.as_ref().is_some_and(|t| t.is_cancelled()),
        )
    })
    .map_err(map_create_stage_commit_error)?;
    let outcome = build_post_commit_cleanup_outcome_from_issues(
        commit_out.committed_files.len() as u64,
        commit_out.post_commit_cleanup_issues,
    );
    let comp_size = std::fs::metadata(out_p).map(|m| m.len()).unwrap_or(0);
    let uncompressed_bytes = calculate_paths_uncompressed_bytes(&paths);
    Ok(finalize_compression_report(
        uncompressed_bytes,
        comp_size,
        start,
        "Mozilla UniFFI Native Create Pipeline",
        outcome,
    ))
}

use super::error_mapping::{
    build_post_commit_cleanup_outcome_from_issues, map_create_stage_commit_error,
};

/// Advanced archive creation configuration parameters exposed via UniFFI.
#[derive(Clone, Debug, uniffi::Record)]
pub struct UniFFICreateArchiveConfig {
    pub source_paths: Vec<String>,
    pub output_path: String,
    pub format: TTZipArchiveFormat,
    pub level: i32,
    pub password: Option<String>,
    pub split_volume_size_bytes: Option<u64>,
    pub exclude_patterns: Vec<String>,
    pub include_patterns: Vec<String>,
    pub exclude_vcs: bool,
    pub no_mac_metadata: bool,
    pub thread_budget: u32,
}

/// Typed extension options for advanced archive creation, preserving `UniFFICreateArchiveConfig` ABI checksums.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, uniffi::Record)]
pub struct UniFFICreateArchiveExtraOptions {
    pub encrypt_file_names: Option<bool>,
}

const INLINE_TUNING_TAG: i32 = 1 << 30;

fn decode_inline_create_config(
    config: &mut UniFFICreateArchiveConfig,
    recovery_redundancy_pct: &mut u8,
    tuning: &mut super::types::UniFFICreateArchiveTuningOptions,
) {
    if (config.level & INLINE_TUNING_TAG) == 0 {
        return;
    }
    let packed = config.level;
    let base_level = packed & 0x3F;
    let zstd_level = ((packed >> 6) & 0x3F) as u8;
    let recovery_pct = ((packed >> 12) & 0xFF) as u8;
    let enable_solid_archive = ((packed >> 20) & 1) != 0;
    let use_zip_crypto = ((packed >> 21) & 1) != 0;
    let enable_ldm = ((packed >> 22) & 1) != 0;

    let cpu_threads = config.thread_budget & 0xFFFF;
    let dictionary_size_mb = (config.thread_budget >> 16) & 0xFFFF;

    config.level = base_level;
    config.thread_budget = cpu_threads;
    *recovery_redundancy_pct = recovery_pct;
    *tuning = super::types::UniFFICreateArchiveTuningOptions {
        dictionary_size_mb,
        enable_solid_archive,
        zip_encryption_method: if use_zip_crypto {
            "ZipCrypto".to_string()
        } else {
            "AES-256".to_string()
        },
        zstd_level: (zstd_level > 0).then_some(zstd_level),
        enable_ldm,
    };
}

/// Advanced archive creation pipeline with explicit typed extension options (`UniFFICreateArchiveExtraOptions`).
#[uniffi::export]
pub fn create_archive_advanced_with_options(
    config: UniFFICreateArchiveConfig,
    extra_options: UniFFICreateArchiveExtraOptions,
    progress: Option<Box<dyn ProgressHandler>>,
    token: Option<Arc<CancellationToken>>,
) -> Result<CompressionReport, TTZipError> {
    create_archive_advanced_inner(config, Some(extra_options), progress, token)
}

/// Advanced archive creation pipeline with single-pass parallel scanning, filtering,
/// split volume output, and atomic transactional cleanup.
#[uniffi::export]
pub fn create_archive_advanced(
    config: UniFFICreateArchiveConfig,
    progress: Option<Box<dyn ProgressHandler>>,
    token: Option<Arc<CancellationToken>>,
) -> Result<CompressionReport, TTZipError> {
    create_archive_advanced_inner(config, None, progress, token)
}

fn validate_advanced_create_config(
    config: &UniFFICreateArchiveConfig,
    extra_options: UniFFICreateArchiveExtraOptions,
    token: Option<&Arc<CancellationToken>>,
) -> Result<(), TTZipError> {
    if token.is_some_and(|t| t.is_cancelled()) {
        return Err(TTZipError::Cancelled);
    }
    if config.source_paths.is_empty() {
        return Err(TTZipError::EngineError { code: -404 });
    }
    if config.password.as_deref() == Some("") {
        return Err(TTZipError::InvalidPassword);
    }
    let has_non_empty_password = config.password.as_ref().is_some_and(|s| !s.is_empty());
    if extra_options.encrypt_file_names == Some(true) && !has_non_empty_password {
        return Err(TTZipError::InvalidPassword);
    }
    Ok(())
}

fn create_archive_advanced_inner(
    mut config: UniFFICreateArchiveConfig,
    explicit_extra_options: Option<UniFFICreateArchiveExtraOptions>,
    progress: Option<Box<dyn ProgressHandler>>,
    token: Option<Arc<CancellationToken>>,
) -> Result<CompressionReport, TTZipError> {
    let (staged_extra_options, mut recovery_redundancy_pct, mut tuning) =
        take_staged_create_options();
    let extra_options = explicit_extra_options.unwrap_or(staged_extra_options);
    decode_inline_create_config(&mut config, &mut recovery_redundancy_pct, &mut tuning);
    validate_advanced_create_config(&config, extra_options, token.as_ref())?;

    let out_path = std::path::Path::new(&config.output_path);
    let parent = out_path.parent().unwrap_or_else(|| std::path::Path::new("."));
    if !parent.exists() {
        let _ = std::fs::create_dir_all(parent);
    }

    let start = std::time::Instant::now();
    let (planned_entries, total_uncompressed_bytes) =
        plan_advanced_create_entries(&config, token.as_ref())?;

    let secure_pwd = config
        .password
        .clone()
        .filter(|s| !s.is_empty())
        .map(crate::crypto::ZeroizingString::new);
    let pwd_cstr = secure_pwd.as_ref().and_then(|p| p.to_c_string().ok());
    let token_for_probe = token.clone();
    let raw_ctx = Arc::into_raw(Arc::new(CreateProgressContext {
        handler: progress,
        token,
        override_total_bytes: Some(total_uncompressed_bytes),
    })) as *mut libc::c_void;
    let _guard = CreateProgressGuard(raw_ctx);

    let (options, ext_opts) = build_advanced_create_options(
        &config,
        &tuning,
        pwd_cstr.as_ref().map(|c| c.as_ptr()),
        raw_ctx,
    );
    let split_size = config.split_volume_size_bytes.unwrap_or(0);
    let commit_out = crate::types::with_extended_create_options(ext_opts, || {
        crate::archive::unified::create::create_archive_explicit_with_files_detailed_full_opts_outcome_with_cancel(
            &planned_entries,
            out_path,
            &options,
            split_size,
            extra_options.encrypt_file_names,
            recovery_redundancy_pct,
            move || token_for_probe.as_ref().is_some_and(|t| t.is_cancelled()),
        )
    })
    .map_err(map_create_stage_commit_error)?;
    let created_files = commit_out.committed_files;
    let outcome = build_post_commit_cleanup_outcome_from_issues(
        created_files.len() as u64,
        commit_out.post_commit_cleanup_issues,
    );
    let comp_size: u64 = created_files
        .iter()
        .map(|f| std::fs::metadata(f).map(|m| m.len()).unwrap_or(0))
        .sum();
    Ok(finalize_compression_report(
        total_uncompressed_bytes,
        comp_size,
        start,
        "Mozilla UniFFI Advanced Create Pipeline",
        outcome,
    ))
}

/// Recovers password against an encrypted archive using Rayon multi-core parallel probing.
//
// The search is a two-stage pipeline: a cheap screening stage eliminates
// candidates, and every candidate that survives it is confirmed by the
// archive-layer content verification. The archive itself is parsed once and the
// confirmation target is selected once, and both are reused for every candidate.
//
// A candidate that the confirmation rejects does not end the search. A reason that
// is not about the candidate at all — an exhausted budget, an unsupported unit, a
// missing integrity credential, a damaged container — is reported as an error
// instead of being flattened into "not found".
#[uniffi::export]
pub fn recover_archive_password(
    archive_path: String,
    dictionary: Vec<String>,
) -> Result<PasswordRecoveryOutcome, TTZipError> {
    let p = std::path::Path::new(&archive_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

    let file_bytes = std::fs::read(p).map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    let start = std::time::Instant::now();
    let search = crate::recovery::RecoverySearch::prepare(
        &file_bytes,
        crate::recovery::ConfirmationBudget::default(),
    )
    .map_err(recovery_stop_reason_to_error)?;

    let candidates: Vec<&str> = dictionary.iter().map(|candidate| candidate.as_str()).collect();
    let outcome = search.run(candidates, None);
    let total_attempts = search.attempted_candidates();

    let elapsed = start.elapsed();
    let elapsed_nanos = elapsed.as_nanos() as u64;
    let elapsed_secs = elapsed.as_secs_f64().max(0.000001);
    let attempts_per_second = (total_attempts as f64) / elapsed_secs;

    let found_password = match outcome {
        crate::recovery::RecoverySearchOutcome::Confirmed { password } => Some(password),
        crate::recovery::RecoverySearchOutcome::Exhausted => None,
        crate::recovery::RecoverySearchOutcome::Stopped(reason) => {
            return Err(recovery_stop_reason_to_error(reason));
        }
        crate::recovery::RecoverySearchOutcome::Cancelled => return Err(TTZipError::Cancelled),
    };

    Ok(PasswordRecoveryOutcome {
        found_password,
        total_attempts,
        elapsed_nanos,
        attempts_per_second,
    })
}

/// Maps an engine-level stop reason onto the public error surface.
///
/// The reasons that are not about the candidate never become `InvalidPassword` and
/// never become a "not found" outcome; they keep the status code that identifies
/// them.
fn recovery_stop_reason_to_error(reason: crate::recovery::RecoveryStopReason) -> TTZipError {
    match reason {
        crate::recovery::RecoveryStopReason::ArchiveDamaged => TTZipError::CorruptHeader {
            details: "archive container is damaged".to_string(),
            offset: 0,
        },
        other => TTZipError::EngineError {
            code: other.status_code(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_inline_create_config_extracts_tuning_and_recovery() {
        let packed_level = INLINE_TUNING_TAG
            | 6
            | (15 << 6)
            | (10 << 12)
            | (1 << 21)
            | (1 << 22);
        let packed_threads = 3u32 | (32u32 << 16);
        let mut cfg = UniFFICreateArchiveConfig {
            source_paths: vec!["a.txt".to_string()],
            output_path: "out.zst".to_string(),
            format: ArchiveFormat::Zstd,
            level: packed_level,
            password: None,
            split_volume_size_bytes: None,
            exclude_patterns: vec![],
            include_patterns: vec![],
            exclude_vcs: false,
            no_mac_metadata: false,
            thread_budget: packed_threads,
        };
        let mut recovery_pct = 0u8;
        let mut tuning = super::super::types::UniFFICreateArchiveTuningOptions::default();

        decode_inline_create_config(&mut cfg, &mut recovery_pct, &mut tuning);

        assert_eq!(cfg.level, 6);
        assert_eq!(cfg.thread_budget, 3);
        assert_eq!(recovery_pct, 10);
        assert_eq!(tuning.dictionary_size_mb, 32);
        assert!(!tuning.enable_solid_archive);
        assert_eq!(tuning.zip_encryption_method, "ZipCrypto");
        assert_eq!(tuning.zstd_level, Some(15));
        assert!(tuning.enable_ldm);
    }
}
