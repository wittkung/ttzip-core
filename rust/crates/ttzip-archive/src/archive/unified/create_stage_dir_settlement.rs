// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Outer transaction stage-directory guard and commit/failure settlement.

use std::ffi::CString;
use std::os::fd::{AsRawFd, OwnedFd};
use std::path::{Path, PathBuf};

use crate::fs::abort_guard::open_dir_nofollow;
use crate::fs::collision::staging::{
    OwnedDirRemovalError, OwnedDirRemovalErrorKind, verify_and_remove_owned_dir_typed,
};
use crate::types::TTZipStatus;

use super::create_staging::{CreateStageCommitError, CreateStagedCommitOutput, next_txn_token};
use super::create_transaction::{
    ArchiveCleanupDisposition, CreateTransactionContext, ResidualPathRole, StageCleanupFailure,
};

pub(crate) struct CreateStagingGuard {
    pub(crate) parent_path: PathBuf,
    pub(crate) parent_fd: OwnedFd,
    pub(crate) parent_id: (u64, u64),
    pub(crate) txn_dir: PathBuf,
    pub(crate) txn_name: CString,
    pub(crate) txn_fd: OwnedFd,
    pub(crate) txn_id: (u64, u64),
    pub(crate) attempted_cleanup: bool,
}

impl CreateStagingGuard {
    pub(crate) fn create(parent: &Path) -> Result<Self, CreateStageCommitError> {
        let (parent_fd, parent_id) = open_dir_nofollow(parent)
            .map_err(|_| CreateStageCommitError::Status(TTZipStatus::ErrOpenFailed))?;
        let (pid, nanos, seq) = next_txn_token();
        let name = format!(".ttzip_create_txn_{pid}_{nanos}_{seq}");
        let txn_name = CString::new(name.as_bytes())
            .map_err(|_| CreateStageCommitError::Status(TTZipStatus::ErrInvalidParam))?;
        let txn_dir = parent.join(&name);
        if unsafe { libc::mkdirat(parent_fd.as_raw_fd(), txn_name.as_ptr(), 0o700) } != 0 {
            return Err(CreateStageCommitError::Status(TTZipStatus::ErrOpenFailed));
        }
        let (txn_fd, txn_id) = open_dir_nofollow(&txn_dir)
            .map_err(|_| CreateStageCommitError::Status(TTZipStatus::ErrOpenFailed))?;
        Ok(Self {
            parent_path: parent.to_path_buf(),
            parent_fd,
            parent_id,
            txn_dir,
            txn_name,
            txn_fd,
            txn_id,
            attempted_cleanup: false,
        })
    }

    pub(crate) fn cleanup_checked(&mut self) -> Result<(), OwnedDirRemovalError> {
        self.attempted_cleanup = true;
        verify_and_remove_owned_dir_typed(
            &self.parent_path,
            self.parent_fd.as_raw_fd(),
            self.parent_id,
            &self.txn_dir,
            &self.txn_name,
            self.txn_fd.as_raw_fd(),
            self.txn_id,
        )
    }
}

impl Drop for CreateStagingGuard {
    fn drop(&mut self) {
        if !self.attempted_cleanup {
            let _ = self.cleanup_checked();
        }
    }
}

fn absorb_legacy_inner_record_into_ctx(
    txn_ctx: &CreateTransactionContext,
    current_primary: TTZipStatus,
) -> TTZipStatus {
    let Some((rec_status, rec_path, rec_msg)) = super::take_last_inner_stage_cleanup_record() else {
        return current_primary;
    };
    let effective_status = match rec_status {
        Some(s) if current_primary == TTZipStatus::ErrCompressionFailed => s,
        _ => current_primary,
    };
    if !txn_ctx.has_failures_for_path(&rec_path) {
        let disposition = if rec_msg.contains("InvalidData") || rec_msg.contains("Refused") {
            ArchiveCleanupDisposition::RefusedOwnershipMismatch
        } else {
            ArchiveCleanupDisposition::UnlinkOrRmdirIoError
        };
        txn_ctx.record_inner_failure(StageCleanupFailure {
            role: ResidualPathRole::StagedArtifact,
            disposition,
            original_path: rec_path.clone(),
            final_known_path: rec_path.exists().then_some(rec_path),
            expected_identity: None,
            observed_identity: None,
            os_error_code: None,
            detail: rec_msg,
        });
    }
    effective_status
}

fn record_outer_cleanup_error_into_ctx(
    guard: &CreateStagingGuard,
    txn_ctx: &CreateTransactionContext,
    err: &OwnedDirRemovalError,
    stage_dir_preserved: bool,
) {
    let disposition = match err.kind {
        OwnedDirRemovalErrorKind::IdentityMismatchRestored => {
            ArchiveCleanupDisposition::RestoredDisplacedSentinel
        }
        OwnedDirRemovalErrorKind::IdentityMismatchRestoreOccupied => {
            ArchiveCleanupDisposition::RestoreDisplacedSentinelFailed
        }
        OwnedDirRemovalErrorKind::IdentityMismatchLocationUnconfirmed => {
            ArchiveCleanupDisposition::OwnershipOrLocationUnconfirmed
        }
        OwnedDirRemovalErrorKind::OwnershipMismatchRefused => {
            ArchiveCleanupDisposition::RefusedOwnershipMismatch
        }
        OwnedDirRemovalErrorKind::IoError => ArchiveCleanupDisposition::UnlinkOrRmdirIoError,
    };
    let io_err = &err.io_err;
    if let Ok(entries) = std::fs::read_dir(&guard.txn_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if !txn_ctx.has_failures_for_path(&p) {
                txn_ctx.record_inner_failure(StageCleanupFailure {
                    role: ResidualPathRole::StagedArtifact,
                    disposition: ArchiveCleanupDisposition::UnlinkOrRmdirIoError,
                    original_path: p.clone(),
                    final_known_path: Some(p),
                    expected_identity: None,
                    observed_identity: None,
                    os_error_code: io_err.raw_os_error(),
                    detail: format!("Staged file remained in '{}': {io_err}", guard.txn_dir.display()),
                });
            }
        }
    }
    txn_ctx.record_outer_failure(StageCleanupFailure {
        role: ResidualPathRole::TransactionStageDirectory,
        disposition,
        original_path: guard.txn_dir.clone(),
        final_known_path: stage_dir_preserved.then(|| guard.txn_dir.clone()),
        expected_identity: Some(guard.txn_id),
        observed_identity: None,
        os_error_code: io_err.raw_os_error(),
        detail: format!(
            "Failed to clean outer transaction stage dir '{}': {io_err}",
            guard.txn_dir.display()
        ),
    });
}

pub(crate) fn finalize_failed_stage_dir_with_ctx<C>(
    guard: &mut CreateStagingGuard,
    txn_ctx: &CreateTransactionContext,
    err: CreateStageCommitError,
    cancel_probe: C,
) -> CreateStageCommitError
where
    C: FnOnce() -> bool,
{
    let primary_status = absorb_legacy_inner_record_into_ctx(txn_ctx, err.status());
    let preserve_outer = txn_ctx.should_preserve_outer_stage_dir();
    let stage_dir_preserved = if preserve_outer {
        guard.attempted_cleanup = true;
        txn_ctx.record_outer_failure(StageCleanupFailure {
            role: ResidualPathRole::TransactionStageDirectory,
            disposition: ArchiveCleanupDisposition::StageDirectoryPreservedDueToInnerFailure,
            original_path: guard.txn_dir.clone(),
            final_known_path: Some(guard.txn_dir.clone()),
            expected_identity: Some(guard.txn_id),
            observed_identity: Some(guard.txn_id),
            os_error_code: None,
            detail: format!(
                "outer stage directory '{}' preserved because inner file/handle/producer responsibility remains unresolved",
                guard.txn_dir.display()
            ),
        });
        true
    } else {
        match guard.cleanup_checked() {
            Ok(()) => false,
            Err(cleanup_err) => {
                let preserved = guard.txn_dir.exists();
                record_outer_cleanup_error_into_ctx(guard, txn_ctx, &cleanup_err, preserved);
                preserved
            }
        }
    };

    if txn_ctx.is_clean_for_primary_error(stage_dir_preserved) {
        let finish_diagnostics = txn_ctx.take_finish_diagnostics_for_clean_failure();
        txn_ctx.mark_settled_clean();
        if finish_diagnostics.is_empty() {
            return CreateStageCommitError::Status(primary_status);
        }
        return CreateStageCommitError::StatusWithFinishDiagnostics {
            status: primary_status,
            finish_diagnostics,
        };
    }

    let report = txn_ctx.freeze_failure_report(primary_status, stage_dir_preserved, cancel_probe);
    CreateStageCommitError::TransactionIncomplete(report)
}

pub(crate) fn settle_committed_stage_dir(
    guard: &mut CreateStagingGuard,
    txn_ctx: &CreateTransactionContext,
    mut output: CreateStagedCommitOutput,
) -> CreateStagedCommitOutput {
    if let Err(cleanup_err) = guard.cleanup_checked() {
        let io_err = cleanup_err.into_io_error();
        let txn_dir = guard.txn_dir.clone();
        super::record_post_commit_cleanup_issue(
            txn_dir.clone(),
            "committed_txn_stage_dir",
            &io_err,
        );
        output
            .post_commit_cleanup_issues
            .push(super::PostCommitCleanupIssue {
                path: txn_dir,
                role: "committed_txn_stage_dir",
                message: format!("committed_txn_stage_dir: {io_err}"),
                errno: io_err.raw_os_error().unwrap_or(libc::EIO),
            });
    }
    txn_ctx.mark_settled_clean();
    output
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;

    #[test]
    fn test_record_outer_cleanup_error_maps_typed_kind_without_parsing_message() {
        let cases = [
            (
                OwnedDirRemovalErrorKind::IdentityMismatchRestored,
                ArchiveCleanupDisposition::RestoredDisplacedSentinel,
            ),
            (
                OwnedDirRemovalErrorKind::IdentityMismatchRestoreOccupied,
                ArchiveCleanupDisposition::RestoreDisplacedSentinelFailed,
            ),
            (
                OwnedDirRemovalErrorKind::IdentityMismatchLocationUnconfirmed,
                ArchiveCleanupDisposition::OwnershipOrLocationUnconfirmed,
            ),
            (
                OwnedDirRemovalErrorKind::OwnershipMismatchRefused,
                ArchiveCleanupDisposition::RefusedOwnershipMismatch,
            ),
            (
                OwnedDirRemovalErrorKind::IoError,
                ArchiveCleanupDisposition::UnlinkOrRmdirIoError,
            ),
        ];

        let tmp = tempfile::tempdir().expect("tempdir");
        for (kind, expected_disposition) in cases {
            let guard = CreateStagingGuard::create(tmp.path()).expect("create staging guard");
            let txn_ctx = CreateTransactionContext::new(guard.txn_dir.clone());
            let err = OwnedDirRemovalError {
                kind,
                io_err: io::Error::new(io::ErrorKind::InvalidData, "message without outcome token"),
            };
            record_outer_cleanup_error_into_ctx(&guard, &txn_ctx, &err, true);
            let report = txn_ctx.freeze_failure_report(TTZipStatus::ErrOpenFailed, true, || false);
            assert_eq!(report.cleanup_failures.len(), 1);
            assert_eq!(report.cleanup_failures[0].disposition, expected_disposition);
        }
    }
}
