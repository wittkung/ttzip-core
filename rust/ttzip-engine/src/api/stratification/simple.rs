// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Layer 1: Simple Stateless One-Shot API.

use crate::codecs::{
    brotli_compress, brotli_compress_bound, brotli_decompress, bzip2_compress,
    bzip2_compress_bound, bzip2_decompress, deflate_compress, deflate_compress_bound,
    deflate_decompress, fl2_compress, fl2_compress_bound, fl2_decompress, gzip_compress,
    gzip_compress_bound, gzip_decompress, lz4_compress, lz4_compress_bound, lz4_decompress,
    lzfse_compress, lzfse_compress_bound, lzfse_decompress, snappy_compress,
    snappy_compress_bound, snappy_decompress, zstd_compress, zstd_compress_bound,
    zstd_decompress,
};
use crate::types::{TTZipArchiveFormat, TTZipCompressionLevel, TTZipStatus};

/// Computes a safe upper bound on compressed size for a given uncompressed length and format.
#[must_use]
pub fn simple_compress_bound(
    uncompressed_len: usize,
    format: TTZipArchiveFormat,
    level: TTZipCompressionLevel,
) -> usize {
    let lvl = level as i32;
    match format {
        TTZipArchiveFormat::Gzip | TTZipArchiveFormat::TarGz => {
            gzip_compress_bound(uncompressed_len, lvl)
        }
        TTZipArchiveFormat::Bzip2 | TTZipArchiveFormat::TarBz2 => {
            bzip2_compress_bound(uncompressed_len)
        }
        TTZipArchiveFormat::Xz | TTZipArchiveFormat::TarXz | TTZipArchiveFormat::SevenZip => {
            fl2_compress_bound(uncompressed_len)
        }
        TTZipArchiveFormat::Zstd | TTZipArchiveFormat::TarZstd => {
            zstd_compress_bound(uncompressed_len)
        }
        TTZipArchiveFormat::Lz4 | TTZipArchiveFormat::TarLz4 => {
            lz4_compress_bound(uncompressed_len)
        }
        TTZipArchiveFormat::Snappy => snappy_compress_bound(uncompressed_len),
        TTZipArchiveFormat::Brotli | TTZipArchiveFormat::TarBrotli => {
            brotli_compress_bound(uncompressed_len)
        }
        TTZipArchiveFormat::Lzfse => lzfse_compress_bound(uncompressed_len),
        _ => deflate_compress_bound(uncompressed_len, lvl),
    }
}

/// One-shot stateless compression of an in-memory byte slice into a newly allocated vector.
pub fn simple_compress(
    data: &[u8],
    format: TTZipArchiveFormat,
    level: TTZipCompressionLevel,
) -> Result<Vec<u8>, TTZipStatus> {
    let bound = simple_compress_bound(data.len(), format, level).max(64);
    let mut out = vec![0u8; bound];
    let written = simple_compress_to_slice(data, &mut out, format, level)?;
    out.truncate(written);
    Ok(out)
}

/// One-shot stateless compression of an in-memory byte slice into a caller-provided destination slice.
pub fn simple_compress_to_slice(
    src: &[u8],
    dst: &mut [u8],
    format: TTZipArchiveFormat,
    level: TTZipCompressionLevel,
) -> Result<usize, TTZipStatus> {
    let lvl = level as i32;
    match format {
        TTZipArchiveFormat::Gzip | TTZipArchiveFormat::TarGz => gzip_compress(src, dst, lvl),
        TTZipArchiveFormat::Bzip2 | TTZipArchiveFormat::TarBz2 => bzip2_compress(src, dst, lvl),
        TTZipArchiveFormat::Xz | TTZipArchiveFormat::TarXz | TTZipArchiveFormat::SevenZip => {
            fl2_compress(src, dst, lvl, 0)
        }
        TTZipArchiveFormat::Zstd | TTZipArchiveFormat::TarZstd => zstd_compress(src, dst, lvl),
        TTZipArchiveFormat::Lz4 | TTZipArchiveFormat::TarLz4 => lz4_compress(src, dst),
        TTZipArchiveFormat::Snappy => snappy_compress(src, dst),
        TTZipArchiveFormat::Brotli | TTZipArchiveFormat::TarBrotli => {
            let quality = (lvl as u32).clamp(0, 11);
            brotli_compress(src, dst, quality, 22)
        }
        TTZipArchiveFormat::Lzfse => lzfse_compress(src, dst),
        _ => deflate_compress(src, dst, lvl),
    }
}

/// One-shot stateless decompression of an in-memory byte slice into a newly allocated vector.
pub fn simple_decompress(
    data: &[u8],
    format: TTZipArchiveFormat,
) -> Result<Vec<u8>, TTZipStatus> {
    let estimated_cap = data.len().saturating_mul(4).max(1024);
    let mut out = vec![0u8; estimated_cap];

    // Try decompressing into initial buffer; if buffer is too small, double capacity up to 1GB
    for _ in 0..6 {
        match simple_decompress_to_slice(data, &mut out, format) {
            Ok(written) => {
                out.truncate(written);
                return Ok(out);
            }
            Err(TTZipStatus::ErrExtractionFailed) | Err(TTZipStatus::ErrInvalidParam) => {
                let new_len = out.len().saturating_mul(2).min(1024 * 1024 * 1024);
                if new_len == out.len() {
                    return Err(TTZipStatus::ErrExtractionFailed);
                }
                out.resize(new_len, 0);
            }
            Err(status) => return Err(status),
        }
    }
    Err(TTZipStatus::ErrExtractionFailed)
}

/// One-shot stateless decompression of an in-memory byte slice into a caller-provided destination slice.
pub fn simple_decompress_to_slice(
    src: &[u8],
    dst: &mut [u8],
    format: TTZipArchiveFormat,
) -> Result<usize, TTZipStatus> {
    match format {
        TTZipArchiveFormat::Gzip | TTZipArchiveFormat::TarGz => gzip_decompress(src, dst),
        TTZipArchiveFormat::Bzip2 | TTZipArchiveFormat::TarBz2 => bzip2_decompress(src, dst),
        TTZipArchiveFormat::Xz | TTZipArchiveFormat::TarXz | TTZipArchiveFormat::SevenZip => {
            fl2_decompress(src, dst, 0)
        }
        TTZipArchiveFormat::Zstd | TTZipArchiveFormat::TarZstd => zstd_decompress(src, dst),
        TTZipArchiveFormat::Lz4 | TTZipArchiveFormat::TarLz4 => lz4_decompress(src, dst),
        TTZipArchiveFormat::Snappy => snappy_decompress(src, dst),
        TTZipArchiveFormat::Brotli | TTZipArchiveFormat::TarBrotli => brotli_decompress(src, dst),
        TTZipArchiveFormat::Lzfse => lzfse_decompress(src, dst),
        _ => deflate_decompress(src, dst),
    }
}
