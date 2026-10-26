// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Safe, high-performance canonical DEFLATE (RFC 1951), Zlib (RFC 1950), and Gzip (RFC 1952)
//! compression and decompression engine backed by native C/SIMD `libdeflate`.

pub mod checksum;
pub mod compressor;
pub mod container;
pub mod decompressor;
pub(crate) mod ffi;
mod pool;
pub mod reader;
pub mod writer;

pub use checksum::*;
pub use compressor::{DeflateCompressor, DeflateStrategy};
pub use container::ContainerFormat;
pub use decompressor::{DeflateDecompressError, DeflateDecompressor};
pub use ffi::LibdeflateResult;
pub use pool::*;
pub use reader::{LibdeflateReader, DEFAULT_DECOMPRESS_BUFFER_SIZE};
pub use writer::{LibdeflateWriter, DEFAULT_COMPRESS_CHUNK_SIZE};

use crate::types::TTZipStatus;

/// RFC 1951 Deflate sliding window size in bytes (32 KB = 32768 bytes).
pub const DEFLATE_WINDOW_SIZE_BYTES: usize = 32 * 1024;

/// Alias for RFC 1951 Deflate sliding window size.
pub const DEFLATE_SLIDING_WINDOW_SIZE: usize = DEFLATE_WINDOW_SIZE_BYTES;

// MARK: - High-Level Zero-Copy Helpers

/// Zero-copy raw DEFLATE compression using thread-local pooled compressor.
pub fn deflate_compress(src: &[u8], dst: &mut [u8], level: i32) -> Result<usize, TTZipStatus> {
    with_thread_local_compressor(level, |c| c.compress(src, dst))
}

/// Zero-copy raw DEFLATE decompression using thread-local pooled decompressor.
pub fn deflate_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    with_thread_local_decompressor(|d| d.decompress(src, dst))
}

/// Zero-copy zlib compression using thread-local pooled compressor.
pub fn zlib_compress(src: &[u8], dst: &mut [u8], level: i32) -> Result<usize, TTZipStatus> {
    with_thread_local_compressor(level, |c| c.zlib_compress(src, dst))
}

/// Zero-copy zlib decompression using thread-local pooled decompressor.
pub fn zlib_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    with_thread_local_decompressor(|d| d.zlib_decompress(src, dst))
}

/// Zero-copy gzip compression using thread-local pooled compressor.
pub fn gzip_compress(src: &[u8], dst: &mut [u8], level: i32) -> Result<usize, TTZipStatus> {
    with_thread_local_compressor(level, |c| c.gzip_compress(src, dst))
}

/// Zero-copy gzip decompression using thread-local pooled decompressor.
pub fn gzip_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    with_thread_local_decompressor(|d| d.gzip_decompress(src, dst))
}

/// Zero-allocation, constant-time upper bound calculation for raw DEFLATE compression.
#[inline]
pub fn deflate_compress_bound(in_len: usize, level: i32) -> usize {
    if level == 0 {
        if in_len == 0 {
            5
        } else {
            in_len + in_len.div_ceil(65535) * 5
        }
    } else {
        unsafe { ffi::libdeflate_deflate_compress_bound(std::ptr::null_mut(), in_len) }
    }
}

/// Zero-allocation, constant-time upper bound calculation for zlib compression.
#[inline]
pub fn zlib_compress_bound(in_len: usize, level: i32) -> usize {
    if level == 0 {
        deflate_compress_bound(in_len, 0) + 6
    } else {
        unsafe { ffi::libdeflate_zlib_compress_bound(std::ptr::null_mut(), in_len) }
    }
}

/// Zero-allocation, constant-time upper bound calculation for gzip compression.
#[inline]
pub fn gzip_compress_bound(in_len: usize, level: i32) -> usize {
    if level == 0 {
        deflate_compress_bound(in_len, 0) + 18
    } else {
        unsafe { ffi::libdeflate_gzip_compress_bound(std::ptr::null_mut(), in_len) }
    }
}

// MARK: - Facade APIs returning Vec<u8>

/// Compresses `src` with raw DEFLATE and returns a newly allocated `Vec<u8>`.
pub fn libdeflate_deflate_compress(src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
    let bound = deflate_compress_bound(src.len(), level);
    let mut out = vec![0u8; bound];
    let written = deflate_compress(src, &mut out, level)?;
    out.truncate(written);
    Ok(out)
}

/// Raw DEFLATE decompression into a pre-allocated buffer.
pub fn libdeflate_deflate_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    deflate_decompress(src, dst)
}

/// Compresses `src` with zlib and returns a newly allocated `Vec<u8>`.
pub fn libdeflate_zlib_compress(src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
    let bound = zlib_compress_bound(src.len(), level);
    let mut out = vec![0u8; bound];
    let written = zlib_compress(src, &mut out, level)?;
    out.truncate(written);
    Ok(out)
}

/// Zlib decompression into a pre-allocated buffer.
pub fn libdeflate_zlib_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    zlib_decompress(src, dst)
}

/// Compresses `src` with gzip and returns a newly allocated `Vec<u8>`.
pub fn libdeflate_gzip_compress(src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
    let bound = gzip_compress_bound(src.len(), level);
    let mut out = vec![0u8; bound];
    let written = gzip_compress(src, &mut out, level)?;
    out.truncate(written);
    Ok(out)
}

/// Gzip decompression into a pre-allocated buffer.
pub fn libdeflate_gzip_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    gzip_decompress(src, dst)
}

/// Validates whether `src` is a valid DEFLATE / Zlib / Gzip container stream.
pub fn libdeflate_validate(src: &[u8], format: ContainerFormat) -> Result<bool, TTZipStatus> {
    if src.is_empty() {
        return Ok(false);
    }
    match format {
        ContainerFormat::Raw => {
            let mut decompressor = DeflateDecompressor::new()?;
            let mut test_dst = [0u8; 4096];
            match decompressor.decompress_precise(src, &mut test_dst) {
                Ok(_) | Err(DeflateDecompressError::InsufficientSpace) => Ok(true),
                Err(DeflateDecompressError::BadData) | Err(DeflateDecompressError::ShortOutput) => Ok(false),
            }
        }
        ContainerFormat::Zlib => {
            if src.len() < 6 {
                return Ok(false);
            }
            let cmf = src[0] as u16;
            let flg = src[1] as u16;
            if !(cmf * 256 + flg).is_multiple_of(31) {
                return Ok(false);
            }
            if (src[0] & 0x0F) != 8 || (src[0] >> 4) > 7 {
                return Ok(false);
            }
            let mut decompressor = DeflateDecompressor::new()?;
            let mut test_dst = [0u8; 4096];
            match decompressor.zlib_decompress_precise(src, &mut test_dst) {
                Ok(_) | Err(DeflateDecompressError::InsufficientSpace) => Ok(true),
                Err(DeflateDecompressError::BadData) | Err(DeflateDecompressError::ShortOutput) => Ok(false),
            }
        }
        ContainerFormat::Gzip => {
            if src.len() < 18 {
                return Ok(false);
            }
            if src[0] != 0x1F || src[1] != 0x8B || src[2] != 8 {
                return Ok(false);
            }
            let mut decompressor = DeflateDecompressor::new()?;
            let mut test_dst = [0u8; 4096];
            match decompressor.gzip_decompress_precise(src, &mut test_dst) {
                Ok(_) | Err(DeflateDecompressError::InsufficientSpace) => Ok(true),
                Err(DeflateDecompressError::BadData) | Err(DeflateDecompressError::ShortOutput) => Ok(false),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deflate_roundtrip_basic() {
        let input = b"Hello libdeflate canonical C/SIMD compression in TTZip Monorepo 2026!";
        let mut comp = vec![0u8; deflate_compress_bound(input.len(), 6)];
        let c_len = deflate_compress(input, &mut comp, 6).expect("deflate compress");
        assert!(c_len > 0);

        let mut decomp = vec![0u8; input.len()];
        let d_len = deflate_decompress(&comp[..c_len], &mut decomp).expect("deflate decompress");
        assert_eq!(d_len, input.len());
        assert_eq!(&decomp, input);
    }

    #[test]
    fn test_zlib_roundtrip() {
        let input = b"Zlib RFC 1950 container test with C libdeflate and Adler-32 verification.";
        let mut comp = vec![0u8; zlib_compress_bound(input.len(), 6)];
        let c_len = zlib_compress(input, &mut comp, 6).expect("zlib compress");
        assert!(c_len > 0);

        let mut decomp = vec![0u8; input.len()];
        let d_len = zlib_decompress(&comp[..c_len], &mut decomp).expect("zlib decompress");
        assert_eq!(d_len, input.len());
        assert_eq!(&decomp, input);
    }

    #[test]
    fn test_gzip_roundtrip() {
        let input = b"Gzip RFC 1952 container test with C libdeflate and CRC-32 verification.";
        let mut comp = vec![0u8; gzip_compress_bound(input.len(), 6)];
        let c_len = gzip_compress(input, &mut comp, 6).expect("gzip compress");
        assert!(c_len > 0);

        let mut decomp = vec![0u8; input.len()];
        let d_len = gzip_decompress(&comp[..c_len], &mut decomp).expect("gzip decompress");
        assert_eq!(d_len, input.len());
        assert_eq!(&decomp, input);
    }
}
