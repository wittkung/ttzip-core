// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! RAII file abort guard that deletes partially written, corrupted, or aborted output files upon drop unless committed.

use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[path = "abort_quarantine.rs"]
pub(crate) mod quarantine;

pub use quarantine::set_abort_guard_after_quarantine_hook;
pub(crate) use quarantine::{
    FsAbortCleanupDisposition, FsAbortCleanupError, FsAbortObserver, quarantine_verify_and_unlink,
};

type AfterCheckHook = Box<dyn FnOnce(&Path) + Send + 'static>;
type PostOpenHook = Box<dyn FnOnce() -> std::io::Result<()> + Send + 'static>;
type VerifiedParentDir<'a> = (RawFd, (u64, u64), &'a Path);

thread_local! {
    static ABORT_GUARD_AFTER_CHECK_HOOK: RefCell<Option<AfterCheckHook>> =
        const { RefCell::new(None) };
    static ABORT_GUARD_FSTAT_HOOK: RefCell<Option<PostOpenHook>> =
        const { RefCell::new(None) };
    static ABORT_GUARD_DUP_FD_HOOK: RefCell<Option<PostOpenHook>> =
        const { RefCell::new(None) };
}

/// Registers a one-shot hook on the current thread invoked during `fstat_dev_ino` after `openat`.
pub fn set_abort_guard_fstat_hook<F>(hook: F)
where
    F: FnOnce() -> std::io::Result<()> + Send + 'static,
{
    ABORT_GUARD_FSTAT_HOOK.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(hook));
    });
}

/// Registers a one-shot hook on the current thread invoked during `dup_fd_cloexec` after `openat`.
pub fn set_abort_guard_dup_fd_hook<F>(hook: F)
where
    F: FnOnce() -> std::io::Result<()> + Send + 'static,
{
    ABORT_GUARD_DUP_FD_HOOK.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(hook));
    });
}

/// Registers a one-shot hook on the current thread invoked after `FileAbortGuard` validates target identity and before unlinking.
pub fn set_abort_guard_after_check_hook<F>(hook: F)
where
    F: FnOnce(&Path) + Send + 'static,
{
    ABORT_GUARD_AFTER_CHECK_HOOK.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(hook));
    });
}

fn run_abort_guard_after_check_hook(path: &Path) {
    let hook = ABORT_GUARD_AFTER_CHECK_HOOK.with(|slot| slot.borrow_mut().take());
    if let Some(cb) = hook {
        cb(path);
    }
}

pub(crate) fn fd_stat_info(
    fd: RawFd,
) -> std::io::Result<(u64, u64, libc::mode_t, libc::nlink_t)> {
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut st) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok((st.st_dev as u64, st.st_ino, st.st_mode, st.st_nlink))
}

pub(crate) fn fstat_dev_ino(fd: RawFd) -> std::io::Result<(u64, u64)> {
    if let Some(hook) = ABORT_GUARD_FSTAT_HOOK.with(|slot| slot.borrow_mut().take()) {
        hook()?;
    }
    let (dev, ino, _mode, _) = fd_stat_info(fd)?;
    Ok((dev, ino))
}

pub(crate) fn dup_fd_cloexec(fd: RawFd) -> std::io::Result<OwnedFd> {
    if let Some(hook) = ABORT_GUARD_DUP_FD_HOOK.with(|slot| slot.borrow_mut().take()) {
        hook()?;
    }
    let dup_raw = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 0) };
    if dup_raw < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { OwnedFd::from_raw_fd(dup_raw) })
}

pub(crate) fn fstatat_dev_ino_nofollow(
    dir_fd: RawFd,
    name: &CStr,
) -> std::io::Result<(u64, u64, libc::mode_t)> {
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::fstatat(dir_fd, name.as_ptr(), &mut st, libc::AT_SYMLINK_NOFOLLOW) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok((st.st_dev as u64, st.st_ino, st.st_mode))
}

pub(crate) fn path_dev_ino_nofollow(path: &Path) -> std::io::Result<(u64, u64, libc::mode_t)> {
    let c_path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "path contains NUL")
    })?;
    fstatat_dev_ino_nofollow(libc::AT_FDCWD, &c_path)
}

pub(crate) fn open_dir_nofollow(path: &Path) -> std::io::Result<(OwnedFd, (u64, u64))> {
    let c_path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "path contains NUL")
    })?;
    let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    let raw = unsafe { libc::open(c_path.as_ptr(), flags) };
    if raw < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    let (dev, ino, mode, _) = fd_stat_info(fd.as_raw_fd())?;
    if (mode & libc::S_IFMT) != libc::S_IFDIR {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "opened handle is not a directory",
        ));
    }
    Ok((fd, (dev, ino)))
}

fn open_file_identity_in_dir(parent_fd: RawFd, leaf: &CStr) -> Option<(OwnedFd, (u64, u64))> {
    let flags = libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    let raw = unsafe { libc::openat(parent_fd, leaf.as_ptr(), flags) };
    if raw < 0 {
        return None;
    }
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    let (dev, ino, mode, _) = fd_stat_info(fd.as_raw_fd()).ok()?;
    if (mode & libc::S_IFMT) != libc::S_IFREG {
        return None;
    }
    Some((fd, (dev, ino)))
}

/// RAII guard that automatically removes a target file from disk if dropped before being explicitly committed.
///
/// Ensures strict cancellation consistency and verifies parent directory and file `(st_dev, st_ino)` ownership
/// before unlinking so path reuse or directory replacement never deletes unrelated files.
#[derive(Debug)]
pub struct FileAbortGuard {
    path: PathBuf,
    parent_path: Option<PathBuf>,
    parent_fd: Option<Arc<OwnedFd>>,
    parent_id: Option<(u64, u64)>,
    leaf_name: Option<CString>,
    file_fd: Option<OwnedFd>,
    file_id: Option<(u64, u64)>,
    preexisting_id: Option<(u64, u64)>,
    replaced_before_bind: bool,
    active: bool,
    attempted_cleanup: bool,
    observer: Option<Arc<dyn FsAbortObserver>>,
}

type ResolvedParentAndLeaf = (
    Option<PathBuf>,
    Option<CString>,
    Option<Arc<OwnedFd>>,
    Option<(u64, u64)>,
);

/// Components captured after exclusive creation and identity verification.
pub(crate) struct VerifiedGuardParts {
    pub path: PathBuf,
    pub parent_path: PathBuf,
    pub parent_fd: Arc<OwnedFd>,
    pub parent_id: (u64, u64),
    pub leaf_name: CString,
    pub file_fd: Option<OwnedFd>,
    pub file_id: (u64, u64),
    pub observer: Option<Arc<dyn FsAbortObserver>>,
}

impl FileAbortGuard {
    pub(crate) fn from_exclusively_created_parts(parts: VerifiedGuardParts) -> Self {
        let VerifiedGuardParts {
            path, parent_path, parent_fd, parent_id, leaf_name, file_fd, file_id, observer,
        } = parts;
        Self {
            path,
            parent_path: Some(parent_path),
            parent_fd: Some(parent_fd),
            parent_id: Some(parent_id),
            leaf_name: Some(leaf_name),
            file_fd,
            file_id: Some(file_id),
            preexisting_id: None,
            replaced_before_bind: false,
            active: true,
            attempted_cleanup: false,
            observer,
        }
    }

    #[inline]
    pub(crate) fn expected_file_id(&self) -> Option<(u64, u64)> {
        self.file_id
    }

    #[inline]
    pub(crate) fn observer(&self) -> Option<&Arc<dyn FsAbortObserver>> {
        self.observer.as_ref()
    }

    pub(crate) fn abort_checked_fs_detailed(&mut self) -> Result<(), Box<FsAbortCleanupError>> {
        let res = self.abort_checked_internal_detailed();
        let _ = self.observer.take();
        res
    }

    fn resolve_parent_and_leaf(path: &Path) -> ResolvedParentAndLeaf {
        let parent_path = path.parent().map(|p| {
            if p.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                p.to_path_buf()
            }
        });
        let leaf_name = path
            .file_name()
            .and_then(|n| CString::new(n.as_bytes()).ok());
        let (parent_fd, parent_id) = match parent_path.as_deref().map(open_dir_nofollow) {
            Some(Ok((fd, id))) => (Some(Arc::new(fd)), Some(id)),
            _ => (None, None),
        };
        (parent_path, leaf_name, parent_fd, parent_id)
    }

    /// Creates a new abort guard for the specified file path.
    /// Captures parent directory handle/identity and, if the file already exists, its `(st_dev, st_ino)` identity.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let (parent_path, leaf_name, parent_fd, parent_id) = Self::resolve_parent_and_leaf(&path);
        let (file_fd, file_id) = match (&parent_fd, &leaf_name) {
            (Some(pfd), Some(leaf)) => match open_file_identity_in_dir(pfd.as_raw_fd(), leaf) {
                Some((ffd, fid)) => (Some(ffd), Some(fid)),
                None => (None, None),
            },
            _ => (None, None),
        };
        Self {
            path,
            parent_path,
            parent_fd,
            parent_id,
            leaf_name,
            file_fd,
            file_id,
            preexisting_id: None,
            replaced_before_bind: false,
            active: true,
            attempted_cleanup: false,
            observer: None,
        }
    }

    /// Creates a guard armed for a file that has NOT yet been created (such as before a C library opens `path`).
    /// Any file already occupying `path` at construction time is recorded as `preexisting_id` and is NEVER adopted as owned.
    #[must_use]
    pub fn for_pending_creation(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let (parent_path, leaf_name, parent_fd, parent_id) = Self::resolve_parent_and_leaf(&path);
        let preexisting_id = match (&parent_fd, &leaf_name) {
            (Some(pfd), Some(leaf)) => fstatat_dev_ino_nofollow(pfd.as_raw_fd(), leaf)
                .ok()
                .map(|(d, i, _)| (d, i)),
            _ => None,
        };
        Self {
            path,
            parent_path,
            parent_fd,
            parent_id,
            leaf_name,
            file_fd: None,
            file_id: None,
            preexisting_id,
            replaced_before_bind: false,
            active: true,
            attempted_cleanup: false,
            observer: None,
        }
    }

    /// Creates a guard bound directly to the `std::fs::File` descriptor returned at creation time,
    /// eliminating any re-open window between `File::create` / `OpenOptions::open` and guard construction.
    #[must_use]
    pub fn from_created_file(path: impl Into<PathBuf>, file: &std::fs::File) -> Self {
        let path = path.into();
        let (parent_path, leaf_name, parent_fd, parent_id) = Self::resolve_parent_and_leaf(&path);
        let (file_fd, file_id) = match dup_fd_cloexec(file.as_raw_fd())
            .and_then(|owned| fd_stat_info(owned.as_raw_fd()).map(|(dev, ino, mode, _)| (owned, dev, ino, mode)))
        {
            Ok((owned, dev, ino, mode)) if (mode & libc::S_IFMT) == libc::S_IFREG => {
                (Some(owned), Some((dev, ino)))
            }
            _ => (None, None),
        };
        let replaced_before_bind = match (&parent_fd, &leaf_name, file_id) {
            (Some(pfd), Some(leaf), Some(expected_id)) => {
                match fstatat_dev_ino_nofollow(pfd.as_raw_fd(), leaf) {
                    Ok((d, i, mode)) => (mode & libc::S_IFMT) != libc::S_IFREG || (d, i) != expected_id,
                    Err(_) => true,
                }
            }
            _ => true,
        };
        Self {
            path,
            parent_path,
            parent_fd,
            parent_id,
            leaf_name,
            file_fd,
            file_id,
            preexisting_id: None,
            replaced_before_bind,
            active: true,
            attempted_cleanup: false,
            observer: None,
        }
    }

    /// Captures the `(st_dev, st_ino)` identity of the monitored file after creation,
    /// refusing to adopt any inode that pre-existed before the pending creation began.
    pub fn capture_created_identity(&mut self) -> std::io::Result<()> {
        if self.parent_fd.is_none() {
            if let Some(parent) = self.parent_path.as_deref() {
                let (fd, id) = open_dir_nofollow(parent)?;
                self.parent_fd = Some(Arc::new(fd));
                self.parent_id = Some(id);
            }
        }
        let (Some(pfd), Some(leaf)) = (&self.parent_fd, &self.leaf_name) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "missing parent directory handle or file leaf name",
            ));
        };
        let Some((ffd, fid)) = open_file_identity_in_dir(pfd.as_raw_fd(), leaf) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "created file not found for identity capture",
            ));
        };
        if self.preexisting_id.is_some() {
            self.replaced_before_bind = true;
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "refusing to claim ownership of pre-existing path '{}' (preexisting_id={:?}, observed_id={:?})",
                    self.path.display(),
                    self.preexisting_id,
                    fid
                ),
            ));
        }
        self.file_fd = Some(ffd);
        self.file_id = Some(fid);
        Ok(())
    }

    /// Disarms the guard, committing the file and preventing its deletion upon drop.
    #[inline]
    pub fn commit(mut self) {
        self.active = false;
        self.attempted_cleanup = true;
        let _ = self.observer.take();
    }

    fn verify_parent_unchanged(
        &self,
    ) -> Result<VerifiedParentDir<'_>, Box<FsAbortCleanupError>> {
        let (Some(pfd), Some(expected_parent_id), Some(parent_path)) =
            (&self.parent_fd, self.parent_id, self.parent_path.as_deref())
        else {
            return Err(Box::new(FsAbortCleanupError {
                disposition: FsAbortCleanupDisposition::OwnershipOrLocationUnconfirmed,
                original_path: self.path.clone(),
                final_known_path: None,
                expected_identity: self.file_id,
                observed_identity: None,
                os_error_code: None,
                io_kind: std::io::ErrorKind::InvalidData,
                detail: "parent directory identity unavailable; refusing cleanup".to_string(),
            }));
        };
        let (cur_dev, cur_ino, cur_mode) =
            path_dev_ino_nofollow(parent_path).map_err(|err| {
                Box::new(FsAbortCleanupError {
                    disposition: FsAbortCleanupDisposition::OwnershipOrLocationUnconfirmed,
                    original_path: self.path.clone(),
                    final_known_path: None,
                    expected_identity: Some(expected_parent_id),
                    observed_identity: None,
                    os_error_code: err.raw_os_error(),
                    io_kind: err.kind(),
                    detail: err.to_string(),
                })
            })?;
        if (cur_mode & libc::S_IFMT) != libc::S_IFDIR || (cur_dev, cur_ino) != expected_parent_id {
            return Err(Box::new(FsAbortCleanupError {
                disposition: FsAbortCleanupDisposition::OwnershipOrLocationUnconfirmed,
                original_path: self.path.clone(),
                final_known_path: None,
                expected_identity: Some(expected_parent_id),
                observed_identity: Some((cur_dev, cur_ino)),
                os_error_code: None,
                io_kind: std::io::ErrorKind::InvalidData,
                detail: format!(
                    "parent directory identity changed (outcome=identity_mismatch_location_unconfirmed, orig_path='{}', expected_parent_id={:?}, observed_parent_id={:?}); refusing destructive cleanup",
                    self.path.display(),
                    expected_parent_id,
                    (cur_dev, cur_ino)
                ),
            }));
        }
        Ok((pfd.as_raw_fd(), expected_parent_id, parent_path))
    }

    fn verify_owned_leaf_before_unlink(
        &self,
        parent_raw: RawFd,
        leaf: &CStr,
        expected_file_id: (u64, u64),
    ) -> Result<bool, Box<FsAbortCleanupError>> {
        if self.replaced_before_bind {
            return Err(Box::new(FsAbortCleanupError {
                disposition: FsAbortCleanupDisposition::RefusedOwnershipMismatch,
                original_path: self.path.clone(),
                final_known_path: Some(self.path.clone()),
                expected_identity: Some(expected_file_id),
                observed_identity: self.preexisting_id,
                os_error_code: None,
                io_kind: std::io::ErrorKind::InvalidData,
                detail: format!(
                    "staged file path '{}' was pre-occupied or replaced before ownership bind; refusing destructive cleanup",
                    self.path.display()
                ),
            }));
        }
        match fstatat_dev_ino_nofollow(parent_raw, leaf) {
            Ok((dev, ino, mode)) => {
                if (mode & libc::S_IFMT) != libc::S_IFREG || (dev, ino) != expected_file_id {
                    return Err(Box::new(FsAbortCleanupError {
                        disposition: FsAbortCleanupDisposition::RefusedOwnershipMismatch,
                        original_path: self.path.clone(),
                        final_known_path: Some(self.path.clone()),
                        expected_identity: Some(expected_file_id),
                        observed_identity: Some((dev, ino)),
                        os_error_code: None,
                        io_kind: std::io::ErrorKind::InvalidData,
                        detail: format!(
                            "staged file path '{}' replaced by another inode (expected={:?}, observed={:?}); refusing destructive cleanup",
                            self.path.display(),
                            expected_file_id,
                            (dev, ino)
                        ),
                    }));
                }
                Ok(true)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if let Some(ffd) = &self.file_fd {
                    if let Ok((_, _, _, nlink)) = fd_stat_info(ffd.as_raw_fd()) {
                        if nlink > 0 {
                            return Err(Box::new(FsAbortCleanupError {
                                disposition: FsAbortCleanupDisposition::OwnershipOrLocationUnconfirmed,
                                original_path: self.path.clone(),
                                final_known_path: None,
                                expected_identity: Some(expected_file_id),
                                observed_identity: None,
                                os_error_code: None,
                                io_kind: std::io::ErrorKind::InvalidData,
                                detail: "staged file moved away from original path and remains linked".to_string(),
                            }));
                        }
                    }
                }
                Ok(false)
            }
            Err(e) => Err(Box::new(FsAbortCleanupError {
                disposition: FsAbortCleanupDisposition::OwnershipOrLocationUnconfirmed,
                original_path: self.path.clone(),
                final_known_path: None,
                expected_identity: Some(expected_file_id),
                observed_identity: None,
                os_error_code: e.raw_os_error(),
                io_kind: e.kind(),
                detail: e.to_string(),
            })),
        }
    }

    fn abort_without_captured_file_id(
        &self,
        parent_raw: RawFd,
        leaf: &CStr,
    ) -> Result<(), Box<FsAbortCleanupError>> {
        match fstatat_dev_ino_nofollow(parent_raw, leaf) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Ok((dev, ino, _)) => Err(Box::new(FsAbortCleanupError {
                disposition: FsAbortCleanupDisposition::RefusedOwnershipMismatch,
                original_path: self.path.clone(),
                final_known_path: Some(self.path.clone()),
                expected_identity: None,
                observed_identity: Some((dev, ino)),
                os_error_code: None,
                io_kind: std::io::ErrorKind::InvalidData,
                detail: format!(
                    "untracked or pre-existing file occupies staged path '{}' (inode={:?}, preexisting_id={:?}); refusing destructive cleanup",
                    self.path.display(),
                    (dev, ino),
                    self.preexisting_id
                ),
            })),
            Err(e) => Err(Box::new(FsAbortCleanupError {
                disposition: FsAbortCleanupDisposition::OwnershipOrLocationUnconfirmed,
                original_path: self.path.clone(),
                final_known_path: None,
                expected_identity: None,
                observed_identity: None,
                os_error_code: e.raw_os_error(),
                io_kind: e.kind(),
                detail: e.to_string(),
            })),
        }
    }

    fn abort_checked_internal_detailed(&mut self) -> Result<(), Box<FsAbortCleanupError>> {
        if !self.active {
            return Ok(());
        }
        self.active = false;
        self.attempted_cleanup = true;
        if self.parent_fd.is_none() && self.file_id.is_none() {
            return match path_dev_ino_nofollow(&self.path) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Ok((dev, ino, _)) => Err(Box::new(FsAbortCleanupError {
                    disposition: FsAbortCleanupDisposition::OwnershipOrLocationUnconfirmed,
                    original_path: self.path.clone(),
                    final_known_path: None,
                    expected_identity: None,
                    observed_identity: Some((dev, ino)),
                    os_error_code: None,
                    io_kind: std::io::ErrorKind::InvalidData,
                    detail: "parent directory identity unavailable for existing path; refusing cleanup".to_string(),
                })),
                Err(e) => Err(Box::new(FsAbortCleanupError {
                    disposition: FsAbortCleanupDisposition::OwnershipOrLocationUnconfirmed,
                    original_path: self.path.clone(),
                    final_known_path: None,
                    expected_identity: None,
                    observed_identity: None,
                    os_error_code: e.raw_os_error(),
                    io_kind: e.kind(),
                    detail: e.to_string(),
                })),
            };
        }
        let (parent_raw, expected_parent_id, parent_path) = self.verify_parent_unchanged()?;
        let Some(leaf) = self.leaf_name.as_deref() else {
            return Err(Box::new(FsAbortCleanupError {
                disposition: FsAbortCleanupDisposition::OwnershipOrLocationUnconfirmed,
                original_path: self.path.clone(),
                final_known_path: None,
                expected_identity: self.file_id,
                observed_identity: None,
                os_error_code: None,
                io_kind: std::io::ErrorKind::InvalidInput,
                detail: "missing leaf name".to_string(),
            }));
        };
        let Some(expected_file_id) = self.file_id else {
            return self.abort_without_captured_file_id(parent_raw, leaf);
        };
        if !self.verify_owned_leaf_before_unlink(parent_raw, leaf, expected_file_id)? {
            return Ok(());
        }
        run_abort_guard_after_check_hook(&self.path);
        quarantine_verify_and_unlink(
            &self.path,
            parent_path,
            parent_raw,
            expected_parent_id,
            leaf,
            expected_file_id,
        )
    }

    fn abort_checked_internal(&mut self) -> std::io::Result<()> {
        self.abort_checked_internal_detailed()
            .map_err(|e| e.into_io_error())
    }

    /// Explicitly removes the monitored file after verifying parent directory and inode ownership.
    #[inline]
    pub fn abort_checked(mut self) -> std::io::Result<()> {
        let res = self.abort_checked_internal();
        let _ = self.observer.take();
        res
    }

    /// Disarms the guard and returns the underlying path.
    #[inline]
    pub fn into_path(mut self) -> PathBuf {
        self.active = false;
        self.attempted_cleanup = true;
        let _ = self.observer.take();
        std::mem::take(&mut self.path)
    }

    /// Returns a reference to the monitored file path.
    #[inline]
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns true if the guard is still active (armed).
    #[inline]
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Returns true if a pre-existing object already occupied the path when `for_pending_creation` ran.
    #[inline]
    #[must_use]
    pub fn has_preexisting(&self) -> bool {
        self.preexisting_id.is_some()
    }
}

impl Drop for FileAbortGuard {
    #[inline]
    fn drop(&mut self) {
        if self.active && !self.attempted_cleanup {
            let observer = self.observer.take();
            if let Err(err) = self.abort_checked_internal_detailed() {
                if let Some(obs) = observer {
                    obs.on_abort_cleanup_error(*err);
                }
            }
        } else {
            let _ = self.observer.take();
        }
    }
}

// Compile-time barrier ensuring thread-safety and unpin invariants
const _: () = {
    const fn assert_send_sync<T: Send + Sync + 'static>() {}
    const fn assert_unpin<T: Unpin>() {}

    assert_send_sync::<FileAbortGuard>();
    assert_unpin::<FileAbortGuard>();
};

pub use super::collision::staging::DeferredHardlinks;
pub use super::transactional_writer::InFlightJailFile;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_abort_guard_uncommitted_deletes_file() {
        let temp_dir = std::env::temp_dir().join(format!("abort_guard_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let test_file = temp_dir.join("partial_archive.zip");

        std::fs::write(&test_file, b"corrupted data").unwrap();
        assert!(test_file.exists());

        {
            let guard = FileAbortGuard::new(&test_file);
            assert!(guard.is_active());
            assert_eq!(guard.path(), test_file.as_path());
        }

        assert!(!test_file.exists());
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_file_abort_guard_committed_preserves_file() {
        let temp_dir = std::env::temp_dir().join(format!("abort_guard_commit_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let test_file = temp_dir.join("valid_archive.zip");

        std::fs::write(&test_file, b"complete data").unwrap();
        assert!(test_file.exists());

        {
            let guard = FileAbortGuard::new(&test_file);
            guard.commit();
        }

        assert!(test_file.exists());
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
