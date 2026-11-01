// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! RAII Safe Cleanup Guards for Libarchive pointers.

use super::sys::*;
use libc::c_void;
use std::ffi::CStr;

/// Safely extracts pathname from an archive entry, prioritizing UTF-8 and falling back to locale-encoded string.
pub unsafe fn archive_entry_pathname_safe<'a>(e: *mut c_void) -> Option<&'a CStr> {
    unsafe {
        if e.is_null() {
            return None;
        }
        let p_utf8 = archive_entry_pathname_utf8(e);
        if !p_utf8.is_null() {
            return Some(CStr::from_ptr(p_utf8));
        }
        let p_loc = archive_entry_pathname(e);
        if !p_loc.is_null() {
            return Some(CStr::from_ptr(p_loc));
        }
        None
    }
}

pub struct ArchiveReadGuard(pub *mut c_void);

impl Drop for ArchiveReadGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                archive_read_close(self.0);
                archive_read_free(self.0);
            }
            self.0 = std::ptr::null_mut();
        }
    }
}

pub struct ArchiveWriteGuard(pub *mut c_void);

impl ArchiveWriteGuard {
    /// Explicitly closes and frees the underlying libarchive write handle,
    /// returning `Err(rc)` if `archive_write_close` fails (`rc != 0`).
    #[inline]
    pub fn close_and_free(mut self) -> Result<(), libc::c_int> {
        if self.0.is_null() {
            return Ok(());
        }
        let ptr = self.0;
        self.0 = std::ptr::null_mut();
        let rc = unsafe {
            let close_rc = archive_write_close(ptr);
            archive_write_free(ptr);
            close_rc
        };
        if rc == 0 {
            Ok(())
        } else {
            Err(rc)
        }
    }
}

impl Drop for ArchiveWriteGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                archive_write_close(self.0);
                archive_write_free(self.0);
            }
            self.0 = std::ptr::null_mut();
        }
    }
}

pub struct ArchiveEntryGuard(pub *mut c_void);

impl Drop for ArchiveEntryGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                archive_entry_free(self.0);
            }
            self.0 = std::ptr::null_mut();
        }
    }
}
