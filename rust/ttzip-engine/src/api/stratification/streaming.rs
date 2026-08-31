// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Layer 3: Streaming API (Structured Cursors & Block Streams).

use std::io::{self, Read, Write};

use crate::api::stratification::simple::{simple_compress, simple_decompress};
use crate::types::{TTZipArchiveFormat, TTZipCompressionLevel, TTZipStatus};

/// Operational state cursor tracking byte progress and stream status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StreamCursor {
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub is_finished: bool,
}

/// Flush directives governing streaming pipeline execution.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum StreamFlushMode {
    #[default]
    None,
    SyncFlush,
    FullFlush,
    Finish,
}

/// Chunk-based streaming compressor adapter wrapping any `std::io::Write` sink.
pub struct StreamCompressor<W: Write> {
    sink: W,
    format: TTZipArchiveFormat,
    level: TTZipCompressionLevel,
    cursor: StreamCursor,
    buffer: Vec<u8>,
    chunk_size: usize,
}

impl<W: Write> StreamCompressor<W> {
    /// Creates a new streaming compressor wrapping the destination `sink` with default 64KB chunks.
    pub fn new(sink: W, format: TTZipArchiveFormat, level: TTZipCompressionLevel) -> Self {
        Self::with_chunk_size(sink, format, level, 64 * 1024)
    }

    /// Creates a new streaming compressor with a user-configured chunk boundary size.
    pub fn with_chunk_size(
        sink: W,
        format: TTZipArchiveFormat,
        level: TTZipCompressionLevel,
        chunk_size: usize,
    ) -> Self {
        Self {
            sink,
            format,
            level,
            cursor: StreamCursor::default(),
            buffer: Vec::with_capacity(chunk_size),
            chunk_size: chunk_size.max(512),
        }
    }

    /// Returns a shared reference to the streaming cursor progress tracker.
    #[inline]
    #[must_use]
    pub fn cursor(&self) -> &StreamCursor {
        &self.cursor
    }

    /// Writes raw input data into the streaming compressor, flushing chunks as required.
    pub fn write_chunk(&mut self, chunk: &[u8], flush: StreamFlushMode) -> Result<usize, TTZipStatus> {
        if self.cursor.is_finished {
            return Err(TTZipStatus::ErrCompressionFailed);
        }

        self.buffer.extend_from_slice(chunk);
        self.cursor.bytes_in += chunk.len() as u64;

        if self.buffer.len() >= self.chunk_size || flush != StreamFlushMode::None {
            self.flush_internal_buffer()?;
        }

        if flush == StreamFlushMode::Finish {
            self.cursor.is_finished = true;
        }

        Ok(chunk.len())
    }

    fn flush_internal_buffer(&mut self) -> Result<(), TTZipStatus> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let compressed = simple_compress(&self.buffer, self.format, self.level)?;
        self.sink
            .write_all(&compressed)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        self.cursor.bytes_out += compressed.len() as u64;
        self.buffer.clear();
        Ok(())
    }

    /// Completes stream processing, flushes all remaining buffered chunks, and returns the inner sink.
    pub fn finish(mut self) -> Result<W, TTZipStatus> {
        if !self.cursor.is_finished {
            self.flush_internal_buffer()?;
            self.cursor.is_finished = true;
        }
        self.sink.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        Ok(self.sink)
    }

    /// Returns an immutable reference to the underlying sink.
    #[inline]
    pub fn get_ref(&self) -> &W {
        &self.sink
    }

    /// Returns a mutable reference to the underlying sink.
    #[inline]
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.sink
    }
}

impl<W: Write> Write for StreamCompressor<W> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write_chunk(buf, StreamFlushMode::None)
            .map_err(|s| io::Error::other(s.as_str()))
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.flush_internal_buffer()
            .map_err(|s| io::Error::other(s.as_str()))?;
        self.sink.flush()
    }
}

/// Chunk-based streaming decompressor adapter wrapping any `std::io::Read` source.
pub struct StreamDecompressor<R: Read> {
    source: R,
    format: TTZipArchiveFormat,
    cursor: StreamCursor,
    in_buffer: Vec<u8>,
    out_buffer: Vec<u8>,
    out_offset: usize,
}

impl<R: Read> StreamDecompressor<R> {
    /// Creates a new streaming decompressor wrapping the given `source`.
    pub fn new(source: R, format: TTZipArchiveFormat) -> Self {
        Self {
            source,
            format,
            cursor: StreamCursor::default(),
            in_buffer: vec![0u8; 64 * 1024],
            out_buffer: Vec::new(),
            out_offset: 0,
        }
    }

    /// Returns a shared reference to the streaming cursor.
    #[inline]
    #[must_use]
    pub fn cursor(&self) -> &StreamCursor {
        &self.cursor
    }

    /// Returns `true` if decompression has completed all input data.
    #[inline]
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.cursor.is_finished
    }

    /// Reads decompressed bytes into the caller-provided destination slice.
    pub fn read_chunk(&mut self, dst: &mut [u8]) -> Result<usize, TTZipStatus> {
        if dst.is_empty() {
            return Ok(0);
        }

        // Deliver existing decompressed bytes from cache
        if self.out_offset < self.out_buffer.len() {
            let available = &self.out_buffer[self.out_offset..];
            let to_copy = available.len().min(dst.len());
            dst[..to_copy].copy_from_slice(&available[..to_copy]);
            self.out_offset += to_copy;
            self.cursor.bytes_out += to_copy as u64;
            return Ok(to_copy);
        }

        if self.cursor.is_finished {
            return Ok(0);
        }

        // Read next compressed chunk from input source
        let n = self
            .source
            .read(&mut self.in_buffer)
            .map_err(|_| TTZipStatus::ErrExtractionFailed)?;

        if n == 0 {
            self.cursor.is_finished = true;
            return Ok(0);
        }

        self.cursor.bytes_in += n as u64;
        let decompressed = simple_decompress(&self.in_buffer[..n], self.format)?;
        self.out_buffer = decompressed;
        self.out_offset = 0;

        let to_copy = self.out_buffer.len().min(dst.len());
        dst[..to_copy].copy_from_slice(&self.out_buffer[..to_copy]);
        self.out_offset = to_copy;
        self.cursor.bytes_out += to_copy as u64;
        Ok(to_copy)
    }

    /// Returns an immutable reference to the underlying reader source.
    #[inline]
    pub fn get_ref(&self) -> &R {
        &self.source
    }

    /// Unwraps and returns the underlying reader source.
    #[inline]
    pub fn into_inner(self) -> R {
        self.source
    }
}

impl<R: Read> Read for StreamDecompressor<R> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_chunk(buf)
            .map_err(|s| io::Error::other(s.as_str()))
    }
}
