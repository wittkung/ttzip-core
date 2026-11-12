// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Zstandard compression context RAII wrapper, buffer compression, and LDM streaming pipeline.

use super::types::*;
use crate::types::TTZipStatus;
use std::cell::RefCell;
use std::ptr::NonNull;

/// Safe RAII wrapper for `ZSTD_CCtx`.
pub struct ZstdCCtx {
    handle: NonNull<ZstdCCtxOpaque>,
}

unsafe impl Send for ZstdCCtx {}

impl ZstdCCtx {
    /// Allocates a new Zstandard compression context.
    pub fn new() -> Result<Self, TTZipStatus> {
        let ptr = unsafe { ZSTD_createCCtx() };
        let handle = NonNull::new(ptr).ok_or(TTZipStatus::ErrOutOfMemory)?;
        Ok(Self { handle })
    }

    /// Sets a generic compression parameter on the context.
    pub fn set_parameter(&mut self, param: ZstdCParameter, value: i32) -> Result<(), TTZipStatus> {
        let res = unsafe { ZSTD_CCtx_setParameter(self.handle.as_ptr(), param, value as libc::c_int) };
        if unsafe { ZSTD_isError(res) } != 0 {
            Err(TTZipStatus::ErrInvalidParam)
        } else {
            Ok(())
        }
    }

    fn set_positive_u32(&mut self, param: ZstdCParameter, value: u32) -> Result<(), TTZipStatus> {
        if value > 0 {
            self.set_parameter(param, value as i32)
        } else {
            Ok(())
        }
    }

    fn apply_ldm_config(&mut self, config: &ZstdConfig) -> Result<(), TTZipStatus> {
        if !config.enable_ldm {
            return Ok(());
        }
        self.set_parameter(ZstdCParameter::EnableLongDistanceMatching, 1)?;
        self.set_positive_u32(ZstdCParameter::LdmHashLog, config.ldm_hash_log)?;
        self.set_positive_u32(ZstdCParameter::LdmMinMatch, config.ldm_min_match)?;
        self.set_positive_u32(ZstdCParameter::LdmBucketSizeLog, config.ldm_bucket_size_log)?;
        self.set_positive_u32(ZstdCParameter::LdmHashRateLog, config.ldm_hash_rate_log)
    }

    /// Applies full configuration parameters (workers, LDM, windowLog, tuning, etc.).
    pub fn apply_config(&mut self, config: &ZstdConfig) -> Result<(), TTZipStatus> {
        self.set_parameter(ZstdCParameter::CompressionLevel, config.level)?;
        self.set_positive_u32(ZstdCParameter::NbWorkers, config.nb_workers)?;
        if config.job_size_mb > 0 {
            let job_size_bytes = (config.job_size_mb as i32).saturating_mul(1024 * 1024);
            self.set_parameter(ZstdCParameter::JobSize, job_size_bytes)?;
        }
        self.set_positive_u32(ZstdCParameter::OverlapLog, config.overlap_log)?;
        self.set_positive_u32(ZstdCParameter::WindowLog, config.window_log)?;
        self.apply_ldm_config(config)?;
        if config.enable_checksum {
            self.set_parameter(ZstdCParameter::ChecksumFlag, 1)?;
        }
        Ok(())
    }

    /// References a digested dictionary (`CDict`) on this compression context.
    pub fn ref_cdict_raw(&mut self, cdict_ptr: *const ZstdCDictOpaque) -> Result<(), TTZipStatus> {
        let res = unsafe { ZSTD_CCtx_refCDict(self.handle.as_ptr(), cdict_ptr) };
        if unsafe { ZSTD_isError(res) } != 0 {
            Err(TTZipStatus::ErrArchiveInitFailed)
        } else {
            Ok(())
        }
    }

    /// Loads raw dictionary bytes directly into the context.
    pub fn load_dictionary(&mut self, dict: &[u8]) -> Result<(), TTZipStatus> {
        let res = unsafe {
            ZSTD_CCtx_loadDictionary(
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

    /// Resets the compression session, parameters, and dictionary references for clean reuse.
    pub fn reset(&mut self) -> Result<(), TTZipStatus> {
        self.reset_session_and_parameters()
    }

    /// Resets both active session state, compression parameters, and dictionary references to library defaults.
    pub fn reset_session_and_parameters(&mut self) -> Result<(), TTZipStatus> {
        let res = unsafe { ZSTD_CCtx_reset(self.handle.as_ptr(), 3) };
        if unsafe { ZSTD_isError(res) } != 0 {
            return Err(TTZipStatus::ErrArchiveInitFailed);
        }
        self.ref_cdict_raw(std::ptr::null())
    }

    /// Returns the raw underlying `ZSTD_CCtx` pointer.
    #[inline]
    pub fn as_ptr(&self) -> *mut ZstdCCtxOpaque {
        self.handle.as_ptr()
    }

    /// Compresses a buffer in a single pass into destination.
    pub fn compress(&mut self, src: &[u8], dst: &mut [u8], level: i32) -> Result<usize, TTZipStatus> {
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
            ZSTD_compressCCtx(
                self.handle.as_ptr(),
                out_ptr,
                dst.len(),
                in_ptr,
                src.len(),
                level as libc::c_int,
            )
        };

        if unsafe { ZSTD_isError(res) } != 0 {
            Err(TTZipStatus::ErrCompressionFailed)
        } else {
            Ok(res)
        }
    }

    /// Compresses a buffer in a single pass using pre-digested `CDict`.
    pub fn compress_using_cdict_raw(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
        cdict_ptr: *const ZstdCDictOpaque,
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
            ZSTD_compress_usingCDict(
                self.handle.as_ptr(),
                out_ptr,
                dst.len(),
                in_ptr,
                src.len(),
                cdict_ptr,
            )
        };

        if unsafe { ZSTD_isError(res) } != 0 {
            Err(TTZipStatus::ErrCompressionFailed)
        } else {
            Ok(res)
        }
    }

    /// Compresses a buffer in a single pass into destination using parameters already configured on the context (`ZSTD_compress2`).
    pub fn compress2(&mut self, src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
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
            ZSTD_compress2(
                self.handle.as_ptr(),
                out_ptr,
                dst.len(),
                in_ptr,
                src.len(),
            )
        };

        if unsafe { ZSTD_isError(res) } != 0 {
            Err(TTZipStatus::ErrCompressionFailed)
        } else {
            Ok(res)
        }
    }

    /// Streams compression data into output buffer.
    pub fn compress_stream(
        &mut self,
        input: &mut ZstdInBuffer,
        output: &mut ZstdOutBuffer,
        end_op: ZstdEndDirective,
    ) -> Result<usize, TTZipStatus> {
        let res = unsafe {
            ZSTD_compressStream2(self.handle.as_ptr(), output, input, end_op)
        };
        if unsafe { ZSTD_isError(res) } != 0 {
            Err(TTZipStatus::ErrCompressionFailed)
        } else {
            Ok(res)
        }
    }
}

impl Drop for ZstdCCtx {
    fn drop(&mut self) {
        unsafe {
            ZSTD_freeCCtx(self.handle.as_ptr());
        }
    }
}

thread_local! {
    static TLS_ZSTD_CCTX: RefCell<Option<ZstdCCtx>> = const { RefCell::new(None) };
}

/// Executes closure with thread-local cached `ZstdCCtx`.
pub fn with_thread_local_zstd_cctx<F, R>(f: F) -> Result<R, TTZipStatus>
where
    F: FnOnce(&mut ZstdCCtx) -> Result<R, TTZipStatus>,
{
    TLS_ZSTD_CCTX.with(|cell| {
        let mut cached = cell.borrow_mut();
        let ctx = match cached.as_mut() {
            Some(c) => c,
            None => cached.insert(ZstdCCtx::new()?),
        };
        f(ctx)
    })
}

/// Computes upper bound on compressed bytes for a given input size in Zstandard.
#[inline]
pub fn zstd_compress_bound(src_size: usize) -> usize {
    unsafe { ZSTD_compressBound(src_size) }
}

/// Zero-copy Zstandard compression using thread-local cached CCtx.
pub fn zstd_compress(src: &[u8], dst: &mut [u8], level: i32) -> Result<usize, TTZipStatus> {
    with_thread_local_zstd_cctx(|cctx| {
        cctx.compress(src, dst, level)
    })
}

/// Zero-copy Zstandard compression with advanced configuration (workers, LDM, etc.).
pub fn zstd_compress_advanced(
    src: &[u8],
    dst: &mut [u8],
    config: &ZstdConfig,
) -> Result<usize, TTZipStatus> {
    with_thread_local_zstd_cctx(|cctx| {
        cctx.reset_session_and_parameters()?;
        let result = cctx.apply_config(config).and_then(|()| cctx.compress2(src, dst));
        let _ = cctx.reset_session_and_parameters();
        result
    })
}

/// Zero-copy Zstandard LDM compression (64MB .. 2GB deduplication window).
pub fn zstd_compress_ldm(
    src: &[u8],
    dst: &mut [u8],
    level: i32,
    window_mb: usize,
) -> Result<usize, TTZipStatus> {
    let config = ZstdConfig::ldm(level, 26).with_ldm_window_mb(window_mb);
    zstd_compress_advanced(src, dst, &config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zstd::zstd_decompress;

    #[test]
    fn test_zstd_compress_advanced_reuses_thread_local_cctx() {
        let ptr_before = with_thread_local_zstd_cctx(|cctx| Ok(cctx.as_ptr() as usize))
            .expect("borrow thread-local cctx before");

        let payload = b"Thread-local ZSTD_CCtx pooling verification payload across varying ZstdConfig parameters.".repeat(64);
        let mut comp_ldm = vec![0u8; zstd_compress_bound(payload.len())];
        let ldm_cfg = ZstdConfig::ldm(6, 20);
        let ldm_len = zstd_compress_advanced(&payload, &mut comp_ldm, &ldm_cfg)
            .expect("advanced ldm compress");

        let ptr_after_ldm = with_thread_local_zstd_cctx(|cctx| Ok(cctx.as_ptr() as usize))
            .expect("borrow thread-local cctx after ldm");
        assert_eq!(ptr_before, ptr_after_ldm);

        let mut comp_std = vec![0u8; zstd_compress_bound(payload.len())];
        let std_cfg = ZstdConfig {
            level: 1,
            enable_checksum: false,
            ..Default::default()
        };
        let std_len = zstd_compress_advanced(&payload, &mut comp_std, &std_cfg)
            .expect("advanced standard compress");

        let ptr_after_std = with_thread_local_zstd_cctx(|cctx| Ok(cctx.as_ptr() as usize))
            .expect("borrow thread-local cctx after std");
        assert_eq!(ptr_before, ptr_after_std);

        let mut decomp = vec![0u8; payload.len()];
        let d1 = zstd_decompress(&comp_ldm[..ldm_len], &mut decomp).expect("decompress ldm");
        assert_eq!(&decomp[..d1], payload.as_slice());

        let d2 = zstd_decompress(&comp_std[..std_len], &mut decomp).expect("decompress std");
        assert_eq!(&decomp[..d2], payload.as_slice());
    }

    #[test]
    fn test_cctx_reset_clears_referenced_cdict() {
        let dict_bytes = b"shared dictionary prefix tokens for zstd cctx reset test 0123456789".repeat(8);
        let dict = crate::zstd::ZstdDictionary::from_bytes("reset_dict", dict_bytes, 3)
            .expect("create dictionary");
        let payload = b"shared dictionary prefix tokens for zstd cctx reset test payload body";

        let mut cctx = ZstdCCtx::new().expect("create cctx");
        cctx.ref_cdict_raw(dict.cdict().as_ptr()).expect("ref cdict");
        let mut dict_comp = vec![0u8; zstd_compress_bound(payload.len())];
        let dict_len = cctx.compress2(payload, &mut dict_comp).expect("compress with cdict");
        drop(dict);

        cctx.reset().expect("reset cctx");
        cctx.set_parameter(ZstdCParameter::CompressionLevel, 3).expect("set level");
        let mut plain_comp = vec![0u8; zstd_compress_bound(payload.len())];
        let plain_len = cctx.compress2(payload, &mut plain_comp).expect("compress without dict");
        assert_ne!(&dict_comp[..dict_len], &plain_comp[..plain_len]);

        let mut decomp = vec![0u8; payload.len()];
        let d_len = zstd_decompress(&plain_comp[..plain_len], &mut decomp)
            .expect("plain decompress must succeed after cctx.reset()");
        assert_eq!(&decomp[..d_len], payload.as_slice());
    }
}

