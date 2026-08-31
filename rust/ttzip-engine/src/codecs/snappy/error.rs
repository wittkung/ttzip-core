// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Strongly-typed Snappy codec error types and status mappings.

use crate::types::TTZipStatus;
use thiserror::Error;

/// Error variants encountered during Snappy varint decoding, bytecode tag parsing,
/// decompression, or frame verification.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SnappyError {
    /// Destination buffer is too small to receive compressed or decompressed output.
    #[error("Destination buffer too small: required {required} bytes, available {available} bytes")]
    BufferTooSmall {
        /// Number of bytes required.
        required: usize,
        /// Number of bytes available.
        available: usize,
    },

    /// A Snappy fragment or block exceeds allowable block boundary limits.
    #[error("Snappy block size {size} exceeds maximum chunk limit {max}")]
    BlockTooLarge {
        /// Actual block size.
        size: usize,
        /// Maximum allowed chunk size.
        max: usize,
    },

    /// Backreference copy offset exceeds currently decompressed history or is zero.
    #[error("Invalid backreference offset {offset} at decompressed position {position}")]
    InvalidOffset {
        /// Requested backreference distance.
        offset: u32,
        /// Current uncompressed stream cursor.
        position: usize,
    },

    /// Back-reference copy offset points outside the decompressed window or is zero.
    #[error("Snappy back-reference offset {offset} is out of bounds at position {current_pos}")]
    OffsetOutOfBounds {
        /// The invalid offset value.
        offset: usize,
        /// Current uncompressed write position.
        current_pos: usize,
    },

    /// The LEB128 unsigned varint header is corrupted or truncated.
    #[error("Corrupted LEB128 varint header in Snappy bitstream")]
    CorruptVarint,

    /// Varint-32 value exceeds 32-bit boundary or requires more than 5 bytes.
    #[error("Varint-32 overflow: encoded value exceeds 32 bits or 5 bytes")]
    VarintOverflow,

    /// General corruption in Snappy header or element tag structure.
    #[error("Corrupted Snappy header: {0}")]
    CorruptHeader(String),

    /// Invalid or unsupported Snappy tag byte.
    #[error("Invalid Snappy bytecode tag: {0:#04x}")]
    InvalidTag(u8),

    /// Literal length descriptor exceeds maximum supported uncompressed block limit.
    #[error("Literal length {length} exceeds maximum allowed limit {max}")]
    LiteralLengthExceeded {
        /// Encountered literal length.
        length: usize,
        /// Configured upper bound.
        max: usize,
    },

    /// Snappy raw block compression failed.
    #[error("Snappy compression failed")]
    CompressionFailed,

    /// General decompression failure or corrupt bitstream.
    #[error("Snappy decompression failed: {0}")]
    DecompressionFailed(String),

    /// Unexpected end of input slice while decoding Snappy bitstream.
    #[error("Unexpected end of Snappy bitstream")]
    UnexpectedEof,

    /// Castagnoli CRC-32C checksum mismatch during frame chunk validation.
    #[error("Castagnoli CRC-32C mismatch: expected 0x{expected:08X}, calculated 0x{actual:08X}")]
    Crc32cMismatch {
        /// Expected masked CRC32C.
        expected: u32,
        /// Calculated masked CRC32C.
        actual: u32,
    },

    /// Encountered an unsupported or unhandled Snappy framing chunk type.
    #[error("Unsupported Snappy framing chunk type: 0x{0:02X}")]
    UnsupportedChunkType(u8),

    /// Missing or invalid 10-byte Snappy stream identifier header.
    #[error("Invalid Snappy stream identifier magic header")]
    InvalidMagicHeader,

    /// Invalid parameter supplied to Snappy codec API.
    #[error("Invalid parameter: {0}")]
    InvalidParam(String),
}

impl From<SnappyError> for TTZipStatus {
    #[inline]
    fn from(err: SnappyError) -> Self {
        match err {
            SnappyError::BufferTooSmall { .. }
            | SnappyError::BlockTooLarge { .. }
            | SnappyError::LiteralLengthExceeded { .. }
            | SnappyError::InvalidParam(_) => Self::ErrInvalidParam,
            SnappyError::InvalidOffset { .. } | SnappyError::OffsetOutOfBounds { .. } => {
                Self::ErrInvalidOffset
            }
            SnappyError::CorruptVarint
            | SnappyError::VarintOverflow
            | SnappyError::CorruptHeader(_)
            | SnappyError::InvalidTag(_)
            | SnappyError::UnexpectedEof
            | SnappyError::Crc32cMismatch { .. }
            | SnappyError::UnsupportedChunkType(_)
            | SnappyError::InvalidMagicHeader => Self::ErrCorruptHeader,
            SnappyError::CompressionFailed => Self::ErrCompressionFailed,
            SnappyError::DecompressionFailed(_) => Self::ErrExtractionFailed,
        }
    }
}

impl From<TTZipStatus> for SnappyError {
    #[inline]
    fn from(status: TTZipStatus) -> Self {
        match status {
            TTZipStatus::Ok => Self::InvalidParam("Success status cannot be converted to error".to_string()),
            TTZipStatus::Eof => Self::UnexpectedEof,
            TTZipStatus::ErrCorruptHeader => Self::CorruptHeader("Header corruption detected".to_string()),
            TTZipStatus::ErrInvalidOffset => Self::InvalidOffset {
                offset: 0,
                position: 0,
            },
            TTZipStatus::ErrInvalidParam => Self::InvalidParam("Invalid parameter".to_string()),
            TTZipStatus::ErrCompressionFailed => Self::CompressionFailed,
            TTZipStatus::ErrExtractionFailed => {
                Self::DecompressionFailed("Extraction/decompression failed".to_string())
            }
            other => Self::DecompressionFailed(format!("Status code {}: {}", other as i32, other.as_str())),
        }
    }
}
