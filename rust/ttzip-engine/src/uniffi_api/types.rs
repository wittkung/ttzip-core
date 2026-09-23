// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Mozilla UniFFI Typed Record, Enum, Error, and Interface Definitions.

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

    #[error("Out of memory during decompression: {details}")]
    OutOfMemory { details: String },

    #[error("Decompression bomb detected: {reason}")]
    DecompressionBombDetected { reason: String },

    #[error("Data checksum mismatch: expected {expected:#x}, got {actual:#x}")]
    ChecksumMismatch { expected: u32, actual: u32 },
}

impl TTZipError {
    pub fn file_not_found(path: &str) -> Self {
        TTZipError::FileNotFound {
            path: path.to_string(),
        }
    }

    pub fn io_error(err: impl std::fmt::Display, context: &str) -> Self {
        TTZipError::IoError {
            message: format!("{context}: {err}"),
        }
    }

    pub fn out_of_memory(details: impl Into<String>) -> Self {
        TTZipError::OutOfMemory {
            details: details.into(),
        }
    }

    pub fn decompression_bomb(reason: impl Into<String>) -> Self {
        TTZipError::DecompressionBombDetected {
            reason: reason.into(),
        }
    }

    pub fn checksum_mismatch(expected: u32, actual: u32) -> Self {
        TTZipError::ChecksumMismatch { expected, actual }
    }
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
    TarLz4,
    TarBrotli,
    TarLzip,
    TarLrzip,
    Dmg,
    Lzfse,
    Snappy,
    Gzip,
    Bzip2,
    Xz,
    Zstd,
    Lz4,
    Brotli,
    Iso,
    Cab,
    Wim,
    Rar,
    Aar,
    Lzip,
    Lrzip,
    Cpio,
    Ar,
    Deb,
    Rpm,
    Xar,
    Squashfs,
    Lzh,
}

/// Canonical TTZip archive format enumeration exposed via UniFFI.
#[derive(Copy, Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum TTZipArchiveFormat {
    Auto,
    Zip,
    SevenZip,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    TarZstd,
    TarLz4,
    TarBrotli,
    TarLzip,
    TarLrzip,
    Dmg,
    Lzfse,
    Snappy,
    Gzip,
    Bzip2,
    Xz,
    Zstd,
    Lz4,
    Brotli,
    Iso,
    Cab,
    Wim,
    Rar,
    Aar,
    Lzip,
    Lrzip,
    Cpio,
    Ar,
    Deb,
    Rpm,
    Xar,
    Squashfs,
    Lzh,
}

impl From<ArchiveFormat> for TTZipArchiveFormat {
    fn from(fmt: ArchiveFormat) -> Self {
        match fmt {
            ArchiveFormat::Auto => Self::Auto,
            ArchiveFormat::Zip => Self::Zip,
            ArchiveFormat::SevenZip => Self::SevenZip,
            ArchiveFormat::Tar => Self::Tar,
            ArchiveFormat::TarGz => Self::TarGz,
            ArchiveFormat::TarBz2 => Self::TarBz2,
            ArchiveFormat::TarXz => Self::TarXz,
            ArchiveFormat::TarZstd => Self::TarZstd,
            ArchiveFormat::TarLz4 => Self::TarLz4,
            ArchiveFormat::TarBrotli => Self::TarBrotli,
            ArchiveFormat::TarLzip => Self::TarLzip,
            ArchiveFormat::TarLrzip => Self::TarLrzip,
            ArchiveFormat::Dmg => Self::Dmg,
            ArchiveFormat::Lzfse => Self::Lzfse,
            ArchiveFormat::Snappy => Self::Snappy,
            ArchiveFormat::Gzip => Self::Gzip,
            ArchiveFormat::Bzip2 => Self::Bzip2,
            ArchiveFormat::Xz => Self::Xz,
            ArchiveFormat::Zstd => Self::Zstd,
            ArchiveFormat::Lz4 => Self::Lz4,
            ArchiveFormat::Brotli => Self::Brotli,
            ArchiveFormat::Iso => Self::Iso,
            ArchiveFormat::Cab => Self::Cab,
            ArchiveFormat::Wim => Self::Wim,
            ArchiveFormat::Rar => Self::Rar,
            ArchiveFormat::Aar => Self::Aar,
            ArchiveFormat::Lzip => Self::Lzip,
            ArchiveFormat::Lrzip => Self::Lrzip,
            ArchiveFormat::Cpio => Self::Cpio,
            ArchiveFormat::Ar => Self::Ar,
            ArchiveFormat::Deb => Self::Deb,
            ArchiveFormat::Rpm => Self::Rpm,
            ArchiveFormat::Xar => Self::Xar,
            ArchiveFormat::Squashfs => Self::Squashfs,
            ArchiveFormat::Lzh => Self::Lzh,
        }
    }
}

impl From<TTZipArchiveFormat> for ArchiveFormat {
    fn from(fmt: TTZipArchiveFormat) -> Self {
        match fmt {
            TTZipArchiveFormat::Auto => Self::Auto,
            TTZipArchiveFormat::Zip => Self::Zip,
            TTZipArchiveFormat::SevenZip => Self::SevenZip,
            TTZipArchiveFormat::Tar => Self::Tar,
            TTZipArchiveFormat::TarGz => Self::TarGz,
            TTZipArchiveFormat::TarBz2 => Self::TarBz2,
            TTZipArchiveFormat::TarXz => Self::TarXz,
            TTZipArchiveFormat::TarZstd => Self::TarZstd,
            TTZipArchiveFormat::TarLz4 => Self::TarLz4,
            TTZipArchiveFormat::TarBrotli => Self::TarBrotli,
            TTZipArchiveFormat::TarLzip => Self::TarLzip,
            TTZipArchiveFormat::TarLrzip => Self::TarLrzip,
            TTZipArchiveFormat::Dmg => Self::Dmg,
            TTZipArchiveFormat::Lzfse => Self::Lzfse,
            TTZipArchiveFormat::Snappy => Self::Snappy,
            TTZipArchiveFormat::Gzip => Self::Gzip,
            TTZipArchiveFormat::Bzip2 => Self::Bzip2,
            TTZipArchiveFormat::Xz => Self::Xz,
            TTZipArchiveFormat::Zstd => Self::Zstd,
            TTZipArchiveFormat::Lz4 => Self::Lz4,
            TTZipArchiveFormat::Brotli => Self::Brotli,
            TTZipArchiveFormat::Iso => Self::Iso,
            TTZipArchiveFormat::Cab => Self::Cab,
            TTZipArchiveFormat::Wim => Self::Wim,
            TTZipArchiveFormat::Rar => Self::Rar,
            TTZipArchiveFormat::Aar => Self::Aar,
            TTZipArchiveFormat::Lzip => Self::Lzip,
            TTZipArchiveFormat::Lrzip => Self::Lrzip,
            TTZipArchiveFormat::Cpio => Self::Cpio,
            TTZipArchiveFormat::Ar => Self::Ar,
            TTZipArchiveFormat::Deb => Self::Deb,
            TTZipArchiveFormat::Rpm => Self::Rpm,
            TTZipArchiveFormat::Xar => Self::Xar,
            TTZipArchiveFormat::Squashfs => Self::Squashfs,
            TTZipArchiveFormat::Lzh => Self::Lzh,
        }
    }
}

impl From<ArchiveFormat> for crate::types::TTZipArchiveFormat {
    fn from(fmt: ArchiveFormat) -> Self {
        match fmt {
            ArchiveFormat::Auto => crate::types::TTZipArchiveFormat::Auto,
            ArchiveFormat::Zip => crate::types::TTZipArchiveFormat::Zip,
            ArchiveFormat::SevenZip => crate::types::TTZipArchiveFormat::SevenZip,
            ArchiveFormat::Tar => crate::types::TTZipArchiveFormat::Tar,
            ArchiveFormat::TarGz => crate::types::TTZipArchiveFormat::TarGz,
            ArchiveFormat::TarBz2 => crate::types::TTZipArchiveFormat::TarBz2,
            ArchiveFormat::TarXz => crate::types::TTZipArchiveFormat::TarXz,
            ArchiveFormat::TarZstd => crate::types::TTZipArchiveFormat::TarZstd,
            ArchiveFormat::TarLz4 => crate::types::TTZipArchiveFormat::TarLz4,
            ArchiveFormat::TarBrotli => crate::types::TTZipArchiveFormat::TarBrotli,
            ArchiveFormat::TarLzip => crate::types::TTZipArchiveFormat::TarLzip,
            ArchiveFormat::TarLrzip => crate::types::TTZipArchiveFormat::TarLrzip,
            ArchiveFormat::Dmg => crate::types::TTZipArchiveFormat::Dmg,
            ArchiveFormat::Lzfse => crate::types::TTZipArchiveFormat::Lzfse,
            ArchiveFormat::Snappy => crate::types::TTZipArchiveFormat::Snappy,
            ArchiveFormat::Gzip => crate::types::TTZipArchiveFormat::Gzip,
            ArchiveFormat::Bzip2 => crate::types::TTZipArchiveFormat::Bzip2,
            ArchiveFormat::Xz => crate::types::TTZipArchiveFormat::Xz,
            ArchiveFormat::Zstd => crate::types::TTZipArchiveFormat::Zstd,
            ArchiveFormat::Lz4 => crate::types::TTZipArchiveFormat::Lz4,
            ArchiveFormat::Brotli => crate::types::TTZipArchiveFormat::Brotli,
            ArchiveFormat::Iso => crate::types::TTZipArchiveFormat::Iso,
            ArchiveFormat::Cab => crate::types::TTZipArchiveFormat::Cab,
            ArchiveFormat::Wim => crate::types::TTZipArchiveFormat::Wim,
            ArchiveFormat::Rar => crate::types::TTZipArchiveFormat::Rar,
            ArchiveFormat::Aar => crate::types::TTZipArchiveFormat::Aar,
            ArchiveFormat::Lzip => crate::types::TTZipArchiveFormat::Lzip,
            ArchiveFormat::Lrzip => crate::types::TTZipArchiveFormat::Lrzip,
            ArchiveFormat::Cpio => crate::types::TTZipArchiveFormat::Cpio,
            ArchiveFormat::Ar => crate::types::TTZipArchiveFormat::Ar,
            ArchiveFormat::Deb => crate::types::TTZipArchiveFormat::Deb,
            ArchiveFormat::Rpm => crate::types::TTZipArchiveFormat::Rpm,
            ArchiveFormat::Xar => crate::types::TTZipArchiveFormat::Xar,
            ArchiveFormat::Squashfs => crate::types::TTZipArchiveFormat::Squashfs,
            ArchiveFormat::Lzh => crate::types::TTZipArchiveFormat::Lzh,
        }
    }
}

impl From<TTZipArchiveFormat> for crate::types::TTZipArchiveFormat {
    fn from(fmt: TTZipArchiveFormat) -> Self {
        ArchiveFormat::from(fmt).into()
    }
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

impl From<ttzip_types::TTZipEntryDetail> for UniFFIEntryMetadata {
    fn from(d: ttzip_types::TTZipEntryDetail) -> Self {
        Self {
            path: d.path,
            uncompressed_size: d.uncompressed_size,
            compressed_size: d.compressed_size,
            crc32: d.crc32,
            mtime_epoch_secs: d.mtime_epoch_secs,
            mode: d.mode,
            is_directory: d.is_directory,
            is_encrypted: d.is_encrypted,
            compression_method: d.compression_method,
            detected_encoding: d.detected_encoding,
        }
    }
}

impl From<UniFFIEntryMetadata> for ttzip_types::TTZipEntryDetail {
    fn from(d: UniFFIEntryMetadata) -> Self {
        Self {
            path: d.path,
            uncompressed_size: d.uncompressed_size,
            compressed_size: d.compressed_size,
            crc32: d.crc32,
            mtime_epoch_secs: d.mtime_epoch_secs,
            mode: d.mode,
            is_directory: d.is_directory,
            is_encrypted: d.is_encrypted,
            compression_method: d.compression_method,
            detected_encoding: d.detected_encoding,
        }
    }
}


#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct InPlaceMutationAction {
    pub is_delete: bool,
    pub entry_path: String,
    pub source_path: Option<String>,
}

/// WAL journal mutation summary record exposed via UniFFI.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIWalMutationSummary {
    pub wal_path: String,
    pub entry_path: String,
    pub delta_bytes: u64,
    pub total_pieces: u32,
    pub is_staged: bool,
}

/// WAL atomic commit execution telemetry record exposed via UniFFI.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIWalCommitResult {
    pub success: bool,
    pub bytes_written: u64,
    pub cow_cloned: bool,
    pub elapsed_millis: u64,
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

/// Configuration options for archive extraction through UniFFI.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIExtractArchiveConfig {
    pub archive_path: String,
    pub destination_dir: String,
    pub password: Option<String>,
    pub thread_budget: u32,
    pub overwrite_existing: bool,
    pub preserve_permissions: bool,
}

impl Default for UniFFIExtractArchiveConfig {
    fn default() -> Self {
        Self {
            archive_path: String::new(),
            destination_dir: String::new(),
            password: None,
            thread_budget: 0,
            overwrite_existing: true,
            preserve_permissions: true,
        }
    }
}

/// VFS Search Result item exposed to Swift.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIVfsMatch {
    pub path: String,
    pub name: String,
    pub is_directory: bool,
    pub size: u64,
}

/// VFS node summary record for zero-copy directory windowed paging.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIVfsNodeSummary {
    pub name: String,
    pub path: String,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub mtime_epoch_secs: i64,
    pub mode: u32,
    pub is_directory: bool,
    pub is_encrypted: bool,
    pub has_children: bool,
}

/// VFS windowed paging response record containing nodes and directory total entry count.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIVfsPagedResult {
    pub nodes: Vec<UniFFIVfsNodeSummary>,
    pub total_count: u32,
}

/// VFS aggregated tree statistics record.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIVfsStats {
    pub total_files: u64,
    pub total_dirs: u64,
    pub total_uncompressed_bytes: u64,
}

/// Password recovery result record.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct PasswordRecoveryOutcome {
    pub found_password: Option<String>,
    pub total_attempts: u64,
    pub elapsed_nanos: u64,
    pub attempts_per_second: f64,
}

/// Disk item summary record.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct DiskItemSummary {
    pub path: String,
    pub name: String,
    pub is_directory: bool,
    pub size: u64,
    pub mtime_epoch_secs: i64,
}

/// Sniffed file format and magic metadata record.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct SniffMetadata {
    pub format_name: String,
    pub mime_type: String,
    pub is_archive: bool,
    pub is_sfx: bool,
    pub sfx_offset: u64,
    pub confidence: u32,
}

/// Corrupted entry information in integrity verification.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFICorruptedEntry {
    pub path: String,
    pub expected_crc32: u32,
    pub actual_crc32: u32,
    pub reason: String,
}

/// Comprehensive archive integrity report.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIIntegrityReport {
    pub is_valid: bool,
    pub total_entries: u64,
    pub verified_entries: u64,
    pub corrupted_entries: Vec<UniFFICorruptedEntry>,
    pub elapsed_nanos: u64,
    pub error_message: Option<String>,
}

/// Path suggestion item for real-time autocompletion.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct PathSuggestionItem {
    pub full_path: String,
    pub display_name: String,
    pub is_directory: bool,
    pub is_archive: bool,
}

/// Parent directory and autocompletion prefix record.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIParentAndPrefix {
    pub parent_directory: String,
    pub prefix: String,
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

