// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Explicit file descriptor finish and single-close primitives with release confirmation
//! classification (`B1` minimal shared filesystem slice).
//!
//! # Scope and Platform Evidence Boundaries
//! - **Single Close & Ownership Consumption**: [`close_owned_fd_once`] and [`close_owned_file_once`]
//!   consume [`OwnedFd`] / [`File`] ownership via [`IntoRawFd::into_raw_fd`] at the immediate
//!   `libc::close(raw_fd)` syscall boundary. `Drop` never runs a second `close(2)`, and bare
//!   `RawFd` values are never retried after an error or probed with `fcntl(F_GETFD)` in production.
//! - **Pre-Close Unwind Safety**: Optional synchronization (`File::sync_all`) executes while
//!   [`File`] still holds RAII ownership on the stack. If an unwind occurs before the close
//!   syscall boundary, standard RAII drops [`File`] / [`OwnedFd`] and closes the descriptor once.
//! - **Close Error vs. Release Confirmation Separation**:
//!   - `Ok(())` from `close(2)` classifies as [`DescriptorReleaseStatus::ConfirmedReleased`].
//!   - On `#[cfg(target_os = "macos")]` **and** when the caller explicitly passes
//!     [`CloseScopePrecondition::MacOsUniqueNonGuardedOrdinaryVnode`] (supported macOS ordinary,
//!     non-guarded, uniquely-owned regular-file vnode descriptors where XNU `fp_close_and_unlock`
//!     clears the descriptor slot via `fdrelse` before `vn_close`), `close(2) -> EIO` classifies as
//!     [`DescriptorReleaseStatus::ConfirmedReleased`] **while remaining a `close_error`
//!     (`Err(EIO)`) that never produces an [`ExplicitFileFinishProof`] and forbids target
//!     publication**.
//!   - On non-macOS targets (`#[cfg(not(target_os = "macos"))]`), `close(2) -> EIO` is always
//!     [`DescriptorReleaseStatus::ReleaseUnconfirmed`] even if passed `MacOsUniqueNonGuardedOrdinaryVnode`.
//!   - `EINTR`, `EBADF`, any other/unknown `errno`, and `EIO` under
//!     [`CloseScopePrecondition::ConservativeUnverifiedScope`] classify as
//!     [`DescriptorReleaseStatus::ReleaseUnconfirmed`]. `EBADF` is never `ConfirmedReleased`.
//! - **Who Proves the Precondition & Identity**: Rust ownership of [`File`] or [`OwnedFd`] does
//!   **not** automatically prove that no duplicate (`dup`/`dup2`/`fork`) shares the open file
//!   description or that the descriptor is non-guarded; the caller must establish provenance before
//!   passing [`CloseScopePrecondition::MacOsUniqueNonGuardedOrdinaryVnode`] (default is
//!   [`CloseScopePrecondition::ConservativeUnverifiedScope`]). On [`ExplicitFileFinishProof`],
//!   `confirmed_identity` is caller-supplied (not verified via `fstat` here), and `closed_raw_fd`
//!   is a historical diagnostic integer that must never be used as a live descriptor.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::os::fd::{IntoRawFd, OwnedFd, RawFd};

use crate::types::TTZipStatus;

/// Minimal descriptor slot release confirmation state after a single explicit `close(2)` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorReleaseStatus {
    /// `close(2)` returned `0`, or failed with `EIO` on `target_os = "macos"` under the
    /// caller-established ordinary, non-guarded, uniquely-owned regular-file vnode precondition.
    ConfirmedReleased,
    /// `close(2)` failed with `EINTR`, `EBADF`, an unknown error, or `EIO` outside the
    /// verified macOS platform/descriptor precondition, so descriptor slot release is not confirmed.
    ReleaseUnconfirmed,
}

impl DescriptorReleaseStatus {
    /// Returns `true` only when the descriptor table slot is confirmed released.
    #[inline]
    #[must_use]
    pub const fn is_confirmed_released(self) -> bool {
        matches!(self, Self::ConfirmedReleased)
    }
}

/// Platform and descriptor precondition scope governing whether `close(2) -> EIO` can be
/// classified as [`DescriptorReleaseStatus::ConfirmedReleased`].
///
/// Rust ownership of [`File`] or [`OwnedFd`] alone does **not** prove this precondition;
/// the caller must establish how the descriptor was opened and held before selecting
/// [`Self::MacOsUniqueNonGuardedOrdinaryVnode`]. Defaults to [`Self::ConservativeUnverifiedScope`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CloseScopePrecondition {
    /// Caller-established precondition that the descriptor is an ordinary, non-guarded,
    /// uniquely-owned regular-file vnode on macOS (`target_os = "macos"`).
    MacOsUniqueNonGuardedOrdinaryVnode,
    /// Default conservative scope for non-macOS targets, guarded or duplicated descriptors,
    /// or any descriptor whose provenance has not been verified by the caller.
    #[default]
    ConservativeUnverifiedScope,
}

impl CloseScopePrecondition {
    /// Returns the precondition applicable when the caller has verified that the descriptor is
    /// an exclusively created, non-guarded, uniquely-owned regular staging/temp file on the
    /// current compilation target.
    #[inline]
    #[must_use]
    pub const fn for_current_target_unique_regular_file() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOsUniqueNonGuardedOrdinaryVnode
        } else {
            Self::ConservativeUnverifiedScope
        }
    }
}

#[inline]
#[must_use]
const fn classify_close_release_status_with_target_os(
    close_ok: bool,
    raw_os_error: Option<i32>,
    scope: CloseScopePrecondition,
    is_macos_target: bool,
) -> DescriptorReleaseStatus {
    if close_ok {
        return DescriptorReleaseStatus::ConfirmedReleased;
    }
    if is_macos_target
        && matches!(scope, CloseScopePrecondition::MacOsUniqueNonGuardedOrdinaryVnode)
        && matches!(raw_os_error, Some(libc::EIO))
    {
        return DescriptorReleaseStatus::ConfirmedReleased;
    }
    DescriptorReleaseStatus::ReleaseUnconfirmed
}

/// Classifies [`DescriptorReleaseStatus`] from a single `close(2)` result under an explicit
/// [`CloseScopePrecondition`] on the current compilation target.
///
/// Requires **both** `cfg!(target_os = "macos")` **and**
/// `scope == CloseScopePrecondition::MacOsUniqueNonGuardedOrdinaryVnode` before classifying
/// `Err(EIO)` as [`DescriptorReleaseStatus::ConfirmedReleased`]. Even then, `close_res` itself
/// remains `Err(EIO)` and never authorizes target publication.
#[inline]
#[must_use]
pub fn classify_close_release_status_for_scope(
    close_res: &io::Result<()>,
    scope: CloseScopePrecondition,
) -> DescriptorReleaseStatus {
    let is_macos = cfg!(target_os = "macos");
    match close_res {
        Ok(()) => classify_close_release_status_with_target_os(true, None, scope, is_macos),
        Err(err) => {
            classify_close_release_status_with_target_os(false, err.raw_os_error(), scope, is_macos)
        }
    }
}

/// Synchronization policy applied while [`File`] still owns the descriptor prior to `close(2)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFinishSyncPolicy {
    /// Do not invoke `File::sync_all()` before closing.
    None,
    /// Invoke `File::sync_all()` while `File` still holds RAII ownership (when no prior write
    /// or userspace flush error occurred).
    SyncAll,
}

#[inline]
fn invoke_close_owned_fd_raw(owned_fd: OwnedFd) -> (RawFd, io::Result<()>) {
    #[cfg(test)]
    if let Some((errno, count)) = TEST_CLOSE_INJECTION.with(|slot| *slot.borrow()) {
        TEST_CLOSE_INJECTION.with(|slot| *slot.borrow_mut() = Some((errno, count + 1)));
        let raw_fd: RawFd = owned_fd.into_raw_fd();
        let rc = unsafe { libc::close(raw_fd) };
        assert_eq!(rc, 0, "underlying test descriptor close must succeed");
        return (raw_fd, Err(io::Error::from_raw_os_error(errno)));
    }
    let raw_fd: RawFd = owned_fd.into_raw_fd();
    let rc = unsafe { libc::close(raw_fd) };
    let close_res = if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    };
    (raw_fd, close_res)
}

/// Consumes an [`OwnedFd`] at the immediate syscall boundary, invokes `libc::close(raw_fd)`
/// exactly once without retry, and classifies [`DescriptorReleaseStatus`] under the explicit
/// caller-supplied [`CloseScopePrecondition`].
///
/// The returned [`RawFd`] is the historical descriptor number that was closed and must never
/// be passed to any subsequent syscall.
#[inline]
pub fn close_owned_fd_once(
    owned_fd: OwnedFd,
    scope: CloseScopePrecondition,
) -> (RawFd, io::Result<()>, DescriptorReleaseStatus) {
    let (raw_fd, close_res) = invoke_close_owned_fd_raw(owned_fd);
    let release_status = classify_close_release_status_for_scope(&close_res, scope);
    (raw_fd, close_res, release_status)
}

/// Consumes a [`File`] and closes its underlying descriptor once via [`close_owned_fd_once`]
/// under the explicit caller-supplied [`CloseScopePrecondition`].
#[inline]
pub fn close_owned_file_once(
    file: File,
    scope: CloseScopePrecondition,
) -> (RawFd, io::Result<()>, DescriptorReleaseStatus) {
    close_owned_fd_once(OwnedFd::from(file), scope)
}

/// Proof that a staged or temporary regular file's descriptor owner completed all prior
/// write, userspace flush, optional sync, and single `close(2)` steps with zero errors and
/// [`DescriptorReleaseStatus::ConfirmedReleased`].
///
/// `closed_raw_fd` is historical diagnostic metadata (never a live descriptor), and
/// `confirmed_identity` is caller-supplied (not verified via `fstat` by this module).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplicitFileFinishProof {
    pub closed_raw_fd: RawFd,
    pub confirmed_identity: (u64, u64),
    pub sync_performed: bool,
    pub release_status: DescriptorReleaseStatus,
}

/// Structured error preserving primary write/encoder failure, `BufWriter::flush()` failure,
/// optional `File::sync_all()` failure, and explicit `close(2)` failure alongside
/// [`DescriptorReleaseStatus`].
#[derive(Debug)]
pub struct StageFileFinishError {
    pub primary_status: TTZipStatus,
    pub prior_write_or_encoder_error: Option<String>,
    pub userspace_flush_error: Option<io::Error>,
    pub sync_error: Option<io::Error>,
    pub close_error: Option<io::Error>,
    pub release_status: DescriptorReleaseStatus,
}

impl StageFileFinishError {
    /// Returns the primary [`TTZipStatus`] associated with the failure.
    #[inline]
    #[must_use]
    pub const fn primary_status(&self) -> TTZipStatus {
        self.primary_status
    }

    /// Returns `true` when the descriptor slot release was confirmed despite the finish error.
    #[inline]
    #[must_use]
    pub const fn is_release_confirmed(&self) -> bool {
        self.release_status.is_confirmed_released()
    }

    /// Returns `true` if any of `userspace_flush_error`, `sync_error`, or `close_error` occurred.
    #[inline]
    #[must_use]
    pub const fn has_any_io_finish_error(&self) -> bool {
        self.userspace_flush_error.is_some()
            || self.sync_error.is_some()
            || self.close_error.is_some()
    }
}

fn finish_and_close_file_with<S, C>(
    file: File,
    confirmed_identity: (u64, u64),
    prior_write_res: Result<(), (TTZipStatus, String)>,
    userspace_flush_err: Option<io::Error>,
    sync_policy: FileFinishSyncPolicy,
    scope: CloseScopePrecondition,
    sync_fn: S,
    close_fn: C,
) -> Result<ExplicitFileFinishProof, StageFileFinishError>
where
    S: FnOnce(&File) -> io::Result<()>,
    C: FnOnce(OwnedFd, CloseScopePrecondition) -> (RawFd, io::Result<()>, DescriptorReleaseStatus),
{
    let prior_failed = prior_write_res.is_err() || userspace_flush_err.is_some();
    // Execute optional sync while `file: File` STILL owns the descriptor on the stack.
    let (sync_performed, sync_error) = match sync_policy {
        FileFinishSyncPolicy::SyncAll if !prior_failed => match sync_fn(&file) {
            Ok(()) => (true, None),
            Err(err) => (true, Some(err)),
        },
        _ => (false, None),
    };

    // Consume `File` ownership only at the close boundary and classify with explicit `scope`.
    let (closed_raw_fd, close_res, release_status) = close_fn(OwnedFd::from(file), scope);
    let close_error = close_res.err();

    if prior_write_res.is_ok()
        && userspace_flush_err.is_none()
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

fn flush_and_close_buf_writer_with<S, C>(
    mut writer: BufWriter<File>,
    confirmed_identity: (u64, u64),
    prior_write_res: Result<(), (TTZipStatus, String)>,
    sync_policy: FileFinishSyncPolicy,
    scope: CloseScopePrecondition,
    sync_fn: S,
    close_fn: C,
) -> Result<ExplicitFileFinishProof, StageFileFinishError>
where
    S: FnOnce(&File) -> io::Result<()>,
    C: FnOnce(OwnedFd, CloseScopePrecondition) -> (RawFd, io::Result<()>, DescriptorReleaseStatus),
{
    let flush_err = if prior_write_res.is_ok() {
        writer.flush().err()
    } else {
        None
    };
    let (inner_file, _unflushed_parts) = writer.into_parts();
    finish_and_close_file_with(
        inner_file,
        confirmed_identity,
        prior_write_res,
        flush_err,
        sync_policy,
        scope,
        sync_fn,
        close_fn,
    )
}

/// Finishes and closes a [`File`] under an explicit [`CloseScopePrecondition`], producing an
/// [`ExplicitFileFinishProof`] only when prior write, userspace flush, optional `sync_all`,
/// and single `close(2)` all succeed with [`DescriptorReleaseStatus::ConfirmedReleased`].
pub fn finish_and_close_file_for_proof(
    file: File,
    confirmed_identity: (u64, u64),
    prior_write_res: Result<(), (TTZipStatus, String)>,
    userspace_flush_err: Option<io::Error>,
    sync_policy: FileFinishSyncPolicy,
    scope: CloseScopePrecondition,
) -> Result<ExplicitFileFinishProof, StageFileFinishError> {
    finish_and_close_file_with(
        file,
        confirmed_identity,
        prior_write_res,
        userspace_flush_err,
        sync_policy,
        scope,
        File::sync_all,
        close_owned_fd_once,
    )
}

/// Explicit file finish and single-close primitive under an explicit [`CloseScopePrecondition`],
/// returning the historical closed [`RawFd`] diagnostic value when all finish stages succeed.
pub fn explicit_finish_and_close_file_production(
    file: File,
    prior_write_res: Result<(), (TTZipStatus, String)>,
    userspace_flush_err: Option<io::Error>,
    sync_policy: FileFinishSyncPolicy,
    scope: CloseScopePrecondition,
) -> Result<RawFd, StageFileFinishError> {
    finish_and_close_file_for_proof(
        file,
        (0, 0),
        prior_write_res,
        userspace_flush_err,
        sync_policy,
        scope,
    )
    .map(|proof| proof.closed_raw_fd)
}

/// Flushes a [`BufWriter<File>`], disassembles it via `into_parts()` to prevent `Drop` from
/// retrying a write, and finishes/closes the inner [`File`] under an explicit
/// [`CloseScopePrecondition`] for an [`ExplicitFileFinishProof`].
pub fn flush_and_close_buf_writer_for_proof(
    writer: BufWriter<File>,
    confirmed_identity: (u64, u64),
    prior_write_res: Result<(), (TTZipStatus, String)>,
    sync_policy: FileFinishSyncPolicy,
    scope: CloseScopePrecondition,
) -> Result<ExplicitFileFinishProof, StageFileFinishError> {
    flush_and_close_buf_writer_with(
        writer,
        confirmed_identity,
        prior_write_res,
        sync_policy,
        scope,
        File::sync_all,
        close_owned_fd_once,
    )
}

/// Flushes a [`BufWriter<File>`], disassembles it via `into_parts()` so `BufWriter::drop`
/// cannot retry a failed write, and closes the inner [`File`] once under an explicit
/// [`CloseScopePrecondition`] while preserving both `userspace_flush_error` and `close_error`.
pub fn flush_and_close_buf_writer_once(
    writer: BufWriter<File>,
    prior_write_res: Result<(), (TTZipStatus, String)>,
    sync_policy: FileFinishSyncPolicy,
    scope: CloseScopePrecondition,
) -> Result<RawFd, StageFileFinishError> {
    flush_and_close_buf_writer_for_proof(writer, (0, 0), prior_write_res, sync_policy, scope)
        .map(|proof| proof.closed_raw_fd)
}

#[cfg(test)]
thread_local! {
    static TEST_CLOSE_INJECTION: std::cell::RefCell<Option<(i32, usize)>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::os::fd::AsRawFd;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::path::Path;

    struct ScopedCloseErrnoInjection;

    impl ScopedCloseErrnoInjection {
        fn install(errno: i32) -> Self {
            TEST_CLOSE_INJECTION.with(|slot| *slot.borrow_mut() = Some((errno, 0)));
            Self
        }

        fn calls() -> usize {
            TEST_CLOSE_INJECTION.with(|slot| slot.borrow().map_or(0, |(_, c)| c))
        }
    }

    impl Drop for ScopedCloseErrnoInjection {
        fn drop(&mut self) {
            TEST_CLOSE_INJECTION.with(|slot| *slot.borrow_mut() = None);
        }
    }

    /// Acquires a non-blocking exclusive `flock(2)` on `target` and opens a second independent
    /// observer [`File`] on the same path, bound to `target`'s kernel open file description.
    fn attach_open_description_observer(path: &Path, target: &File) -> File {
        let rc = unsafe { libc::flock(target.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        assert_eq!(rc, 0, "exclusive flock on target file description must succeed");
        File::open(path).expect("open observer file description")
    }

    /// Returns `true` while the target's kernel open file description still holds its exclusive
    /// `flock(2)`, and `false` once that open file description has been closed by the kernel.
    fn is_target_description_open(observer: &File) -> bool {
        let rc = unsafe { libc::flock(observer.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if rc == 0 {
            let _ = unsafe { libc::flock(observer.as_raw_fd(), libc::LOCK_UN) };
            false
        } else {
            true
        }
    }

    #[test]
    fn real_regular_file_close_and_proof_succeeds_once() {
        assert_eq!(
            CloseScopePrecondition::default(),
            CloseScopePrecondition::ConservativeUnverifiedScope
        );
        let dir = tempfile::tempdir().expect("tempdir");
        let path_a = dir.path().join("real_close_none.bin");
        let mut file_a = File::create(&path_a).expect("create file_a");
        file_a.write_all(b"ttzip-b1").expect("write file_a");
        let obs_a = attach_open_description_observer(&path_a, &file_a);
        let raw_a = file_a.as_raw_fd();
        assert!(is_target_description_open(&obs_a));

        let proof_a = finish_and_close_file_for_proof(
            file_a,
            (11, 22),
            Ok(()),
            None,
            FileFinishSyncPolicy::None,
            CloseScopePrecondition::ConservativeUnverifiedScope,
        )
        .expect("normal finish must succeed");
        assert_eq!(proof_a.closed_raw_fd, raw_a);
        assert_eq!(proof_a.confirmed_identity, (11, 22));
        assert!(!proof_a.sync_performed);
        assert_eq!(proof_a.release_status, DescriptorReleaseStatus::ConfirmedReleased);
        assert!(!is_target_description_open(&obs_a));

        let path_b = dir.path().join("real_close_sync.bin");
        let mut file_b = File::create(&path_b).expect("create file_b");
        file_b.write_all(b"ttzip-b1-sync").expect("write file_b");
        let obs_b = attach_open_description_observer(&path_b, &file_b);
        let raw_b = file_b.as_raw_fd();
        assert!(is_target_description_open(&obs_b));
        let closed_b = explicit_finish_and_close_file_production(
            file_b,
            Ok(()),
            None,
            FileFinishSyncPolicy::SyncAll,
            CloseScopePrecondition::for_current_target_unique_regular_file(),
        )
        .expect("sync_all + close must succeed");
        assert_eq!(closed_b, raw_b);
        assert!(!is_target_description_open(&obs_b));
    }

    #[test]
    fn classify_macos_applicable_eio_is_confirmed_released_but_still_error_without_proof() {
        let eio_res: io::Result<()> = Err(io::Error::from_raw_os_error(libc::EIO));
        #[cfg(target_os = "macos")]
        let expected = DescriptorReleaseStatus::ConfirmedReleased;
        #[cfg(not(target_os = "macos"))]
        let expected = DescriptorReleaseStatus::ReleaseUnconfirmed;
        assert_eq!(
            classify_close_release_status_for_scope(
                &eio_res,
                CloseScopePrecondition::MacOsUniqueNonGuardedOrdinaryVnode,
            ),
            expected
        );

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("macos_eio.bin");
        let file = File::create(&path).expect("create");
        let obs = attach_open_description_observer(&path, &file);
        let _inj = ScopedCloseErrnoInjection::install(libc::EIO);
        let res = finish_and_close_file_for_proof(
            file,
            (1, 2),
            Ok(()),
            None,
            FileFinishSyncPolicy::None,
            CloseScopePrecondition::MacOsUniqueNonGuardedOrdinaryVnode,
        );
        assert_eq!(ScopedCloseErrnoInjection::calls(), 1);
        assert!(!is_target_description_open(&obs));
        let err = res.expect_err("close EIO must still be an error and never yield proof");
        assert_eq!(err.release_status, expected);
        assert_eq!(err.close_error.as_ref().and_then(io::Error::raw_os_error), Some(libc::EIO));
    }

    #[test]
    fn classify_eintr_ebadf_and_unknown_errors_are_release_unconfirmed() {
        for errno in [libc::EINTR, libc::EBADF, libc::ENOSPC, libc::EACCES, 9999] {
            let res: io::Result<()> = Err(io::Error::from_raw_os_error(errno));
            for scope in [
                CloseScopePrecondition::MacOsUniqueNonGuardedOrdinaryVnode,
                CloseScopePrecondition::ConservativeUnverifiedScope,
            ] {
                assert_eq!(
                    classify_close_release_status_for_scope(&res, scope),
                    DescriptorReleaseStatus::ReleaseUnconfirmed,
                    "errno {errno} under {scope:?} must be ReleaseUnconfirmed"
                );
            }
        }
        let custom_err: io::Result<()> = Err(io::Error::other("non-os close failure"));
        assert_eq!(
            classify_close_release_status_for_scope(
                &custom_err,
                CloseScopePrecondition::MacOsUniqueNonGuardedOrdinaryVnode,
            ),
            DescriptorReleaseStatus::ReleaseUnconfirmed
        );

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("eintr_unconfirmed.bin");
        let file = File::create(&path).expect("create");
        let obs = attach_open_description_observer(&path, &file);
        let parked: RefCell<Vec<OwnedFd>> = RefCell::new(Vec::new());
        let mut close_calls = 0usize;

        let err = finish_and_close_file_with(
            file,
            (3, 4),
            Ok(()),
            None,
            FileFinishSyncPolicy::None,
            CloseScopePrecondition::MacOsUniqueNonGuardedOrdinaryVnode,
            File::sync_all,
            |owned_fd, scope| {
                close_calls += 1;
                let fd = owned_fd.as_raw_fd();
                parked.borrow_mut().push(owned_fd);
                let sim_err: io::Result<()> = Err(io::Error::from_raw_os_error(libc::EINTR));
                let status = classify_close_release_status_for_scope(&sim_err, scope);
                (fd, sim_err, status)
            },
        )
        .expect_err("EINTR must fail");

        assert_eq!(close_calls, 1);
        assert!(!err.is_release_confirmed());
        assert_eq!(err.release_status, DescriptorReleaseStatus::ReleaseUnconfirmed);
        assert_eq!(err.close_error.as_ref().and_then(io::Error::raw_os_error), Some(libc::EINTR));
        assert!(is_target_description_open(&obs));
        parked.borrow_mut().clear();
        assert!(!is_target_description_open(&obs));
    }

    #[test]
    fn classify_out_of_scope_eio_conservatively_returns_release_unconfirmed() {
        let eio_res: io::Result<()> = Err(io::Error::from_raw_os_error(libc::EIO));
        assert_eq!(
            classify_close_release_status_for_scope(
                &eio_res,
                CloseScopePrecondition::ConservativeUnverifiedScope,
            ),
            DescriptorReleaseStatus::ReleaseUnconfirmed
        );
        // Non-macOS targets must reject the macOS EIO exception even when passed the macOS enum:
        assert_eq!(
            classify_close_release_status_with_target_os(
                false,
                Some(libc::EIO),
                CloseScopePrecondition::MacOsUniqueNonGuardedOrdinaryVnode,
                false,
            ),
            DescriptorReleaseStatus::ReleaseUnconfirmed
        );

        let dir = tempfile::tempdir().expect("tempdir");
        let _inj = ScopedCloseErrnoInjection::install(libc::EIO);

        // 1. Public `explicit_finish_and_close_file_production` with ConservativeUnverifiedScope:
        let file_a = File::create(dir.path().join("unverified_file_eio.bin")).expect("create a");
        let err_a = explicit_finish_and_close_file_production(
            file_a,
            Ok(()),
            None,
            FileFinishSyncPolicy::None,
            CloseScopePrecondition::ConservativeUnverifiedScope,
        )
        .expect_err("conservative file finish EIO must fail");
        assert_eq!(err_a.release_status, DescriptorReleaseStatus::ReleaseUnconfirmed);
        assert!(!err_a.is_release_confirmed());
        assert_eq!(err_a.close_error.as_ref().and_then(io::Error::raw_os_error), Some(libc::EIO));

        // 2. Public `flush_and_close_buf_writer_once` with ConservativeUnverifiedScope:
        let file_b = File::create(dir.path().join("unverified_bw_eio.bin")).expect("create b");
        let mut bw_b = BufWriter::new(file_b);
        bw_b.write_all(b"conservative-scope").expect("write bw_b");
        let err_b = flush_and_close_buf_writer_once(
            bw_b,
            Ok(()),
            FileFinishSyncPolicy::None,
            CloseScopePrecondition::ConservativeUnverifiedScope,
        )
        .expect_err("conservative BufWriter finish EIO must fail");
        assert_eq!(err_b.release_status, DescriptorReleaseStatus::ReleaseUnconfirmed);
        assert!(!err_b.is_release_confirmed());
        assert_eq!(err_b.close_error.as_ref().and_then(io::Error::raw_os_error), Some(libc::EIO));
        assert_eq!(ScopedCloseErrnoInjection::calls(), 2);
    }

    #[test]
    fn buf_writer_flush_and_close_dual_errors_are_both_preserved_without_drop_retry() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("readonly_bufwriter.bin");
        File::create(&path).expect("create target");

        // Open read-only so `BufWriter::flush()` fails with a real kernel write error (`EBADF`),
        // and inject a simulated `close(2) -> EINTR` error on the single close call.
        let ro_file = File::open(&path).expect("open read-only");
        let obs = attach_open_description_observer(&path, &ro_file);
        let mut bw = BufWriter::with_capacity(64, ro_file);
        bw.write_all(b"buffered-bytes-that-cannot-flush")
            .expect("buffer in memory");

        let _inj = ScopedCloseErrnoInjection::install(libc::EINTR);
        let err = flush_and_close_buf_writer_once(
            bw,
            Ok(()),
            FileFinishSyncPolicy::SyncAll,
            CloseScopePrecondition::MacOsUniqueNonGuardedOrdinaryVnode,
        )
        .expect_err("flush + close failure must return Err");

        assert_eq!(ScopedCloseErrnoInjection::calls(), 1);
        assert!(!is_target_description_open(&obs));
        assert!(err.userspace_flush_error.is_some());
        assert!(err.sync_error.is_none(), "sync must not run after flush error");
        assert_eq!(err.close_error.as_ref().and_then(io::Error::raw_os_error), Some(libc::EINTR));
        assert_eq!(err.release_status, DescriptorReleaseStatus::ReleaseUnconfirmed);
    }

    #[test]
    fn single_close_ownership_and_sync_panic_unwind_release_open_description() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target_path = dir.path().join("target_slot.bin");
        let file = File::create(&target_path).expect("create target");
        let obs = attach_open_description_observer(&target_path, &file);
        let expected_fd = file.as_raw_fd();
        assert!(is_target_description_open(&obs));

        let (closed_fd, close_res, status) =
            close_owned_file_once(file, CloseScopePrecondition::ConservativeUnverifiedScope);
        assert_eq!(closed_fd, expected_fd);
        assert!(close_res.is_ok());
        assert_eq!(status, DescriptorReleaseStatus::ConfirmedReleased);
        assert!(!is_target_description_open(&obs));

        // Verify pre-close sync panic drops `File` cleanly once without leaking the descriptor.
        let panic_path = dir.path().join("sync_panic.bin");
        let panic_file = File::create(&panic_path).expect("create panic_file");
        let panic_obs = attach_open_description_observer(&panic_path, &panic_file);
        assert!(is_target_description_open(&panic_obs));
        let mut close_fn_called_on_panic = false;
        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            let _ = finish_and_close_file_with(
                panic_file,
                (9, 10),
                Ok(()),
                None,
                FileFinishSyncPolicy::SyncAll,
                CloseScopePrecondition::ConservativeUnverifiedScope,
                |_f| panic!("simulated panic inside pre-close sync"),
                |owned_fd, scope| {
                    close_fn_called_on_panic = true;
                    close_owned_fd_once(owned_fd, scope)
                },
            );
        }));
        assert!(unwind_res.is_err());
        assert!(!close_fn_called_on_panic);
        assert!(
            !is_target_description_open(&panic_obs),
            "RAII must close open file description on sync unwind"
        );
    }

    #[test]
    fn explicit_finish_proof_never_produced_when_any_prior_flush_sync_or_close_error_occurs() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scope = CloseScopePrecondition::ConservativeUnverifiedScope;

        // 1. Prior encoder/write failure with clean close -> Err, preserves primary_status.
        let p1 = dir.path().join("prior_err.bin");
        let f1 = File::create(&p1).expect("f1");
        let obs1 = attach_open_description_observer(&p1, &f1);
        let prior_err = Err((TTZipStatus::ErrInvalidPassword, "bad password".to_string()));
        let e1 = finish_and_close_file_for_proof(f1, (1, 1), prior_err, None, FileFinishSyncPolicy::None, scope)
            .expect_err("prior error must forbid proof");
        assert!(!is_target_description_open(&obs1));
        assert_eq!(e1.primary_status(), TTZipStatus::ErrInvalidPassword);
        assert_eq!(e1.prior_write_or_encoder_error.as_deref(), Some("bad password"));
        assert!(!e1.has_any_io_finish_error());
        assert!(e1.is_release_confirmed());

        // 2. Userspace flush error with clean close -> Err, no proof.
        let p2 = dir.path().join("flush_err.bin");
        let f2 = File::create(&p2).expect("f2");
        let obs2 = attach_open_description_observer(&p2, &f2);
        let flush_io_err = Some(io::Error::from_raw_os_error(libc::EIO));
        let e2 = finish_and_close_file_for_proof(f2, (2, 2), Ok(()), flush_io_err, FileFinishSyncPolicy::None, scope)
            .expect_err("flush error must forbid proof");
        assert!(!is_target_description_open(&obs2));
        assert!(e2.userspace_flush_error.is_some());
        assert!(e2.has_any_io_finish_error());

        // 3. Sync error with clean close -> Err, no proof.
        let p3 = dir.path().join("sync_err.bin");
        let f3 = File::create(&p3).expect("f3");
        let obs3 = attach_open_description_observer(&p3, &f3);
        let e3 = finish_and_close_file_with(
            f3,
            (3, 3),
            Ok(()),
            None,
            FileFinishSyncPolicy::SyncAll,
            scope,
            |_f| Err(io::Error::from_raw_os_error(libc::EIO)),
            close_owned_fd_once,
        )
        .expect_err("sync error must forbid proof");
        assert!(!is_target_description_open(&obs3));
        assert!(e3.sync_error.is_some());
        assert!(e3.close_error.is_none());
        assert!(e3.is_release_confirmed());

        // 4. Real BufWriter normal path succeeds via `flush_and_close_buf_writer_once`.
        let p4 = dir.path().join("bw_ok.bin");
        let f4 = File::create(&p4).expect("f4");
        let obs4 = attach_open_description_observer(&p4, &f4);
        let fd4 = f4.as_raw_fd();
        let mut bw4 = BufWriter::new(f4);
        bw4.write_all(b"ok").expect("write bw4");
        let closed4 =
            flush_and_close_buf_writer_once(bw4, Ok(()), FileFinishSyncPolicy::None, scope).expect("bw4");
        assert_eq!(closed4, fd4);
        assert!(!is_target_description_open(&obs4));
    }
}

#[cfg(test)]
#[path = "split_finish_tests.rs"]
mod split_finish_tests;
