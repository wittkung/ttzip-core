// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! XZ Stream Header and Stream Footer fixed-length structural geometry, field layout constants, and strong types.

/// Size of the XZ Stream Header in bytes (12 bytes).
pub const XZ_STREAM_HEADER_SIZE: usize = 12;

/// Size of the XZ Stream Footer in bytes (12 bytes).
pub const XZ_STREAM_FOOTER_SIZE: usize = 12;

/// Magic bytes prefix for the XZ Stream Header (`\xFD7zXZ\x00`).
pub const XZ_HEADER_MAGIC: [u8; 6] = [0xFD, b'7', b'z', b'X', b'Z', 0x00];

/// Magic bytes suffix for the XZ Stream Footer (`YZ`).
pub const XZ_FOOTER_MAGIC: [u8; 2] = *b"YZ";

/// Field offsets in a standard 12-byte XZ Stream Header.
pub const OFFSET_HEADER_MAGIC: usize = 0;
pub const LEN_HEADER_MAGIC: usize = 6;
pub const OFFSET_HEADER_FLAGS: usize = 6;
pub const LEN_HEADER_FLAGS: usize = 2;
pub const OFFSET_HEADER_CRC: usize = 8;
pub const LEN_HEADER_CRC: usize = 4;

/// Field offsets in a standard 12-byte XZ Stream Footer.
pub const OFFSET_FOOTER_CRC: usize = 0;
pub const LEN_FOOTER_CRC: usize = 4;
pub const OFFSET_FOOTER_BACKWARD_SIZE: usize = 4;
pub const LEN_FOOTER_BACKWARD_SIZE: usize = 4;
pub const OFFSET_FOOTER_FLAGS: usize = 8;
pub const LEN_FOOTER_FLAGS: usize = 2;
pub const OFFSET_FOOTER_MAGIC: usize = 10;
pub const LEN_FOOTER_MAGIC: usize = 2;

/// Minimum valid real backward size in bytes (4 bytes).
pub const XZ_MIN_BACKWARD_SIZE: u64 = 4;

/// Maximum valid real backward size in bytes (16 GiB = 2^34 bytes).
pub const XZ_MAX_BACKWARD_SIZE: u64 = (u32::MAX as u64 + 1) * 4;

/// Backward size quantum unit (4 bytes).
pub const XZ_BACKWARD_SIZE_UNIT: u64 = 4;

/// Raw 12-byte C-compatible memory layout of an XZ Stream Header.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XzRawStreamHeader {
    pub magic: [u8; 6],
    pub flags: [u8; 2],
    pub crc32: [u8; 4],
}

/// Raw 12-byte C-compatible memory layout of an XZ Stream Footer.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XzRawStreamFooter {
    pub crc32: [u8; 4],
    pub backward_size: [u8; 4],
    pub flags: [u8; 2],
    pub magic: [u8; 2],
}

/// Strong enumeration of supported and standardized XZ integrity check types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum XzCheckType {
    /// No integrity check (0 bytes).
    None = 0x00,
    /// 32-bit CRC (4 bytes).
    Crc32 = 0x01,
    /// 64-bit CRC (8 bytes).
    Crc64 = 0x04,
    /// SHA-256 (32 bytes).
    Sha256 = 0x0A,
}

impl Default for XzCheckType {
    #[inline]
    fn default() -> Self {
        Self::Crc32
    }
}

impl XzCheckType {
    /// Returns the byte size of the check value calculated across decoded uncompressed data.
    #[inline]
    pub const fn check_size(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Crc32 => 4,
            Self::Crc64 => 8,
            Self::Sha256 => 32,
        }
    }

    /// Converts a raw 4-bit check type ID into a typed `XzCheckType`.
    #[inline]
    pub fn from_id(id: u8) -> Result<Self, XzError> {
        match id {
            0x00 => Ok(Self::None),
            0x01 => Ok(Self::Crc32),
            0x04 => Ok(Self::Crc64),
            0x0A => Ok(Self::Sha256),
            other => Err(XzError::UnsupportedCheckType(other)),
        }
    }

    /// Returns the raw 4-bit ID corresponding to this check type.
    #[inline]
    pub const fn id(&self) -> u8 {
        *self as u8
    }
}

/// Strong-typed error variants for XZ stream header and footer decoding and validation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum XzError {
    /// The stream header magic bytes do not match `XZ_HEADER_MAGIC`.
    #[error("Invalid XZ stream header magic: expected {expected:?}, found {actual:?}")]
    InvalidHeaderMagic {
        expected: [u8; 6],
        actual: [u8; 6],
    },

    /// The stream footer magic bytes do not match `XZ_FOOTER_MAGIC`.
    #[error("Invalid XZ stream footer magic: expected {expected:?}, found {actual:?}")]
    InvalidFooterMagic {
        expected: [u8; 2],
        actual: [u8; 2],
    },

    /// Stream Header CRC32 checksum mismatch.
    #[error("Stream header CRC32 mismatch: expected 0x{expected:08X}, computed 0x{actual:08X}")]
    HeaderCrcMismatch {
        expected: u32,
        actual: u32,
    },

    /// Stream Footer CRC32 checksum mismatch.
    #[error("Stream footer CRC32 mismatch: expected 0x{expected:08X}, computed 0x{actual:08X}")]
    FooterCrcMismatch {
        expected: u32,
        actual: u32,
    },

    /// Stream flags contain non-zero bits in reserved positions.
    #[error("Reserved stream flag bits are non-zero: byte0=0x{byte0:02X}, reserved_bits=0x{reserved_bits:02X}")]
    ReservedFlagsNonZero {
        byte0: u8,
        reserved_bits: u8,
    },

    /// An unsupported or unrecognized check type ID was encountered.
    #[error("Unsupported check type ID: 0x{0:02X}")]
    UnsupportedCheckType(u8),

    /// Invalid backward size (must be >= 4, <= 17,179,869,184, and a multiple of 4).
    #[error("Invalid backward size {0}: must be >= 4, <= 17179869184, and a multiple of 4")]
    InvalidBackwardSize(u64),

    /// Stream flags in Stream Footer do not match Stream Header flags.
    #[error("Stream flags mismatch between header ({header:?}) and footer ({footer:?})")]
    FlagsMismatch {
        header: crate::xz::header::XzStreamFlags,
        footer: crate::xz::header::XzStreamFlags,
    },

    /// Stream buffer truncated before fixed header/footer boundary.
    #[error("Truncated stream: expected at least {expected} bytes, found {actual}")]
    TruncatedData {
        expected: usize,
        actual: usize,
    },

    /// Index Indicator byte is not 0x00.
    #[error("Invalid XZ Index indicator: expected 0x00, found 0x{0:02X}")]
    InvalidIndexIndicator(u8),

    /// Index CRC32 checksum mismatch.
    #[error("Index CRC32 mismatch: expected 0x{expected:08X}, computed 0x{actual:08X}")]
    IndexCrcMismatch {
        expected: u32,
        actual: u32,
    },

    /// Index padding contains non-zero byte.
    #[error("Index padding contains non-zero byte")]
    NonZeroIndexPadding,

    /// Index record count mismatch or truncated records.
    #[error("Index record count mismatch: expected {expected} records, found {actual}")]
    IndexRecordCountMismatch {
        expected: usize,
        actual: usize,
    },

    /// Index backward size in footer does not match real parsed index size.
    #[error("Backward size mismatch: footer specified {expected} bytes, but Index is {actual} bytes")]
    BackwardSizeMismatch {
        expected: u64,
        actual: u64,
    },

    /// Arithmetic overflow occurred while calculating total sizes or prefix sums.
    #[error("XZ Index size overflow: {0}")]
    SizeOverflow(&'static str),

    /// Unpadded size is invalid (must be non-zero and <= VLI_MAX).
    #[error("Invalid unpadded size: {0}")]
    InvalidUnpaddedSize(u64),

    /// VLI integer parsing or encoding failed.
    #[error("VLI error: {0}")]
    InvalidVli(#[from] crate::xz::vli::XzVliError),

    /// XZ Block parsing or encoding failed.
    #[error("Block error: {0}")]
    BlockError(#[from] crate::xz::block::XzBlockError),

    /// XZ Checksum verification failed.
    #[error("Checksum error: {0}")]
    ChecksumError(#[from] crate::xz::checksum::XzChecksumError),

    /// Decompression failed.
    #[error("Decompression failed: {0}")]
    DecompressError(String),

    /// Unsupported filter ID encountered in block header.
    #[error("Unsupported filter ID: 0x{0:02X}")]
    UnsupportedFilter(u64),

    /// I/O error occurred while reading or seeking stream.
    #[error("I/O error: {0}")]
    Io(String),
}

impl From<std::io::Error> for XzError {
    #[inline]
    fn from(err: std::io::Error) -> Self {
        XzError::Io(err.to_string())
    }
}


