// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Seekable Zstandard data types, constants, and error definitions.

use std::io;

/// Zstandard Seekable Format Magic Number (0x8F92EAB1).
pub const SEEKABLE_MAGIC_NUMBER: u32 = 0x8F92EAB1;

/// Zstandard Skippable Frame Magic Number for Seek Table (0x184D2A5E).
pub const SKIPPABLE_MAGIC_NUMBER: u32 = 0x184D2A5E;

/// Fixed byte size of the Seek Table footer (4 + 1 + 4 = 9 bytes).
pub const SEEKABLE_FOOTER_SIZE: usize = 9;

/// Fixed byte size of the Skippable Frame header (4 + 4 = 8 bytes).
pub const SKIPPABLE_HEADER_SIZE: usize = 8;

/// Descriptor flag: Bit 7 indicates that a 4-byte checksum is present for every frame entry.
pub const SEEK_TABLE_FLAG_CHECKSUM: u8 = 0x80;

/// Default frame chunk size for seekable archive writers (256 KB).
pub const DEFAULT_SEEKABLE_FRAME_SIZE: usize = 256 * 1024;

/// Errors returned during Seek Table generation, parsing, or random-access decompression.
#[derive(Debug, thiserror::Error)]
pub enum SeekableError {
    /// The input archive is smaller than the minimum seek table size.
    #[error("Archive size ({0} bytes) is too small to contain a valid seek table")]
    ArchiveTooSmall(usize),

    /// The Seekable magic number in footer does not match 0x8F92EAB1.
    #[error("Invalid seekable magic number: expected 0x{expected:08X}, found 0x{found:08X}")]
    InvalidSeekableMagic { expected: u32, found: u32 },

    /// The Skippable Frame magic number does not match 0x184D2A5E.
    #[error("Invalid skippable frame magic number: expected 0x{expected:08X}, found 0x{found:08X}")]
    InvalidSkippableMagic { expected: u32, found: u32 },

    /// Skippable frame length declared in header does not match calculated seek table payload size.
    #[error("Mismatched seek table frame size: expected {expected} bytes, declared {declared} bytes")]
    MismatchedFrameSize { expected: u32, declared: u32 },

    /// Frame index is out of bounds.
    #[error("Frame index {index} out of bounds (total frames: {total})")]
    FrameIndexOutOfBounds { index: usize, total: usize },

    /// Decompressed byte offset is beyond the total uncompressed length.
    #[error("Decompressed offset {offset} out of bounds (total decompressed size: {total})")]
    OffsetOutOfBounds { offset: u64, total: u64 },

    /// Decompression failure in the underlying Zstd engine.
    #[error("Decompression failed for frame {frame_index}: {reason}")]
    DecompressionFailed { frame_index: usize, reason: String },

    /// Checksum mismatch for decompressed frame data.
    #[error("Checksum mismatch for frame {frame_index}: expected 0x{expected:08X}, calculated 0x{calculated:08X}")]
    ChecksumMismatch {
        frame_index: usize,
        expected: u32,
        calculated: u32,
    },

    /// Destination buffer is too small for uncompressed output.
    #[error("Destination buffer too small: required {required} bytes, provided {provided} bytes")]
    BufferTooSmall { required: usize, provided: usize },

    /// Underlying I/O error during stream reading/seeking.
    #[error("I/O error during seekable archive operation: {0}")]
    Io(#[from] io::Error),
}

/// Metadata entry describing a single compressed Zstandard frame in the Seek Table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeekTableEntry {
    /// Compressed frame size in bytes.
    pub c_size: u32,
    /// Decompressed data size in bytes.
    pub d_size: u32,
    /// Optional 32-bit checksum (CRC-32 / XXH32) of the uncompressed data.
    pub checksum: u32,
}

/// Resolved random-access coordinates for a specific frame in the archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeekFrameInfo {
    /// 0-indexed frame index.
    pub frame_index: usize,
    /// Byte offset where this frame begins in the compressed archive stream.
    pub c_offset: u64,
    /// Compressed size in bytes.
    pub c_size: u32,
    /// Byte offset of this frame's first uncompressed byte in the logical stream.
    pub d_offset: u64,
    /// Decompressed size in bytes.
    pub d_size: u32,
    /// Optional 32-bit checksum.
    pub checksum: Option<u32>,
}
