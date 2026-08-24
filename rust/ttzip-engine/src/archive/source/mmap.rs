// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zero-copy memory mapped archive source for local filesystems using libc mmap.

use super::{ArchiveSource, StorageMedium};
use crate::types::TTZipStatus;
use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::path::Path;

struct SafeMmap {
    ptr: *const u8,
    len: usize,
}

unsafe impl Send for SafeMmap {}
unsafe impl Sync for SafeMmap {}

impl Drop for SafeMmap {
    fn drop(&mut self) {
        if !self.ptr.is_null() && self.len > 0 {
            unsafe {
                libc::munmap(self.ptr as *mut libc::c_void, self.len);
            }
        }
    }
}

/// Memory-mapped archive source for high-performance random access.
pub struct MmapSource {
    mmap: Option<SafeMmap>,
    len: u64,
    medium: StorageMedium,
}

impl MmapSource {
    /// Maps a local archive file into virtual address space.
    pub fn open(path: &Path, medium: StorageMedium) -> Result<Self, TTZipStatus> {
        let file = File::open(path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
        let meta = file.metadata().map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let len = meta.len();

        if len == 0 {
            return Ok(Self {
                mmap: None,
                len: 0,
                medium,
            });
        }

        let map_len = len as usize;
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                map_len,
                libc::PROT_READ,
                libc::MAP_PRIVATE,
                file.as_raw_fd(),
                0,
            )
        };

        if ptr == libc::MAP_FAILED {
            return Err(TTZipStatus::ErrMmapFailed);
        }

        // Advise kernel of sequential/random access pattern
        unsafe {
            libc::madvise(ptr, map_len, libc::MADV_SEQUENTIAL);
        }

        Ok(Self {
            mmap: Some(SafeMmap {
                ptr: ptr as *const u8,
                len: map_len,
            }),
            len,
            medium,
        })
    }
}

impl ArchiveSource for MmapSource {
    #[inline]
    fn as_slice(&self) -> Option<&[u8]> {
        self.mmap.as_ref().map(|m| unsafe {
            std::slice::from_raw_parts(m.ptr, m.len)
        })
    }

    #[inline]
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize, TTZipStatus> {
        if offset >= self.len || self.mmap.is_none() {
            return Ok(0);
        }
        let m = self.mmap.as_ref().unwrap();
        let start = offset as usize;
        let available = (self.len - offset) as usize;
        let to_copy = buf.len().min(available);
        unsafe {
            let src = std::slice::from_raw_parts(m.ptr.add(start), to_copy);
            buf[..to_copy].copy_from_slice(src);
        }
        Ok(to_copy)
    }

    #[inline]
    fn len(&self) -> u64 {
        self.len
    }

    #[inline]
    fn medium(&self) -> StorageMedium {
        self.medium
    }
}
