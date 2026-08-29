// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Ultra-fast `LZ4` block compression and High Compression (`LZ4 HC`) codecs.
//!
//! Wraps native C `liblz4` with safe Rust interfaces, zero-allocation buffer APIs,
//! supporting acceleration factors 1..=100 and HC levels 1..=12.

use crate::types::TTZipStatus;

// MARK: - Native C LZ4 Bindings

#[allow(dead_code)]
extern "C" {

    fn LZ4_compress_default(
        src: *const libc::c_char,
        dst: *mut libc::c_char,
        src_size: libc::c_int,
        dst_capacity: libc::c_int,
    ) -> libc::c_int;

    fn LZ4_compress_fast(
        src: *const libc::c_char,
        dst: *mut libc::c_char,
        src_size: libc::c_int,
        dst_capacity: libc::c_int,
        acceleration: libc::c_int,
    ) -> libc::c_int;

    fn LZ4_compress_HC(
        src: *const libc::c_char,
        dst: *mut libc::c_char,
        src_size: libc::c_int,
        dst_capacity: libc::c_int,
        compression_level: libc::c_int,
    ) -> libc::c_int;

    fn LZ4_decompress_safe(
        src: *const libc::c_char,
        dst: *mut libc::c_char,
        compressed_size: libc::c_int,
        dst_capacity: libc::c_int,
    ) -> libc::c_int;

    fn LZ4_compressBound(input_size: libc::c_int) -> libc::c_int;
}

// MARK: - Safe Buffer Bound Calculation

/// Computes worst-case output buffer size for LZ4 block compression.
#[inline]
pub fn lz4_compress_bound(src_size: usize) -> usize {
    if src_size > i32::MAX as usize {
        0
    } else {
        unsafe { LZ4_compressBound(src_size as libc::c_int) as usize }
    }
}

// MARK: - Compression Functions

/// Compresses a memory block using LZ4 default acceleration (acceleration = 1).
#[inline]
pub fn lz4_compress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    lz4_compress_fast(src, dst, 1)
}

/// Compresses a memory block using LZ4 with custom acceleration factor (1..=100).
///
/// Higher acceleration values increase compression speed at a slight cost to compression ratio.
pub fn lz4_compress_fast(
    src: &[u8],
    dst: &mut [u8],
    acceleration: i32,
) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    if src.len() > i32::MAX as usize || dst.len() > i32::MAX as usize {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let accel = acceleration.clamp(1, 100) as libc::c_int;
    let written = unsafe {
        LZ4_compress_fast(
            src.as_ptr() as *const libc::c_char,
            dst.as_mut_ptr() as *mut libc::c_char,
            src.len() as libc::c_int,
            dst.len() as libc::c_int,
            accel,
        )
    };

    if written <= 0 {
        Err(TTZipStatus::ErrCompressionFailed)
    } else {
        Ok(written as usize)
    }
}

/// Compresses a memory block using LZ4 High Compression (HC) algorithm.
///
/// `level` must be between 1 and 12 (default recommended: 9).
pub fn lz4_compress_hc(src: &[u8], dst: &mut [u8], level: i32) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    if src.len() > i32::MAX as usize || dst.len() > i32::MAX as usize {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let clevel = level.clamp(1, 12) as libc::c_int;
    let written = unsafe {
        LZ4_compress_HC(
            src.as_ptr() as *const libc::c_char,
            dst.as_mut_ptr() as *mut libc::c_char,
            src.len() as libc::c_int,
            dst.len() as libc::c_int,
            clevel,
        )
    };

    if written <= 0 {
        Err(TTZipStatus::ErrCompressionFailed)
    } else {
        Ok(written as usize)
    }
}

// MARK: - Decompression Functions

/// Decompresses an LZ4 compressed block into a pre-allocated destination buffer.
pub fn lz4_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    if src.len() > i32::MAX as usize || dst.len() > i32::MAX as usize {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let written = unsafe {
        LZ4_decompress_safe(
            src.as_ptr() as *const libc::c_char,
            dst.as_mut_ptr() as *mut libc::c_char,
            src.len() as libc::c_int,
            dst.len() as libc::c_int,
        )
    };

    if written < 0 {
        Err(TTZipStatus::ErrCorruptHeader)
    } else {
        Ok(written as usize)
    }
}

// MARK: - Vector Helpers

/// Compresses a memory slice into a newly allocated `Vec<u8>` using LZ4 Fast acceleration.
pub fn lz4_compress_to_vec(src: &[u8], acceleration: i32) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() {
        return Ok(Vec::new());
    }
    let bound = lz4_compress_bound(src.len());
    let mut out = vec![0u8; bound];
    let written = lz4_compress_fast(src, &mut out, acceleration)?;
    out.truncate(written);
    Ok(out)
}

/// Compresses a memory slice into a newly allocated `Vec<u8>` using LZ4 HC.
pub fn lz4_compress_hc_to_vec(src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() {
        return Ok(Vec::new());
    }
    let bound = lz4_compress_bound(src.len());
    let mut out = vec![0u8; bound];
    let written = lz4_compress_hc(src, &mut out, level)?;
    out.truncate(written);
    Ok(out)
}

/// Decompresses an LZ4 compressed slice into a newly allocated `Vec<u8>`.
pub fn lz4_decompress_to_vec(
    src: &[u8],
    uncompressed_len: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() || uncompressed_len == 0 {
        return Ok(Vec::new());
    }
    let mut out = vec![0u8; uncompressed_len];
    let written = lz4_decompress(src, &mut out)?;
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
    fn test_lz4_default_roundtrip() {
        let input = b"LZ4 fast compression block testing in TTZip native microkernel 2026.";
        let mut comp = vec![0u8; lz4_compress_bound(input.len())];
        let c_len = lz4_compress(input, &mut comp).expect("lz4 compress");
        assert!(c_len > 0);

        let mut decomp = vec![0u8; input.len()];
        let d_len = lz4_decompress(&comp[..c_len], &mut decomp).expect("lz4 decompress");
        assert_eq!(d_len, input.len());
        assert_eq!(&decomp[..d_len], input);
    }

    #[test]
    fn test_lz4_fast_acceleration_range() {
        let input = b"Repeated pattern for LZ4 acceleration tests: ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let mut repeated = Vec::new();
        for _ in 0..100 {
            repeated.extend_from_slice(input);
        }

        for &accel in &[1, 3, 10, 50, 100] {
            let mut comp = vec![0u8; lz4_compress_bound(repeated.len())];
            let c_len = lz4_compress_fast(&repeated, &mut comp, accel).expect("lz4 compress fast");
            assert!(c_len > 0);

            let mut decomp = vec![0u8; repeated.len()];
            let d_len = lz4_decompress(&comp[..c_len], &mut decomp).expect("lz4 decompress");
            assert_eq!(d_len, repeated.len());
            assert_eq!(&decomp[..d_len], repeated.as_slice());
        }
    }

    #[test]
    fn test_lz4_hc_all_levels() {
        let input = b"High Compression LZ4 HC verification payload with redundant text structures. Compresses tightly across levels 1 through 12.";

        let mut payload = Vec::new();
        for _ in 0..50 {
            payload.extend_from_slice(input);
        }

        for level in [1, 3, 6, 9, 12] {
            let mut comp = vec![0u8; lz4_compress_bound(payload.len())];
            let c_len = lz4_compress_hc(&payload, &mut comp, level).expect("lz4 hc compress");
            assert!(c_len > 0);
            assert!(c_len < payload.len());

            let mut decomp = vec![0u8; payload.len()];
            let d_len = lz4_decompress(&comp[..c_len], &mut decomp).expect("lz4 decompress");
            assert_eq!(d_len, payload.len());
            assert_eq!(&decomp[..d_len], payload.as_slice());
        }
    }

    #[test]
    fn test_lz4_empty_buffer() {
        let empty = b"";
        let mut dst = [0u8; 64];
        let c_len = lz4_compress(empty, &mut dst).expect("empty compress");
        assert_eq!(c_len, 0);

        let d_len = lz4_decompress(empty, &mut dst).expect("empty decompress");
        assert_eq!(d_len, 0);
    }

    #[test]
    fn test_lz4_corrupt_data_rejected() {
        let corrupt = [0xFF, 0xFF, 0xFF, 0xFF, 0x01, 0x02];
        let mut dst = [0u8; 128];
        let res = lz4_decompress(&corrupt, &mut dst);
        assert!(res.is_err());
    }

    #[test]
    fn test_lz4_to_vec_helpers() {
        let input = b"Helper function verification for Vec allocation pipelines.";
        let comp = lz4_compress_to_vec(input, 1).expect("compress to vec");
        assert!(!comp.is_empty());

        let decomp = lz4_decompress_to_vec(&comp, input.len()).expect("decompress to vec");
        assert_eq!(decomp.as_slice(), input);

        let hc_comp = lz4_compress_hc_to_vec(input, 9).expect("hc compress to vec");
        assert!(!hc_comp.is_empty());

        let hc_decomp = lz4_decompress_to_vec(&hc_comp, input.len()).expect("hc decompress to vec");
        assert_eq!(hc_decomp.as_slice(), input);
    }
}
