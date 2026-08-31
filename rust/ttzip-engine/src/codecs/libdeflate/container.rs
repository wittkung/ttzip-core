// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, high-performance RFC 1950 (Zlib) and RFC 1952 (Gzip) container state machines.
//!
//! Provides zero-copy, memory-safe compression and decompression wrappers around DEFLATE payload:
//! - **Zlib (RFC 1950)**:
//!   - 2-byte CMF/FLG header with $(CMF \times 256 + FLG) \bmod 31 == 0$ alignment validation.
//!   - Dynamic level hint encoding (`FASTEST`, `FAST`, `DEFAULT`, `SLOWEST`).
//!   - 4-byte big-endian Adler-32 checksum footer computation and verification.
//! - **Gzip (RFC 1952)**:
//!   - 10-byte fixed header (`ID1=0x1F, ID2=0x8B, CM=8, MTIME=0, OS=255`).
//!   - Variable-length header extension support: `FEXTRA`, `FNAME`, `FCOMMENT`, `FHCRC`.
//!   - 8-byte little-endian CRC-32 and uncompressed size (ISIZE $\bmod 2^{32}$) validation.
//! - **Unified Framing Interface**:
//!   - [`ContainerFormat`]: Strongly-typed container selector (`Raw`, `Zlib`, `Gzip`).

use crate::codecs::deflate::{deflate_compress, deflate_compress_bound, deflate_decompress};
use crate::codecs::libdeflate::checksum::{adler32_compute, crc32_compute};
use crate::types::TTZipStatus;

// ============================================================================
// 1. Zlib Constants & Header Specifications (RFC 1950)
// ============================================================================

/// Minimum size of a zlib header in bytes (CMF + FLG).
pub const ZLIB_MIN_HEADER_SIZE: usize = 2;

/// Size of a zlib footer in bytes (4-byte big-endian Adler-32).
pub const ZLIB_FOOTER_SIZE: usize = 4;

/// Minimum total overhead for a zlib stream (header + footer = 6 bytes).
pub const ZLIB_MIN_OVERHEAD: usize = ZLIB_MIN_HEADER_SIZE + ZLIB_FOOTER_SIZE;

/// Zlib compression method for DEFLATE (CM = 8).
pub const ZLIB_CM_DEFLATE: u8 = 8;

/// Zlib window size logarithmic indicator for 32KB window (CINFO = 7).
pub const ZLIB_CINFO_32K_WINDOW: u8 = 7;

/// Zlib compression level hint: fastest algorithm.
pub const ZLIB_FASTEST_COMPRESSION: u8 = 0;

/// Zlib compression level hint: fast algorithm.
pub const ZLIB_FAST_COMPRESSION: u8 = 1;

/// Zlib compression level hint: default algorithm.
pub const ZLIB_DEFAULT_COMPRESSION: u8 = 2;

/// Zlib compression level hint: slowest/maximum algorithm.
pub const ZLIB_SLOWEST_COMPRESSION: u8 = 3;

// ============================================================================
// 2. Gzip Constants & Header Specifications (RFC 1952)
// ============================================================================

/// Minimum size of a gzip header in bytes (10 fixed bytes).
pub const GZIP_MIN_HEADER_SIZE: usize = 10;

/// Size of a gzip footer in bytes (4-byte CRC-32 + 4-byte ISIZE = 8 bytes).
pub const GZIP_FOOTER_SIZE: usize = 8;

/// Minimum total overhead for a gzip stream (10-byte header + 8-byte footer = 18 bytes).
pub const GZIP_MIN_OVERHEAD: usize = GZIP_MIN_HEADER_SIZE + GZIP_FOOTER_SIZE;

/// Gzip magic ID byte 1.
pub const GZIP_ID1: u8 = 0x1F;

/// Gzip magic ID byte 2.
pub const GZIP_ID2: u8 = 0x8B;

/// Gzip compression method for DEFLATE (CM = 8).
pub const GZIP_CM_DEFLATE: u8 = 8;

/// Gzip flag bit 0: text hint flag (informational).
pub const GZIP_FTEXT: u8 = 0x01;

/// Gzip flag bit 1: 16-bit header CRC present.
pub const GZIP_FHCRC: u8 = 0x02;

/// Gzip flag bit 2: variable-length extra fields present.
pub const GZIP_FEXTRA: u8 = 0x04;

/// Gzip flag bit 3: zero-terminated original filename present.
pub const GZIP_FNAME: u8 = 0x08;

/// Gzip flag bit 4: zero-terminated file comment present.
pub const GZIP_FCOMMENT: u8 = 0x10;

/// Gzip reserved flag bits (must be zero in valid RFC 1952 stream).
pub const GZIP_FRESERVED: u8 = 0xE0;

/// Gzip timestamp unavailable constant.
pub const GZIP_MTIME_UNAVAILABLE: u32 = 0;

/// Gzip extra flag for maximum/slowest compression level.
pub const GZIP_XFL_SLOWEST_COMPRESSION: u8 = 0x02;

/// Gzip extra flag for fastest compression level.
pub const GZIP_XFL_FASTEST_COMPRESSION: u8 = 0x04;

/// Gzip operating system: unknown / default.
pub const GZIP_OS_UNKNOWN: u8 = 255;

// ============================================================================
// 3. Container Format Enumeration
// ============================================================================

/// Supported container framing formats for RFC 1951 Deflate compression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ContainerFormat {
    /// Pure raw RFC 1951 DEFLATE byte stream (no container headers or footers).
    #[default]
    Raw,
    /// RFC 1950 zlib container (2-byte CMF/FLG header, 4-byte big-endian Adler-32 footer).
    Zlib,
    /// RFC 1952 gzip container (10-byte header, optional extensions, 8-byte little-endian CRC32/ISIZE footer).
    Gzip,
}

impl ContainerFormat {
    /// Pure raw RFC 1951 DEFLATE byte stream (alias for Raw).
    #[allow(non_upper_case_globals)]
    pub const Deflate: ContainerFormat = ContainerFormat::Raw;
}

// ============================================================================
// 4. Bound Calculations
// ============================================================================

/// Computes the maximum upper bound in bytes required to store zlib-compressed data.
#[inline]
pub fn zlib_compress_bound(in_len: usize, level: i32) -> usize {
    ZLIB_MIN_OVERHEAD + deflate_compress_bound(in_len, level)
}

/// Computes the maximum upper bound in bytes required to store gzip-compressed data.
#[inline]
pub fn gzip_compress_bound(in_len: usize, level: i32) -> usize {
    GZIP_MIN_OVERHEAD + deflate_compress_bound(in_len, level)
}

// ============================================================================
// 5. Zlib Container Compression & Decompression (RFC 1950)
// ============================================================================

/// Compresses source slice using the RFC 1950 Zlib container format into a new vector.
pub fn zlib_compress(src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
    let bound = zlib_compress_bound(src.len(), level);
    let mut dst = vec![0u8; bound];
    let written = zlib_compress_to_slice(src, &mut dst, level)?;
    dst.truncate(written);
    Ok(dst)
}

/// Zero-copy in-place compression of source data into destination buffer using RFC 1950 Zlib format.
pub fn zlib_compress_to_slice(
    src: &[u8],
    dst: &mut [u8],
    level: i32,
) -> Result<usize, TTZipStatus> {
    if dst.len() < ZLIB_MIN_OVERHEAD {
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    // Determine level hint for FLG byte
    let level_hint = if level < 2 {
        ZLIB_FASTEST_COMPRESSION
    } else if level < 6 {
        ZLIB_FAST_COMPRESSION
    } else if level < 8 {
        ZLIB_DEFAULT_COMPRESSION
    } else {
        ZLIB_SLOWEST_COMPRESSION
    };

    // 2-byte header: CMF (0x78) and FLG
    let cmf = (ZLIB_CINFO_32K_WINDOW << 4) | ZLIB_CM_DEFLATE;
    let mut hdr = ((cmf as u16) << 8) | ((level_hint as u16) << 6);
    let remainder = hdr % 31;
    if remainder != 0 {
        hdr += 31 - remainder;
    }

    dst[0..2].copy_from_slice(&hdr.to_be_bytes());

    // Compress raw DEFLATE payload
    let dst_len = dst.len();
    let payload_dst = &mut dst[ZLIB_MIN_HEADER_SIZE..dst_len - ZLIB_FOOTER_SIZE];
    let deflate_size = deflate_compress(src, payload_dst, level)?;

    // Adler-32 checksum written in big-endian
    let adler = adler32_compute(src);
    let footer_offset = ZLIB_MIN_HEADER_SIZE + deflate_size;
    dst[footer_offset..footer_offset + ZLIB_FOOTER_SIZE].copy_from_slice(&adler.to_be_bytes());

    Ok(footer_offset + ZLIB_FOOTER_SIZE)
}

/// Decompresses RFC 1950 Zlib container stream into pre-allocated destination buffer.
pub fn zlib_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.len() < ZLIB_MIN_OVERHEAD {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    // 2-byte header: CMF and FLG
    let hdr = u16::from_be_bytes([src[0], src[1]]);

    // FCHECK validation: (CMF * 256 + FLG) % 31 == 0
    if !hdr.is_multiple_of(31) {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    // Compression method must be DEFLATE (CM = 8)
    let cm = (hdr >> 8) & 0x0F;
    if cm as u8 != ZLIB_CM_DEFLATE {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    // Window size must not exceed 32KB (CINFO <= 7)
    let cinfo = (hdr >> 12) & 0x0F;
    if (cinfo as u8) > ZLIB_CINFO_32K_WINDOW {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    // Preset dictionary (FDICT) is not supported
    if ((hdr >> 5) & 1) != 0 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    // Extract and decompress raw DEFLATE payload
    let payload = &src[ZLIB_MIN_HEADER_SIZE..src.len() - ZLIB_FOOTER_SIZE];
    let decompressed_size = deflate_decompress(payload, dst)?;

    // Validate big-endian Adler-32 checksum
    let footer = &src[src.len() - ZLIB_FOOTER_SIZE..];
    let expected_adler = u32::from_be_bytes([footer[0], footer[1], footer[2], footer[3]]);
    let actual_adler = adler32_compute(&dst[..decompressed_size]);

    if actual_adler != expected_adler {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    Ok(decompressed_size)
}

// ============================================================================
// 6. Gzip Container Compression & Decompression (RFC 1952)
// ============================================================================

/// Compresses source slice using the RFC 1952 Gzip container format into a new vector.
pub fn gzip_compress(src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
    let bound = gzip_compress_bound(src.len(), level);
    let mut dst = vec![0u8; bound];
    let written = gzip_compress_to_slice(src, &mut dst, level)?;
    dst.truncate(written);
    Ok(dst)
}

/// Zero-copy in-place compression of source data into destination buffer using RFC 1952 Gzip format.
pub fn gzip_compress_to_slice(
    src: &[u8],
    dst: &mut [u8],
    level: i32,
) -> Result<usize, TTZipStatus> {
    if dst.len() < GZIP_MIN_OVERHEAD {
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    // 10-byte fixed header: ID1, ID2, CM, FLG, MTIME, XFL, OS
    dst[0] = GZIP_ID1;
    dst[1] = GZIP_ID2;
    dst[2] = GZIP_CM_DEFLATE;
    dst[3] = 0; // FLG = 0 (no extra headers)
    dst[4..8].copy_from_slice(&GZIP_MTIME_UNAVAILABLE.to_le_bytes());

    let xfl = if level < 2 {
        GZIP_XFL_FASTEST_COMPRESSION
    } else if level >= 8 {
        GZIP_XFL_SLOWEST_COMPRESSION
    } else {
        0
    };
    dst[8] = xfl;
    dst[9] = GZIP_OS_UNKNOWN;

    // Compress raw DEFLATE payload
    let dst_len = dst.len();
    let payload_dst = &mut dst[GZIP_MIN_HEADER_SIZE..dst_len - GZIP_FOOTER_SIZE];
    let deflate_size = deflate_compress(src, payload_dst, level)?;

    // CRC-32 (4-byte little-endian)
    let crc = crc32_compute(src);
    let footer_offset = GZIP_MIN_HEADER_SIZE + deflate_size;
    dst[footer_offset..footer_offset + 4].copy_from_slice(&crc.to_le_bytes());

    // ISIZE (4-byte little-endian, modulo 2^32)
    let isize = src.len() as u32;
    dst[footer_offset + 4..footer_offset + 8].copy_from_slice(&isize.to_le_bytes());

    Ok(footer_offset + GZIP_FOOTER_SIZE)
}

/// Decompresses RFC 1952 Gzip container stream into pre-allocated destination buffer.
pub fn gzip_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.len() < GZIP_MIN_OVERHEAD {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    // Validate 10-byte header: ID1, ID2, CM
    if src[0] != GZIP_ID1 || src[1] != GZIP_ID2 || src[2] != GZIP_CM_DEFLATE {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let flg = src[3];
    if (flg & GZIP_FRESERVED) != 0 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let mut pos = GZIP_MIN_HEADER_SIZE;

    // FEXTRA: Extra field
    if (flg & GZIP_FEXTRA) != 0 {
        if pos + 2 + GZIP_FOOTER_SIZE > src.len() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let xlen = u16::from_le_bytes([src[pos], src[pos + 1]]) as usize;
        pos += 2;
        if pos + xlen + GZIP_FOOTER_SIZE > src.len() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        pos += xlen;
    }

    // FNAME: Original zero-terminated file name
    if (flg & GZIP_FNAME) != 0 {
        let max_search = src.len().saturating_sub(GZIP_FOOTER_SIZE);
        let mut found = false;
        while pos < max_search {
            let b = src[pos];
            pos += 1;
            if b == 0 {
                found = true;
                break;
            }
        }
        if !found {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
    }

    // FCOMMENT: Zero-terminated file comment
    if (flg & GZIP_FCOMMENT) != 0 {
        let max_search = src.len().saturating_sub(GZIP_FOOTER_SIZE);
        let mut found = false;
        while pos < max_search {
            let b = src[pos];
            pos += 1;
            if b == 0 {
                found = true;
                break;
            }
        }
        if !found {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
    }

    // FHCRC: Header CRC16 checksum
    if (flg & GZIP_FHCRC) != 0 {
        if pos + 2 + GZIP_FOOTER_SIZE > src.len() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let header_crc16 = u16::from_le_bytes([src[pos], src[pos + 1]]);
        let computed_crc = (crc32_compute(&src[..pos]) & 0xFFFF) as u16;
        if header_crc16 != computed_crc {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        pos += 2;
    }

    // Extract and decompress raw DEFLATE payload
    let payload = &src[pos..src.len() - GZIP_FOOTER_SIZE];
    let decompressed_size = deflate_decompress(payload, dst)?;

    // CRC-32 & ISIZE verification
    let footer = &src[src.len() - GZIP_FOOTER_SIZE..];
    let expected_crc = u32::from_le_bytes([footer[0], footer[1], footer[2], footer[3]]);
    let expected_isize = u32::from_le_bytes([footer[4], footer[5], footer[6], footer[7]]);

    let actual_crc = crc32_compute(&dst[..decompressed_size]);
    if actual_crc != expected_crc {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    if (decompressed_size as u32) != expected_isize {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    Ok(decompressed_size)
}

// ============================================================================
// 7. Generic Container Dispatcher
// ============================================================================

/// Generic container compression helper dispatching to Raw, Zlib, or Gzip formatting.
pub fn compress_container(
    src: &[u8],
    format: ContainerFormat,
    level: i32,
) -> Result<Vec<u8>, TTZipStatus> {
    match format {
        ContainerFormat::Raw => {
            let bound = deflate_compress_bound(src.len(), level);
            let mut dst = vec![0u8; bound];
            let written = deflate_compress(src, &mut dst, level)?;
            dst.truncate(written);
            Ok(dst)
        }
        ContainerFormat::Zlib => zlib_compress(src, level),
        ContainerFormat::Gzip => gzip_compress(src, level),
    }
}

/// Generic container decompression helper dispatching to Raw, Zlib, or Gzip formatting.
pub fn decompress_container(
    src: &[u8],
    dst: &mut [u8],
    format: ContainerFormat,
) -> Result<usize, TTZipStatus> {
    match format {
        ContainerFormat::Raw => deflate_decompress(src, dst),
        ContainerFormat::Zlib => zlib_decompress(src, dst),
        ContainerFormat::Gzip => gzip_decompress(src, dst),
    }
}
