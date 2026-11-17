// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

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
                    return classify_storage_medium(&sfs);
                }
            }
        }
    }
    StorageMedium::LocalStandard
}

#[cfg(unix)]
fn classify_storage_medium(stat: &libc::statfs) -> StorageMedium {
    if (stat.f_flags & (libc::MNT_LOCAL as u32)) == 0 {
        return StorageMedium::RemoteNetwork;
    }
    // Successful statfs and fstatfs fill the NUL-terminated filesystem name.
    let fs_type = unsafe { std::ffi::CStr::from_ptr(stat.f_fstypename.as_ptr()) }.to_bytes();
    if fs_type == b"apfs" {
        StorageMedium::LocalFastApfs
    } else {
        StorageMedium::LocalStandard
    }
}

/// Opens one regular file on a local volume for a bounded preview.
pub fn open_local_regular_archive_mmap(path: &Path) -> Result<MmapSource, TTZipStatus> {
    try_open_local_regular_archive_mmap(path)?.ok_or(TTZipStatus::ErrUnsupportedFeature)
}

/// A nonlocal regular file has no local preview mapping.
#[cfg(unix)]
pub fn try_open_local_regular_archive_mmap(path: &Path) -> Result<Option<MmapSource>, TTZipStatus> {
    use std::fs::OpenOptions;
    use std::io::ErrorKind;
    use std::os::{fd::AsRawFd, unix::fs::OpenOptionsExt};

    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY)
        .open(path)
        .map_err(|error| match error.kind() {
            ErrorKind::NotFound => TTZipStatus::ErrFileNotFound,
            _ => TTZipStatus::ErrOpenFailed,
        })?;
    if !file
        .metadata()
        .map_err(|_| TTZipStatus::ErrOpenFailed)?
        .is_file()
    {
        return Err(TTZipStatus::ErrUnsupportedFeature);
    }
    let mut stat = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // The owned descriptor stays live and fstatfs initializes stat only on success.
    if unsafe { libc::fstatfs(file.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
        return Err(TTZipStatus::ErrOpenFailed);
    }
    let medium = classify_storage_medium(&unsafe { stat.assume_init() });
    match medium {
        StorageMedium::LocalFastApfs | StorageMedium::LocalStandard => {
            MmapSource::from_file(&file, medium).map(Some)
        }
        StorageMedium::RemoteNetwork | StorageMedium::VirtualFilesystem => Ok(None),
    }
}

#[cfg(not(unix))]
pub fn try_open_local_regular_archive_mmap(_path: &Path) -> Result<Option<MmapSource>, TTZipStatus> {
    Ok(None)
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
