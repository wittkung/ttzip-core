// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Mozilla UniFFI Cross-Language Lossless Error Mapping Subsystem.
//!
//! Provides typed, lossless mapping from internal engine errors to Swift,
//! Kotlin, and Python exception types with full context preservation.

use thiserror::Error;
use super::types::TTZipError as ApiTTZipError;
use crate::types::TTZipStatus;
use crate::zip::scanner::ZipEngineError;

/// Strongly-typed cross-language error enumeration exposed to foreign runtimes.
#[derive(Debug, Clone, PartialEq, Eq, Error, uniffi::Error)]
pub enum UniFFIError {
    #[error("I/O error: {message}")]
    IoError { message: String },

    #[error("Corrupt archive at offset {offset}: {message}")]
    CorruptArchive { message: String, offset: u64 },

    #[error("Invalid password: {message}")]
    InvalidPassword { message: String },

    #[error("Unsupported compression method: {method}")]
    UnsupportedCompression { method: String },

    #[error("Memory mapping error: {message}")]
    MmapError { message: String },

    #[error("Permission denied accessing path: {path}")]
    PermissionDenied { path: String },

    #[error("Out of memory: {message}")]
    OutOfMemory { message: String },

    #[error("Cryptographic error: {message}")]
    CryptoError { message: String },
}

impl UniFFIError {
    #[inline]
    pub fn io(msg: impl Into<String>) -> Self {
        Self::IoError { message: msg.into() }
    }

    #[inline]
    pub fn corrupt(msg: impl Into<String>, offset: u64) -> Self {
        Self::CorruptArchive {
            message: msg.into(),
            offset,
        }
    }

    #[inline]
    pub fn invalid_password(msg: impl Into<String>) -> Self {
        Self::InvalidPassword { message: msg.into() }
    }

    #[inline]
    pub fn unsupported_compression(method: impl Into<String>) -> Self {
        Self::UnsupportedCompression { method: method.into() }
    }

    #[inline]
    pub fn mmap(msg: impl Into<String>) -> Self {
        Self::MmapError { message: msg.into() }
    }

    #[inline]
    pub fn permission_denied(path: impl Into<String>) -> Self {
        Self::PermissionDenied { path: path.into() }
    }

    #[inline]
    pub fn out_of_memory(msg: impl Into<String>) -> Self {
        Self::OutOfMemory { message: msg.into() }
    }

    #[inline]
    pub fn crypto(msg: impl Into<String>) -> Self {
        Self::CryptoError { message: msg.into() }
    }
}

impl From<std::io::Error> for UniFFIError {
    fn from(err: std::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied {
                path: "unknown".to_string(),
            },
            std::io::ErrorKind::OutOfMemory => Self::OutOfMemory {
                message: err.to_string(),
            },
            _ => Self::IoError {
                message: err.to_string(),
            },
        }
    }
}

impl From<ApiTTZipError> for UniFFIError {
    fn from(err: ApiTTZipError) -> Self {
        match err {
            ApiTTZipError::FileNotFound { path } => Self::IoError {
                message: format!("File not found: {path}"),
            },
            ApiTTZipError::InvalidPassword => Self::InvalidPassword {
                message: "Invalid or missing archive password".to_string(),
            },
            ApiTTZipError::CorruptHeader { details, offset } => Self::CorruptArchive {
                message: details,
                offset,
            },
            ApiTTZipError::SecurityViolation { reason } => Self::PermissionDenied {
                path: reason,
            },
            ApiTTZipError::EngineError { code } => {
                if code == TTZipStatus::ErrOutOfMemory.to_i32() {
                    Self::OutOfMemory { message: "Engine allocation failure".to_string() }
                } else if code == TTZipStatus::ErrMmapFailed.to_i32() {
                    Self::MmapError { message: "Mmap allocation failed".to_string() }
                } else if code == TTZipStatus::ErrUnsupportedFeature.to_i32() {
                    Self::UnsupportedCompression { method: "Unknown feature".to_string() }
                } else {
                    Self::IoError {
                        message: format!("Engine error with status code {code}"),
                    }
                }
            }
            ApiTTZipError::IoError { message } => Self::IoError { message },
            ApiTTZipError::Cancelled => Self::IoError {
                message: "Operation was cancelled by caller".to_string(),
            },
            ApiTTZipError::InvalidSessionState { expected, actual } => Self::IoError {
                message: format!("Invalid session state: expected {expected}, actual {actual:?}"),
            },
            ApiTTZipError::InvalidResolution { detail } => Self::IoError {
                message: format!("Invalid resolution: {detail}"),
            },
            ApiTTZipError::DestinationUnavailable { errno } => Self::IoError {
                message: format!("Destination unavailable (errno {errno})"),
            },
            ApiTTZipError::InvalidArgument { message } => Self::IoError {
                message: format!("Invalid argument: {message}"),
            },
            ApiTTZipError::OutOfMemory { details } => Self::OutOfMemory { message: details },
            ApiTTZipError::DecompressionBombDetected { reason } => Self::PermissionDenied {
                path: format!("Decompression bomb: {reason}"),
            },
            ApiTTZipError::ChecksumMismatch { expected, actual } => Self::CorruptArchive {
                message: format!("Checksum mismatch: expected {expected:#x}, got {actual:#x}"),
                offset: 0,
            },
            ApiTTZipError::CreateTransactionIncomplete {
                summary_message, ..
            } => Self::IoError {
                message: summary_message,
            },
        }
    }
}

impl From<TTZipStatus> for UniFFIError {
    fn from(status: TTZipStatus) -> Self {
        match status {
            TTZipStatus::Ok | TTZipStatus::Eof => Self::IoError {
                message: status.as_str().to_string(),
            },
            TTZipStatus::Cancelled => Self::IoError {
                message: "Operation cancelled".to_string(),
            },
            TTZipStatus::ErrFileNotFound => Self::IoError {
                message: "File not found".to_string(),
            },
            TTZipStatus::ErrMmapFailed => Self::MmapError {
                message: "Memory mapping failed".to_string(),
            },
            TTZipStatus::ErrCorruptHeader | TTZipStatus::ErrInvalidOffset => Self::CorruptArchive {
                message: status.as_str().to_string(),
                offset: 0,
            },
            TTZipStatus::ErrOutOfMemory => Self::OutOfMemory {
                message: "System out of memory".to_string(),
            },
            TTZipStatus::ErrInvalidPassword => Self::InvalidPassword {
                message: "Invalid password".to_string(),
            },
            TTZipStatus::ErrUnsupportedFeature => Self::UnsupportedCompression {
                method: "Unsupported codec or feature".to_string(),
            },
            TTZipStatus::ErrSecurityViolation => Self::PermissionDenied {
                path: "Security policy boundary violation".to_string(),
            },
            _ => Self::IoError {
                message: status.as_str().to_string(),
            },
        }
    }
}

impl From<ZipEngineError> for UniFFIError {
    fn from(err: ZipEngineError) -> Self {
        match err {
            ZipEngineError::FileTooSmall { required, actual } => Self::CorruptArchive {
                message: format!("Archive too small: required {required}, actual {actual}"),
                offset: 0,
            },
            ZipEngineError::EocdNotFound => Self::CorruptArchive {
                message: "End of Central Directory record not found".to_string(),
                offset: 0,
            },
            ZipEngineError::InvalidCommentLength { declared, available } => Self::CorruptArchive {
                message: format!("Invalid comment length: declared {declared}, available {available}"),
                offset: 0,
            },
            ZipEngineError::CorruptedHeader(msg) => Self::CorruptArchive {
                message: msg,
                offset: 0,
            },
            ZipEngineError::InvalidCentralDirectoryBoundary { offset, size, file_len } => Self::CorruptArchive {
                message: format!("Invalid Central Directory boundary: offset {offset}, size {size}, file_len {file_len}"),
                offset,
            },
            ZipEngineError::Io(err) => Self::IoError { message: err.to_string() },
            ZipEngineError::Status(status) => status.into(),
        }
    }
}

/// Extension trait to convert generic Results into UniFFI typed Results.
pub trait IntoUniFFIResult<T> {
    fn into_uniffi(self) -> Result<T, UniFFIError>;
}

impl<T, E> IntoUniFFIResult<T> for Result<T, E>
where
    E: Into<UniFFIError>,
{
    #[inline]
    fn into_uniffi(self) -> Result<T, UniFFIError> {
        self.map_err(Into::into)
    }
}

/// Synchronously consumes secondary file-finish diagnostics carried by a clean
/// primary failure (`ConfirmedReleased` descriptors and zero on-disk/producer residuals)
/// BEFORE `map_create_stage_commit_error` returns `TTZipError`.
pub(crate) fn emit_clean_primary_failure_finish_diagnostics(
    status: crate::types::TTZipStatus,
    finish_diagnostics: &[ttzip_archive::archive::unified::create_transaction::StageFileFinishDiagnostic],
) {
    for diag in finish_diagnostics {
        log::warn!(
            target: "ttzip_engine::create_transaction",
            "clean_create_failure_finish_diagnostic primary_status={:?} diag_primary_status={:?} role={:?} path='{}' expected_identity={:?} prior_err={:?} flush_os_err={:?} flush_detail={:?} sync_os_err={:?} sync_detail={:?} close_os_err={:?} close_detail={:?} release_status={:?}",
            status,
            diag.primary_status,
            diag.role,
            diag.diagnostic_path.display(),
            diag.expected_identity,
            diag.prior_write_or_encoder_error,
            diag.userspace_flush_os_error,
            diag.userspace_flush_detail,
            diag.sync_os_error,
            diag.sync_detail,
            diag.close_os_error,
            diag.close_detail,
            diag.release_status,
        );
    }
}

#[inline]
fn map_clean_primary_status_to_uniffi(
    status: crate::types::TTZipStatus,
) -> super::types::TTZipError {
    use super::types::TTZipError;
    match status {
        crate::types::TTZipStatus::Cancelled => TTZipError::Cancelled,
        crate::types::TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
        s => TTZipError::EngineError { code: s.to_i32() },
    }
}

fn map_role_to_uniffi(
    role: ttzip_archive::archive::unified::create_transaction::ResidualPathRole,
) -> super::types::UniFFIResidualPathRole {
    use super::types::UniFFIResidualPathRole;
    use ttzip_archive::archive::unified::create_transaction::ResidualPathRole;
    match role {
        ResidualPathRole::StagedArtifact => UniFFIResidualPathRole::StagedArtifact,
        ResidualPathRole::StagedTempFile => UniFFIResidualPathRole::StagedTempFile,
        ResidualPathRole::UnownedPathOccupant => UniFFIResidualPathRole::UnownedPathOccupant,
        ResidualPathRole::TransactionStageDirectory => {
            UniFFIResidualPathRole::TransactionStageDirectory
        }
        ResidualPathRole::PreexistingBackupDirectory => {
            UniFFIResidualPathRole::PreexistingBackupDirectory
        }
        ResidualPathRole::FinalDestinationTarget => UniFFIResidualPathRole::FinalDestinationTarget,
    }
}

fn map_disposition_to_uniffi(
    disposition: ttzip_archive::archive::unified::create_transaction::ArchiveCleanupDisposition,
) -> super::types::UniFFIArchiveCleanupDisposition {
    use super::types::UniFFIArchiveCleanupDisposition;
    use ttzip_archive::archive::unified::create_transaction::ArchiveCleanupDisposition;
    match disposition {
        ArchiveCleanupDisposition::RefusedOwnershipMismatch => {
            UniFFIArchiveCleanupDisposition::RefusedOwnershipMismatch
        }
        ArchiveCleanupDisposition::OwnershipOrLocationUnconfirmed => {
            UniFFIArchiveCleanupDisposition::OwnershipOrLocationUnconfirmed
        }
        ArchiveCleanupDisposition::RestoredDisplacedSentinel => {
            UniFFIArchiveCleanupDisposition::RestoredDisplacedSentinel
        }
        ArchiveCleanupDisposition::RestoreDisplacedSentinelFailed => {
            UniFFIArchiveCleanupDisposition::RestoreDisplacedSentinelFailed
        }
        ArchiveCleanupDisposition::UnlinkOrRmdirIoError => {
            UniFFIArchiveCleanupDisposition::UnlinkOrRmdirIoError
        }
        ArchiveCleanupDisposition::StageDirectoryPreservedDueToInnerFailure => {
            UniFFIArchiveCleanupDisposition::StageDirectoryPreservedDueToInnerFailure
        }
        ArchiveCleanupDisposition::PostCommitCleanupFailed => {
            UniFFIArchiveCleanupDisposition::PostCommitCleanupFailed
        }
        ArchiveCleanupDisposition::DescriptorReleaseUnconfirmed => {
            UniFFIArchiveCleanupDisposition::DescriptorReleaseUnconfirmed
        }
    }
}

fn map_release_status_to_uniffi(
    status: ttzip_archive::archive::unified::create_transaction::DescriptorReleaseStatus,
) -> super::types::UniFFIDescriptorReleaseStatus {
    use super::types::UniFFIDescriptorReleaseStatus;
    use ttzip_archive::archive::unified::create_transaction::DescriptorReleaseStatus;
    match status {
        DescriptorReleaseStatus::ConfirmedReleased => {
            UniFFIDescriptorReleaseStatus::ConfirmedReleased
        }
        DescriptorReleaseStatus::ReleaseUnconfirmed => {
            UniFFIDescriptorReleaseStatus::ReleaseUnconfirmed
        }
    }
}

fn map_recovery_state_to_uniffi(
    state: ttzip_archive::archive::unified::create_transaction::CreateBackupRecoveryState,
) -> super::types::UniFFICreateBackupRecoveryState {
    use super::types::UniFFICreateBackupRecoveryState;
    use ttzip_archive::archive::unified::create_transaction::CreateBackupRecoveryState;
    match state {
        CreateBackupRecoveryState::NotApplicable => UniFFICreateBackupRecoveryState::NotApplicable,
        CreateBackupRecoveryState::RestoredAndVerified => {
            UniFFICreateBackupRecoveryState::RestoredAndVerified
        }
        CreateBackupRecoveryState::RollbackIncompletePreservedInBackup => {
            UniFFICreateBackupRecoveryState::RollbackIncompletePreservedInBackup
        }
    }
}

pub(crate) fn map_failure_report_to_uniffi(
    report: &ttzip_archive::archive::unified::create_transaction::CreateTransactionFailureReport,
) -> super::types::UniFFICreateTransactionFailureReport {
    use super::types::{
        UniFFIStageCleanupFailureItem, UniFFIStageFileFinishDiagnostic,
    };
    let cleanup_failures = report
        .cleanup_failures
        .iter()
        .map(|item| UniFFIStageCleanupFailureItem {
            role: map_role_to_uniffi(item.role),
            disposition: map_disposition_to_uniffi(item.disposition),
            path: item.final_known_path.as_ref().map(|p| p.display().to_string()),
            original_staged_path: Some(item.original_path.display().to_string()),
            os_error_code: item.os_error_code,
            message: item.detail.clone(),
        })
        .collect();
    let finish_diagnostics = report
        .finish_diagnostics
        .iter()
        .map(|diag| UniFFIStageFileFinishDiagnostic {
            role: map_role_to_uniffi(diag.role),
            diagnostic_path: diag.diagnostic_path.display().to_string(),
            expected_dev: diag.expected_identity.0,
            expected_ino: diag.expected_identity.1,
            primary_status_code: diag.primary_status.to_i32(),
            prior_write_or_encoder_error: diag.prior_write_or_encoder_error.clone(),
            userspace_flush_os_error: diag.userspace_flush_os_error,
            userspace_flush_detail: diag.userspace_flush_detail.clone(),
            sync_os_error: diag.sync_os_error,
            sync_detail: diag.sync_detail.clone(),
            close_os_error: diag.close_os_error,
            close_detail: diag.close_detail.clone(),
            release_status: map_release_status_to_uniffi(diag.release_status),
        })
        .collect();
    build_uniffi_report_with_recovery(report, cleanup_failures, finish_diagnostics)
}

fn build_uniffi_report_with_recovery(
    report: &ttzip_archive::archive::unified::create_transaction::CreateTransactionFailureReport,
    cleanup_failures: Vec<super::types::UniFFIStageCleanupFailureItem>,
    finish_diagnostics: Vec<super::types::UniFFIStageFileFinishDiagnostic>,
) -> super::types::UniFFICreateTransactionFailureReport {
    use super::types::{
        UniFFIBackupRecoveryEntryFailure, UniFFICreateBackupRecoveryState,
        UniFFICreateTransactionFailureReport,
    };
    let (
        recovery_state,
        backup_dir,
        restored_targets,
        rollback_failed_entries,
        backup_dir_cleanup_error,
    ) = match report.recovery_attempt.as_ref() {
        Some(attempt) => (
            map_recovery_state_to_uniffi(attempt.state),
            attempt.backup_dir.as_ref().map(|p| p.display().to_string()),
            attempt
                .restored_targets
                .iter()
                .map(|p| p.display().to_string())
                .collect(),
            attempt
                .failed_entries
                .iter()
                .map(|e| UniFFIBackupRecoveryEntryFailure {
                    backup_source_path: e.backup_source_path.display().to_string(),
                    target_path: e.target_path.display().to_string(),
                    os_error_code: e.os_error_code,
                    message: e.detail.clone(),
                })
                .collect(),
            attempt.backup_dir_cleanup_error.clone(),
        ),
        None => (
            UniFFICreateBackupRecoveryState::NotApplicable,
            None,
            Vec::new(),
            Vec::new(),
            None,
        ),
    };
    UniFFICreateTransactionFailureReport {
        primary_status_code: report.primary_status.to_i32(),
        was_primary_cancelled: report.was_primary_cancelled,
        is_cancellation_requested: report.is_cancellation_requested,
        stage_dir: report.stage_dir.display().to_string(),
        stage_dir_preserved: report.stage_dir_preserved,
        unconfirmed_release_count: report.unconfirmed_release_count,
        cleanup_failures,
        finish_diagnostics,
        recovery_state,
        backup_dir,
        restored_targets,
        rollback_failed_entries,
        backup_dir_cleanup_error,
        summary_message: report.summary_message(),
    }
}

fn map_legacy_stage_cleanup_incomplete_to_uniffi(
    status: crate::types::TTZipStatus,
    was_primary_cancelled: bool,
    uncommitted_stage_dir: &std::path::Path,
    lingering_staged_files: &[std::path::PathBuf],
    inner_cleanup_error: Option<&String>,
    outer_cleanup_error: Option<&String>,
    summary: String,
) -> super::types::UniFFICreateTransactionFailureReport {
    use super::types::{
        UniFFIArchiveCleanupDisposition, UniFFIResidualPathRole, UniFFIStageCleanupFailureItem,
    };
    let mut cleanup_failures: Vec<UniFFIStageCleanupFailureItem> = lingering_staged_files
        .iter()
        .map(|p| {
            let s = p.display().to_string();
            UniFFIStageCleanupFailureItem {
                role: UniFFIResidualPathRole::StagedArtifact,
                disposition: UniFFIArchiveCleanupDisposition::UnlinkOrRmdirIoError,
                path: Some(s.clone()),
                original_staged_path: Some(s),
                os_error_code: None,
                message: inner_cleanup_error.cloned().unwrap_or_default(),
            }
        })
        .collect();
    let dir_str = uncommitted_stage_dir.display().to_string();
    cleanup_failures.push(UniFFIStageCleanupFailureItem {
        role: UniFFIResidualPathRole::TransactionStageDirectory,
        disposition: UniFFIArchiveCleanupDisposition::StageDirectoryPreservedDueToInnerFailure,
        path: Some(dir_str.clone()),
        original_staged_path: Some(dir_str.clone()),
        os_error_code: None,
        message: outer_cleanup_error.cloned().unwrap_or_default(),
    });
    empty_legacy_report(
        status,
        was_primary_cancelled,
        dir_str,
        uncommitted_stage_dir.exists(),
        cleanup_failures,
        summary,
    )
}

fn map_legacy_stage_commit_error_to_uniffi(
    err: &crate::archive::unified::create::CreateStageCommitError,
) -> super::types::UniFFICreateTransactionFailureReport {
    use super::types::{
        UniFFIBackupRecoveryEntryFailure, UniFFICreateBackupRecoveryState,
        UniFFICreateTransactionFailureReport,
    };
    let status = err.status();
    let was_primary_cancelled = status == crate::types::TTZipStatus::Cancelled;
    match err {
        crate::archive::unified::create::CreateStageCommitError::StageCleanupIncomplete {
            uncommitted_stage_dir,
            lingering_staged_files,
            inner_cleanup_error,
            outer_cleanup_error,
            ..
        } => map_legacy_stage_cleanup_incomplete_to_uniffi(
            status,
            was_primary_cancelled,
            uncommitted_stage_dir,
            lingering_staged_files,
            inner_cleanup_error.as_ref(),
            outer_cleanup_error.as_ref(),
            err.to_string(),
        ),
        crate::archive::unified::create::CreateStageCommitError::RollbackIncomplete {
            backup_dir,
            unrestored_origins,
            ..
        } => {
            let rollback_failed_entries = unrestored_origins
                .iter()
                .map(|target| UniFFIBackupRecoveryEntryFailure {
                    backup_source_path: backup_dir
                        .join(target.file_name().unwrap_or_default())
                        .display()
                        .to_string(),
                    target_path: target.display().to_string(),
                    os_error_code: None,
                    message: err.to_string(),
                })
                .collect();
            UniFFICreateTransactionFailureReport {
                primary_status_code: status.to_i32(),
                was_primary_cancelled,
                is_cancellation_requested: was_primary_cancelled,
                stage_dir: String::new(),
                stage_dir_preserved: false,
                unconfirmed_release_count: 0,
                cleanup_failures: Vec::new(),
                finish_diagnostics: Vec::new(),
                recovery_state:
                    UniFFICreateBackupRecoveryState::RollbackIncompletePreservedInBackup,
                backup_dir: Some(backup_dir.display().to_string()),
                restored_targets: Vec::new(),
                rollback_failed_entries,
                backup_dir_cleanup_error: None,
                summary_message: err.to_string(),
            }
        }
        _ => empty_legacy_report(status, was_primary_cancelled, String::new(), false, Vec::new(), err.to_string()),
    }
}

fn empty_legacy_report(
    status: crate::types::TTZipStatus,
    was_primary_cancelled: bool,
    stage_dir: String,
    stage_dir_preserved: bool,
    cleanup_failures: Vec<super::types::UniFFIStageCleanupFailureItem>,
    summary_message: String,
) -> super::types::UniFFICreateTransactionFailureReport {
    use super::types::{UniFFICreateBackupRecoveryState, UniFFICreateTransactionFailureReport};
    UniFFICreateTransactionFailureReport {
        primary_status_code: status.to_i32(),
        was_primary_cancelled,
        is_cancellation_requested: was_primary_cancelled,
        stage_dir,
        stage_dir_preserved,
        unconfirmed_release_count: 0,
        cleanup_failures,
        finish_diagnostics: Vec::new(),
        recovery_state: UniFFICreateBackupRecoveryState::NotApplicable,
        backup_dir: None,
        restored_targets: Vec::new(),
        rollback_failed_entries: Vec::new(),
        backup_dir_cleanup_error: None,
        summary_message,
    }
}

pub(crate) fn map_create_stage_commit_error(
    err: crate::archive::unified::create::CreateStageCommitError,
) -> super::types::TTZipError {
    use super::types::TTZipError;
    use crate::archive::unified::create::CreateStageCommitError;
    match err {
        CreateStageCommitError::Status(status) => map_clean_primary_status_to_uniffi(status),
        CreateStageCommitError::StatusWithFinishDiagnostics {
            status,
            finish_diagnostics,
        } => {
            emit_clean_primary_failure_finish_diagnostics(status, &finish_diagnostics);
            map_clean_primary_status_to_uniffi(status)
        }
        CreateStageCommitError::TransactionIncomplete(report) => {
            let uniffi_report = map_failure_report_to_uniffi(&report);
            let summary_message = uniffi_report.summary_message.clone();
            TTZipError::CreateTransactionIncomplete {
                report: Box::new(uniffi_report),
                summary_message,
            }
        }
        other => {
            let uniffi_report = map_legacy_stage_commit_error_to_uniffi(&other);
            let summary_message = uniffi_report.summary_message.clone();
            TTZipError::CreateTransactionIncomplete {
                report: Box::new(uniffi_report),
                summary_message,
            }
        }
    }
}

pub(crate) fn build_post_commit_cleanup_outcome_from_issues(
    created_count: u64,
    issues: Vec<crate::archive::unified::create_staging::PostCommitCleanupIssue>,
) -> Option<super::extraction_session::ExtractionOutcome> {
    use super::extraction_session::{ExtractionOutcome, FailedItem, OutcomeStatus};
    if issues.is_empty() {
        return None;
    }
    let failed = issues
        .into_iter()
        .map(|issue| FailedItem {
            relative_path: issue.path.display().to_string(),
            reason: ttzip_archive::fs::collision::fs_ops::map_errno(issue.errno).into(),
            errno: issue.errno,
        })
        .collect();
    Some(ExtractionOutcome {
        status: OutcomeStatus::CompletedWithFailures,
        extracted: created_count,
        replaced: 0,
        kept_both: 0,
        skipped: 0,
        failed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniffi_error_variants_and_constructors() {
        let err1 = UniFFIError::io("Disk read error");
        assert!(matches!(err1, UniFFIError::IoError { .. }));
        assert_eq!(err1.to_string(), "I/O error: Disk read error");

        let err2 = UniFFIError::corrupt("Invalid magic", 0x1000);
        assert!(matches!(err2, UniFFIError::CorruptArchive { offset: 0x1000, .. }));

        let err3 = UniFFIError::invalid_password("Wrong key");
        assert!(matches!(err3, UniFFIError::InvalidPassword { .. }));

        let err4 = UniFFIError::unsupported_compression("ZSTD-Ultra");
        assert!(matches!(err4, UniFFIError::UnsupportedCompression { .. }));

        let err5 = UniFFIError::mmap("mprotect failed");
        assert!(matches!(err5, UniFFIError::MmapError { .. }));

        let err6 = UniFFIError::permission_denied("/etc/shadow");
        assert!(matches!(err6, UniFFIError::PermissionDenied { .. }));

        let err7 = UniFFIError::out_of_memory("16GB allocation failed");
        assert!(matches!(err7, UniFFIError::OutOfMemory { .. }));

        let err8 = UniFFIError::crypto("AES-GCM tag mismatch");
        assert!(matches!(err8, UniFFIError::CryptoError { .. }));
    }

    #[test]
    fn test_conversions_from_engine_and_std_errors() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let uniffi_err: UniFFIError = io_err.into();
        assert!(matches!(uniffi_err, UniFFIError::IoError { .. }));

        let status_err: UniFFIError = TTZipStatus::ErrOutOfMemory.into();
        assert!(matches!(status_err, UniFFIError::OutOfMemory { .. }));

        let status_mmap: UniFFIError = TTZipStatus::ErrMmapFailed.into();
        assert!(matches!(status_mmap, UniFFIError::MmapError { .. }));

        let zip_err: UniFFIError = ZipEngineError::EocdNotFound.into();
        assert!(matches!(zip_err, UniFFIError::CorruptArchive { .. }));

        let api_err = ApiTTZipError::CorruptHeader {
            details: "Bad central directory".to_string(),
            offset: 4096,
        };
        let mapped: UniFFIError = api_err.into();
        assert_eq!(mapped, UniFFIError::CorruptArchive {
            message: "Bad central directory".to_string(),
            offset: 4096,
        });
    }

    #[test]
    fn test_into_uniffi_result_extension() {
        let res: Result<u32, std::io::Error> = Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        let uniffi_res = res.into_uniffi();
        assert!(matches!(uniffi_res, Err(UniFFIError::PermissionDenied { .. })));

        let ok_res: Result<u32, std::io::Error> = Ok(42);
        assert_eq!(ok_res.into_uniffi().unwrap(), 42);
    }
}
