// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Quarantine-and-verify file removal and displaced sentinel restoration for [`super::FileAbortGuard`].

use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::fd::RawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::fstatat_dev_ino_nofollow;

static ABORT_QUARANTINE_SEQ: AtomicU64 = AtomicU64::new(1);

type AfterQuarantineHook = Box<dyn FnOnce(&Path, &Path) + Send + 'static>;

thread_local! {
    static ABORT_GUARD_AFTER_QUARANTINE_HOOK: RefCell<Option<AfterQuarantineHook>> =
        const { RefCell::new(None) };
}

/// Pure filesystem cleanup failure disposition produced by [`super::FileAbortGuard`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FsAbortCleanupDisposition {
    RefusedOwnershipMismatch,
    OwnershipOrLocationUnconfirmed,
    RestoredDisplacedSentinel,
    RestoreDisplacedSentinelFailed,
    UnlinkOrRmdirIoError,
}

/// Structured filesystem cleanup error produced at the exact point of failure in [`super::FileAbortGuard`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FsAbortCleanupError {
    pub disposition: FsAbortCleanupDisposition,
    pub original_path: PathBuf,
    pub final_known_path: Option<PathBuf>,
    pub expected_identity: Option<(u64, u64)>,
    pub observed_identity: Option<(u64, u64)>,
    pub os_error_code: Option<i32>,
    pub io_kind: std::io::ErrorKind,
    pub detail: String,
}

impl FsAbortCleanupError {
    #[inline]
    pub fn into_io_error(self) -> std::io::Error {
        std::io::Error::new(self.io_kind, self.detail)
    }
}

/// Observer notified when a [`super::FileAbortGuard`] encounters a cleanup error during drop.
pub(crate) trait FsAbortObserver:
    Send + Sync + std::fmt::Debug + std::panic::RefUnwindSafe + std::panic::UnwindSafe
{
    fn on_abort_cleanup_error(&self, err: FsAbortCleanupError);
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Registers a one-shot hook invoked after `FileAbortGuard` renames the leaf to `.ttzip_abort_q_*` and before `fstatat` / `unlinkat`.
pub fn set_abort_guard_after_quarantine_hook<F>(hook: F)
where
    F: FnOnce(&Path, &Path) + Send + 'static,
{
    ABORT_GUARD_AFTER_QUARANTINE_HOOK.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(hook));
    });
}

fn run_abort_guard_after_quarantine_hook(orig_path: &Path, quarantine_path: &Path) {
    let hook = ABORT_GUARD_AFTER_QUARANTINE_HOOK.with(|slot| slot.borrow_mut().take());
    if let Some(cb) = hook {
        cb(orig_path, quarantine_path);
    }
}

fn format_quarantined_unlink_err(
    orig_path: &Path,
    q_path: &Path,
    expected_parent_id: (u64, u64),
    expected_id: (u64, u64),
    unlink_err: std::io::Error,
) -> FsAbortCleanupError {
    let errno = unlink_err.raw_os_error().unwrap_or(libc::EIO);
    let msg = format!(
        "quarantined staged file unlink failed (outcome=quarantined_unlink_failed, orig_path='{}', quarantine_path='{}', parent_id={:?}, os_err={errno}: {unlink_err}); residual owned file remains at quarantine path",
        orig_path.display(),
        q_path.display(),
        expected_parent_id,
    );
    FsAbortCleanupError {
        disposition: FsAbortCleanupDisposition::UnlinkOrRmdirIoError,
        original_path: orig_path.to_path_buf(),
        final_known_path: Some(q_path.to_path_buf()),
        expected_identity: Some(expected_id),
        observed_identity: Some(expected_id),
        os_error_code: Some(errno),
        io_kind: unlink_err.kind(),
        detail: msg,
    }
}

fn build_sentinel_restore_failure(
    orig_path: &Path,
    q_path: &Path,
    expected_parent_id: (u64, u64),
    expected_id: (u64, u64),
    observed_id: Option<(u64, u64)>,
    restore_err: std::io::Error,
) -> FsAbortCleanupError {
    let errno = restore_err.raw_os_error().unwrap_or(libc::EIO);
    if errno == libc::EEXIST {
        FsAbortCleanupError {
            disposition: FsAbortCleanupDisposition::RestoreDisplacedSentinelFailed,
            original_path: orig_path.to_path_buf(),
            final_known_path: Some(q_path.to_path_buf()),
            expected_identity: Some(expected_id),
            observed_identity: observed_id,
            os_error_code: Some(errno),
            io_kind: std::io::ErrorKind::InvalidData,
            detail: format!(
                "staged file replaced between identity check and unlink and original leaf was occupied during restore (outcome=identity_mismatch_restore_occupied, sentinel_preserved_at_quarantine_path='{}', occupant_preserved_at_original_path='{}', parent_id={:?}, os_err={errno}); sentinel NOT restored to original name",
                q_path.display(),
                orig_path.display(),
                expected_parent_id,
            ),
        }
    } else {
        FsAbortCleanupError {
            disposition: FsAbortCleanupDisposition::OwnershipOrLocationUnconfirmed,
            original_path: orig_path.to_path_buf(),
            final_known_path: None,
            expected_identity: Some(expected_id),
            observed_identity: observed_id,
            os_error_code: Some(errno),
            io_kind: std::io::ErrorKind::InvalidData,
            detail: format!(
                "staged file replaced between identity check and unlink and restore rename failed (outcome=identity_mismatch_location_unconfirmed, orig_path='{}', quarantine_path='{}', parent_id={:?}, os_err={errno}: {restore_err})",
                orig_path.display(),
                q_path.display(),
                expected_parent_id,
            ),
        }
    }
}

#[expect(clippy::too_many_arguments, reason = "internal sentinel restore helper with parent/leaf/quarantine descriptors")]
fn restore_quarantined_file_sentinel(
    orig_path: &Path,
    q_path: &Path,
    parent_fd: RawFd,
    expected_parent_id: (u64, u64),
    leaf: &CStr,
    q_name: &CStr,
    expected_id: (u64, u64),
    observed_id: Option<(u64, u64)>,
) -> FsAbortCleanupError {
    let flags: libc::c_uint = (crate::fs::collision::fs_ops::RENAME_EXCL
        | crate::fs::collision::fs_ops::RENAME_NOFOLLOW_ANY) as libc::c_uint;
    let rc = unsafe {
        libc::renameatx_np(parent_fd, q_name.as_ptr(), parent_fd, leaf.as_ptr(), flags)
    };
    if rc == 0 {
        let verified = match (observed_id, fstatat_dev_ino_nofollow(parent_fd, leaf)) {
            (Some(obs), Ok((r_dev, r_ino, _))) => (r_dev, r_ino) == obs,
            _ => false,
        };
        if verified {
            return FsAbortCleanupError {
                disposition: FsAbortCleanupDisposition::RestoredDisplacedSentinel,
                original_path: orig_path.to_path_buf(),
                final_known_path: Some(orig_path.to_path_buf()),
                expected_identity: Some(expected_id),
                observed_identity: observed_id,
                os_error_code: None,
                io_kind: std::io::ErrorKind::InvalidData,
                detail: format!(
                    "staged file replaced between identity check and unlink (outcome=identity_mismatch_restored, restored_path='{}', quarantine_name='{}', parent_id={:?}); restored sentinel",
                    orig_path.display(),
                    q_name.to_string_lossy(),
                    expected_parent_id,
                ),
            };
        }
        return FsAbortCleanupError {
            disposition: FsAbortCleanupDisposition::OwnershipOrLocationUnconfirmed,
            original_path: orig_path.to_path_buf(),
            final_known_path: None,
            expected_identity: Some(expected_id),
            observed_identity: observed_id,
            os_error_code: None,
            io_kind: std::io::ErrorKind::InvalidData,
            detail: format!(
                "staged file replaced between identity check and unlink, but post-restore verification did not match (outcome=identity_mismatch_location_unconfirmed, orig_path='{}', quarantine_path='{}', parent_id={:?})",
                orig_path.display(),
                q_path.display(),
                expected_parent_id,
            ),
        };
    }
    build_sentinel_restore_failure(
        orig_path,
        q_path,
        expected_parent_id,
        expected_id,
        observed_id,
        std::io::Error::last_os_error(),
    )
}

pub(crate) fn quarantine_verify_and_unlink(
    orig_path: &Path,
    parent_path: &Path,
    parent_fd: RawFd,
    expected_parent_id: (u64, u64),
    leaf: &CStr,
    expected_id: (u64, u64),
) -> Result<(), Box<FsAbortCleanupError>> {
    let pid = std::process::id();
    let seq = ABORT_QUARANTINE_SEQ.fetch_add(1, Ordering::Relaxed);
    let q_str = format!(".ttzip_abort_q_{pid}_{seq}");
    let q_name = CString::new(q_str.as_bytes()).map_err(|_| {
        Box::new(FsAbortCleanupError {
            disposition: FsAbortCleanupDisposition::UnlinkOrRmdirIoError,
            original_path: orig_path.to_path_buf(),
            final_known_path: Some(orig_path.to_path_buf()),
            expected_identity: Some(expected_id),
            observed_identity: None,
            os_error_code: None,
            io_kind: std::io::ErrorKind::InvalidInput,
            detail: "invalid quarantine name".to_string(),
        })
    })?;
    let q_path = parent_path.join(&q_str);
    let flags: libc::c_uint = (crate::fs::collision::fs_ops::RENAME_EXCL
        | crate::fs::collision::fs_ops::RENAME_NOFOLLOW_ANY) as libc::c_uint;
    if unsafe {
        libc::renameatx_np(parent_fd, leaf.as_ptr(), parent_fd, q_name.as_ptr(), flags)
    } != 0
    {
        let err = std::io::Error::last_os_error();
        let still_at_orig = fstatat_dev_ino_nofollow(parent_fd, leaf).is_ok();
        return Err(Box::new(FsAbortCleanupError {
            disposition: FsAbortCleanupDisposition::UnlinkOrRmdirIoError,
            original_path: orig_path.to_path_buf(),
            final_known_path: still_at_orig.then(|| orig_path.to_path_buf()),
            expected_identity: Some(expected_id),
            observed_identity: Some(expected_id),
            os_error_code: err.raw_os_error(),
            io_kind: err.kind(),
            detail: err.to_string(),
        }));
    }
    run_abort_guard_after_quarantine_hook(orig_path, &q_path);
    match fstatat_dev_ino_nofollow(parent_fd, &q_name) {
        Ok((dev, ino, mode))
            if (mode & libc::S_IFMT) == libc::S_IFREG && (dev, ino) == expected_id =>
        {
            if unsafe { libc::unlinkat(parent_fd, q_name.as_ptr(), 0) } != 0 {
                let unlink_err = std::io::Error::last_os_error();
                return Err(Box::new(format_quarantined_unlink_err(
                    orig_path,
                    &q_path,
                    expected_parent_id,
                    expected_id,
                    unlink_err,
                )));
            }
            Ok(())
        }
        other => {
            let observed = other.ok().map(|(d, i, _)| (d, i));
            Err(Box::new(restore_quarantined_file_sentinel(
                orig_path,
                &q_path,
                parent_fd,
                expected_parent_id,
                leaf,
                &q_name,
                expected_id,
                observed,
            )))
        }
    }
}
