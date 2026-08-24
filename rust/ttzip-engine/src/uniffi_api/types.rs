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

/// VFS aggregated tree statistics record.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIVfsStats {
    pub total_files: u64,
    pub total_dirs: u64,
    pub total_uncompressed_bytes: u64,
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
