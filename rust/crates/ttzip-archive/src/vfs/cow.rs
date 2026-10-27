// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Cross-platform Copy-on-Write (CoW) block cloning drivers and universal stream fallbacks.
//!
//! Provides unified hardware-accelerated block cloning on supported filesystems:
//! - macOS APFS: `fclonefileat` / `clonefile` (returns `Ok(false)` on ENOTSUP / EXDEV)
//! - Linux: `ioctl(FICLONE)` / `ioctl(FICLONERANGE)` (returns `Ok(false)` on EOPNOTSUPP / EXDEV)
//! - Windows: `FSCTL_DUPLICATE_EXTENTS_TO_FILE` for ReFS (returns `Ok(false)` on unsupported filesystems)
//!
//! If CoW is unavailable or fails due to cross-volume boundaries, callers can safely use
//! `copy_stream_fallback` or `copy_range_to_file` to ensure reliable byte-for-byte extraction.

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

#[cfg(unix)]
use std::os::unix::io::RawFd;

#[cfg(target_os = "macos")]
extern "C" {
    fn fclonefileat(
        src_fd: libc::c_int,
        dst_dirfd: libc::c_int,
        dst: *const libc::c_char,
        flags: libc::c_uint,
    ) -> libc::c_int;

    fn clonefile(
        src: *const libc::c_char,
        dst: *const libc::c_char,
        flags: libc::c_uint,
    ) -> libc::c_int;
}

/// Fallback stream copy function transferring `len` bytes from `src_fd` to `dst_fd`.
///
/// Guaranteed to reliably copy across all filesystems, network mounts, and partition boundaries.
#[cfg(unix)]
pub fn copy_stream_fallback(src_fd: RawFd, dst_fd: RawFd, len: u64) -> io::Result<u64> {
    const CHUNK_SIZE: usize = 128 * 1024;
    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut total_copied: u64 = 0;

    while total_copied < len {
        let to_read = ((len - total_copied) as usize).min(CHUNK_SIZE);
        let n = unsafe { libc::read(src_fd, buf.as_mut_ptr() as *mut libc::c_void, to_read) };
        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(err);
        }
        if n == 0 {
            break; // EOF
        }

        let bytes_read = n as usize;
        let mut written: usize = 0;
        while written < bytes_read {
            let w = unsafe {
                libc::write(
                    dst_fd,
                    buf.as_ptr().add(written) as *const libc::c_void,
                    bytes_read - written,
                )
            };
            if w < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(err);
            }
            written += w as usize;
        }

        total_copied += bytes_read as u64;
    }

    Ok(total_copied)
}

/// Attempts to perform a Copy-on-Write (CoW) clone of a source byte range into `dst_path`.
///
/// Returns `Ok(true)` if native hardware/filesystem cloning succeeded.
/// Returns `Ok(false)` on unsupported filesystems (e.g. FAT, exFAT, ext3) or across partition
/// boundaries (`EXDEV` / `ENOTSUP`), allowing transparent fallback to stream copying.
pub fn try_cow_clone_file_range(
    src_file: &File,
    src_path: Option<&Path>,
    src_offset: u64,
    dst_path: &Path,
    len: u64,
) -> io::Result<bool> {
    if len == 0 {
        // Zero-length file: create/truncate destination directly
        File::create(dst_path)?;
        return Ok(true);
    }

    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::io::AsRawFd;

        // APFS clonefile operates on whole-file boundaries.
        let total_size = src_file.metadata()?.len();
        if src_offset == 0 && len == total_size {
            let dst_c = match CString::new(dst_path.as_os_str().as_bytes()) {
                Ok(c) => c,
                Err(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Invalid destination path",
                    ))
                }
            };

            // Remove existing destination file to avoid EEXIST
            if dst_path.exists() {
                let _ = std::fs::remove_file(dst_path);
            }

            let ret = unsafe {
                fclonefileat(
                    src_file.as_raw_fd(),
                    libc::AT_FDCWD,
                    dst_c.as_ptr(),
                    0,
                )
            };

            if ret == 0 {
                return Ok(true);
            }

            // If fclonefileat failed, attempt clonefile via src_path if available
            if let Some(src_p) = src_path {
                if let Ok(src_c) = CString::new(src_p.as_os_str().as_bytes()) {
                    let ret_cf = unsafe { clonefile(src_c.as_ptr(), dst_c.as_ptr(), 0) };
                    if ret_cf == 0 {
                        return Ok(true);
                    }
                }
            }

            let err = io::Error::last_os_error();
            if let Some(code) = err.raw_os_error() {
                if code == libc::ENOTSUP
                    || code == libc::EXDEV
                    || code == libc::ENOSYS
                    || code == libc::EINVAL
                    || code == libc::EOPNOTSUPP
                {
                    return Ok(false);
                }
            }
            return Err(err);
        }

        // Subrange CoW cloning is not natively exposed by APFS fclonefileat.
        // Return Ok(false) for graceful fallback to copy_range_to_file.
        let _ = src_path;
        Ok(false)
    }

    #[cfg(target_os = "linux")]
    {
        use std::os::unix::io::AsRawFd;

        #[repr(C)]
        struct FileCloneRange {
            src_fd: i64,
            src_offset: u64,
            src_length: u64,
            dest_offset: u64,
        }

        const FICLONERANGE: libc::c_ulong = 0x4020940d;
        const FICLONE: libc::c_ulong = 0x40049409;

        let dst_file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(dst_path)?;

        let src_fd = src_file.as_raw_fd();
        let dst_fd = dst_file.as_raw_fd();

        let ret = if src_offset == 0 && len == src_file.metadata()?.len() {
            unsafe { libc::ioctl(dst_fd, FICLONE, src_fd) }
        } else {
            let range = FileCloneRange {
                src_fd: src_fd as i64,
                src_offset,
                src_length: len,
                dest_offset: 0,
            };
            unsafe { libc::ioctl(dst_fd, FICLONERANGE, &range) }
        };

        if ret == 0 {
            let _ = dst_file.set_len(len);
            return Ok(true);
        }

        let err = io::Error::last_os_error();
        if let Some(code) = err.raw_os_error() {
            if code == libc::EOPNOTSUPP
                || code == libc::ENOTSUP
                || code == libc::EXDEV
                || code == libc::EINVAL
                || code == libc::ENOSYS
                || code == libc::ENOTTY
            {
                drop(dst_file);
                let _ = std::fs::remove_file(dst_path);
                return Ok(false);
            }
        }
        Err(err)
    }

    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;

        let dst_file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(dst_path)?;

        dst_file.set_len(len)?;

        #[repr(C)]
        struct DUPLICATE_EXTENTS_DATA {
            file_handle: *mut std::ffi::c_void,
            source_file_offset: i64,
            target_file_offset: i64,
            byte_count: i64,
        }

        const FSCTL_DUPLICATE_EXTENTS_TO_FILE: u32 = 0x00098344;

        extern "system" {
            fn DeviceIoControl(
                hDevice: *mut std::ffi::c_void,
                dwIoControlCode: u32,
                lpInBuffer: *mut std::ffi::c_void,
                nInBufferSize: u32,
                lpOutBuffer: *mut std::ffi::c_void,
                nOutBufferSize: u32,
                lpBytesReturned: *mut u32,
                lpOverlapped: *mut std::ffi::c_void,
            ) -> i32;
        }

        let mut dup_data = DUPLICATE_EXTENTS_DATA {
            file_handle: src_file.as_raw_handle() as *mut std::ffi::c_void,
            source_file_offset: src_offset as i64,
            target_file_offset: 0,
            byte_count: len as i64,
        };

        let mut bytes_returned: u32 = 0;
        let ok = unsafe {
            DeviceIoControl(
                dst_file.as_raw_handle() as *mut std::ffi::c_void,
                FSCTL_DUPLICATE_EXTENTS_TO_FILE,
                &mut dup_data as *mut _ as *mut std::ffi::c_void,
                std::mem::size_of::<DUPLICATE_EXTENTS_DATA>() as u32,
                std::ptr::null_mut(),
                0,
                &mut bytes_returned,
                std::ptr::null_mut(),
            )
        };

        if ok != 0 {
            return Ok(true);
        }

        let err = io::Error::last_os_error();
        if let Some(code) = err.raw_os_error() {
            if code == 50 || code == 1 || code == 87 || code == 17 {
                drop(dst_file);
                let _ = std::fs::remove_file(dst_path);
                return Ok(false);
            }
        }
        Err(err)
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    {
        let _ = (src_file, src_path, src_offset, dst_path, len);
        Ok(false)
    }
}

/// Fallback helper: copies `len` bytes starting at `src_offset` into `dst_path` using chunked positioned reads.
pub fn copy_range_to_file(
    src_file: &File,
    src_offset: u64,
    dst_path: &Path,
    len: u64,
) -> io::Result<u64> {
    let mut dst_file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(dst_path)?;

    if len == 0 {
        return Ok(0);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        const CHUNK_SIZE: usize = 128 * 1024;
        let mut buf = vec![0u8; CHUNK_SIZE];
        let mut transferred = 0u64;

        while transferred < len {
            let to_read = ((len - transferred) as usize).min(CHUNK_SIZE);
            let n = src_file.read_at(&mut buf[..to_read], src_offset + transferred)?;
            if n == 0 {
                break;
            }
            dst_file.write_all(&buf[..n])?;
            transferred += n as u64;
        }

        dst_file.flush()?;
        Ok(transferred)
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt;
        const CHUNK_SIZE: usize = 128 * 1024;
        let mut buf = vec![0u8; CHUNK_SIZE];
        let mut transferred = 0u64;

        while transferred < len {
            let to_read = ((len - transferred) as usize).min(CHUNK_SIZE);
            let n = src_file.seek_read(&mut buf[..to_read], src_offset + transferred)?;
            if n == 0 {
                break;
            }
            dst_file.write_all(&buf[..n])?;
            transferred += n as u64;
        }

        dst_file.flush()?;
        Ok(transferred)
    }

    #[cfg(not(any(unix, windows)))]
    {
        use std::io::{Read, Seek, SeekFrom};
        let mut src_clone = src_file.try_clone()?;
        src_clone.seek(SeekFrom::Start(src_offset))?;
        let mut limited = src_clone.take(len);
        let copied = io::copy(&mut limited, &mut dst_file)?;
        dst_file.flush()?;
        Ok(copied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_copy_stream_fallback_unix() {
        let dir = tempdir().unwrap();
        let src_path = dir.path().join("src.bin");
        let dst_path = dir.path().join("dst.bin");

        let data = b"MetastudyLineCoWFallbackTestPayloadForCrossDeviceReplication1234567890";
        std::fs::write(&src_path, data).unwrap();

        let src_file = File::open(&src_path).unwrap();
        let dst_file = File::create(&dst_path).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let copied = copy_stream_fallback(
                src_file.as_raw_fd(),
                dst_file.as_raw_fd(),
                data.len() as u64,
            )
            .unwrap();
            assert_eq!(copied, data.len() as u64);

            let read_back = std::fs::read(&dst_path).unwrap();
            assert_eq!(&read_back, data);
        }
    }

    #[test]
    fn test_copy_range_to_file_slice() {
        let dir = tempdir().unwrap();
        let src_path = dir.path().join("src_slice.bin");
        let dst_path = dir.path().join("dst_slice.bin");

        let data = vec![0xABu8; 10000];
        std::fs::write(&src_path, &data).unwrap();

        let src_file = File::open(&src_path).unwrap();
        let copied = copy_range_to_file(&src_file, 1000, &dst_path, 5000).unwrap();
        assert_eq!(copied, 5000);

        let read_back = std::fs::read(&dst_path).unwrap();
        assert_eq!(read_back.len(), 5000);
        assert_eq!(&read_back[..], &data[1000..6000]);
    }

    #[test]
    fn test_try_cow_clone_zero_length() {
        let dir = tempdir().unwrap();
        let src_path = dir.path().join("zero_src.bin");
        let dst_path = dir.path().join("zero_dst.bin");
        File::create(&src_path).unwrap();

        let src_file = File::open(&src_path).unwrap();
        let ok = try_cow_clone_file_range(&src_file, Some(&src_path), 0, &dst_path, 0).unwrap();
        assert!(ok);
        assert!(dst_path.exists());
        assert_eq!(std::fs::metadata(&dst_path).unwrap().len(), 0);
    }
}

