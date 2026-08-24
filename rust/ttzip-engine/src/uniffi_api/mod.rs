// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

//! Mozilla UniFFI Proc-Macro Export Layer.
//!
//! Provides typed, memory-safe, and Swift 6 Sendable bindings directly generated
//! from Rust business logic without manual C-ABI pointers.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;

/// Typed error enum mapped directly to Swift `throws TTZipError`.
#[derive(Debug, Error, uniffi::Error)]
pub enum TTZipError {
    #[error("File not found: {path}")]
    FileNotFound { path: String },

    #[error("Invalid password provided for encrypted archive")]
    InvalidPassword,

    #[error("Corrupted archive header at offset {offset}: {details}")]
    CorruptHeader { details: String, offset: u64 },

    #[error("Security policy violation: {reason}")]
    SecurityViolation { reason: String },

    #[error("Operation failed with status code: {code}")]
    EngineError { code: i32 },

    #[error("I/O error: {message}")]
    IoError { message: String },

    #[error("Operation was cancelled by caller")]
    Cancelled,
}

/// Archive format enum exposed to Swift.
#[derive(Copy, Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ArchiveFormat {
    Auto,
    Zip,
    SevenZip,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    TarZstd,
    Dmg,
    Lzfse,
    Snappy,
    Wim,
}

/// Metadata record for a single archive entry exposed via UniFFI.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIEntryMetadata {
    pub path: String,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub mtime_epoch_secs: i64,
    pub mode: u32,
    pub is_directory: bool,
    pub is_encrypted: bool,
    pub compression_method: String,
    pub detected_encoding: Option<String>,
}

/// Telemetry report for compression / extraction operations.
#[derive(Clone, Debug, uniffi::Record)]
pub struct CompressionReport {
    pub uncompressed_bytes: u64,
    pub compressed_bytes: u64,
    pub elapsed_nanos: u64,
    pub throughput_mbs: f64,
    pub space_savings_pct: f64,
    pub engine_provenance: String,
}

/// VFS Search Result item exposed to Swift.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIVfsMatch {
    pub path: String,
    pub name: String,
    pub is_directory: bool,
    pub size: u64,
}

/// Callback interface protocol implemented in Swift.
#[uniffi::export(callback_interface)]
pub trait ProgressHandler: Send + Sync {
    fn on_progress(&self, processed_bytes: u64, total_bytes: u64, current_entry: Option<String>) -> bool;
}

/// Thread-safe cancellation token object.
#[derive(uniffi::Object)]
pub struct CancellationToken {
    cancelled: AtomicBool,
}

#[uniffi::export]
impl CancellationToken {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            cancelled: AtomicBool::new(false),
        })
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

/// Thread-safe in-memory VFS Tree object exposed to Swift.
#[derive(uniffi::Object)]
pub struct UniFFIVfsTree {
    tree: parking_lot::RwLock<crate::fs::vfs::tree::VfsTree>,
}

#[uniffi::export]
impl UniFFIVfsTree {
    #[uniffi::constructor]
    pub fn build(entries: Vec<UniFFIEntryMetadata>, root_name: String) -> Arc<Self> {
        let vfs_entries: Vec<crate::fs::vfs::node::VfsEntry> = entries
            .into_iter()
            .map(|e| crate::fs::vfs::node::VfsEntry {
                path: e.path,
                uncompressed_size: e.uncompressed_size,
                compressed_size: e.compressed_size,
                crc32: e.crc32,
                mtime_epoch_secs: e.mtime_epoch_secs,
                mode: e.mode,
                is_directory: e.is_directory,
                is_encrypted: e.is_encrypted,
            })
            .collect();

        let tree = crate::fs::vfs::tree::VfsTree::build_from_entries(&vfs_entries, &root_name);
        Arc::new(Self {
            tree: parking_lot::RwLock::new(tree),
        })
    }

    pub fn search(&self, query: String, max_results: u32) -> Vec<UniFFIVfsMatch> {
        let guard = self.tree.read();
        let matches = guard.fuzzy_search(&query);
        matches
            .into_iter()
            .take(max_results as usize)
            .map(|m| UniFFIVfsMatch {
                path: m.path,
                name: m.name,
                is_directory: m.is_directory,
                size: m.uncompressed_size,
            })
            .collect()
    }

    pub fn total_entries(&self) -> u64 {
        let guard = self.tree.read();
        guard.total_entries as u64
    }
}

/// Detects archive format from file magic bytes.
#[uniffi::export]
pub fn detect_archive_format(path: String) -> Result<ArchiveFormat, TTZipError> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path });
    }

    let mut f = std::fs::File::open(p).map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    let mut magic = [0u8; 16];
    let bytes_read = std::io::Read::read(&mut f, &mut magic).unwrap_or(0);
    let buf = &magic[..bytes_read];

    if buf.starts_with(b"PK\x03\x04") || buf.starts_with(b"PK\x05\x06") || buf.starts_with(b"PK\x07\x08") {
        Ok(ArchiveFormat::Zip)
    } else if buf.starts_with(b"7z\xBC\xAF\x27\x1C") {
        Ok(ArchiveFormat::SevenZip)
    } else if buf.starts_with(b"\x1F\x8B") {
        Ok(ArchiveFormat::TarGz)
    } else if buf.starts_with(b"\x28\xB5\x2F\xFD") {
        Ok(ArchiveFormat::TarZstd)
    } else if buf.starts_with(b"bzx\x00") || buf.starts_with(b"bvf\x00") {
        Ok(ArchiveFormat::Lzfse)
    } else {
        Ok(ArchiveFormat::Auto)
    }
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
    let p = std::path::Path::new(&archive_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

    struct EntryCollector {
        entries: Vec<UniFFIEntryMetadata>,
    }

    let mut collector = EntryCollector {
        entries: Vec::new(),
    };

    unsafe extern "C" fn inspect_cb(
        meta: *const crate::types::TTZipEntryMetadata,
        user_data: *mut libc::c_void,
    ) -> bool {
        if meta.is_null() || user_data.is_null() {
            return true;
        }
        let collector = &mut *(user_data as *mut EntryCollector);
        let m = &*meta;
        let path = if !m.path.is_null() {
            std::ffi::CStr::from_ptr(m.path).to_string_lossy().into_owned()
        } else {
            String::new()
        };
        let method = match m.compression_method {
            0 => "store",
            8 => "deflate",
            12 => "bzip2",
            14 => "lzma",
            95 => "zstd",
            _ => "unknown",
        }.to_string();
        let encoding = if !m.detected_encoding.is_null() {
            Some(std::ffi::CStr::from_ptr(m.detected_encoding).to_string_lossy().into_owned())
        } else {
            None
        };

        collector.entries.push(UniFFIEntryMetadata {
            path,
            uncompressed_size: m.uncompressed_size,
            compressed_size: m.compressed_size,
            crc32: m.crc32,
            mtime_epoch_secs: m.mtime_epoch_secs,
            mode: m.mode,
            is_directory: m.is_directory,
            is_encrypted: m.is_encrypted,
            compression_method: method,
            detected_encoding: encoding,
        });
        true
    }

    let res = crate::archive::unified::inspect::inspect_archive(
        p,
        password.as_deref(),
        true,
        Some(inspect_cb),
        &mut collector as *mut EntryCollector as *mut libc::c_void,
    );

    match res {
        Ok(_) => Ok(collector.entries),
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
    let p = std::path::Path::new(&archive_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

    let file_bytes = std::fs::read(p).map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    if file_bytes.starts_with(b"7z\xBC\xAF\x27\x1C") {
        let arch = crate::sevenz::decoder::SevenZArchive::open_slice(&file_bytes)
            .map_err(|_| TTZipError::CorruptHeader { details: "Invalid 7z header".to_string(), offset: 0 })?;
        arch.extract_entry_bytes_stream(entry_index as usize, password.as_deref())
            .map_err(|_| TTZipError::InvalidPassword)
    } else {
        Err(TTZipError::EngineError { code: -1 })
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

    let raw_fmt = match format {
        ArchiveFormat::Zip => crate::types::TTZipArchiveFormat::Zip,
        ArchiveFormat::SevenZip => crate::types::TTZipArchiveFormat::SevenZip,
        ArchiveFormat::Tar => crate::types::TTZipArchiveFormat::Tar,
        ArchiveFormat::TarGz => crate::types::TTZipArchiveFormat::TarGz,
        ArchiveFormat::TarBz2 => crate::types::TTZipArchiveFormat::TarBz2,
        ArchiveFormat::TarXz => crate::types::TTZipArchiveFormat::TarXz,
        ArchiveFormat::TarZstd => crate::types::TTZipArchiveFormat::TarZstd,
        ArchiveFormat::Dmg => crate::types::TTZipArchiveFormat::Dmg,
        ArchiveFormat::Lzfse => crate::types::TTZipArchiveFormat::Lzfse,
        ArchiveFormat::Snappy => crate::types::TTZipArchiveFormat::Snappy,
        ArchiveFormat::Auto | ArchiveFormat::Wim => crate::types::TTZipArchiveFormat::Auto,
    };

    let comp_level = match level {
        0 => crate::types::TTZipCompressionLevel::Store,
        1 => crate::types::TTZipCompressionLevel::Fastest,
        2 => crate::types::TTZipCompressionLevel::Fast,
        9 => crate::types::TTZipCompressionLevel::Ultra,
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
        user_data: &mut pbox as *mut ProgressBox as *mut libc::c_void,
    };

    crate::archive::unified::create::create_archive(&paths, out_p, &options, 0)
        .map_err(|s| {
            if s == crate::types::TTZipStatus::Cancelled {
                TTZipError::Cancelled
            } else {
                TTZipError::EngineError { code: s as i32 }
            }
        })?;

    let elapsed = start.elapsed();
    let elapsed_nanos = elapsed.as_nanos() as u64;
    let elapsed_secs = elapsed.as_secs_f64().max(0.000001);
    let comp_size = std::fs::metadata(out_p).map(|m| m.len()).unwrap_or(0);
    let throughput_mbs = (comp_size as f64 / (1024.0 * 1024.0)) / elapsed_secs;

    Ok(CompressionReport {
        uncompressed_bytes: comp_size,
        compressed_bytes: comp_size,
        elapsed_nanos,
        throughput_mbs,
        space_savings_pct: 0.0,
        engine_provenance: "Mozilla UniFFI Native Create Pipeline".to_string(),
    })
}

/// Computes SHA-256 hex digest of a file.
#[uniffi::export]
pub fn compute_file_sha256(file_path: String) -> Result<String, TTZipError> {
    let p = std::path::Path::new(&file_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: file_path });
    }
    let bytes = std::fs::read(p).map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    let mut hasher = crate::crypto::sha256::FastSha256::new();
    hasher.update(&bytes);
    let hash = hasher.finalize();
    Ok(hash.iter().map(|b| format!("{:02x}", b)).collect())
}

/// Computes CRC32 checksum of a file.
#[uniffi::export]
pub fn compute_file_crc32(file_path: String) -> Result<u32, TTZipError> {
    let p = std::path::Path::new(&file_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: file_path });
    }
    let bytes = std::fs::read(p).map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    Ok(crate::crypto::crc32::crc32(&bytes))
}

/// Detects split volume chain members starting from seed file.
#[uniffi::export]
pub fn detect_split_volume_chain(seed_path: String) -> Result<Vec<String>, TTZipError> {
    let p = std::path::Path::new(&seed_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: seed_path });
    }
    let chain = crate::archive::split::detect_volume_chain(p)
        .map_err(|_| TTZipError::EngineError { code: -1 })?;
    Ok(chain.into_iter().filter_map(|p| p.to_str().map(|s| s.to_string())).collect())
}

/// Joins multi-volume split archive files into a continuous output file.
#[uniffi::export]
pub fn join_split_volume_chain(
    first_volume_path: String,
    output_path: String,
) -> Result<(), TTZipError> {
    let first = std::path::Path::new(&first_volume_path);
    let out = std::path::Path::new(&output_path);
    if !first.exists() {
        return Err(TTZipError::FileNotFound { path: first_volume_path });
    }
    let chain = crate::archive::split::detect_volume_chain(first)
        .map_err(|_| TTZipError::EngineError { code: -1 })?;
    let mut virtual_reader = crate::archive::split::VirtualMultiVolumeReader::from_volumes(chain)
        .map_err(|_| TTZipError::EngineError { code: -2 })?;
    let mut out_file = std::fs::File::create(out)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    std::io::copy(&mut virtual_reader, &mut out_file)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    Ok(())
}

pub use crate::i18n::{AppLanguage, ByteSizeStandard, TTZipLocalizationEngine};

/// Convenient static function to retrieve localized string via UniFFI.
#[uniffi::export]
pub fn ttzip_i18n_get_string(key: String, lang: AppLanguage) -> String {
    crate::i18n::get_string_or_fallback(&key, lang).to_string()
}

/// Convenient static function to format byte sizes via UniFFI.
#[uniffi::export]
pub fn ttzip_i18n_format_bytes(bytes: i64, standard: ByteSizeStandard, lang: AppLanguage) -> String {
    crate::i18n::format_bytes(bytes, standard, lang)
}

/// Convenient static function to format throughput via UniFFI.
#[uniffi::export]
pub fn ttzip_i18n_format_throughput(mb_per_sec: f64, lang: AppLanguage) -> String {
    crate::i18n::format_throughput(mb_per_sec, lang)
}

/// Convenient static function to localize errors via UniFFI.
#[uniffi::export]
pub fn ttzip_i18n_localize_error(
    error_code: i32,
    param1: Option<String>,
    param2: Option<String>,
    lang: AppLanguage,
) -> String {
    crate::i18n::localize_error(error_code, param1.as_deref(), param2.as_deref(), lang)
}

