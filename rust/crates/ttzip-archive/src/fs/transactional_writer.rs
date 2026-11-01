// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Transactional atomic file writer that isolates in-flight writes to a sibling temporary file
//! before performing an atomic rename upon commit, preventing partially written or corrupted
//! archive files on disk.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::abort_guard::FileAbortGuard;

static TMP_COUNTER: AtomicU64 = AtomicU64::new(1);

/// RAII-based transactional file writer.
///
/// All payload data is written to a temporary sibling file in the same directory.
/// When [`TransactionalFileWriter::commit`] is called:
/// 1. Underlying file buffers are flushed and synchronized to disk (`sync_all`).
/// 2. The open file handle is safely closed.
/// 3. The temporary file is atomically renamed over the target path (`std::fs::rename`).
/// 4. The abort guard is committed, leaving the finalized file on disk.
///
/// If dropped before `commit` (due to error, panic, or cancellation), the temporary file is
/// immediately deleted from disk, preserving file system cleanliness.
#[derive(Debug)]
pub struct TransactionalFileWriter {
    file: Option<File>,
    guard: Option<FileAbortGuard>,
    target_path: PathBuf,
}

impl TransactionalFileWriter {
    /// Creates a new transactional writer targeting `target_path`.
    ///
    /// The temporary file is created in the same directory as `target_path` to guarantee
    /// that the final rename operation is atomic across identical filesystem boundaries.
    ///
    /// # Errors
    /// Returns [`std::io::Error`] if the parent directory cannot be created or if the temporary
    /// file cannot be opened.
    pub fn create(target_path: impl Into<PathBuf>) -> std::io::Result<Self> {
        let target_path = target_path.into();
        let parent_dir = target_path.parent().unwrap_or_else(|| Path::new("."));
        let parent_dir = if parent_dir.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent_dir
        };

        if !parent_dir.exists() {
            std::fs::create_dir_all(parent_dir)?;
        }

        let file_stem = target_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("ttzip_payload");

        let pid = std::process::id();

        // Attempt up to 10 unique candidate temporary file names to prevent collisions.
        let mut attempts = 0;
        loop {
            let counter = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let timestamp_ns = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);

            let tmp_name = format!(".{}.ttzip_tmp_{:x}_{:x}_{:x}", file_stem, pid, timestamp_ns, counter);
            let tmp_path = parent_dir.join(tmp_name);

            match OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&tmp_path)
            {
                Ok(file) => {
                    let guard = FileAbortGuard::from_created_file(&tmp_path, &file);
                    return Ok(Self {
                        file: Some(file),
                        guard: Some(guard),
                        target_path,
                    });
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    attempts += 1;
                    if attempts >= 10 {
                        return Err(err);
                    }
                }
                Err(err) => return Err(err),
            }
        }
    }

    /// Returns a reference to the final target file path.
    #[inline]
    #[must_use]
    pub fn target_path(&self) -> &Path {
        &self.target_path
    }

    /// Returns a reference to the temporary working file path.
    ///
    /// # Panics
    /// Panics if called after commit or abort has disarmed the writer.
    #[inline]
    #[must_use]
    pub fn temp_path(&self) -> &Path {
        self.guard
            .as_ref()
            .expect("temporary path accessed after disarm")
            .path()
    }

    /// Returns a reference to the underlying file descriptor.
    ///
    /// # Panics
    /// Panics if called after commit or abort has closed the file handle.
    #[inline]
    #[must_use]
    pub fn file(&self) -> &File {
        self.file.as_ref().expect("file handle accessed after close")
    }

    /// Returns a mutable reference to the underlying file descriptor.
    ///
    /// # Panics
    /// Panics if called after commit or abort has closed the file handle.
    #[inline]
    pub fn file_mut(&mut self) -> &mut File {
        self.file.as_mut().expect("file handle accessed after close")
    }

    /// Flushes user-space buffers and asks the OS to synchronize metadata and payload to disk.
    pub fn sync_all(&mut self) -> std::io::Result<()> {
        let file = self.file.as_mut().ok_or_else(|| {
            std::io::Error::other("transactional file handle already closed")
        })?;
        file.flush()?;
        file.sync_all()
    }

    /// Flushes user-space buffers and asks the OS to synchronize payload data to disk.
    pub fn sync_data(&mut self) -> std::io::Result<()> {
        let file = self.file.as_mut().ok_or_else(|| {
            std::io::Error::other("transactional file handle already closed")
        })?;
        file.flush()?;
        file.sync_data()
    }

    /// Explicitly aborts the transaction, closing the file and deleting the temporary file.
    pub fn abort(mut self) {
        drop(self.file.take());
        if let Some(guard) = self.guard.take() {
            let _ = guard.abort_checked();
        }
    }

    /// Commits the transaction by flushing all data, closing the file handle, and atomically
    /// renaming the temporary file to the target path.
    ///
    /// # Errors
    /// Returns [`std::io::Error`] if flushing, syncing, or file renaming fails.
    /// If an error occurs, the temporary file is unlinked during cleanup.
    pub fn commit(mut self) -> std::io::Result<()> {
        // Step 1: Flush user buffers and sync to physical media.
        if let Some(mut file) = self.file.take() {
            file.flush()?;
            file.sync_all()?;
            // Explicitly drop file handle to release Windows/NFS locks before rename.
            drop(file);
        }

        // Step 2: Perform atomic rename from temp path to target path.
        let guard = self.guard.as_ref().ok_or_else(|| {
            std::io::Error::other("transactional writer already disarmed")
        })?;
        let tmp_path = guard.path().to_path_buf();

        std::fs::rename(&tmp_path, &self.target_path)?;

        // Step 3: Disarm guard now that the rename has completed successfully.
        if let Some(guard) = self.guard.take() {
            guard.commit();
        }

        Ok(())
    }
}

impl Write for TransactionalFileWriter {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let file = self.file.as_mut().ok_or_else(|| {
            std::io::Error::other("cannot write: file handle already closed")
        })?;
        file.write(buf)
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        let file = self.file.as_mut().ok_or_else(|| {
            std::io::Error::other("cannot flush: file handle already closed")
        })?;
        file.flush()
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        let file = self.file.as_mut().ok_or_else(|| {
            std::io::Error::other("cannot write_all: file handle already closed")
        })?;
        file.write_all(buf)
    }
}

impl Read for TransactionalFileWriter {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let file = self.file.as_mut().ok_or_else(|| {
            std::io::Error::other("cannot read: file handle already closed")
        })?;
        file.read(buf)
    }
}

impl Seek for TransactionalFileWriter {
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let file = self.file.as_mut().ok_or_else(|| {
            std::io::Error::other("cannot seek: file handle already closed")
        })?;
        file.seek(pos)
    }
}

impl Drop for TransactionalFileWriter {
    #[inline]
    fn drop(&mut self) {
        // Ensure open descriptor is closed first to avoid locked files on Windows.
        drop(self.file.take());

        // Unlink remaining temporary file if transaction was not committed.
        if let Some(guard) = self.guard.take() {
            let _ = guard.abort_checked();
        }
    }
}

// Compile-time static concurrency assertions.
const _: () = {
    const fn assert_send_sync<T: Send + Sync + 'static>() {}
    const fn assert_unpin<T: Unpin>() {}

    assert_send_sync::<TransactionalFileWriter>();
    assert_unpin::<TransactionalFileWriter>();
};

static INFLIGHT_JAIL_SEQ: AtomicU64 = AtomicU64::new(1);

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

pub(crate) fn open_exclusive_temp_in_dir(
    parent_fd: libc::c_int,
    leaf: &str,
) -> Result<(std::fs::File, std::ffi::CString), crate::types::TTZipStatus> {
    use crate::types::TTZipStatus;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transactional_commit_success() {
        let temp_dir = std::env::temp_dir().join(format!("tx_writer_commit_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let target_file = temp_dir.join("final_archive.zip");

        let mut writer = TransactionalFileWriter::create(&target_file).expect("create transactional writer");
        let temp_path = writer.temp_path().to_path_buf();

        assert!(temp_path.exists(), "Temporary file must exist during writing");
        assert!(!target_file.exists(), "Target file must not exist before commit");

        writer.write_all(b"PK\x03\x04test_archive_data").expect("write payload");
        writer.commit().expect("commit transactional file");

        assert!(target_file.exists(), "Target file must exist after commit");
        assert!(!temp_path.exists(), "Temporary file must not exist after commit");

        let content = std::fs::read(&target_file).expect("read committed file");
        assert_eq!(content, b"PK\x03\x04test_archive_data");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_transactional_drop_without_commit_rolls_back() {
        let temp_dir = std::env::temp_dir().join(format!("tx_writer_rollback_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let target_file = temp_dir.join("corrupted.zip");

        let temp_path = {
            let mut writer = TransactionalFileWriter::create(&target_file).expect("create writer");
            let path = writer.temp_path().to_path_buf();
            assert!(path.exists());
            writer.write_all(b"half_written_trash").expect("write bytes");
            path
            // Dropped here without calling commit()
        };

        assert!(!temp_path.exists(), "Temporary file must be deleted upon uncommitted drop");
        assert!(!target_file.exists(), "Target file must never have been created");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_transactional_explicit_abort() {
        let temp_dir = std::env::temp_dir().join(format!("tx_writer_abort_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let target_file = temp_dir.join("cancelled.zip");

        let mut writer = TransactionalFileWriter::create(&target_file).expect("create writer");
        let temp_path = writer.temp_path().to_path_buf();
        writer.write_all(b"cancelled_data").expect("write");
        assert!(temp_path.exists());

        writer.abort();

        assert!(!temp_path.exists(), "Temporary file must be deleted after explicit abort");
        assert!(!target_file.exists(), "Target file must not exist");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_transactional_collision_policy() {
        let temp_dir = std::env::temp_dir().join(format!("tx_writer_overwrite_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let target_file = temp_dir.join("existing.zip");

        std::fs::write(&target_file, b"old_version_data").expect("write initial file");

        let mut writer = TransactionalFileWriter::create(&target_file).expect("create writer");
        writer.write_all(b"new_overwritten_version").expect("write new payload");
        writer.commit().expect("commit overwrite");

        let content = std::fs::read(&target_file).expect("read updated file");
        assert_eq!(content, b"new_overwritten_version");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_transactional_seek_and_read_write() {
        let temp_dir = std::env::temp_dir().join(format!("tx_writer_seek_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let target_file = temp_dir.join("seek_test.bin");

        let mut writer = TransactionalFileWriter::create(&target_file).expect("create writer");
        writer.write_all(b"0123456789").expect("write initial");

        // Seek back and overwrite
        writer.seek(SeekFrom::Start(4)).expect("seek to 4");
        writer.write_all(b"ABCD").expect("write overwrite");

        // Seek to 0 and read back
        writer.seek(SeekFrom::Start(0)).expect("seek to 0");
        let mut buf = [0u8; 10];
        writer.read_exact(&mut buf).expect("read exact");
        assert_eq!(&buf, b"0123ABCD89");

        writer.commit().expect("commit");

        let content = std::fs::read(&target_file).expect("read committed");
        assert_eq!(&content, b"0123ABCD89");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
