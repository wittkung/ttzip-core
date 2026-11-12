// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Hardened security extraction sandbox and file descriptor pinning engine.

use super::deferred::DeferredSecureEntry;
use super::flags::SecurityFlags;
use crate::types::TTZipStatus;
use std::ffi::CString;
use std::fs::{self, File};
use std::path::{Component, Path, PathBuf};

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::DirBuilderExt;
#[cfg(unix)]
use std::os::unix::io::{FromRawFd, RawFd};

/// Hardened security extraction sandbox and file descriptor pinning engine.
#[derive(Debug)]
pub struct SecurePathExtractor {
    sandbox_root: PathBuf,
    flags: SecurityFlags,
    deferred_entries: Vec<DeferredSecureEntry>,
    #[cfg(unix)]
    root_fd: Option<RawFd>,
}

impl SecurePathExtractor {
    /// Initializes a new secure extractor anchored to the specified sandbox directory.
    ///
    /// # Errors
    /// Returns `ErrSecurityViolation` or `ErrInvalidParam` if the sandbox path is invalid.
    pub fn new(sandbox_root: impl AsRef<Path>, flags: SecurityFlags) -> Result<Self, TTZipStatus> {
        let sandbox_path = sandbox_root.as_ref().to_path_buf();
        if !sandbox_path.exists() {
            fs::create_dir_all(&sandbox_path).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
        }

        let canonical_root = fs::canonicalize(&sandbox_path).map_err(|_| TTZipStatus::ErrInvalidParam)?;

        #[cfg(unix)]
        {
            let c_path = CString::new(canonical_root.as_os_str().as_bytes())
                .map_err(|_| TTZipStatus::ErrInvalidParam)?;
            let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) };
            if fd < 0 {
                return Err(TTZipStatus::ErrOpenFailed);
            }
            Ok(Self {
                sandbox_root: canonical_root,
                flags,
                deferred_entries: Vec::new(),
                root_fd: Some(fd),
            })
        }

        #[cfg(not(unix))]
        {
            Ok(Self {
                sandbox_root: canonical_root,
                flags,
                deferred_entries: Vec::new(),
            })
        }
    }

    /// Returns a reference to the sandbox root path.
    #[inline]
    #[must_use]
    pub fn sandbox_root(&self) -> &Path {
        &self.sandbox_root
    }

    /// Returns the current active security flags.
    #[inline]
    #[must_use]
    pub fn flags(&self) -> SecurityFlags {
        self.flags
    }

    /// Updates active security flags.
    #[inline]
    pub fn set_flags(&mut self, flags: SecurityFlags) {
        self.flags = flags;
    }

    /// Returns the number of deferred metadata entries awaiting application.
    #[inline]
    #[must_use]
    pub fn deferred_count(&self) -> usize {
        self.deferred_entries.len()
    }

    /// Sanitizes an entry path and strictly validates it against the active security flags.
    ///
    /// Returns the sanitized relative `PathBuf` within the sandbox.
    ///
    /// # Errors
    /// Returns `ErrSecurityViolation` if the path attempts traversal, escapes sandbox boundaries,
    /// or violates enabled security flags.
    pub fn sanitize_and_validate_path(&self, raw_path: &str) -> Result<PathBuf, TTZipStatus> {
        self.validate_raw_path_prefix(raw_path)?;

        let normalized = raw_path.replace('\\', "/");
        let path_obj = Path::new(&normalized);
        let mut segments: Vec<String> = Vec::with_capacity(8);

        for comp in path_obj.components() {
            match comp {
                Component::Normal(c) => {
                    let s = c.to_string_lossy();
                    self.validate_normal_component(&s)?;
                    segments.push(s.into_owned());
                }
                Component::CurDir => {}
                Component::ParentDir => {
                    if self.flags.contains(SecurityFlags::SECURE_NODOTDOT) || segments.is_empty() {
                        return Err(TTZipStatus::ErrSecurityViolation);
                    }
                    segments.pop();
                }
                Component::RootDir | Component::Prefix(_) => {
                    if self.flags.contains(SecurityFlags::SECURE_NOABSOLUTEPATHS) {
                        return Err(TTZipStatus::ErrSecurityViolation);
                    }
                }
            }
        }

        if segments.is_empty() {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        Ok(segments.into_iter().collect())
    }

    fn validate_raw_path_prefix(&self, raw_path: &str) -> Result<(), TTZipStatus> {
        if raw_path.is_empty() || raw_path.contains('\0') || raw_path.contains("://") {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        if self.flags.contains(SecurityFlags::SECURE_NOABSOLUTEPATHS) && has_absolute_prefix(raw_path) {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        Ok(())
    }

    fn validate_normal_component(&self, s: &str) -> Result<(), TTZipStatus> {
        if !self.flags.contains(SecurityFlags::SECURE_NODOTDOT) {
            return Ok(());
        }
        if s == ".." || s.contains("..") || s.chars().all(|ch| ch == '.') || has_drive_letter_prefix(s) {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        Ok(())
    }

    /// Validates that no intermediate directory in `rel_path` is a symlink.
    ///
    /// # Errors
    /// Returns `ErrSecurityViolation` if any intermediate ancestor is a symlink pointing
    /// outside the sandbox or if symlink traversal is detected.
    pub fn validate_intermediate_path(&self, rel_path: &Path) -> Result<(), TTZipStatus> {
        if !self.flags.contains(SecurityFlags::SECURE_SYMLINKS) {
            return Ok(());
        }

        let parent = match rel_path.parent() {
            Some(p) if p != Path::new("") => p,
            _ => return Ok(()),
        };

        #[cfg(unix)]
        {
            let mut current = self.sandbox_root.clone();
            for comp in parent.components() {
                if let Component::Normal(seg) = comp {
                    current.push(seg);
                    let c_path = match CString::new(current.as_os_str().as_bytes()) {
                        Ok(c) => c,
                        Err(_) => return Err(TTZipStatus::ErrInvalidParam),
                    };

                    unsafe {
                        let mut st: libc::stat = std::mem::zeroed();
                        if libc::lstat(c_path.as_ptr(), &mut st) == 0
                            && (st.st_mode & libc::S_IFMT) == libc::S_IFLNK
                        {
                            return Err(TTZipStatus::ErrSecurityViolation);
                        }
                    }
                }
            }
            Ok(())
        }

        #[cfg(not(unix))]
        {
            let mut current = self.sandbox_root.clone();
            for comp in parent.components() {
                if let Component::Normal(seg) = comp {
                    current.push(seg);
                    if let Ok(meta) = fs::symlink_metadata(&current) {
                        if meta.file_type().is_symlink() {
                            return Err(TTZipStatus::ErrSecurityViolation);
                        }
                    }
                }
            }
            Ok(())
        }
    }

    /// Securely creates directory hierarchy with restricted `0700` POSIX mode,
    /// registering the directory for deferred metadata application.
    ///
    /// # Errors
    /// Returns `ErrSecurityViolation` if path validation fails, or `ErrExtractionFailed` on I/O error.
    pub fn create_dir_all_secure(
        &mut self,
        rel_path: &Path,
        mode: u32,
        mtime_secs: i64,
        mtime_nanos: u32,
    ) -> Result<(), TTZipStatus> {
        self.validate_intermediate_path(rel_path)?;

        let full_target = self.sandbox_root.join(rel_path);

        #[cfg(unix)]
        {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            builder.mode(0o700); // Stage 1: Temporary restricted permissions

            builder.create(&full_target).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
        }

        #[cfg(not(unix))]
        {
            fs::create_dir_all(&full_target).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
        }

        if self.flags.contains(SecurityFlags::RESTORE_PERMISSIONS) {
            self.deferred_entries.push(DeferredSecureEntry {
                rel_path: rel_path.to_path_buf(),
                mode,
                mtime_epoch_secs: mtime_secs,
                mtime_nanos,
                is_directory: true,
            });
        }

        Ok(())
    }

    /// Securely creates and opens a destination file with descriptor pinning (`O_NOFOLLOW`).
    ///
    /// If `SECURE_UNLINK_FIRST` is set, existing files or symlinks at destination are atomically unlinked first.
    ///
    /// # Errors
    /// Returns `ErrSecurityViolation` on security violation or `ErrExtractionFailed` on I/O error.
    pub fn create_file_secure(
        &mut self,
        rel_path: &Path,
        mode: u32,
        mtime_secs: i64,
        mtime_nanos: u32,
        overwrite: bool,
    ) -> Result<File, TTZipStatus> {
        self.validate_intermediate_path(rel_path)?;

        let full_target = self.sandbox_root.join(rel_path);
        ensure_parent_dir_exists(&full_target)?;

        let unlink_first = self.flags.contains(SecurityFlags::SECURE_UNLINK_FIRST) || overwrite;
        let file = open_pinned_file(&full_target, overwrite, unlink_first)?;

        if self.flags.contains(SecurityFlags::RESTORE_PERMISSIONS) {
            self.deferred_entries.push(DeferredSecureEntry {
                rel_path: rel_path.to_path_buf(),
                mode,
                mtime_epoch_secs: mtime_secs,
                mtime_nanos,
                is_directory: false,
            });
        }

        Ok(file)
    }

    /// Securely creates a symbolic link within the sandbox after validating its target.
    ///
    /// # Errors
    /// Returns `ErrSecurityViolation` if target points outside sandbox and `SECURE_SYMLINKS` is active.
    pub fn create_symlink_secure(&mut self, rel_path: &Path, symlink_target: &str) -> Result<(), TTZipStatus> {
        self.validate_intermediate_path(rel_path)?;

        if self.flags.contains(SecurityFlags::SECURE_SYMLINKS) {
            validate_symlink_target_within_sandbox(rel_path, symlink_target)?;
        }

        let full_target = self.sandbox_root.join(rel_path);
        ensure_parent_dir_exists(&full_target)?;

        #[cfg(unix)]
        {
            let c_link = CString::new(full_target.as_os_str().as_bytes())
                .map_err(|_| TTZipStatus::ErrInvalidParam)?;
            let c_target = CString::new(symlink_target.as_bytes())
                .map_err(|_| TTZipStatus::ErrInvalidParam)?;

            if self.flags.contains(SecurityFlags::SECURE_UNLINK_FIRST) {
                unsafe {
                    libc::unlink(c_link.as_ptr());
                }
            }

            let ret = unsafe { libc::symlink(c_target.as_ptr(), c_link.as_ptr()) };
            if ret != 0 {
                return Err(TTZipStatus::ErrExtractionFailed);
            }
            Ok(())
        }

        #[cfg(not(unix))]
        {
            let _ = symlink_target;
            Err(TTZipStatus::ErrUnsupportedFeature)
        }
    }

    /// Stage 2: Post-extraction deferred metadata application in Bottom-Up order.
    ///
    /// Directories are sorted deepest-first (descending depth) to prevent modifying
    /// parent permissions before child entries have their metadata and timestamps applied.
    ///
    /// # Errors
    /// Returns `ErrSecurityViolation` or `ErrExtractionFailed` if metadata application fails.
    pub fn apply_deferred_metadata(&mut self) -> Result<(), TTZipStatus> {
        if !self.flags.contains(SecurityFlags::RESTORE_PERMISSIONS) {
            self.deferred_entries.clear();
            return Ok(());
        }

        let mut files = Vec::new();
        let mut dirs = Vec::new();

        for entry in self.deferred_entries.drain(..) {
            if entry.is_directory {
                dirs.push(entry);
            } else {
                files.push(entry);
            }
        }

        // 1. Apply file metadata first
        for file_entry in files {
            self.apply_single_entry(&file_entry)?;
        }

        // 2. Sort directories Bottom-Up (descending depth, deepest first)
        dirs.sort_by(|a, b| b.depth().cmp(&a.depth()).then_with(|| b.rel_path.cmp(&a.rel_path)));

        // 3. Apply directory metadata in bottom-up order
        for dir_entry in dirs {
            self.apply_single_entry(&dir_entry)?;
        }

        Ok(())
    }

    fn apply_single_entry(&self, entry: &DeferredSecureEntry) -> Result<(), TTZipStatus> {
        let full_path = self.sandbox_root.join(&entry.rel_path);

        #[cfg(unix)]
        {
            let c_path = match CString::new(full_path.as_os_str().as_bytes()) {
                Ok(c) => c,
                Err(_) => return Err(TTZipStatus::ErrInvalidParam),
            };

            unsafe {
                // Verify entry using lstat to ensure it has not been replaced with a symlink (TOCTOU defense)
                let mut st: libc::stat = std::mem::zeroed();
                if libc::lstat(c_path.as_ptr(), &mut st) != 0 {
                    return Err(TTZipStatus::ErrFileNotFound);
                }

                if (st.st_mode & libc::S_IFMT) == libc::S_IFLNK {
                    return Err(TTZipStatus::ErrSecurityViolation);
                }

                // Restore POSIX permissions
                if entry.mode != 0 {
                    let target_mode = (entry.mode & 0o7777) as libc::mode_t;
                    if libc::chmod(c_path.as_ptr(), target_mode) != 0 {
                        return Err(TTZipStatus::ErrExtractionFailed);
                    }
                }

                // Restore nanosecond timestamps
                if entry.mtime_epoch_secs > 0 {
                    let times = [
                        libc::timespec {
                            tv_sec: entry.mtime_epoch_secs as libc::time_t,
                            tv_nsec: entry.mtime_nanos as libc::c_long,
                        },
                        libc::timespec {
                            tv_sec: entry.mtime_epoch_secs as libc::time_t,
                            tv_nsec: entry.mtime_nanos as libc::c_long,
                        },
                    ];
                    libc::utimensat(libc::AT_FDCWD, c_path.as_ptr(), times.as_ptr(), libc::AT_SYMLINK_NOFOLLOW);
                }
            }
            Ok(())
        }

        #[cfg(not(unix))]
        {
            let _ = full_path;
            Ok(())
        }
    }
}

impl Drop for SecurePathExtractor {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(fd) = self.root_fd.take() {
            unsafe {
                libc::close(fd);
            }
        }
    }
}

#[inline]
fn has_drive_letter_prefix(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

#[inline]
fn has_absolute_prefix(raw_path: &str) -> bool {
    raw_path.starts_with('/')
        || raw_path.starts_with('\\')
        || raw_path.starts_with(r"\\")
        || raw_path.starts_with("//")
        || has_drive_letter_prefix(raw_path)
}

fn ensure_parent_dir_exists(full_target: &Path) -> Result<(), TTZipStatus> {
    if let Some(parent) = full_target.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
        }
    }
    Ok(())
}

fn open_pinned_file(full_target: &Path, overwrite: bool, unlink_first: bool) -> Result<File, TTZipStatus> {
    #[cfg(unix)]
    {
        let c_path = CString::new(full_target.as_os_str().as_bytes())
            .map_err(|_| TTZipStatus::ErrInvalidParam)?;

        if unlink_first {
            unsafe {
                libc::unlink(c_path.as_ptr());
            }
        }

        let mode_flag = if overwrite { libc::O_TRUNC } else { libc::O_EXCL };
        let open_flags = libc::O_WRONLY | libc::O_CREAT | libc::O_NOFOLLOW | libc::O_CLOEXEC | mode_flag;

        let fd = unsafe { libc::open(c_path.as_ptr(), open_flags, 0o600) };
        if fd < 0 {
            return Err(TTZipStatus::ErrExtractionFailed);
        }

        Ok(unsafe { File::from_raw_fd(fd) })
    }

    #[cfg(not(unix))]
    {
        if unlink_first && full_target.exists() {
            let _ = fs::remove_file(full_target);
        }

        let mut options = fs::OpenOptions::new();
        options.write(true).create(true);
        if overwrite {
            options.truncate(true);
        } else {
            options.create_new(true);
        }

        options.open(full_target).map_err(|_| TTZipStatus::ErrExtractionFailed)
    }
}

fn validate_symlink_target_within_sandbox(rel_path: &Path, symlink_target: &str) -> Result<(), TTZipStatus> {
    if has_absolute_prefix(symlink_target) {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    let mut depth = rel_path.components().count().saturating_sub(1);
    for comp in Path::new(symlink_target).components() {
        match comp {
            Component::ParentDir => {
                if depth == 0 {
                    return Err(TTZipStatus::ErrSecurityViolation);
                }
                depth -= 1;
            }
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) => return Err(TTZipStatus::ErrSecurityViolation),
        }
    }
    Ok(())
}
