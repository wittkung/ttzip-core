// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Archive creation transaction context, finish/cleanup diagnostics, and failure reports.

use std::fmt;
use std::io;
use std::os::fd::{OwnedFd, RawFd};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use crate::fs::abort_guard::{FsAbortCleanupDisposition, FsAbortCleanupError};
use crate::types::TTZipStatus;

pub(crate) use super::create_guard_settlement::run_finish_and_close_file;
pub use super::create_guard_settlement::{
    set_create_txn_close_fault_hook, set_create_txn_sync_fault_hook,
};
pub use crate::fs::abort_guard::{set_abort_guard_dup_fd_hook, set_abort_guard_fstat_hook};
pub use crate::fs::finish_io::{
    CloseScopePrecondition, DescriptorReleaseStatus, ExplicitFileFinishProof,
    FileFinishSyncPolicy, StageFileFinishError, classify_close_release_status_for_scope,
    close_owned_fd_once, close_owned_file_once, explicit_finish_and_close_file_production,
    finish_and_close_file_for_proof, flush_and_close_buf_writer_for_proof,
    flush_and_close_buf_writer_once,
};

/// Access mode for exclusively created staged files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateFileAccessMode {
    WriteOnly,
    ReadWrite,
}
/// Structural role of a path involved in a staging or recovery failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualPathRole {
    StagedArtifact,
    StagedTempFile,
    UnownedPathOccupant,
    TransactionStageDirectory,
    PreexistingBackupDirectory,
    FinalDestinationTarget,
}
/// Structural reason why cleanup or recovery could not cleanly complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveCleanupDisposition {
    RefusedOwnershipMismatch,
    OwnershipOrLocationUnconfirmed,
    RestoredDisplacedSentinel,
    RestoreDisplacedSentinelFailed,
    UnlinkOrRmdirIoError,
    StageDirectoryPreservedDueToInnerFailure,
    PostCommitCleanupFailed,
    DescriptorReleaseUnconfirmed,
}
/// Serializable diagnostic record preserving write/encoder, `BufWriter::flush`,
/// optional `sync_all`, and explicit `close(2)` errors alongside [`DescriptorReleaseStatus`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageFileFinishDiagnostic {
    pub role: ResidualPathRole,
    pub diagnostic_path: PathBuf,
    pub expected_identity: (u64, u64),
    pub primary_status: TTZipStatus,
    pub prior_write_or_encoder_error: Option<String>,
    pub userspace_flush_os_error: Option<i32>,
    pub userspace_flush_detail: Option<String>,
    pub sync_os_error: Option<i32>,
    pub sync_detail: Option<String>,
    pub close_os_error: Option<i32>,
    pub close_detail: Option<String>,
    pub release_status: DescriptorReleaseStatus,
}
impl StageFileFinishDiagnostic {
    #[must_use]
    pub fn from_finish_error(
        role: ResidualPathRole,
        diagnostic_path: PathBuf,
        expected_identity: (u64, u64),
        err: &StageFileFinishError,
    ) -> Self {
        Self {
            role,
            diagnostic_path,
            expected_identity,
            primary_status: err.primary_status,
            prior_write_or_encoder_error: err.prior_write_or_encoder_error.clone(),
            userspace_flush_os_error: err.userspace_flush_error.as_ref().and_then(io::Error::raw_os_error),
            userspace_flush_detail: err.userspace_flush_error.as_ref().map(ToString::to_string),
            sync_os_error: err.sync_error.as_ref().and_then(io::Error::raw_os_error),
            sync_detail: err.sync_error.as_ref().map(ToString::to_string),
            close_os_error: err.close_error.as_ref().and_then(io::Error::raw_os_error),
            close_detail: err.close_error.as_ref().map(ToString::to_string),
            release_status: err.release_status,
        }
    }
}
/// Point-of-failure structured cleanup record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageCleanupFailure {
    pub role: ResidualPathRole,
    pub disposition: ArchiveCleanupDisposition,
    pub original_path: PathBuf,
    pub final_known_path: Option<PathBuf>,
    pub expected_identity: Option<(u64, u64)>,
    pub observed_identity: Option<(u64, u64)>,
    pub os_error_code: Option<i32>,
    pub detail: String,
}
impl StageCleanupFailure {
    pub(crate) fn from_fs_abort_error(role: ResidualPathRole, err: FsAbortCleanupError) -> Self {
        let disposition = match err.disposition {
            FsAbortCleanupDisposition::RefusedOwnershipMismatch => ArchiveCleanupDisposition::RefusedOwnershipMismatch,
            FsAbortCleanupDisposition::OwnershipOrLocationUnconfirmed => ArchiveCleanupDisposition::OwnershipOrLocationUnconfirmed,
            FsAbortCleanupDisposition::RestoredDisplacedSentinel => ArchiveCleanupDisposition::RestoredDisplacedSentinel,
            FsAbortCleanupDisposition::RestoreDisplacedSentinelFailed => ArchiveCleanupDisposition::RestoreDisplacedSentinelFailed,
            FsAbortCleanupDisposition::UnlinkOrRmdirIoError => ArchiveCleanupDisposition::UnlinkOrRmdirIoError,
        };
        Self {
            role,
            disposition,
            original_path: err.original_path,
            final_known_path: err.final_known_path,
            expected_identity: err.expected_identity,
            observed_identity: err.observed_identity,
            os_error_code: err.os_error_code,
            detail: err.detail,
        }
    }
}

impl fmt::Display for StageCleanupFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{:?} / {:?}] '{}': {}",
            self.role,
            self.disposition,
            self.original_path.display(),
            self.detail
        )
    }
}

/// Per-entry rollback failure detail during split-volume backup restoration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupRecoveryEntryFailure {
    pub backup_source_path: PathBuf,
    pub target_path: PathBuf,
    pub os_error_code: Option<i32>,
    pub detail: String,
}
/// Outcome of attempting to restore `.ttzip_create_bak_<pid>_<nanos>_<seq>` after split-volume commit failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateBackupRecoveryState {
    NotApplicable,
    RestoredAndVerified,
    RollbackIncompletePreservedInBackup,
}
/// Structured record of split-volume backup & recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateBackupRecoveryAttempt {
    pub state: CreateBackupRecoveryState,
    pub backup_dir: Option<PathBuf>,
    pub restored_targets: Vec<PathBuf>,
    pub failed_entries: Vec<BackupRecoveryEntryFailure>,
    pub backup_dir_cleanup_error: Option<String>,
}
/// Immutable failure report frozen when a creation transaction fails with unresolved responsibilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateTransactionFailureReport {
    pub primary_status: TTZipStatus,
    pub was_primary_cancelled: bool,
    pub is_cancellation_requested: bool,
    pub stage_dir: PathBuf,
    pub stage_dir_preserved: bool,
    pub unconfirmed_release_count: u32,
    pub cleanup_failures: Vec<StageCleanupFailure>,
    pub finish_diagnostics: Vec<StageFileFinishDiagnostic>,
    pub recovery_attempt: Option<Box<CreateBackupRecoveryAttempt>>,
}
impl CreateTransactionFailureReport {
    /// Formats a concise human-readable summary of the incomplete creation transaction.
    #[must_use]
    pub fn summary_message(&self) -> String {
        let recovery_state = self
            .recovery_attempt
            .as_ref()
            .map_or(CreateBackupRecoveryState::NotApplicable, |r| r.state);
        format!(
            "Archive creation transaction incomplete (primary_status={:?} ({}), was_primary_cancelled={}, is_cancellation_requested={}, stage_dir='{}', stage_dir_preserved={}, unconfirmed_release_count={}, cleanup_failures={}, finish_diagnostics={}, recovery_state={:?})",
            self.primary_status,
            self.primary_status.to_i32(),
            self.was_primary_cancelled,
            self.is_cancellation_requested,
            self.stage_dir.display(),
            self.stage_dir_preserved,
            self.unconfirmed_release_count,
            self.cleanup_failures.len(),
            self.finish_diagnostics.len(),
            recovery_state,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransactionPhase {
    Open,
    Finalizing,
    Settled,
}

type AfterCancelSampleHook = Box<dyn FnOnce() + Send + 'static>;

struct CreateTransactionState {
    stage_dir: PathBuf,
    captured_parent: Option<(RawFd, (u64, u64))>,
    shared_guard_parent: Option<Arc<OwnedFd>>,
    phase: TransactionPhase,
    active_producers: usize,
    is_cancellation_requested: bool,
    requires_outer_preservation: bool,
    unconfirmed_release_count: u32,
    inner_failures: Vec<StageCleanupFailure>,
    outer_failures: Vec<StageCleanupFailure>,
    finish_diagnostics: Vec<StageFileFinishDiagnostic>,
    recovery_attempt: Option<CreateBackupRecoveryAttempt>,
    after_final_cancel_sample_before_unlock_hook: Option<AfterCancelSampleHook>,
}

/// Synchronous transaction context tracking active producers, staged file finish diagnostics,
/// inner/outer cleanup failures, and split-volume backup recovery across an archive creation call.
#[derive(Clone)]
pub struct CreateTransactionContext {
    inner: Arc<Mutex<CreateTransactionState>>,
}

impl std::panic::UnwindSafe for CreateTransactionContext {}
impl std::panic::RefUnwindSafe for CreateTransactionContext {}

impl std::fmt::Debug for CreateTransactionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateTransactionContext").finish_non_exhaustive()
    }
}

/// RAII ticket representing an active producer holding a staged artifact inside [`CreateTransactionContext`].
#[derive(Debug)]
pub struct ProducerTicket {
    ctx: CreateTransactionContext,
}

impl Drop for ProducerTicket {
    fn drop(&mut self) {
        let mut st = self.ctx.inner.lock().unwrap_or_else(PoisonError::into_inner);
        st.active_producers = st.active_producers.saturating_sub(1);
    }
}

impl CreateTransactionContext {
    /// Creates a new transaction context rooted at `stage_dir`.
    #[must_use]
    pub fn new(stage_dir: PathBuf) -> Self {
        Self::build(stage_dir, None)
    }

    /// Creates a new transaction context rooted at `stage_dir` with captured `(parent_fd, parent_id)`.
    #[must_use]
    pub fn with_captured_parent(stage_dir: PathBuf, parent_fd: RawFd, parent_id: (u64, u64)) -> Self {
        Self::build(stage_dir, Some((parent_fd, parent_id)))
    }

    fn build(stage_dir: PathBuf, captured_parent: Option<(RawFd, (u64, u64))>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(CreateTransactionState {
                stage_dir,
                captured_parent,
                shared_guard_parent: None,
                phase: TransactionPhase::Open,
                active_producers: 0,
                is_cancellation_requested: false,
                requires_outer_preservation: false,
                unconfirmed_release_count: 0,
                inner_failures: Vec::new(),
                outer_failures: Vec::new(),
                finish_diagnostics: Vec::new(),
                recovery_attempt: None,
                after_final_cancel_sample_before_unlock_hook: None,
            })),
        }
    }

    /// Returns the captured parent directory descriptor and identity if `parent` matches `stage_dir`.
    #[must_use]
    pub fn captured_parent_for(&self, parent: &Path) -> Option<(RawFd, (u64, u64))> {
        let st = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        (st.stage_dir == parent).then_some(st.captured_parent).flatten()
    }

    /// Reuses the owned stage-directory anchor across guards without per-volume dup.
    pub(crate) fn shared_guard_parent(&self, parent: &Path) -> Option<Arc<OwnedFd>> {
        let st = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        (st.stage_dir == parent).then(|| st.shared_guard_parent.clone()).flatten()
    }

    pub(crate) fn retain_guard_parent(&self, parent: &Path, fd: Arc<OwnedFd>) -> Arc<OwnedFd> {
        let mut st = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if st.stage_dir != parent {
            return fd;
        }
        st.shared_guard_parent.get_or_insert(fd).clone()
    }

    /// Admits a new producer prior to any filesystem side effect.
    pub fn try_admit_producer(&self) -> io::Result<ProducerTicket> {
        let mut st = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if st.phase != TransactionPhase::Open {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "archive creation transaction is already finalizing or settled",
            ));
        }
        st.active_producers += 1;
        drop(st);
        Ok(ProducerTicket { ctx: self.clone() })
    }

    /// Returns the current number of active producer tickets.
    #[must_use]
    pub fn active_producers(&self) -> usize {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner).active_producers
    }

    /// Records that cancellation was observed by a progress or cancellation probe.
    pub fn mark_cancellation_requested(&self) {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner).is_cancellation_requested = true;
    }

    /// Records an inner file/temp cleanup or ownership failure (idempotent for identical entries).
    pub fn record_inner_failure(&self, failure: StageCleanupFailure) {
        let mut st = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        st.requires_outer_preservation = true;
        let exists = st.inner_failures.iter().any(|e| {
            e.role == failure.role
                && e.disposition == failure.disposition
                && e.original_path == failure.original_path
                && e.final_known_path == failure.final_known_path
        });
        if !exists {
            st.inner_failures.push(failure);
        }
    }

    /// Returns `true` if `inner_failures` already contains an entry for `path`.
    #[must_use]
    pub fn has_failures_for_path(&self, path: &Path) -> bool {
        let st = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        st.inner_failures
            .iter()
            .any(|e| e.original_path == path || e.final_known_path.as_deref() == Some(path))
    }

    /// Records an outer staging directory cleanup or preservation failure.
    pub fn record_outer_failure(&self, failure: StageCleanupFailure) {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner).outer_failures.push(failure);
    }

    /// Records a per-file finish diagnostic.
    pub fn record_finish_diagnostic(&self, diag: StageFileFinishDiagnostic) {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner).finish_diagnostics.push(diag);
    }

    /// Records an unconfirmed descriptor release (`EINTR` or unknown close failure).
    pub fn record_unconfirmed_descriptor_release(
        &self,
        role: ResidualPathRole,
        original_path: PathBuf,
        expected_identity: Option<(u64, u64)>,
        os_error_code: Option<i32>,
        detail: String,
    ) {
        let mut st = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        st.unconfirmed_release_count = st.unconfirmed_release_count.saturating_add(1);
        st.requires_outer_preservation = true;
        st.inner_failures.push(StageCleanupFailure {
            role,
            disposition: ArchiveCleanupDisposition::DescriptorReleaseUnconfirmed,
            original_path,
            final_known_path: None,
            expected_identity,
            observed_identity: None,
            os_error_code,
            detail,
        });
    }

    /// Records the split-volume backup recovery outcome.
    pub fn set_recovery_attempt(&self, attempt: CreateBackupRecoveryAttempt) {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner).recovery_attempt = Some(attempt);
    }

    /// Marks that the outer staging directory must be preserved.
    pub fn mark_requires_outer_preservation(&self) {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner).requires_outer_preservation = true;
    }

    /// Transitions the transaction to `Finalizing` and checks whether the pre-commit gate is clean.
    #[must_use]
    pub fn begin_finalization_and_check_pre_commit_gate(&self) -> bool {
        let mut st = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if st.phase == TransactionPhase::Open {
            st.phase = TransactionPhase::Finalizing;
        }
        if st.active_producers > 0 {
            st.requires_outer_preservation = true;
            return false;
        }
        st.unconfirmed_release_count == 0
            && st.inner_failures.is_empty()
            && !st.requires_outer_preservation
    }

    /// Returns `true` if inner cleanup failures, unconfirmed releases, or undrained producers require preserving `.ttzip_create_txn_*`.
    #[must_use]
    pub fn should_preserve_outer_stage_dir(&self) -> bool {
        let st = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        st.requires_outer_preservation
            || st.active_producers > 0
            || st.unconfirmed_release_count > 0
            || !st.inner_failures.is_empty()
    }

    /// Returns `true` if the failed transaction has zero unresolved responsibilities and can return a clean primary error.
    #[must_use]
    pub fn is_clean_for_primary_error(&self, stage_dir_preserved: bool) -> bool {
        let st = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        !stage_dir_preserved
            && st.active_producers == 0
            && st.unconfirmed_release_count == 0
            && st.inner_failures.is_empty()
            && st.outer_failures.is_empty()
            && !st.requires_outer_preservation
            && !matches!(
                st.recovery_attempt.as_ref().map(|r| r.state),
                Some(CreateBackupRecoveryState::RollbackIncompletePreservedInBackup)
            )
    }

    /// Moves `finish_diagnostics` out of `self` when settling a clean primary failure before `self` is dropped.
    #[must_use]
    pub fn take_finish_diagnostics_for_clean_failure(&self) -> Vec<StageFileFinishDiagnostic> {
        let mut st = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        st.phase = TransactionPhase::Settled;
        std::mem::take(&mut st.finish_diagnostics)
    }

    /// Marks the transaction as cleanly settled after successful target commit.
    pub fn mark_settled_clean(&self) {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner).phase = TransactionPhase::Settled;
    }

    /// Registers a deterministic hook invoked inside `freeze_failure_report` immediately after `T_sample` (`AC-U1-12b`).
    pub fn set_after_final_cancel_sample_before_unlock_hook<F>(&self, hook: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .after_final_cancel_sample_before_unlock_hook = Some(Box::new(hook));
    }

    /// Freezes an immutable [`CreateTransactionFailureReport`] at the terminal sampling point `T_sample`.
    #[must_use]
    pub fn freeze_failure_report(
        &self,
        primary_status: TTZipStatus,
        stage_dir_preserved: bool,
        cancel_probe: impl FnOnce() -> bool,
    ) -> CreateTransactionFailureReport {
        let mut st = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let sampled_cancel = cancel_probe();
        if let Some(hook) = st.after_final_cancel_sample_before_unlock_hook.take() {
            hook();
        }
        let was_primary_cancelled = primary_status == TTZipStatus::Cancelled;
        let is_cancellation_requested =
            st.is_cancellation_requested || sampled_cancel || was_primary_cancelled;
        st.phase = TransactionPhase::Settled;
        let mut cleanup_failures = st.inner_failures.clone();
        cleanup_failures.extend(st.outer_failures.iter().cloned());
        CreateTransactionFailureReport {
            primary_status,
            was_primary_cancelled,
            is_cancellation_requested,
            stage_dir: st.stage_dir.clone(),
            stage_dir_preserved,
            unconfirmed_release_count: st.unconfirmed_release_count,
            cleanup_failures,
            finish_diagnostics: st.finish_diagnostics.clone(),
            recovery_attempt: st.recovery_attempt.clone().map(Box::new),
        }
    }
}
