// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Safe RAII-governed streaming Zstandard compressor and decompressor adapters.
//!
//! Provides standard `std::io::Write` and `std::io::Read` implementations
//! backed by a 64KB bounded intermediate buffer.

use super::cctx::{with_thread_local_zstd_cctx, zstd_compress_bound, ZstdCCtx};
use super::dctx::ZstdDCtx;
use super::types::*;
use crate::types::TTZipStatus;
use rayon::prelude::*;
use std::cell::RefCell;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Streaming intermediate buffer size: 64KB.
pub const ZSTD_STREAM_BUFFER_SIZE: usize = 64 * 1024;
const ZSTD_MT_BATCH_SIZE: usize = 4 * 1024 * 1024;
const ZSTD_MT_MIN_PARALLEL_BYTES: usize = 256 * 1024;

static STREAM_CCTX_POOL: Mutex<Vec<(u32, ZstdCCtx)>> = Mutex::new(Vec::new());
static STREAM_MT_BUF_POOL: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());
static WARM_MT_POOL: AtomicBool = AtomicBool::new(false);

thread_local! {
    static TLS_MT_CHUNK_DST: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn acquire_stream_cctx(nb_workers: u32) -> Result<ZstdCCtx, TTZipStatus> {
    if let Ok(mut pool) = STREAM_CCTX_POOL.lock() {
        if let Some(pos) = pool.iter().rposition(|(w, _)| *w == nb_workers) {
            let (_, mut cctx) = pool.swap_remove(pos);
            if cctx.reset_session_and_parameters().is_ok() {
                return Ok(cctx);
            }
        }
    }
    ZstdCCtx::new()
}

fn release_stream_cctx(nb_workers: u32, mut cctx: ZstdCCtx) {
    if cctx.reset_session_and_parameters().is_ok() {
        if let Ok(mut pool) = STREAM_CCTX_POOL.lock() {
            if pool.len() < 8 {
                pool.push((nb_workers, cctx));
            }
        }
    }
}

fn acquire_mt_buf() -> Vec<u8> {
    if let Ok(mut pool) = STREAM_MT_BUF_POOL.lock() {
        if let Some(mut buf) = pool.pop() {
            buf.clear();
            return buf;
        }
    }
    Vec::with_capacity(ZSTD_MT_BATCH_SIZE)
}

fn release_mt_buf(mut buf: Vec<u8>) {
    if buf.capacity() >= ZSTD_MT_MIN_PARALLEL_BYTES && buf.capacity() <= ZSTD_MT_BATCH_SIZE * 2 {
        buf.clear();
        if let Ok(mut pool) = STREAM_MT_BUF_POOL.lock() {
            if pool.len() < 4 {
                pool.push(buf);
            }
        }
    }
}

fn compress_chunk_frame(chunk: &[u8], level: i32, enable_checksum: bool) -> Result<Vec<u8>, TTZipStatus> {
    let bound = zstd_compress_bound(chunk.len());
    TLS_MT_CHUNK_DST.with(|cell| {
        let mut dst = cell.borrow_mut();
        if dst.len() < bound {
            dst.resize(bound, 0);
        }
        let n = with_thread_local_zstd_cctx(|cctx| {
            cctx.reset()?;
            cctx.set_parameter(ZstdCParameter::CompressionLevel, level)?;
            cctx.set_parameter(
                ZstdCParameter::ChecksumFlag,
                if enable_checksum { 1 } else { 0 },
            )?;
            cctx.compress2(chunk, &mut dst[..bound])
        })?;
        Ok(dst[..n].to_vec())
    })
}

fn try_claim_mt_pool_warmup(flag: &AtomicBool) -> bool {
    if flag.load(Ordering::Acquire) || rayon::current_thread_index().is_some() {
        return false;
    }
    flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
}

fn ensure_mt_pool_warmed() {
    if try_claim_mt_pool_warmup(&WARM_MT_POOL) {
        let mut warm_buf = Vec::with_capacity(ZSTD_MT_BATCH_SIZE);
        warm_buf.resize(1024 * 1024, 0);
        release_mt_buf(warm_buf);
        let threads = rayon::current_num_threads().max(2);
        (0..threads).into_par_iter().for_each(|_| {
            let _ = compress_chunk_frame(&[0u8; 128 * 1024], 3, true);
        });
    }
}

/// RAII streaming compressor implementing `std::io::Write`.
pub struct ZstdStreamWriter<W: Write> {
    writer: Option<W>,
    cctx: Option<ZstdCCtx>,
    nb_workers: u32,
    level: i32,
    enable_checksum: bool,
    mt_buf: Vec<u8>,
    mt_emitted: bool,
    out_buf: Vec<u8>,
    finished: bool,
}

impl<W: Write> ZstdStreamWriter<W> {
    /// Creates a new streaming compressor wrapping the given writer with custom configuration.
    pub fn new(writer: W, config: &ZstdConfig) -> Result<Self, TTZipStatus> {
        let use_mt_batch = config.nb_workers > 1 && !config.enable_ldm && config.window_log == 0;
        if use_mt_batch {
            ensure_mt_pool_warmed();
        }
        let cctx = if use_mt_batch {
            None
        } else {
            let mut ctx = acquire_stream_cctx(config.nb_workers)?;
            ctx.apply_config(config)?;
            Some(ctx)
        };
        Ok(Self {
            writer: Some(writer),
            cctx,
            nb_workers: config.nb_workers,
            level: config.level,
            enable_checksum: config.enable_checksum,
            mt_buf: if use_mt_batch { acquire_mt_buf() } else { Vec::new() },
            mt_emitted: false,
            out_buf: if use_mt_batch {
                Vec::new()
            } else {
                vec![0u8; ZSTD_STREAM_BUFFER_SIZE]
            },
            finished: false,
        })
    }

    /// Creates a new streaming compressor with a specific compression level.
    pub fn with_level(writer: W, level: i32) -> Result<Self, TTZipStatus> {
        let config = ZstdConfig {
            level,
            ..Default::default()
        };
        Self::new(writer, &config)
    }

    /// Creates a new streaming compressor with pre-digested `CDict`.
    pub fn with_cdict(writer: W, cdict: &super::dict::CDict) -> Result<Self, TTZipStatus> {
        let mut cctx = ZstdCCtx::new()?;
        cctx.ref_cdict_raw(cdict.as_ptr())?;
        Ok(Self {
            writer: Some(writer),
            cctx: Some(cctx),
            nb_workers: 0,
            level: 3,
            enable_checksum: false,
            mt_buf: Vec::new(),
            mt_emitted: false,
            out_buf: vec![0u8; ZSTD_STREAM_BUFFER_SIZE],
            finished: false,
        })
    }

    /// Creates a new streaming compressor with a high-level `ZstdDictionary`.
    pub fn with_dict(writer: W, dict: &super::dict::ZstdDictionary) -> Result<Self, TTZipStatus> {
        Self::with_cdict(writer, dict.cdict())
    }

    /// Flushes all pending data, finalizes the Zstandard frame, and returns the underlying writer.
    pub fn finish(mut self) -> Result<W, TTZipStatus> {
        self.finish_frame()?;
        if let Some(cctx) = self.cctx.take() {
            release_stream_cctx(self.nb_workers, cctx);
        } else {
            release_mt_buf(std::mem::take(&mut self.mt_buf));
        }
        self.writer.take().ok_or(TTZipStatus::ErrInvalidParam)
    }

    fn flush_mt_batch(&mut self, force_empty_frame: bool) -> Result<(), TTZipStatus> {
        if self.mt_buf.is_empty() && !force_empty_frame {
            return Ok(());
        }
        let level = self.level;
        let checksum = self.enable_checksum;
        if self.mt_buf.len() >= ZSTD_MT_MIN_PARALLEL_BYTES && self.nb_workers > 1 {
            let par_chunks = (self.nb_workers as usize).clamp(2, 8);
            let chunk_size = self.mt_buf.len().div_ceil(par_chunks).max(128 * 1024);
            let frames: Result<Vec<Vec<u8>>, TTZipStatus> = self
                .mt_buf
                .par_chunks(chunk_size)
                .map(|chunk| compress_chunk_frame(chunk, level, checksum))
                .collect();
            let frames = frames?;
            if let Some(w) = &mut self.writer {
                for frame in &frames {
                    w.write_all(frame).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                }
            }
        } else {
            let frame = compress_chunk_frame(&self.mt_buf, level, checksum)?;
            if let Some(w) = &mut self.writer {
                w.write_all(&frame).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            }
        }
        self.mt_buf.clear();
        self.mt_emitted = true;
        Ok(())
    }

    fn drain_cctx_with_directive(&mut self, directive: ZstdEndDirective) -> Result<(), TTZipStatus> {
        let cctx = self.cctx.as_mut().ok_or(TTZipStatus::ErrInvalidParam)?;
        let mut in_struct = ZstdInBuffer {
            src: std::ptr::null(),
            size: 0,
            pos: 0,
        };
        loop {
            let mut out_struct = ZstdOutBuffer {
                dst: self.out_buf.as_mut_ptr() as *mut libc::c_void,
                capacity: self.out_buf.len(),
                pos: 0,
            };
            let remaining = cctx.compress_stream(&mut in_struct, &mut out_struct, directive)?;
            if out_struct.pos > 0 {
                if let Some(w) = &mut self.writer {
                    w.write_all(&self.out_buf[..out_struct.pos])
                        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                }
            }
            if remaining == 0 {
                break;
            }
        }
        Ok(())
    }

    fn finish_frame(&mut self) -> Result<(), TTZipStatus> {
        if self.finished {
            return Ok(());
        }
        if self.cctx.is_none() {
            self.flush_mt_batch(!self.mt_emitted)?;
        } else {
            self.drain_cctx_with_directive(ZstdEndDirective::End)?;
        }
        if let Some(w) = &mut self.writer {
            w.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        }
        self.finished = true;
        Ok(())
    }
}

impl<W: Write> Write for ZstdStreamWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.finished {
            return Err(std::io::Error::other("cannot write to finished zstd stream"));
        }
        if self.cctx.is_none() {
            let mut remaining = buf;
            while !remaining.is_empty() {
                let space = ZSTD_MT_BATCH_SIZE.saturating_sub(self.mt_buf.len());
                let take = remaining.len().min(space);
                self.mt_buf.extend_from_slice(&remaining[..take]);
                remaining = &remaining[take..];
                if self.mt_buf.len() >= ZSTD_MT_BATCH_SIZE {
                    self.flush_mt_batch(false)
                        .map_err(|_| std::io::Error::other("zstd mt batch compression failure"))?;
                }
            }
            return Ok(buf.len());
        }
        let cctx = self
            .cctx
            .as_mut()
            .ok_or_else(|| std::io::Error::other("missing zstd context"))?;
        let mut in_struct = ZstdInBuffer {
            src: buf.as_ptr() as *const libc::c_void,
            size: buf.len(),
            pos: 0,
        };
        while in_struct.pos < in_struct.size {
            let mut out_struct = ZstdOutBuffer {
                dst: self.out_buf.as_mut_ptr() as *mut libc::c_void,
                capacity: self.out_buf.len(),
                pos: 0,
            };
            cctx.compress_stream(&mut in_struct, &mut out_struct, ZstdEndDirective::Continue)
                .map_err(|_| std::io::Error::other("zstd compression failure"))?;
            if out_struct.pos > 0 {
                if let Some(w) = &mut self.writer {
                    w.write_all(&self.out_buf[..out_struct.pos])?;
                }
            }
        }
        Ok(in_struct.pos)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.finished {
            return Ok(());
        }
        if self.cctx.is_none() {
            self.flush_mt_batch(false)
                .map_err(|_| std::io::Error::other("zstd mt flush failure"))?;
        } else {
            self.drain_cctx_with_directive(ZstdEndDirective::Flush)
                .map_err(|_| std::io::Error::other("zstd flush failure"))?;
        }
        if let Some(w) = &mut self.writer {
            w.flush()?;
        }
        Ok(())
    }
}

impl<W: Write> Drop for ZstdStreamWriter<W> {
    fn drop(&mut self) {
        if !self.finished && self.writer.is_some() {
            let _ = self.finish_frame();
        }
        if let Some(cctx) = self.cctx.take() {
            release_stream_cctx(self.nb_workers, cctx);
        } else {
            release_mt_buf(std::mem::take(&mut self.mt_buf));
        }
    }
}

/// RAII streaming decompressor implementing `std::io::Read`.
pub struct ZstdStreamReader<R: Read> {
    reader: R,
    dctx: ZstdDCtx,
    in_buf: Vec<u8>,
    in_pos: usize,
    in_len: usize,
    eof_reached: bool,
    last_ret: usize,
    total_in: u64,
}

impl<R: Read> ZstdStreamReader<R> {
    /// Creates a new streaming decompressor wrapping the given reader.
    pub fn new(reader: R) -> Result<Self, TTZipStatus> {
        let mut dctx = ZstdDCtx::new()?;
        dctx.set_max_window_log(31)?;
        Ok(Self {
            reader,
            dctx,
            in_buf: vec![0u8; ZSTD_STREAM_BUFFER_SIZE],
            in_pos: 0,
            in_len: 0,
            eof_reached: false,
            last_ret: 0,
            total_in: 0,
        })
    }

    /// Creates a new streaming decompressor supporting large LDM windows (up to 2GB).
    pub fn with_max_window_log(reader: R, max_window_log: u32) -> Result<Self, TTZipStatus> {
        let mut dctx = ZstdDCtx::new()?;
        dctx.set_max_window_log(max_window_log)?;
        Ok(Self {
            reader,
            dctx,
            in_buf: vec![0u8; ZSTD_STREAM_BUFFER_SIZE],
            in_pos: 0,
            in_len: 0,
            eof_reached: false,
            last_ret: 0,
            total_in: 0,
        })
    }

    /// Creates a new streaming decompressor with pre-digested `DDict`.
    pub fn with_ddict(reader: R, ddict: &super::dict::DDict) -> Result<Self, TTZipStatus> {
        let mut dctx = ZstdDCtx::new()?;
        dctx.ref_ddict_raw(ddict.as_ptr())?;
        Ok(Self {
            reader,
            dctx,
            in_buf: vec![0u8; ZSTD_STREAM_BUFFER_SIZE],
            in_pos: 0,
            in_len: 0,
            eof_reached: false,
            last_ret: 0,
            total_in: 0,
        })
    }

    /// Creates a new streaming decompressor with a high-level `ZstdDictionary`.
    pub fn with_dict(reader: R, dict: &super::dict::ZstdDictionary) -> Result<Self, TTZipStatus> {
        Self::with_ddict(reader, dict.ddict())
    }

    /// Consumes the wrapper and returns the underlying reader.
    pub fn into_inner(self) -> R {
        self.reader
    }

    /// Returns an immutable reference to the underlying reader.
    pub fn get_ref(&self) -> &R {
        &self.reader
    }

    /// Returns a mutable reference to the underlying reader.
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.reader
    }

    fn verify_clean_eof(&self) -> std::io::Result<()> {
        if self.total_in == 0 || self.last_ret != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "truncated or empty zstd stream",
            ));
        }
        Ok(())
    }

    fn ensure_input_available(&mut self) -> std::io::Result<bool> {
        if self.in_pos < self.in_len {
            return Ok(true);
        }
        if self.eof_reached {
            self.verify_clean_eof()?;
            return Ok(false);
        }
        let n = self.reader.read(&mut self.in_buf)?;
        if n == 0 {
            self.eof_reached = true;
            self.verify_clean_eof()?;
            return Ok(false);
        }
        self.total_in += n as u64;
        self.in_pos = 0;
        self.in_len = n;
        Ok(true)
    }

    fn compact_and_refill_input(&mut self) -> std::io::Result<()> {
        if self.eof_reached {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "unexpected eof in zstd stream: decoder needs more input to progress",
            ));
        }
        if self.in_pos > 0 {
            let unconsumed = self.in_len - self.in_pos;
            self.in_buf.copy_within(self.in_pos..self.in_len, 0);
            self.in_pos = 0;
            self.in_len = unconsumed;
        }
        if self.in_len < self.in_buf.len() {
            let n = self.reader.read(&mut self.in_buf[self.in_len..])?;
            if n == 0 {
                self.eof_reached = true;
            } else {
                self.total_in += n as u64;
                self.in_len += n;
            }
        }
        Ok(())
    }
}

impl<R: Read> Read for ZstdStreamReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let mut out_struct = ZstdOutBuffer {
            dst: buf.as_mut_ptr() as *mut libc::c_void,
            capacity: buf.len(),
            pos: 0,
        };

        while out_struct.pos == 0 {
            if !self.ensure_input_available()? {
                break;
            }

            let prev_in_pos = self.in_pos;
            let mut in_struct = ZstdInBuffer {
                src: self.in_buf.as_ptr() as *const libc::c_void,
                size: self.in_len,
                pos: self.in_pos,
            };

            let res = self
                .dctx
                .decompress_stream(&mut in_struct, &mut out_struct)
                .map_err(|e| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("zstd decompression error: {:?}", e),
                    )
                })?;

            self.in_pos = in_struct.pos;
            self.last_ret = res;

            if out_struct.pos > 0 {
                break;
            }

            if self.in_pos == prev_in_pos {
                self.compact_and_refill_input()?;
            }
        }

        Ok(out_struct.pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zstd::dict::ZstdDictionary;
    use std::io::Cursor;

    #[test]
    fn test_pooled_stream_cctx_after_with_dict_does_not_leak_dictionary() {
        if let Ok(mut pool) = STREAM_CCTX_POOL.lock() {
            pool.clear();
        }
        let payload = b"dictionary-matching structured stream payload 0123456789abcdef".repeat(16);
        {
            let dict_bytes =
                b"dictionary-matching structured stream payload 0123456789abcdef".repeat(32);
            let dict = ZstdDictionary::from_bytes("pooled_dict", dict_bytes, 3)
                .expect("create dictionary");
            let mut dict_out = Vec::new();
            let mut dict_writer =
                ZstdStreamWriter::with_dict(&mut dict_out, &dict).expect("with_dict writer");
            dict_writer.write_all(&payload).expect("write dict payload");
            dict_writer.finish().expect("finish dict writer");
        }

        let mut plain_out = Vec::new();
        let mut plain_writer =
            ZstdStreamWriter::with_level(&mut plain_out, 3).expect("with_level writer");
        plain_writer.write_all(&payload).expect("write plain payload");
        plain_writer.finish().expect("finish plain writer");

        let mut reader =
            ZstdStreamReader::new(Cursor::new(&plain_out)).expect("plain stream reader");
        let mut decomp = Vec::new();
        reader
            .read_to_end(&mut decomp)
            .expect("dictionary-free reader must decode output from pooled cctx");
        assert_eq!(decomp, payload);
    }

    #[test]
    fn test_mt_stream_writer_bounds_mt_buf_on_large_single_write() {
        let config = ZstdConfig {
            level: 1,
            nb_workers: 2,
            ..Default::default()
        };
        let mut writer = ZstdStreamWriter::new(Vec::new(), &config).expect("mt stream writer");
        let large_len = ZSTD_MT_BATCH_SIZE * 2 + 512 * 1024;
        let payload: Vec<u8> = (0..large_len).map(|i| ((i * 17 + 5) & 0xff) as u8).collect();

        let written = writer.write(&payload).expect("single large write");
        assert_eq!(written, payload.len());
        assert_eq!(
            writer.mt_buf.len(),
            512 * 1024,
            "mt_buf must only retain the trailing sub-batch remainder"
        );
        assert!(
            writer.mt_buf.capacity() <= ZSTD_MT_BATCH_SIZE,
            "mt_buf capacity ({}) must not exceed ZSTD_MT_BATCH_SIZE ({})",
            writer.mt_buf.capacity(),
            ZSTD_MT_BATCH_SIZE
        );
        assert!(
            !writer.writer.as_ref().expect("inner writer").is_empty(),
            "full 4 MiB batches must be flushed during write()"
        );

        let compressed = writer.finish().expect("finish mt stream writer");
        let mut reader = ZstdStreamReader::new(Cursor::new(&compressed)).expect("stream reader");
        let mut decomp = Vec::new();
        reader.read_to_end(&mut decomp).expect("decompress mt stream");
        assert_eq!(decomp, payload);
    }

    #[test]
    fn test_mt_pool_warmup_skips_rayon_workers_and_never_blocks() {
        let local_flag = AtomicBool::new(false);
        let worker_claims: usize = (0..4usize)
            .into_par_iter()
            .map(|_| usize::from(try_claim_mt_pool_warmup(&local_flag)))
            .sum();
        assert_eq!(
            worker_claims, 0,
            "Rayon worker threads must never claim or block on warmup"
        );
        assert!(!local_flag.load(Ordering::Acquire));

        assert!(try_claim_mt_pool_warmup(&local_flag));
        assert!(!try_claim_mt_pool_warmup(&local_flag));

        let config = ZstdConfig {
            level: 1,
            nb_workers: 2,
            ..Default::default()
        };
        let outputs: Vec<Vec<u8>> = (0..4usize)
            .into_par_iter()
            .map(|i| {
                let mut writer =
                    ZstdStreamWriter::new(Vec::new(), &config).expect("par mt writer");
                let msg = format!("concurrent rayon mt stream writer {i}").repeat(32);
                writer.write_all(msg.as_bytes()).expect("write msg");
                writer.finish().expect("finish msg")
            })
            .collect();
        assert_eq!(outputs.len(), 4);
    }
}
