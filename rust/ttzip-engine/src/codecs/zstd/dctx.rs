// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zstandard decompression context RAII wrapper and buffer decompression.

use super::types::*;
use crate::types::TTZipStatus;
use std::cell::RefCell;
use std::ptr::NonNull;

/// Safe RAII wrapper for `ZSTD_DCtx`.
pub struct ZstdDCtx {
    handle: NonNull<ZstdDCtxOpaque>,
}

unsafe impl Send for ZstdDCtx {}

impl ZstdDCtx {
    /// Allocates a new Zstandard decompression context.
    pub fn new() -> Result<Self, TTZipStatus> {
        let ptr = unsafe { ZSTD_createDCtx() };
        let handle = NonNull::new(ptr).ok_or(TTZipStatus::ErrOutOfMemory)?;
        Ok(Self { handle })
    }

    /// Sets maximum allowed windowLog on the decompression context (allowing up to 2GB window).
    pub fn set_max_window_log(&mut self, max_window_log: u32) -> Result<(), TTZipStatus> {
        let res = unsafe {
            ZSTD_DCtx_setParameter(
                self.handle.as_ptr(),
                ZstdDParameter::WindowLogMax,
                max_window_log.clamp(10, 31) as libc::c_int,
            )
        };
        if unsafe { ZSTD_isError(res) } != 0 {
            Err(TTZipStatus::ErrInvalidParam)
        } else {
            Ok(())
        }
    }

    /// References a digested dictionary (`DDict`) on this decompression context.
    pub fn ref_ddict_raw(&mut self, ddict_ptr: *const ZstdDDictOpaque) -> Result<(), TTZipStatus> {
        let res = unsafe { ZSTD_DCtx_refDDict(self.handle.as_ptr(), ddict_ptr) };
        if unsafe { ZSTD_isError(res) } != 0 {
            Err(TTZipStatus::ErrArchiveInitFailed)
        } else {
            Ok(())
        }
    }

    /// Loads raw dictionary bytes directly into the context.
    pub fn load_dictionary(&mut self, dict: &[u8]) -> Result<(), TTZipStatus> {
        let res = unsafe {
            ZSTD_DCtx_loadDictionary(
                self.handle.as_ptr(),
                dict.as_ptr() as *const libc::c_void,
                dict.len(),
            )
        };
        if unsafe { ZSTD_isError(res) } != 0 {
            Err(TTZipStatus::ErrArchiveInitFailed)
        } else {
            Ok(())
        }
    }

    /// Resets the decompression context for reuse.
    pub fn reset(&mut self) -> Result<(), TTZipStatus> {
        let res = unsafe { ZSTD_DCtx_reset(self.handle.as_ptr(), 1) }; // ZSTD_reset_session_only
        if unsafe { ZSTD_isError(res) } != 0 {
            Err(TTZipStatus::ErrArchiveInitFailed)
        } else {
            Ok(())
        }
    }

    /// Decompresses a buffer in a single pass into destination.
    pub fn decompress(&mut self, src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
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

        let res = unsafe {
            ZSTD_decompressDCtx(
                self.handle.as_ptr(),
                out_ptr,
                dst.len(),
                in_ptr,
                src.len(),
            )
        };

        if unsafe { ZSTD_isError(res) } != 0 {
            Err(TTZipStatus::ErrCorruptHeader)
        } else {
            Ok(res)
        }
    }

    /// Decompresses a buffer in a single pass using pre-digested `DDict`.
    pub fn decompress_using_ddict_raw(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
        ddict_ptr: *const ZstdDDictOpaque,
    ) -> Result<usize, TTZipStatus> {
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

        let res = unsafe {
            ZSTD_decompress_usingDDict(
                self.handle.as_ptr(),
                out_ptr,
                dst.len(),
                in_ptr,
                src.len(),
                ddict_ptr,
            )
        };

        if unsafe { ZSTD_isError(res) } != 0 {
            Err(TTZipStatus::ErrCorruptHeader)
        } else {
            Ok(res)
        }
    }

    /// Streams decompression data into output buffer.
    pub fn decompress_stream(
        &mut self,
        input: &mut ZstdInBuffer,
        output: &mut ZstdOutBuffer,
    ) -> Result<usize, TTZipStatus> {
        let res = unsafe {
            ZSTD_decompressStream(self.handle.as_ptr(), output, input)
        };
        if unsafe { ZSTD_isError(res) } != 0 {
            Err(TTZipStatus::ErrCorruptHeader)
        } else {
            Ok(res)
        }
    }
}

impl Drop for ZstdDCtx {
    fn drop(&mut self) {
        unsafe {
            ZSTD_freeDCtx(self.handle.as_ptr());
        }
    }
}

thread_local! {
    static TLS_ZSTD_DCTX: RefCell<Option<ZstdDCtx>> = const { RefCell::new(None) };
}

/// Executes closure with thread-local cached `ZstdDCtx`.
pub fn with_thread_local_zstd_dctx<F, R>(f: F) -> Result<R, TTZipStatus>
where
    F: FnOnce(&mut ZstdDCtx) -> Result<R, TTZipStatus>,
{
    TLS_ZSTD_DCTX.with(|cell| {
        let mut cached = cell.borrow_mut();
        if cached.is_none() {
            *cached = Some(ZstdDCtx::new()?);
        }
        let ctx = cached.as_mut().unwrap();
        f(ctx)
    })
}

/// Obtains uncompressed content size from Zstandard frame header, if available.
#[inline]
pub fn zstd_get_decompressed_size(src: &[u8]) -> Option<u64> {
    if src.is_empty() {
        return None;
    }
    let res = unsafe { ZSTD_getFrameContentSize(src.as_ptr() as *const libc::c_void, src.len()) };
    if res == u64::MAX || res == u64::MAX - 1 {
        None
    } else {
        Some(res)
    }
}

/// Zero-copy Zstandard decompression using thread-local cached DCtx.
pub fn zstd_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    with_thread_local_zstd_dctx(|dctx| {
        dctx.decompress(src, dst)
    })
}

