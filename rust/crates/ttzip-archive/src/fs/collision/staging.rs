// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Staging area management.

use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::fs;
use std::os::fd::{AsRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::fs::abort_guard::{
    fd_stat_info, fstatat_dev_ino_nofollow, open_dir_nofollow, path_dev_ino_nofollow,
};
use crate::fs::collision::fs_ops::{FsOps, LibcFsOps};
use crate::fs::collision::ItemKind;

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);
static DIR_QUARANTINE_SEQ: AtomicU64 = AtomicU64::new(1);

type StagingAfterCheckHook = Box<dyn FnOnce(&Path) + Send + 'static>;
type StagingAfterQuarantineHook = Box<dyn FnOnce(&Path, &Path) + Send + 'static>;

thread_local! {
    static STAGING_AFTER_CHECK_HOOK: RefCell<Option<StagingAfterCheckHook>> =
        const { RefCell::new(None) };
    static STAGING_AFTER_QUARANTINE_HOOK: RefCell<Option<StagingAfterQuarantineHook>> =
        const { RefCell::new(None) };
}

/// Registers a one-shot hook on the current thread invoked after staging directory identity is verified and before removal.
pub fn set_staging_after_check_hook<F>(hook: F)
where
    F: FnOnce(&Path) + Send + 'static,
{
    STAGING_AFTER_CHECK_HOOK.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(hook));
    });
}

/// Registers a one-shot hook invoked after `quarantine_verify_and_rmdir` renames a directory to `.ttzip_dir_q_*` and before `fstatat` / `unlinkat(AT_REMOVEDIR)`.
pub fn set_staging_after_quarantine_hook<F>(hook: F)
where
    F: FnOnce(&Path, &Path) + Send + 'static,
{
    STAGING_AFTER_QUARANTINE_HOOK.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(hook));
    });
}

pub(crate) fn run_staging_after_check_hook(path: &Path) {
    let hook = STAGING_AFTER_CHECK_HOOK.with(|slot| slot.borrow_mut().take());
    if let Some(cb) = hook {
        cb(path);
    }
}

pub(crate) fn run_staging_after_quarantine_hook(orig_path: &Path, quarantine_path: &Path) {
    let hook = STAGING_AFTER_QUARANTINE_HOOK.with(|slot| slot.borrow_mut().take());
    if let Some(cb) = hook {
        cb(orig_path, quarantine_path);
    }
}

pub(crate) fn remove_dir_contents_by_fd(dir_fd: RawFd) -> Result<(), std::io::Error> {
    let dup_fd = unsafe { libc::dup(dir_fd) };
    if dup_fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let dir = unsafe { libc::fdopendir(dup_fd) };
    if dir.is_null() {
        unsafe {
            libc::close(dup_fd);
        }
        return Err(std::io::Error::last_os_error());
    }
    struct DirGuard(*mut libc::DIR);
    impl Drop for DirGuard {
        fn drop(&mut self) {
            unsafe {
                libc::closedir(self.0);
            }
        }
    }
    let _guard = DirGuard(dir);
    let fs_ops = LibcFsOps;
    loop {
        unsafe {
            *libc::__error() = 0;
        }
        let entry = unsafe { libc::readdir(dir) };
        if entry.is_null() {
            let err = unsafe { *libc::__error() };
            if err != 0 {
                return Err(std::io::Error::from_raw_os_error(err));
            }
            break;
        }
        let name_cstr = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
        let name_bytes = name_cstr.to_bytes();
        if name_bytes == b"." || name_bytes == b".." {
            continue;
        }
        let child_path = Path::new(std::ffi::OsStr::from_bytes(name_bytes));
        if let Some(meta) = fs_ops.fstatat_nofollow(dir_fd, child_path)? {
            if meta.kind == ItemKind::Directory {
                fs_ops.remove_tree(dir_fd, child_path)?;
            } else {
                fs_ops.unlinkat(dir_fd, child_path, false)?;
            }
        }
    }
    Ok(())
}

fn format_quarantined_rmdir_err(
    orig_dir_path: &Path,
    q_path: &Path,
    expected_parent_id: (u64, u64),
    rmdir_err: std::io::Error,
) -> std::io::Error {
    let errno = rmdir_err.raw_os_error().unwrap_or(libc::EIO);
    let msg = format!(
        "quarantined staging directory rmdir failed (outcome=quarantined_rmdir_failed, orig_path='{}', quarantine_path='{}', parent_id={:?}, os_err={errno}: {rmdir_err}); residual owned directory remains at quarantine path",
        orig_dir_path.display(),
        q_path.display(),
        expected_parent_id,
    );
    std::io::Error::new(rmdir_err.kind(), msg)
}

fn restore_quarantined_dir_sentinel(
    orig_dir_path: &Path,
    q_path: &Path,
    parent_fd: RawFd,
    expected_parent_id: (u64, u64),
    dir_name: &CStr,
    q_name: &CStr,
    observed_id: Option<(u64, u64)>,
) -> std::io::Error {
    let flags: libc::c_uint = (crate::fs::collision::fs_ops::RENAME_EXCL
        | crate::fs::collision::fs_ops::RENAME_NOFOLLOW_ANY) as libc::c_uint;
    let rc = unsafe {
        libc::renameatx_np(parent_fd, q_name.as_ptr(), parent_fd, dir_name.as_ptr(), flags)
    };
    if rc == 0 {
        let verified = match (observed_id, fstatat_dev_ino_nofollow(parent_fd, dir_name)) {
            (Some(obs), Ok((r_dev, r_ino, _))) => (r_dev, r_ino) == obs,
            _ => false,
        };
        if verified {
            return std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "staging directory replaced between identity check and rmdir (outcome=identity_mismatch_restored, restored_path='{}', quarantine_name='{}', parent_id={:?}); restored sentinel",
                    orig_dir_path.display(),
                    q_name.to_string_lossy(),
                    expected_parent_id,
                ),
            );
        }
        return std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "staging directory replaced between identity check and rmdir, but post-restore verification did not match (outcome=identity_mismatch_location_unconfirmed, orig_path='{}', quarantine_path='{}', parent_id={:?})",
                orig_dir_path.display(),
                q_path.display(),
                expected_parent_id,
            ),
        );
    }
    let restore_err = std::io::Error::last_os_error();
    let errno = restore_err.raw_os_error().unwrap_or(libc::EIO);
    if errno == libc::EEXIST {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "staging directory replaced between identity check and rmdir and original leaf was occupied during restore (outcome=identity_mismatch_restore_occupied, sentinel_preserved_at_quarantine_path='{}', occupant_preserved_at_original_path='{}', parent_id={:?}, os_err={errno}); sentinel NOT restored to original name",
                q_path.display(),
                orig_dir_path.display(),
                expected_parent_id,
            ),
        )
    } else {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "staging directory replaced between identity check and rmdir and restore rename failed (outcome=identity_mismatch_location_unconfirmed, orig_path='{}', quarantine_path='{}', parent_id={:?}, os_err={errno}: {restore_err})",
                orig_dir_path.display(),
                q_path.display(),
                expected_parent_id,
            ),
        )
    }
}

pub(crate) fn quarantine_verify_and_rmdir(
    parent_path: &Path,
    parent_fd: RawFd,
    expected_parent_id: (u64, u64),
    dir_path: &Path,
    dir_name: &CStr,
    expected_dir_id: (u64, u64),
) -> Result<(), std::io::Error> {
    let pid = std::process::id();
    let seq = DIR_QUARANTINE_SEQ.fetch_add(1, Ordering::Relaxed);
    let q_str = format!(".ttzip_dir_q_{pid}_{seq}");
    let q_name = CString::new(q_str.as_bytes()).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid quarantine name")
    })?;
    let q_path = parent_path.join(&q_str);
    let flags: libc::c_uint = (crate::fs::collision::fs_ops::RENAME_EXCL
        | crate::fs::collision::fs_ops::RENAME_NOFOLLOW_ANY) as libc::c_uint;
    if unsafe {
        libc::renameatx_np(
            parent_fd,
            dir_name.as_ptr(),
            parent_fd,
            q_name.as_ptr(),
            flags,
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error());
    }
    run_staging_after_quarantine_hook(dir_path, &q_path);
    match fstatat_dev_ino_nofollow(parent_fd, &q_name) {
        Ok((dev, ino, mode))
            if (mode & libc::S_IFMT) == libc::S_IFDIR && (dev, ino) == expected_dir_id =>
        {
            if unsafe { libc::unlinkat(parent_fd, q_name.as_ptr(), libc::AT_REMOVEDIR) } != 0 {
                let err = std::io::Error::last_os_error();
                return Err(format_quarantined_rmdir_err(
                    dir_path,
                    &q_path,
                    expected_parent_id,
                    err,
                ));
            }
            Ok(())
        }
        other => {
            let observed = other.ok().map(|(d, i, _)| (d, i));
            Err(restore_quarantined_dir_sentinel(
                dir_path,
                &q_path,
                parent_fd,
                expected_parent_id,
                dir_name,
                &q_name,
                observed,
            ))
        }
    }
}

pub(crate) fn verify_and_remove_owned_dir(
    parent_path: &Path,
    parent_fd: RawFd,
    expected_parent_id: (u64, u64),
    dir_path: &Path,
    dir_name: &CStr,
    dir_fd: RawFd,
    expected_dir_id: (u64, u64),
) -> Result<(), std::io::Error> {
    let (p_dev, p_ino, p_mode) = path_dev_ino_nofollow(parent_path)?;
    if (p_mode & libc::S_IFMT) != libc::S_IFDIR || (p_dev, p_ino) != expected_parent_id {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "parent directory identity changed (outcome=identity_mismatch_location_unconfirmed, orig_path='{}', expected_parent_id={:?}, observed_parent_id={:?}); refusing staging directory cleanup",
                dir_path.display(),
                expected_parent_id,
                (p_dev, p_ino)
            ),
        ));
    }
    match fstatat_dev_ino_nofollow(parent_fd, dir_name) {
        Ok((dev, ino, mode)) => {
            if (mode & libc::S_IFMT) != libc::S_IFDIR || (dev, ino) != expected_dir_id {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "staging directory path '{}' replaced by another inode (expected={:?}, observed={:?}); refusing destructive cleanup",
                        dir_path.display(),
                        expected_dir_id,
                        (dev, ino)
                    ),
                ));
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if let Ok((_, _, _, nlink)) = fd_stat_info(dir_fd) {
                if nlink >= 2 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "staging directory moved away from parent and remains linked",
                    ));
                }
            }
            return Ok(());
        }
        Err(e) => return Err(e),
    }
    remove_dir_contents_by_fd(dir_fd)?;
    run_staging_after_check_hook(dir_path);
    quarantine_verify_and_rmdir(
        parent_path,
        parent_fd,
        expected_parent_id,
        dir_path,
        dir_name,
        expected_dir_id,
    )
}

pub struct StagingRoot(pub PathBuf);

impl AsRef<Path> for StagingRoot {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl StagingRoot {
    pub fn path(&self) -> &Path {
        &self.0
    }
}

pub struct StagingArea {
    pub destination_root: PathBuf,
    pub staging_dir: PathBuf,
    parent_fd: OwnedFd,
    parent_id: (u64, u64),
    staging_dir_fd: OwnedFd,
    staging_dir_id: (u64, u64),
    staging_name: CString,
    created_root: bool,
    committed: bool,
    attempted_cleanup: bool,
}

impl StagingArea {
    pub fn create(destination_root: &Path) -> Result<Self, std::io::Error> {
        let created_root = if !destination_root.exists() {
            fs::DirBuilder::new().recursive(true).create(destination_root)?;
            true
        } else {
            false
        };
        let (parent_fd, parent_id) = open_dir_nofollow(destination_root)?;

        // .ttzip_staging_<pid>_<session_id>_<rand>
        let pid = std::process::id();
        let session_id = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst);
        let mut entropy = [0u8; 4];

        // SAFETY: getentropy handles buffer correctly
        let res =
            unsafe { libc::getentropy(entropy.as_mut_ptr() as *mut libc::c_void, entropy.len()) };
        if res != 0 {
            return Err(std::io::Error::last_os_error());
        }

        let rand_part = u32::from_ne_bytes(entropy);
        let name = format!(".ttzip_staging_{}_{}_{}", pid, session_id, rand_part);
        let staging_name = CString::new(name.as_bytes()).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid staging name")
        })?;
        let staging_dir = destination_root.join(&name);
        if unsafe { libc::mkdirat(parent_fd.as_raw_fd(), staging_name.as_ptr(), 0o700) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        let (staging_dir_fd, staging_dir_id) = open_dir_nofollow(&staging_dir)?;

        let payload = staging_dir.join("payload");
        fs::DirBuilder::new().mode(0o700).create(&payload)?;

        let displaced = staging_dir.join("displaced");
        fs::DirBuilder::new().mode(0o700).create(&displaced)?;

        Ok(Self {
            destination_root: destination_root.to_path_buf(),
            staging_dir,
            parent_fd,
            parent_id,
            staging_dir_fd,
            staging_dir_id,
            staging_name,
            created_root,
            committed: false,
            attempted_cleanup: false,
        })
    }

    pub fn payload_dir(&self) -> StagingRoot {
        StagingRoot(self.staging_dir.join("payload"))
    }

    pub fn displaced_dir(&self) -> PathBuf {
        self.staging_dir.join("displaced")
    }

    pub fn commit(mut self) {
        self.committed = true;
    }

    /// Marks the staging area committed and explicitly removes the staging tree,
    /// returning any post-commit cleanup error while disarming `Drop`.
    pub fn commit_and_cleanup_checked(mut self) -> Result<(), std::io::Error> {
        self.committed = true;
        self.cleanup_internal()
    }

    pub fn abort(self) {
        let _ = self.abort_checked();
    }

    /// Explicitly removes the staging tree and checks for cleanup errors.
    ///
    /// Verifies parent directory and staging directory `(st_dev, st_ino)` ownership before
    /// deleting children via `staging_dir_fd`, disarms `Drop` from repeating a failed
    /// removal, and returns an `io::Error` if the staging tree could not be removed.
    pub fn abort_checked(mut self) -> Result<(), std::io::Error> {
        self.cleanup_internal()
    }

    fn cleanup_internal(&mut self) -> Result<(), std::io::Error> {
        self.attempted_cleanup = true;
        crate::fs::staged_metadata::discard_staged_metadata_plan(&self.staging_dir.join("payload"));
        verify_and_remove_owned_dir(
            &self.destination_root,
            self.parent_fd.as_raw_fd(),
            self.parent_id,
            &self.staging_dir,
            &self.staging_name,
            self.staging_dir_fd.as_raw_fd(),
            self.staging_dir_id,
        )?;
        if self.created_root && !self.committed {
            remove_created_root_if_empty(&self.destination_root, self.parent_id)?;
        }
        Ok(())
    }
}

fn remove_created_root_if_empty(
    destination_root: &Path,
    expected_parent_id: (u64, u64),
) -> Result<(), std::io::Error> {
    let (dev, ino, mode) = path_dev_ino_nofollow(destination_root)?;
    if (mode & libc::S_IFMT) != libc::S_IFDIR || (dev, ino) != expected_parent_id {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "destination_root identity changed; refusing root removal",
        ));
    }
    match fs::remove_dir(destination_root) {
        Ok(()) => Ok(()),
        Err(e) if is_ignorable_root_remove_err(&e) => Ok(()),
        Err(e) => Err(e),
    }
}

fn is_ignorable_root_remove_err(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
    ) || matches!(e.raw_os_error(), Some(libc::ENOTEMPTY | libc::EEXIST))
}

pub fn sweep_stale(destination_root: &Path) -> u32 {
    let mut swept = 0;
    
    // SAFETY: destination_root to c string
    let dest_c = match std::ffi::CString::new(destination_root.as_os_str().as_bytes()) {
        Ok(c) => c,
        Err(_) => return 0,
    };
    
    // SAFETY: openat with NOFOLLOW and DIRECTORY
    let dirfd = unsafe { libc::open(dest_c.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) };
    if dirfd < 0 {
        return 0;
    }
    
    if let Ok(entries) = fs::read_dir(destination_root) {
        let fs_ops = LibcFsOps;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(suffix) = name.strip_prefix(".ttzip_staging_") {
                let parts: Vec<&str> = suffix.split('_').collect();
                if parts.len() == 3 {
                    if let (Ok(pid), Ok(_ses), Ok(_rand)) = (parts[0].parse::<i32>(), parts[1].parse::<u64>(), parts[2].parse::<u32>()) {
                        if pid > 0 {
                            // Check liveness: dead ONLY if return is -1 AND errno == libc::ESRCH
                            // SAFETY: kill 0 is safe
                            let is_dead = unsafe {
                                *libc::__error() = 0;
                                let res = libc::kill(pid, 0);
                                let err = *libc::__error();
                                res == -1 && err == libc::ESRCH
                            };
                            
                            if is_dead {
                                // Double check it is a directory by using fstatat
                                let entry_name = entry.file_name();
                                let name_path = Path::new(entry_name.as_os_str());
                                if let Ok(Some(meta)) = fs_ops.fstatat_nofollow(dirfd, name_path) {
                                    if meta.kind == ItemKind::Directory {
                                        let _ = fs_ops.remove_tree(dirfd, name_path);
                                        swept += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    // SAFETY: closing valid fd
    unsafe { libc::close(dirfd); }
    
    swept
}

impl Drop for StagingArea {
    /// Always removes the staging tree via ownership-verified descriptors when not already cleaned up.
    fn drop(&mut self) {
        if !self.attempted_cleanup {
            let _ = self.cleanup_internal();
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
        jail: &crate::fs::dir_jail::DirJail,
        dst_rel: &str,
        src_rel: &str,
    ) -> Result<(), crate::types::TTZipStatus> {
        use crate::types::TTZipStatus;
        let dst_comps = crate::fs::dir_jail::DirJail::extract_clean_components(dst_rel)?;
        let src_comps = crate::fs::dir_jail::DirJail::extract_clean_components(src_rel)?;
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
        jail: &crate::fs::dir_jail::DirJail,
        entry_comps: &[String],
        hardlink_target_rel: Option<&str>,
    ) -> Result<(), crate::types::TTZipStatus> {
        self.invalidate_destination(entry_comps);
        if let Some(src_rel) = hardlink_target_rel {
            let src_comps = crate::fs::dir_jail::DirJail::extract_clean_components(src_rel)?;
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
        jail: &crate::fs::dir_jail::DirJail,
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
        jail: &crate::fs::dir_jail::DirJail,
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


