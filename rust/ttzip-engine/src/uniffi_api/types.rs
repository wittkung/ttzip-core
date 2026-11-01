// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

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

    #[error("Invalid session state: expected {expected}, actual {actual:?}")]
    InvalidSessionState { expected: String, actual: crate::uniffi_api::extraction_session::SessionState },

    #[error("Invalid resolution: {detail}")]
    InvalidResolution { detail: String },

    #[error("Destination unavailable (errno {errno})")]
    DestinationUnavailable { errno: i32 },

    #[error("Invalid argument: {message}")]
    InvalidArgument { message: String },

    #[error("Out of memory during decompression: {details}")]
    OutOfMemory { details: String },

    #[error("Decompression bomb detected: {reason}")]
    DecompressionBombDetected { reason: String },

    #[error("Data checksum mismatch: expected {expected:#x}, got {actual:#x}")]
    ChecksumMismatch { expected: u32, actual: u32 },

    #[error("{summary_message}")]
    CreateTransactionIncomplete {
        report: Box<UniFFICreateTransactionFailureReport>,
        summary_message: String,
    },
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

macro_rules! define_canonical_archive_formats {
    ($($variant:ident),* $(,)?) => {
        /// Archive format enum exposed to Swift and UniFFI SDKs.
        #[derive(Copy, Clone, Debug, PartialEq, Eq, uniffi::Enum)]
        pub enum ArchiveFormat {
            $($variant),*
        }

        /// Canonical alias for TTZip archive format exposed via UniFFI.
        pub type TTZipArchiveFormat = ArchiveFormat;

        impl From<ArchiveFormat> for crate::types::TTZipArchiveFormat {
            fn from(fmt: ArchiveFormat) -> Self {
                match fmt {
                    $(ArchiveFormat::$variant => crate::types::TTZipArchiveFormat::$variant,)*
                }
            }
        }

        impl From<crate::types::TTZipArchiveFormat> for ArchiveFormat {
            fn from(fmt: crate::types::TTZipArchiveFormat) -> Self {
                match fmt {
                    $(crate::types::TTZipArchiveFormat::$variant => ArchiveFormat::$variant,)*
                    _ => ArchiveFormat::Auto,
                }
            }
        }
    };
}

define_canonical_archive_formats!(
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
);

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
    pub outcome: Option<super::extraction_session::ExtractionOutcome>,
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

/// Structural role of a path involved in an incomplete creation transaction across FFI.
#[derive(Copy, Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum UniFFIResidualPathRole {
    StagedArtifact,
    StagedTempFile,
    UnownedPathOccupant,
    TransactionStageDirectory,
    PreexistingBackupDirectory,
    FinalDestinationTarget,
}

/// Structural cleanup disposition for a failed creation transaction item across FFI.
#[derive(Copy, Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum UniFFIArchiveCleanupDisposition {
    RefusedOwnershipMismatch,
    OwnershipOrLocationUnconfirmed,
    RestoredDisplacedSentinel,
    RestoreDisplacedSentinelFailed,
    UnlinkOrRmdirIoError,
    StageDirectoryPreservedDueToInnerFailure,
    PostCommitCleanupFailed,
    DescriptorReleaseUnconfirmed,
}

/// Process descriptor release status across FFI.
#[derive(Copy, Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum UniFFIDescriptorReleaseStatus {
    ConfirmedReleased,
    ReleaseUnconfirmed,
}

/// Per-file finish diagnostic record across FFI.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIStageFileFinishDiagnostic {
    pub role: UniFFIResidualPathRole,
    pub diagnostic_path: String,
    pub expected_dev: u64,
    pub expected_ino: u64,
    pub primary_status_code: i32,
    pub prior_write_or_encoder_error: Option<String>,
    pub userspace_flush_os_error: Option<i32>,
    pub userspace_flush_detail: Option<String>,
    pub sync_os_error: Option<i32>,
    pub sync_detail: Option<String>,
    pub close_os_error: Option<i32>,
    pub close_detail: Option<String>,
    pub release_status: UniFFIDescriptorReleaseStatus,
}

/// Per-item structured cleanup failure record across FFI.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIStageCleanupFailureItem {
    pub role: UniFFIResidualPathRole,
    pub disposition: UniFFIArchiveCleanupDisposition,
    /// Confirmed current on-disk path (`Some` when known, `None` when location/ownership or descriptor release is unconfirmed).
    pub path: Option<String>,
    /// Historical/expected staged path at the start of the operation (diagnostic reference only).
    pub original_staged_path: Option<String>,
    pub os_error_code: Option<i32>,
    pub message: String,
}

/// Backup recovery state across FFI.
#[derive(Copy, Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum UniFFICreateBackupRecoveryState {
    NotApplicable,
    RestoredAndVerified,
    RollbackIncompletePreservedInBackup,
}

/// Per-entry backup rollback failure across FFI.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIBackupRecoveryEntryFailure {
    pub backup_source_path: String,
    pub target_path: String,
    pub os_error_code: Option<i32>,
    pub message: String,
}

/// Complete creation transaction failure report across FFI.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFICreateTransactionFailureReport {
    /// Raw status code from `TTZipStatus::to_i32()` (`Cancelled = 2`, `ErrOpenFailed = -7`, etc.).
    pub primary_status_code: i32,
    /// True iff `primary_status == TTZipStatus::Cancelled` (`2`).
    pub was_primary_cancelled: bool,
    /// True if cancellation was requested at or before the transaction settlement linearization point `T_sample`.
    pub is_cancellation_requested: bool,
    pub stage_dir: String,
    pub stage_dir_preserved: bool,
    pub unconfirmed_release_count: u32,
    pub cleanup_failures: Vec<UniFFIStageCleanupFailureItem>,
    pub finish_diagnostics: Vec<UniFFIStageFileFinishDiagnostic>,
    pub recovery_state: UniFFICreateBackupRecoveryState,
    pub backup_dir: Option<String>,
    pub restored_targets: Vec<String>,
    pub rollback_failed_entries: Vec<UniFFIBackupRecoveryEntryFailure>,
    pub backup_dir_cleanup_error: Option<String>,
    pub summary_message: String,
}

// SAFETY: `Box<UniFFICreateTransactionFailureReport>` is strictly a Rust-side heap indirection
// used to bound stack frame size for `Result<_, TTZipError>`. Across the FFI boundary it uses
// the identical `RustBuffer` wire format, `TYPE_ID_META` schema metadata, and `write`/`try_read`
// serialization by delegating directly to `<UniFFICreateTransactionFailureReport as FfiConverter>`.
// No raw `Box` pointer address is ever exposed across FFI, and generated Swift/Python bindings
// decode the value as the matching `UniFFICreateTransactionFailureReport` record.
unsafe impl ::uniffi::FfiConverter<crate::UniFfiTag> for Box<UniFFICreateTransactionFailureReport> {
    ::uniffi::ffi_converter_rust_buffer_lift_and_lower!(crate::UniFfiTag);

    fn write(obj: Self, buf: &mut ::std::vec::Vec<u8>) {
        <UniFFICreateTransactionFailureReport as ::uniffi::FfiConverter<crate::UniFfiTag>>::write(
            *obj, buf,
        );
    }

    fn try_read(buf: &mut &[u8]) -> ::uniffi::deps::anyhow::Result<Self> {
        <UniFFICreateTransactionFailureReport as ::uniffi::FfiConverter<crate::UniFfiTag>>::try_read(
            buf,
        )
        .map(Box::new)
    }

    const TYPE_ID_META: ::uniffi::MetadataBuffer =
        <UniFFICreateTransactionFailureReport as ::uniffi::FfiConverter<crate::UniFfiTag>>::TYPE_ID_META;
}

::uniffi::derive_ffi_traits!(local Box<UniFFICreateTransactionFailureReport>);

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

