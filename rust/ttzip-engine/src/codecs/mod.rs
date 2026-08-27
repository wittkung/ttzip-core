// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, RAII-governed single-format compression and character encoding codecs.

pub mod brotli;
pub mod chardet;
pub mod deflate;
pub mod fast_blocks;
pub mod lzma;
pub mod lzma2;
pub mod snappy;
pub mod zstd;

// Safe RAII stream byte counting wrappers

use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Zero-overhead counting writer that wraps an underlying `Write` sink and tracks total bytes written.
pub struct CountingWriter<W: Write> {
    inner: W,
    count: Arc<AtomicU64>,
}

impl<W: Write> CountingWriter<W> {
    /// Creates a new counting writer wrapping `inner` with initial count 0.
    #[inline]
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Returns a shared handle to the atomic byte counter.
    #[inline]
    pub fn counter(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.count)
    }

    /// Returns total bytes written through this writer.
    #[inline]
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Unwraps and returns the underlying writer.
    #[inline]
    pub fn into_inner(self) -> W {
        self.inner
    }

    /// Returns an immutable reference to the underlying writer.
    #[inline]
    pub fn get_ref(&self) -> &W {
        &self.inner
    }

    /// Returns a mutable reference to the underlying writer.
    #[inline]
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }
}

impl<W: Write> Write for CountingWriter<W> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.count.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Zero-overhead counting reader that wraps an underlying `Read` source and tracks total bytes read.
pub struct CountingReader<R: Read> {
    inner: R,
    count: Arc<AtomicU64>,
}

impl<R: Read> CountingReader<R> {
    /// Creates a new counting reader wrapping `inner` with initial count 0.
    #[inline]
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Returns a shared handle to the atomic byte counter.
    #[inline]
    pub fn counter(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.count)
    }

    /// Returns total bytes read through this reader.
    #[inline]
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Unwraps and returns the underlying reader.
    #[inline]
    pub fn into_inner(self) -> R {
        self.inner
    }

    /// Returns an immutable reference to the underlying reader.
    #[inline]
    pub fn get_ref(&self) -> &R {
        &self.inner
    }

    /// Returns a mutable reference to the underlying reader.
    #[inline]
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }
}

impl<R: Read> Read for CountingReader<R> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.count.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

pub use brotli::{
    brotli_compress, brotli_compress_bound, brotli_compress_file, brotli_compress_stream_pipe,
    brotli_compress_to_vec, brotli_decompress, brotli_decompress_file,
    brotli_decompress_stream_pipe, brotli_decompress_to_vec, BrotliCompressorWriter, BrotliConfig,
    BrotliDecompressorReader, BROTLI_PIPE_BUFFER_SIZE,
};
pub use deflate::*;
pub use fast_blocks::*;
pub use lzma::*;
pub use lzma2::*;
pub use snappy::{
    is_framed_snappy, mask_crc32c, parse_varint, snappy_compress, snappy_compress_bound,
    snappy_compress_file, snappy_compress_stream_pipe, snappy_compress_to_vec, snappy_decompress,
    snappy_decompress_file, snappy_decompress_stream_pipe, snappy_decompress_to_vec,
    snappy_frame_decode, snappy_frame_decode_to_vec, snappy_frame_encode,
    snappy_frame_encode_to_vec, snappy_frame_max_encoded_length, snappy_frame_validate,
    snappy_uncompressed_length, snappy_validate, snappy_validate_bounded, unmask_crc32c,
    SNAPPY_MAX_CHUNK_SIZE, SNAPPY_PIPE_BUFFER_SIZE, SNAPPY_STREAM_IDENTIFIER,
};
pub use zstd::*;

