// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Brotli codec error types and diagnostics.

use crate::types::TTZipStatus;
use thiserror::Error;

/// Errors that can occur during Brotli bitstream decoding, decompression, and transforms.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BrotliError {
    /// Unexpected end of bitstream or input slice while reading bits.
    #[error("Unexpected end of Brotli bitstream")]
    UnexpectedEof,

    /// Non-zero padding bits encountered when jumping to a byte boundary.
    #[error("Invalid non-zero padding bits in Brotli stream")]
    InvalidPadding,

    /// Invalid or unsupported window bits parameter.
    #[error("Invalid window bits: {0}")]
    InvalidWindowBits(u8),

    /// Corrupted Brotli header or block metadata format.
    #[error("Corrupted Brotli header: {0}")]
    CorruptHeader(String),

    /// General Brotli decompression failure.
    #[error("Brotli decompression error: {0}")]
    DecompressionFailed(String),

    /// The specified transform index is out of bounds (must be < 121 for static dictionary).
    #[error("Invalid Brotli transform index: {0} (must be < 121)")]
    InvalidTransformIndex(usize),

    /// The destination slice does not have enough capacity for the transformed word.
    #[error("Destination buffer too small: required {required} bytes, available {available} bytes")]
    BufferTooSmall {
        required: usize,
        available: usize,
    },

    /// An invalid UTF-8 byte sequence was encountered during multi-byte transformation.
    #[error("Invalid UTF-8 sequence during dictionary word transformation")]
    InvalidUtf8Sequence,

    /// The specified compression quality is invalid (must be 0..=11).
    #[error("Invalid Brotli quality level: {0} (must be 0..=11)")]
    InvalidQuality(u32),

    /// Brotli compression failed during block/stream processing.
    #[error("Brotli compression failed")]
    CompressionFailed,

    /// Huffman space violation (Kraft inequality under-subscribed or over-subscribed tree).
    #[error("Huffman space violation: tree is over-subscribed or under-subscribed")]
    HuffmanSpaceViolation,

    /// Duplicate symbol encountered in simple prefix code.
    #[error("Duplicate symbol in simple Huffman code")]
    DuplicateSymbol,
}

impl From<BrotliError> for TTZipStatus {
    fn from(err: BrotliError) -> Self {
        match err {
            BrotliError::InvalidQuality(_)
            | BrotliError::InvalidTransformIndex(_)
            | BrotliError::BufferTooSmall { .. } => Self::ErrInvalidParam,
            BrotliError::UnexpectedEof
            | BrotliError::InvalidPadding
            | BrotliError::InvalidWindowBits(_)
            | BrotliError::InvalidUtf8Sequence
            | BrotliError::HuffmanSpaceViolation
            | BrotliError::DuplicateSymbol
            | BrotliError::CorruptHeader(_) => Self::ErrCorruptHeader,
            BrotliError::CompressionFailed => Self::ErrCompressionFailed,
            BrotliError::DecompressionFailed(_) => Self::ErrExtractionFailed,
        }
    }
}
