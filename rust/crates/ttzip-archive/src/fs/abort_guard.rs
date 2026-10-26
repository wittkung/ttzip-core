// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! RAII file abort guard that deletes partially written, corrupted, or aborted output files upon drop unless committed.

use std::path::{Path, PathBuf};

/// RAII guard that automatically removes a target file from disk if dropped before being explicitly committed.
///
/// Ensures strict cancellation consistency and prevents corrupted or partial archive files from lingering
/// on disk when an operation is cancelled, fails with an I/O error, or panics.
#[derive(Debug)]
pub struct FileAbortGuard {
    path: PathBuf,
    active: bool,
}

impl FileAbortGuard {
    /// Creates a new abort guard for the specified file path.
    /// The guard is initially active (armed).
    #[inline]
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            active: true,
        }
    }

    /// Disarms the guard, committing the file and preventing its deletion upon drop.
    #[inline]
    pub fn commit(mut self) {
        self.active = false;
    }

    /// Disarms the guard and returns the underlying path.
    #[inline]
    pub fn into_path(mut self) -> PathBuf {
        self.active = false;
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
}

impl Drop for FileAbortGuard {
    #[inline]
    fn drop(&mut self) {
        if self.active && self.path.exists() {
            let _ = std::fs::remove_file(&self.path);
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

static INFLIGHT_JAIL_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Resolves `comps` step-by-step from `jail.root_fd()` using `O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC`
/// and returns an owned directory file descriptor.
pub(crate) fn resolve_jail_parent_fd(
    jail: &super::dir_jail::DirJail,
    comps: &[String],
    create_if_missing: bool,
) -> Result<libc::c_int, crate::types::TTZipStatus> {
    use crate::types::TTZipStatus;
    if create_if_missing {
        jail.create_dir_all_components(comps)?;
    }
    let mut cur_fd = unsafe { libc::fcntl(jail.root_fd(), libc::F_DUPFD_CLOEXEC, 0) };
    if cur_fd < 0 {
        return Err(TTZipStatus::ErrExtractionFailed);
    }
    let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    for seg in comps {
        let Ok(seg_c) = std::ffi::CString::new(seg.as_bytes()) else {
            unsafe { libc::close(cur_fd) };
            return Err(TTZipStatus::ErrSecurityViolation);
        };
        let next_fd = unsafe { libc::openat(cur_fd, seg_c.as_ptr(), flags) };
        unsafe { libc::close(cur_fd) };
        if next_fd < 0 {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        cur_fd = next_fd;
    }
    Ok(cur_fd)
}

/// Verifies that `jail` path components `comps` still resolve (without symlinks) to the exact same
/// directory inode `(st_dev, st_ino)` as `expected_fd`.
pub(crate) fn verify_jail_parent_fd_unchanged(
    jail: &super::dir_jail::DirJail,
    comps: &[String],
    expected_fd: libc::c_int,
) -> Result<(), crate::types::TTZipStatus> {
    use crate::types::TTZipStatus;
    let reopened_fd = resolve_jail_parent_fd(jail, comps, false)?;
    let (mut st_a, mut st_b): (libc::stat, libc::stat) =
        unsafe { (std::mem::zeroed(), std::mem::zeroed()) };
    let ok = unsafe {
        libc::fstat(reopened_fd, &mut st_a) == 0 && libc::fstat(expected_fd, &mut st_b) == 0
    } && (st_a.st_mode & libc::S_IFMT) == libc::S_IFDIR
        && st_a.st_dev == st_b.st_dev
        && st_a.st_ino == st_b.st_ino;
    unsafe {
        libc::close(reopened_fd);
    }
    if ok {
        Ok(())
    } else {
        Err(TTZipStatus::ErrSecurityViolation)
    }
}

/// Exclusively creates and opens a regular file leaf inside `parent_fd` with `O_CREAT | O_EXCL | O_NOFOLLOW`.
pub(crate) fn open_exclusive_file_in_dir(
    parent_fd: libc::c_int,
    leaf_c: &std::ffi::CStr,
) -> Result<std::fs::File, i32> {
    use std::os::unix::io::FromRawFd;
    let flags =
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    let fd = unsafe { libc::openat(parent_fd, leaf_c.as_ptr(), flags, 0o600 as libc::c_uint) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error()
            .raw_os_error()
            .unwrap_or(libc::EIO));
    }
    Ok(unsafe { std::fs::File::from_raw_fd(fd) })
}

/// Checks `fstatat(parent_fd, leaf_c, AT_SYMLINK_NOFOLLOW)` and returns `Ok(true)` if an existing
/// regular file is present, `Ok(false)` if missing (`ENOENT`), or `Err(ErrSecurityViolation)` if
/// a symlink, directory, or other non-regular file occupies `leaf_c`.
pub(crate) fn check_regular_or_missing_in_dir(
    parent_fd: libc::c_int,
    leaf_c: &std::ffi::CStr,
) -> Result<bool, crate::types::TTZipStatus> {
    use crate::types::TTZipStatus;
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    let rc =
        unsafe { libc::fstatat(parent_fd, leaf_c.as_ptr(), &mut st, libc::AT_SYMLINK_NOFOLLOW) };
    if rc == 0 {
        if (st.st_mode & libc::S_IFMT) == libc::S_IFREG {
            Ok(true)
        } else {
            Err(TTZipStatus::ErrSecurityViolation)
        }
    } else {
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        if errno == libc::ENOENT {
            Ok(false)
        } else {
            Err(TTZipStatus::ErrSecurityViolation)
        }
    }
}

/// Exclusively creates a sibling temporary file inside `parent_fd` with `O_CREAT | O_EXCL | O_NOFOLLOW`,
/// retrying with a new candidate name on `EEXIST` so no existing file is ever truncated.
pub(crate) fn open_exclusive_temp_in_dir(
    parent_fd: libc::c_int,
    leaf: &str,
) -> Result<(std::fs::File, std::ffi::CString), crate::types::TTZipStatus> {
    use crate::types::TTZipStatus;
    use std::sync::atomic::Ordering;
    let prefix: String = leaf.chars().take(48).collect();
    for attempt in 0..64u32 {
        let cand = if attempt == 0 {
            format!(".ttzip_inflight_{prefix}_0")
        } else {
            let seq = INFLIGHT_JAIL_SEQ.fetch_add(1, Ordering::Relaxed);
            format!(".ttzip_inflight_{prefix}_{seq:x}_{attempt}")
        };
        let cand_c =
            std::ffi::CString::new(cand.as_bytes()).map_err(|_| TTZipStatus::ErrSecurityViolation)?;
        match open_exclusive_file_in_dir(parent_fd, &cand_c) {
            Ok(file) => return Ok((file, cand_c)),
            Err(libc::EEXIST) => continue,
            Err(libc::ELOOP | libc::ENOTDIR | libc::EISDIR | libc::EPERM | libc::EACCES) => {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
            Err(_) => return Err(TTZipStatus::ErrExtractionFailed),
        }
    }
    Err(TTZipStatus::ErrExtractionFailed)
}

pub(crate) type InFlightOpenParts = (
    std::fs::File,
    libc::c_int,
    Vec<String>,
    std::ffi::CString,
    Option<std::ffi::CString>,
);

/// Opens an in-flight entry inside `jail` using an owned `parent_fd` and `O_CREAT | O_EXCL | O_NOFOLLOW`.
/// When `overwrite` is true and a regular file already exists at `out_rel`, opens an exclusive sibling
/// temporary file (retrying on `EEXIST`) to be atomically committed via `renameat(parent_fd, ...)`.
pub(crate) fn open_inflight_jail_entry(
    jail: &super::dir_jail::DirJail,
    out_rel: &str,
    overwrite: bool,
) -> Result<InFlightOpenParts, crate::types::TTZipStatus> {
    use crate::types::TTZipStatus;
    let comps = super::dir_jail::DirJail::extract_clean_components(out_rel)?;
    let (leaf, parent_comps) = comps.split_last().ok_or(TTZipStatus::ErrSecurityViolation)?;
    let leaf_c =
        std::ffi::CString::new(leaf.as_bytes()).map_err(|_| TTZipStatus::ErrSecurityViolation)?;
    let parent_fd = resolve_jail_parent_fd(jail, parent_comps, true)?;
    let res = match open_exclusive_file_in_dir(parent_fd, &leaf_c) {
        Ok(file) => Ok((file, parent_fd, parent_comps.to_vec(), leaf_c, None)),
        Err(libc::EEXIST) => (|| {
            let is_reg = check_regular_or_missing_in_dir(parent_fd, &leaf_c)?;
            if !is_reg || !overwrite {
                return Err(TTZipStatus::ErrExtractionFailed);
            }
            let (file, tmp_c) = open_exclusive_temp_in_dir(parent_fd, leaf)?;
            Ok((file, parent_fd, parent_comps.to_vec(), tmp_c, Some(leaf_c)))
        })(),
        Err(libc::ELOOP | libc::ENOTDIR | libc::EISDIR | libc::EPERM | libc::EACCES) => {
            Err(TTZipStatus::ErrSecurityViolation)
        }
        Err(_) => Err(TTZipStatus::ErrExtractionFailed),
    };
    if res.is_err() {
        unsafe {
            libc::close(parent_fd);
        }
    }
    res
}

/// RAII guard for an in-flight extracted regular file inside a [`super::dir_jail::DirJail`].
///
/// Holds an open directory descriptor (`parent_fd`) for the entry's parent directory:
/// - New files are created exclusively (`O_CREAT | O_EXCL | O_NOFOLLOW`).
/// - Overwrites (`overwrite == true` on `EEXIST`) open an exclusive sibling temporary file
///   (`.ttzip_inflight_*`, retrying on `EEXIST`) without `O_TRUNC`, preserving existing formal
///   files and shared hardlink inodes until [`Self::commit`] atomically installs the replacement
///   via `renameat(parent_fd, ...)`.
/// - Dropping before [`Self::commit`] removes the uncommitted file or sibling temporary file via
///   `unlinkat(parent_fd, ...)`.
pub struct InFlightJailFile {
    file: Option<std::fs::File>,
    parent_fd: libc::c_int,
    parent_comps: Vec<String>,
    cleanup_leaf: Option<std::ffi::CString>,
    commit_leaf: Option<std::ffi::CString>,
}

impl InFlightJailFile {
    /// Opens an in-flight regular file inside `jail` at `out_rel`.
    pub fn open(
        jail: &super::dir_jail::DirJail,
        out_rel: &str,
        overwrite: bool,
    ) -> Result<Self, crate::types::TTZipStatus> {
        let (file, parent_fd, parent_comps, cleanup_leaf, commit_leaf) =
            open_inflight_jail_entry(jail, out_rel, overwrite)?;
        Ok(Self {
            file: Some(file),
            parent_fd,
            parent_comps,
            cleanup_leaf: Some(cleanup_leaf),
            commit_leaf,
        })
    }

    /// Returns a mutable reference to the underlying open file handle.
    #[inline]
    pub fn file_mut(&mut self) -> &mut std::fs::File {
        self.file.as_mut().expect("in-flight file handle is open")
    }

    /// Flushes the file, verifies that the parent directory path in `jail` still resolves to
    /// `parent_fd`, atomically installs any sibling temporary file via `renameat(parent_fd, ...)`,
    /// and disarms drop cleanup.
    pub fn commit(mut self, jail: &super::dir_jail::DirJail) -> Result<(), crate::types::TTZipStatus> {
        use crate::types::TTZipStatus;
        if let Some(mut f) = self.file.take() {
            use std::io::Write;
            f.flush().map_err(|_| TTZipStatus::ErrExtractionFailed)?;
        }
        verify_jail_parent_fd_unchanged(jail, &self.parent_comps, self.parent_fd)?;
        if let (Some(cleanup_c), Some(target_c)) = (&self.cleanup_leaf, &self.commit_leaf) {
            let _ = check_regular_or_missing_in_dir(self.parent_fd, target_c)?;
            let rc = unsafe {
                libc::renameat(
                    self.parent_fd,
                    cleanup_c.as_ptr(),
                    self.parent_fd,
                    target_c.as_ptr(),
                )
            };
            if rc != 0 {
                return Err(TTZipStatus::ErrExtractionFailed);
            }
        }
        self.cleanup_leaf = None;
        Ok(())
    }
}

impl Drop for InFlightJailFile {
    fn drop(&mut self) {
        drop(self.file.take());
        if self.parent_fd >= 0 {
            if let Some(cleanup_c) = self.cleanup_leaf.take() {
                unsafe {
                    libc::unlinkat(self.parent_fd, cleanup_c.as_ptr(), 0);
                }
            }
            unsafe {
                libc::close(self.parent_fd);
            }
            self.parent_fd = -1;
        }
    }
}

/// Tracks forward hardlinks during sequential archive extraction and prevents stale deferred
/// hardlinks from overwriting later archive entries with the same destination path.
#[derive(Debug, Default)]
pub struct DeferredHardlinks {
    pending: Vec<(Vec<String>, Vec<String>)>,
}

impl DeferredHardlinks {
    /// Invalidates any pending deferred hardlink whose destination matches `dst_comps`.
    pub fn invalidate_destination(&mut self, dst_comps: &[String]) {
        self.pending.retain(|(_, dst)| dst.as_slice() != dst_comps);
    }

    /// Attempts to create hardlink `dst_rel -> src_rel` immediately; queues it as pending only
    /// if `src_rel` does not exist yet (`ErrFileNotFound`).
    pub fn record_or_apply(
        &mut self,
        jail: &super::dir_jail::DirJail,
        dst_rel: &str,
        src_rel: &str,
    ) -> Result<(), crate::types::TTZipStatus> {
        use crate::types::TTZipStatus;
        let dst_comps = super::dir_jail::DirJail::extract_clean_components(dst_rel)?;
        let src_comps = super::dir_jail::DirJail::extract_clean_components(src_rel)?;
        self.invalidate_destination(&dst_comps);
        if dst_comps == src_comps {
            return Ok(());
        }
        match jail.create_hardlink_components(&dst_comps, &src_comps) {
            Ok(()) => self.resolve_ready(jail),
            Err(TTZipStatus::ErrFileNotFound) => {
                self.pending.push((src_comps, dst_comps));
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    /// Notifies tracker that `entry_comps` has been committed on disk as a regular file,
    /// handling optional payload-carrying hardlink target `hardlink_target_rel` and resolving
    /// any pending forward hardlinks that were waiting on `entry_comps`.
    pub fn on_regular_committed(
        &mut self,
        jail: &super::dir_jail::DirJail,
        entry_comps: &[String],
        hardlink_target_rel: Option<&str>,
    ) -> Result<(), crate::types::TTZipStatus> {
        self.invalidate_destination(entry_comps);
        if let Some(src_rel) = hardlink_target_rel {
            let src_comps = super::dir_jail::DirJail::extract_clean_components(src_rel)?;
            if entry_comps != src_comps.as_slice() {
                self.invalidate_destination(&src_comps);
                jail.create_hardlink_components(&src_comps, entry_comps)?;
            }
        }
        self.resolve_ready(jail)
    }

    /// Resolves any pending hardlinks whose source file now exists in `jail`.
    pub fn resolve_ready(
        &mut self,
        jail: &super::dir_jail::DirJail,
    ) -> Result<(), crate::types::TTZipStatus> {
        use crate::types::TTZipStatus;
        loop {
            let mut progressed = false;
            let mut still_pending = Vec::with_capacity(self.pending.len());
            for (src_comps, dst_comps) in self.pending.drain(..) {
                match jail.create_hardlink_components(&dst_comps, &src_comps) {
                    Ok(()) => progressed = true,
                    Err(TTZipStatus::ErrFileNotFound) => {
                        still_pending.push((src_comps, dst_comps));
                    }
                    Err(err) => return Err(err),
                }
            }
            self.pending = still_pending;
            if !progressed || self.pending.is_empty() {
                break;
            }
        }
        Ok(())
    }

    /// Finalizes any remaining deferred hardlinks after all archive entries have been processed.
    pub fn finalize(
        mut self,
        jail: &super::dir_jail::DirJail,
    ) -> Result<(), crate::types::TTZipStatus> {
        use crate::types::TTZipStatus;
        self.resolve_ready(jail)?;
        for (src_comps, dst_comps) in self.pending {
            if src_comps == dst_comps {
                continue;
            }
            match jail.create_hardlink_components(&dst_comps, &src_comps) {
                Ok(()) => {}
                Err(TTZipStatus::ErrFileNotFound) => {
                    jail.create_hardlink_components(&src_comps, &dst_comps)?;
                }
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }
}

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
            // Drops here without commit
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
