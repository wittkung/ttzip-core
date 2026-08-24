// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Filesystem-aware ArchiveSource factory and medium detection.

use super::{ArchiveSource, MmapSource, StorageMedium, StreamSource};
use crate::types::TTZipStatus;
use std::ffi::CString;
use std::path::Path;

/// Proactively probes the storage medium backing `path` via statfs(2).
pub fn detect_storage_medium(path: &Path) -> StorageMedium {
    #[cfg(unix)]
    {
        let probe_target = if path.exists() {
            path
        } else if let Some(parent) = path.parent() {
            parent
        } else {
            path
        };

        if let Some(path_str) = probe_target.to_str() {
            if let Ok(c_path) = CString::new(path_str) {
                let mut sfs: libc::statfs = unsafe { std::mem::zeroed() };
                if unsafe { libc::statfs(c_path.as_ptr(), &mut sfs) } == 0 {
                    let is_local = (sfs.f_flags & (libc::MNT_LOCAL as u32)) != 0;
                    if !is_local {
                        return StorageMedium::RemoteNetwork;
                    }

                    let fstype_bytes = unsafe {
                        std::ffi::CStr::from_ptr(sfs.f_fstypename.as_ptr()).to_bytes()
                    };
                    if fstype_bytes == b"apfs" {
                        return StorageMedium::LocalFastApfs;
                    } else {
                        return StorageMedium::LocalStandard;
                    }
                }
            }
        }
    }
    StorageMedium::LocalStandard
}

/// Opens an optimal ArchiveSource instance dynamically matching the underlying filesystem.
pub fn open_archive_source(path: &Path) -> Result<Box<dyn ArchiveSource>, TTZipStatus> {
    if !path.exists() {
        return Err(TTZipStatus::ErrFileNotFound);
    }

    let medium = detect_storage_medium(path);
    match medium {
        StorageMedium::LocalFastApfs | StorageMedium::LocalStandard => {
            match MmapSource::open(path, medium) {
                Ok(mmap) => Ok(Box::new(mmap)),
                Err(_) => {
                    let stream = StreamSource::open(path, medium)?;
                    Ok(Box::new(stream))
                }
            }
        }
        StorageMedium::RemoteNetwork | StorageMedium::VirtualFilesystem => {
            let stream = StreamSource::open(path, medium)?;
            Ok(Box::new(stream))
        }
    }
}
