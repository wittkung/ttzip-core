// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zero-copy memory mapped archive source with 16KB/4KB page alignment for local filesystems.

use crate::archive::source::MmapAdvice;
use super::{ArchiveSource, StorageMedium};
use crate::types::TTZipStatus;
use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

static CACHED_PAGE_SIZE: AtomicUsize = AtomicUsize::new(0);

/// Returns the system virtual memory page size in bytes (e.g. 16384 on Apple Silicon, 4096 on standard x86_64).
#[inline]
pub fn get_system_page_size() -> usize {
    let cached = CACHED_PAGE_SIZE.load(Ordering::Relaxed);
    if cached != 0 {
        return cached;
    }

    #[cfg(unix)]
    let page_size = {
        let sc = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if sc > 0 {
            sc as usize
        } else {
            16384
        }
    };

    #[cfg(not(unix))]
    let page_size = 4096;

    CACHED_PAGE_SIZE.store(page_size, Ordering::Relaxed);
    page_size
}

/// RAII wrapper around a read-only POSIX `mmap` allocation with page-alignment offset tracking.
pub struct SafeMmap {
    base_ptr: *const u8,
    aligned_len: usize,
    data_ptr: *const u8,
    len: usize,
}

unsafe impl Send for SafeMmap {}
unsafe impl Sync for SafeMmap {}

impl SafeMmap {
    /// Creates a full file memory mapping.
    pub fn new_full(file: &File, len: usize) -> Result<Self, TTZipStatus> {
        Self::new_range(file, 0, len)
    }

    /// Creates a page-aligned range memory mapping.
    pub fn new_range(file: &File, offset: u64, len: usize) -> Result<Self, TTZipStatus> {
        if len == 0 {
            return Err(TTZipStatus::ErrInvalidParam);
        }

        let page_size = get_system_page_size() as u64;
        let aligned_offset = (offset / page_size) * page_size;
        let page_diff = (offset - aligned_offset) as usize;

        let aligned_len = page_diff
            .checked_add(len)
            .ok_or(TTZipStatus::ErrInvalidParam)?;

        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                aligned_len,
                libc::PROT_READ,
                libc::MAP_PRIVATE,
                file.as_raw_fd(),
                aligned_offset as libc::off_t,
            )
        };

        if ptr == libc::MAP_FAILED || ptr.is_null() {
            return Err(TTZipStatus::ErrMmapFailed);
        }

        let base_ptr = ptr as *const u8;
        let data_ptr = unsafe { base_ptr.add(page_diff) };

        Ok(Self {
            base_ptr,
            aligned_len,
            data_ptr,
            len,
        })
    }

    /// Returns the read-only borrowed slice of the mapped region.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        if self.len == 0 || self.data_ptr.is_null() {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.data_ptr, self.len) }
        }
    }

    /// Reads up to `buf.len()` bytes at `offset` relative to this mapped view.
    #[inline]
    pub fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize, TTZipStatus> {
        if offset >= self.len as u64 {
            return Ok(0);
        }
        let start = offset as usize;
        let available = self.len.saturating_sub(start);
        let to_copy = buf.len().min(available);
        if to_copy > 0 {
            unsafe {
                let src = std::slice::from_raw_parts(self.data_ptr.add(start), to_copy);
                buf[..to_copy].copy_from_slice(src);
            }
        }
        Ok(to_copy)
    }

    /// Returns the view length in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if mapped length is 0.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns pointer to the start of the user data slice.
    #[inline]
    pub fn data_ptr(&self) -> *const u8 {
        self.data_ptr
    }

    /// Returns pointer to the page-aligned base memory allocation.
    #[inline]
    pub fn base_ptr(&self) -> *const u8 {
        self.base_ptr
    }

    /// Returns total mapped length including page alignment headroom.
    #[inline]
    pub fn aligned_len(&self) -> usize {
        self.aligned_len
    }
}

impl Drop for SafeMmap {
    fn drop(&mut self) {
        if !self.base_ptr.is_null() && self.aligned_len > 0 {
            unsafe {
                libc::munmap(self.base_ptr as *mut libc::c_void, self.aligned_len);
            }
        }
    }
}

/// Zero-copy memory-mapped archive source supporting full-file and sub-range views.
pub struct MmapSource {
    mmap: Option<SafeMmap>,
    len: u64,
    medium: StorageMedium,
}

impl MmapSource {
    /// Maps a local archive file completely into virtual address space.
    pub fn open(path: &Path, medium: StorageMedium) -> Result<Self, TTZipStatus> {
        let file = File::open(path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
        Self::from_file(&file, medium)
    }

    /// Maps a specific range `[offset, offset + len)` of an archive file.
    pub fn open_range(
        path: &Path,
        offset: u64,
        len: u64,
        medium: StorageMedium,
    ) -> Result<Self, TTZipStatus> {
        let file = File::open(path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
        Self::from_file_range(&file, offset, len, medium)
    }

    /// Creates an `MmapSource` from an already opened `File`.
    pub fn from_file(file: &File, medium: StorageMedium) -> Result<Self, TTZipStatus> {
        let meta = file.metadata().map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let total_len = meta.len();

        if total_len == 0 {
            return Ok(Self {
                mmap: None,
                len: 0,
                medium,
            });
        }

        let map_len = usize::try_from(total_len).map_err(|_| TTZipStatus::ErrInvalidParam)?;
        let safe_mmap = SafeMmap::new_full(file, map_len)?;

        // Default to sequential access pattern for initial header/stream access
        unsafe {
            libc::madvise(
                safe_mmap.base_ptr() as *mut libc::c_void,
                safe_mmap.aligned_len(),
                libc::MADV_SEQUENTIAL,
            );
        }

        Ok(Self {
            mmap: Some(safe_mmap),
            len: total_len,
            medium,
        })
    }

    /// Creates an `MmapSource` for a sub-range `[offset, offset + len)` from an opened `File`.
    pub fn from_file_range(
        file: &File,
        offset: u64,
        len: u64,
        medium: StorageMedium,
    ) -> Result<Self, TTZipStatus> {
        let meta = file.metadata().map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let file_len = meta.len();

        if offset.checked_add(len).is_none_or(|end| end > file_len) {
            return Err(TTZipStatus::ErrInvalidOffset);
        }

        if len == 0 {
            return Ok(Self {
                mmap: None,
                len: 0,
                medium,
            });
        }

        let map_len = usize::try_from(len).map_err(|_| TTZipStatus::ErrInvalidParam)?;
        let safe_mmap = SafeMmap::new_range(file, offset, map_len)?;

        Ok(Self {
            mmap: Some(safe_mmap),
            len,
            medium,
        })
    }

    /// Returns raw data pointer if mapped.
    #[inline]
    pub fn raw_data_ptr(&self) -> Option<*const u8> {
        self.mmap.as_ref().map(|m| m.data_ptr())
    }

    /// Returns raw page-aligned base pointer if mapped.
    #[inline]
    pub fn raw_base_ptr(&self) -> Option<*const u8> {
        self.mmap.as_ref().map(|m| m.base_ptr())
    }

    /// Returns the page-aligned allocation length.
    #[inline]
    pub fn aligned_len(&self) -> usize {
        self.mmap.as_ref().map_or(0, |m| m.aligned_len())
    }

    /// Issues kernel virtual memory paging advice across the entire mapped region.
    pub fn advise(&self, advice: MmapAdvice) -> Result<(), TTZipStatus> {
        if let Some(ref m) = self.mmap {
            let res = unsafe {
                libc::madvise(
                    m.base_ptr() as *mut libc::c_void,
                    m.aligned_len(),
                    advice.to_libc_advice(),
                )
            };
            if res != 0 {
                return Err(TTZipStatus::ErrInvalidParam);
            }
        }
        Ok(())
    }

    /// Issues kernel virtual memory paging advice for a sub-range within this mapping.
    pub fn advise_range(&self, offset: u64, len: u64, advice: MmapAdvice) -> Result<(), TTZipStatus> {
        if let Some(ref m) = self.mmap {
            if offset.checked_add(len).is_none_or(|end| end > self.len) {
                return Err(TTZipStatus::ErrInvalidOffset);
            }
            if len == 0 {
                return Ok(());
            }

            let page_size = get_system_page_size() as u64;
            let abs_data_offset = (m.data_ptr() as usize) - (m.base_ptr() as usize);
            let raw_offset = (abs_data_offset as u64) + offset;

            let aligned_offset = (raw_offset / page_size) * page_size;
            let page_diff = (raw_offset - aligned_offset) as usize;
            let aligned_len = page_diff
                .checked_add(usize::try_from(len).map_err(|_| TTZipStatus::ErrInvalidParam)?)
                .ok_or(TTZipStatus::ErrInvalidParam)?;

            let target_ptr = unsafe { (m.base_ptr() as *mut u8).add(aligned_offset as usize) };
            let res = unsafe {
                libc::madvise(
                    target_ptr as *mut libc::c_void,
                    aligned_len,
                    advice.to_libc_advice(),
                )
            };
            if res != 0 {
                return Err(TTZipStatus::ErrInvalidParam);
            }
        }
        Ok(())
    }

    /// Returns the borrowed immutable byte slice view if mapped.
    #[inline]
    pub fn as_slice(&self) -> Option<&[u8]> {
        match self.mmap {
            Some(ref m) => Some(m.as_slice()),
            None => {
                if self.len == 0 {
                    Some(&[])
                } else {
                    None
                }
            }
        }
    }

    /// Returns the total archive length in bytes.
    #[inline]
    pub fn len(&self) -> u64 {
        self.len
    }

    /// Returns true if archive has 0 bytes.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl ArchiveSource for MmapSource {
    #[inline]
    fn as_slice(&self) -> Option<&[u8]> {
        match self.mmap {
            Some(ref m) => Some(m.as_slice()),
            None => {
                if self.len == 0 {
                    Some(&[])
                } else {
                    None
                }
            }
        }
    }

    #[inline]
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize, TTZipStatus> {
        match self.mmap {
            Some(ref m) => m.read_at(buf, offset),
            None => Ok(0),
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_system_page_size_valid() {
        let ps = get_system_page_size();
        assert!(ps == 4096 || ps == 16384 || ps == 65536);
    }

    #[test]
    fn test_mmap_empty_file() {
        let temp = NamedTempFile::new().unwrap();
        let source = MmapSource::open(temp.path(), StorageMedium::LocalFastApfs).unwrap();
        assert_eq!(source.len(), 0);
        assert!(source.is_empty());
        assert_eq!(source.as_slice(), Some(&[][..]));
        let mut buf = [0u8; 16];
        assert_eq!(source.read_at(&mut buf, 0).unwrap(), 0);
    }

    #[test]
    fn test_mmap_full_file() {
        let mut temp = NamedTempFile::new().unwrap();
        let payload: Vec<u8> = (0..32768).map(|i| (i % 251) as u8).collect();
        temp.write_all(&payload).unwrap();
        temp.flush().unwrap();

        let source = MmapSource::open(temp.path(), StorageMedium::LocalFastApfs).unwrap();
        assert_eq!(source.len(), payload.len() as u64);
        assert_eq!(source.as_slice(), Some(payload.as_slice()));

        let mut buf = [0u8; 100];
        let n = source.read_at(&mut buf, 500).unwrap();
        assert_eq!(n, 100);
        assert_eq!(&buf[..], &payload[500..600]);

        // Read at EOF
        assert_eq!(source.read_at(&mut buf, payload.len() as u64).unwrap(), 0);
    }

    #[test]
    fn test_mmap_range_page_alignment() {
        let mut temp = NamedTempFile::new().unwrap();
        let payload: Vec<u8> = (0..65536).map(|i| (i % 255) as u8).collect();
        temp.write_all(&payload).unwrap();
        temp.flush().unwrap();

        // Non-page-aligned offset and length
        let offset = 12345u64;
        let len = 7890u64;
        let source = MmapSource::open_range(temp.path(), offset, len, StorageMedium::LocalFastApfs).unwrap();
        assert_eq!(source.len(), len);

        let slice = source.as_slice().unwrap();
        assert_eq!(slice.len(), len as usize);
        assert_eq!(slice, &payload[offset as usize..(offset + len) as usize]);

        let mut buf = vec![0u8; 500];
        let n = source.read_at(&mut buf, 100).unwrap();
        assert_eq!(n, 500);
        assert_eq!(&buf[..], &payload[(offset + 100) as usize..(offset + 600) as usize]);
    }

    #[test]
    fn test_mmap_out_of_bounds_range() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"short payload").unwrap();
        temp.flush().unwrap();

        let res = MmapSource::open_range(temp.path(), 5, 100, StorageMedium::LocalFastApfs);
        assert_eq!(res.err(), Some(TTZipStatus::ErrInvalidOffset));
    }

    #[test]
    fn test_mmap_advise_operations() {
        let mut temp = NamedTempFile::new().unwrap();
        let payload = vec![0xABu8; 32768];
        temp.write_all(&payload).unwrap();
        temp.flush().unwrap();

        let source = MmapSource::open(temp.path(), StorageMedium::LocalFastApfs).unwrap();
        assert_eq!(source.advise(MmapAdvice::Sequential), Ok(()));
        assert_eq!(source.advise(MmapAdvice::Random), Ok(()));
        assert_eq!(source.advise(MmapAdvice::WillNeed), Ok(()));
        assert_eq!(source.advise_range(100, 2000, MmapAdvice::WillNeed), Ok(()));
        assert_eq!(source.advise_range(0, 50000, MmapAdvice::WillNeed).err(), Some(TTZipStatus::ErrInvalidOffset));
    }
}
