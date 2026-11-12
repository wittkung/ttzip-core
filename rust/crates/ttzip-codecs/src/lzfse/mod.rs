// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Safe, canonical Apple `LZFSE` and `LZVN` hardware/native block codecs backed by the Apple reference C library.

use crate::types::TTZipStatus;
use std::cell::RefCell;
use std::io::{self, Read, Write};

pub const LZFSE_BLOCK_CHUNK_SIZE: usize = 256 * 1024;
pub const LZFSE_CHUNK_SIZE: usize = LZFSE_BLOCK_CHUNK_SIZE;
pub const LZFSE_MAX_BLOCK_SIZE: usize = 256 * 1024;
pub const DEFAULT_LZVN_THRESHOLD: usize = 4096;

const LZFSE_ENDOFSTREAM_BLOCK_MAGIC: u32 = 0x2478_7662;
const LZFSE_UNCOMPRESSED_BLOCK_MAGIC: u32 = 0x2d78_7662;
const LZFSE_COMPRESSEDV1_BLOCK_MAGIC: u32 = 0x3178_7662;
const LZFSE_COMPRESSEDV2_BLOCK_MAGIC: u32 = 0x3278_7662;
const LZFSE_COMPRESSEDLZVN_BLOCK_MAGIC: u32 = 0x6e78_7662;
const LZFSE_EOS_BYTES: [u8; 4] = [0x62, 0x76, 0x78, 0x24];
const LZFSE_V1_HEADER_SIZE: usize = 772;
const LZFSE_V2_FIXED_PREFIX_SIZE: usize = 32;
const LZFSE_V2_MAX_HEADER_SIZE: usize = 776;
const LZFSE_MAX_HISTORY_BYTES: usize = 262_144;
const LZFSE_MAX_SINGLE_BLOCK_RAW: usize = 32 * 1024 * 1024;

// MARK: - Native C LZFSE & LZVN Bindings

#[repr(C)]
struct LzfseDecoderStatePrefix {
    src: *const u8,
    src_begin: *const u8,
    src_end: *const u8,
    dst: *mut u8,
    dst_begin: *mut u8,
    dst_end: *mut u8,
    end_of_stream: libc::c_int,
    block_magic: u32,
}

unsafe extern "C" {
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
    fn lzfse_decode(state: *mut LzfseDecoderStatePrefix) -> libc::c_int;
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
        lzfse_encode_buffer(dst.as_mut_ptr(), dst.len(), src.as_ptr(), src.len(), scratch)
    });
    if written == 0 { Err(TTZipStatus::ErrCompressionFailed) } else { Ok(written) }
}

#[inline]
pub fn lzfse_compress_raw(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    lzfse_compress_to_vec(src)
}

pub fn lzfse_compress_to_vec(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    let mut out = vec![0u8; lzfse_compress_bound(src.len())];
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
        lzfse_decode_buffer(dst.as_mut_ptr(), dst.len(), src.as_ptr(), src.len(), scratch)
    });
    if written == 0 { Err(TTZipStatus::ErrExtractionFailed) } else { Ok(written) }
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
    const MAX_CEILING: usize = 256 * 1024 * 1024;
    let mut capacity = (src.len() * 4).clamp(65536, MAX_CEILING);
    loop {
        let mut out = vec![0u8; capacity];
        let written = lzfse_decompress(src, &mut out)?;
        if written == out.len() {
            if capacity >= MAX_CEILING {
                return Err(TTZipStatus::ErrBufferTooSmall);
            }
            capacity = capacity.saturating_mul(2).min(MAX_CEILING);
        } else {
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
    if written == 0 { Err(TTZipStatus::ErrCompressionFailed) } else { Ok(written) }
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

fn read_exact_or_eof<R: Read>(reader: &mut R, buf: &mut [u8]) -> io::Result<usize> {
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

fn parse_v2_block_header<R: Read>(
    source: &mut R,
    comp_block: &mut Vec<u8>,
) -> io::Result<(usize, usize)> {
    let mut hdr = [0u8; LZFSE_V2_FIXED_PREFIX_SIZE - 4];
    if read_exact_or_eof(source, &mut hdr)? < hdr.len() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Truncated bvx2 header"));
    }
    comp_block.extend_from_slice(&hdr);
    let raw = u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]) as usize;
    let p0 = u64::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7], hdr[8], hdr[9], hdr[10], hdr[11]]);
    let p1 = u64::from_le_bytes([hdr[12], hdr[13], hdr[14], hdr[15], hdr[16], hdr[17], hdr[18], hdr[19]]);
    let p2 = u64::from_le_bytes([hdr[20], hdr[21], hdr[22], hdr[23], hdr[24], hdr[25], hdr[26], hdr[27]]);
    let header_size = (p2 & 0xFFFF_FFFF) as usize;
    if !(LZFSE_V2_FIXED_PREFIX_SIZE..=LZFSE_V2_MAX_HEADER_SIZE).contains(&header_size) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Corrupt bvx2 header size"));
    }
    let lit_payload = ((p0 >> 20) & 0xFFFFF) as usize;
    let lmd_payload = ((p1 >> 40) & 0xFFFFF) as usize;
    let rem = (header_size - LZFSE_V2_FIXED_PREFIX_SIZE)
        .saturating_add(lit_payload)
        .saturating_add(lmd_payload);
    Ok((raw, rem))
}

fn parse_v1_block_header<R: Read>(
    source: &mut R,
    comp_block: &mut Vec<u8>,
) -> io::Result<(usize, usize)> {
    let mut hdr = vec![0u8; LZFSE_V1_HEADER_SIZE - 4];
    if read_exact_or_eof(source, &mut hdr)? < hdr.len() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Truncated bvx1 header"));
    }
    comp_block.extend_from_slice(&hdr);
    let raw = u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]) as usize;
    let lit_payload = u32::from_le_bytes([hdr[16], hdr[17], hdr[18], hdr[19]]) as usize;
    let lmd_payload = u32::from_le_bytes([hdr[20], hdr[21], hdr[22], hdr[23]]) as usize;
    Ok((raw, lit_payload.saturating_add(lmd_payload)))
}

fn parse_uncompressed_block_header<R: Read>(
    source: &mut R,
    comp_block: &mut Vec<u8>,
) -> io::Result<(usize, usize)> {
    let mut hdr = [0u8; 4];
    if read_exact_or_eof(source, &mut hdr)? < 4 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Truncated bvx- header"));
    }
    comp_block.extend_from_slice(&hdr);
    let raw = u32::from_le_bytes(hdr) as usize;
    Ok((raw, raw))
}

fn parse_lzvn_block_header<R: Read>(
    source: &mut R,
    comp_block: &mut Vec<u8>,
) -> io::Result<(usize, usize)> {
    let mut hdr = [0u8; 8];
    if read_exact_or_eof(source, &mut hdr)? < 8 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Truncated bvxn header"));
    }
    comp_block.extend_from_slice(&hdr);
    let raw = u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]) as usize;
    let payload = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as usize;
    Ok((raw, payload))
}

fn read_lzfse_block_payload<R: Read>(
    source: &mut R,
    magic: u32,
    comp_block: &mut Vec<u8>,
) -> io::Result<usize> {
    comp_block.clear();
    comp_block.extend_from_slice(&magic.to_le_bytes());

    let (raw_len, rem_len) = match magic {
        LZFSE_UNCOMPRESSED_BLOCK_MAGIC => parse_uncompressed_block_header(source, comp_block)?,
        LZFSE_COMPRESSEDLZVN_BLOCK_MAGIC => parse_lzvn_block_header(source, comp_block)?,
        LZFSE_COMPRESSEDV1_BLOCK_MAGIC => parse_v1_block_header(source, comp_block)?,
        LZFSE_COMPRESSEDV2_BLOCK_MAGIC => parse_v2_block_header(source, comp_block)?,
        _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid LZFSE block magic")),
    };

    if raw_len.max(rem_len) > LZFSE_MAX_SINGLE_BLOCK_RAW {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "LZFSE block exceeds bound"));
    }

    let prev_len = comp_block.len();
    comp_block.resize(prev_len + rem_len, 0);
    if read_exact_or_eof(source, &mut comp_block[prev_len..])? < rem_len {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Truncated LZFSE block payload"));
    }
    comp_block.extend_from_slice(&LZFSE_EOS_BYTES);
    Ok(raw_len)
}

pub struct LzfseReader<R: Read> {
    source: R,
    comp_block: Vec<u8>,
    window: Vec<u8>,
    history_len: usize,
    read_pos: usize,
    decoded_end: usize,
    in_open_stream: bool,
    eof: bool,
}

impl<R: Read> LzfseReader<R> {
    pub fn new(source: R) -> io::Result<Self> {
        let mut reader = Self {
            source,
            comp_block: Vec::new(),
            window: Vec::new(),
            history_len: 0,
            read_pos: 0,
            decoded_end: 0,
            in_open_stream: false,
            eof: false,
        };
        reader.decode_next_block()?;
        Ok(reader)
    }

    fn slide_history(&mut self) {
        if self.decoded_end == 0 {
            return;
        }
        let keep = self.decoded_end.min(LZFSE_MAX_HISTORY_BYTES);
        let start = self.decoded_end - keep;
        if start > 0 && keep > 0 {
            self.window.copy_within(start..self.decoded_end, 0);
        }
        self.history_len = keep;
        self.read_pos = keep;
        self.decoded_end = keep;
    }

    fn decode_loaded_block(&mut self, raw_len: usize) -> io::Result<()> {
        let total_window = self.history_len + raw_len;
        if self.window.len() < total_window {
            self.window.resize(total_window, 0);
        }
        let scratch_size = unsafe { lzfse_decode_scratch_size() };
        let comp_ptr = self.comp_block.as_ptr();
        let comp_len = self.comp_block.len();
        let win_ptr = self.window.as_mut_ptr();
        let hist_len = self.history_len;

        let ok = with_lzfse_scratch(scratch_size, |scratch| unsafe {
            std::ptr::write_bytes(scratch as *mut u8, 0, scratch_size);
            let state = scratch as *mut LzfseDecoderStatePrefix;
            (*state).src = comp_ptr;
            (*state).src_begin = comp_ptr;
            (*state).src_end = comp_ptr.add(comp_len);
            (*state).dst_begin = win_ptr;
            (*state).dst = win_ptr.add(hist_len);
            (*state).dst_end = win_ptr.add(total_window);
            let status = lzfse_decode(state);
            let written = ((*state).dst as usize).saturating_sub(win_ptr as usize);
            status == 0 && (*state).end_of_stream == 1 && written == total_window
        });

        if !ok {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "LZFSE decode error"));
        }
        self.read_pos = self.history_len;
        self.decoded_end = total_window;
        Ok(())
    }

    fn decode_next_block(&mut self) -> io::Result<()> {
        self.slide_history();
        loop {
            let mut magic_bytes = [0u8; 4];
            let n = read_exact_or_eof(&mut self.source, &mut magic_bytes)?;
            if n == 0 {
                if self.in_open_stream {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Truncated LZFSE stream missing EOS",
                    ));
                }
                self.eof = true;
                return Ok(());
            }
            if n < 4 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Truncated LZFSE magic"));
            }
            let magic = u32::from_le_bytes(magic_bytes);
            if magic == LZFSE_ENDOFSTREAM_BLOCK_MAGIC {
                self.in_open_stream = false;
                self.history_len = 0;
                self.read_pos = 0;
                self.decoded_end = 0;
                continue;
            }
            self.in_open_stream = true;
            let raw_len = read_lzfse_block_payload(&mut self.source, magic, &mut self.comp_block)?;
            self.decode_loaded_block(raw_len)?;
            if raw_len > 0 {
                return Ok(());
            }
        }
    }
}

impl<R: Read> Read for LzfseReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        while self.read_pos >= self.decoded_end {
            if self.eof {
                return Ok(0);
            }
            self.decode_next_block()?;
        }
        let avail = self.decoded_end - self.read_pos;
        let to_copy = avail.min(buf.len());
        buf[..to_copy].copy_from_slice(&self.window[self.read_pos..self.read_pos + to_copy]);
        self.read_pos += to_copy;
        Ok(to_copy)
    }
}

pub struct LzfseWriter<W: Write> {
    sink: W,
    buffer: Vec<u8>,
    comp_buf: Vec<u8>,
    emitted_blocks: bool,
}

impl<W: Write> LzfseWriter<W> {
    pub fn new(sink: W) -> Self {
        Self {
            sink,
            buffer: Vec::new(),
            comp_buf: Vec::new(),
            emitted_blocks: false,
        }
    }

    fn flush_chunk(&mut self, is_final: bool) -> io::Result<()> {
        if self.buffer.is_empty() {
            if is_final && self.emitted_blocks {
                self.sink.write_all(&LZFSE_EOS_BYTES)?;
            }
            return Ok(());
        }
        let bound = lzfse_compress_bound(self.buffer.len());
        if self.comp_buf.len() < bound {
            self.comp_buf.resize(bound, 0);
        }
        let written = lzfse_compress(&self.buffer, &mut self.comp_buf)
            .map_err(|_| io::Error::other("LZFSE compression failed"))?;
        if written < 4 || self.comp_buf[written - 4..written] != LZFSE_EOS_BYTES {
            return Err(io::Error::other("LZFSE block missing EOS marker"));
        }
        if is_final {
            self.sink.write_all(&self.comp_buf[..written])?;
        } else {
            self.sink.write_all(&self.comp_buf[..written - 4])?;
            self.emitted_blocks = true;
        }
        self.buffer.clear();
        Ok(())
    }

    pub fn finish(mut self) -> io::Result<W> {
        self.flush_chunk(true)?;
        self.sink.flush()?;
        Ok(self.sink)
    }
}

impl<W: Write> Write for LzfseWriter<W> {
    fn write(&mut self, mut buf: &[u8]) -> io::Result<usize> {
        let total = buf.len();
        while self.buffer.len() + buf.len() >= LZFSE_BLOCK_CHUNK_SIZE {
            let space = LZFSE_BLOCK_CHUNK_SIZE - self.buffer.len();
            self.buffer.extend_from_slice(&buf[..space]);
            buf = &buf[space..];
            self.flush_chunk(false)?;
        }
        if !buf.is_empty() {
            self.buffer.extend_from_slice(buf);
        }
        Ok(total)
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.buffer.is_empty() {
            self.flush_chunk(false)?;
        }
        self.sink.flush()
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

    #[test]
    fn test_lzfse_bounded_reader_writer_multi_chunk_roundtrip() {
        let total_len = LZFSE_BLOCK_CHUNK_SIZE * 3 + 19_457;
        let mut payload = Vec::with_capacity(total_len);
        for i in 0..total_len {
            payload.push(((i * 73 + (i >> 5)) & 0xFF) as u8);
        }

        let mut streamed_comp = Vec::new();
        {
            let mut writer = LzfseWriter::new(&mut streamed_comp);
            for chunk in payload.chunks(31_111) {
                writer.write_all(chunk).expect("lzfse writer chunk");
            }
            writer.finish().expect("lzfse writer finish");
        }

        let single_shot_decomp = lzfse_decompress_stream(&streamed_comp)
            .expect("single-shot decompress of LzfseWriter output");
        assert_eq!(single_shot_decomp, payload);

        let mut reader = LzfseReader::new(std::io::Cursor::new(&streamed_comp))
            .expect("lzfse reader init");
        let mut streamed_decomp = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            let n = reader.read(&mut buf).expect("lzfse reader chunk");
            if n == 0 {
                break;
            }
            streamed_decomp.extend_from_slice(&buf[..n]);
        }
        assert_eq!(streamed_decomp, payload);
    }
}
