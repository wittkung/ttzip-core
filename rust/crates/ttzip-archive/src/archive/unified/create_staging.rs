// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Transactional staging, single-file atomic replacement, and multi-volume verified rollback
//! with backup preservation and structured transaction failure reporting for archive creation.

use std::fmt;
use std::fs;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::archive::split::VolumeNamingScheme;
use crate::types::{TTZipArchiveFormat, TTZipStatus};

use super::create_guard_settlement::{
    finalize_failed_stage_dir_with_ctx, settle_committed_stage_dir, CreateStagingGuard,
};
pub use super::create_split_rollback::recover_preserved_create_backup;
use super::create_split_rollback::commit_split_staged_volumes_in_txn_with;
use super::create_transaction::{
    CreateBackupRecoveryState, CreateTransactionContext, CreateTransactionFailureReport,
    ResidualPathRole, StageFileFinishDiagnostic,
};
pub use super::{
    clear_create_stage_diagnostics, record_inner_stage_cleanup_error,
    record_inner_stage_cleanup_error_with_status, record_post_commit_cleanup_issue,
    set_create_stage_after_work_hook, set_create_stage_before_work_hook,
    take_last_inner_stage_cleanup_error, take_last_post_commit_cleanup_issues,
    PostCommitCleanupIssue,
};
use super::{run_after_work_hook, run_before_work_hook};

static CREATE_TXN_SEQ: AtomicU64 = AtomicU64::new(1);

/// Explicit commit mode and volume naming scheme supplied by the archive creation caller.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CreateCommitMode {
    /// Unsplit single-file archive creation (`split_volume_size_bytes == 0`).
    SingleFile,
    /// Multi-volume split archive creation (`split_volume_size_bytes > 0`).
    SplitVolume(VolumeNamingScheme),
}

impl CreateCommitMode {
    /// Resolves the canonical split base path (stripping a trailing `.001` if present) and
    /// `VolumeNamingScheme` for split-volume creation.
    pub fn resolve_split_target(
        destination_path: &Path,
        resolved_format: TTZipArchiveFormat,
    ) -> (PathBuf, VolumeNamingScheme) {
        if destination_path.extension().and_then(|e| e.to_str()) == Some("001") {
            (destination_path.with_extension(""), VolumeNamingScheme::NumberedExtension)
        } else if matches!(resolved_format, TTZipArchiveFormat::Zip | TTZipArchiveFormat::Auto) {
            (destination_path.to_path_buf(), VolumeNamingScheme::PkzipSpanned)
        } else {
            (destination_path.to_path_buf(), VolumeNamingScheme::NumberedExtension)
        }
    }

    /// Resolves the explicit `CreateCommitMode` from `destination_path`, `resolved_format`,
    /// and `split_volume_size_bytes`.
    pub fn for_creation(
        destination_path: &Path,
        resolved_format: TTZipArchiveFormat,
        split_volume_size_bytes: u64,
    ) -> Self {
        if split_volume_size_bytes == 0 {
            return Self::SingleFile;
        }
        let (_, scheme) = Self::resolve_split_target(destination_path, resolved_format);
        Self::SplitVolume(scheme)
    }
}

/// Successful staged commit output carrying committed destination paths and any non-fatal
/// post-commit cleanup issues (`.ttzip_create_txn_*` or `.ttzip_create_bak_*` directory removal
/// after target installation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateStagedCommitOutput {
    pub committed_files: Vec<PathBuf>,
    pub post_commit_cleanup_issues: Vec<PostCommitCleanupIssue>,
}

/// Detailed error returned when staging or committing a newly created archive fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateStageCommitError {
    /// Compression or commit failed with all pre-existing destination files intact or cleanly restored.
    Status(TTZipStatus),
    /// Compression or commit failed cleanly (all descriptors `ConfirmedReleased`, zero residual
    /// cleanup/rollback/producer responsibilities), carrying secondary file-finish diagnostics
    /// so `map_create_stage_commit_error` can log them before folding to the clean primary error.
    StatusWithFinishDiagnostics {
        status: TTZipStatus,
        finish_diagnostics: Vec<StageFileFinishDiagnostic>,
    },
    /// Structured transaction-incomplete report carrying unresolved inner/outer cleanup failures,
    /// unconfirmed descriptor releases (`unconfirmed_release_count > 0`), active producer leaks,
    /// or split-volume backup rollback failures.
    TransactionIncomplete(CreateTransactionFailureReport),
    /// Archive creation failed before commit, and cleanup of the uncommitted transaction staging
    /// directory (`.ttzip_create_txn_*`) or inner staged file failed. `backup_dir()` is `None`.
    StageCleanupIncomplete {
        status: TTZipStatus,
        uncommitted_stage_dir: PathBuf,
        lingering_staged_files: Vec<PathBuf>,
        inner_cleanup_error: Option<String>,
        outer_cleanup_error: Option<String>,
    },
    /// Multi-volume commit failed and one or more pre-existing volumes could not be restored from
    /// `backup_dir`. `backup_dir` is preserved on disk so no backup copy is lost.
    RollbackIncomplete {
        status: TTZipStatus,
        backup_dir: PathBuf,
        unrestored_origins: Vec<PathBuf>,
        lingering_installed: Vec<PathBuf>,
    },
}

impl CreateStageCommitError {
    /// Returns the underlying `TTZipStatus` code.
    pub fn status(&self) -> TTZipStatus {
        match self {
            Self::Status(s) | Self::StatusWithFinishDiagnostics { status: s, .. } => *s,
            Self::TransactionIncomplete(report) => report.primary_status,
            Self::StageCleanupIncomplete { status, .. } => *status,
            Self::RollbackIncomplete { status, .. } => *status,
        }
    }

    /// Returns the preserved `.ttzip_create_bak_*` backup directory path ONLY when multi-volume rollback was incomplete.
    pub fn backup_dir(&self) -> Option<&Path> {
        match self {
            Self::RollbackIncomplete { backup_dir, .. } => Some(backup_dir.as_path()),
            Self::TransactionIncomplete(report) => match report.recovery_attempt.as_ref() {
                Some(attempt)
                    if attempt.state
                        == CreateBackupRecoveryState::RollbackIncompletePreservedInBackup
                        || attempt.backup_dir_cleanup_error.is_some() =>
                {
                    attempt.backup_dir.as_deref()
                }
                _ => None,
            },
            Self::Status(_)
            | Self::StatusWithFinishDiagnostics { .. }
            | Self::StageCleanupIncomplete { .. } => None,
        }
    }

    /// Returns secondary file-finish diagnostics carried by this error.
    pub fn finish_diagnostics(&self) -> &[StageFileFinishDiagnostic] {
        match self {
            Self::StatusWithFinishDiagnostics {
                finish_diagnostics, ..
            } => finish_diagnostics.as_slice(),
            Self::TransactionIncomplete(report) => report.finish_diagnostics.as_slice(),
            _ => &[],
        }
    }

    /// Consumes `self`, emitting any clean-failure `finish_diagnostics` to `log::warn!` before returning `TTZipStatus`.
    pub fn log_and_into_status(self) -> TTZipStatus {
        if let Self::StatusWithFinishDiagnostics {
            status,
            finish_diagnostics,
        } = &self
        {
            for diag in finish_diagnostics {
                log::warn!(
                    target: "ttzip_engine::create_transaction",
                    "clean primary archive creation failure ({:?}) accompanied by file-finish diagnostic: path='{}', role={:?}, expected_identity={:?}, prior_err={:?}, flush_os_err={:?}, sync_os_err={:?}, close_os_err={:?}, release_status={:?}",
                    status,
                    diag.diagnostic_path.display(),
                    diag.role,
                    diag.expected_identity,
                    diag.prior_write_or_encoder_error,
                    diag.userspace_flush_os_error,
                    diag.sync_os_error,
                    diag.close_os_error,
                    diag.release_status,
                );
            }
        }
        self.status()
    }

    /// Returns the uncommitted `.ttzip_create_txn_*` staging directory path when pre-commit stage cleanup failed.
    pub fn uncommitted_stage_dir(&self) -> Option<&Path> {
        match self {
            Self::StageCleanupIncomplete {
                uncommitted_stage_dir,
                ..
            } => Some(uncommitted_stage_dir.as_path()),
            Self::TransactionIncomplete(report) if report.stage_dir_preserved => {
                Some(report.stage_dir.as_path())
            }
            _ => None,
        }
    }

    /// Returns the original target paths of volumes that remain inside `backup_dir`.
    pub fn unrestored_origins(&self) -> &[PathBuf] {
        match self {
            Self::RollbackIncomplete {
                unrestored_origins, ..
            } => unrestored_origins.as_slice(),
            _ => &[],
        }
    }

    /// Returns the structured `CreateTransactionFailureReport` when present.
    pub fn transaction_failure_report(&self) -> Option<&CreateTransactionFailureReport> {
        match self {
            Self::TransactionIncomplete(report) => Some(report),
            _ => None,
        }
    }
}

impl fmt::Display for CreateStageCommitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Status(status) | Self::StatusWithFinishDiagnostics { status, .. } => {
                write!(f, "Archive creation failed with status {:?}", status)
            }
            Self::TransactionIncomplete(report) => {
                write!(f, "TransactionIncomplete: {}", report.summary_message())
            }
            Self::StageCleanupIncomplete {
                status,
                uncommitted_stage_dir,
                lingering_staged_files,
                inner_cleanup_error,
                outer_cleanup_error,
            } => write!(
                f,
                "StageCleanupIncomplete: Archive creation failed before commit ({:?}) and uncommitted stage cleanup failed for '{}'; inner_cleanup_error={:?}, outer_cleanup_error={:?}, lingering_staged_files={:?}",
                status,
                uncommitted_stage_dir.display(),
                inner_cleanup_error,
                outer_cleanup_error,
                lingering_staged_files
            ),
            Self::RollbackIncomplete {
                status,
                backup_dir,
                unrestored_origins,
                lingering_installed,
            } => write!(
                f,
                "RollbackIncomplete: Multi-volume archive commit failed ({:?}) and rollback was incomplete; preserved backup directory '{}' contains unrestored volume(s): {:?} (lingering installed: {:?})",
                status,
                backup_dir.display(),
                unrestored_origins,
                lingering_installed
            ),
        }
    }
}

pub(crate) fn next_txn_token() -> (u32, u128, u64) {
    let seq = CREATE_TXN_SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    (std::process::id(), nanos, seq)
}

fn collect_staged_files(txn_dir: &Path) -> Result<Vec<PathBuf>, CreateStageCommitError> {
    let entries = fs::read_dir(txn_dir)
        .map_err(|_| CreateStageCommitError::Status(TTZipStatus::ErrOpenFailed))?;
    let mut staged_files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| CreateStageCommitError::Status(TTZipStatus::ErrOpenFailed))?;
        staged_files.push(entry.path());
    }
    if staged_files.is_empty() {
        return Err(CreateStageCommitError::Status(TTZipStatus::ErrCompressionFailed));
    }
    staged_files.sort();
    Ok(staged_files)
}

fn commit_single_staged_file_with<R>(
    staged_files: &[PathBuf],
    parent: &Path,
    base_file_name: &str,
    mut rename_staged: R,
) -> Result<Vec<PathBuf>, CreateStageCommitError>
where
    R: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    if staged_files.len() != 1
        || staged_files[0].file_name().and_then(|n| n.to_str()) != Some(base_file_name)
    {
        return Err(CreateStageCommitError::Status(
            TTZipStatus::ErrCompressionFailed,
        ));
    }
    let dst_file = parent.join(base_file_name);
    rename_staged(&staged_files[0], &dst_file)
        .map_err(|_| CreateStageCommitError::Status(TTZipStatus::ErrOpenFailed))?;
    Ok(vec![dst_file])
}

/// Commits staged archive output into `parent` according to `mode`, recording any split-volume
/// backup recovery or post-commit cleanup diagnostics into `txn_ctx`.
pub fn commit_staged_archive_files_in_txn_with<R, B>(
    txn_dir: &Path,
    parent: &Path,
    base_file_name: &str,
    mode: CreateCommitMode,
    txn_ctx: Option<&CreateTransactionContext>,
    rename_staged: R,
    restore_backup: B,
) -> Result<CreateStagedCommitOutput, CreateStageCommitError>
where
    R: FnMut(&Path, &Path) -> std::io::Result<()>,
    B: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    let staged_files = collect_staged_files(txn_dir)?;
    match mode {
        CreateCommitMode::SingleFile => {
            let committed_files =
                commit_single_staged_file_with(&staged_files, parent, base_file_name, rename_staged)?;
            Ok(CreateStagedCommitOutput {
                committed_files,
                post_commit_cleanup_issues: Vec::new(),
            })
        }
        CreateCommitMode::SplitVolume(scheme) => {
            let (committed_files, post_commit_cleanup_issues) =
                commit_split_staged_volumes_in_txn_with(
                    staged_files,
                    parent,
                    base_file_name,
                    scheme,
                    txn_ctx,
                    rename_staged,
                    restore_backup,
                )?;
            Ok(CreateStagedCommitOutput {
                committed_files,
                post_commit_cleanup_issues,
            })
        }
    }
}

/// Commits staged archive output into `parent` according to the explicit caller-supplied `mode`.
pub fn commit_staged_archive_files_with<R, B>(
    txn_dir: &Path,
    parent: &Path,
    base_file_name: &str,
    mode: CreateCommitMode,
    rename_staged: R,
    restore_backup: B,
) -> Result<Vec<PathBuf>, CreateStageCommitError>
where
    R: FnMut(&Path, &Path) -> std::io::Result<()>,
    B: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    commit_staged_archive_files_in_txn_with(
        txn_dir,
        parent,
        base_file_name,
        mode,
        None,
        rename_staged,
        restore_backup,
    )
    .map(|out| out.committed_files)
}

fn validate_and_prepare_destination_parent(
    destination_path: &Path,
) -> Result<(&std::ffi::OsStr, &str, &Path), CreateStageCommitError> {
    let file_name = destination_path
        .file_name()
        .ok_or(CreateStageCommitError::Status(TTZipStatus::ErrInvalidParam))?;
    let base_file_name = file_name
        .to_str()
        .ok_or(CreateStageCommitError::Status(TTZipStatus::ErrInvalidParam))?;
    let parent = destination_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.exists() {
        fs::create_dir_all(parent)
            .map_err(|_| CreateStageCommitError::Status(TTZipStatus::ErrOpenFailed))?;
    }
    Ok((file_name, base_file_name, parent))
}

/// Executes `work` against a staged destination path and `CreateTransactionContext`, enforcing
/// the 6-step pre-commit responsibility gate and returning `CreateStagedCommitOutput` or
/// structured `CreateStageCommitError`.
pub fn with_staged_archive_destination_detailed_using_outcome<F, R, B, C>(
    destination_path: &Path,
    mode: CreateCommitMode,
    work: F,
    rename_staged: R,
    restore_backup: B,
    cancel_probe: C,
) -> Result<CreateStagedCommitOutput, CreateStageCommitError>
where
    F: FnOnce(&Path, &CreateTransactionContext) -> Result<(), TTZipStatus>,
    R: FnMut(&Path, &Path) -> std::io::Result<()>,
    B: FnMut(&Path, &Path) -> std::io::Result<()>,
    C: FnOnce() -> bool,
{
    clear_create_stage_diagnostics();
    let (file_name, base_file_name, parent) =
        validate_and_prepare_destination_parent(destination_path)?;

    let mut guard = CreateStagingGuard::create(parent)?;
    let txn_dir = guard.txn_dir.clone();
    let txn_ctx = CreateTransactionContext::with_captured_parent(
        txn_dir.clone(),
        guard.txn_fd.as_raw_fd(),
        guard.txn_id,
    );
    run_before_work_hook(&txn_dir);

    let staged_dest = txn_dir.join(file_name);
    let work_res: Result<(), TTZipStatus> = match std::panic::catch_unwind(
        std::panic::AssertUnwindSafe(|| work(&staged_dest, &txn_ctx)),
    ) {
        Ok(res) => res,
        Err(_) => Err(TTZipStatus::ErrPanicCaught),
    };
    run_after_work_hook(&txn_dir, &work_res);

    let pre_commit_clean = txn_ctx.begin_finalization_and_check_pre_commit_gate();
    let commit_outcome = match work_res {
        Ok(()) if !pre_commit_clean => {
            Err(CreateStageCommitError::Status(TTZipStatus::ErrCompressionFailed))
        }
        Ok(()) => commit_staged_archive_files_in_txn_with(
            &txn_dir,
            parent,
            base_file_name,
            mode,
            Some(&txn_ctx),
            rename_staged,
            restore_backup,
        ),
        Err(status) => Err(CreateStageCommitError::Status(status)),
    };

    match commit_outcome {
        Ok(output) => Ok(settle_committed_stage_dir(&mut guard, &txn_ctx, output)),
        Err(err) => Err(finalize_failed_stage_dir_with_ctx(
            &mut guard,
            &txn_ctx,
            err,
            cancel_probe,
        )),
    }
}

/// Executes `work` against a staged destination path and `CreateTransactionContext` using standard
/// `fs::rename` hooks, returning `CreateStagedCommitOutput` or `CreateStageCommitError`.
pub fn with_staged_archive_destination_detailed_outcome<F, C>(
    destination_path: &Path,
    mode: CreateCommitMode,
    work: F,
    cancel_probe: C,
) -> Result<CreateStagedCommitOutput, CreateStageCommitError>
where
    F: FnOnce(&Path, &CreateTransactionContext) -> Result<(), TTZipStatus>,
    C: FnOnce() -> bool,
{
    with_staged_archive_destination_detailed_using_outcome(
        destination_path,
        mode,
        work,
        |from, to| fs::rename(from, to),
        |from, to| fs::rename(from, to),
        cancel_probe,
    )
}

/// Executes `work` against a staged destination path and commits generated output using explicit
/// `mode` and custom `rename_staged` / `restore_backup` hooks, returning detailed rollback diagnostics.
pub fn with_staged_archive_destination_detailed_using<F, R, B>(
    destination_path: &Path,
    mode: CreateCommitMode,
    work: F,
    rename_staged: R,
    restore_backup: B,
) -> Result<Vec<PathBuf>, CreateStageCommitError>
where
    F: FnOnce(&Path) -> Result<(), TTZipStatus>,
    R: FnMut(&Path, &Path) -> std::io::Result<()>,
    B: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    with_staged_archive_destination_detailed_using_outcome(
        destination_path,
        mode,
        |staged_dest, _txn_ctx| work(staged_dest),
        rename_staged,
        restore_backup,
        || false,
    )
    .map(|out| out.committed_files)
    .map_err(|err| match err {
        CreateStageCommitError::TransactionIncomplete(ref report)
            if report.unconfirmed_release_count == 0
                && report
                    .recovery_attempt
                    .as_ref()
                    .is_some_and(|r| r.state == CreateBackupRecoveryState::RollbackIncompletePreservedInBackup) =>
        {
            let rec = report.recovery_attempt.as_ref().unwrap();
            let lingering_installed = report
                .cleanup_failures
                .iter()
                .filter(|f| f.role == ResidualPathRole::FinalDestinationTarget)
                .map(|f| f.original_path.clone())
                .collect();
            CreateStageCommitError::RollbackIncomplete {
                status: report.primary_status,
                backup_dir: rec.backup_dir.clone().unwrap_or_default(),
                unrestored_origins: rec.failed_entries.iter().map(|e| e.target_path.clone()).collect(),
                lingering_installed,
            }
        }
        other => other,
    })
}

/// Executes `work` against a staged destination path and commits according to `mode`, returning detailed rollback diagnostics.
pub fn with_staged_archive_destination_detailed<F>(
    destination_path: &Path,
    mode: CreateCommitMode,
    work: F,
) -> Result<Vec<PathBuf>, CreateStageCommitError>
where
    F: FnOnce(&Path) -> Result<(), TTZipStatus>,
{
    with_staged_archive_destination_detailed_using(
        destination_path,
        mode,
        work,
        |from, to| fs::rename(from, to),
        |from, to| fs::rename(from, to),
    )
}

/// Executes `work` against a staged destination path with explicit `mode` and a custom `rename_staged` hook.
pub fn with_staged_archive_destination_using<F, R>(
    destination_path: &Path,
    mode: CreateCommitMode,
    work: F,
    rename_staged: R,
) -> Result<Vec<PathBuf>, TTZipStatus>
where
    F: FnOnce(&Path) -> Result<(), TTZipStatus>,
    R: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    with_staged_archive_destination_detailed_using(
        destination_path,
        mode,
        work,
        rename_staged,
        |from, to| fs::rename(from, to),
    )
    .map_err(CreateStageCommitError::log_and_into_status)
}

/// Executes `work` against a staged destination path and commits generated output according to `mode`.
pub fn with_staged_archive_destination<F>(
    destination_path: &Path,
    mode: CreateCommitMode,
    work: F,
) -> Result<Vec<PathBuf>, TTZipStatus>
where
    F: FnOnce(&Path) -> Result<(), TTZipStatus>,
{
    with_staged_archive_destination_detailed(destination_path, mode, work)
        .map_err(CreateStageCommitError::log_and_into_status)
}

pub(crate) use super::{
    check_xar_progress, insert_xar_tree_node, render_xar_tree_xml, stream_compress_xar_file,
    validate_create_encryption_options, ScopedTempFile, XarNodeKind, XarStreamState, XarTreeNode,
};
