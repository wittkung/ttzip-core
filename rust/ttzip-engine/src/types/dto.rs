// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Data transfer objects (DTOs), entry metadata, and zero-copy buffer views.

use libc::c_char;

use super::options::TTZIP_ABI_VERSION_2;

/// Zero-copy read-only contiguous byte buffer slice descriptor.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TTZipBufferRef {
    pub data: *const u8,
    pub len: usize,
}

impl TTZipBufferRef {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            data: std::ptr::null(),
            len: 0,
        }
    }

    #[inline]
    pub const fn from_slice(slice: &[u8]) -> Self {
        Self {
            data: slice.as_ptr(),
            len: slice.len(),
        }
    }

    #[inline]
    pub unsafe fn as_slice<'a>(&self) -> &'a [u8] {
        if self.data.is_null() || self.len == 0 {
            &[]
        } else {
            std::slice::from_raw_parts(self.data, self.len)
        }
    }
}

/// Zero-copy mutable contiguous byte buffer slice descriptor.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TTZipBufferMut {
    pub data: *mut u8,
    pub len: usize,
    pub capacity: usize,
}

impl TTZipBufferMut {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            data: std::ptr::null_mut(),
            len: 0,
            capacity: 0,
        }
    }

    #[inline]
    pub fn from_vec(vec: &mut Vec<u8>) -> Self {
        Self {
            data: vec.as_mut_ptr(),
            len: vec.len(),
            capacity: vec.capacity(),
        }
    }

    #[inline]
    pub unsafe fn as_slice<'a>(&self) -> &'a [u8] {
        if self.data.is_null() || self.len == 0 {
            &[]
        } else {
            std::slice::from_raw_parts(self.data, self.len)
        }
    }

    #[inline]
    pub unsafe fn as_mut_slice<'a>(&mut self) -> &'a mut [u8] {
        if self.data.is_null() || self.len == 0 {
            &mut []
        } else {
            std::slice::from_raw_parts_mut(self.data, self.len)
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TTZipEntryMetadata {
    pub struct_size: u32,
    pub abi_version: u32,
    pub path: *const c_char,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub mtime_epoch_secs: i64,
    pub mode: u32,
    pub is_directory: bool,
    pub is_encrypted: bool,
    pub compression_method: u16,
    pub detected_encoding: *const c_char,
}

impl Default for TTZipEntryMetadata {
    fn default() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            abi_version: TTZIP_ABI_VERSION_2,
            path: std::ptr::null(),
            uncompressed_size: 0,
            compressed_size: 0,
            crc32: 0,
            mtime_epoch_secs: 0,
            mode: 0,
            is_directory: false,
            is_encrypted: false,
            compression_method: 0,
            detected_encoding: std::ptr::null(),
        }
    }
}

/// Zero-copy C-ABI packed array representing batch entries for FFI transfer.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TTZipPackedEntryArray {
    pub struct_size: u32,
    pub abi_version: u32,
    pub utf8_bytes: *const u8,
    pub total_bytes_len: usize,
    pub path_offsets: *const u32,
    pub path_lens: *const u32,
    pub uncompressed_sizes: *const u64,
    pub compressed_sizes: *const u64,
    pub crc32s: *const u32,
    pub mtimes: *const i64,
    pub modes: *const u32,
    pub flags: *const u8,
    pub count: usize,
}

/// Windowed VFS node summary DTO for zero-copy UI paging queries.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TTZipVfsNodeSummary {
    pub struct_size: u32,
    pub abi_version: u32,
    pub node_id: u32,
    pub name_utf8: *const c_char,
    pub name_len: u32,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub mtime_epoch_secs: i64,
    pub mode: u32,
    pub is_directory: bool,
    pub is_encrypted: bool,
    pub has_children: bool,
}
