// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Guarded memory allocation using OS virtual memory page protections (mmap/mprotect).
//!
//! Provides `GuardedBuffer` which places unmapped/inaccessible guard pages immediately before
//! and after the payload buffer to detect off-by-one out-of-bounds reads and writes.

use std::ops::{Deref, DerefMut};
use std::slice;

/// A virtual memory buffer flanked by inaccessible (PROT_NONE) guard pages.
///
/// If right-aligned, the end of the user data slice is immediately adjacent to the trailing
/// guard page, causing any 1-byte read or write past the end of the buffer to immediately
/// trigger a hardware segmentation fault / page violation.
#[derive(Debug)]
pub struct GuardedBuffer {
    base_ptr: *mut u8,
    total_len: usize,
    data_ptr: *mut u8,
    data_len: usize,
}

unsafe impl Send for GuardedBuffer {}
unsafe impl Sync for GuardedBuffer {}

impl GuardedBuffer {
    /// Returns the system page size in bytes.
    #[inline]
    pub fn page_size() -> usize {
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

    /// Allocates a guarded buffer with `data` right-aligned against the trailing guard page.
    ///
    /// Any read or write past `&buffer[buffer.len() - 1]` immediately hits the trailing guard page.
    pub fn right_aligned(data: &[u8]) -> Result<Self, String> {
        let page_size = Self::page_size();
        let guard_bytes = page_size;
        let data_len = data.len();

        let payload_pages = if data_len == 0 {
            1
        } else {
            data_len.div_ceil(page_size)
        };
        let payload_bytes = payload_pages * page_size;
        let total_len = guard_bytes + payload_bytes + guard_bytes;

        #[cfg(unix)]
        {
            let base_ptr = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    total_len,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_ANON | libc::MAP_PRIVATE,
                    -1,
                    0,
                )
            };

            if base_ptr == libc::MAP_FAILED || base_ptr.is_null() {
                return Err("Failed to allocate virtual memory via mmap".to_string());
            }
            let base_ptr = base_ptr as *mut u8;

            // Protect leading guard page (PROT_NONE)
            let ret1 = unsafe { libc::mprotect(base_ptr as *mut libc::c_void, guard_bytes, libc::PROT_NONE) };
            if ret1 != 0 {
                unsafe { libc::munmap(base_ptr as *mut libc::c_void, total_len) };
                return Err("Failed to set leading guard page protection".to_string());
            }

            // Protect trailing guard page (PROT_NONE)
            let trailing_guard_ptr = unsafe { base_ptr.add(guard_bytes + payload_bytes) };
            let ret2 = unsafe {
                libc::mprotect(
                    trailing_guard_ptr as *mut libc::c_void,
                    guard_bytes,
                    libc::PROT_NONE,
                )
            };
            if ret2 != 0 {
                unsafe { libc::munmap(base_ptr as *mut libc::c_void, total_len) };
                return Err("Failed to set trailing guard page protection".to_string());
            }

            // In right-aligned mode, align data to the very end of payload_bytes
            let offset = payload_bytes - data_len;
            let data_ptr = unsafe { base_ptr.add(guard_bytes + offset) };

            if data_len > 0 {
                unsafe {
                    std::ptr::copy_nonoverlapping(data.as_ptr(), data_ptr, data_len);
                }
            }

            Ok(Self {
                base_ptr,
                total_len,
                data_ptr,
                data_len,
            })
        }
        #[cfg(not(unix))]
        {
            let mut vec = data.to_vec();
            let data_ptr = vec.as_mut_ptr();
            std::mem::forget(vec);
            Ok(Self {
                base_ptr: data_ptr,
                total_len: data_len,
                data_ptr,
                data_len,
            })
        }
    }

    /// Allocates a guarded buffer with `data` left-aligned against the leading guard page.
    ///
    /// Any read or write before `&buffer[0]` immediately hits the leading guard page.
    pub fn left_aligned(data: &[u8]) -> Result<Self, String> {
        let page_size = Self::page_size();
        let guard_bytes = page_size;
        let data_len = data.len();

        let payload_pages = if data_len == 0 {
            1
        } else {
            data_len.div_ceil(page_size)
        };
        let payload_bytes = payload_pages * page_size;
        let total_len = guard_bytes + payload_bytes + guard_bytes;

        #[cfg(unix)]
        {
            let base_ptr = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    total_len,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_ANON | libc::MAP_PRIVATE,
                    -1,
                    0,
                )
            };

            if base_ptr == libc::MAP_FAILED || base_ptr.is_null() {
                return Err("Failed to allocate virtual memory via mmap".to_string());
            }
            let base_ptr = base_ptr as *mut u8;

            // Protect leading guard page (PROT_NONE)
            let ret1 = unsafe { libc::mprotect(base_ptr as *mut libc::c_void, guard_bytes, libc::PROT_NONE) };
            if ret1 != 0 {
                unsafe { libc::munmap(base_ptr as *mut libc::c_void, total_len) };
                return Err("Failed to set leading guard page protection".to_string());
            }

            // Protect trailing guard page (PROT_NONE)
            let trailing_guard_ptr = unsafe { base_ptr.add(guard_bytes + payload_bytes) };
            let ret2 = unsafe {
                libc::mprotect(
                    trailing_guard_ptr as *mut libc::c_void,
                    guard_bytes,
                    libc::PROT_NONE,
                )
            };
            if ret2 != 0 {
                unsafe { libc::munmap(base_ptr as *mut libc::c_void, total_len) };
                return Err("Failed to set trailing guard page protection".to_string());
            }

            // In left-aligned mode, data starts immediately after leading guard page
            let data_ptr = unsafe { base_ptr.add(guard_bytes) };

            if data_len > 0 {
                unsafe {
                    std::ptr::copy_nonoverlapping(data.as_ptr(), data_ptr, data_len);
                }
            }

            Ok(Self {
                base_ptr,
                total_len,
                data_ptr,
                data_len,
            })
        }
        #[cfg(not(unix))]
        {
            let mut vec = data.to_vec();
            let data_ptr = vec.as_mut_ptr();
            std::mem::forget(vec);
            Ok(Self {
                base_ptr: data_ptr,
                total_len: data_len,
                data_ptr,
                data_len,
            })
        }
    }

    /// Returns a slice over the accessible data.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        if self.data_len == 0 || self.data_ptr.is_null() {
            &[]
        } else {
            unsafe { slice::from_raw_parts(self.data_ptr, self.data_len) }
        }
    }

    /// Returns a mutable slice over the accessible data.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        if self.data_len == 0 || self.data_ptr.is_null() {
            &mut []
        } else {
            unsafe { slice::from_raw_parts_mut(self.data_ptr, self.data_len) }
        }
    }

    /// Returns the length of the payload in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.data_len
    }

    /// Returns true if the buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data_len == 0
    }

    /// Returns raw pointer to the data slice.
    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        self.data_ptr
    }

    /// Returns raw mutable pointer to the data slice.
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.data_ptr
    }

    /// Returns the total virtual memory size allocated in bytes.
    #[inline]
    pub fn total_allocated_size(&self) -> usize {
        self.total_len
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

impl Drop for GuardedBuffer {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            if !self.base_ptr.is_null() && self.total_len > 0 {
                unsafe {
                    libc::munmap(self.base_ptr as *mut libc::c_void, self.total_len);
                }
                self.base_ptr = std::ptr::null_mut();
                self.data_ptr = std::ptr::null_mut();
            }
        }
        #[cfg(not(unix))]
        {
            if !self.base_ptr.is_null() && self.total_len > 0 {
                unsafe {
                    let _ = Vec::from_raw_parts(self.base_ptr, self.data_len, self.total_len);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guarded_buffer_right_aligned_basic() {
        let input = b"Hello, TTZip Guarded Memory!";
        let mut buf = GuardedBuffer::right_aligned(input).expect("Allocation must succeed");

        assert_eq!(buf.len(), input.len());
        assert_eq!(&buf[..], input);
        assert!(!buf.is_empty());

        // Modify in place
        buf[0] = b'h';
        assert_eq!(&buf[..], b"hello, TTZip Guarded Memory!");
    }

    #[test]
    fn test_guarded_buffer_left_aligned_basic() {
        let input = b"Left aligned test buffer payload";
        let buf = GuardedBuffer::left_aligned(input).expect("Allocation must succeed");

        assert_eq!(buf.len(), input.len());
        assert_eq!(&buf[..], input);
    }

    #[test]
    fn test_guarded_buffer_empty() {
        let buf = GuardedBuffer::right_aligned(&[]).expect("Empty allocation must succeed");
        assert_eq!(buf.len(), 0);
        assert!(buf.is_empty());
        assert_eq!(&buf[..], &[] as &[u8]);
    }

    #[test]
    fn test_guarded_buffer_multipage() {
        let page_size = GuardedBuffer::page_size();
        let size = page_size * 2 + 123;
        let pattern: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();

        let buf = GuardedBuffer::right_aligned(&pattern).expect("Multipage allocation must succeed");
        assert_eq!(buf.len(), size);
        assert_eq!(&buf[..], &pattern[..]);
        assert!(buf.total_allocated_size() >= size + page_size * 2);
    }
}
