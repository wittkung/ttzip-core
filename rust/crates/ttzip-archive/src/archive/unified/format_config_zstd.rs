// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Zstandard target stream writer and context pool for unified archive creation.

use super::CreateTransactionContext;
use super::map_zstd_compression_level;
use crate::types::{TTZipCreateOptions, TTZipStatus};

pub(super) fn zstd_window_log_from_mb(dictionary_size_mb: u32, enable_ldm: bool) -> u32 {
    if dictionary_size_mb == 0 {
        return 0;
    }
    let bytes = u64::from(dictionary_size_mb).saturating_mul(1024 * 1024);
    let log = (64 - bytes.leading_zeros()).saturating_sub(1);
    let max_log = if enable_ldm { 31 } else { 27 };
    log.clamp(10, max_log)
}

pub(super) fn build_zstd_stream_config(
    options: &TTZipCreateOptions,
) -> crate::codecs::zstd::types::ZstdConfig {
    let ext = crate::types::current_extended_create_options();
    let z_lvl = ext
        .raw_level
        .map(|raw| i32::from(raw).clamp(1, 22))
        .unwrap_or_else(|| map_zstd_compression_level(options.level));
    let nb_workers = if options.thread_budget > 0 {
        options.thread_budget
    } else {
        std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(4)
    };
    let window_log = zstd_window_log_from_mb(ext.dictionary_size_mb, ext.enable_ldm);
    crate::codecs::zstd::types::ZstdConfig {
        level: z_lvl,
        nb_workers,
        window_log,
        enable_ldm: ext.enable_ldm,
        enable_checksum: true,
        ..Default::default()
    }
}

static TARGET_ZSTD_CCTX_POOL: std::sync::Mutex<Vec<crate::codecs::zstd::cctx::ZstdCCtx>> =
    std::sync::Mutex::new(Vec::new());

fn acquire_target_zstd_cctx(
    config: &crate::codecs::zstd::types::ZstdConfig,
) -> Result<crate::codecs::zstd::cctx::ZstdCCtx, TTZipStatus> {
    use crate::codecs::zstd::{cctx::ZstdCCtx, types::ZstdCParameter};
    let mut cctx = TARGET_ZSTD_CCTX_POOL
        .lock()
        .ok()
        .and_then(|mut pool| pool.pop())
        .map_or_else(ZstdCCtx::new, Ok)?;
    cctx.reset_session_and_parameters()?;
    cctx.set_parameter(
        ZstdCParameter::EnableLongDistanceMatching,
        i32::from(config.enable_ldm),
    )?;
    cctx.apply_config(config)?;
    Ok(cctx)
}

fn release_target_zstd_cctx(mut cctx: crate::codecs::zstd::cctx::ZstdCCtx) {
    if cctx.reset_session_and_parameters().is_ok() {
        if let Ok(mut pool) = TARGET_ZSTD_CCTX_POOL.lock() {
            if pool.len() < 8 {
                pool.push(cctx);
            }
        }
    }
}

pub(crate) struct ZstdTargetStreamWriter<W: std::io::Write> {
    writer: Option<W>,
    cctx: Option<crate::codecs::zstd::cctx::ZstdCCtx>,
    out_buf: Vec<u8>,
    finished: bool,
}

impl<W: std::io::Write> ZstdTargetStreamWriter<W> {
    fn new(
        writer: W,
        config: &crate::codecs::zstd::types::ZstdConfig,
    ) -> Result<Self, TTZipStatus> {
        let cctx = acquire_target_zstd_cctx(config)?;
        Ok(Self {
            writer: Some(writer),
            cctx: Some(cctx),
            out_buf: vec![0u8; crate::codecs::zstd::ZSTD_STREAM_BUFFER_SIZE],
            finished: false,
        })
    }

    fn drain(
        &mut self,
        directive: crate::codecs::zstd::types::ZstdEndDirective,
    ) -> Result<(), TTZipStatus> {
        use crate::codecs::zstd::types::{ZstdInBuffer, ZstdOutBuffer};
        let cctx = self.cctx.as_mut().ok_or(TTZipStatus::ErrInvalidParam)?;
        let mut input = ZstdInBuffer {
            src: std::ptr::null(),
            size: 0,
            pos: 0,
        };
        loop {
            let mut output = ZstdOutBuffer {
                dst: self.out_buf.as_mut_ptr() as *mut libc::c_void,
                capacity: self.out_buf.len(),
                pos: 0,
            };
            let rem = cctx.compress_stream(&mut input, &mut output, directive)?;
            if output.pos > 0 {
                if let Some(w) = &mut self.writer {
                    w.write_all(&self.out_buf[..output.pos])
                        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                }
            }
            if rem == 0 {
                return Ok(());
            }
        }
    }

    pub(crate) fn finish(mut self) -> Result<W, TTZipStatus> {
        if !self.finished {
            self.drain(crate::codecs::zstd::types::ZstdEndDirective::End)?;
            if let Some(w) = &mut self.writer {
                w.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            }
            self.finished = true;
        }
        if let Some(cctx) = self.cctx.take() {
            release_target_zstd_cctx(cctx);
        }
        self.writer.take().ok_or(TTZipStatus::ErrInvalidParam)
    }
}

impl<W: std::io::Write> std::io::Write for ZstdTargetStreamWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        use crate::codecs::zstd::types::{ZstdEndDirective, ZstdInBuffer, ZstdOutBuffer};
        if buf.is_empty() {
            return Ok(0);
        }
        let cctx = self
            .cctx
            .as_mut()
            .ok_or_else(|| std::io::Error::other("missing zstd context"))?;
        let mut input = ZstdInBuffer {
            src: buf.as_ptr() as *const libc::c_void,
            size: buf.len(),
            pos: 0,
        };
        while input.pos < input.size {
            let mut output = ZstdOutBuffer {
                dst: self.out_buf.as_mut_ptr() as *mut libc::c_void,
                capacity: self.out_buf.len(),
                pos: 0,
            };
            cctx.compress_stream(&mut input, &mut output, ZstdEndDirective::Continue)
                .map_err(|_| std::io::Error::other("zstd stream compression failure"))?;
            if output.pos > 0 {
                if let Some(w) = &mut self.writer {
                    w.write_all(&self.out_buf[..output.pos])?;
                }
            }
        }
        Ok(input.pos)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if !self.finished {
            self.drain(crate::codecs::zstd::types::ZstdEndDirective::Flush)
                .map_err(|_| std::io::Error::other("zstd stream flush failure"))?;
            if let Some(w) = &mut self.writer {
                w.flush()?;
            }
        }
        Ok(())
    }
}

impl<W: std::io::Write> Drop for ZstdTargetStreamWriter<W> {
    fn drop(&mut self) {
        if !self.finished && self.writer.is_some() {
            let _ = self.drain(crate::codecs::zstd::types::ZstdEndDirective::End);
        }
        if let Some(cctx) = self.cctx.take() {
            release_target_zstd_cctx(cctx);
        }
    }
}

pub(crate) fn write_zstd_target_in_txn<F>(
    destination_path: &std::path::Path,
    split_volume_size_bytes: u64,
    options: &TTZipCreateOptions,
    raw_source: Option<&std::path::Path>,
    txn_ctx: Option<&CreateTransactionContext>,
    write_tar: F,
) -> Result<(), TTZipStatus>
where
    F: FnOnce(
        ZstdTargetStreamWriter<crate::archive::split::StreamOutputTarget>,
    ) -> Result<
        ZstdTargetStreamWriter<crate::archive::split::StreamOutputTarget>,
        TTZipStatus,
    >,
{
    let mut raw_in = match raw_source {
        Some(src_file) => {
            Some(std::fs::File::open(src_file).map_err(|_| TTZipStatus::ErrFileNotFound)?)
        }
        None => None,
    };
    let target = crate::archive::split::StreamOutputTarget::open_in_txn(
        destination_path,
        split_volume_size_bytes,
        txn_ctx,
    )
    .map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let config = build_zstd_stream_config(options);
    let mut zstd_writer = ZstdTargetStreamWriter::new(target, &config)?;
    if let Some(ref mut in_file) = raw_in {
        std::io::copy(in_file, &mut zstd_writer)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    } else {
        zstd_writer = write_tar(zstd_writer)?;
    }
    zstd_writer
        .finish()?
        .finish_and_settle_in_txn(Ok(()))
}
