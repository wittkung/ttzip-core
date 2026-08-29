// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Apple `LZFSE` and `LZVN` hardware/native block codecs.
//!
//! Provides thread-private 2MB scratch buffer pooling for zero-allocation LZFSE operations
//! and ultra-high-speed LZVN block encoding/decoding.

use crate::types::TTZipStatus;
use std::cell::RefCell;

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

    fn lzvn_encode_buffer(
        dst_buffer: *mut libc::c_void,
        dst_size: libc::size_t,
        src_buffer: *const libc::c_void,
        src_size: libc::size_t,
        work_buffer: *mut libc::c_void,
    ) -> libc::size_t;
}

#[repr(C)]
struct LzvnDecoderState {
    src: *const u8,
    src_end: *const u8,
    dst: *mut u8,
    dst_begin: *mut u8,
    dst_end: *mut u8,
    dst_current: *mut u8,
    l: libc::size_t,
    m: libc::size_t,
    d: libc::size_t,
    d_prev: libc::size_t,
    end_of_stream: libc::c_int,
}

extern "C" {
    fn lzvn_decode(state: *mut LzvnDecoderState);
}

// 14-bit hash table (16384 entries * 32 bytes = 512KB)
const LZVN_WORK_BUFFER_SIZE: usize = 512 * 1024;
const LZFSE_SCRATCH_MIN_CAPACITY: usize = 2 * 1024 * 1024; // 2MB Scratch buffer

thread_local! {
    static LZFSE_SCRATCH_BUFFER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static LZVN_WORK_BUFFER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
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

fn with_lzvn_work<F, R>(f: F) -> R
where
    F: FnOnce(*mut libc::c_void) -> R,
{
    LZVN_WORK_BUFFER.with(|cell| {
        let mut buf = cell.borrow_mut();
        if buf.len() < LZVN_WORK_BUFFER_SIZE {
            buf.resize(LZVN_WORK_BUFFER_SIZE, 0);
        }
        f(buf.as_mut_ptr() as *mut libc::c_void)
    })
}

// MARK: - Safe Buffer Bound Calculations

/// Computes worst-case output buffer size for LZFSE block compression.
#[inline]
pub fn lzfse_compress_bound(src_size: usize) -> usize {
    src_size.saturating_add(4096)
}

/// Computes worst-case output buffer size for LZVN block compression.
#[inline]
pub fn lzvn_compress_bound(src_size: usize) -> usize {
    src_size.saturating_add(1024)
}

// MARK: - LZFSE Compression & Decompression

/// Compresses a buffer with Apple LZFSE using thread-private 2MB scratch buffer.
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

    if written == 0 && !src.is_empty() {
        Err(TTZipStatus::ErrCompressionFailed)
    } else {
        Ok(written)
    }
}

/// Decompresses an Apple LZFSE buffer using thread-private 2MB scratch buffer.
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

    if written == 0 && !src.is_empty() {
        Err(TTZipStatus::ErrCorruptHeader)
    } else {
        Ok(written)
    }
}

/// Compresses a buffer to newly allocated `Vec<u8>` using Apple LZFSE.
pub fn lzfse_compress_to_vec(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() {
        return Ok(Vec::new());
    }
    let bound = lzfse_compress_bound(src.len());
    let mut out = vec![0u8; bound];
    let written = lzfse_compress(src, &mut out)?;
    out.truncate(written);
    Ok(out)
}

/// Decompresses an Apple LZFSE slice into a newly allocated `Vec<u8>`.
pub fn lzfse_decompress_to_vec(
    src: &[u8],
    uncompressed_len: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() || uncompressed_len == 0 {
        return Ok(Vec::new());
    }
    let mut out = vec![0u8; uncompressed_len];
    let written = lzfse_decompress(src, &mut out)?;
    if written != uncompressed_len {
        return Err(TTZipStatus::ErrExtractionFailed);
    }
    Ok(out)
}

// MARK: - Apple LZVN Compression & Decompression

/// Compresses a buffer with Apple LZVN using thread-private work buffer.
pub fn lzvn_compress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    if dst.len() < 8 {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let written = with_lzvn_work(|work| unsafe {
        lzvn_encode_buffer(
            dst.as_mut_ptr() as *mut libc::c_void,
            dst.len(),
            src.as_ptr() as *const libc::c_void,
            src.len(),
            work,
        )
    });

    if written == 0 && !src.is_empty() {
        Err(TTZipStatus::ErrCompressionFailed)
    } else {
        Ok(written)
    }
}

/// Decompresses an Apple LZVN buffer into destination slice.
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

    let written = (state.dst as usize).saturating_sub(dst.as_ptr() as usize);
    if written == 0 && !src.is_empty() {
        Err(TTZipStatus::ErrCorruptHeader)
    } else {
        Ok(written)
    }
}

/// Compresses a buffer to newly allocated `Vec<u8>` using Apple LZVN.
pub fn lzvn_compress_to_vec(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() {
        return Ok(Vec::new());
    }
    let bound = lzvn_compress_bound(src.len());
    let mut out = vec![0u8; bound];
    let written = lzvn_compress(src, &mut out)?;
    out.truncate(written);
    Ok(out)
}

/// Decompresses an Apple LZVN slice into a newly allocated `Vec<u8>`.
pub fn lzvn_decompress_to_vec(
    src: &[u8],
    uncompressed_len: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() || uncompressed_len == 0 {
        return Ok(Vec::new());
    }
    let mut out = vec![0u8; uncompressed_len];
    let written = lzvn_decompress(src, &mut out)?;
    if written != uncompressed_len {
        return Err(TTZipStatus::ErrExtractionFailed);
    }
    Ok(out)
}

// MARK: - Unit Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lzfse_scratch_roundtrip() {
        let input = b"Apple LZFSE proprietary high-ratio block compression with 2MB scratch buffer.";
        let mut comp = vec![0u8; lzfse_compress_bound(input.len())];
        let c_len = lzfse_compress(input, &mut comp).expect("lzfse compress");
        assert!(c_len > 0);

        let mut decomp = vec![0u8; input.len()];
        let d_len = lzfse_decompress(&comp[..c_len], &mut decomp).expect("lzfse decompress");
        assert_eq!(d_len, input.len());
        assert_eq!(&decomp[..d_len], input);
    }

    #[test]
    fn test_lzfse_to_vec_roundtrip() {
        let input = b"LZFSE vector-based compression pipeline verification in TTZip core engine.";
        let comp = lzfse_compress_to_vec(input).expect("lzfse to vec compress");
        assert!(!comp.is_empty());

        let decomp = lzfse_decompress_to_vec(&comp, input.len()).expect("lzfse to vec decompress");
        assert_eq!(decomp.as_slice(), input);
    }

    #[test]
    fn test_lzvn_roundtrip() {
        let input = b"Apple LZVN ultra-fast decoder hardware-oriented block compression test.";
        let mut comp = vec![0u8; lzvn_compress_bound(input.len())];
        let c_len = lzvn_compress(input, &mut comp).expect("lzvn compress");
        assert!(c_len > 0);

        let mut decomp = vec![0u8; input.len()];
        let d_len = lzvn_decompress(&comp[..c_len], &mut decomp).expect("lzvn decompress");
        assert_eq!(d_len, input.len());
        assert_eq!(&decomp[..d_len], input);
    }

    #[test]
    fn test_lzvn_to_vec_roundtrip() {
        let input = b"Repetitive string test for LZVN to vec: LZVN_LZVN_LZVN_LZVN_LZVN_LZVN_LZVN_2026";
        let comp = lzvn_compress_to_vec(input).expect("lzvn to vec compress");
        assert!(!comp.is_empty());

        let decomp = lzvn_decompress_to_vec(&comp, input.len()).expect("lzvn to vec decompress");
        assert_eq!(decomp.as_slice(), input);
    }

    #[test]
    fn test_lzfse_lzvn_empty_buffer() {
        let empty = b"";
        let mut dst = [0u8; 64];

        let c_lzfse = lzfse_compress(empty, &mut dst).expect("empty lzfse compress");
        assert_eq!(c_lzfse, 0);
        let d_lzfse = lzfse_decompress(empty, &mut dst).expect("empty lzfse decompress");
        assert_eq!(d_lzfse, 0);

        let c_lzvn = lzvn_compress(empty, &mut dst).expect("empty lzvn compress");
        assert_eq!(c_lzvn, 0);
        let d_lzvn = lzvn_decompress(empty, &mut dst).expect("empty lzvn decompress");
        assert_eq!(d_lzvn, 0);
    }
}
