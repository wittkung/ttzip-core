// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Transaction-aware [`FileAbortGuard`] creation, explicit finish, and cleanup settlement.

use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::fs::FileAbortGuard;
use crate::fs::abort_guard::{
    FsAbortCleanupError, FsAbortObserver, VerifiedGuardParts, dup_fd_cloexec, fstat_dev_ino,
    fstatat_dev_ino_nofollow, open_dir_nofollow, path_dev_ino_nofollow,
    quarantine_verify_and_unlink,
};
use crate::fs::finish_io::{
    CloseScopePrecondition, DescriptorReleaseStatus, ExplicitFileFinishProof,
    FileFinishSyncPolicy, StageFileFinishError, close_owned_fd_once,
    finish_and_close_file_for_proof,
};
use crate::types::TTZipStatus;

use super::create_transaction::{
    ArchiveCleanupDisposition, CreateFileAccessMode, CreateTransactionContext, ProducerTicket,
    ResidualPathRole, StageCleanupFailure, StageFileFinishDiagnostic,
};

type SyncFaultHook = Box<dyn FnOnce(&File) -> io::Result<()> + Send + 'static>;
type CloseFaultHook = Box<
    dyn FnOnce(OwnedFd, CloseScopePrecondition) -> (RawFd, io::Result<()>, DescriptorReleaseStatus)
        + Send
        + 'static,
>;

thread_local! {
    static CREATE_TXN_SYNC_FAULT_HOOK: RefCell<Option<SyncFaultHook>> =
        const { RefCell::new(None) };
    static CREATE_TXN_CLOSE_FAULT_HOOK: RefCell<Option<CloseFaultHook>> =
        const { RefCell::new(None) };
}

/// Registers a one-shot hook on the current thread invoked during pre-close `sync_all` in creation transactions.
pub fn set_create_txn_sync_fault_hook<F>(hook: F)
where
    F: FnOnce(&File) -> io::Result<()> + Send + 'static,
{
    CREATE_TXN_SYNC_FAULT_HOOK.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
}

/// Registers a one-shot hook on the current thread invoked at the single-close boundary in creation transactions.
pub fn set_create_txn_close_fault_hook<F>(hook: F)
where
    F: FnOnce(OwnedFd, CloseScopePrecondition) -> (RawFd, io::Result<()>, DescriptorReleaseStatus)
        + Send
        + 'static,
{
    CREATE_TXN_CLOSE_FAULT_HOOK.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
}

#[derive(Debug)]
struct GuardAbortObserver {
    ctx: Option<CreateTransactionContext>,
    role: ResidualPathRole,
    _ticket: Option<ProducerTicket>,
}

impl FsAbortObserver for GuardAbortObserver {
    fn on_abort_cleanup_error(&self, err: FsAbortCleanupError) {
        if let Some(ctx) = &self.ctx {
            ctx.record_inner_failure(StageCleanupFailure::from_fs_abort_error(self.role, err));
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn record_openat_eexist_occupant(
    txn_ctx: Option<&CreateTransactionContext>,
    parent_raw_fd: RawFd,
    leaf_c: &CStr,
    path: &Path,
) {
    let Some(ctx) = txn_ctx else {
        return;
    };
    let observed_id = fstatat_dev_ino_nofollow(parent_raw_fd, leaf_c)
        .ok()
        .map(|(d, i, _)| (d, i));
    ctx.record_inner_failure(StageCleanupFailure {
        role: ResidualPathRole::UnownedPathOccupant,
        disposition: ArchiveCleanupDisposition::RefusedOwnershipMismatch,
        original_path: path.to_path_buf(),
        final_known_path: observed_id.map(|_| path.to_path_buf()),
        expected_identity: None,
        observed_identity: observed_id,
        os_error_code: Some(libc::EEXIST),
        detail: format!("Refused to overwrite pre-existing occupant at '{}'", path.display()),
    });
}

struct ResolvedCreateParent {
    owned_parent_opt: Option<Arc<OwnedFd>>,
    parent_raw_fd: RawFd,
    expected_parent_id: (u64, u64),
    parent_path: PathBuf,
    leaf_c: CString,
}

fn resolve_and_verify_create_parent(
    path: &Path,
    role: ResidualPathRole,
    txn_ctx: Option<&CreateTransactionContext>,
) -> io::Result<ResolvedCreateParent> {
    let parent_path = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let leaf_os = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing file name"))?;
    let leaf_c = CString::new(leaf_os.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))?;
    let shared = txn_ctx.and_then(|ctx| ctx.shared_guard_parent(&parent_path));
    let captured = txn_ctx.and_then(|ctx| ctx.captured_parent_for(&parent_path));
    let (owned_parent_opt, parent_raw_fd, expected_parent_id) = match (shared, captured) {
        (Some(fd), _) => {
            let id = crate::fs::abort_guard::fstat_dev_ino(fd.as_raw_fd())?;
            let raw = fd.as_raw_fd();
            (Some(fd), raw, id)
        }
        (None, Some((pfd, pid))) => (None, pfd, pid),
        (None, None) => {
            let (pfd, pid) = open_dir_nofollow(&parent_path)?;
            let raw = pfd.as_raw_fd();
            (Some(Arc::new(pfd)), raw, pid)
        }
    };
    let parent_path_id = path_dev_ino_nofollow(&parent_path).ok().map(|(d, i, _)| (d, i));
    let parent_fd_id = crate::fs::abort_guard::fd_stat_info(parent_raw_fd)
        .ok()
        .map(|(d, i, _, _)| (d, i));
    if parent_path_id != Some(expected_parent_id) || parent_fd_id != Some(expected_parent_id) {
        if let Some(ctx) = txn_ctx {
            ctx.record_inner_failure(StageCleanupFailure {
                role,
                disposition: ArchiveCleanupDisposition::OwnershipOrLocationUnconfirmed,
                original_path: path.to_path_buf(),
                final_known_path: None,
                expected_identity: Some(expected_parent_id),
                observed_identity: parent_path_id,
                os_error_code: Some(libc::EXDEV),
                detail: format!("Parent directory identity drifted before creating '{}'", path.display()),
            });
        }
        return Err(io::Error::new(io::ErrorKind::InvalidData, "parent directory identity drifted"));
    }
    Ok(ResolvedCreateParent {
        owned_parent_opt,
        parent_raw_fd,
        expected_parent_id,
        parent_path,
        leaf_c,
    })
}

impl FileAbortGuard {
    fn guard_observer(&self) -> Option<&GuardAbortObserver> {
        self.observer()
            .and_then(|obs| obs.as_any().downcast_ref::<GuardAbortObserver>())
    }

    /// Returns the [`CreateTransactionContext`] associated with this guard, if created inside a transaction.
    #[must_use]
    pub fn txn_ctx(&self) -> Option<&CreateTransactionContext> {
        self.guard_observer().and_then(|obs| obs.ctx.as_ref())
    }

    /// Returns the [`ResidualPathRole`] associated with this guard.
    #[must_use]
    pub fn role(&self) -> ResidualPathRole {
        self.guard_observer()
            .map_or(ResidualPathRole::StagedArtifact, |obs| obs.role)
    }

    /// Exclusively creates a staged file inside an active creation transaction with immediate
    /// RAII `OwnedFd` ownership on every exit path (`F2.1`).
    pub fn create_staged_file_in_txn(
        path: &Path,
        access_mode: CreateFileAccessMode,
        txn_ctx: Option<&CreateTransactionContext>,
        role: ResidualPathRole,
    ) -> io::Result<(File, Self)> {
        let producer_ticket = match txn_ctx {
            Some(ctx) => Some(ctx.try_admit_producer()?),
            None => None,
        };
        let ResolvedCreateParent {
            owned_parent_opt,
            parent_raw_fd,
            expected_parent_id,
            parent_path,
            leaf_c,
        } = resolve_and_verify_create_parent(path, role, txn_ctx)?;
        let created_fd = openat_exclusive_staged_fd(
            path,
            access_mode,
            parent_raw_fd,
            &leaf_c,
            txn_ctx,
        )?;
        let created_id = fstat_created_staged_fd(path, &created_fd, role, txn_ctx)?;
        let guard_parent_fd = acquire_or_dup_parent_fd_or_unlink(
            path,
            &parent_path,
            owned_parent_opt,
            parent_raw_fd,
            expected_parent_id,
            &leaf_c,
            created_id,
            &created_fd,
            role,
            txn_ctx,
        )?;

        let file = File::from(created_fd);
        let observer: Option<Arc<dyn FsAbortObserver>> = Some(Arc::new(GuardAbortObserver {
            ctx: txn_ctx.cloned(),
            role,
            _ticket: producer_ticket,
        }));
        let guard_parent_fd = txn_ctx.map_or_else(
            || guard_parent_fd.clone(),
            |ctx| ctx.retain_guard_parent(&parent_path, guard_parent_fd.clone()),
        );
        let guard = Self::from_exclusively_created_parts(VerifiedGuardParts {
            path: path.to_path_buf(),
            parent_path,
            parent_fd: guard_parent_fd,
            parent_id: expected_parent_id,
            leaf_name: leaf_c,
            file_fd: None,
            file_id: created_id,
            observer,
        });
        Ok((file, guard))
    }

    /// Explicitly removes the monitored file and records any [`StageCleanupFailure`] into its transaction context.
    pub fn abort_checked_detailed(
        &mut self,
        primary_status: TTZipStatus,
    ) -> Result<(), Box<StageCleanupFailure>> {
        let role = self.role();
        let ctx = self.txn_ctx().cloned();
        match self.abort_checked_fs_detailed() {
            Ok(()) => Ok(()),
            Err(fs_err) => {
                let failure = StageCleanupFailure::from_fs_abort_error(role, *fs_err);
                if let Some(c) = ctx {
                    c.record_inner_failure(failure.clone());
                } else {
                    let synth = io::Error::other(failure.detail.clone());
                    super::create_staging::record_inner_stage_cleanup_error_with_status(
                        &failure.original_path,
                        primary_status,
                        &synth,
                    );
                }
                Err(Box::new(failure))
            }
        }
    }

    /// Records diagnostics and unconfirmed release status from `finish_err` and runs `abort_checked_detailed`.
    pub fn settle_failed_file_finish(&mut self, finish_err: &StageFileFinishError) {
        let role = self.role();
        let expected_id = self.expected_file_id().unwrap_or((0, 0));
        let orig_path = self.path().to_path_buf();
        if let Some(ctx) = self.txn_ctx().cloned() {
            if finish_err.has_any_io_finish_error() || !finish_err.is_release_confirmed() {
                ctx.record_finish_diagnostic(StageFileFinishDiagnostic::from_finish_error(
                    role,
                    orig_path.clone(),
                    expected_id,
                    finish_err,
                ));
            }
            if finish_err.release_status == DescriptorReleaseStatus::ReleaseUnconfirmed {
                ctx.record_unconfirmed_descriptor_release(
                    role,
                    orig_path,
                    (expected_id != (0, 0)).then_some(expected_id),
                    finish_err.close_error.as_ref().and_then(io::Error::raw_os_error),
                    finish_err
                        .close_error
                        .as_ref()
                        .map_or_else(|| "descriptor release unconfirmed".to_string(), ToString::to_string),
                );
            }
        }
        let _ = self.abort_checked_detailed(finish_err.primary_status);
    }

    /// Finishes and closes `file` under [`CloseScopePrecondition::for_current_target_unique_regular_file`]
    /// and settles `self` in its associated transaction context.
    pub fn finish_file_and_settle_in_txn(
        mut self,
        file: File,
        prior_write_res: Result<(), (TTZipStatus, String)>,
        userspace_flush_err: Option<io::Error>,
        sync_policy: FileFinishSyncPolicy,
    ) -> Result<ExplicitFileFinishProof, StageFileFinishError> {
        let expected_id = self.expected_file_id().unwrap_or((0, 0));
        let verified_prior = match (prior_write_res, fstat_dev_ino(file.as_raw_fd())) {
            (Err(prior_err), _) => Err(prior_err),
            (Ok(()), Ok(live_id)) if expected_id == (0, 0) || live_id == expected_id => Ok(()),
            (Ok(()), Ok(live_id)) => Err((
                TTZipStatus::ErrCompressionFailed,
                format!("open file descriptor identity {live_id:?} did not match guarded inode {expected_id:?}"),
            )),
            (Ok(()), Err(stat_err)) => Err((
                TTZipStatus::ErrCompressionFailed,
                format!("fstat before finish failed: {stat_err}"),
            )),
        };

        let finish_res = run_finish_and_close_file(
            file,
            expected_id,
            verified_prior,
            userspace_flush_err,
            sync_policy,
            CloseScopePrecondition::for_current_target_unique_regular_file(),
        );
        match finish_res {
            Ok(proof) => {
                self.commit();
                Ok(proof)
            }
            Err(finish_err) => {
                self.settle_failed_file_finish(&finish_err);
                Err(finish_err)
            }
        }
    }

    /// Flushes and closes `BufWriter<File>` and settles `self` in its associated transaction context.
    pub fn finish_buf_writer_and_settle_in_txn(
        self,
        mut writer: BufWriter<File>,
        prior_write_res: Result<(), (TTZipStatus, String)>,
        sync_policy: FileFinishSyncPolicy,
    ) -> Result<ExplicitFileFinishProof, StageFileFinishError> {
        let flush_err = if prior_write_res.is_ok() {
            writer.flush().err()
        } else {
            None
        };
        let (inner_file, _parts) = writer.into_parts();
        self.finish_file_and_settle_in_txn(inner_file, prior_write_res, flush_err, sync_policy)
    }
}

fn openat_exclusive_staged_fd(
    path: &Path,
    access_mode: CreateFileAccessMode,
    parent_raw_fd: RawFd,
    leaf_c: &CString,
    txn_ctx: Option<&CreateTransactionContext>,
) -> io::Result<OwnedFd> {
    let mode_flag = match access_mode {
        CreateFileAccessMode::WriteOnly => libc::O_WRONLY,
        CreateFileAccessMode::ReadWrite => libc::O_RDWR,
    };
    let flags = libc::O_CREAT | libc::O_EXCL | mode_flag | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    let raw_fd = unsafe { libc::openat(parent_raw_fd, leaf_c.as_ptr(), flags, 0o600 as libc::c_uint) };
    if raw_fd < 0 {
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::EEXIST) {
            record_openat_eexist_occupant(txn_ctx, parent_raw_fd, leaf_c, path);
        }
        return Err(err);
    }
    Ok(unsafe { OwnedFd::from_raw_fd(raw_fd) })
}

fn fstat_created_staged_fd(
    path: &Path,
    created_fd: &OwnedFd,
    role: ResidualPathRole,
    txn_ctx: Option<&CreateTransactionContext>,
) -> io::Result<(u64, u64)> {
    fstat_dev_ino(created_fd.as_raw_fd()).inspect_err(|fstat_err| {
        if let Some(ctx) = txn_ctx {
            ctx.record_inner_failure(StageCleanupFailure {
                role,
                disposition: ArchiveCleanupDisposition::OwnershipOrLocationUnconfirmed,
                original_path: path.to_path_buf(),
                final_known_path: None,
                expected_identity: None,
                observed_identity: None,
                os_error_code: fstat_err.raw_os_error(),
                detail: format!("Post-creation fstat failed for '{}': {fstat_err}", path.display()),
            });
        }
    })
}

#[expect(clippy::too_many_arguments, reason = "internal post-openat parent fd duplication and rollback helper")]
fn acquire_or_dup_parent_fd_or_unlink(
    path: &Path,
    parent_path: &Path,
    owned_parent_opt: Option<Arc<OwnedFd>>,
    parent_raw_fd: RawFd,
    expected_parent_id: (u64, u64),
    leaf_c: &CString,
    created_id: (u64, u64),
    _created_fd: &OwnedFd,
    role: ResidualPathRole,
    txn_ctx: Option<&CreateTransactionContext>,
) -> io::Result<Arc<OwnedFd>> {
    if let Some(pfd) = owned_parent_opt {
        return Ok(pfd);
    }
    dup_fd_cloexec(parent_raw_fd).map(Arc::new).inspect_err(|_| {
        if let Err(fs_err) = quarantine_verify_and_unlink(
            path,
            parent_path,
            parent_raw_fd,
            expected_parent_id,
            leaf_c,
            created_id,
        ) {
            if let Some(ctx) = txn_ctx {
                ctx.record_inner_failure(StageCleanupFailure::from_fs_abort_error(role, *fs_err));
            }
        }
    })
}

fn run_sync_with_optional_hook(
    file: &File,
    prior_failed: bool,
    sync_policy: FileFinishSyncPolicy,
    sync_hook: Option<SyncFaultHook>,
) -> (bool, Option<io::Error>) {
    match sync_policy {
        FileFinishSyncPolicy::SyncAll if !prior_failed => {
            let res = match sync_hook {
                Some(cb) => cb(file),
                None => file.sync_all(),
            };
            (true, res.err())
        }
        _ => (false, None),
    }
}

pub(crate) fn run_finish_and_close_file(
    file: File,
    confirmed_identity: (u64, u64),
    prior_write_res: Result<(), (TTZipStatus, String)>,
    userspace_flush_err: Option<io::Error>,
    sync_policy: FileFinishSyncPolicy,
    scope: CloseScopePrecondition,
) -> Result<ExplicitFileFinishProof, StageFileFinishError> {
    let sync_hook = CREATE_TXN_SYNC_FAULT_HOOK.with(|slot| slot.borrow_mut().take());
    let close_hook = CREATE_TXN_CLOSE_FAULT_HOOK.with(|slot| slot.borrow_mut().take());
    if sync_hook.is_none() && close_hook.is_none() {
        return finish_and_close_file_for_proof(
            file,
            confirmed_identity,
            prior_write_res,
            userspace_flush_err,
            sync_policy,
            scope,
        );
    }
    let prior_failed = prior_write_res.is_err() || userspace_flush_err.is_some();
    let (sync_performed, sync_error) =
        run_sync_with_optional_hook(&file, prior_failed, sync_policy, sync_hook);
    let (closed_raw_fd, close_res, release_status) = match close_hook {
        Some(cb) => cb(OwnedFd::from(file), scope),
        None => close_owned_fd_once(OwnedFd::from(file), scope),
    };
    let close_error = close_res.err();
    if !prior_failed
        && sync_error.is_none()
        && close_error.is_none()
        && release_status == DescriptorReleaseStatus::ConfirmedReleased
    {
        return Ok(ExplicitFileFinishProof {
            closed_raw_fd,
            confirmed_identity,
            sync_performed,
            release_status,
        });
    }
    let (primary_status, prior_write_or_encoder_error) = match prior_write_res {
        Ok(()) => (TTZipStatus::ErrCompressionFailed, None),
        Err((status, msg)) => (status, Some(msg)),
    };
    Err(StageFileFinishError {
        primary_status,
        prior_write_or_encoder_error,
        userspace_flush_error: userspace_flush_err,
        sync_error,
        close_error,
        release_status,
    })
}

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
    pub(crate) fn create(parent: &Path) -> Result<Self, super::create_staging::CreateStageCommitError> {
        use super::create_staging::{next_txn_token, CreateStageCommitError};
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

    pub(crate) fn cleanup_checked(&mut self) -> io::Result<()> {
        self.attempted_cleanup = true;
        crate::fs::collision::staging::verify_and_remove_owned_dir(
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
    err: &io::Error,
    stage_dir_preserved: bool,
) {
    let msg = err.to_string();
    let disposition = if msg.contains("outcome=identity_mismatch_restored") {
        ArchiveCleanupDisposition::RestoredDisplacedSentinel
    } else if msg.contains("outcome=identity_mismatch_restore_occupied") {
        ArchiveCleanupDisposition::RestoreDisplacedSentinelFailed
    } else if err.kind() == io::ErrorKind::InvalidData {
        ArchiveCleanupDisposition::RefusedOwnershipMismatch
    } else {
        ArchiveCleanupDisposition::UnlinkOrRmdirIoError
    };
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
                    os_error_code: err.raw_os_error(),
                    detail: format!("Staged file remained in '{}': {err}", guard.txn_dir.display()),
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
        os_error_code: err.raw_os_error(),
        detail: format!(
            "Failed to clean outer transaction stage dir '{}': {err}",
            guard.txn_dir.display()
        ),
    });
}

pub(crate) fn finalize_failed_stage_dir_with_ctx<C>(
    guard: &mut CreateStagingGuard,
    txn_ctx: &CreateTransactionContext,
    err: super::create_staging::CreateStageCommitError,
    cancel_probe: C,
) -> super::create_staging::CreateStageCommitError
where
    C: FnOnce() -> bool,
{
    use super::create_staging::CreateStageCommitError;
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
    mut output: super::create_staging::CreateStagedCommitOutput,
) -> super::create_staging::CreateStagedCommitOutput {
    if let Err(cleanup_err) = guard.cleanup_checked() {
        let txn_dir = guard.txn_dir.clone();
        super::record_post_commit_cleanup_issue(
            txn_dir.clone(),
            "committed_txn_stage_dir",
            &cleanup_err,
        );
        output
            .post_commit_cleanup_issues
            .push(super::PostCommitCleanupIssue {
                path: txn_dir,
                role: "committed_txn_stage_dir",
                message: format!("committed_txn_stage_dir: {cleanup_err}"),
                errno: cleanup_err.raw_os_error().unwrap_or(libc::EIO),
            });
    }
    txn_ctx.mark_settled_clean();
    output
}
