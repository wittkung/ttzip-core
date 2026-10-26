// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Upstream `brotli = "7.0"` canonical Google Brotli block and streaming compression codecs.
//!
//! Provides single canonical backend for Google Brotli block compression/decompression,
//! streaming readers/writers, and bounded 4MB pipe operations.

use std::io::{Cursor, Read, Write};
use brotli::{CompressorWriter, Decompressor};
use brotli::enc::BrotliEncoderParams;
use crate::codecs::{CountingReader, CountingWriter};
use crate::types::TTZipStatus;

pub const BROTLI_PIPE_BUFFER_SIZE: usize = 4 * 1024 * 1024; // 4MB

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BrotliError {
    #[error("Unexpected end of Brotli bitstream")]
    UnexpectedEof,
    #[error("Invalid non-zero padding bits in Brotli stream")]
    InvalidPadding,
    #[error("Invalid window bits: {0}")]
    InvalidWindowBits(u8),
    #[error("Corrupted Brotli header: {0}")]
    CorruptHeader(String),
    #[error("Brotli decompression error: {0}")]
    DecompressionFailed(String),
    #[error("Invalid Brotli transform index: {0}")]
    InvalidTransformIndex(usize),
    #[error("Destination buffer too small")]
    BufferTooSmall,
    #[error("Invalid UTF-8 sequence during dictionary word transformation")]
    InvalidUtf8Sequence,
    #[error("Invalid Brotli quality level: {0}")]
    InvalidQuality(u32),
    #[error("Brotli compression failed")]
    CompressionFailed,
    #[error("Huffman space violation")]
    HuffmanSpaceViolation,
    #[error("Duplicate symbol in simple Huffman code")]
    DuplicateSymbol,
    #[error("Brotli block decode error")]
    BlockDecodeError,
}

impl From<BrotliError> for TTZipStatus {
    fn from(e: BrotliError) -> Self {
        match e {
            BrotliError::CorruptHeader(_)
            | BrotliError::UnexpectedEof
            | BrotliError::InvalidPadding
            | BrotliError::InvalidWindowBits(_)
            | BrotliError::HuffmanSpaceViolation
            | BrotliError::DuplicateSymbol
            | BrotliError::BlockDecodeError => TTZipStatus::ErrCorruptHeader,
            BrotliError::BufferTooSmall => TTZipStatus::ErrBufferTooSmall,
            BrotliError::CompressionFailed => TTZipStatus::ErrCompressionFailed,
            _ => TTZipStatus::ErrExtractionFailed,
        }
    }
}

/// Configuration parameters for Brotli streaming compression.
#[derive(Debug, Clone)]
pub struct BrotliConfig {
    pub quality: u32,
    pub lgwin: u32,
    pub buffer_size: usize,
}

impl Default for BrotliConfig {
    fn default() -> Self {
        Self {
            quality: 6,
            lgwin: 22,
            buffer_size: 65536,
        }
    }
}

// MARK: - Core Block APIs

/// Computes worst-case output buffer size for Brotli block compression.
#[inline]
pub fn brotli_compress_bound(src_size: usize) -> usize {
    if src_size == 0 {
        1024
    } else {
        src_size + (src_size >> 2) + 10240
    }
}

/// Compresses a memory block using Brotli into a pre-allocated destination buffer.
pub fn brotli_compress(
    src: &[u8],
    dst: &mut [u8],
    quality: u32,
    lgwin: u32,
) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    let params = BrotliEncoderParams {
        quality: quality.clamp(0, 11) as i32,
        lgwin: if lgwin == 0 { 22 } else { lgwin.clamp(10, 24) as i32 },
        ..Default::default()
    };

    let mut cursor_in = Cursor::new(src);
    let mut cursor_out = Cursor::new(dst);

    brotli::BrotliCompress(&mut cursor_in, &mut cursor_out, &params)
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;

    Ok(cursor_out.position() as usize)
}

/// Decompresses a Brotli compressed block into a pre-allocated destination buffer.
pub fn brotli_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    let mut cursor_in = Cursor::new(src);
    let mut cursor_out = Cursor::new(dst);

    brotli::BrotliDecompress(&mut cursor_in, &mut cursor_out)
        .map_err(|_| TTZipStatus::ErrCorruptHeader)?;

    Ok(cursor_out.position() as usize)
}

/// Compresses a memory slice into a newly allocated `Vec<u8>`.
pub fn brotli_compress_to_vec(
    src: &[u8],
    quality: u32,
    lgwin: u32,
) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() {
        return Ok(Vec::new());
    }
    let bound = brotli_compress_bound(src.len());
    let mut out = vec![0u8; bound];
    let written = brotli_compress(src, &mut out, quality, lgwin)?;
    out.truncate(written);
    Ok(out)
}

/// Decompresses a Brotli slice into a newly allocated `Vec<u8>`.
pub fn brotli_decompress_to_vec(src: &[u8], max_allowed: usize) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() {
        return Ok(Vec::new());
    }
    let mut decompressor = Decompressor::new(src, 65536);
    let mut out = Vec::new();
    let mut chunk = [0u8; 65536];
    loop {
        let n = decompressor.read(&mut chunk).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
        if n == 0 {
            break;
        }
        if out.len().saturating_add(n) > max_allowed {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        out.extend_from_slice(&chunk[..n]);
    }
    Ok(out)
}

// MARK: - Streaming Wrappers

/// RAII wrapper around `brotli::CompressorWriter`.
pub struct BrotliCompressorWriter<W: Write> {
    inner: Option<CompressorWriter<W>>,
}

impl<W: Write> BrotliCompressorWriter<W> {
    pub fn new(writer: W, config: &BrotliConfig) -> Self {
        let q = config.quality.clamp(0, 11);
        let lg = if config.lgwin == 0 { 22 } else { config.lgwin.clamp(10, 24) };
        let buf_size = if config.buffer_size == 0 { 65536 } else { config.buffer_size };
        let inner = CompressorWriter::new(writer, buf_size, q, lg);
        Self {
            inner: Some(inner),
        }
    }

    pub fn write_chunk(&mut self, data: &[u8]) -> Result<usize, TTZipStatus> {
        match &mut self.inner {
            Some(w) => w.write(data).map_err(|_| TTZipStatus::ErrCompressionFailed),
            None => Err(TTZipStatus::ErrInvalidParam),
        }
    }

    pub fn finish(mut self) -> Result<W, TTZipStatus> {
        match self.inner.take() {
            Some(w) => Ok(w.into_inner()),
            None => Err(TTZipStatus::ErrInvalidParam),
        }
    }
}

impl<W: Write> Write for BrotliCompressorWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match &mut self.inner {
            Some(w) => w.write(buf),
            None => Err(std::io::Error::other("closed stream")),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match &mut self.inner {
            Some(w) => w.flush(),
            None => Ok(()),
        }
    }
}

/// RAII wrapper around `brotli::Decompressor`.
pub struct BrotliDecompressorReader<R: Read> {
    inner: Decompressor<R>,
}

impl<R: Read> BrotliDecompressorReader<R> {
    pub fn new(reader: R, buffer_size: usize) -> Self {
        let buf_size = if buffer_size == 0 { 65536 } else { buffer_size };
        Self {
            inner: Decompressor::new(reader, buf_size),
        }
    }
}

impl<R: Read> Read for BrotliDecompressorReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

// MARK: - Streaming Pipe APIs

pub fn brotli_compress_stream_pipe<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    quality: u32,
    lgwin: u32,
    progress_callback: Option<&dyn Fn(u64, u64) -> bool>,
) -> Result<(u64, u64), TTZipStatus> {
    let config = BrotliConfig {
        quality,
        lgwin,
        buffer_size: 65536,
    };

    let mut counting_writer = CountingWriter::new(writer);
    let written_counter = counting_writer.counter();
    let mut encoder = BrotliCompressorWriter::new(&mut counting_writer, &config);
    let mut in_buf = vec![0u8; BROTLI_PIPE_BUFFER_SIZE];
    let mut total_read: u64 = 0;

    loop {
        let bytes_read = reader.read(&mut in_buf).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        if bytes_read == 0 {
            break;
        }

        total_read = total_read.saturating_add(bytes_read as u64);
        encoder
            .write_all(&in_buf[..bytes_read])
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;

        if let Some(cb) = progress_callback {
            if !cb(total_read, written_counter.load(std::sync::atomic::Ordering::Relaxed)) {
                return Err(TTZipStatus::Cancelled);
            }
        }
    }

    let inner_writer = encoder.finish()?;
    inner_writer.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;

    Ok((total_read, written_counter.load(std::sync::atomic::Ordering::Relaxed)))
}

pub fn brotli_decompress_stream_pipe<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    progress_callback: Option<&dyn Fn(u64, u64) -> bool>,
) -> Result<(u64, u64), TTZipStatus> {
    let mut counting_reader = CountingReader::new(reader);
    let read_counter = counting_reader.counter();
    let mut decompressor = BrotliDecompressorReader::new(&mut counting_reader, 65536);
    let mut out_buf = vec![0u8; BROTLI_PIPE_BUFFER_SIZE];
    let mut total_written: u64 = 0;

    loop {
        let bytes_read = decompressor.read(&mut out_buf).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
        if bytes_read == 0 {
            break;
        }

        writer
            .write_all(&out_buf[..bytes_read])
            .map_err(|_| TTZipStatus::ErrExtractionFailed)?;
        total_written = total_written.saturating_add(bytes_read as u64);

        if let Some(cb) = progress_callback {
            if !cb(read_counter.load(std::sync::atomic::Ordering::Relaxed), total_written) {
                return Err(TTZipStatus::Cancelled);
            }
        }
    }

    writer.flush().map_err(|_| TTZipStatus::ErrExtractionFailed)?;
    Ok((read_counter.load(std::sync::atomic::Ordering::Relaxed), total_written))
}

pub fn brotli_compress_file<P: AsRef<std::path::Path>, Q: AsRef<std::path::Path>>(
    src: P,
    dst: Q,
    quality: u32,
    lgwin: u32,
    progress_callback: Option<&dyn Fn(u64, u64) -> bool>,
) -> Result<(u64, u64), TTZipStatus> {
    let mut src_file = std::fs::File::open(src).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let mut dst_file = std::fs::File::create(dst).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    brotli_compress_stream_pipe(&mut src_file, &mut dst_file, quality, lgwin, progress_callback)
}

pub fn brotli_decompress_file<P: AsRef<std::path::Path>, Q: AsRef<std::path::Path>>(
    src: P,
    dst: Q,
    progress_callback: Option<&dyn Fn(u64, u64) -> bool>,
) -> Result<(u64, u64), TTZipStatus> {
    let mut src_file = std::fs::File::open(src).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let mut dst_file = std::fs::File::create(dst).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    brotli_decompress_stream_pipe(&mut src_file, &mut dst_file, progress_callback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brotli_block_roundtrip() {
        let input = b"Pure Rust Brotli block compression test in TTZip native glue layer 2026.";
        let mut compressed = vec![0u8; brotli_compress_bound(input.len())];
        let comp_len = brotli_compress(input, &mut compressed, 6, 22).expect("brotli compress");
        assert!(comp_len > 0);

        let mut decompressed = vec![0u8; input.len()];
        let decomp_len = brotli_decompress(&compressed[..comp_len], &mut decompressed).expect("brotli decompress");
        assert_eq!(decomp_len, input.len());
        assert_eq!(&decompressed[..decomp_len], input);
    }

    #[test]
    fn test_brotli_to_vec_roundtrip() {
        let input = b"Repetitive string test for Brotli to vector compression: ABCABCABCABCABCABCABCABCABCABCABCABCABCABCABCABCABCABC";
        let comp = brotli_compress_to_vec(input, 9, 22).expect("brotli to vec compress");
        assert!(comp.len() < input.len());

        let decomp = brotli_decompress_to_vec(&comp, 1024 * 1024).expect("brotli to vec decompress");
        assert_eq!(decomp, input);
    }

    #[test]
    fn test_brotli_pipe_exact_byte_counts_and_compression_ratio() {
        let payload = vec![0xFEu8; 5 * 1024 * 1024];
        let mut reader = Cursor::new(&payload);
        let mut compressed = Vec::new();

        let (read_bytes, written_bytes) = brotli_compress_stream_pipe(&mut reader, &mut compressed, 4, 20, None)
            .expect("brotli compress pipe failed");
        assert_eq!(read_bytes, payload.len() as u64);
        assert_eq!(written_bytes, compressed.len() as u64);
        assert!(written_bytes < read_bytes);

        let ratio = (written_bytes as f64) / (read_bytes as f64);
        assert!(ratio < 0.01);

        let mut comp_reader = Cursor::new(&compressed);
        let mut decompressed = Vec::new();
        let (dec_read, dec_written) = brotli_decompress_stream_pipe(&mut comp_reader, &mut decompressed, None)
            .expect("brotli decompress pipe failed");
        assert_eq!(dec_read, compressed.len() as u64);
        assert_eq!(dec_written, payload.len() as u64);
        assert_eq!(decompressed, payload);
    }

    #[test]
    fn test_brotli_corrupt_data() {
        let corrupt = [0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x01];
        let mut out = [0u8; 128];
        let res = brotli_decompress(&corrupt, &mut out);
        assert!(res.is_err());
    }

    #[test]
    fn test_brotli_quality_and_window_range() {
        let text = b"Testing Brotli across various quality settings and sliding window sizes in TTZip.";
        for q in [0, 1, 4, 6, 9, 11] {
            for lgwin in [10, 16, 22, 24] {
                let comp = brotli_compress_to_vec(text, q, lgwin).expect("compress");
                assert!(!comp.is_empty());
                let decomp = brotli_decompress_to_vec(&comp, 4096).expect("decompress");
                assert_eq!(decomp.as_slice(), text);
            }
        }
    }

    #[test]
    fn test_brotli_rfc7932_static_dictionary_efficiency() {
        let web_snippet = b"<html><head><meta charset=\"utf-8\"><title>TTZip RFC 7932</title></head><body><div class=\"container\"><p>http://www.w3.org/1999/xhtml</p></div></body></html>";
        let comp = brotli_compress_to_vec(web_snippet, 11, 22).expect("brotli rfc7932 compress");
        assert!(comp.len() < web_snippet.len());

        let decomp = brotli_decompress_to_vec(&comp, 4096).expect("brotli rfc7932 decompress");
        assert_eq!(decomp.as_slice(), web_snippet);
    }

    #[test]
    fn test_brotli_empty_slice() {
        let empty = b"";
        let mut dst = [0u8; 64];
        let c_len = brotli_compress(empty, &mut dst, 6, 22).expect("empty compress");
        assert_eq!(c_len, 0);

        let d_len = brotli_decompress(empty, &mut dst).expect("empty decompress");
        assert_eq!(d_len, 0);
    }
}
