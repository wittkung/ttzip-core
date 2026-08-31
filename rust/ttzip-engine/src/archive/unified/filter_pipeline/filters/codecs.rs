// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Codec-backed streaming filter wrappers for Gzip, Bzip2, Xz, Zstd, Snappy, and Brotli.

use std::io::{self, Read};

use crate::archive::unified::filter_pipeline::kinds::FilterKind;
use crate::archive::unified::filter_pipeline::lookahead::SlidingLookaheadReader;
use crate::archive::unified::filter_pipeline::traits::StreamFilter;
use crate::codecs::bzip2::Bzip2Decompressor;
use crate::codecs::zstd::stream::ZstdStreamReader;

// MARK: - Gzip Filter

/// Streaming Gzip filter wrapping standard `flate2::read::GzDecoder`.
pub struct GzipFilter<R: Read + Send> {
    decoder: flate2::read::GzDecoder<R>,
    bytes_produced: u64,
}

impl<R: Read + Send> GzipFilter<R> {
    /// Creates a new Gzip streaming filter.
    pub fn new(reader: R) -> Self {
        Self {
            decoder: flate2::read::GzDecoder::new(reader),
            bytes_produced: 0,
        }
    }
}

impl<R: Read + Send> Read for GzipFilter<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.decoder.read(buf)?;
        self.bytes_produced += n as u64;
        Ok(n)
    }
}

impl<R: Read + Send> StreamFilter for GzipFilter<R> {
    fn filter_kind(&self) -> FilterKind {
        FilterKind::Gzip
    }
    fn bytes_consumed(&self) -> u64 {
        0
    }
    fn bytes_produced(&self) -> u64 {
        self.bytes_produced
    }
}

// MARK: - Bzip2 Filter

/// Streaming Bzip2 filter wrapping `Bzip2Decompressor`.
pub struct Bzip2Filter<R: Read + Send> {
    inner: SlidingLookaheadReader<R>,
    decompressor: Bzip2Decompressor,
    out_buf: Vec<u8>,
    out_pos: usize,
    out_len: usize,
    stream_end: bool,
    bytes_consumed: u64,
    bytes_produced: u64,
}

impl<R: Read + Send> Bzip2Filter<R> {
    /// Creates a new Bzip2 streaming filter.
    pub fn new(reader: R) -> io::Result<Self> {
        let decompressor = Bzip2Decompressor::new(false, 0)
            .map_err(|e| io::Error::other(format!("bzip2 init failed: {:?}", e)))?;
        Ok(Self {
            inner: SlidingLookaheadReader::new(reader),
            decompressor,
            out_buf: vec![0u8; 65536],
            out_pos: 0,
            out_len: 0,
            stream_end: false,
            bytes_consumed: 0,
            bytes_produced: 0,
        })
    }
}

impl<R: Read + Send> Read for Bzip2Filter<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        while self.out_pos >= self.out_len && !self.stream_end {
            let peeked = self.inner.peek(4096)?;
            if peeked.is_empty() {
                break;
            }
            let (in_consumed, out_produced, finished) = self
                .decompressor
                .decompress_chunk(peeked, &mut self.out_buf)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("bzip2 error: {:?}", e)))?;

            self.inner.consume_bytes(in_consumed);
            self.bytes_consumed += in_consumed as u64;
            self.out_pos = 0;
            self.out_len = out_produced;
            self.stream_end = finished;

            if in_consumed == 0 && out_produced == 0 {
                break;
            }
        }

        if self.out_pos < self.out_len {
            let avail = self.out_len - self.out_pos;
            let to_copy = avail.min(buf.len());
            buf[..to_copy].copy_from_slice(&self.out_buf[self.out_pos..self.out_pos + to_copy]);
            self.out_pos += to_copy;
            self.bytes_produced += to_copy as u64;
            return Ok(to_copy);
        }

        Ok(0)
    }
}

impl<R: Read + Send> StreamFilter for Bzip2Filter<R> {
    fn filter_kind(&self) -> FilterKind {
        FilterKind::Bzip2
    }
    fn bytes_consumed(&self) -> u64 {
        self.bytes_consumed
    }
    fn bytes_produced(&self) -> u64 {
        self.bytes_produced
    }
}

// MARK: - Native C liblzma XZ FFI Bindings

#[repr(C)]
struct XzLzmaStream {
    next_in: *const u8,
    avail_in: libc::size_t,
    total_in: u64,
    next_out: *mut u8,
    avail_out: libc::size_t,
    total_out: u64,
    allocator: *const libc::c_void,
    internal: *mut libc::c_void,
    reserved_ptr1: *mut libc::c_void,
    reserved_ptr2: *mut libc::c_void,
    reserved_ptr3: *mut libc::c_void,
    reserved_ptr4: *mut libc::c_void,
    reserved_seek: u64,
    reserved_int1: u64,
    reserved_int2: libc::size_t,
    reserved_int3: libc::size_t,
    reserved_enum1: libc::c_int,
    reserved_enum2: libc::c_int,
}

impl Default for XzLzmaStream {
    fn default() -> Self {
        Self {
            next_in: std::ptr::null(),
            avail_in: 0,
            total_in: 0,
            next_out: std::ptr::null_mut(),
            avail_out: 0,
            total_out: 0,
            allocator: std::ptr::null(),
            internal: std::ptr::null_mut(),
            reserved_ptr1: std::ptr::null_mut(),
            reserved_ptr2: std::ptr::null_mut(),
            reserved_ptr3: std::ptr::null_mut(),
            reserved_ptr4: std::ptr::null_mut(),
            reserved_seek: 0,
            reserved_int1: 0,
            reserved_int2: 0,
            reserved_int3: 0,
            reserved_enum1: 0,
            reserved_enum2: 0,
        }
    }
}

unsafe impl Send for XzLzmaStream {}

extern "C" {
    fn lzma_stream_decoder(strm: *mut XzLzmaStream, memlimit: u64, flags: u32) -> libc::c_int;
    fn lzma_code(strm: *mut XzLzmaStream, action: libc::c_int) -> libc::c_int;
    fn lzma_end(strm: *mut XzLzmaStream);
}

/// Streaming XZ filter wrapping `liblzma`.
pub struct XzFilter<R: Read + Send> {
    inner: SlidingLookaheadReader<R>,
    strm: XzLzmaStream,
    out_buf: Vec<u8>,
    out_pos: usize,
    out_len: usize,
    initialized: bool,
    stream_end: bool,
    bytes_consumed: u64,
    bytes_produced: u64,
}

unsafe impl<R: Read + Send> Send for XzFilter<R> {}

impl<R: Read + Send> XzFilter<R> {
    /// Creates a new XZ streaming filter with 512MB memory budget.
    pub fn new(reader: R) -> io::Result<Self> {
        let mut strm = XzLzmaStream::default();
        let ret = unsafe { lzma_stream_decoder(&mut strm, 512 * 1024 * 1024, 0) };
        if ret != 0 {
            return Err(io::Error::other(format!("xz decoder init failed (code {})", ret)));
        }
        Ok(Self {
            inner: SlidingLookaheadReader::new(reader),
            strm,
            out_buf: vec![0u8; 65536],
            out_pos: 0,
            out_len: 0,
            initialized: true,
            stream_end: false,
            bytes_consumed: 0,
            bytes_produced: 0,
        })
    }
}

impl<R: Read + Send> Read for XzFilter<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        while self.out_pos >= self.out_len && !self.stream_end {
            let peeked = self.inner.peek(8192)?;
            let eof = peeked.is_empty();

            self.strm.next_in = if eof { std::ptr::null() } else { peeked.as_ptr() };
            self.strm.avail_in = peeked.len();
            self.strm.next_out = self.out_buf.as_mut_ptr();
            self.strm.avail_out = self.out_buf.len();

            let action = if eof { 3 /* LZMA_FINISH */ } else { 0 /* LZMA_RUN */ };
            let ret = unsafe { lzma_code(&mut self.strm, action) };

            let in_consumed = peeked.len().saturating_sub(self.strm.avail_in);
            let out_produced = self.out_buf.len().saturating_sub(self.strm.avail_out);

            self.inner.consume_bytes(in_consumed);
            self.bytes_consumed += in_consumed as u64;
            self.out_pos = 0;
            self.out_len = out_produced;

            if ret == 1 /* LZMA_STREAM_END */ {
                self.stream_end = true;
                break;
            } else if ret != 0 && ret != 1 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, format!("xz decompression error: {}", ret)));
            }

            if in_consumed == 0 && out_produced == 0 {
                break;
            }
        }

        if self.out_pos < self.out_len {
            let avail = self.out_len - self.out_pos;
            let to_copy = avail.min(buf.len());
            buf[..to_copy].copy_from_slice(&self.out_buf[self.out_pos..self.out_pos + to_copy]);
            self.out_pos += to_copy;
            self.bytes_produced += to_copy as u64;
            return Ok(to_copy);
        }

        Ok(0)
    }
}

impl<R: Read + Send> Drop for XzFilter<R> {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { lzma_end(&mut self.strm) };
            self.initialized = false;
        }
    }
}

impl<R: Read + Send> StreamFilter for XzFilter<R> {
    fn filter_kind(&self) -> FilterKind {
        FilterKind::Xz
    }
    fn bytes_consumed(&self) -> u64 {
        self.bytes_consumed
    }
    fn bytes_produced(&self) -> u64 {
        self.bytes_produced
    }
}

// MARK: - Zstandard Filter

/// Streaming Zstandard filter wrapping `ZstdStreamReader`.
pub struct ZstdFilter<R: Read + Send> {
    reader: ZstdStreamReader<R>,
    bytes_produced: u64,
}

impl<R: Read + Send> ZstdFilter<R> {
    /// Creates a new Zstd streaming filter.
    pub fn new(reader: R) -> io::Result<Self> {
        let zstd_reader = ZstdStreamReader::new(reader)
            .map_err(|e| io::Error::other(format!("zstd init failed: {:?}", e)))?;
        Ok(Self {
            reader: zstd_reader,
            bytes_produced: 0,
        })
    }
}

impl<R: Read + Send> Read for ZstdFilter<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.reader.read(buf)?;
        self.bytes_produced += n as u64;
        Ok(n)
    }
}

impl<R: Read + Send> StreamFilter for ZstdFilter<R> {
    fn filter_kind(&self) -> FilterKind {
        FilterKind::Zstd
    }
    fn bytes_consumed(&self) -> u64 {
        0
    }
    fn bytes_produced(&self) -> u64 {
        self.bytes_produced
    }
}

// MARK: - Snappy Filter

/// Streaming Snappy filter wrapping `snap::read::FrameDecoder`.
pub struct SnappyFilter<R: Read + Send> {
    reader: snap::read::FrameDecoder<R>,
    bytes_produced: u64,
}

impl<R: Read + Send> SnappyFilter<R> {
    /// Creates a new Snappy frame decompressor filter.
    pub fn new(reader: R) -> Self {
        Self {
            reader: snap::read::FrameDecoder::new(reader),
            bytes_produced: 0,
        }
    }
}

impl<R: Read + Send> Read for SnappyFilter<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.reader.read(buf)?;
        self.bytes_produced += n as u64;
        Ok(n)
    }
}

impl<R: Read + Send> StreamFilter for SnappyFilter<R> {
    fn filter_kind(&self) -> FilterKind {
        FilterKind::Snappy
    }
    fn bytes_consumed(&self) -> u64 {
        0
    }
    fn bytes_produced(&self) -> u64 {
        self.bytes_produced
    }
}

// MARK: - Brotli Filter

/// Streaming Brotli filter wrapping `brotli::Decompressor`.
pub struct BrotliFilter<R: Read + Send> {
    reader: brotli::Decompressor<R>,
    bytes_produced: u64,
}

impl<R: Read + Send> BrotliFilter<R> {
    /// Creates a new Brotli streaming filter.
    pub fn new(reader: R) -> Self {
        Self {
            reader: brotli::Decompressor::new(reader, 65536),
            bytes_produced: 0,
        }
    }
}

impl<R: Read + Send> Read for BrotliFilter<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.reader.read(buf)?;
        self.bytes_produced += n as u64;
        Ok(n)
    }
}

impl<R: Read + Send> StreamFilter for BrotliFilter<R> {
    fn filter_kind(&self) -> FilterKind {
        FilterKind::Brotli
    }
    fn bytes_consumed(&self) -> u64 {
        0
    }
    fn bytes_produced(&self) -> u64 {
        self.bytes_produced
    }
}
