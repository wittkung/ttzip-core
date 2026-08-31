// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zero-Vtable Statically Monomorphized Strategy Dispatch Engine.
//!
//! Provides zero-vtable polymorphism across 11 core archive and compression engines
//! via strong-typed enum variants and direct `match` inlining (`#[inline(always)]`).
//!
//! Eliminates `Box<dyn ArchiveEngine>` vtable indirect jumps, branch mispredictions,
//! and pointer indirection cache misses during high-throughput chunk processing.

use crate::types::{TTZipArchiveFormat, TTZipCompressionLevel, TTZipStatus};

// MARK: - Strategy Header Information

/// Inspection metadata parsed directly from format headers without memory allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrategyHeaderInfo {
    pub format: TTZipArchiveFormat,
    pub is_valid: bool,
    pub has_magic: bool,
    pub uncompressed_size: Option<u64>,
    pub block_size: Option<usize>,
    pub compression_level: Option<u32>,
}

// MARK: - Strategy Markers

/// ZIP Archive format strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ZipStrategy;

/// TAR Archive container format strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TarStrategy;

/// 7-Zip (7z) Archive format strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SevenZipStrategy;

/// Zstandard (zstd) high-throughput block/stream compression strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ZstdStrategy;

/// LZ4 ultra-fast block compression strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Lz4Strategy;

/// Gzip / Deflate stream compression strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GzipStrategy;

/// Bzip2 BWT statistical compression strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Bzip2Strategy;

/// Google Brotli web/general purpose compression strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BrotliStrategy;

/// Apple LZFSE / LZVN native compression strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LzfseStrategy;

/// Google Snappy framed/raw block compression strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SnappyStrategy;

/// XZ / LZMA high-ratio compression strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct XzStrategy;

// MARK: - Zero-Vtable Strategy Enum

/// Strong-typed zero-vtable polymorphic strategy dispatcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveEngineStrategy {
    Zip(ZipStrategy),
    Tar(TarStrategy),
    SevenZip(SevenZipStrategy),
    Zstd(ZstdStrategy),
    Lz4(Lz4Strategy),
    Gz(GzipStrategy),
    Bz2(Bzip2Strategy),
    Brotli(BrotliStrategy),
    Lzfse(LzfseStrategy),
    Snappy(SnappyStrategy),
    Xz(XzStrategy),
}

impl ArchiveEngineStrategy {
    /// Resolves an `ArchiveEngineStrategy` from a strong-typed `TTZipArchiveFormat`.
    #[inline]
    pub fn from_format(format: TTZipArchiveFormat) -> Result<Self, TTZipStatus> {
        match format {
            TTZipArchiveFormat::Zip => Ok(Self::Zip(ZipStrategy)),
            TTZipArchiveFormat::Tar => Ok(Self::Tar(TarStrategy)),
            TTZipArchiveFormat::SevenZip => Ok(Self::SevenZip(SevenZipStrategy)),
            TTZipArchiveFormat::Zstd | TTZipArchiveFormat::TarZstd => Ok(Self::Zstd(ZstdStrategy)),
            TTZipArchiveFormat::Lz4 | TTZipArchiveFormat::TarLz4 => Ok(Self::Lz4(Lz4Strategy)),
            TTZipArchiveFormat::Gzip | TTZipArchiveFormat::TarGz => Ok(Self::Gz(GzipStrategy)),
            TTZipArchiveFormat::Bzip2 | TTZipArchiveFormat::TarBz2 => Ok(Self::Bz2(Bzip2Strategy)),
            TTZipArchiveFormat::Brotli | TTZipArchiveFormat::TarBrotli => {
                Ok(Self::Brotli(BrotliStrategy))
            }
            TTZipArchiveFormat::Lzfse => Ok(Self::Lzfse(LzfseStrategy)),
            TTZipArchiveFormat::Snappy => Ok(Self::Snappy(SnappyStrategy)),
            TTZipArchiveFormat::Xz | TTZipArchiveFormat::TarXz => Ok(Self::Xz(XzStrategy)),
            _ => Err(TTZipStatus::ErrUnsupportedFeature),
        }
    }

    /// Sniffs signature bytes to detect format strategy with zero allocations.
    #[inline]
    pub fn detect_from_magic(magic: &[u8]) -> Option<Self> {
        if magic.len() < 2 {
            return None;
        }

        // ZIP: PK\x03\x04 or PK\x05\x06 or PK\x07\x08
        if magic.len() >= 4
            && magic[0] == 0x50
            && magic[1] == 0x4B
            && ((magic[2] == 0x03 && magic[3] == 0x04)
                || (magic[2] == 0x05 && magic[3] == 0x06)
                || (magic[2] == 0x07 && magic[3] == 0x08))
        {
            return Some(Self::Zip(ZipStrategy));
        }

        // 7-Zip: '7' 'z' 0xBC 0xAF 0x27 0x1C
        if magic.len() >= 6
            && magic[0] == 0x37
            && magic[1] == 0x7A
            && magic[2] == 0xBC
            && magic[3] == 0xAF
            && magic[4] == 0x27
            && magic[5] == 0x1C
        {
            return Some(Self::SevenZip(SevenZipStrategy));
        }

        // Zstandard: 0x28 0xB5 0x2F 0xFD
        if magic.len() >= 4
            && magic[0] == 0x28
            && magic[1] == 0xB5
            && magic[2] == 0x2F
            && magic[3] == 0xFD
        {
            return Some(Self::Zstd(ZstdStrategy));
        }

        // Gzip: 0x1F 0x8B
        if magic[0] == 0x1F && magic[1] == 0x8B {
            return Some(Self::Gz(GzipStrategy));
        }

        // Bzip2: 'B' 'Z' 'h'
        if magic.len() >= 3 && magic[0] == 0x42 && magic[1] == 0x5A && magic[2] == 0x68 {
            return Some(Self::Bz2(Bzip2Strategy));
        }

        // LZ4 Frame: 0x04 0x22 0x4D 0x18
        if magic.len() >= 4
            && magic[0] == 0x04
            && magic[1] == 0x22
            && magic[2] == 0x4D
            && magic[3] == 0x18
        {
            return Some(Self::Lz4(Lz4Strategy));
        }

        // XZ: 0xFD '7' 'z' 'X' 'Z' 0x00
        if magic.len() >= 6
            && magic[0] == 0xFD
            && magic[1] == 0x37
            && magic[2] == 0x7A
            && magic[3] == 0x58
            && magic[4] == 0x5A
            && magic[5] == 0x00
        {
            return Some(Self::Xz(XzStrategy));
        }

        // LZFSE: 'b' 'v' 'x' '2' or 'b' 'v' 'x' '-' or 'b' 'v' 'x' 'n'
        if magic.len() >= 4 && magic[0] == 0x62 && magic[1] == 0x76 && magic[2] == 0x78 {
            return Some(Self::Lzfse(LzfseStrategy));
        }

        // Snappy Framed: 0xFF 0x06 0x00 0x00 's' 'N' 'a' 'P' 'p' 'Y'
        if magic.len() >= 10
            && magic[0] == 0xFF
            && magic[1] == 0x06
            && magic[2] == 0x00
            && magic[3] == 0x00
            && &magic[4..10] == b"sNaPpY"
        {
            return Some(Self::Snappy(SnappyStrategy));
        }

        // TAR: ustar magic at offset 257 (handled if sufficient length)
        if magic.len() >= 262 && &magic[257..262] == b"ustar" {
            return Some(Self::Tar(TarStrategy));
        }

        None
    }

    /// Returns corresponding canonical `TTZipArchiveFormat`.
    #[inline(always)]
    pub const fn format(&self) -> TTZipArchiveFormat {
        match self {
            Self::Zip(_) => TTZipArchiveFormat::Zip,
            Self::Tar(_) => TTZipArchiveFormat::Tar,
            Self::SevenZip(_) => TTZipArchiveFormat::SevenZip,
            Self::Zstd(_) => TTZipArchiveFormat::Zstd,
            Self::Lz4(_) => TTZipArchiveFormat::Lz4,
            Self::Gz(_) => TTZipArchiveFormat::Gzip,
            Self::Bz2(_) => TTZipArchiveFormat::Bzip2,
            Self::Brotli(_) => TTZipArchiveFormat::Brotli,
            Self::Lzfse(_) => TTZipArchiveFormat::Lzfse,
            Self::Snappy(_) => TTZipArchiveFormat::Snappy,
            Self::Xz(_) => TTZipArchiveFormat::Xz,
        }
    }

    /// Returns canonical format name.
    #[inline(always)]
    pub const fn name(&self) -> &'static str {
        self.format().as_str()
    }

    /// Returns canonical MIME type.
    #[inline(always)]
    pub const fn mime_type(&self) -> &'static str {
        self.format().mime_type()
    }

    /// Returns `true` if this strategy represents a multi-file container archive.
    #[inline(always)]
    pub const fn is_container(&self) -> bool {
        matches!(self, Self::Zip(_) | Self::Tar(_) | Self::SevenZip(_))
    }

    /// Returns `true` if this strategy represents a single-stream compression algorithm.
    #[inline(always)]
    pub const fn is_stream_compressor(&self) -> bool {
        !self.is_container()
    }

    /// Returns `true` if this engine supports parallel multi-threaded block operations.
    #[inline(always)]
    pub const fn supports_multithreading(&self) -> bool {
        match self {
            Self::Zstd(_) | Self::Lz4(_) | Self::Snappy(_) | Self::Gz(_) | Self::Brotli(_) => true,
            Self::Zip(_) | Self::Tar(_) | Self::SevenZip(_) | Self::Bz2(_) | Self::Lzfse(_)
            | Self::Xz(_) => false,
        }
    }

    /// Returns default optimal chunk size for streaming block operations.
    #[inline(always)]
    pub const fn default_chunk_size(&self) -> usize {
        match self {
            Self::Zstd(_) => 1024 * 1024,      // 1MB Zstd block
            Self::Lz4(_) => 256 * 1024,        // 256KB LZ4 block
            Self::Snappy(_) => 64 * 1024,      // 64KB Snappy block
            Self::Gz(_) => 128 * 1024,         // 128KB Gzip Deflate block
            Self::Brotli(_) => 512 * 1024,     // 512KB Brotli block
            Self::Bz2(_) => 900 * 1024,        // 900KB Bzip2 level 9 block
            Self::Lzfse(_) => 256 * 1024,      // 256KB LZFSE block
            Self::Xz(_) => 2 * 1024 * 1024,    // 2MB XZ block
            Self::Zip(_) | Self::Tar(_) | Self::SevenZip(_) => 512 * 1024,
        }
    }

    /// Decompresses a single chunk from `input` into pre-allocated `output`.
    #[inline(always)]
    pub fn decompress_chunk(&self, input: &[u8], output: &mut [u8]) -> Result<usize, TTZipStatus> {
        match self {
            Self::Zstd(_) => crate::codecs::zstd::zstd_decompress(input, output),
            Self::Lz4(_) => crate::codecs::lz4::lz4_decompress(input, output),
            Self::Gz(_) => crate::codecs::deflate::gzip_decompress(input, output),
            Self::Brotli(_) => crate::codecs::brotli::brotli_decompress(input, output),
            Self::Snappy(_) => crate::codecs::snappy::snappy_decompress(input, output),
            Self::Lzfse(_) => crate::codecs::lzfse::lzfse_decompress(input, output),
            Self::Bz2(_) => {
                let decompressed =
                    crate::codecs::bzip2::bzip2_decompress_to_vec(input, output.len())?;
                if decompressed.len() > output.len() {
                    return Err(TTZipStatus::ErrExtractionFailed);
                }
                output[..decompressed.len()].copy_from_slice(&decompressed);
                Ok(decompressed.len())
            }
            Self::Xz(_) => {
                let coder_props = if input.len() >= 5 { &input[0..5] } else { &[0u8; 5] };
                let raw_payload = if input.len() >= 5 { &input[5..] } else { input };
                crate::codecs::lzma::lzma1_decompress(
                    raw_payload,
                    coder_props,
                    output.len() as u64,
                    output,
                )
            }
            Self::Zip(_) | Self::Tar(_) | Self::SevenZip(_) => {
                Err(TTZipStatus::ErrUnsupportedFeature)
            }
        }
    }

    /// Compresses a single chunk from `input` into pre-allocated `output`.
    #[inline(always)]
    pub fn compress_chunk(
        &self,
        input: &[u8],
        output: &mut [u8],
        level: TTZipCompressionLevel,
    ) -> Result<usize, TTZipStatus> {
        let level_int = match level {
            TTZipCompressionLevel::Store => 0,
            TTZipCompressionLevel::Fastest => 1,
            TTZipCompressionLevel::Fast => 3,
            TTZipCompressionLevel::Normal => 6,
            TTZipCompressionLevel::Maximum => 9,
            TTZipCompressionLevel::Ultra => 19,
        };

        match self {
            Self::Zstd(_) => crate::codecs::zstd::zstd_compress(input, output, level_int),
            Self::Lz4(_) => {
                if level == TTZipCompressionLevel::Fastest {
                    crate::codecs::lz4::lz4_compress_fast(input, output, 1)
                } else if level_int >= 6 {
                    crate::codecs::lz4::lz4_compress_hc(input, output, level_int.min(12))
                } else {
                    crate::codecs::lz4::lz4_compress(input, output)
                }
            }
            Self::Gz(_) => crate::codecs::deflate::gzip_compress(input, output, level_int.clamp(1, 9)),
            Self::Brotli(_) => {
                let q = level_int.clamp(0, 11) as u32;
                crate::codecs::brotli::brotli_compress(input, output, q, 22)
            }
            Self::Snappy(_) => crate::codecs::snappy::snappy_compress(input, output),
            Self::Lzfse(_) => crate::codecs::lzfse::lzfse_compress(input, output),
            Self::Bz2(_) => {
                let bz_level = level_int.clamp(1, 9);
                let comp = crate::codecs::bzip2::bzip2_compress_to_vec(input, bz_level)?;
                if comp.len() > output.len() {
                    return Err(TTZipStatus::ErrCompressionFailed);
                }
                output[..comp.len()].copy_from_slice(&comp);
                Ok(comp.len())
            }
            Self::Xz(_) | Self::Zip(_) | Self::Tar(_) | Self::SevenZip(_) => {
                Err(TTZipStatus::ErrUnsupportedFeature)
            }
        }
    }

    /// Compresses `input` into newly allocated `Vec<u8>`.
    #[inline]
    pub fn compress_to_vec(
        &self,
        input: &[u8],
        level: TTZipCompressionLevel,
    ) -> Result<Vec<u8>, TTZipStatus> {
        let level_int = match level {
            TTZipCompressionLevel::Store => 0,
            TTZipCompressionLevel::Fastest => 1,
            TTZipCompressionLevel::Fast => 3,
            TTZipCompressionLevel::Normal => 6,
            TTZipCompressionLevel::Maximum => 9,
            TTZipCompressionLevel::Ultra => 19,
        };

        match self {
            Self::Zstd(_) => {
                let bound = crate::codecs::zstd::zstd_compress_bound(input.len());
                let mut out = vec![0u8; bound];
                let written = crate::codecs::zstd::zstd_compress(input, &mut out, level_int)?;
                out.truncate(written);
                Ok(out)
            }
            Self::Lz4(_) => {
                let bound = crate::codecs::lz4::lz4_compress_bound(input.len());
                let mut out = vec![0u8; bound];
                let written = if level_int >= 6 {
                    crate::codecs::lz4::lz4_compress_hc(input, &mut out, level_int.min(12))?
                } else {
                    crate::codecs::lz4::lz4_compress(input, &mut out)?
                };
                out.truncate(written);
                Ok(out)
            }
            Self::Gz(_) => {
                let bound = input.len() + (input.len() / 16) + 64;
                let mut out = vec![0u8; bound];
                let written = crate::codecs::deflate::gzip_compress(
                    input,
                    &mut out,
                    level_int.clamp(1, 9),
                )?;
                out.truncate(written);
                Ok(out)
            }
            Self::Brotli(_) => {
                let q = level_int.clamp(0, 11) as u32;
                crate::codecs::brotli::brotli_compress_to_vec(input, q, 22)
            }
            Self::Snappy(_) => {
                let bound = crate::codecs::snappy::snappy_compress_bound(input.len());
                let mut out = vec![0u8; bound];
                let written = crate::codecs::snappy::snappy_compress(input, &mut out)?;
                out.truncate(written);
                Ok(out)
            }
            Self::Lzfse(_) => crate::codecs::lzfse::lzfse_compress_to_vec(input),
            Self::Bz2(_) => {
                let bz_level = level_int.clamp(1, 9);
                crate::codecs::bzip2::bzip2_compress_to_vec(input, bz_level)
            }
            Self::Xz(_) | Self::Zip(_) | Self::Tar(_) | Self::SevenZip(_) => {
                Err(TTZipStatus::ErrUnsupportedFeature)
            }
        }
    }

    /// Decompresses `input` into newly allocated `Vec<u8>`.
    #[inline]
    pub fn decompress_to_vec(&self, input: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
        match self {
            Self::Zstd(_) => {
                let upper_bound = crate::codecs::zstd::zstd_get_decompressed_size(input)
                    .unwrap_or(input.len().saturating_mul(4).max(64 * 1024) as u64);
                let mut out = vec![0u8; upper_bound as usize];
                let written = crate::codecs::zstd::zstd_decompress(input, &mut out)?;
                out.truncate(written);
                Ok(out)
            }
            Self::Lz4(_) => {
                let mut out = vec![0u8; input.len().saturating_mul(4).max(64 * 1024)];
                let written = crate::codecs::lz4::lz4_decompress(input, &mut out)?;
                out.truncate(written);
                Ok(out)
            }
            Self::Gz(_) => {
                let mut out = vec![0u8; input.len().saturating_mul(4).max(64 * 1024)];
                let written = crate::codecs::deflate::gzip_decompress(input, &mut out)?;
                out.truncate(written);
                Ok(out)
            }
            Self::Brotli(_) => {
                crate::codecs::brotli::brotli_decompress_to_vec(input, 64 * 1024 * 1024)
            }
            Self::Snappy(_) => {
                let uncompressed_len =
                    crate::codecs::snappy::snappy_uncompressed_length(input).unwrap_or(0);
                let mut out = vec![0u8; uncompressed_len];
                let written = crate::codecs::snappy::snappy_decompress(input, &mut out)?;
                out.truncate(written);
                Ok(out)
            }
            Self::Lzfse(_) => {
                let mut out = vec![0u8; input.len().saturating_mul(16).max(64 * 1024)];
                let written = crate::codecs::lzfse::lzfse_decompress(input, &mut out)?;
                out.truncate(written);
                Ok(out)
            }
            Self::Bz2(_) => {
                crate::codecs::bzip2::bzip2_decompress_to_vec(input, 64 * 1024 * 1024)
            }
            Self::Xz(_) | Self::Zip(_) | Self::Tar(_) | Self::SevenZip(_) => {
                Err(TTZipStatus::ErrUnsupportedFeature)
            }
        }
    }

    /// Fast inspects format headers without decompression.
    #[inline]
    pub fn inspect_header(&self, input: &[u8]) -> Result<StrategyHeaderInfo, TTZipStatus> {
        let fmt = self.format();
        let has_magic = Self::detect_from_magic(input).is_some();

        match self {
            Self::Zstd(_) => {
                let unc_sz = crate::codecs::zstd::zstd_get_decompressed_size(input);
                Ok(StrategyHeaderInfo {
                    format: fmt,
                    is_valid: has_magic,
                    has_magic,
                    uncompressed_size: unc_sz,
                    block_size: Some(1024 * 1024),
                    compression_level: None,
                })
            }
            Self::Bz2(_) => {
                let info = crate::codecs::bzip2::bzip2_inspect_header(input).ok();
                Ok(StrategyHeaderInfo {
                    format: fmt,
                    is_valid: info.as_ref().map(|i| i.is_valid).unwrap_or(false),
                    has_magic,
                    uncompressed_size: None,
                    block_size: info.as_ref().map(|i| i.block_size_bytes),
                    compression_level: info.as_ref().map(|i| i.level as u32),
                })
            }
            Self::Snappy(_) => {
                let unc_sz = crate::codecs::snappy::snappy_uncompressed_length(input).ok();
                Ok(StrategyHeaderInfo {
                    format: fmt,
                    is_valid: unc_sz.is_some() || has_magic,
                    has_magic,
                    uncompressed_size: unc_sz.map(|s| s as u64),
                    block_size: Some(64 * 1024),
                    compression_level: None,
                })
            }
            _ => Ok(StrategyHeaderInfo {
                format: fmt,
                is_valid: has_magic,
                has_magic,
                uncompressed_size: None,
                block_size: Some(self.default_chunk_size()),
                compression_level: None,
            }),
        }
    }
}
