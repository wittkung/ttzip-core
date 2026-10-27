// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Safe, canonical Apple `LZFSE` and `LZVN` hardware/native block codecs backed by the Apple reference C library.

use crate::types::TTZipStatus;
use std::cell::RefCell;
use std::io::{self, Cursor, Read, Write};

pub const LZFSE_BLOCK_CHUNK_SIZE: usize = 256 * 1024;
pub const LZFSE_CHUNK_SIZE: usize = LZFSE_BLOCK_CHUNK_SIZE;
pub const LZFSE_MAX_BLOCK_SIZE: usize = 256 * 1024;
pub const DEFAULT_LZVN_THRESHOLD: usize = 4096;

// MARK: - Native C LZFSE & LZVN Bindings

extern "C" {
    fn lzfse_encode_scratch_size() -> libc::size_t;
    fn lzfse_encode_buffer(
        dst_buffer: *mut u8,
        dst_size: libc::size_t,
        src_buffer: *const u8,
        src_size: libc::size_t,
        scratch_buffer: *mut libc::c_void,
    ) -> libc::size_t;

    fn lzfse_decode_scratch_size() -> libc::size_t;
    fn lzfse_decode_buffer(
        dst_buffer: *mut u8,
        dst_size: libc::size_t,
        src_buffer: *const u8,
        src_size: libc::size_t,
        scratch_buffer: *mut libc::c_void,
    ) -> libc::size_t;

    fn lzvn_encode_scratch_size() -> libc::size_t;
    fn lzvn_encode_buffer(
        dst_buffer: *mut libc::c_void,
        dst_size: libc::size_t,
        src_buffer: *const libc::c_void,
        src_size: libc::size_t,
        scratch_buffer: *mut libc::c_void,
    ) -> libc::size_t;

    fn lzvn_decode(state: *mut LzvnDecoderState);
}

#[repr(C)]
struct LzvnDecoderState {
    src: *const u8,
    src_end: *const u8,
    dst: *mut u8,
    dst_begin: *mut u8,
    dst_end: *mut u8,
    dst_current: *mut u8,
    l: usize,
    m: usize,
    d: usize,
    d_prev: isize,
    end_of_stream: libc::c_int,
}

const LZFSE_SCRATCH_MIN_CAPACITY: usize = 2 * 1024 * 1024; // 2MB scratch buffer

thread_local! {
    static LZFSE_SCRATCH_BUFFER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static LZVN_SCRATCH_BUFFER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn with_lzfse_scratch<F, R>(min_size: usize, f: F) -> R
where
    F: FnOnce(*mut libc::c_void) -> R,
{
    LZFSE_SCRATCH_BUFFER.with(|cell| {
        let mut buf = cell.borrow_mut();
        let target_size = min_size.max(LZFSE_SCRATCH_MIN_CAPACITY);
        if buf.len() < target_size {
            buf.resize(target_size, 0);
        }
        f(buf.as_mut_ptr() as *mut libc::c_void)
    })
}

fn with_lzvn_scratch<F, R>(f: F) -> R
where
    F: FnOnce(*mut libc::c_void) -> R,
{
    LZVN_SCRATCH_BUFFER.with(|cell| {
        let mut buf = cell.borrow_mut();
        let target_size = unsafe { lzvn_encode_scratch_size() }.max(65536);
        if buf.len() < target_size {
            buf.resize(target_size, 0);
        }
        f(buf.as_mut_ptr() as *mut libc::c_void)
    })
}

// MARK: - Bounds Calculation

#[inline]
pub fn lzfse_compress_bound(src_size: usize) -> usize {
    src_size.saturating_add(4096)
}

#[inline]
pub fn lzvn_compress_bound(src_size: usize) -> usize {
    src_size.saturating_add(4096)
}

// MARK: - LZFSE Compression & Decompression

pub fn lzfse_compress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    let scratch_size = unsafe { lzfse_encode_scratch_size() };
    let written = with_lzfse_scratch(scratch_size, |scratch| unsafe {
        lzfse_encode_buffer(
            dst.as_mut_ptr(),
            dst.len(),
            src.as_ptr(),
            src.len(),
            scratch,
        )
    });

    if written == 0 {
        Err(TTZipStatus::ErrCompressionFailed)
    } else {
        Ok(written)
    }
}

#[inline]
pub fn lzfse_compress_raw(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    lzfse_compress_to_vec(src)
}

pub fn lzfse_compress_to_vec(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    let bound = lzfse_compress_bound(src.len());
    let mut out = vec![0u8; bound];
    let written = lzfse_compress(src, &mut out)?;
    out.truncate(written);
    Ok(out)
}

#[inline]
pub fn lzfse_compress_stream(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    lzfse_compress_to_vec(src)
}

pub fn lzfse_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    let scratch_size = unsafe { lzfse_decode_scratch_size() };
    let written = with_lzfse_scratch(scratch_size, |scratch| unsafe {
        lzfse_decode_buffer(
            dst.as_mut_ptr(),
            dst.len(),
            src.as_ptr(),
            src.len(),
            scratch,
        )
    });

    if written == 0 {
        Err(TTZipStatus::ErrExtractionFailed)
    } else {
        Ok(written)
    }
}

pub fn lzfse_decompress_raw(src: &[u8], uncompressed_size: usize) -> Result<Vec<u8>, TTZipStatus> {
    let mut out = vec![0u8; uncompressed_size];
    let written = lzfse_decompress(src, &mut out)?;
    out.truncate(written);
    Ok(out)
}

#[inline]
pub fn lzfse_decompress_to_vec(src: &[u8], uncompressed_size: usize) -> Result<Vec<u8>, TTZipStatus> {
    lzfse_decompress_raw(src, uncompressed_size)
}

pub fn lzfse_decompress_stream(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() {
        return Ok(Vec::new());
    }
    const MAX_CEILING: usize = 256 * 1024 * 1024; // 256MB strict ceiling
    let mut capacity = (src.len() * 4).clamp(65536, MAX_CEILING);

    loop {
        let mut out = vec![0u8; capacity];
        let written = match lzfse_decompress(src, &mut out) {
            Ok(n) => n,
            Err(e) => {
                // written == 0: Apple LZFSE returns 0 on error/corrupt data; do NOT double capacity!
                return Err(e);
            }
        };

        if written == out.len() {
            // Destination buffer was exhausted (LZFSE_STATUS_DST_FULL).
            if capacity >= MAX_CEILING {
                return Err(TTZipStatus::ErrBufferTooSmall);
            }
            capacity = capacity.saturating_mul(2).min(MAX_CEILING);
        } else {
            // 0 < written < out.len(): decompression succeeded.
            out.truncate(written);
            return Ok(out);
        }
    }
}

pub fn lzfse_validate(src: &[u8]) -> bool {
    let mut probe = [0u8; 16];
    lzfse_decompress(src, &mut probe).is_ok()
}

// MARK: - LZVN Compression & Decompression

pub fn lzvn_compress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    let written = with_lzvn_scratch(|scratch| unsafe {
        lzvn_encode_buffer(
            dst.as_mut_ptr() as *mut libc::c_void,
            dst.len(),
            src.as_ptr() as *const libc::c_void,
            src.len(),
            scratch,
        )
    });

    if written == 0 {
        Err(TTZipStatus::ErrCompressionFailed)
    } else {
        Ok(written)
    }
}

#[inline]
pub fn lzvn_compress_raw(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    lzvn_compress_to_vec(src)
}

pub fn lzvn_compress_to_vec(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    let bound = lzvn_compress_bound(src.len());
    let mut out = vec![0u8; bound];
    let written = lzvn_compress(src, &mut out)?;
    out.truncate(written);
    Ok(out)
}

pub fn lzvn_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    let mut state = LzvnDecoderState {
        src: src.as_ptr(),
        src_end: unsafe { src.as_ptr().add(src.len()) },
        dst: dst.as_mut_ptr(),
        dst_begin: dst.as_mut_ptr(),
        dst_end: unsafe { dst.as_mut_ptr().add(dst.len()) },
        dst_current: dst.as_mut_ptr(),
        l: 0,
        m: 0,
        d: 0,
        d_prev: 0,
        end_of_stream: 0,
    };

    unsafe {
        lzvn_decode(&mut state);
    }

    if state.end_of_stream == 0 {
        if state.dst == state.dst_end {
            return Err(TTZipStatus::ErrBufferTooSmall);
        }
        return Err(TTZipStatus::ErrExtractionFailed);
    }

    let written = (state.dst as usize).saturating_sub(dst.as_ptr() as usize);
    Ok(written)
}

pub fn lzvn_decompress_raw(src: &[u8], uncompressed_size: usize) -> Result<Vec<u8>, TTZipStatus> {
    let mut out = vec![0u8; uncompressed_size];
    let written = lzvn_decompress(src, &mut out)?;
    out.truncate(written);
    Ok(out)
}

#[inline]
pub fn lzvn_decompress_to_vec(src: &[u8], uncompressed_size: usize) -> Result<Vec<u8>, TTZipStatus> {
    lzvn_decompress_raw(src, uncompressed_size)
}

pub fn lzvn_validate(src: &[u8]) -> bool {
    let mut probe = [0u8; 16];
    lzvn_decompress(src, &mut probe).is_ok()
}

// MARK: - Streaming Adapters

pub struct LzfseReader<R: Read> {
    inner: Cursor<Vec<u8>>,
    _phantom: std::marker::PhantomData<R>,
}

impl<R: Read> LzfseReader<R> {
    pub fn new(mut source: R) -> io::Result<Self> {
        let mut compressed = Vec::new();
        source.read_to_end(&mut compressed)?;
        let mut decompressed = vec![0u8; compressed.len().saturating_mul(4).max(65536)];
        let n = lzfse_decompress(&compressed, &mut decompressed)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "LZFSE decode error"))?;
        decompressed.truncate(n);
        Ok(Self {
            inner: Cursor::new(decompressed),
            _phantom: std::marker::PhantomData,
        })
    }
}

impl<R: Read> Read for LzfseReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

pub struct LzfseWriter<W: Write> {
    sink: W,
    buffer: Vec<u8>,
}

impl<W: Write> LzfseWriter<W> {
    pub fn new(sink: W) -> Self {
        Self {
            sink,
            buffer: Vec::new(),
        }
    }

    pub fn finish(mut self) -> io::Result<W> {
        let compressed = lzfse_compress_to_vec(&self.buffer)
            .map_err(|_| io::Error::other("LZFSE compression failed"))?;
        self.sink.write_all(&compressed)?;
        self.sink.flush()?;
        Ok(self.sink)
    }
}

impl<W: Write> Write for LzfseWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lzfse_basic_roundtrip() {
        let input = b"Apple LZFSE canonical C library compression roundtrip test in TTZip 2026!";
        let mut comp = vec![0u8; lzfse_compress_bound(input.len())];
        let c_len = lzfse_compress(input, &mut comp).expect("lzfse compress");
        assert!(c_len > 0);

        let mut decomp = vec![0u8; input.len()];
        let d_len = lzfse_decompress(&comp[..c_len], &mut decomp).expect("lzfse decompress");
        assert_eq!(d_len, input.len());
        assert_eq!(&decomp, input);
    }

    #[test]
    fn test_lzvn_basic_roundtrip() {
        let input = b"Apple LZVN canonical C library compression roundtrip test in TTZip 2026!";
        let mut comp = vec![0u8; lzvn_compress_bound(input.len())];
        let c_len = lzvn_compress(input, &mut comp).expect("lzvn compress");
        assert!(c_len > 0);

        let mut decomp = vec![0u8; input.len()];
        let d_len = lzvn_decompress(&comp[..c_len], &mut decomp).expect("lzvn decompress");
        assert_eq!(d_len, input.len());
        assert_eq!(&decomp, input);
    }

    #[test]
    fn test_lzfse_decompress_stream_corrupt_fails_immediately() {
        let corrupt = b"not a valid lzfse compressed payload";
        let res = lzfse_decompress_stream(corrupt);
        assert_eq!(res.unwrap_err(), TTZipStatus::ErrExtractionFailed);
    }

    #[test]
    fn test_lzfse_decompress_stream_large_payload() {
        let input = vec![0x42u8; 128 * 1024];
        let compressed = lzfse_compress_stream(&input).expect("compress stream");
        let decompressed = lzfse_decompress_stream(&compressed).expect("decompress stream");
        assert_eq!(decompressed, input);
    }

    #[test]
    fn test_lzvn_truncated_stream_rejected() {
        let input = b"LZVN test data with enough content to create multiple compression tokens and test stream termination";
        let compressed = lzvn_compress_to_vec(input).expect("lzvn compress");
        let truncated = &compressed[..compressed.len() / 2];
        let mut out = vec![0u8; input.len() * 2];
        let res = lzvn_decompress(truncated, &mut out);
        assert!(res.is_err(), "Truncated LZVN stream must return an error");
    }
}
