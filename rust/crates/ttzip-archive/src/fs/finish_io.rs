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
#[path = "finish_io_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "split_finish_tests.rs"]
mod split_finish_tests;
