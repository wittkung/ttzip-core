// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Safe, canonical Bzip2 compression and decompression microkernel backed by native C `libbz2`.

use crate::types::TTZipStatus;
use std::io::{self, Cursor, Read, Write};

pub const BZ_MAGIC: [u8; 3] = *b"BZh";
pub const BZ_PI_BLOCK_MAGIC: [u8; 6] = [0x31, 0x41, 0x59, 0x26, 0x53, 0x59];
pub const BZ_EOS_BLOCK_MAGIC: [u8; 6] = [0x17, 0x72, 0x45, 0x38, 0x50, 0x90];
pub const BZIP2_PIPE_BUFFER_SIZE: usize = 64 * 1024;

// MARK: - Native C libbz2 FFI Bindings

extern "C" {
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

const BZ_OK: libc::c_int = 0;
const BZ_OUTBUFF_FULL: libc::c_int = -8;

/// Header metadata inspected from a Bzip2 stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bzip2HeaderInfo {
    pub is_valid: bool,
    pub level: u8,
    pub block_size_100k: u8,
    pub block_size_bytes: usize,
}

/// Returns the worst-case compressed buffer capacity bound for Bzip2.
#[inline]
pub fn bzip2_compress_bound(src_len: usize) -> usize {
    src_len + (src_len / 100) + 600
}

/// Inspects the 4-byte header of a Bzip2 stream.
pub fn bzip2_inspect_header(header: &[u8]) -> Result<Bzip2HeaderInfo, TTZipStatus> {
    if header.len() < 4 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    if header[0] != b'B' || header[1] != b'Z' || header[2] != b'h' {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let lvl_char = header[3];
    if !(b'1'..=b'9').contains(&lvl_char) {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let block_size_100k = lvl_char - b'0';
    let block_size_bytes = (block_size_100k as usize) * 100_000;
    Ok(Bzip2HeaderInfo {
        is_valid: true,
        level: block_size_100k,
        block_size_100k,
        block_size_bytes,
    })
}

/// Validates whether a byte slice is a valid Bzip2 stream header.
pub fn bzip2_validate(src: &[u8]) -> bool {
    bzip2_inspect_header(src).is_ok()
}

/// Compresses a byte slice into a pre-allocated destination buffer using native C `libbz2`.
pub fn bzip2_compress(src: &[u8], dst: &mut [u8], level: i32) -> Result<usize, TTZipStatus> {
    let block_size = level.clamp(1, 9) as libc::c_int;
    let mut dest_len = dst.len() as libc::c_uint;

    let res = unsafe {
        BZ2_bzBuffToBuffCompress(
            dst.as_mut_ptr() as *mut libc::c_char,
            &mut dest_len,
            src.as_ptr() as *const libc::c_char,
            src.len() as libc::c_uint,
            block_size,
            0,
            30,
        )
    };

    if res == BZ_OK {
        Ok(dest_len as usize)
    } else if res == BZ_OUTBUFF_FULL {
        Err(TTZipStatus::ErrInvalidParam)
    } else {
        Err(TTZipStatus::ErrCompressionFailed)
    }
}

/// Decompresses a Bzip2-compressed byte slice into a destination buffer using native C `libbz2`.
pub fn bzip2_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.len() < 4 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let mut dest_len = dst.len() as libc::c_uint;

    let res = unsafe {
        BZ2_bzBuffToBuffDecompress(
            dst.as_mut_ptr() as *mut libc::c_char,
            &mut dest_len,
            src.as_ptr() as *const libc::c_char,
            src.len() as libc::c_uint,
            0,
            0,
        )
    };

    if res == BZ_OK {
        Ok(dest_len as usize)
    } else if res == BZ_OUTBUFF_FULL {
        Err(TTZipStatus::ErrInvalidParam)
    } else {
        Err(TTZipStatus::ErrExtractionFailed)
    }
}

/// Compresses a byte slice and returns the compressed bytes as a newly allocated `Vec<u8>`.
pub fn bzip2_compress_to_vec(src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
    let bound = bzip2_compress_bound(src.len());
    let mut out = vec![0u8; bound];
    let written = bzip2_compress(src, &mut out, level)?;
    out.truncate(written);
    Ok(out)
}

/// Alias for `bzip2_compress_to_vec`.
#[inline]
pub fn bzip2_compress_vec(src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
    bzip2_compress_to_vec(src, level)
}

/// Decompresses a Bzip2-compressed byte slice and returns decompressed bytes as `Vec<u8>`.
pub fn bzip2_decompress_vec(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    bzip2_decompress_to_vec(src, 1024 * 1024 * 1024)
}

/// Decompresses a Bzip2-compressed byte slice with an explicit `max_allowed` output bound.
pub fn bzip2_decompress_to_vec(src: &[u8], max_allowed: usize) -> Result<Vec<u8>, TTZipStatus> {
    let hard_cap = max_allowed.min(1024 * 1024 * 1024);
    if hard_cap == 0 {
        let mut empty = [];
        let n = bzip2_decompress(src, &mut empty)?;
        return if n == 0 {
            Ok(Vec::new())
        } else {
            Err(TTZipStatus::ErrExtractionFailed)
        };
    }
    let mut capacity = (src.len().saturating_mul(4)).max(65536).min(hard_cap);
    loop {
        let mut out = vec![0u8; capacity];
        match bzip2_decompress(src, &mut out) {
            Ok(n) => {
                out.truncate(n);
                return Ok(out);
            }
            Err(TTZipStatus::ErrInvalidParam) => {
                if capacity >= hard_cap {
                    return Err(TTZipStatus::ErrExtractionFailed);
                }
                capacity = capacity.saturating_mul(2).min(hard_cap);
            }
            Err(e) => return Err(e),
        }
    }
}

/// Streams compression from a reader to a writer over a chunked pipe.
pub fn bzip2_compress_stream_pipe<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    level: i32,
) -> Result<u64, TTZipStatus> {
    let mut raw_input = Vec::new();
    reader
        .read_to_end(&mut raw_input)
        .map_err(|_| TTZipStatus::ErrExtractionFailed)?;
    let in_bytes = raw_input.len() as u64;

    let compressed = bzip2_compress_to_vec(&raw_input, level)?;
    writer
        .write_all(&compressed)
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    Ok(in_bytes)
}

/// Streams decompression from a reader to a writer over a chunked pipe.
pub fn bzip2_decompress_stream_pipe<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
) -> Result<u64, TTZipStatus> {
    let mut raw_input = Vec::new();
    reader
        .read_to_end(&mut raw_input)
        .map_err(|_| TTZipStatus::ErrExtractionFailed)?;

    let decompressed = bzip2_decompress_vec(&raw_input)?;
    let out_bytes = decompressed.len() as u64;
    writer
        .write_all(&decompressed)
        .map_err(|_| TTZipStatus::ErrExtractionFailed)?;
    Ok(out_bytes)
}

/// Chunk-based decompressor state machine for filter pipeline.
pub struct Bzip2Decompressor {
    _small: bool,
    accumulated_in: Vec<u8>,
    decompressed_out: Vec<u8>,
    out_cursor: usize,
    finished: bool,
}

impl Bzip2Decompressor {
    pub fn new(small: bool, _verbosity: i32) -> Result<Self, TTZipStatus> {
        Ok(Self {
            _small: small,
            accumulated_in: Vec::new(),
            decompressed_out: Vec::new(),
            out_cursor: 0,
            finished: false,
        })
    }

    pub fn decompress_chunk(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
    ) -> Result<(usize, usize, bool), TTZipStatus> {
        self.accumulated_in.extend_from_slice(src);
        let in_consumed = src.len();

        if !self.finished && self.accumulated_in.len() >= 4 {
            if let Ok(out) = bzip2_decompress_vec(&self.accumulated_in) {
                self.decompressed_out = out;
                self.finished = true;
            }
        }

        let avail = self.decompressed_out.len() - self.out_cursor;
        let to_copy = avail.min(dst.len());
        if to_copy > 0 {
            dst[..to_copy].copy_from_slice(
                &self.decompressed_out[self.out_cursor..self.out_cursor + to_copy],
            );
            self.out_cursor += to_copy;
        }

        Ok((
            in_consumed,
            to_copy,
            self.finished && self.out_cursor >= self.decompressed_out.len(),
        ))
    }
}

/// Chunk-based compressor state machine for filter pipeline.
pub struct Bzip2Compressor {
    level: i32,
    accumulated_in: Vec<u8>,
}

impl Bzip2Compressor {
    pub fn new(level: i32, _verbosity: i32, _work_factor: i32) -> Result<Self, TTZipStatus> {
        Ok(Self {
            level,
            accumulated_in: Vec::new(),
        })
    }

    pub fn compress_chunk(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
        finish: bool,
    ) -> Result<(usize, usize, bool), TTZipStatus> {
        self.accumulated_in.extend_from_slice(src);
        let in_consumed = src.len();
        let mut out_produced = 0;
        let mut is_finished = false;

        if finish {
            let comp = bzip2_compress_to_vec(&self.accumulated_in, self.level)?;
            if comp.len() > dst.len() {
                return Err(TTZipStatus::ErrInvalidParam);
            }
            dst[..comp.len()].copy_from_slice(&comp);
            out_produced = comp.len();
            is_finished = true;
        }

        Ok((in_consumed, out_produced, is_finished))
    }
}

/// Streaming Bzip2 reader implementing `std::io::Read`.
pub struct Bzip2Reader<R: Read> {
    inner: Cursor<Vec<u8>>,
    header: [u8; 4],
    _phantom: std::marker::PhantomData<R>,
}

impl<R: Read> Bzip2Reader<R> {
    pub fn new(mut source: R) -> io::Result<Self> {
        let mut compressed = Vec::new();
        source.read_to_end(&mut compressed)?;
        if compressed.len() < 4 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid Bzip2 header"));
        }
        let header = [compressed[0], compressed[1], compressed[2], compressed[3]];
        bzip2_inspect_header(&header).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid Bzip2 header"))?;
        let decompressed = bzip2_decompress_vec(&compressed)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Bzip2 decompress error"))?;
        Ok(Self {
            inner: Cursor::new(decompressed),
            header,
            _phantom: std::marker::PhantomData,
        })
    }

    pub fn header(&self) -> &[u8; 4] {
        &self.header
    }
}

impl<R: Read> Read for Bzip2Reader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

/// Streaming Bzip2 writer implementing `std::io::Write`.
pub struct Bzip2Writer<W: Write> {
    sink: W,
    buffer: Vec<u8>,
    level: i32,
}

impl<W: Write> Bzip2Writer<W> {
    pub fn new(sink: W, level: u32) -> Self {
        Self {
            sink,
            buffer: Vec::new(),
            level: level as i32,
        }
    }

    pub fn finish(mut self) -> io::Result<W> {
        let compressed = bzip2_compress_to_vec(&self.buffer, self.level)
            .map_err(|_| io::Error::other("Bzip2 compression failed"))?;
        self.sink.write_all(&compressed)?;
        self.sink.flush()?;
        Ok(self.sink)
    }
}

impl<W: Write> Write for Bzip2Writer<W> {
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
    fn test_bzip2_basic_roundtrip() {
        let input = b"Hello canonical C libbz2 compression and decompression in TTZip 2026!";
        let mut comp = vec![0u8; bzip2_compress_bound(input.len())];
        let c_len = bzip2_compress(input, &mut comp, 9).expect("bzip2 compress");
        assert!(c_len > 0);

        let mut decomp = vec![0u8; input.len()];
        let d_len = bzip2_decompress(&comp[..c_len], &mut decomp).expect("bzip2 decompress");
        assert_eq!(d_len, input.len());
        assert_eq!(&decomp, input);
    }

    #[test]
    fn test_bzip2_to_vec_roundtrip() {
        let input = b"Repeating repetitive pattern designed to test bzip2 compression efficiency. ".repeat(50);
        let comp = bzip2_compress_to_vec(&input, 6).expect("compress to vec");
        assert!(comp.len() < input.len());

        let decomp = bzip2_decompress_vec(&comp).expect("decompress to vec");
        assert_eq!(decomp, input);
    }
}
