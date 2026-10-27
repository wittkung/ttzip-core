// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Staging area management.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::AsRawFd;

use crate::fs::collision::fs_ops::{FsOps, LibcFsOps};
use crate::fs::collision::ItemKind;

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

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
    created_root: bool,
    committed: bool,
}

impl StagingArea {
    pub fn create(destination_root: &Path) -> Result<Self, std::io::Error> {
        let created_root = if !destination_root.exists() {
            fs::DirBuilder::new().recursive(true).create(destination_root)?;
            true
        } else {
            false
        };

        // .ttzip_staging_<pid>_<session_id>_<rand>
        let pid = std::process::id();
        let session_id = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst);
        let mut entropy = [0u8; 4];
        
        // SAFETY: getentropy handles buffer correctly
        let res = unsafe { libc::getentropy(entropy.as_mut_ptr() as *mut libc::c_void, entropy.len()) };
        if res != 0 {
            return Err(std::io::Error::last_os_error());
        }
        
        let rand_part = u32::from_ne_bytes(entropy);

        let name = format!(".ttzip_staging_{}_{}_{}", pid, session_id, rand_part);
        let staging_dir = destination_root.join(name);
        fs::DirBuilder::new().mode(0o700).create(&staging_dir)?;
        
        let payload = staging_dir.join("payload");
        fs::DirBuilder::new().mode(0o700).create(&payload)?;
        
        let displaced = staging_dir.join("displaced");
        fs::DirBuilder::new().mode(0o700).create(&displaced)?;

        Ok(Self {
            destination_root: destination_root.to_path_buf(),
            staging_dir,
            created_root,
            committed: false,
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
    
    pub fn abort(self) {
        // Will be cleaned up by drop
    }
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
    /// Always removes the staging tree: after a successful commit it holds only
    /// the drained payload and the displaced originals of replaced items. A
    /// destination root created by this session is removed only when nothing
    /// was committed into it.
    fn drop(&mut self) {
        crate::fs::staged_metadata::discard_staged_metadata_plan(&self.staging_dir.join("payload"));
        if let (Ok(dest), Some(name)) = (fs::File::open(&self.destination_root), self.staging_dir.file_name()) {
            let _ = LibcFsOps.remove_tree(dest.as_raw_fd(), Path::new(name));
        }
        if self.created_root && !self.committed {
            let _ = fs::remove_dir(&self.destination_root);
        }
    }
}


