// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe RAII wrapper around `libdeflate_decompressor`.

use super::ffi::*;
use crate::types::TTZipStatus;
use std::ptr::NonNull;

/// Specific decompress error distinctions.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DeflateDecompressError {
    /// The compressed data stream is corrupted or invalid.
    BadData,
    /// The compressed data ended prematurely before completing the output.
    ShortOutput,
    /// Destination buffer is insufficient to store the decompressed output (data is valid).
    InsufficientSpace,
}

impl std::fmt::Display for DeflateDecompressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadData => write!(f, "corrupt or invalid DEFLATE stream"),
            Self::ShortOutput => write!(f, "premature end of DEFLATE stream"),
            Self::InsufficientSpace => write!(f, "insufficient space in destination buffer"),
        }
    }
}

impl std::error::Error for DeflateDecompressError {}

impl From<DeflateDecompressError> for TTZipStatus {
    fn from(err: DeflateDecompressError) -> Self {
        match err {
            DeflateDecompressError::BadData => TTZipStatus::ErrCorruptHeader,
            DeflateDecompressError::ShortOutput | DeflateDecompressError::InsufficientSpace => {
                TTZipStatus::ErrExtractionFailed
            }
        }
    }
}

/// Safe RAII wrapper around `libdeflate_decompressor`.
pub struct DeflateDecompressor {
    handle: NonNull<LibdeflateDecompressorOpaque>,
}

unsafe impl Send for DeflateDecompressor {}

impl DeflateDecompressor {
    /// Creates a new Deflate decompressor context.
    pub fn new() -> Result<Self, TTZipStatus> {
        let ptr = unsafe { libdeflate_alloc_decompressor() };
        let handle = NonNull::new(ptr).ok_or(TTZipStatus::ErrOutOfMemory)?;
        Ok(Self { handle })
    }

    /// Decompresses raw RFC 1951 DEFLATE stream into pre-allocated destination buffer.
    pub fn decompress(&mut self, src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
        self.decompress_precise(src, dst).map_err(Into::into)
    }

    /// Precise raw DEFLATE decompression returning specific error enum.
    pub fn decompress_precise(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
    ) -> Result<usize, DeflateDecompressError> {
        let dummy_in = [0u8; 1];
        let in_ptr = if src.is_empty() {
            dummy_in.as_ptr() as *const libc::c_void
        } else {
            src.as_ptr() as *const libc::c_void
        };
        let mut dummy_out = [0u8; 1];
        let out_ptr = if dst.is_empty() {
            dummy_out.as_mut_ptr() as *mut libc::c_void
        } else {
            dst.as_mut_ptr() as *mut libc::c_void
        };

        let mut actual_out_size: libc::size_t = 0;
        let res = unsafe {
            libdeflate_deflate_decompress(
                self.handle.as_ptr(),
                in_ptr,
                src.len(),
                out_ptr,
                dst.len(),
                &mut actual_out_size,
            )
        };

        match res {
            LibdeflateResult::Success => Ok(actual_out_size),
            LibdeflateResult::BadData => Err(DeflateDecompressError::BadData),
            LibdeflateResult::ShortOutput => Err(DeflateDecompressError::ShortOutput),
            LibdeflateResult::InsufficientSpace => Err(DeflateDecompressError::InsufficientSpace),
        }
    }

    /// Extended raw DEFLATE decompression returning `(actual_in_bytes, actual_out_bytes)`.
    pub fn decompress_ex(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
    ) -> Result<(usize, usize), DeflateDecompressError> {
        let dummy_in = [0u8; 1];
        let in_ptr = if src.is_empty() {
            dummy_in.as_ptr() as *const libc::c_void
        } else {
            src.as_ptr() as *const libc::c_void
        };
        let mut dummy_out = [0u8; 1];
        let out_ptr = if dst.is_empty() {
            dummy_out.as_mut_ptr() as *mut libc::c_void
        } else {
            dst.as_mut_ptr() as *mut libc::c_void
        };

        let mut actual_in_size: libc::size_t = 0;
        let mut actual_out_size: libc::size_t = 0;
        let res = unsafe {
            libdeflate_deflate_decompress_ex(
                self.handle.as_ptr(),
                in_ptr,
                src.len(),
                out_ptr,
                dst.len(),
                &mut actual_in_size,
                &mut actual_out_size,
            )
        };

        match res {
            LibdeflateResult::Success => Ok((actual_in_size, actual_out_size)),
            LibdeflateResult::BadData => Err(DeflateDecompressError::BadData),
            LibdeflateResult::ShortOutput => Err(DeflateDecompressError::ShortOutput),
            LibdeflateResult::InsufficientSpace => Err(DeflateDecompressError::InsufficientSpace),
        }
    }

    /// Decompresses zlib (RFC 1950) stream into destination buffer.
    pub fn zlib_decompress(&mut self, src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
        self.zlib_decompress_precise(src, dst).map_err(Into::into)
    }

    /// Precise zlib decompression returning specific error enum.
    pub fn zlib_decompress_precise(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
    ) -> Result<usize, DeflateDecompressError> {
        let dummy_in = [0u8; 1];
        let in_ptr = if src.is_empty() {
            dummy_in.as_ptr() as *const libc::c_void
        } else {
            src.as_ptr() as *const libc::c_void
        };
        let mut dummy_out = [0u8; 1];
        let out_ptr = if dst.is_empty() {
            dummy_out.as_mut_ptr() as *mut libc::c_void
        } else {
            dst.as_mut_ptr() as *mut libc::c_void
        };

        let mut actual_out_size: libc::size_t = 0;
        let res = unsafe {
            libdeflate_zlib_decompress(
                self.handle.as_ptr(),
                in_ptr,
                src.len(),
                out_ptr,
                dst.len(),
                &mut actual_out_size,
            )
        };

        match res {
            LibdeflateResult::Success => Ok(actual_out_size),
            LibdeflateResult::BadData => Err(DeflateDecompressError::BadData),
            LibdeflateResult::ShortOutput => Err(DeflateDecompressError::ShortOutput),
            LibdeflateResult::InsufficientSpace => Err(DeflateDecompressError::InsufficientSpace),
        }
    }

    /// Extended zlib decompression returning `(actual_in_bytes, actual_out_bytes)`.
    pub fn zlib_decompress_ex(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
    ) -> Result<(usize, usize), DeflateDecompressError> {
        let dummy_in = [0u8; 1];
        let in_ptr = if src.is_empty() {
            dummy_in.as_ptr() as *const libc::c_void
        } else {
            src.as_ptr() as *const libc::c_void
        };
        let mut dummy_out = [0u8; 1];
        let out_ptr = if dst.is_empty() {
            dummy_out.as_mut_ptr() as *mut libc::c_void
        } else {
            dst.as_mut_ptr() as *mut libc::c_void
        };

        let mut actual_in_size: libc::size_t = 0;
        let mut actual_out_size: libc::size_t = 0;
        let res = unsafe {
            libdeflate_zlib_decompress_ex(
                self.handle.as_ptr(),
                in_ptr,
                src.len(),
                out_ptr,
                dst.len(),
                &mut actual_in_size,
                &mut actual_out_size,
            )
        };

        match res {
            LibdeflateResult::Success => Ok((actual_in_size, actual_out_size)),
            LibdeflateResult::BadData => Err(DeflateDecompressError::BadData),
            LibdeflateResult::ShortOutput => Err(DeflateDecompressError::ShortOutput),
            LibdeflateResult::InsufficientSpace => Err(DeflateDecompressError::InsufficientSpace),
        }
    }

    /// Decompresses gzip (RFC 1952) stream into destination buffer.
    pub fn gzip_decompress(&mut self, src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
        self.gzip_decompress_precise(src, dst).map_err(Into::into)
    }

    /// Precise gzip decompression returning specific error enum.
    pub fn gzip_decompress_precise(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
    ) -> Result<usize, DeflateDecompressError> {
        let dummy_in = [0u8; 1];
        let in_ptr = if src.is_empty() {
            dummy_in.as_ptr() as *const libc::c_void
        } else {
            src.as_ptr() as *const libc::c_void
        };
        let mut dummy_out = [0u8; 1];
        let out_ptr = if dst.is_empty() {
            dummy_out.as_mut_ptr() as *mut libc::c_void
        } else {
            dst.as_mut_ptr() as *mut libc::c_void
        };

        let mut actual_out_size: libc::size_t = 0;
        let res = unsafe {
            libdeflate_gzip_decompress(
                self.handle.as_ptr(),
                in_ptr,
                src.len(),
                out_ptr,
                dst.len(),
                &mut actual_out_size,
            )
        };

        match res {
            LibdeflateResult::Success => Ok(actual_out_size),
            LibdeflateResult::BadData => Err(DeflateDecompressError::BadData),
            LibdeflateResult::ShortOutput => Err(DeflateDecompressError::ShortOutput),
            LibdeflateResult::InsufficientSpace => Err(DeflateDecompressError::InsufficientSpace),
        }
    }

    /// Extended gzip decompression returning `(actual_in_bytes, actual_out_bytes)`.
    pub fn gzip_decompress_ex(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
    ) -> Result<(usize, usize), DeflateDecompressError> {
        let in_ptr = if src.is_empty() {
            std::ptr::null()
        } else {
            src.as_ptr() as *const libc::c_void
        };
        let out_ptr = if dst.is_empty() {
            std::ptr::null_mut()
        } else {
            dst.as_mut_ptr() as *mut libc::c_void
        };

        let mut actual_in_size: libc::size_t = 0;
        let mut actual_out_size: libc::size_t = 0;
        let res = unsafe {
            libdeflate_gzip_decompress_ex(
                self.handle.as_ptr(),
                in_ptr,
                src.len(),
                out_ptr,
                dst.len(),
                &mut actual_in_size,
                &mut actual_out_size,
            )
        };

        match res {
            LibdeflateResult::Success => Ok((actual_in_size, actual_out_size)),
            LibdeflateResult::BadData => Err(DeflateDecompressError::BadData),
            LibdeflateResult::ShortOutput => Err(DeflateDecompressError::ShortOutput),
            LibdeflateResult::InsufficientSpace => Err(DeflateDecompressError::InsufficientSpace),
        }
    }
}

impl Drop for DeflateDecompressor {
    fn drop(&mut self) {
        unsafe {
            libdeflate_free_decompressor(self.handle.as_ptr());
        }
    }
}

