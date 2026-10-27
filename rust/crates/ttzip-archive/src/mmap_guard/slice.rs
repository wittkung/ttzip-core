// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Safe Memory-Mapped Slice with Hardware Fault Isolation and RAII Bounds Enforcement.
//!
//! Wraps virtual memory mappings with bounds checking, hardware fault interception,
//! and automatic unmapping and registration cleanup on drop.

use std::fs::File;
use std::io::{Error, ErrorKind, Result};
use std::ops::Deref;

#[cfg(unix)]
use crate::mmap_guard::posix::{self, GuardToken};

/// Storage backing variant for a `SafeMmapSlice`.
enum MmapStorage {
    /// Memory mapping backed by `memmap2::Mmap`.
    Memmap(memmap2::Mmap),
    /// POSIX anonymous memory mapping allocated via `libc::mmap`.
    #[cfg(unix)]
    Anonymous { ptr: *mut u8, len: usize },
    /// Borrowed static or test slice.
    Borrowed,
    /// Zero-length empty allocation.
    Empty,
}

/// A guarded, bounds-checked memory-mapped slice that intercepts hardware I/O faults.
pub struct SafeMmapSlice {
    storage: MmapStorage,
    ptr: *const u8,
    len: usize,
    #[cfg(unix)]
    token: Option<GuardToken>,
}

// Safety: SafeMmapSlice exposes only immutable shared access to memory.
unsafe impl Send for SafeMmapSlice {}
unsafe impl Sync for SafeMmapSlice {}

impl SafeMmapSlice {
    /// Creates a zero-allocation empty slice.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            storage: MmapStorage::Empty,
            ptr: std::ptr::null(),
            len: 0,
            #[cfg(unix)]
            token: None,
        }
    }

    /// Wraps an existing `memmap2::Mmap` instance with hardware fault isolation.
    #[must_use]
    pub fn from_mmap(mmap: memmap2::Mmap) -> Self {
        let ptr = mmap.as_ptr();
        let len = mmap.len();

        #[cfg(unix)]
        let token = if len > 0 {
            posix::register_guarded_range(ptr as usize, len)
        } else {
            None
        };

        Self {
            storage: MmapStorage::Memmap(mmap),
            ptr,
            len,
            #[cfg(unix)]
            token,
        }
    }

    /// Maps a sub-range of a file into a safe, hardware-guarded slice.
    ///
    /// # Errors
    /// Returns `std::io::Error` if mapping fails or offset exceeds file bounds.
    pub fn from_file(file: &File, offset: u64, len: usize) -> Result<Self> {
        if len == 0 {
            return Ok(Self::empty());
        }

        let mmap = unsafe {
            memmap2::MmapOptions::new()
                .offset(offset)
                .len(len)
                .map(file)?
        };

        Ok(Self::from_mmap(mmap))
    }

    /// Maps an entire file into a safe, hardware-guarded slice.
    ///
    /// # Errors
    /// Returns `std::io::Error` if querying file metadata or mapping fails.
    pub fn from_file_full(file: &File) -> Result<Self> {
        let metadata = file.metadata()?;
        let file_len = metadata.len();
        if file_len == 0 {
            return Ok(Self::empty());
        }
        if file_len > usize::MAX as u64 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "file size exceeds addressable memory architecture",
            ));
        }

        let mmap = unsafe { memmap2::MmapOptions::new().map(file)? };
        Ok(Self::from_mmap(mmap))
    }

    /// Allocates an anonymous read-only memory-mapped slice.
    ///
    /// # Errors
    /// Returns `std::io::Error` on out-of-memory or kernel allocation failure.
    pub fn anonymous(len: usize) -> Result<Self> {
        if len == 0 {
            return Ok(Self::empty());
        }

        #[cfg(unix)]
        {
            let raw_ptr = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    len,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                    -1,
                    0,
                )
            };

            if raw_ptr == libc::MAP_FAILED {
                return Err(Error::last_os_error());
            }

            let ptr = raw_ptr as *mut u8;
            let token = posix::register_guarded_range(ptr as usize, len);

            Ok(Self {
                storage: MmapStorage::Anonymous { ptr, len },
                ptr,
                len,
                token,
            })
        }

        #[cfg(not(unix))]
        {
            let mmap = memmap2::MmapOptions::new().len(len).map_anon()?;
            Ok(Self::from_mmap(mmap))
        }
    }

    /// Creates a guarded slice from a borrowed static byte slice (primarily for testing).
    #[must_use]
    pub fn from_static_slice(slice: &'static [u8]) -> Self {
        let ptr = slice.as_ptr();
        let len = slice.len();

        #[cfg(unix)]
        let token = if len > 0 {
            posix::register_guarded_range(ptr as usize, len)
        } else {
            None
        };

        Self {
            storage: MmapStorage::Borrowed,
            ptr,
            len,
            #[cfg(unix)]
            token,
        }
    }

    /// Returns the length in bytes.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the slice contains 0 bytes.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the raw pointer to the mapped memory.
    #[inline]
    #[must_use]
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr
    }

    /// Returns the virtual memory address range `[start, end)`.
    #[inline]
    #[must_use]
    pub fn address_range(&self) -> (usize, usize) {
        let start = self.ptr as usize;
        let end = start.saturating_add(self.len);
        (start, end)
    }

    /// Validates sub-range bounds against arithmetic overflow and slice boundaries.
    ///
    /// # Errors
    /// Returns `ErrorKind::InvalidInput` if `offset + count > self.len()`.
    pub fn check_bounds(&self, offset: usize, count: usize) -> Result<()> {
        let end = offset.checked_add(count).ok_or_else(|| {
            Error::new(ErrorKind::InvalidInput, "mmap slice offset overflow")
        })?;
        if end > self.len {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "mmap slice bounds exceeded (offset: {}, len: {}, slice_len: {})",
                    offset, count, self.len
                ),
            ));
        }
        Ok(())
    }

    /// Returns a reference to the underlying `memmap2::Mmap` if backed by one.
    #[must_use]
    pub fn as_memmap(&self) -> Option<&memmap2::Mmap> {
        match &self.storage {
            MmapStorage::Memmap(ref m) => Some(m),
            _ => None,
        }
    }

    /// Exposes the underlying memory as an immutable byte slice.
    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        if self.len == 0 || self.ptr.is_null() {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
        }
    }

    /// Returns a sub-slice within the view after strict bounds validation.
    ///
    /// # Errors
    /// Returns `ErrorKind::InvalidInput` if requested range exceeds bounds.
    pub fn slice(&self, offset: usize, count: usize) -> Result<&[u8]> {
        self.check_bounds(offset, count)?;
        Ok(&self.as_slice()[offset..offset + count])
    }

    /// Safely copies up to `dst.len()` bytes into `dst` through the hardware fault barrier.
    ///
    /// # Errors
    /// Returns `ErrorKind::UnexpectedEof` if hardware page fault occurs during read.
    pub fn safe_read(&self, offset: usize, dst: &mut [u8]) -> Result<usize> {
        if offset >= self.len || dst.is_empty() {
            return Ok(0);
        }
        let available = self.len - offset;
        let to_read = dst.len().min(available);
        let src = &self.as_slice()[offset..offset + to_read];
        crate::mmap_guard::platform_safe_read(src, &mut dst[..to_read])
    }

    /// Safely reads exactly `dst.len()` bytes into `dst`.
    ///
    /// # Errors
    /// Returns `ErrorKind::UnexpectedEof` on hardware fault or if insufficient bytes remain.
    pub fn safe_read_exact(&self, offset: usize, dst: &mut [u8]) -> Result<()> {
        self.check_bounds(offset, dst.len())?;
        let read = self.safe_read(offset, dst)?;
        if read != dst.len() {
            return Err(Error::new(
                ErrorKind::UnexpectedEof,
                "failed to fill whole buffer from guarded mmap",
            ));
        }
        Ok(())
    }

    /// Safely reads a single byte at the specified offset.
    ///
    /// # Errors
    /// Returns `ErrorKind::UnexpectedEof` or `ErrorKind::InvalidInput`.
    pub fn safe_read_byte(&self, offset: usize) -> Result<u8> {
        let mut byte = [0u8; 1];
        self.safe_read_exact(offset, &mut byte)?;
        Ok(byte[0])
    }
}

impl Deref for SafeMmapSlice {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl AsRef<[u8]> for SafeMmapSlice {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl Drop for SafeMmapSlice {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(token) = self.token.take() {
            posix::unregister_guarded_range(token);
        }

        #[cfg(unix)]
        if let MmapStorage::Anonymous { ptr, len } = self.storage {
            if !ptr.is_null() && len > 0 {
                unsafe {
                    libc::munmap(ptr as *mut libc::c_void, len);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_empty_slice() {
        let empty = SafeMmapSlice::empty();
        assert_eq!(empty.len(), 0);
        assert!(empty.is_empty());
        assert_eq!(empty.as_slice(), &[] as &[u8]);
        assert_eq!(empty.address_range(), (0, 0));
        assert!(empty.as_memmap().is_none());

        let mut buf = [0u8; 4];
        let read = empty.safe_read(0, &mut buf).unwrap();
        assert_eq!(read, 0);
        assert!(empty.safe_read_exact(0, &mut buf).is_err());
    }

    #[test]
    fn test_from_static_slice() {
        static TEST_DATA: &[u8] = b"HELLO_METASTUDYLINE_MMAP_GUARD";
        let guarded = SafeMmapSlice::from_static_slice(TEST_DATA);

        assert_eq!(guarded.len(), TEST_DATA.len());
        assert!(!guarded.is_empty());
        assert_eq!(&*guarded, TEST_DATA);

        let mut buf = [0u8; 5];
        guarded.safe_read_exact(0, &mut buf).unwrap();
        assert_eq!(&buf, b"HELLO");

        let byte = guarded.safe_read_byte(6).unwrap();
        assert_eq!(byte, b'M');

        let sub = guarded.slice(6, 14).unwrap();
        assert_eq!(sub, b"METASTUDYLINE_");

        assert!(guarded.check_bounds(0, TEST_DATA.len()).is_ok());
        assert!(guarded.check_bounds(1, TEST_DATA.len()).is_err());
    }

    #[test]
    fn test_from_file_full_and_range() {
        let mut tmp = NamedTempFile::new().expect("temp file");
        let payload = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        tmp.write_all(payload).expect("write payload");
        tmp.flush().expect("flush");

        let full_slice = SafeMmapSlice::from_file_full(tmp.as_file()).expect("mmap full");
        assert!(full_slice.as_memmap().is_some());
        assert_eq!(full_slice.len(), payload.len());
        assert_eq!(&*full_slice, payload);

        let sub_slice = SafeMmapSlice::from_file(tmp.as_file(), 10, 16).expect("mmap sub");
        assert_eq!(sub_slice.len(), 16);
        assert_eq!(&*sub_slice, &payload[10..26]);

        let mut out = [0u8; 4];
        sub_slice.safe_read_exact(0, &mut out).unwrap();
        assert_eq!(&out, b"ABCD");
    }

    #[test]
    fn test_anonymous_slice_lifecycle() {
        let anon = SafeMmapSlice::anonymous(4096).expect("anonymous mmap");
        assert_eq!(anon.len(), 4096);
        assert_ne!(anon.as_ptr(), std::ptr::null());

        let (start, end) = anon.address_range();
        assert_eq!(end - start, 4096);

        // Writing to anonymous memory is allowed before conversion or read
        let mut read_buf = [0u8; 8];
        let bytes_read = anon.safe_read(0, &mut read_buf).unwrap();
        assert_eq!(bytes_read, 8);
        assert_eq!(read_buf, [0u8; 8]);
    }
}

