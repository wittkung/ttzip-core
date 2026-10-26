// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Filesystem operations for collision handling.

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::path::Path;
use libc::{attrlist, attrreference_t as attrreference, renameatx_np};
use libc::{ATTR_CMN_NAME, ATTR_VOL_CAPABILITIES, VOL_CAP_INT_RENAME_EXCL, VOL_CAP_INT_RENAME_SWAP};

use crate::fs::collision::{FailureReason, ItemKind, ItemMeta};

// macOS renameatx_np flags from <sys/stdio.h>
pub const RENAME_SWAP: u32 = 0x00000002;
pub const RENAME_EXCL: u32 = 0x00000004;
pub const RENAME_NOFOLLOW_ANY: u32 = 0x00000010;

/// Rename capabilities of the destination volume.
///
/// `VolCaps::default()` means "unknown or unsupported" and is the value to use when probing fails:
/// free-name placement then goes through [`place_exclusive`] (FR-022) and never through a plain,
/// overwriting `rename`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct VolCaps {
    pub rename_swap: bool,
    pub rename_excl: bool,
}

pub trait FsOps {
    fn openat_dir_nofollow(&self, dirfd: RawFd, path: &Path) -> Result<OwnedFd, std::io::Error>;
    fn fstatat_nofollow(&self, dirfd: RawFd, path: &Path) -> Result<Option<ItemMeta>, std::io::Error>;
    fn getattrlistat_name(&self, dirfd: RawFd, path: &Path) -> Result<Option<String>, std::io::Error>;
    fn renameatx(&self, from_fd: RawFd, from: &Path, to_fd: RawFd, to: &Path, flags: u32) -> Result<(), std::io::Error>;
    fn unlinkat(&self, dirfd: RawFd, path: &Path, is_dir: bool) -> Result<(), std::io::Error>;
    fn remove_tree(&self, dirfd: RawFd, path: &Path) -> Result<(), std::io::Error>;
    fn probe_vol_caps(&self, path: &Path) -> Result<VolCaps, std::io::Error>;

    /// `linkat(from_fd, from, to_fd, to, 0)`: creates `to` atomically and fails with `EEXIST`
    /// when the name is occupied; never replaces an existing entry.
    fn linkat(&self, from_fd: RawFd, from: &Path, to_fd: RawFd, to: &Path) -> Result<(), std::io::Error> {
        sys_linkat(from_fd, from, to_fd, to)
    }
}

#[derive(Default)]
pub struct LibcFsOps;

impl LibcFsOps {
    fn path_to_cstr(path: &Path) -> Result<CString, std::io::Error> {
        CString::new(path.as_os_str().as_bytes()).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "path contains null byte")
        })
    }
}

impl FsOps for LibcFsOps {
    fn openat_dir_nofollow(&self, dirfd: RawFd, path: &Path) -> Result<OwnedFd, std::io::Error> {
        let c_path = Self::path_to_cstr(path)?;
        let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
        
        // SAFETY: Path is null-terminated, flags are valid.
        let fd = unsafe { libc::openat(dirfd, c_path.as_ptr(), flags) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        
        // SAFETY: fd is a valid opened file descriptor.
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }

    fn fstatat_nofollow(&self, dirfd: RawFd, path: &Path) -> Result<Option<ItemMeta>, std::io::Error> {
        let c_path = Self::path_to_cstr(path)?;
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        
        // SAFETY: stat is valid memory, path is null-terminated.
        let res = unsafe { libc::fstatat(dirfd, c_path.as_ptr(), stat.as_mut_ptr(), libc::AT_SYMLINK_NOFOLLOW) };
        if res < 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::ENOENT) {
                return Ok(None);
            }
            return Err(err);
        }
        
        // SAFETY: fstatat succeeded.
        let stat = unsafe { stat.assume_init() };
        let kind = if (stat.st_mode & libc::S_IFMT) == libc::S_IFDIR {
            ItemKind::Directory
        } else if (stat.st_mode & libc::S_IFMT) == libc::S_IFLNK {
            ItemKind::Symlink
        } else {
            ItemKind::File
        };
        
        Ok(Some(ItemMeta {
            kind,
            size: stat.st_size as u64,
            mtime_unix_ns: stat.st_mtime * 1_000_000_000 + stat.st_mtime_nsec,
        }))
    }

    fn getattrlistat_name(&self, dirfd: RawFd, path: &Path) -> Result<Option<String>, std::io::Error> {
        let c_path = Self::path_to_cstr(path)?;
        let mut attr = attrlist {
            bitmapcount: libc::ATTR_BIT_MAP_COUNT,
            reserved: 0,
            commonattr: ATTR_CMN_NAME,
            volattr: 0,
            dirattr: 0,
            fileattr: 0,
            forkattr: 0,
        };
        
        #[repr(C)]
        struct NameBuf {
            length: u32,
            name_ref: attrreference,
            buf: [u8; 1024],
        }
        
        let mut buf: NameBuf = unsafe { std::mem::zeroed() };
        
        // SAFETY: passing valid attrlist and buffer.
        let res = unsafe {
            libc::getattrlistat(
                dirfd,
                c_path.as_ptr(),
                &mut attr as *mut _ as *mut libc::c_void,
                &mut buf as *mut _ as *mut libc::c_void,
                std::mem::size_of::<NameBuf>(),
                libc::FSOPT_NOFOLLOW as libc::c_ulong,
            )
        };
        
        if res < 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::ENOENT) {
                return Ok(None);
            }
            return Err(err);
        }
        
        let offset = buf.name_ref.attr_dataoffset as usize;
        let length = buf.name_ref.attr_length as usize;
        
        let buf_size = std::mem::size_of::<NameBuf>();
        let start = std::mem::size_of::<u32>().checked_add(offset)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "overflow in offset"))?;
        let end = start.checked_add(length)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "overflow in length"))?;
            
        if end > buf_size {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "attr_length exceeds buffer"));
        }
        
        if length > 0 {
            let end_idx = end - 1; // exclude null terminator
            if end_idx < start {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "length is zero after exclusion"));
            }
            // SAFETY: bounds are checked above.
            let slice = &unsafe { std::slice::from_raw_parts(&buf as *const _ as *const u8, buf_size) }[start..end_idx];
            Ok(Some(String::from_utf8_lossy(slice).into_owned()))
        } else {
            Ok(Some(String::new()))
        }
    }

    fn renameatx(&self, from_fd: RawFd, from: &Path, to_fd: RawFd, to: &Path, flags: u32) -> Result<(), std::io::Error> {
        let from_c = Self::path_to_cstr(from)?;
        let to_c = Self::path_to_cstr(to)?;
        
        // SAFETY: paths are valid C-strings.
        let res = unsafe {
            renameatx_np(
                from_fd,
                from_c.as_ptr(),
                to_fd,
                to_c.as_ptr(),
                flags,
            )
        };
        if res < 0 {
            let err = std::io::Error::last_os_error();
            return Err(err);
        }
        Ok(())
    }

    fn unlinkat(&self, dirfd: RawFd, path: &Path, is_dir: bool) -> Result<(), std::io::Error> {
        let c_path = Self::path_to_cstr(path)?;
        let flag = if is_dir { libc::AT_REMOVEDIR } else { 0 };
        // SAFETY: valid C-string path.
        let res = unsafe { libc::unlinkat(dirfd, c_path.as_ptr(), flag) };
        if res < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }

    fn remove_tree(&self, dirfd: RawFd, path: &Path) -> Result<(), std::io::Error> {
        let fd = self.openat_dir_nofollow(dirfd, path)?;
        
        // SAFETY: duplicating a valid fd
        let dup_fd = unsafe { libc::dup(fd.as_raw_fd()) };
        if dup_fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        
        // SAFETY: valid duplicated fd
        let dir = unsafe { libc::fdopendir(dup_fd) };
        if dir.is_null() {
            // SAFETY: safe to close fd on error
            unsafe { libc::close(dup_fd); }
            return Err(std::io::Error::last_os_error());
        }
        
        struct DirGuard(*mut libc::DIR);
        impl Drop for DirGuard {
            fn drop(&mut self) {
                // SAFETY: closing a valid DIR pointer
                unsafe { libc::closedir(self.0); }
            }
        }
        let _dir_guard = DirGuard(dir);
        
        loop {
            // SAFETY: resetting errno before calling readdir
            unsafe { *libc::__error() = 0; }
            // SAFETY: reading from valid DIR pointer
            let entry = unsafe { libc::readdir(dir) };
            if entry.is_null() {
                // SAFETY: checking errno
                let err = unsafe { *libc::__error() };
                if err != 0 {
                    return Err(std::io::Error::from_raw_os_error(err));
                }
                break;
            }
            
            // SAFETY: d_name is null terminated
            let name_cstr = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) };
            let name_bytes = name_cstr.to_bytes();
            if name_bytes == b"." || name_bytes == b".." {
                continue;
            }
            
            let child_path = Path::new(std::ffi::OsStr::from_bytes(name_bytes));
            
            let meta = self.fstatat_nofollow(fd.as_raw_fd(), child_path)?;
            if let Some(meta) = meta {
                if meta.kind == ItemKind::Directory {
                    self.remove_tree(fd.as_raw_fd(), child_path)?;
                } else {
                    self.unlinkat(fd.as_raw_fd(), child_path, false)?;
                }
            }
        }
        
        // Finally remove the directory itself
        self.unlinkat(dirfd, path, true)?;
        
        Ok(())
    }

    fn probe_vol_caps(&self, path: &Path) -> Result<VolCaps, std::io::Error> {
        #[repr(C)]
        struct VolCapabilitiesBuffer {
            length: u32,
            capabilities: [u32; 4],
            valid: [u32; 4],
        }

        let c_path = Self::path_to_cstr(path)?;
        let mut attr = attrlist {
            bitmapcount: libc::ATTR_BIT_MAP_COUNT,
            reserved: 0,
            commonattr: 0,
            volattr: ATTR_VOL_CAPABILITIES,
            dirattr: 0,
            fileattr: 0,
            forkattr: 0,
        };
        
        let mut buf: VolCapabilitiesBuffer = unsafe { std::mem::zeroed() };
        
        // SAFETY: valid attrlist and buffer.
        let res = unsafe {
            libc::getattrlist(
                c_path.as_ptr(),
                &mut attr as *mut _ as *mut libc::c_void,
                &mut buf as *mut _ as *mut libc::c_void,
                std::mem::size_of::<VolCapabilitiesBuffer>(),
                0,
            )
        };
        
        if res < 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::ENOTSUP) || err.raw_os_error() == Some(libc::ENOSYS) {
                return Ok(VolCaps::default());
            }
            return Err(err);
        }
        
        let mut caps = VolCaps::default();
        if (buf.valid[1] & VOL_CAP_INT_RENAME_SWAP) != 0 {
            caps.rename_swap = (buf.capabilities[1] & VOL_CAP_INT_RENAME_SWAP) != 0;
        }
        if (buf.valid[1] & VOL_CAP_INT_RENAME_EXCL) != 0 {
            caps.rename_excl = (buf.capabilities[1] & VOL_CAP_INT_RENAME_EXCL) != 0;
        }
        Ok(caps)
    }
}

pub fn map_errno(errno: i32) -> FailureReason {
    match errno {
        libc::EACCES | libc::EPERM => FailureReason::PermissionDenied,
        libc::ENAMETOOLONG => FailureReason::NameTooLong,
        libc::ENOTSUP | libc::EOPNOTSUPP | libc::ENOSYS => FailureReason::NotSupportedByVolume,
        _ => FailureReason::Io,
    }
}

fn sys_linkat(from_fd: RawFd, from: &Path, to_fd: RawFd, to: &Path) -> Result<(), std::io::Error> {
    let from_c = LibcFsOps::path_to_cstr(from)?;
    let to_c = LibcFsOps::path_to_cstr(to)?;
    // SAFETY: valid C strings; flags 0 links `from` itself and never follows a symlink at `from`.
    let res = unsafe { libc::linkat(from_fd, from_c.as_ptr(), to_fd, to_c.as_ptr(), 0) };
    if res < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Source and target of one placement from the staged payload into the destination.
#[derive(Debug, Clone, Copy)]
pub struct Placement<'a> {
    pub from_fd: RawFd,
    pub from: &'a Path,
    pub to_fd: RawFd,
    pub to: &'a Path,
}

/// Errno values meaning that an exclusive primitive is not available on this volume.
fn is_unsupported_errno(err: &std::io::Error) -> bool {
    matches!(err.raw_os_error(), Some(libc::ENOTSUP | libc::EOPNOTSUPP | libc::ENOSYS))
}

/// Errno values meaning that hard links cannot be used for placement on this volume.
fn is_link_unsupported(err: &std::io::Error) -> bool {
    is_unsupported_errno(err) || matches!(err.raw_os_error(), Some(libc::EPERM | libc::EMLINK))
}

/// Places a staged item at a free destination name (FR-022).
///
/// Succeeds only if the target name was free: an occupied name yields `EEXIST` and the existing
/// entry is never replaced. Exclusivity comes solely from a single-step atomic kernel primitive
/// (`RENAME_EXCL` when supported, or `linkat` + staged unlink for regular files). When no atomic
/// exclusive primitive is supported by the volume for `kind`, returns `ENOTSUP`
/// (`FailureReason::NotSupportedByVolume`), leaving both destination and staged source untouched
/// with zero reservation artifacts.
pub fn place_exclusive<F: FsOps + ?Sized>(
    fs: &F,
    caps: VolCaps,
    placement: Placement<'_>,
    kind: ItemKind,
) -> Result<(), std::io::Error> {
    if caps.rename_excl {
        let p = placement;
        match fs.renameatx(p.from_fd, p.from, p.to_fd, p.to, RENAME_EXCL | RENAME_NOFOLLOW_ANY) {
            Err(e) if is_unsupported_errno(&e) => {}
            result => return result,
        }
    }
    match kind {
        ItemKind::File => place_file_by_link(fs, placement),
        ItemKind::Directory | ItemKind::Symlink => {
            Err(std::io::Error::from_raw_os_error(libc::ENOTSUP))
        }
    }
}

fn place_file_by_link<F: FsOps + ?Sized>(fs: &F, p: Placement<'_>) -> Result<(), std::io::Error> {
    match fs.linkat(p.from_fd, p.from, p.to_fd, p.to) {
        Ok(()) => {
            // Staging cleanup only: the destination name is already committed.
            if let Err(e) = fs.unlinkat(p.from_fd, p.from, false) {
                log::debug!("Staging cleanup deferred after linking {}: {e}", p.to.display());
            }
            Ok(())
        }
        Err(e) if is_link_unsupported(&e) => {
            Err(std::io::Error::from_raw_os_error(libc::ENOTSUP))
        }
        Err(e) => Err(e),
    }
}
