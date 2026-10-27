// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Cross-platform sparse file allocation and hole-punching drivers.
//!
//! Provides zero-block deallocation and sparse extent punching:
//! - macOS APFS: `fcntl(fd, F_PUNCHHOLE)`
//! - Linux: `fallocate(fd, FALLOC_FL_PUNCH_HOLE | FALLOC_FL_KEEP_SIZE)`
//! - Windows: `DeviceIoControl(FSCTL_SET_SPARSE)` and `FSCTL_SET_ZERO_DATA`

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

#[cfg(target_os = "macos")]
pub const F_PUNCHHOLE: libc::c_int = 99;

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct fpunchhole_t {
    pub fp_flags: libc::c_uint,
    pub reserved: libc::c_uint,
    pub fp_offset: libc::off_t,
    pub fp_length: libc::off_t,
}

#[cfg(target_os = "linux")]
pub const FALLOC_FL_KEEP_SIZE: libc::c_int = 0x0001;
#[cfg(target_os = "linux")]
pub const FALLOC_FL_PUNCH_HOLE: libc::c_int = 0x0002;

#[cfg(windows)]
pub const FSCTL_SET_SPARSE: u32 = 0x000900c4;
#[cfg(windows)]
pub const FSCTL_SET_ZERO_DATA: u32 = 0x000980c8;

#[cfg(windows)]
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct FILE_ZERO_DATA_INFORMATION {
    pub file_offset: i64,
    pub beyond_final_zero: i64,
}

/// Allocates a sparse file at `path` with a logical size of `len` bytes.
///
/// On APFS (macOS) and modern Linux filesystems (ext4, XFS, Btrfs), preallocating length
/// without writing data only creates metadata entries, leaving physical disk blocks unallocated.
/// On Windows, explicitly flags the file as sparse via `FSCTL_SET_SPARSE`.
pub fn allocate_sparse_file(path: &Path, len: u64) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;

    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        let _ = set_sparse_handle(file.as_raw_handle() as *mut std::ffi::c_void);
    }

    file.set_len(len)?;
    Ok(file)
}

/// Punches a hole in an existing file across `[offset, offset + len)`.
///
/// ### APFS / POSIX Physical Storage Semantics
/// - **Logical File Size Invariant**: The logical file length (`st_size` / EOF) remains unaltered.
///   Subsequent reads over the punched range return zero bytes synthesized by the kernel.
/// - **Physical Extent Reclamation Condition**: Deallocates underlying physical storage blocks
///   back to the free pool ONLY if the extent blocks are not referenced by other hard links
///   or APFS Copy-on-Write clones (e.g. created via `clonefile(2)`). If shared references exist,
///   hole-punching merely unmaps the extent from this inode without reclaiming physical disk blocks.
pub fn punch_hole(file: &File, offset: u64, len: u64) -> io::Result<()> {
    if len == 0 {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        use std::os::unix::io::AsRawFd;
        let punch = fpunchhole_t {
            fp_flags: 0,
            reserved: 0,
            fp_offset: offset as libc::off_t,
            fp_length: len as libc::off_t,
        };
        let ret = unsafe { libc::fcntl(file.as_raw_fd(), F_PUNCHHOLE, &punch) };
        if ret == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    #[cfg(target_os = "linux")]
    {
        use std::os::unix::io::AsRawFd;
        let ret = unsafe {
            libc::fallocate(
                file.as_raw_fd(),
                FALLOC_FL_PUNCH_HOLE | FALLOC_FL_KEEP_SIZE,
                offset as libc::off_t,
                len as libc::off_t,
            )
        };
        if ret == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        let handle = file.as_raw_handle() as *mut std::ffi::c_void;
        let _ = set_sparse_handle(handle);

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

        let zero_info = FILE_ZERO_DATA_INFORMATION {
            file_offset: offset as i64,
            beyond_final_zero: (offset + len) as i64,
        };

        let mut bytes_returned: u32 = 0;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                FSCTL_SET_ZERO_DATA,
                &zero_info as *const _ as *mut std::ffi::c_void,
                std::mem::size_of::<FILE_ZERO_DATA_INFORMATION>() as u32,
                std::ptr::null_mut(),
                0,
                &mut bytes_returned,
                std::ptr::null_mut(),
            )
        };

        if ok != 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    {
        let _ = (file, offset, len);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Sparse file hole-punching is unsupported on this platform",
        ))
    }
}

/// Attempts to punch a hole; returns `Ok(false)` if the underlying filesystem does not support hole punching.
pub fn try_punch_hole(file: &File, offset: u64, len: u64) -> io::Result<bool> {
    match punch_hole(file, offset, len) {
        Ok(()) => Ok(true),
        Err(err) => {
            if let Some(code) = err.raw_os_error() {
                #[cfg(unix)]
                if code == libc::ENOTSUP
                    || code == libc::EOPNOTSUPP
                    || code == libc::EINVAL
                    || code == libc::ENOSYS
                {
                    return Ok(false);
                }

                #[cfg(windows)]
                if code == 50 || code == 1 || code == 87 {
                    return Ok(false);
                }
            }
            if err.kind() == io::ErrorKind::Unsupported {
                return Ok(false);
            }
            Err(err)
        }
    }
}

/// Marks a Windows file handle as sparse via `FSCTL_SET_SPARSE`.
#[cfg(windows)]
pub fn set_sparse_handle(handle: *mut std::ffi::c_void) -> io::Result<()> {
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

    let mut bytes_returned: u32 = 0;
    let ok = unsafe {
        DeviceIoControl(
            handle,
            FSCTL_SET_SPARSE,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
            &mut bytes_returned,
            std::ptr::null_mut(),
        )
    };

    if ok != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Returns the physical disk allocation size in bytes (via POSIX `st_blocks * 512`).
pub fn physical_allocated_bytes(file: &File) -> io::Result<u64> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let meta = file.metadata()?;
        Ok(meta.blocks().saturating_mul(512))
    }

    #[cfg(windows)]
    {
        // On Windows, fallback to logical file length
        let meta = file.metadata()?;
        Ok(meta.len())
    }

    #[cfg(not(any(unix, windows)))]
    {
        let meta = file.metadata()?;
        Ok(meta.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_sparse_file_allocation() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("sparse_alloc.bin");
        let file = allocate_sparse_file(&path, 1024 * 1024).unwrap();
        assert_eq!(file.metadata().unwrap().len(), 1024 * 1024);

        // On APFS, physical allocated blocks for an empty sparse file are 0 or minimal metadata
        #[cfg(target_os = "macos")]
        {
            let physical = physical_allocated_bytes(&file).unwrap();
            assert!(physical < 1024 * 1024);
        }
    }

    #[test]
    fn test_punch_hole_lifecycle() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("punch_hole_target.bin");
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();

        // Write 64KB of data (4 x 16KB blocks)
        let data = vec![0xFFu8; 65536];
        file.write_all(&data).unwrap();
        file.flush().unwrap();

        // Punch a hole in the middle (16KB..48KB = 32KB)
        let punch_res = try_punch_hole(&file, 16384, 32768);
        assert!(punch_res.is_ok());

        // Punch zero length should always succeed
        assert!(punch_hole(&file, 0, 0).is_ok());
    }
}

