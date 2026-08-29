// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe RAII-governed streaming Zstandard compressor and decompressor adapters.
//!
//! Provides standard `std::io::Write` and `std::io::Read` implementations
//! backed by a 64KB bounded intermediate buffer.

use super::cctx::ZstdCCtx;
use super::dctx::ZstdDCtx;
use super::types::*;
use crate::types::TTZipStatus;
use std::io::{Read, Write};

/// Streaming intermediate buffer size: 64KB.
pub const ZSTD_STREAM_BUFFER_SIZE: usize = 64 * 1024;

/// RAII streaming compressor implementing `std::io::Write`.
pub struct ZstdStreamWriter<W: Write> {
    writer: Option<W>,
    cctx: ZstdCCtx,
    out_buf: Vec<u8>,
    finished: bool,
}

impl<W: Write> ZstdStreamWriter<W> {
    /// Creates a new streaming compressor wrapping the given writer with custom configuration.
    pub fn new(writer: W, config: &ZstdConfig) -> Result<Self, TTZipStatus> {
        let mut cctx = ZstdCCtx::new()?;
        cctx.apply_config(config)?;
        Ok(Self {
            writer: Some(writer),
            cctx,
            out_buf: vec![0u8; ZSTD_STREAM_BUFFER_SIZE],
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
            cctx,
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
        self.writer.take().ok_or(TTZipStatus::ErrInvalidParam)
    }

    fn finish_frame(&mut self) -> Result<(), TTZipStatus> {
        if self.finished {
            return Ok(());
        }
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
            let remaining = self
                .cctx
                .compress_stream(&mut in_struct, &mut out_struct, ZstdEndDirective::End)?;
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
            self.cctx
                .compress_stream(&mut in_struct, &mut out_struct, ZstdEndDirective::Continue)
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
            let remaining = self
                .cctx
                .compress_stream(&mut in_struct, &mut out_struct, ZstdEndDirective::Flush)
                .map_err(|_| std::io::Error::other("zstd flush failure"))?;
            if out_struct.pos > 0 {
                if let Some(w) = &mut self.writer {
                    w.write_all(&self.out_buf[..out_struct.pos])?;
                }
            }
            if remaining == 0 {
                break;
            }
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
        let dctx = ZstdDCtx::new()?;
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
            if self.in_pos >= self.in_len {
                if self.eof_reached {
                    if self.total_in == 0 || self.last_ret != 0 {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "truncated or empty zstd stream",
                        ));
                    }
                    break;
                }
                let n = self.reader.read(&mut self.in_buf)?;
                if n == 0 {
                    self.eof_reached = true;
                    if self.total_in == 0 || self.last_ret != 0 {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "truncated or empty zstd stream",
                        ));
                    }
                    break;
                }
                self.total_in += n as u64;
                self.in_pos = 0;
                self.in_len = n;
            }

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
        }

        Ok(out_struct.pos)
    }
}
