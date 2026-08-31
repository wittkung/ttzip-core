// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Cross-Language Safe Export Layer for Codecs.
//!
//! Provides typed, memory-safe, and Swift 6 Sendable bindings for all 13 compression algorithms:
//! - Raw Deflate (RFC 1951, levels 0..12)
//! - Zlib (RFC 1950, levels 0..12)
//! - Gzip (RFC 1952, levels 0..12)
//! - Zstandard (RFC 8878, levels 1..22)
//! - Zstandard LDM (Long Distance Matching 64MB..2GB)
//! - Zstandard Pre-trained 112KB Dictionary Engine
//! - LZ4 Fast (acceleration 1..100)
//! - LZ4 High Compression (HC, levels 1..12)
//! - Apple LZFSE (Hardware / Native block codec)
//! - Apple LZVN (Ultra-fast hardware block codec)
//! - Google Brotli (RFC 7932, quality 0..11)
//! - Snappy (Raw Block format)
//! - Snappy Framed (Stream format)
//! - Bzip2 (Levels 1..9)
//! - PPMd (PPMd Model H, Orders 2..16, Memory 2MB..256MB)

use super::types::TTZipError;
use crate::types::TTZipStatus;

/// Compression codec identifier exposed to Swift and multi-language SDKs.
#[derive(Copy, Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum UniFFICompressionCodec {
    DeflateRaw,
    Zlib,
    Gzip,
    Zstd,
    ZstdLdm,
    Lz4Fast,
    Lz4Hc,
    Lzfse,
    Lzvn,
    Brotli,
    SnappyRaw,
    SnappyFramed,
    Bzip2,
    Ppmd,
}

/// Compression parameters and options container.
#[derive(Clone, Debug, Default, PartialEq, Eq, uniffi::Record)]
pub struct UniFFICompressionOptions {
    pub level: Option<i32>,
    pub acceleration: Option<i32>,
    pub window_mb: Option<u32>,
    pub ppmd_order: Option<u32>,
    pub ppmd_mem_mb: Option<u32>,
}

fn map_status(st: TTZipStatus) -> TTZipError {
    match st {
        TTZipStatus::ErrFileNotFound => TTZipError::FileNotFound {
            path: "buffer".to_string(),
        },
        TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
        TTZipStatus::ErrCorruptHeader => TTZipError::CorruptHeader {
            details: "Corrupt compressed stream or invalid header".to_string(),
            offset: 0,
        },
        TTZipStatus::ErrSecurityViolation => TTZipError::SecurityViolation {
            reason: "Decompression security boundary violated".to_string(),
        },
        TTZipStatus::Cancelled => TTZipError::Cancelled,
        _ => TTZipError::EngineError { code: st as i32 },
    }
}

// ============================================================================
// Unified Buffer Compression & Decompression Entrypoints
// ============================================================================

/// Compresses a memory buffer using the specified codec and options.
#[uniffi::export]
pub fn uniffi_compress_buffer(
    codec: UniFFICompressionCodec,
    src: Vec<u8>,
    options: Option<UniFFICompressionOptions>,
) -> Result<Vec<u8>, TTZipError> {
    let opts = options.unwrap_or_default();
    match codec {
        UniFFICompressionCodec::DeflateRaw => {
            let level = opts.level.unwrap_or(6);
            uniffi_deflate_compress(src, level)
        }
        UniFFICompressionCodec::Zlib => {
            let level = opts.level.unwrap_or(6);
            uniffi_zlib_compress(src, level)
        }
        UniFFICompressionCodec::Gzip => {
            let level = opts.level.unwrap_or(6);
            uniffi_gzip_compress(src, level)
        }
        UniFFICompressionCodec::Zstd => {
            let level = opts.level.unwrap_or(3);
            uniffi_zstd_compress(src, level)
        }
        UniFFICompressionCodec::ZstdLdm => {
            let level = opts.level.unwrap_or(3);
            let window_mb = opts.window_mb.unwrap_or(64);
            uniffi_zstd_compress_ldm(src, level, window_mb)
        }
        UniFFICompressionCodec::Lz4Fast => {
            let accel = opts.acceleration.unwrap_or(1);
            uniffi_lz4_compress_fast(src, accel)
        }
        UniFFICompressionCodec::Lz4Hc => {
            let level = opts.level.unwrap_or(9);
            uniffi_lz4_compress_hc(src, level)
        }
        UniFFICompressionCodec::Lzfse => uniffi_lzfse_compress(src),
        UniFFICompressionCodec::Lzvn => uniffi_lzvn_compress(src),
        UniFFICompressionCodec::Brotli => {
            let quality = opts.level.unwrap_or(6);
            uniffi_brotli_compress(src, quality, 22)
        }
        UniFFICompressionCodec::SnappyRaw => uniffi_snappy_compress(src),
        UniFFICompressionCodec::SnappyFramed => uniffi_snappy_frame_encode(src),
        UniFFICompressionCodec::Bzip2 => {
            let level = opts.level.unwrap_or(9);
            uniffi_bzip2_compress(src, level)
        }
        UniFFICompressionCodec::Ppmd => {
            let order = opts.ppmd_order.unwrap_or(6);
            let mem_mb = opts.ppmd_mem_mb.unwrap_or(16);
            uniffi_ppmd_compress(src, order, mem_mb)
        }
    }
}

/// Decompresses a memory buffer using the specified codec.
#[uniffi::export]
pub fn uniffi_decompress_buffer(
    codec: UniFFICompressionCodec,
    src: Vec<u8>,
    expected_uncompressed_size: Option<u64>,
    options: Option<UniFFICompressionOptions>,
) -> Result<Vec<u8>, TTZipError> {
    let opts = options.unwrap_or_default();
    match codec {
        UniFFICompressionCodec::DeflateRaw => {
            let exp = expected_uncompressed_size.ok_or(TTZipError::EngineError { code: -1 })?;
            uniffi_deflate_decompress(src, exp)
        }
        UniFFICompressionCodec::Zlib => {
            let exp = expected_uncompressed_size.ok_or(TTZipError::EngineError { code: -1 })?;
            uniffi_zlib_decompress(src, exp)
        }
        UniFFICompressionCodec::Gzip => {
            let exp = expected_uncompressed_size.ok_or(TTZipError::EngineError { code: -1 })?;
            uniffi_gzip_decompress(src, exp)
        }
        UniFFICompressionCodec::Zstd | UniFFICompressionCodec::ZstdLdm => {
            uniffi_zstd_decompress(src, expected_uncompressed_size)
        }
        UniFFICompressionCodec::Lz4Fast | UniFFICompressionCodec::Lz4Hc => {
            let exp = expected_uncompressed_size.ok_or(TTZipError::EngineError { code: -1 })?;
            uniffi_lz4_decompress(src, exp)
        }
        UniFFICompressionCodec::Lzfse => {
            let exp = expected_uncompressed_size.ok_or(TTZipError::EngineError { code: -1 })?;
            uniffi_lzfse_decompress(src, exp)
        }
        UniFFICompressionCodec::Lzvn => {
            let exp = expected_uncompressed_size.ok_or(TTZipError::EngineError { code: -1 })?;
            uniffi_lzvn_decompress(src, exp)
        }
        UniFFICompressionCodec::Brotli => {
            uniffi_brotli_decompress(src, expected_uncompressed_size)
        }
        UniFFICompressionCodec::SnappyRaw => {
            uniffi_snappy_decompress(src)
        }
        UniFFICompressionCodec::SnappyFramed => {
            uniffi_snappy_frame_decode(src)
        }
        UniFFICompressionCodec::Bzip2 => {
            uniffi_bzip2_decompress(src, expected_uncompressed_size)
        }
        UniFFICompressionCodec::Ppmd => {
            let exp = expected_uncompressed_size.ok_or(TTZipError::EngineError { code: -1 })?;
            let order = opts.ppmd_order.unwrap_or(6);
            let mem_mb = opts.ppmd_mem_mb.unwrap_or(16);
            uniffi_ppmd_decompress(src, exp, order, mem_mb)
        }
    }
}

/// Computes upper bound on compressed bytes for a given codec and input size.
#[uniffi::export]
pub fn uniffi_compress_bound(
    codec: UniFFICompressionCodec,
    src_len: u64,
    level: Option<i32>,
) -> u64 {
    let len = src_len as usize;
    let lvl = level.unwrap_or(6);
    let bound = match codec {
        UniFFICompressionCodec::DeflateRaw => crate::codecs::deflate::deflate_compress_bound(len, lvl),
        UniFFICompressionCodec::Zlib => crate::codecs::deflate::zlib_compress_bound(len, lvl),
        UniFFICompressionCodec::Gzip => crate::codecs::deflate::gzip_compress_bound(len, lvl),
        UniFFICompressionCodec::Zstd | UniFFICompressionCodec::ZstdLdm => {
            crate::codecs::zstd::zstd_compress_bound(len)
        }
        UniFFICompressionCodec::Lz4Fast | UniFFICompressionCodec::Lz4Hc => {
            crate::codecs::lz4::lz4_compress_bound(len)
        }
        UniFFICompressionCodec::Lzfse => crate::codecs::lzfse::lzfse_compress_bound(len),
        UniFFICompressionCodec::Lzvn => crate::codecs::lzfse::lzvn_compress_bound(len),
        UniFFICompressionCodec::Brotli => crate::codecs::brotli::brotli_compress_bound(len),
        UniFFICompressionCodec::SnappyRaw => crate::codecs::snappy::snappy_compress_bound(len),
        UniFFICompressionCodec::SnappyFramed => {
            crate::codecs::snappy::snappy_frame_max_encoded_length(len)
        }
        UniFFICompressionCodec::Bzip2 => crate::codecs::bzip2::bzip2_compress_bound(len),
        UniFFICompressionCodec::Ppmd => len.saturating_add(4096),
    };
    bound as u64
}

// ============================================================================
// Deflate / Zlib / Gzip Codec Exports
// ============================================================================

/// Compresses buffer with raw DEFLATE (RFC 1951, levels 0..12).
#[uniffi::export]
pub fn uniffi_deflate_compress(src: Vec<u8>, level: i32) -> Result<Vec<u8>, TTZipError> {
    let bound = crate::codecs::deflate::deflate_compress_bound(src.len(), level);
    let mut dst = vec![0u8; bound];
    let written = crate::codecs::deflate::deflate_compress(&src, &mut dst, level).map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

/// Decompresses raw DEFLATE buffer into expected uncompressed size.
#[uniffi::export]
pub fn uniffi_deflate_decompress(src: Vec<u8>, expected_uncompressed_size: u64) -> Result<Vec<u8>, TTZipError> {
    let mut dst = vec![0u8; expected_uncompressed_size as usize];
    let written = crate::codecs::deflate::deflate_decompress(&src, &mut dst).map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

/// Compresses buffer with zlib format (RFC 1950, levels 0..12).
#[uniffi::export]
pub fn uniffi_zlib_compress(src: Vec<u8>, level: i32) -> Result<Vec<u8>, TTZipError> {
    let bound = crate::codecs::deflate::zlib_compress_bound(src.len(), level);
    let mut dst = vec![0u8; bound];
    let written = crate::codecs::deflate::zlib_compress(&src, &mut dst, level).map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

/// Decompresses zlib buffer into expected uncompressed size.
#[uniffi::export]
pub fn uniffi_zlib_decompress(src: Vec<u8>, expected_uncompressed_size: u64) -> Result<Vec<u8>, TTZipError> {
    let mut dst = vec![0u8; expected_uncompressed_size as usize];
    let written = crate::codecs::deflate::zlib_decompress(&src, &mut dst).map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

/// Compresses buffer with gzip format (RFC 1952, levels 0..12).
#[uniffi::export]
pub fn uniffi_gzip_compress(src: Vec<u8>, level: i32) -> Result<Vec<u8>, TTZipError> {
    let bound = crate::codecs::deflate::gzip_compress_bound(src.len(), level);
    let mut dst = vec![0u8; bound];
    let written = crate::codecs::deflate::gzip_compress(&src, &mut dst, level).map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

/// Decompresses gzip buffer into expected uncompressed size.
#[uniffi::export]
pub fn uniffi_gzip_decompress(src: Vec<u8>, expected_uncompressed_size: u64) -> Result<Vec<u8>, TTZipError> {
    let mut dst = vec![0u8; expected_uncompressed_size as usize];
    let written = crate::codecs::deflate::gzip_decompress(&src, &mut dst).map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

// ============================================================================
// Zstandard & Dictionary Codec Exports
// ============================================================================

/// Compresses buffer with Zstandard (RFC 8878, levels 1..22).
#[uniffi::export]
pub fn uniffi_zstd_compress(src: Vec<u8>, level: i32) -> Result<Vec<u8>, TTZipError> {
    let bound = crate::codecs::zstd::zstd_compress_bound(src.len());
    let mut dst = vec![0u8; bound];
    let written = crate::codecs::zstd::zstd_compress(&src, &mut dst, level).map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

/// Decompresses Zstandard buffer. If size is omitted, inspects frame header or dynamically allocates.
#[uniffi::export]
pub fn uniffi_zstd_decompress(
    src: Vec<u8>,
    expected_uncompressed_size: Option<u64>,
) -> Result<Vec<u8>, TTZipError> {
    let target_size = expected_uncompressed_size
        .or_else(|| crate::codecs::zstd::zstd_get_decompressed_size(&src))
        .unwrap_or(64 * 1024 * 1024); // Fallback: 64MB upper bound for stream
    let mut dst = vec![0u8; target_size as usize];
    let written = crate::codecs::zstd::zstd_decompress(&src, &mut dst).map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

/// Compresses buffer with Zstandard Long Distance Matching (LDM).
#[uniffi::export]
pub fn uniffi_zstd_compress_ldm(
    src: Vec<u8>,
    level: i32,
    window_mb: u32,
) -> Result<Vec<u8>, TTZipError> {
    let bound = crate::codecs::zstd::zstd_compress_bound(src.len());
    let mut dst = vec![0u8; bound];
    let written = crate::codecs::zstd::zstd_compress_ldm(&src, &mut dst, level, window_mb as usize)
        .map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

/// Compresses buffer using an explicit Zstandard dictionary byte array.
#[uniffi::export]
pub fn uniffi_zstd_dict_compress(
    src: Vec<u8>,
    dict_bytes: Vec<u8>,
    level: i32,
) -> Result<Vec<u8>, TTZipError> {
    let dict = crate::codecs::zstd::dict::ZstdDictionary::from_bytes("uniffi_custom", dict_bytes, level)
        .map_err(map_status)?;
    let bound = crate::codecs::zstd::zstd_compress_bound(src.len());
    let mut dst = vec![0u8; bound];
    let written = dict.compress_small(&src, &mut dst).map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

/// Decompresses buffer using an explicit Zstandard dictionary byte array.
#[uniffi::export]
pub fn uniffi_zstd_dict_decompress(
    src: Vec<u8>,
    dict_bytes: Vec<u8>,
    expected_uncompressed_size: Option<u64>,
) -> Result<Vec<u8>, TTZipError> {
    let dict = crate::codecs::zstd::dict::ZstdDictionary::from_bytes("uniffi_custom_dec", dict_bytes, 3)
        .map_err(map_status)?;
    let target_size = expected_uncompressed_size
        .or_else(|| crate::codecs::zstd::zstd_get_decompressed_size(&src))
        .unwrap_or(16 * 1024 * 1024);
    let mut dst = vec![0u8; target_size as usize];
    let written = dict.decompress_small(&src, &mut dst).map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

/// Compresses a small buffer using a registered named pre-trained dictionary.
#[uniffi::export]
pub fn uniffi_zstd_compress_with_named_dict(
    src: Vec<u8>,
    dict_name: String,
) -> Result<Vec<u8>, TTZipError> {
    let mgr = crate::codecs::zstd::dict::ZstdDictionaryManager::global();
    let bound = crate::codecs::zstd::zstd_compress_bound(src.len());
    let mut dst = vec![0u8; bound];
    let written = mgr.compress_small_file(&dict_name, &src, &mut dst).map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

/// Decompresses a small buffer using a registered named pre-trained dictionary.
#[uniffi::export]
pub fn uniffi_zstd_decompress_with_named_dict(
    src: Vec<u8>,
    dict_name: String,
    expected_uncompressed_size: Option<u64>,
) -> Result<Vec<u8>, TTZipError> {
    let mgr = crate::codecs::zstd::dict::ZstdDictionaryManager::global();
    let dict = mgr.get_by_name(&dict_name).ok_or(TTZipError::EngineError { code: -1 })?;
    let target_size = expected_uncompressed_size
        .or_else(|| crate::codecs::zstd::zstd_get_decompressed_size(&src))
        .unwrap_or(16 * 1024 * 1024);
    let mut dst = vec![0u8; target_size as usize];
    let written = dict.decompress_small(&src, &mut dst).map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

/// Returns the raw binary bytes of the built-in 112KB standard corpus dictionary.
#[uniffi::export]
pub fn uniffi_zstd_get_standard_112kb_dict() -> Vec<u8> {
    let mgr = crate::codecs::zstd::dict::ZstdDictionaryManager::global();
    let dict = mgr.ensure_standard_112kb();
    dict.raw_bytes().to_vec()
}

// ============================================================================
// LZ4 Fast & LZ4 HC Codec Exports
// ============================================================================

/// Compresses buffer with LZ4 Fast mode (acceleration 1..100).
#[uniffi::export]
pub fn uniffi_lz4_compress_fast(src: Vec<u8>, acceleration: i32) -> Result<Vec<u8>, TTZipError> {
    let bound = crate::codecs::lz4::lz4_compress_bound(src.len());
    let mut dst = vec![0u8; bound];
    let written = crate::codecs::lz4::lz4_compress_fast(&src, &mut dst, acceleration).map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

/// Compresses buffer with LZ4 High Compression (HC, levels 1..12).
#[uniffi::export]
pub fn uniffi_lz4_compress_hc(src: Vec<u8>, level: i32) -> Result<Vec<u8>, TTZipError> {
    let bound = crate::codecs::lz4::lz4_compress_bound(src.len());
    let mut dst = vec![0u8; bound];
    let written = crate::codecs::lz4::lz4_compress_hc(&src, &mut dst, level).map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

/// Decompresses LZ4 block into expected uncompressed size.
#[uniffi::export]
pub fn uniffi_lz4_decompress(src: Vec<u8>, expected_uncompressed_size: u64) -> Result<Vec<u8>, TTZipError> {
    let mut dst = vec![0u8; expected_uncompressed_size as usize];
    let written = crate::codecs::lz4::lz4_decompress(&src, &mut dst).map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

// ============================================================================
// Apple LZFSE & LZVN Codec Exports
// ============================================================================

/// Compresses buffer with Apple LZFSE.
#[uniffi::export]
pub fn uniffi_lzfse_compress(src: Vec<u8>) -> Result<Vec<u8>, TTZipError> {
    let bound = crate::codecs::lzfse::lzfse_compress_bound(src.len());
    let mut dst = vec![0u8; bound];
    let written = crate::codecs::lzfse::lzfse_compress(&src, &mut dst).map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

/// Decompresses Apple LZFSE buffer into expected uncompressed size.
#[uniffi::export]
pub fn uniffi_lzfse_decompress(src: Vec<u8>, expected_uncompressed_size: u64) -> Result<Vec<u8>, TTZipError> {
    let mut dst = vec![0u8; expected_uncompressed_size as usize];
    let written = crate::codecs::lzfse::lzfse_decompress(&src, &mut dst).map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

/// Compresses buffer with Apple LZVN.
#[uniffi::export]
pub fn uniffi_lzvn_compress(src: Vec<u8>) -> Result<Vec<u8>, TTZipError> {
    let bound = crate::codecs::lzfse::lzvn_compress_bound(src.len());
    let mut dst = vec![0u8; bound.max(16)];
    let written = crate::codecs::lzfse::lzvn_compress(&src, &mut dst).map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

/// Decompresses Apple LZVN buffer into expected uncompressed size.
#[uniffi::export]
pub fn uniffi_lzvn_decompress(src: Vec<u8>, expected_uncompressed_size: u64) -> Result<Vec<u8>, TTZipError> {
    let mut dst = vec![0u8; expected_uncompressed_size as usize];
    let written = crate::codecs::lzfse::lzvn_decompress(&src, &mut dst).map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

// ============================================================================
// Google Brotli Codec Exports
// ============================================================================

/// Compresses buffer with Google Brotli (quality 0..11, lgwin 10..24).
#[uniffi::export]
pub fn uniffi_brotli_compress(src: Vec<u8>, quality: i32, lgwin: u32) -> Result<Vec<u8>, TTZipError> {
    let bound = crate::codecs::brotli::brotli_compress_bound(src.len());
    let mut dst = vec![0u8; bound];
    let written = crate::codecs::brotli::brotli_compress(&src, &mut dst, quality as u32, lgwin)
        .map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

/// Decompresses Google Brotli stream.
#[uniffi::export]
pub fn uniffi_brotli_decompress(
    src: Vec<u8>,
    expected_uncompressed_size: Option<u64>,
) -> Result<Vec<u8>, TTZipError> {
    let max_allowed = expected_uncompressed_size.unwrap_or(256 * 1024 * 1024) as usize;
    crate::codecs::brotli::brotli_decompress_to_vec(&src, max_allowed).map_err(map_status)
}

// ============================================================================
// Snappy Raw Block & Framed Codec Exports
// ============================================================================

/// Compresses buffer with raw Snappy block format.
#[uniffi::export]
pub fn uniffi_snappy_compress(src: Vec<u8>) -> Result<Vec<u8>, TTZipError> {
    let bound = crate::codecs::snappy::snappy_compress_bound(src.len());
    let mut dst = vec![0u8; bound];
    let written = crate::codecs::snappy::snappy_compress(&src, &mut dst).map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

/// Decompresses raw Snappy block format.
#[uniffi::export]
pub fn uniffi_snappy_decompress(src: Vec<u8>) -> Result<Vec<u8>, TTZipError> {
    let uncompressed_len = crate::codecs::snappy::snappy_uncompressed_length(&src).map_err(map_status)?;
    let mut dst = vec![0u8; uncompressed_len];
    let written = crate::codecs::snappy::snappy_decompress(&src, &mut dst).map_err(map_status)?;
    dst.truncate(written);
    Ok(dst)
}

/// Compresses buffer into official Snappy Framed stream format.
#[uniffi::export]
pub fn uniffi_snappy_frame_encode(src: Vec<u8>) -> Result<Vec<u8>, TTZipError> {
    crate::codecs::snappy::snappy_frame_encode_to_vec(&src).map_err(map_status)
}

/// Decompresses official Snappy Framed stream format.
#[uniffi::export]
pub fn uniffi_snappy_frame_decode(src: Vec<u8>) -> Result<Vec<u8>, TTZipError> {
    crate::codecs::snappy::snappy_frame_decode_to_vec(&src, 256 * 1024 * 1024).map_err(map_status)
}

// ============================================================================
// Bzip2 Codec Exports
// ============================================================================

/// Compresses buffer with Bzip2 (levels 1..9).
#[uniffi::export]
pub fn uniffi_bzip2_compress(src: Vec<u8>, level: i32) -> Result<Vec<u8>, TTZipError> {
    crate::codecs::bzip2::bzip2_compress_to_vec(&src, level).map_err(map_status)
}

/// Decompresses Bzip2 stream into memory.
#[uniffi::export]
pub fn uniffi_bzip2_decompress(
    src: Vec<u8>,
    expected_uncompressed_size: Option<u64>,
) -> Result<Vec<u8>, TTZipError> {
    let max_allowed = expected_uncompressed_size.unwrap_or(256 * 1024 * 1024) as usize;
    crate::codecs::bzip2::bzip2_decompress_to_vec(&src, max_allowed).map_err(map_status)
}

// ============================================================================
// PPMd Model H Codec Exports
// ============================================================================

/// Compresses buffer with PPMd Model H (orders 2..16, mem_mb 2..256).
#[uniffi::export]
pub fn uniffi_ppmd_compress(src: Vec<u8>, order: u32, mem_mb: u32) -> Result<Vec<u8>, TTZipError> {
    let mem_bytes = (mem_mb as usize).saturating_mul(1024 * 1024);
    crate::codecs::ppmd::ppmd_compress_to_vec(&src, order, mem_bytes).map_err(map_status)
}

/// Decompresses PPMd stream into memory.
#[uniffi::export]
pub fn uniffi_ppmd_decompress(
    src: Vec<u8>,
    expected_uncompressed_size: u64,
    order: u32,
    mem_mb: u32,
) -> Result<Vec<u8>, TTZipError> {
    let mem_bytes = (mem_mb as usize).saturating_mul(1024 * 1024);
    crate::codecs::ppmd::ppmd_decompress_to_vec(&src, expected_uncompressed_size as usize, order, mem_bytes)
        .map_err(map_status)
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PAYLOAD: &[u8] = b"Mozilla UniFFI 0.28 Codec Verification Test Payload for TTZip Engine 2026. ABCDEFGHIJKLMNOPQRSTUVWXYZ 1234567890.";

    #[test]
    fn test_deflate_variants_roundtrip() {
        // Raw Deflate
        let def_c = uniffi_deflate_compress(TEST_PAYLOAD.to_vec(), 6).expect("deflate compress");
        let def_d = uniffi_deflate_decompress(def_c, TEST_PAYLOAD.len() as u64).expect("deflate decompress");
        assert_eq!(def_d.as_slice(), TEST_PAYLOAD);

        // Zlib
        let zlib_c = uniffi_zlib_compress(TEST_PAYLOAD.to_vec(), 6).expect("zlib compress");
        let zlib_d = uniffi_zlib_decompress(zlib_c, TEST_PAYLOAD.len() as u64).expect("zlib decompress");
        assert_eq!(zlib_d.as_slice(), TEST_PAYLOAD);

        // Gzip
        let gzip_c = uniffi_gzip_compress(TEST_PAYLOAD.to_vec(), 6).expect("gzip compress");
        let gzip_d = uniffi_gzip_decompress(gzip_c, TEST_PAYLOAD.len() as u64).expect("gzip decompress");
        assert_eq!(gzip_d.as_slice(), TEST_PAYLOAD);
    }

    #[test]
    fn test_zstd_and_ldm_and_dict_roundtrip() {
        // Standard Zstd
        let zstd_c = uniffi_zstd_compress(TEST_PAYLOAD.to_vec(), 3).expect("zstd compress");
        let zstd_d = uniffi_zstd_decompress(zstd_c, None).expect("zstd decompress");
        assert_eq!(zstd_d.as_slice(), TEST_PAYLOAD);

        // Zstd LDM
        let ldm_c = uniffi_zstd_compress_ldm(TEST_PAYLOAD.to_vec(), 3, 64).expect("zstd ldm compress");
        let ldm_d = uniffi_zstd_decompress(ldm_c, Some(TEST_PAYLOAD.len() as u64)).expect("zstd ldm decompress");
        assert_eq!(ldm_d.as_slice(), TEST_PAYLOAD);

        // Zstd 112KB Dict
        let dict = uniffi_zstd_get_standard_112kb_dict();
        assert!(!dict.is_empty() && dict.len() <= 112 * 1024);

        let dict_c = uniffi_zstd_dict_compress(TEST_PAYLOAD.to_vec(), dict.clone(), 3).expect("dict compress");
        let dict_d = uniffi_zstd_dict_decompress(dict_c, dict, Some(TEST_PAYLOAD.len() as u64)).expect("dict decompress");
        assert_eq!(dict_d.as_slice(), TEST_PAYLOAD);
    }

    #[test]
    fn test_lz4_fast_and_hc_roundtrip() {
        let fast_c = uniffi_lz4_compress_fast(TEST_PAYLOAD.to_vec(), 1).expect("lz4 fast");
        let fast_d = uniffi_lz4_decompress(fast_c, TEST_PAYLOAD.len() as u64).expect("lz4 fast dec");
        assert_eq!(fast_d.as_slice(), TEST_PAYLOAD);

        let hc_c = uniffi_lz4_compress_hc(TEST_PAYLOAD.to_vec(), 9).expect("lz4 hc");
        let hc_d = uniffi_lz4_decompress(hc_c, TEST_PAYLOAD.len() as u64).expect("lz4 hc dec");
        assert_eq!(hc_d.as_slice(), TEST_PAYLOAD);
    }

    #[test]
    fn test_apple_lzfse_and_lzvn_roundtrip() {
        let lzfse_c = uniffi_lzfse_compress(TEST_PAYLOAD.to_vec()).expect("lzfse comp");
        let lzfse_d = uniffi_lzfse_decompress(lzfse_c, TEST_PAYLOAD.len() as u64).expect("lzfse dec");
        assert_eq!(lzfse_d.as_slice(), TEST_PAYLOAD);

        let lzvn_c = uniffi_lzvn_compress(TEST_PAYLOAD.to_vec()).expect("lzvn comp");
        let lzvn_d = uniffi_lzvn_decompress(lzvn_c, TEST_PAYLOAD.len() as u64).expect("lzvn dec");
        assert_eq!(lzvn_d.as_slice(), TEST_PAYLOAD);
    }

    #[test]
    fn test_brotli_snappy_bzip2_ppmd_roundtrip() {
        // Brotli
        let brotli_c = uniffi_brotli_compress(TEST_PAYLOAD.to_vec(), 6, 22).expect("brotli comp");
        let brotli_d = uniffi_brotli_decompress(brotli_c, None).expect("brotli dec");
        assert_eq!(brotli_d.as_slice(), TEST_PAYLOAD);

        // Snappy Raw & Framed
        let snap_c = uniffi_snappy_compress(TEST_PAYLOAD.to_vec()).expect("snappy comp");
        let snap_d = uniffi_snappy_decompress(snap_c).expect("snappy dec");
        assert_eq!(snap_d.as_slice(), TEST_PAYLOAD);

        let snap_f_c = uniffi_snappy_frame_encode(TEST_PAYLOAD.to_vec()).expect("snappy framed enc");
        let snap_f_d = uniffi_snappy_frame_decode(snap_f_c).expect("snappy framed dec");
        assert_eq!(snap_f_d.as_slice(), TEST_PAYLOAD);

        // Bzip2
        let bz2_c = uniffi_bzip2_compress(TEST_PAYLOAD.to_vec(), 9).expect("bz2 comp");
        let bz2_d = uniffi_bzip2_decompress(bz2_c, None).expect("bz2 dec");
        assert_eq!(bz2_d.as_slice(), TEST_PAYLOAD);

        // PPMd
        let ppmd_c = uniffi_ppmd_compress(TEST_PAYLOAD.to_vec(), 6, 16).expect("ppmd comp");
        let ppmd_d = uniffi_ppmd_decompress(ppmd_c, TEST_PAYLOAD.len() as u64, 6, 16).expect("ppmd dec");
        assert_eq!(ppmd_d.as_slice(), TEST_PAYLOAD);
    }

    #[test]
    fn test_unified_buffer_api_all_13_codecs() {
        let codecs = [
            UniFFICompressionCodec::DeflateRaw,
            UniFFICompressionCodec::Zlib,
            UniFFICompressionCodec::Gzip,
            UniFFICompressionCodec::Zstd,
            UniFFICompressionCodec::ZstdLdm,
            UniFFICompressionCodec::Lz4Fast,
            UniFFICompressionCodec::Lz4Hc,
            UniFFICompressionCodec::Lzfse,
            UniFFICompressionCodec::Lzvn,
            UniFFICompressionCodec::Brotli,
            UniFFICompressionCodec::SnappyRaw,
            UniFFICompressionCodec::SnappyFramed,
            UniFFICompressionCodec::Bzip2,
            UniFFICompressionCodec::Ppmd,
        ];

        for codec in codecs {
            let bound = uniffi_compress_bound(codec, TEST_PAYLOAD.len() as u64, None);
            assert!(bound > 0, "Bound check for {:?}", codec);

            let compressed = uniffi_compress_buffer(codec, TEST_PAYLOAD.to_vec(), None)
                .unwrap_or_else(|e| panic!("Compress failed for {:?}: {:?}", codec, e));
            assert!(!compressed.is_empty());

            let decompressed = uniffi_decompress_buffer(codec, compressed, Some(TEST_PAYLOAD.len() as u64), None)
                .unwrap_or_else(|e| panic!("Decompress failed for {:?}: {:?}", codec, e));
            assert_eq!(decompressed.as_slice(), TEST_PAYLOAD, "Payload mismatch for {:?}", codec);
        }
    }
}
