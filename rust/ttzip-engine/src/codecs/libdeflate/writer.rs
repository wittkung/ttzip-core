// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, high-throughput streaming compressor for DEFLATE, Zlib, and Gzip streams.
//!
//! Provides [`LibdeflateWriter`], an adapter implementing [`std::io::Write`] with
//! bounded 64KB micro-buffering, multi-block streaming compression, RFC 1950 (Zlib) / RFC 1952 (Gzip)
//! framing headers/trailers, and deterministic flushing on [`LibdeflateWriter::finish`].

use super::checksum::{adler32_update, crc32_update};
use super::container::ContainerFormat;
use crate::types::TTZipStatus;
use flate2::{Compress, Compression, FlushCompress, Status};
use std::io::{self, Error, ErrorKind, Write};

/// Default internal micro-buffer capacity for streaming DEFLATE compression (64 KB).
pub const DEFAULT_COMPRESS_CHUNK_SIZE: usize = 64 * 1024;

/// High-throughput streaming compressor for DEFLATE, Zlib, and Gzip containers.
///
/// Implements [`std::io::Write`] with bounded 64KB micro-buffering, automatic multi-block
/// emission, on-the-fly checksum accumulation, and deterministic flush/finish semantics.
pub struct LibdeflateWriter<W: Write> {
    /// Underlying destination byte stream.
    inner: Option<W>,
    /// Container framing format.
    format: ContainerFormat,
    /// Effective compression level (0..=12 clamped to flate2 0..=9).
    level: i32,
    /// Streaming compression engine.
    compressor: Compress,
    /// Internal micro-buffer accumulating uncompressed input bytes.
    chunk_buf: Vec<u8>,
    /// Chunk threshold size (typically 64KB).
    chunk_size: usize,
    /// Output buffer for compressed chunk emission.
    out_buf: Vec<u8>,
    /// Flag tracking whether container header has been emitted.
    header_written: bool,
    /// Flag tracking whether stream was finalized via [`finish`](Self::finish).
    finished: bool,
    /// Running Adler-32 checksum of uncompressed data (RFC 1950).
    adler: u32,
    /// Running IEEE 802.3 CRC-32 checksum of uncompressed data (RFC 1952).
    crc: u32,
    /// Total number of uncompressed bytes written to this stream.
    total_in: u64,
}

impl<W: Write> LibdeflateWriter<W> {
    /// Creates a new `LibdeflateWriter` wrapping `writer` with the given `format` and compression `level` (0..=12).
    pub fn new(writer: W, format: ContainerFormat, level: i32) -> Result<Self, TTZipStatus> {
        Self::with_chunk_size(writer, format, level, DEFAULT_COMPRESS_CHUNK_SIZE)
    }

    /// Creates a new `LibdeflateWriter` with a customized chunk micro-buffer size.
    pub fn with_chunk_size(
        writer: W,
        format: ContainerFormat,
        level: i32,
        chunk_size: usize,
    ) -> Result<Self, TTZipStatus> {
        let valid_level = if level < 0 { 6 } else { level.clamp(0, 12) };
        let flate_level = valid_level.clamp(0, 9) as u32;
        let actual_chunk_size = chunk_size.max(512);

        Ok(Self {
            inner: Some(writer),
            format,
            level: valid_level,
            compressor: Compress::new(Compression::new(flate_level), false),
            chunk_buf: Vec::with_capacity(actual_chunk_size),
            chunk_size: actual_chunk_size,
            out_buf: vec![0u8; actual_chunk_size],
            header_written: false,
            finished: false,
            adler: 1,
            crc: 0,
            total_in: 0,
        })
    }

    /// Creates a new `LibdeflateWriter` configured for raw RFC 1951 DEFLATE streams.
    #[inline]
    pub fn new_raw(writer: W, level: i32) -> Result<Self, TTZipStatus> {
        Self::new(writer, ContainerFormat::Raw, level)
    }

    /// Creates a new `LibdeflateWriter` configured for RFC 1950 Zlib container streams.
    #[inline]
    pub fn new_zlib(writer: W, level: i32) -> Result<Self, TTZipStatus> {
        Self::new(writer, ContainerFormat::Zlib, level)
    }

    /// Creates a new `LibdeflateWriter` configured for RFC 1952 Gzip container streams.
    #[inline]
    pub fn new_gzip(writer: W, level: i32) -> Result<Self, TTZipStatus> {
        Self::new(writer, ContainerFormat::Gzip, level)
    }

    /// Returns the container framing format of this writer.
    #[inline]
    pub fn format(&self) -> ContainerFormat {
        self.format
    }

    /// Returns the total number of uncompressed bytes written to this stream.
    #[inline]
    pub fn total_in(&self) -> u64 {
        self.total_in
    }

    /// Returns the current running Adler-32 checksum of the uncompressed data.
    #[inline]
    pub fn checksum_adler32(&self) -> u32 {
        self.adler
    }

    /// Returns the current running CRC-32 checksum of the uncompressed data.
    #[inline]
    pub fn checksum_crc32(&self) -> u32 {
        self.crc
    }

    /// Gets a shared reference to the underlying writer, if not yet finalized.
    #[inline]
    pub fn get_ref(&self) -> Option<&W> {
        self.inner.as_ref()
    }

    /// Gets a mutable reference to the underlying writer, if not yet finalized.
    #[inline]
    pub fn get_mut(&mut self) -> Option<&mut W> {
        self.inner.as_mut()
    }

    // MARK: - Internal Emission Methods

    /// Emits the container header according to `self.format`.
    fn write_header(&mut self) -> io::Result<()> {
        if self.header_written {
            return Ok(());
        }

        let inner = self
            .inner
            .as_mut()
            .ok_or_else(|| Error::other("writer already closed"))?;

        match self.format {
            ContainerFormat::Raw => {}
            ContainerFormat::Zlib => {
                let cmf = 0x78u8; // CM=8 (DEFLATE), CINFO=7 (32KB window)
                let flg = match self.level {
                    0 | 1 => 0x01u8,
                    2..=5 => 0x5Eu8,
                    6..=8 => 0x9Cu8,
                    _ => 0xDAu8,
                };
                inner.write_all(&[cmf, flg])?;
            }
            ContainerFormat::Gzip => {
                let xfl = if self.level >= 9 {
                    2u8
                } else if self.level <= 2 {
                    4u8
                } else {
                    0u8
                };
                // RFC 1952 fixed 10-byte header: ID1=1F, ID2=8B, CM=8, FLG=0, MTIME=0, XFL=xfl, OS=255 (unknown)
                let header = [0x1Fu8, 0x8Bu8, 0x08u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, 0x00u8, xfl, 0xFFu8];
                inner.write_all(&header)?;
            }
        }

        self.header_written = true;
        Ok(())
    }

    /// Compresses and flushes buffered chunks into the underlying writer.
    fn flush_chunk_internal(&mut self, flush: FlushCompress) -> io::Result<()> {
        self.write_header()?;

        let mut offset = 0;
        loop {
            let in_slice = &self.chunk_buf[offset..];
            let prev_in = self.compressor.total_in();
            let prev_out = self.compressor.total_out();

            let status = self.compressor.compress(
                in_slice,
                &mut self.out_buf[..],
                flush,
            ).map_err(|e| {
                Error::other(format!("compression stream error: {e:?}"))
            })?;

            let in_consumed = (self.compressor.total_in() - prev_in) as usize;
            let out_produced = (self.compressor.total_out() - prev_out) as usize;
            offset += in_consumed;

            if out_produced > 0 {
                if let Some(ref mut inner) = self.inner {
                    inner.write_all(&self.out_buf[..out_produced])?;
                }
            }

            if flush == FlushCompress::Finish {
                if status == Status::StreamEnd {
                    break;
                }
            } else if offset >= self.chunk_buf.len() && out_produced < self.out_buf.len() {
                break;
            }
        }

        self.chunk_buf.clear();
        Ok(())
    }

    /// Internal helper finalizing stream trailer and flushing inner writer.
    fn finish_internal(&mut self) -> io::Result<()> {
        if self.finished {
            return Ok(());
        }

        self.write_header()?;
        self.flush_chunk_internal(FlushCompress::Finish)?;

        let inner = self
            .inner
            .as_mut()
            .ok_or_else(|| Error::other("writer already closed"))?;

        // Emit container trailers
        match self.format {
            ContainerFormat::Raw => {}
            ContainerFormat::Zlib => {
                // 4-byte Adler-32 in big-endian (RFC 1950)
                let adler_bytes = self.adler.to_be_bytes();
                inner.write_all(&adler_bytes)?;
            }
            ContainerFormat::Gzip => {
                // 4-byte CRC-32 (LE) + 4-byte ISIZE (LE) (RFC 1952)
                let crc_bytes = self.crc.to_le_bytes();
                let isize_bytes = (self.total_in as u32).to_le_bytes();
                inner.write_all(&crc_bytes)?;
                inner.write_all(&isize_bytes)?;
            }
        }

        inner.flush()?;
        self.finished = true;
        Ok(())
    }

    /// Finalizes the compression stream, writes all trailers, flushes, and returns the underlying writer.
    pub fn finish(mut self) -> io::Result<W> {
        self.finish_internal()?;
        self.inner
            .take()
            .ok_or_else(|| Error::other("inner writer already consumed"))
    }

    /// Consumes this `LibdeflateWriter`, finalizing the stream and returning the underlying writer.
    #[inline]
    pub fn into_inner(self) -> io::Result<W> {
        self.finish()
    }
}

// MARK: - std::io::Write Implementation

impl<W: Write> Write for LibdeflateWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.finished {
            return Err(Error::new(
                ErrorKind::WriteZero,
                "cannot write to finished LibdeflateWriter",
            ));
        }
        if buf.is_empty() {
            return Ok(0);
        }

        // On-the-fly uncompressed checksum updates
        self.adler = adler32_update(self.adler, buf);
        self.crc = crc32_update(self.crc, buf);
        self.total_in = self.total_in.wrapping_add(buf.len() as u64);

        let mut offset = 0;
        while offset < buf.len() {
            let available = self.chunk_size.saturating_sub(self.chunk_buf.len());
            let to_copy = (buf.len() - offset).min(available);
            self.chunk_buf.extend_from_slice(&buf[offset..offset + to_copy]);
            offset += to_copy;

            if self.chunk_buf.len() >= self.chunk_size {
                self.flush_chunk_internal(FlushCompress::None)?;
            }
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.chunk_buf.is_empty() {
            self.flush_chunk_internal(FlushCompress::Sync)?;
        }
        if let Some(ref mut inner) = self.inner {
            inner.flush()
        } else {
            Ok(())
        }
    }
}

// MARK: - Drop Safety Implementation

impl<W: Write> Drop for LibdeflateWriter<W> {
    fn drop(&mut self) {
        if !self.finished && self.inner.is_some() {
            let _ = self.finish_internal();
        }
    }
}
