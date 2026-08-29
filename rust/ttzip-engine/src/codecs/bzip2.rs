// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, RAII-governed Bzip2 compression and decompression microkernel.
//!
//! Features:
//! - Levels 1..=9 corresponding to 100KB..900KB Burrows-Wheeler Transform (BWT) block sizes.
//! - Move-to-Front (MTF) and multi-table canonical Huffman tree encoding.
//! - Robust bounds checking, stream magic verification, and anti-malformed stream DoS guards.
//! - Configurable output expansion limit to defend against decompression bombs.

use crate::types::TTZipStatus;
use std::io::{Read, Write};

// MARK: - Native C libbz2 FFI Bindings

#[repr(C)]
struct BzStream {
    next_in: *const libc::c_char,
    avail_in: libc::c_uint,
    total_in_lo32: libc::c_uint,
    total_in_hi32: libc::c_uint,
    next_out: *mut libc::c_char,
    avail_out: libc::c_uint,
    total_out_lo32: libc::c_uint,
    total_out_hi32: libc::c_uint,
    state: *mut libc::c_void,
    bzalloc: Option<unsafe extern "C" fn(*mut libc::c_void, libc::c_int, libc::c_int) -> *mut libc::c_void>,
    bzfree: Option<unsafe extern "C" fn(*mut libc::c_void, *mut libc::c_void)>,
    opaque: *mut libc::c_void,
}

impl Default for BzStream {
    fn default() -> Self {
        Self {
            next_in: std::ptr::null(),
            avail_in: 0,
            total_in_lo32: 0,
            total_in_hi32: 0,
            next_out: std::ptr::null_mut(),
            avail_out: 0,
            total_out_lo32: 0,
            total_out_hi32: 0,
            state: std::ptr::null_mut(),
            bzalloc: None,
            bzfree: None,
            opaque: std::ptr::null_mut(),
        }
    }
}

#[allow(dead_code)]
const BZ_RUN: libc::c_int = 0;
#[allow(dead_code)]
const BZ_FLUSH: libc::c_int = 1;
#[allow(dead_code)]
const BZ_FINISH: libc::c_int = 2;

const BZ_OK: libc::c_int = 0;
const BZ_RUN_OK: libc::c_int = 1;
const BZ_FLUSH_OK: libc::c_int = 2;
const BZ_FINISH_OK: libc::c_int = 3;
const BZ_STREAM_END: libc::c_int = 4;

const BZ_SEQUENCE_ERROR: libc::c_int = -1;
const BZ_PARAM_ERROR: libc::c_int = -2;
const BZ_MEM_ERROR: libc::c_int = -3;
const BZ_DATA_ERROR: libc::c_int = -4;
const BZ_DATA_ERROR_MAGIC: libc::c_int = -5;
const BZ_IO_ERROR: libc::c_int = -6;
const BZ_UNEXPECTED_EOF: libc::c_int = -7;
const BZ_OUTBUFF_FULL: libc::c_int = -8;
const BZ_CONFIG_ERROR: libc::c_int = -9;

extern "C" {
    fn BZ2_bzCompressInit(
        strm: *mut BzStream,
        blockSize100k: libc::c_int,
        verbosity: libc::c_int,
        workFactor: libc::c_int,
    ) -> libc::c_int;

    fn BZ2_bzCompress(strm: *mut BzStream, action: libc::c_int) -> libc::c_int;

    fn BZ2_bzCompressEnd(strm: *mut BzStream) -> libc::c_int;

    fn BZ2_bzDecompressInit(
        strm: *mut BzStream,
        verbosity: libc::c_int,
        small: libc::c_int,
    ) -> libc::c_int;

    fn BZ2_bzDecompress(strm: *mut BzStream) -> libc::c_int;

    fn BZ2_bzDecompressEnd(strm: *mut BzStream) -> libc::c_int;

    fn BZ2_bzBuffToBuffCompress(
        dest: *mut libc::c_char,
        destLen: *mut libc::c_uint,
        source: *const libc::c_char,
        sourceLen: libc::c_uint,
        blockSize100k: libc::c_int,
        verbosity: libc::c_int,
        workFactor: libc::c_int,
    ) -> libc::c_int;

    fn BZ2_bzBuffToBuffDecompress(
        dest: *mut libc::c_char,
        destLen: *mut libc::c_uint,
        source: *const libc::c_char,
        sourceLen: libc::c_uint,
        small: libc::c_int,
        verbosity: libc::c_int,
    ) -> libc::c_int;
}

// MARK: - Constants & DoS Limits

/// Standard Bzip2 header magic bytes (`BZh`).
pub const BZ_MAGIC: &[u8; 3] = b"BZh";

/// BWT data block header magic bytes (Pi digits: `0x314159265359`).
pub const BZ_PI_BLOCK_MAGIC: &[u8; 6] = &[0x31, 0x41, 0x59, 0x26, 0x53, 0x59];

/// Stream terminator EOS block magic bytes (Sqrt(Pi) digits: `0x177245385090`).
pub const BZ_EOS_BLOCK_MAGIC: &[u8; 6] = &[0x17, 0x72, 0x45, 0x38, 0x50, 0x90];

/// Maximum default uncompressed payload size (512MB) to prevent decompression bombs.
pub const DEFAULT_MAX_DECOMPRESSED_LIMIT: usize = 512 * 1024 * 1024;

/// Default stream pipe buffer size (64KB).
pub const BZIP2_PIPE_BUFFER_SIZE: usize = 64 * 1024;

#[inline]
fn map_bz_error(code: libc::c_int) -> TTZipStatus {
    match code {
        BZ_OK | BZ_RUN_OK | BZ_FLUSH_OK | BZ_FINISH_OK | BZ_STREAM_END => TTZipStatus::Ok,
        BZ_PARAM_ERROR => TTZipStatus::ErrInvalidParam,
        BZ_MEM_ERROR => TTZipStatus::ErrOutOfMemory,
        BZ_DATA_ERROR | BZ_DATA_ERROR_MAGIC => TTZipStatus::ErrCorruptHeader,
        BZ_OUTBUFF_FULL => TTZipStatus::ErrExtractionFailed,
        BZ_UNEXPECTED_EOF => TTZipStatus::ErrCorruptHeader,
        BZ_SEQUENCE_ERROR | BZ_IO_ERROR | BZ_CONFIG_ERROR => TTZipStatus::ErrCompressionFailed,
        _ => TTZipStatus::ErrExtractionFailed,
    }
}

// MARK: - Safe RAII Streaming Bzip2 Compressor

/// Safe RAII streaming Bzip2 compressor wrapping `libbz2`.
pub struct Bzip2Compressor {
    strm: Box<BzStream>,
    initialized: bool,
    level: i32,
}

unsafe impl Send for Bzip2Compressor {}

impl Bzip2Compressor {
    /// Creates a new Bzip2 compressor with compression level in `1..=9`.
    pub fn new(level: i32) -> Result<Self, TTZipStatus> {
        let clamped_level = level.clamp(1, 9);
        let mut strm = Box::new(BzStream::default());

        let ret = unsafe { BZ2_bzCompressInit(&mut *strm, clamped_level, 0, 30) };
        if ret != BZ_OK {
            return Err(map_bz_error(ret));
        }

        Ok(Self {
            strm,
            initialized: true,
            level: clamped_level,
        })
    }

    /// Returns the configured compression level.
    #[inline]
    pub fn level(&self) -> i32 {
        self.level
    }

    /// Compresses a chunk of input data into output buffer.
    /// Returns `(consumed_bytes, written_bytes, is_finished)`.
    pub fn compress_chunk(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        finish: bool,
    ) -> Result<(usize, usize, bool), TTZipStatus> {
        if !self.initialized {
            return Err(TTZipStatus::ErrCompressionFailed);
        }

        self.strm.next_in = if input.is_empty() {
            std::ptr::null()
        } else {
            input.as_ptr() as *const libc::c_char
        };
        self.strm.avail_in = input.len().min(u32::MAX as usize) as libc::c_uint;

        self.strm.next_out = if output.is_empty() {
            std::ptr::null_mut()
        } else {
            output.as_mut_ptr() as *mut libc::c_char
        };
        self.strm.avail_out = output.len().min(u32::MAX as usize) as libc::c_uint;

        let action = if finish { BZ_FINISH } else { BZ_RUN };
        let ret = unsafe { BZ2_bzCompress(&mut *self.strm, action) };

        let in_consumed = input.len().saturating_sub(self.strm.avail_in as usize);
        let out_produced = output.len().saturating_sub(self.strm.avail_out as usize);

        if ret == BZ_STREAM_END {
            Ok((in_consumed, out_produced, true))
        } else if ret == BZ_RUN_OK || ret == BZ_FINISH_OK || ret == BZ_FLUSH_OK {
            Ok((in_consumed, out_produced, false))
        } else {
            Err(map_bz_error(ret))
        }
    }
}

impl Drop for Bzip2Compressor {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                BZ2_bzCompressEnd(&mut *self.strm);
            }
            self.initialized = false;
        }
    }
}

// MARK: - Safe RAII Streaming Bzip2 Decompressor

/// Safe RAII streaming Bzip2 decompressor wrapping `libbz2`.
pub struct Bzip2Decompressor {
    strm: Box<BzStream>,
    initialized: bool,
    total_decompressed: usize,
    max_output_limit: usize,
}

unsafe impl Send for Bzip2Decompressor {}

impl Bzip2Decompressor {
    /// Creates a new Bzip2 decompressor with specified output memory limit.
    pub fn new(small_memory: bool, max_output_limit: usize) -> Result<Self, TTZipStatus> {
        let mut strm = Box::new(BzStream::default());
        let small = if small_memory { 1 } else { 0 };
        let ret = unsafe { BZ2_bzDecompressInit(&mut *strm, 0, small) };
        if ret != BZ_OK {
            return Err(map_bz_error(ret));
        }

        Ok(Self {
            strm,
            initialized: true,
            total_decompressed: 0,
            max_output_limit: if max_output_limit == 0 {
                DEFAULT_MAX_DECOMPRESSED_LIMIT
            } else {
                max_output_limit
            },
        })
    }

    /// Decompresses a chunk of input data into output buffer.
    /// Returns `(consumed_bytes, written_bytes, is_stream_end)`.
    pub fn decompress_chunk(
        &mut self,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(usize, usize, bool), TTZipStatus> {
        if !self.initialized {
            return Err(TTZipStatus::ErrExtractionFailed);
        }

        self.strm.next_in = if input.is_empty() {
            std::ptr::null()
        } else {
            input.as_ptr() as *const libc::c_char
        };
        self.strm.avail_in = input.len().min(u32::MAX as usize) as libc::c_uint;

        self.strm.next_out = if output.is_empty() {
            std::ptr::null_mut()
        } else {
            output.as_mut_ptr() as *mut libc::c_char
        };
        self.strm.avail_out = output.len().min(u32::MAX as usize) as libc::c_uint;

        let ret = unsafe { BZ2_bzDecompress(&mut *self.strm) };

        let in_consumed = input.len().saturating_sub(self.strm.avail_in as usize);
        let out_produced = output.len().saturating_sub(self.strm.avail_out as usize);

        self.total_decompressed = self.total_decompressed.saturating_add(out_produced);
        if self.total_decompressed > self.max_output_limit {
            return Err(TTZipStatus::ErrSolidBudgetExceeded);
        }

        if ret == BZ_STREAM_END {
            Ok((in_consumed, out_produced, true))
        } else if ret == BZ_OK {
            Ok((in_consumed, out_produced, false))
        } else {
            Err(map_bz_error(ret))
        }
    }

    /// Returns total uncompressed bytes produced so far.
    #[inline]
    pub fn total_decompressed(&self) -> usize {
        self.total_decompressed
    }
}

impl Drop for Bzip2Decompressor {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                BZ2_bzDecompressEnd(&mut *self.strm);
            }
            self.initialized = false;
        }
    }
}

// MARK: - Header Inspection & Stream Validation

/// Metadata descriptor for a parsed Bzip2 stream header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bzip2HeaderInfo {
    pub level: u8,
    pub block_size_bytes: usize,
    pub is_valid: bool,
}

/// Inspects and parses standard 4-byte Bzip2 header (`BZh1`..`BZh9`).
pub fn bzip2_inspect_header(header_slice: &[u8]) -> Result<Bzip2HeaderInfo, TTZipStatus> {
    if header_slice.len() < 4 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    if &header_slice[0..3] != BZ_MAGIC {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let digit = header_slice[3];
    if !(b'1'..=b'9').contains(&digit) {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let level = digit - b'0';
    let block_size_bytes = (level as usize) * 100_000;

    Ok(Bzip2HeaderInfo {
        level,
        block_size_bytes,
        is_valid: true,
    })
}

// MARK: - Public In-Memory Convenience Functions

/// Computes worst-case output buffer capacity required for Bzip2 compression.
#[inline]
pub fn bzip2_compress_bound(src_len: usize) -> usize {
    src_len + (src_len / 100) + 1024
}

/// Compresses a slice using Bzip2 with specified level (1..=9) into a pre-allocated buffer.
pub fn bzip2_compress(src: &[u8], dst: &mut [u8], level: i32) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }

    if src.len() > u32::MAX as usize || dst.len() > u32::MAX as usize {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let mut dst_len = dst.len() as libc::c_uint;
    let clamped_level = level.clamp(1, 9);

    let ret = unsafe {
        BZ2_bzBuffToBuffCompress(
            dst.as_mut_ptr() as *mut libc::c_char,
            &mut dst_len,
            src.as_ptr() as *const libc::c_char,
            src.len() as libc::c_uint,
            clamped_level,
            0,
            30,
        )
    };

    if ret == BZ_OK {
        Ok(dst_len as usize)
    } else {
        Err(map_bz_error(ret))
    }
}

/// Decompresses a Bzip2 compressed slice into a pre-allocated destination buffer.
pub fn bzip2_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }

    if src.len() < 4 || &src[0..3] != BZ_MAGIC {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    if src.len() > u32::MAX as usize || dst.len() > u32::MAX as usize {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let mut dst_len = dst.len() as libc::c_uint;
    let ret = unsafe {
        BZ2_bzBuffToBuffDecompress(
            dst.as_mut_ptr() as *mut libc::c_char,
            &mut dst_len,
            src.as_ptr() as *const libc::c_char,
            src.len() as libc::c_uint,
            0,
            0,
        )
    };

    if ret == BZ_OK {
        Ok(dst_len as usize)
    } else {
        Err(map_bz_error(ret))
    }
}

/// Compresses a slice using Bzip2 and returns an owned `Vec<u8>`.
pub fn bzip2_compress_to_vec(src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() {
        return Ok(Vec::new());
    }

    let bound = bzip2_compress_bound(src.len());
    let mut dst = vec![0u8; bound];
    let written = bzip2_compress(src, &mut dst, level)?;
    dst.truncate(written);
    Ok(dst)
}

/// Decompresses a Bzip2 compressed slice into an owned `Vec<u8>` with maximum size bounds.
pub fn bzip2_decompress_to_vec(
    src: &[u8],
    max_output_limit: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() {
        return Ok(Vec::new());
    }

    if src.len() < 4 || &src[0..3] != BZ_MAGIC {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let limit = if max_output_limit == 0 {
        DEFAULT_MAX_DECOMPRESSED_LIMIT
    } else {
        max_output_limit
    };

    let mut capacity = (src.len() * 4).max(4096).min(limit);
    loop {
        let mut out = vec![0u8; capacity];
        match bzip2_decompress(src, &mut out) {
            Ok(written) => {
                out.truncate(written);
                return Ok(out);
            }
            Err(TTZipStatus::ErrExtractionFailed) => {
                if capacity >= limit {
                    return Err(TTZipStatus::ErrSolidBudgetExceeded);
                }
                let next_cap = (capacity.saturating_mul(2)).min(limit);
                if next_cap == capacity {
                    return Err(TTZipStatus::ErrSolidBudgetExceeded);
                }
                capacity = next_cap;
            }
            Err(e) => return Err(e),
        }
    }
}


// MARK: - Streaming Read/Write Adapters

/// Streaming pipe decompressor from `Read` source to `Write` sink.
pub fn bzip2_decompress_stream_pipe<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    max_output_bytes: u64,
) -> Result<u64, TTZipStatus> {
    let limit = if max_output_bytes == 0 {
        DEFAULT_MAX_DECOMPRESSED_LIMIT as u64
    } else {
        max_output_bytes
    };

    let mut decompressor = Bzip2Decompressor::new(false, limit as usize)?;
    let mut in_buf = vec![0u8; BZIP2_PIPE_BUFFER_SIZE];
    let mut out_buf = vec![0u8; BZIP2_PIPE_BUFFER_SIZE];
    let mut total_written: u64 = 0;

    loop {
        let in_read = reader.read(&mut in_buf).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
        if in_read == 0 {
            break;
        }

        let mut in_pos = 0;
        while in_pos < in_read {
            let (consumed, produced, is_end) =
                decompressor.decompress_chunk(&in_buf[in_pos..in_read], &mut out_buf)?;
            in_pos += consumed;

            if produced > 0 {
                total_written = total_written.saturating_add(produced as u64);
                if total_written > limit {
                    return Err(TTZipStatus::ErrSolidBudgetExceeded);
                }
                writer
                    .write_all(&out_buf[..produced])
                    .map_err(|_| TTZipStatus::ErrExtractionFailed)?;
            }

            if is_end {
                return Ok(total_written);
            }

            if consumed == 0 && produced == 0 {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
        }
    }

    Ok(total_written)
}

/// Streaming pipe compressor from `Read` source to `Write` sink.
pub fn bzip2_compress_stream_pipe<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    level: i32,
) -> Result<u64, TTZipStatus> {
    let mut compressor = Bzip2Compressor::new(level)?;
    let mut in_buf = vec![0u8; BZIP2_PIPE_BUFFER_SIZE];
    let mut out_buf = vec![0u8; BZIP2_PIPE_BUFFER_SIZE];
    let mut total_written: u64 = 0;

    loop {
        let in_read = reader.read(&mut in_buf).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        let is_eof = in_read == 0;

        let mut in_pos = 0;
        loop {
            let (consumed, produced, is_finished) =
                compressor.compress_chunk(&in_buf[in_pos..in_read], &mut out_buf, is_eof)?;
            in_pos += consumed;

            if produced > 0 {
                writer
                    .write_all(&out_buf[..produced])
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                total_written = total_written.saturating_add(produced as u64);
            }

            if is_eof && is_finished {
                return Ok(total_written);
            }

            if in_pos >= in_read && !is_eof {
                break;
            }

            if consumed == 0 && produced == 0 {
                break;
            }
        }

        if is_eof {
            break;
        }
    }

    Ok(total_written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bzip2_header_inspection() {
        let valid_header = b"BZh9";
        let info = bzip2_inspect_header(valid_header).expect("valid header");
        assert_eq!(info.level, 9);
        assert_eq!(info.block_size_bytes, 900_000);
        assert!(info.is_valid);

        let invalid_magic = b"PK\x03\x04";
        assert!(bzip2_inspect_header(invalid_magic).is_err());

        let invalid_digit = b"BZh0";
        assert!(bzip2_inspect_header(invalid_digit).is_err());
    }

    #[test]
    fn test_bzip2_roundtrip_all_levels() {
        let payload = b"TTZip Bzip2 high-efficiency BWT and Huffman statistical compression testing payload with repeated runs aaaaaaaaaabbbbbbbbcccccccc 1234567890.";

        for level in 1..=9 {
            let compressed = bzip2_compress_to_vec(payload, level).expect("compress");
            assert!(!compressed.is_empty());
            assert_eq!(&compressed[0..3], BZ_MAGIC);
            assert_eq!(compressed[3], b'0' + level as u8);

            let decompressed = bzip2_decompress_to_vec(&compressed, 1024 * 1024).expect("decompress");
            assert_eq!(decompressed.as_slice(), payload.as_slice());
        }
    }

    #[test]
    fn test_bzip2_streaming_pipe_roundtrip() {
        let payload = b"TTZip Bzip2 Streaming Pipe verification test with large data patterns repeated ".repeat(200);

        let mut compressed_sink = Vec::new();
        let mut reader = &payload[..];
        let written = bzip2_compress_stream_pipe(&mut reader, &mut compressed_sink, 6)
            .expect("compress stream pipe");
        assert_eq!(written as usize, compressed_sink.len());

        let mut decompressed_sink = Vec::new();
        let mut comp_reader = &compressed_sink[..];
        let decomp_written = bzip2_decompress_stream_pipe(&mut comp_reader, &mut decompressed_sink, 10 * 1024 * 1024)
            .expect("decompress stream pipe");

        assert_eq!(decomp_written as usize, payload.len());
        assert_eq!(decompressed_sink, payload);
    }

    #[test]
    fn test_bzip2_decompression_bomb_guard() {
        let large_payload = vec![b'A'; 200_000];
        let compressed = bzip2_compress_to_vec(&large_payload, 9).expect("compress");

        // Set maximum decompression budget to 10KB (far smaller than 200KB uncompressed)
        let res = bzip2_decompress_to_vec(&compressed, 10 * 1024);
        assert_eq!(res, Err(TTZipStatus::ErrSolidBudgetExceeded));
    }

    #[test]
    fn test_bzip2_corrupt_stream_defense() {
        let corrupt_data = b"BZh9CORRUPTED_STREAM_DATA_NOT_BZIP2_BODY_1234567890";
        let res = bzip2_decompress_to_vec(corrupt_data, 1024 * 1024);
        assert!(res.is_err());
    }
}
