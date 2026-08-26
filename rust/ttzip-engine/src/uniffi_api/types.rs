// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

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

