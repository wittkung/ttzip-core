// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Safe, canonical Bzip2 compression and decompression microkernel backed by native C `libbz2`.

use crate::types::TTZipStatus;
use std::io::{self, Read, Write};

pub const BZ_MAGIC: [u8; 3] = *b"BZh";
pub const BZ_PI_BLOCK_MAGIC: [u8; 6] = [0x31, 0x41, 0x59, 0x26, 0x53, 0x59];
pub const BZ_EOS_BLOCK_MAGIC: [u8; 6] = [0x17, 0x72, 0x45, 0x38, 0x50, 0x90];
pub const BZIP2_PIPE_BUFFER_SIZE: usize = 64 * 1024;

// MARK: - Native C libbz2 FFI Bindings

#[repr(C)]
struct BzStream {
    next_in: *mut libc::c_char,
    avail_in: libc::c_uint,
    total_in_lo32: libc::c_uint,
    total_in_hi32: libc::c_uint,
    next_out: *mut libc::c_char,
    avail_out: libc::c_uint,
    total_out_lo32: libc::c_uint,
    total_out_hi32: libc::c_uint,
    state: *mut libc::c_void,
    bzalloc: Option<
        unsafe extern "C" fn(*mut libc::c_void, libc::c_int, libc::c_int) -> *mut libc::c_void,
    >,
    bzfree: Option<unsafe extern "C" fn(*mut libc::c_void, *mut libc::c_void)>,
    opaque: *mut libc::c_void,
}

impl BzStream {
    fn zeroed() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

unsafe extern "C" {
    fn BZ2_bzBuffToBuffCompress(
        dest: *mut libc::c_char,
        destLen: *mut libc::c_uint,
        source: *const libc::c_char,
        sourceLen: libc::c_uint,
        blockSize100k: libc::c_int,
        verbosity: libc::c_int,
        workFactor: libc::c_int,
    ) -> libc::c_int;
    fn BZ2_bzCompressInit(
        strm: *mut BzStream,
        blockSize100k: libc::c_int,
        verbosity: libc::c_int,
        workFactor: libc::c_int,
    ) -> libc::c_int;
    fn BZ2_bzCompress(strm: *mut BzStream, action: libc::c_int) -> libc::c_int;
    fn BZ2_bzCompressEnd(strm: *mut BzStream) -> libc::c_int;
    fn BZ2_bzDecompressInit(strm: *mut BzStream, verbosity: libc::c_int, small: libc::c_int) -> libc::c_int;
    fn BZ2_bzDecompress(strm: *mut BzStream) -> libc::c_int;
    fn BZ2_bzDecompressEnd(strm: *mut BzStream) -> libc::c_int;
}

const BZ_RUN: libc::c_int = 0;
const BZ_FINISH: libc::c_int = 2;

const BZ_OK: libc::c_int = 0;
const BZ_RUN_OK: libc::c_int = 1;
const BZ_FINISH_OK: libc::c_int = 3;
const BZ_STREAM_END: libc::c_int = 4;
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
    if header.len() < 4 || header[0] != b'B' || header[1] != b'Z' || header[2] != b'h' {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let lvl_char = header[3];
    if !(b'1'..=b'9').contains(&lvl_char) {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let block_size_100k = lvl_char - b'0';
    Ok(Bzip2HeaderInfo {
        is_valid: true,
        level: block_size_100k,
        block_size_100k,
        block_size_bytes: (block_size_100k as usize) * 100_000,
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
    match res {
        BZ_OK => Ok(dest_len as usize),
        BZ_OUTBUFF_FULL => Err(TTZipStatus::ErrInvalidParam),
        _ => Err(TTZipStatus::ErrCompressionFailed),
    }
}

/// Decompresses a Bzip2-compressed byte slice (including multi-member streams) into a destination buffer.
pub fn bzip2_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.len() < 4 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let mut decompressor = Bzip2Decompressor::new(false, 0)?;
    let (consumed, produced, finished) = decompressor.decompress_chunk(src, dst)?;
    if finished && consumed == src.len() {
        return Ok(produced);
    }
    if produced == dst.len() {
        let (_, extra_out, _) = decompressor.step_decompress(&src[consumed..], &mut [0u8; 1]);
        if consumed < src.len() || extra_out > 0 {
            return Err(TTZipStatus::ErrInvalidParam);
        }
    }
    Err(TTZipStatus::ErrExtractionFailed)
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
        return match bzip2_decompress(src, &mut empty) {
            Ok(0) => Ok(Vec::new()),
            Ok(_) | Err(TTZipStatus::ErrInvalidParam) => Err(TTZipStatus::ErrExtractionFailed),
            Err(e) => Err(e),
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

fn read_chunk_full<R: Read>(reader: &mut R, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}

/// Streams compression from a reader to a writer in bounded `level * 100_000` byte Bzip2 members.
pub fn bzip2_compress_stream_pipe<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    level: i32,
) -> Result<u64, TTZipStatus> {
    let clamped_level = level.clamp(1, 9);
    let block_bytes = (clamped_level as usize) * 100_000;
    let mut in_buf = vec![0u8; block_bytes];
    let mut out_buf = vec![0u8; bzip2_compress_bound(block_bytes)];
    let mut total_in: u64 = 0;

    loop {
        let n = read_chunk_full(reader, &mut in_buf).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
        if n == 0 {
            if total_in == 0 {
                let written = bzip2_compress(&[], &mut out_buf, clamped_level)?;
                writer
                    .write_all(&out_buf[..written])
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            }
            break;
        }
        total_in = total_in.saturating_add(n as u64);
        let written = bzip2_compress(&in_buf[..n], &mut out_buf, clamped_level)?;
        writer
            .write_all(&out_buf[..written])
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    }

    writer.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    Ok(total_in)
}

/// Streams decompression from a reader to a writer in bounded 64 KiB chunks.
pub fn bzip2_decompress_stream_pipe<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
) -> Result<u64, TTZipStatus> {
    let mut bz_reader = Bzip2Reader::new(reader).map_err(|e| {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            TTZipStatus::ErrCorruptHeader
        } else {
            TTZipStatus::ErrExtractionFailed
        }
    })?;
    let mut out_buf = vec![0u8; BZIP2_PIPE_BUFFER_SIZE];
    let mut total_out: u64 = 0;

    loop {
        let n = bz_reader
            .read(&mut out_buf)
            .map_err(|_| TTZipStatus::ErrExtractionFailed)?;
        if n == 0 {
            break;
        }
        writer
            .write_all(&out_buf[..n])
            .map_err(|_| TTZipStatus::ErrExtractionFailed)?;
        total_out = total_out.saturating_add(n as u64);
    }

    writer.flush().map_err(|_| TTZipStatus::ErrExtractionFailed)?;
    Ok(total_out)
}

/// Stateful streaming decompressor backed by native C `libbz2` `bz_stream`.
pub struct Bzip2Decompressor {
    strm: Box<BzStream>,
    small: bool,
    verbosity: i32,
    initialized: bool,
    at_member_boundary: bool,
}

unsafe impl Send for Bzip2Decompressor {}

impl Bzip2Decompressor {
    pub fn new(small: bool, verbosity: i32) -> Result<Self, TTZipStatus> {
        let mut dec = Self {
            strm: Box::new(BzStream::zeroed()),
            small,
            verbosity,
            initialized: false,
            at_member_boundary: false,
        };
        dec.reset_stream()?;
        Ok(dec)
    }

    fn reset_stream(&mut self) -> Result<(), TTZipStatus> {
        if self.initialized {
            unsafe {
                let _ = BZ2_bzDecompressEnd(&mut *self.strm);
            }
            self.initialized = false;
        }
        *self.strm = BzStream::zeroed();
        let res = unsafe {
            BZ2_bzDecompressInit(&mut *self.strm, self.verbosity as libc::c_int, libc::c_int::from(self.small))
        };
        if res != BZ_OK {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        self.initialized = true;
        Ok(())
    }

    fn step_decompress(&mut self, src: &[u8], dst: &mut [u8]) -> (usize, usize, libc::c_int) {
        let avail_in = src.len().min(u32::MAX as usize) as libc::c_uint;
        let avail_out = dst.len().min(u32::MAX as usize) as libc::c_uint;
        self.strm.next_in = src.as_ptr() as *mut libc::c_char;
        self.strm.avail_in = avail_in;
        self.strm.next_out = dst.as_mut_ptr() as *mut libc::c_char;
        self.strm.avail_out = avail_out;
        let res = unsafe { BZ2_bzDecompress(&mut *self.strm) };
        ((avail_in - self.strm.avail_in) as usize, (avail_out - self.strm.avail_out) as usize, res)
    }

    pub fn decompress_chunk(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
    ) -> Result<(usize, usize, bool), TTZipStatus> {
        let (mut in_pos, mut out_pos) = (0usize, 0usize);
        while in_pos < src.len() || (!self.at_member_boundary && out_pos < dst.len()) {
            let (consumed, produced, res) = self.step_decompress(&src[in_pos..], &mut dst[out_pos..]);
            in_pos += consumed;
            out_pos += produced;
            if res == BZ_STREAM_END {
                self.reset_stream()?;
                self.at_member_boundary = true;
                if in_pos == src.len() {
                    break;
                }
                continue;
            }
            if res != BZ_OK {
                return Err(TTZipStatus::ErrExtractionFailed);
            }
            self.at_member_boundary = false;
            if consumed == 0 && produced == 0 {
                break;
            }
        }
        Ok((in_pos, out_pos, self.at_member_boundary && in_pos == src.len()))
    }
}

impl Drop for Bzip2Decompressor {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                let _ = BZ2_bzDecompressEnd(&mut *self.strm);
            }
            self.initialized = false;
        }
    }
}

/// Stateful streaming compressor backed by native C `libbz2` `bz_stream`.
pub struct Bzip2Compressor {
    strm: Box<BzStream>,
    initialized: bool,
    finished: bool,
}

unsafe impl Send for Bzip2Compressor {}

impl Bzip2Compressor {
    pub fn new(level: i32, verbosity: i32, work_factor: i32) -> Result<Self, TTZipStatus> {
        let mut strm = Box::new(BzStream::zeroed());
        let res = unsafe {
            BZ2_bzCompressInit(
                &mut *strm,
                level.clamp(1, 9) as libc::c_int,
                verbosity.max(0) as libc::c_int,
                work_factor.clamp(0, 250) as libc::c_int,
            )
        };
        if res != BZ_OK {
            return Err(TTZipStatus::ErrCompressionFailed);
        }
        Ok(Self { strm, initialized: true, finished: false })
    }

    fn step_compress(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
        action: libc::c_int,
    ) -> Result<(usize, usize, bool), TTZipStatus> {
        let avail_in = src.len().min(u32::MAX as usize) as libc::c_uint;
        let avail_out = dst.len().min(u32::MAX as usize) as libc::c_uint;
        self.strm.next_in = src.as_ptr() as *mut libc::c_char;
        self.strm.avail_in = avail_in;
        self.strm.next_out = dst.as_mut_ptr() as *mut libc::c_char;
        self.strm.avail_out = avail_out;

        let res = unsafe { BZ2_bzCompress(&mut *self.strm, action) };
        let consumed = (avail_in - self.strm.avail_in) as usize;
        let produced = (avail_out - self.strm.avail_out) as usize;
        if res == BZ_STREAM_END {
            self.finished = true;
            return Ok((consumed, produced, true));
        }
        if res != BZ_RUN_OK && res != BZ_FINISH_OK {
            return Err(TTZipStatus::ErrCompressionFailed);
        }
        Ok((consumed, produced, false))
    }

    pub fn compress_chunk(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
        finish: bool,
    ) -> Result<(usize, usize, bool), TTZipStatus> {
        if self.finished {
            return Ok((0, 0, true));
        }
        let mut in_pos = 0usize;
        let mut out_pos = 0usize;
        let action = if finish { BZ_FINISH } else { BZ_RUN };

        while out_pos < dst.len() && (in_pos < src.len() || finish) {
            let (consumed, produced, done) =
                self.step_compress(&src[in_pos..], &mut dst[out_pos..], action)?;
            in_pos += consumed;
            out_pos += produced;
            if done || (consumed == 0 && produced == 0) {
                break;
            }
        }

        if finish && !self.finished && out_pos == dst.len() {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        Ok((in_pos, out_pos, self.finished))
    }
}

impl Drop for Bzip2Compressor {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                let _ = BZ2_bzCompressEnd(&mut *self.strm);
            }
            self.initialized = false;
        }
    }
}

/// Streaming Bzip2 reader implementing `std::io::Read` with bounded 64 KiB input buffering.
pub struct Bzip2Reader<R: Read> {
    source: R,
    decompressor: Bzip2Decompressor,
    header: [u8; 4],
    in_buf: Vec<u8>,
    in_pos: usize,
    in_len: usize,
    eof_reached: bool,
    in_member: bool,
}

impl<R: Read> Bzip2Reader<R> {
    pub fn new(mut source: R) -> io::Result<Self> {
        let mut header = [0u8; 4];
        let n = read_chunk_full(&mut source, &mut header)?;
        if n < 4 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Invalid Bzip2 header",
            ));
        }
        bzip2_inspect_header(&header)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid Bzip2 header"))?;
        let decompressor = Bzip2Decompressor::new(false, 0)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Bzip2 init error"))?;
        let mut in_buf = vec![0u8; BZIP2_PIPE_BUFFER_SIZE];
        in_buf[..4].copy_from_slice(&header);
        Ok(Self {
            source,
            decompressor,
            header,
            in_buf,
            in_pos: 0,
            in_len: 4,
            eof_reached: false,
            in_member: true,
        })
    }

    pub fn header(&self) -> &[u8; 4] {
        &self.header
    }

    fn refill_input_buffer(&mut self) -> io::Result<bool> {
        if self.in_pos < self.in_len {
            self.in_member = true;
            return Ok(true);
        }
        if !self.eof_reached {
            let n = self.source.read(&mut self.in_buf)?;
            if n > 0 {
                self.in_pos = 0;
                self.in_len = n;
                self.in_member = true;
                return Ok(true);
            }
            self.eof_reached = true;
        }
        if self.in_member {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Truncated Bzip2 stream",
            ));
        }
        Ok(false)
    }
}

impl<R: Read> Read for Bzip2Reader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            if self.in_pos < self.in_len || self.in_member {
                let (consumed, produced, member_done) = self
                    .decompressor
                    .decompress_chunk(&self.in_buf[self.in_pos..self.in_len], buf)
                    .map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, "Bzip2 decompress error")
                    })?;
                self.in_pos += consumed;
                self.in_member = !member_done && !self.decompressor.at_member_boundary;
                if produced > 0 {
                    return Ok(produced);
                }
            }
            if !self.refill_input_buffer()? {
                return Ok(0);
            }
        }
    }
}

/// Streaming Bzip2 writer implementing `std::io::Write` with bounded `level * 100_000` member flushing.
pub struct Bzip2Writer<W: Write> {
    sink: W,
    buffer: Vec<u8>,
    comp_buf: Vec<u8>,
    level: i32,
    emitted_member: bool,
}

impl<W: Write> Bzip2Writer<W> {
    pub fn new(sink: W, level: u32) -> Self {
        let clamped_level = (level as i32).clamp(1, 9);
        Self {
            sink,
            buffer: Vec::new(),
            comp_buf: Vec::new(),
            level: clamped_level,
            emitted_member: false,
        }
    }

    fn flush_member(&mut self) -> io::Result<()> {
        let bound = bzip2_compress_bound(self.buffer.len());
        if self.comp_buf.len() < bound {
            self.comp_buf.resize(bound, 0);
        }
        let written = bzip2_compress(&self.buffer, &mut self.comp_buf, self.level)
            .map_err(|_| io::Error::other("Bzip2 compression failed"))?;
        self.sink.write_all(&self.comp_buf[..written])?;
        self.buffer.clear();
        self.emitted_member = true;
        Ok(())
    }

    pub fn finish(mut self) -> io::Result<W> {
        if !self.buffer.is_empty() || !self.emitted_member {
            self.flush_member()?;
        }
        self.sink.flush()?;
        Ok(self.sink)
    }
}

impl<W: Write> Write for Bzip2Writer<W> {
    fn write(&mut self, mut buf: &[u8]) -> io::Result<usize> {
        let total = buf.len();
        let block_cap = (self.level.clamp(1, 9) as usize) * 100_000;
        while self.buffer.len() + buf.len() >= block_cap {
            let space = block_cap - self.buffer.len();
            self.buffer.extend_from_slice(&buf[..space]);
            buf = &buf[space..];
            self.flush_member()?;
        }
        if !buf.is_empty() {
            self.buffer.extend_from_slice(buf);
        }
        Ok(total)
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.buffer.is_empty() {
            self.flush_member()?;
        }
        self.sink.flush()
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

    #[test]
    fn test_bzip2_multi_member_stream_pipe_and_reader_writer_roundtrip() {
        let mut payload = Vec::with_capacity(250_000);
        for i in 0..250_000usize {
            payload.push(((i * 31 + (i >> 7)) & 0xFF) as u8);
        }

        let mut pipe_compressed = Vec::new();
        let mut pipe_in = std::io::Cursor::new(&payload);
        let in_count = bzip2_compress_stream_pipe(&mut pipe_in, &mut pipe_compressed, 1)
            .expect("bzip2 multi-member pipe compress");
        assert_eq!(in_count, payload.len() as u64);

        let mut pipe_decompressed = Vec::new();
        let mut pipe_comp_in = std::io::Cursor::new(&pipe_compressed);
        let out_count = bzip2_decompress_stream_pipe(&mut pipe_comp_in, &mut pipe_decompressed)
            .expect("bzip2 multi-member pipe decompress");
        assert_eq!(out_count, payload.len() as u64);
        assert_eq!(pipe_decompressed, payload);

        let mut writer_compressed = Vec::new();
        {
            let mut writer = Bzip2Writer::new(&mut writer_compressed, 1);
            for chunk in payload.chunks(17_389) {
                writer.write_all(chunk).expect("bzip2 writer chunk");
            }
            writer.finish().expect("bzip2 writer finish");
        }

        let mut reader = Bzip2Reader::new(std::io::Cursor::new(&writer_compressed))
            .expect("bzip2 reader init");
        assert_eq!(reader.header(), b"BZh1");
        let mut reader_out = Vec::new();
        let mut small_buf = [0u8; 4096];
        loop {
            let n = reader.read(&mut small_buf).expect("bzip2 reader chunk");
            if n == 0 {
                break;
            }
            reader_out.extend_from_slice(&small_buf[..n]);
        }
        assert_eq!(reader_out, payload);
    }

    #[test]
    fn test_bzip2_empty_payload_zero_len_dst_and_zero_bound_vec() {
        let empty_comp = bzip2_compress_to_vec(b"", 9).expect("compress empty bzip2");
        let mut empty_dst = [];
        let n = bzip2_decompress(&empty_comp, &mut empty_dst)
            .expect("empty bzip2 stream must decode into zero-length buffer");
        assert_eq!(n, 0);

        let vec_out = bzip2_decompress_to_vec(&empty_comp, 0)
            .expect("empty bzip2 stream must decode with max_allowed = 0");
        assert!(vec_out.is_empty());

        let non_empty_comp = bzip2_compress_to_vec(b"non-empty payload", 1)
            .expect("compress non-empty bzip2");
        assert_eq!(
            bzip2_decompress(&non_empty_comp, &mut empty_dst),
            Err(TTZipStatus::ErrInvalidParam)
        );
        assert_eq!(
            bzip2_decompress_to_vec(&non_empty_comp, 0),
            Err(TTZipStatus::ErrExtractionFailed)
        );
    }
}
