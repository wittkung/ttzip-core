// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Hardware MMU Guard-Page Protected Buffer (`GuardedBuffer`).
//!
//! Allocates virtual memory pages containing readable/writable working memory
//! immediately followed by a `PROT_NONE` guard page. The active buffer slice is
//! strictly end-aligned against the leading edge of the guard page so that any
//! 1-byte out-of-bounds read or write immediately triggers a hardware MMU page fault.

use std::ops::{Deref, DerefMut};

/// Errors encountered during guarded buffer memory allocation and protection.
#[derive(Debug, thiserror::Error)]
pub enum GuardedBufferError {
    /// Memory allocation (`mmap`) failed.
    #[error("Failed to mmap guarded buffer memory: {0}")]
    AllocationFailed(#[source] std::io::Error),

    /// Guard page protection (`mprotect`) failed.
    #[error("Failed to mprotect guard page to PROT_NONE: {0}")]
    ProtectionFailed(#[source] std::io::Error),
}

/// Retrieves the system virtual memory page size in bytes.
#[inline]
pub fn system_page_size() -> usize {
    #[cfg(unix)]
    {
        let ps = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if ps > 0 {
            ps as usize
        } else {
            4096
        }
    }
    #[cfg(not(unix))]
    {
        4096
    }
}

/// A contiguous memory buffer protected by an adjacent trailing MMU `PROT_NONE` guard page.
///
/// Layout:
/// ```text
/// +---------------------------+------------------------+-------------------+
/// | Unused Lead Padding       | Usable Working Slice   | PROT_NONE Guard   |
/// | (0 .. page_size - 1)      | (exact `size` bytes)   | (1 page: 4K/16K)  |
/// +---------------------------+------------------------+-------------------+
/// ^ base_ptr                  ^ data_ptr               ^ guard_ptr
///                             |<----- len == size ---->|
/// ```
///
/// Any read or write access to `data_ptr[size]` immediately hits the hardware
/// guard page and triggers an MMU segmentation fault (SIGSEGV / SIGBUS).
#[derive(Debug)]
pub struct GuardedBuffer {
    base_ptr: *mut libc::c_void,
    data_ptr: *mut u8,
    size: usize,
    total_bytes: usize,
    page_size: usize,
}

impl GuardedBuffer {
    /// Attempts to allocate a new guarded buffer of the requested byte size.
    pub fn try_new(size: usize) -> Result<Self, GuardedBufferError> {
        let page_size = system_page_size();
        let nr_data_pages = if size == 0 {
            1
        } else {
            size.div_ceil(page_size)
        };
        let total_pages = nr_data_pages + 1; // Data pages + 1 trailing guard page
        let total_bytes = total_pages
            .checked_mul(page_size)
            .ok_or_else(|| GuardedBufferError::AllocationFailed(std::io::Error::from(std::io::ErrorKind::InvalidInput)))?;

        #[cfg(unix)]
        let base_ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                total_bytes,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };

        #[cfg(not(unix))]
        let base_ptr = libc::MAP_FAILED;

        if base_ptr == libc::MAP_FAILED || base_ptr.is_null() {
            return Err(GuardedBufferError::AllocationFailed(
                std::io::Error::last_os_error(),
            ));
        }

        let guard_offset = nr_data_pages * page_size;
        let guard_ptr = unsafe { (base_ptr as *mut u8).add(guard_offset) };

        #[cfg(unix)]
        let mprotect_status = unsafe {
            libc::mprotect(
                guard_ptr as *mut libc::c_void,
                page_size,
                libc::PROT_NONE,
            )
        };

        #[cfg(not(unix))]
        let mprotect_status = -1;

        if mprotect_status != 0 {
            let err = std::io::Error::last_os_error();
            #[cfg(unix)]
            unsafe {
                libc::munmap(base_ptr, total_bytes);
            }
            return Err(GuardedBufferError::ProtectionFailed(err));
        }

        // End-aligned offset: working buffer ends precisely at guard page boundary.
        let data_ptr = unsafe { guard_ptr.sub(size) };

        Ok(Self {
            base_ptr,
            data_ptr,
            size,
            total_bytes,
            page_size,
        })
    }

    /// Allocates a new guarded buffer, panicking on allocation failure.
    #[inline]
    pub fn new(size: usize) -> Self {
        Self::try_new(size).expect("Failed to allocate GuardedBuffer with MMU guard page")
    }

    /// Returns a shared slice to the usable memory region.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        if self.size == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.data_ptr, self.size) }
        }
    }

    /// Returns an exclusive mutable slice to the usable memory region.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        if self.size == 0 {
            &mut []
        } else {
            unsafe { std::slice::from_raw_parts_mut(self.data_ptr, self.size) }
        }
    }

    /// Length of the usable memory buffer in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.size
    }

    /// Returns `true` if the buffer has a logical size of 0.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// System virtual page size used for this allocation.
    #[inline]
    pub fn page_size(&self) -> usize {
        self.page_size
    }

    /// Pointer to the active usable data slice.
    #[inline]
    pub fn data_ptr(&self) -> *const u8 {
        self.data_ptr
    }

    /// Mutable pointer to the active usable data slice.
    #[inline]
    pub fn data_mut_ptr(&mut self) -> *mut u8 {
        self.data_ptr
    }

    /// Pointer to the beginning of the `PROT_NONE` guard page.
    #[inline]
    pub fn guard_page_ptr(&self) -> *const u8 {
        unsafe { (self.base_ptr as *const u8).add(self.total_bytes - self.page_size) }
    }
}

impl Drop for GuardedBuffer {
    fn drop(&mut self) {
        if !self.base_ptr.is_null() && self.base_ptr != libc::MAP_FAILED {
            #[cfg(unix)]
            unsafe {
                libc::munmap(self.base_ptr, self.total_bytes);
            }
            self.base_ptr = std::ptr::null_mut();
        }
    }
}

impl Deref for GuardedBuffer {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl DerefMut for GuardedBuffer {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl AsRef<[u8]> for GuardedBuffer {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl AsMut<[u8]> for GuardedBuffer {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        self.as_mut_slice()
    }
}

unsafe impl Send for GuardedBuffer {}
unsafe impl Sync for GuardedBuffer {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codecs::deflate::{deflate_compress, deflate_decompress};
    use crate::codecs::lz4::{lz4_compress, lz4_decompress};
    use crate::codecs::zstd::{zstd_compress, zstd_decompress};

    #[test]
    fn test_guarded_buffer_allocation_and_alignment() {
        let size = 1024;
        let mut buf = GuardedBuffer::new(size);
        assert_eq!(buf.len(), size);
        assert!(!buf.is_empty());

        let guard_ptr = buf.guard_page_ptr() as usize;
        let data_end_ptr = (buf.as_slice().as_ptr() as usize) + buf.len();
        assert_eq!(guard_ptr, data_end_ptr, "Buffer must be end-aligned to the guard page");

        // Test normal read and write
        buf[0] = 0xAA;
        buf[size - 1] = 0xBB;
        assert_eq!(buf[0], 0xAA);
        assert_eq!(buf[size - 1], 0xBB);
    }

    #[test]
    fn test_guarded_buffer_zero_sized() {
        let buf = GuardedBuffer::new(0);
        assert_eq!(buf.len(), 0);
        assert!(buf.is_empty());
        assert_eq!(buf.as_slice(), &[] as &[u8]);
    }

    #[test]
    fn test_guarded_buffer_page_boundary_sizes() {
        let page_sz = system_page_size();
        for sz in [1, page_sz - 1, page_sz, page_sz + 1, page_sz * 2] {
            let mut buf = GuardedBuffer::new(sz);
            assert_eq!(buf.len(), sz);
            let guard_ptr = buf.guard_page_ptr() as usize;
            let data_end_ptr = (buf.as_slice().as_ptr() as usize) + buf.len();
            assert_eq!(guard_ptr, data_end_ptr);

            if sz == 1 {
                buf[0] = 0x12;
                assert_eq!(buf[0], 0x12);
            } else {
                buf[0] = 0x12;
                buf[sz - 1] = 0x34;
                assert_eq!(buf[0], 0x12);
                assert_eq!(buf[sz - 1], 0x34);
            }
        }
    }

    #[test]
    fn test_guarded_buffer_codec_decompression_bounds_deflate() {
        let payload = b"Deflate boundary test payload with repetitive patterns for compression verification. 1234567890.";
        let mut comp_buf = vec![0u8; 1024];
        let comp_len = deflate_compress(payload, &mut comp_buf, 6).expect("deflate compress");

        let mut guarded_dst = GuardedBuffer::new(payload.len());
        let decomp_len = deflate_decompress(&comp_buf[..comp_len], guarded_dst.as_mut_slice())
            .expect("deflate decompress into guarded buffer");

        assert_eq!(decomp_len, payload.len());
        assert_eq!(guarded_dst.as_slice(), payload);
    }

    #[test]
    fn test_guarded_buffer_codec_decompression_bounds_lz4() {
        let payload = b"LZ4 fast compression boundary check on MMU guarded page buffer. High performance native verification.";
        let mut comp_buf = vec![0u8; 1024];
        let comp_len = lz4_compress(payload, &mut comp_buf).expect("lz4 compress");

        let mut guarded_dst = GuardedBuffer::new(payload.len());
        let decomp_len = lz4_decompress(&comp_buf[..comp_len], guarded_dst.as_mut_slice())
            .expect("lz4 decompress into guarded buffer");

        assert_eq!(decomp_len, payload.len());
        assert_eq!(guarded_dst.as_slice(), payload);
    }

    #[test]
    fn test_guarded_buffer_codec_decompression_bounds_zstd() {
        let payload = b"Zstandard end-aligned guard page verification. Testing zero-byte overrun safety in modern compression codecs.";
        let mut comp_buf = vec![0u8; 1024];
        let comp_len = zstd_compress(payload, &mut comp_buf, 3).expect("zstd compress");

        let mut guarded_dst = GuardedBuffer::new(payload.len());
        let decomp_len = zstd_decompress(&comp_buf[..comp_len], guarded_dst.as_mut_slice())
            .expect("zstd decompress into guarded buffer");

        assert_eq!(decomp_len, payload.len());
        assert_eq!(guarded_dst.as_slice(), payload);
    }
}
