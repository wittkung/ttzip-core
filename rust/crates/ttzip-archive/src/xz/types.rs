// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

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
#[derive(Debug, thiserror::Error)]
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
    Io(#[from] std::io::Error),
}

impl XzError {
    /// Returns a zero-allocation iterator over the causal chain of errors.
    #[inline]
    #[must_use]
    pub fn chain(&self) -> crate::types::ErrorChain<'_> {
        crate::types::ErrorChain::empty()
    }

    /// Downcasts by reference to a specific error type along the causal source chain.
    #[inline]
    #[must_use]
    pub fn downcast_ref<T: std::error::Error + 'static>(&self) -> Option<&T> {
        let mut current: Option<&(dyn std::error::Error + 'static)> = Some(self);
        while let Some(err) = current {
            if let Some(target) = err.downcast_ref::<T>() {
                return Some(target);
            }
            current = err.source();
        }
        None
    }

    /// Returns true if this error or any error in its causal chain matches the target type `T`.
    #[inline]
    #[must_use]
    pub fn is<T: std::error::Error + 'static>(&self) -> bool {
        self.downcast_ref::<T>().is_some()
    }

    /// Navigates the causal chain to locate the bottom-most root cause.
    #[inline]
    #[must_use]
    pub fn root_cause(&self) -> &(dyn std::error::Error + 'static) {
        let mut current: &(dyn std::error::Error + 'static) = self;
        while let Some(cause) = current.source() {
            current = cause;
        }
        current
    }

    /// Borrows the underlying standard I/O error if this failure was caused by I/O.
    #[inline]
    #[must_use]
    pub fn as_io_error(&self) -> Option<&std::io::Error> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }

    /// Mutably borrows the underlying standard I/O error if this failure was caused by I/O.
    #[inline]
    #[must_use]
    pub fn as_io_error_mut(&mut self) -> Option<&mut std::io::Error> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }

    /// Extracts the underlying standard I/O error by value if this failure was caused by I/O.
    #[inline]
    #[must_use]
    pub fn into_io_error(self) -> Option<std::io::Error> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }

    /// Returns true if this error was caused by an underlying I/O failure.
    #[inline]
    #[must_use]
    pub fn is_io(&self) -> bool {
        self.as_io_error().is_some()
    }
}

impl Clone for XzError {
    fn clone(&self) -> Self {
        match self {
            Self::InvalidHeaderMagic { expected, actual } => Self::InvalidHeaderMagic { expected: *expected, actual: *actual },
            Self::InvalidFooterMagic { expected, actual } => Self::InvalidFooterMagic { expected: *expected, actual: *actual },
            Self::HeaderCrcMismatch { expected, actual } => Self::HeaderCrcMismatch { expected: *expected, actual: *actual },
            Self::FooterCrcMismatch { expected, actual } => Self::FooterCrcMismatch { expected: *expected, actual: *actual },
            Self::ReservedFlagsNonZero { byte0, reserved_bits } => Self::ReservedFlagsNonZero { byte0: *byte0, reserved_bits: *reserved_bits },
            Self::UnsupportedCheckType(id) => Self::UnsupportedCheckType(*id),
            Self::InvalidBackwardSize(sz) => Self::InvalidBackwardSize(*sz),
            Self::FlagsMismatch { header, footer } => Self::FlagsMismatch { header: *header, footer: *footer },
            Self::TruncatedData { expected, actual } => Self::TruncatedData { expected: *expected, actual: *actual },
            Self::InvalidIndexIndicator(b) => Self::InvalidIndexIndicator(*b),
            Self::IndexCrcMismatch { expected, actual } => Self::IndexCrcMismatch { expected: *expected, actual: *actual },
            Self::NonZeroIndexPadding => Self::NonZeroIndexPadding,
            Self::IndexRecordCountMismatch { expected, actual } => Self::IndexRecordCountMismatch { expected: *expected, actual: *actual },
            Self::BackwardSizeMismatch { expected, actual } => Self::BackwardSizeMismatch { expected: *expected, actual: *actual },
            Self::SizeOverflow(s) => Self::SizeOverflow(s),
            Self::InvalidUnpaddedSize(sz) => Self::InvalidUnpaddedSize(*sz),
            Self::InvalidVli(e) => Self::InvalidVli(e.clone()),
            Self::BlockError(e) => Self::BlockError(e.clone()),
            Self::ChecksumError(e) => Self::ChecksumError(e.clone()),
            Self::DecompressError(s) => Self::DecompressError(s.clone()),
            Self::UnsupportedFilter(f) => Self::UnsupportedFilter(*f),
            Self::Io(err) => Self::Io(std::io::Error::new(err.kind(), err.to_string())),
        }
    }
}

impl PartialEq for XzError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::InvalidHeaderMagic { expected: e1, actual: a1 }, Self::InvalidHeaderMagic { expected: e2, actual: a2 }) => e1 == e2 && a1 == a2,
            (Self::InvalidFooterMagic { expected: e1, actual: a1 }, Self::InvalidFooterMagic { expected: e2, actual: a2 }) => e1 == e2 && a1 == a2,
            (Self::HeaderCrcMismatch { expected: e1, actual: a1 }, Self::HeaderCrcMismatch { expected: e2, actual: a2 }) => e1 == e2 && a1 == a2,
            (Self::FooterCrcMismatch { expected: e1, actual: a1 }, Self::FooterCrcMismatch { expected: e2, actual: a2 }) => e1 == e2 && a1 == a2,
            (Self::ReservedFlagsNonZero { byte0: b1, reserved_bits: r1 }, Self::ReservedFlagsNonZero { byte0: b2, reserved_bits: r2 }) => b1 == b2 && r1 == r2,
            (Self::UnsupportedCheckType(a), Self::UnsupportedCheckType(b)) => a == b,
            (Self::InvalidBackwardSize(a), Self::InvalidBackwardSize(b)) => a == b,
            (Self::FlagsMismatch { header: h1, footer: f1 }, Self::FlagsMismatch { header: h2, footer: f2 }) => h1 == h2 && f1 == f2,
            (Self::TruncatedData { expected: e1, actual: a1 }, Self::TruncatedData { expected: e2, actual: a2 }) => e1 == e2 && a1 == a2,
            (Self::InvalidIndexIndicator(a), Self::InvalidIndexIndicator(b)) => a == b,
            (Self::IndexCrcMismatch { expected: e1, actual: a1 }, Self::IndexCrcMismatch { expected: e2, actual: a2 }) => e1 == e2 && a1 == a2,
            (Self::NonZeroIndexPadding, Self::NonZeroIndexPadding) => true,
            (Self::IndexRecordCountMismatch { expected: e1, actual: a1 }, Self::IndexRecordCountMismatch { expected: e2, actual: a2 }) => e1 == e2 && a1 == a2,
            (Self::BackwardSizeMismatch { expected: e1, actual: a1 }, Self::BackwardSizeMismatch { expected: e2, actual: a2 }) => e1 == e2 && a1 == a2,
            (Self::SizeOverflow(a), Self::SizeOverflow(b)) => a == b,
            (Self::InvalidUnpaddedSize(a), Self::InvalidUnpaddedSize(b)) => a == b,
            (Self::InvalidVli(a), Self::InvalidVli(b)) => a == b,
            (Self::BlockError(a), Self::BlockError(b)) => a == b,
            (Self::ChecksumError(a), Self::ChecksumError(b)) => a == b,
            (Self::DecompressError(a), Self::DecompressError(b)) => a == b,
            (Self::UnsupportedFilter(a), Self::UnsupportedFilter(b)) => a == b,
            (Self::Io(a), Self::Io(b)) => a.kind() == b.kind() && a.to_string() == b.to_string(),
            _ => false,
        }
    }
}

impl Eq for XzError {}

impl From<XzError> for std::io::Error {
    fn from(err: XzError) -> Self {
        match err {
            XzError::Io(io_err) => io_err,
            err => std::io::Error::new(std::io::ErrorKind::InvalidData, err),
        }
    }
}

// Compile-time barrier ensuring XzError memory bounds and contracts
const _: () = {
    const fn assert_send_sync<T: Send + Sync + 'static>() {}
    const fn assert_clone<T: Clone>() {}
    const fn assert_unpin<T: Unpin>() {}
    const fn assert_eq_bounds<T: Eq>() {}
    const fn assert_from<T, U>()
    where
        U: From<T>,
    {
    }

    assert_send_sync::<XzError>();
    assert_clone::<XzError>();
    assert_unpin::<XzError>();
    assert_eq_bounds::<XzError>();

    assert_from::<std::io::Error, XzError>();
    assert_from::<XzError, std::io::Error>();

    assert!(std::mem::size_of::<XzError>() <= 64);
    assert!(std::mem::align_of::<XzError>() <= 8);
};


