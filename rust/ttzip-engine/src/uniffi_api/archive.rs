// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Mozilla UniFFI Archive Creation, Inspection, and Format Detection Scaffolding.

use std::sync::Arc;
use rayon::prelude::*;
use super::types::{ArchiveFormat, CancellationToken, CompressionReport, PasswordRecoveryOutcome, ProgressHandler, SniffMetadata, TTZipArchiveFormat, TTZipError, UniFFIEntryMetadata};

/// Detects archive format from file using the full 16-format magic and SFX sniffer.
#[uniffi::export]
pub fn detect_archive_format(path: String) -> Result<ArchiveFormat, TTZipError> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path });
    }

    let sniff = crate::standards::detect_format_file(p)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;

    let fmt = match sniff.format {
        crate::standards::signatures::DetectedFormat::Zip => ArchiveFormat::Zip,
        crate::standards::signatures::DetectedFormat::SevenZip => ArchiveFormat::SevenZip,
        crate::standards::signatures::DetectedFormat::Tar => {
            match sniff.compound_format {
                Some(crate::standards::signatures::CompoundFormat::TarGz) => ArchiveFormat::TarGz,
                Some(crate::standards::signatures::CompoundFormat::TarBz2) => ArchiveFormat::TarBz2,
                Some(crate::standards::signatures::CompoundFormat::TarXz) => ArchiveFormat::TarXz,
                Some(crate::standards::signatures::CompoundFormat::TarZstd) => ArchiveFormat::TarZstd,
                Some(crate::standards::signatures::CompoundFormat::TarLz4) => ArchiveFormat::TarLz4,
                Some(crate::standards::signatures::CompoundFormat::TarBrotli) => ArchiveFormat::TarBrotli,
                Some(crate::standards::signatures::CompoundFormat::TarLzip) => ArchiveFormat::TarLzip,
                Some(crate::standards::signatures::CompoundFormat::TarLrzip) => ArchiveFormat::TarLrzip,
                _ => ArchiveFormat::Tar,
            }
        }
        crate::standards::signatures::DetectedFormat::Gzip => if sniff.compound_format == Some(crate::standards::signatures::CompoundFormat::TarGz) { ArchiveFormat::TarGz } else { ArchiveFormat::Gzip },
        crate::standards::signatures::DetectedFormat::Bzip2 => if sniff.compound_format == Some(crate::standards::signatures::CompoundFormat::TarBz2) { ArchiveFormat::TarBz2 } else { ArchiveFormat::Bzip2 },
        crate::standards::signatures::DetectedFormat::Xz => if sniff.compound_format == Some(crate::standards::signatures::CompoundFormat::TarXz) { ArchiveFormat::TarXz } else { ArchiveFormat::Xz },
        crate::standards::signatures::DetectedFormat::Zstd => if sniff.compound_format == Some(crate::standards::signatures::CompoundFormat::TarZstd) { ArchiveFormat::TarZstd } else { ArchiveFormat::Zstd },
        crate::standards::signatures::DetectedFormat::Lz4 => if sniff.compound_format == Some(crate::standards::signatures::CompoundFormat::TarLz4) { ArchiveFormat::TarLz4 } else { ArchiveFormat::Lz4 },
        crate::standards::signatures::DetectedFormat::Brotli => if sniff.compound_format == Some(crate::standards::signatures::CompoundFormat::TarBrotli) { ArchiveFormat::TarBrotli } else { ArchiveFormat::Brotli },
        crate::standards::signatures::DetectedFormat::Lzip => if sniff.compound_format == Some(crate::standards::signatures::CompoundFormat::TarLzip) { ArchiveFormat::TarLzip } else { ArchiveFormat::Lzip },
        crate::standards::signatures::DetectedFormat::Lrzip => if sniff.compound_format == Some(crate::standards::signatures::CompoundFormat::TarLrzip) { ArchiveFormat::TarLrzip } else { ArchiveFormat::Lrzip },
        crate::standards::signatures::DetectedFormat::Dmg => ArchiveFormat::Dmg,
        crate::standards::signatures::DetectedFormat::Lzfse => ArchiveFormat::Lzfse,
        crate::standards::signatures::DetectedFormat::Snappy => ArchiveFormat::Snappy,
        crate::standards::signatures::DetectedFormat::Iso => ArchiveFormat::Iso,
        crate::standards::signatures::DetectedFormat::Cab => ArchiveFormat::Cab,
        crate::standards::signatures::DetectedFormat::Wim => ArchiveFormat::Wim,
        crate::standards::signatures::DetectedFormat::Rar => ArchiveFormat::Rar,
        crate::standards::signatures::DetectedFormat::Aar => ArchiveFormat::Aar,
        crate::standards::signatures::DetectedFormat::Cpio => ArchiveFormat::Cpio,
        crate::standards::signatures::DetectedFormat::Ar => ArchiveFormat::Ar,
        crate::standards::signatures::DetectedFormat::Deb => ArchiveFormat::Deb,
        crate::standards::signatures::DetectedFormat::Rpm => ArchiveFormat::Rpm,
        crate::standards::signatures::DetectedFormat::Xar => ArchiveFormat::Xar,
        crate::standards::signatures::DetectedFormat::Squashfs => ArchiveFormat::Squashfs,
        crate::standards::signatures::DetectedFormat::Lzh => ArchiveFormat::Lzh,
        _ => ArchiveFormat::Auto,
    };
    Ok(fmt)
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

    if mapped.starts_with(b"7z\xBC\xAF\x27\x1C") {
        let arch = crate::sevenz::decoder::SevenZArchive::open_slice(mapped)
            .map_err(|_| TTZipError::CorruptHeader { details: "Invalid 7z header".to_string(), offset: 0 })?;
        if idx >= arch.files().len() {
            return Err(TTZipError::FileNotFound {
                path: format!("entry #{}", entry_index),
            });
        }
        if let Some(loc) = arch.seek_index().entries.get(idx) {
            if loc.uncompressed_size > budget_bytes {
                return Err(TTZipError::EngineError {
                    code: crate::types::TTZipStatus::ErrOutOfMemory as i32,
                });
            }
        }
        crate::sevenz::decoder::stream::extract_entry_bytes_stream_bounded(
            mapped,
            arch.info(),
            arch.seek_index(),
            idx,
            secure_pwd.as_deref(),
            budget_bytes,
        ).map_err(|status| match status {
            crate::types::TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
            crate::types::TTZipStatus::ErrSolidBudgetExceeded => TTZipError::EngineError { code: -24 },
            _ => TTZipError::EngineError { code: status as i32 },
        })
    } else if let Ok(zip_archive) = crate::zip::reader::ZipArchive::open_slice(mapped) {
        if idx >= zip_archive.entries().len() {
            return Err(TTZipError::FileNotFound {
                path: format!("entry #{}", entry_index),
            });
        }
        let entry = &zip_archive.entries()[idx];
        if entry.uncompressed_size > budget_bytes {
            return Err(TTZipError::EngineError {
                code: crate::types::TTZipStatus::ErrOutOfMemory as i32,
            });
        }
        zip_archive.extract_entry_bytes(idx, secure_pwd.as_deref())
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
    if source_paths.is_empty() {
        return Err(TTZipError::EngineError { code: -404 });
    }
    let out_p = std::path::Path::new(&output_path);
    let paths: Vec<std::path::PathBuf> = source_paths.iter().map(std::path::PathBuf::from).collect();

    let secure_pwd = password.map(crate::crypto::ZeroizingString::new);
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
    let raw_fmt: crate::types::TTZipArchiveFormat = format.into();

    let comp_level = match level {
        0 => crate::types::TTZipCompressionLevel::Store,
        1 => crate::types::TTZipCompressionLevel::Fastest,
        2..=4 => crate::types::TTZipCompressionLevel::Fast,
        5..=6 => crate::types::TTZipCompressionLevel::Normal,
        7..=9 => crate::types::TTZipCompressionLevel::Maximum,
        10..=22 => crate::types::TTZipCompressionLevel::Ultra,
        _ => crate::types::TTZipCompressionLevel::Normal,
    };

    let enc_method = if pwd_cstr.is_some() {
        crate::types::TTZipEncryptionMethod::Aes256
    } else {
        crate::types::TTZipEncryptionMethod::None
    };

    let options = crate::types::TTZipCreateOptions {
        struct_size: std::mem::size_of::<crate::types::TTZipCreateOptions>() as u32,
        abi_version: crate::types::TTZIP_ABI_VERSION_2,
        format: raw_fmt,
        level: comp_level,
        encryption: enc_method,
        password: pwd_cstr.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null()),
        thread_budget: 0,
        solid_block_size_mb: 64,
        progress_callback: Some(progress_cb),
        user_data: raw_ctx,
    };

    crate::archive::unified::create::create_archive(&paths, out_p, &options, 0)
        .map_err(|s| {
            if s == crate::types::TTZipStatus::Cancelled {
                TTZipError::Cancelled
            } else {
                TTZipError::EngineError { code: s as i32 }
            }
        })?;

    fn calculate_paths_uncompressed_bytes(paths: &[std::path::PathBuf]) -> u64 {
        fn calculate_single(path: &std::path::Path) -> u64 {
            if path.is_file() {
                std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
            } else if path.is_dir() {
                let mut total = 0;
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        total += calculate_single(&entry.path());
                    }
                }
                total
            } else {
                0
            }
        }
        paths.iter().map(|p| calculate_single(p)).sum()
    }

    let elapsed = start.elapsed();
    let elapsed_nanos = elapsed.as_nanos() as u64;
    let elapsed_secs = elapsed.as_secs_f64().max(0.000001);
    let comp_size = std::fs::metadata(out_p).map(|m| m.len()).unwrap_or(0);
    let uncompressed_bytes = calculate_paths_uncompressed_bytes(&paths);
    let benchmark_bytes = if uncompressed_bytes > 0 {
        uncompressed_bytes
    } else {
        comp_size
    };
    let throughput_mbs = (benchmark_bytes as f64 / (1024.0 * 1024.0)) / elapsed_secs;
    let space_savings_pct = if uncompressed_bytes > 0 && comp_size <= uncompressed_bytes {
        ((uncompressed_bytes - comp_size) as f64 / uncompressed_bytes as f64) * 100.0
    } else {
        0.0
    };

    Ok(CompressionReport {
        uncompressed_bytes,
        compressed_bytes: comp_size,
        elapsed_nanos,
        throughput_mbs,
        space_savings_pct,
        engine_provenance: "Mozilla UniFFI Native Create Pipeline".to_string(),
    })
}

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

/// Advanced archive creation pipeline with single-pass parallel scanning, filtering,
/// split volume output, and atomic transactional cleanup.
#[uniffi::export]
pub fn create_archive_advanced(
    config: UniFFICreateArchiveConfig,
    progress: Option<Box<dyn ProgressHandler>>,
    token: Option<Arc<CancellationToken>>,
) -> Result<CompressionReport, TTZipError> {
    if let Some(ref t) = token {
        if t.is_cancelled() {
            return Err(TTZipError::Cancelled);
        }
    }

    if config.source_paths.is_empty() {
        return Err(TTZipError::EngineError { code: -404 });
    }

    let out_path = std::path::Path::new(&config.output_path);
    let parent = out_path.parent().unwrap_or_else(|| std::path::Path::new("."));
    if !parent.exists() {
        let _ = std::fs::create_dir_all(parent);
    }

    let inc_refs: Vec<&str> = config.include_patterns.iter().map(|s| s.as_str()).collect();
    let exc_refs: Vec<&str> = config.exclude_patterns.iter().map(|s| s.as_str()).collect();
    let filter = crate::fs::filter::pattern::PathPatternFilter::new(
        &inc_refs,
        &exc_refs,
        config.exclude_vcs,
        config.no_mac_metadata,
    )
    .map_err(|e| TTZipError::IoError { message: e })?;

    let start = std::time::Instant::now();
    let mut planned_entries: Vec<(std::path::PathBuf, String)> = Vec::new();
    let mut total_uncompressed_bytes: u64 = 0;

    for src_str in &config.source_paths {
        if let Some(ref t) = token {
            if t.is_cancelled() {
                return Err(TTZipError::Cancelled);
            }
        }
        let src_path = std::path::Path::new(src_str);
        if !src_path.exists() && std::fs::symlink_metadata(src_path).is_err() {
            return Err(TTZipError::FileNotFound { path: src_str.clone() });
        }
        let canonical_src = std::fs::canonicalize(src_path).unwrap_or_else(|_| src_path.to_path_buf());
        let meta = match std::fs::metadata(&canonical_src) {
            Ok(m) => m,
            Err(_) => match std::fs::symlink_metadata(src_path) {
                Ok(m) => m,
                Err(e) => return Err(TTZipError::IoError { message: e.to_string() }),
            },
        };

        if meta.is_dir() && !meta.file_type().is_symlink() {
            let scan_opts = crate::fs::scanner::ScanOptions {
                include_hidden: true,
                skip_mac_junk: config.no_mac_metadata,
                max_depth: 0,
                thread_budget: config.thread_budget,
            };
            let scanned_items = crate::fs::scanner::scan_directory_parallel(&canonical_src, &scan_opts);
            for item in scanned_items {
                if filter.should_include(&item.rel_path) {
                    total_uncompressed_bytes = total_uncompressed_bytes.saturating_add(item.file_size);
                    planned_entries.push((std::path::PathBuf::from(item.src_path), item.rel_path));
                }
            }
        } else {
            let file_name = src_path.file_name().and_then(|n| n.to_str()).unwrap_or(src_str);
            if filter.should_include(file_name) {
                let size = if meta.file_type().is_symlink() { 0 } else { meta.len() };
                total_uncompressed_bytes = total_uncompressed_bytes.saturating_add(size);
                planned_entries.push((canonical_src, file_name.to_string()));
            }
        }
    }

    if planned_entries.is_empty() {
        return Err(TTZipError::EngineError { code: -404 });
    }

    let file_name = out_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or(TTZipError::EngineError { code: -1 })?;
    static TXN_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let seq = TXN_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let txn_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let txn_dir = parent.join(format!(".ttzip_txn_{}_{}_{}_{}", std::process::id(), txn_id, seq, file_name));
    std::fs::create_dir_all(&txn_dir).map_err(|e| TTZipError::IoError { message: e.to_string() })?;

    struct TxnCleanupGuard<'a> {
        dir: &'a std::path::Path,
        defused: bool,
    }
    impl<'a> Drop for TxnCleanupGuard<'a> {
        fn drop(&mut self) {
            if !self.defused {
                let _ = std::fs::remove_dir_all(self.dir);
            }
        }
    }
    let mut cleanup_guard = TxnCleanupGuard {
        dir: &txn_dir,
        defused: false,
    };

    let staged_target = txn_dir.join(file_name);

    let secure_pwd = config.password.map(crate::crypto::ZeroizingString::new);
    let pwd_cstr = secure_pwd.as_ref().and_then(|p| p.to_c_string().ok());

    struct ProgressContextExplicit {
        handler: Option<Box<dyn ProgressHandler>>,
        token: Option<Arc<CancellationToken>>,
        total_bytes: u64,
    }
    let progress_ctx = Arc::new(ProgressContextExplicit {
        handler: progress,
        token,
        total_bytes: total_uncompressed_bytes,
    });
    let raw_ctx = Arc::into_raw(progress_ctx) as *mut libc::c_void;

    struct ContextGuardExplicit(*mut libc::c_void);
    impl Drop for ContextGuardExplicit {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    drop(Arc::from_raw(self.0 as *const ProgressContextExplicit));
                }
            }
        }
    }
    let _guard = ContextGuardExplicit(raw_ctx);

    unsafe extern "C" fn progress_cb(
        processed: u64,
        _total: u64,
        entry_name: *const libc::c_char,
        user_data: *mut libc::c_void,
    ) -> bool {
        if user_data.is_null() {
            return true;
        }
        let unwind_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let p = &*(user_data as *const ProgressContextExplicit);
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
                return h.on_progress(processed, p.total_bytes, name);
            }
            true
        }));

        unwind_res.unwrap_or(false)
    }

    let raw_fmt: crate::types::TTZipArchiveFormat = config.format.into();
    let comp_level = match config.level {
        0 => crate::types::TTZipCompressionLevel::Store,
        1 => crate::types::TTZipCompressionLevel::Fastest,
        2..=4 => crate::types::TTZipCompressionLevel::Fast,
        5..=6 => crate::types::TTZipCompressionLevel::Normal,
        7..=9 => crate::types::TTZipCompressionLevel::Maximum,
        10..=22 => crate::types::TTZipCompressionLevel::Ultra,
        _ => crate::types::TTZipCompressionLevel::Normal,
    };

    let enc_method = if pwd_cstr.is_some() {
        crate::types::TTZipEncryptionMethod::Aes256
    } else {
        crate::types::TTZipEncryptionMethod::None
    };

    let options = crate::types::TTZipCreateOptions {
        struct_size: std::mem::size_of::<crate::types::TTZipCreateOptions>() as u32,
        abi_version: crate::types::TTZIP_ABI_VERSION_2,
        format: raw_fmt,
        level: comp_level,
        encryption: enc_method,
        password: pwd_cstr.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null()),
        thread_budget: config.thread_budget,
        solid_block_size_mb: 64,
        progress_callback: Some(progress_cb),
        user_data: raw_ctx,
    };

    let split_size = config.split_volume_size_bytes.unwrap_or(0);
    crate::archive::unified::create::create_archive_explicit(
        &planned_entries,
        &staged_target,
        &options,
        split_size,
    )
    .map_err(|s| {
        if s == crate::types::TTZipStatus::Cancelled {
            TTZipError::Cancelled
        } else {
            TTZipError::EngineError { code: s as i32 }
        }
    })?;

    let entries_in_txn = std::fs::read_dir(&txn_dir)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    let mut created_files = Vec::new();
    for entry in entries_in_txn.flatten() {
        let src_file = entry.path();
        let fname = entry.file_name();
        let dst_file = parent.join(fname);
        if dst_file.exists() {
            let _ = std::fs::remove_file(&dst_file);
        }
        std::fs::rename(&src_file, &dst_file)
            .map_err(|e| TTZipError::IoError { message: e.to_string() })?;
        created_files.push(dst_file);
    }
    cleanup_guard.defused = true;
    let _ = std::fs::remove_dir_all(&txn_dir);

    let elapsed = start.elapsed();
    let elapsed_nanos = elapsed.as_nanos() as u64;
    let elapsed_secs = elapsed.as_secs_f64().max(0.000001);
    let comp_size: u64 = created_files
        .iter()
        .map(|f| std::fs::metadata(f).map(|m| m.len()).unwrap_or(0))
        .sum();
    let throughput_mbs = (total_uncompressed_bytes as f64 / (1024.0 * 1024.0)) / elapsed_secs;
    let space_savings_pct = if total_uncompressed_bytes > 0 && comp_size <= total_uncompressed_bytes {
        ((total_uncompressed_bytes - comp_size) as f64 / total_uncompressed_bytes as f64) * 100.0
    } else {
        0.0
    };

    Ok(CompressionReport {
        uncompressed_bytes: total_uncompressed_bytes,
        compressed_bytes: comp_size,
        elapsed_nanos,
        throughput_mbs,
        space_savings_pct,
        engine_provenance: "Mozilla UniFFI Advanced Create Pipeline".to_string(),
    })
}

/// Recovers password against an encrypted archive using Rayon multi-core parallel probing.
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
    let total_attempts = dictionary.len() as u64;

    // Use Rayon parallel search
    let found_password = dictionary.into_par_iter().find_any(|candidate| {
        if file_bytes.starts_with(b"7z\xBC\xAF\x27\x1C") {
            if let Ok(arch) = crate::sevenz::decoder::SevenZArchive::open_slice(&file_bytes) {
                if !arch.info().is_encrypted {
                    return false;
                }
                let enc_idx = arch.files().iter().position(|f| !f.is_directory).unwrap_or(0);
                return arch.extract_entry_bytes_stream(enc_idx, Some(candidate.as_str())).is_ok();
            }
        } else if let Ok(zip_archive) = crate::zip::reader::ZipArchive::open_slice(&file_bytes) {
            let enc_idx = zip_archive.entries().iter().position(|e| e.is_encrypted && !e.is_directory && e.uncompressed_size > 0);
            if let Some(idx) = enc_idx {
                return zip_archive.extract_entry_bytes(idx, Some(candidate.as_str())).is_ok();
            }
        }
        false
    });

    let elapsed = start.elapsed();
    let elapsed_nanos = elapsed.as_nanos() as u64;
    let elapsed_secs = elapsed.as_secs_f64().max(0.000001);
    let attempts_per_second = (total_attempts as f64) / elapsed_secs;

    Ok(PasswordRecoveryOutcome {
        found_password,
        total_attempts,
        elapsed_nanos,
        attempts_per_second,
    })
}
