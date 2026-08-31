// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! XZ Stream Header and Stream Footer parser, encoder, and flags comparison logic.

use crate::crypto::crc32::crc32_fast;
use crate::xz::types::{
    XzCheckType, XzError, XZ_BACKWARD_SIZE_UNIT, XZ_FOOTER_MAGIC, XZ_HEADER_MAGIC,
    XZ_MAX_BACKWARD_SIZE, XZ_MIN_BACKWARD_SIZE,
};

/// Strongly typed Stream Flags representing integrity check configuration and reserved bit validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct XzStreamFlags {
    /// The integrity check algorithm used by blocks in this stream.
    pub check_type: XzCheckType,
}

impl XzStreamFlags {
    /// Creates a new `XzStreamFlags` instance with the specified check type.
    #[inline]
    pub const fn new(check_type: XzCheckType) -> Self {
        Self { check_type }
    }

    /// Parses 2 raw flag bytes according to Section 2.1.1.2 of the XZ specification.
    ///
    /// # Errors
    /// Returns `XzError::ReservedFlagsNonZero` if the first byte is non-zero or if
    /// bits 4..=7 of the second byte are non-zero.
    /// Returns `XzError::UnsupportedCheckType` if bits 0..=3 do not map to a known check type.
    pub fn parse(bytes: [u8; 2]) -> Result<Self, XzError> {
        let byte0 = bytes[0];
        let byte1 = bytes[1];
        let reserved_bits = byte1 & 0xF0;

        if byte0 != 0x00 || reserved_bits != 0x00 {
            return Err(XzError::ReservedFlagsNonZero {
                byte0,
                reserved_bits,
            });
        }

        let check_id = byte1 & 0x0F;
        let check_type = XzCheckType::from_id(check_id)?;

        Ok(Self { check_type })
    }

    /// Encodes the stream flags into a 2-byte array.
    #[inline]
    pub const fn encode(&self) -> [u8; 2] {
        [0x00, self.check_type.id()]
    }
}

/// Compares Stream Header flags and Stream Footer flags for parity.
#[inline]
pub fn compare_stream_flags(header: &XzStreamFlags, footer: &XzStreamFlags) -> bool {
    header == footer
}

/// Strongly typed 12-byte XZ Stream Header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XzStreamHeader {
    /// Stream flags governing this stream.
    pub flags: XzStreamFlags,
}

impl XzStreamHeader {
    /// Creates a new `XzStreamHeader` with the given flags.
    #[inline]
    pub const fn new(flags: XzStreamFlags) -> Self {
        Self { flags }
    }

    /// Parses and validates a 12-byte XZ Stream Header slice.
    ///
    /// # Layout
    /// - `[0..6]`: Header Magic (`\xFD7zXZ\x00`)
    /// - `[6..8]`: Stream Flags (2 bytes)
    /// - `[8..12]`: CRC32 of Stream Flags (4 bytes, little-endian)
    pub fn parse(bytes: &[u8; 12]) -> Result<Self, XzError> {
        let magic: &[u8; 6] = bytes[0..6].try_into().unwrap();
        if magic != &XZ_HEADER_MAGIC {
            return Err(XzError::InvalidHeaderMagic {
                expected: XZ_HEADER_MAGIC,
                actual: *magic,
            });
        }

        let flags_bytes = [bytes[6], bytes[7]];
        let flags = XzStreamFlags::parse(flags_bytes)?;

        let expected_crc = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let actual_crc = crc32_fast(0, &flags_bytes);

        if expected_crc != actual_crc {
            return Err(XzError::HeaderCrcMismatch {
                expected: expected_crc,
                actual: actual_crc,
            });
        }

        Ok(Self { flags })
    }

    /// Encodes the 12-byte XZ Stream Header into a fixed-size byte array.
    pub fn encode(&self) -> [u8; 12] {
        let mut out = [0u8; 12];
        out[0..6].copy_from_slice(&XZ_HEADER_MAGIC);
        let flags_bytes = self.flags.encode();
        out[6..8].copy_from_slice(&flags_bytes);
        let crc = crc32_fast(0, &flags_bytes);
        out[8..12].copy_from_slice(&crc.to_le_bytes());
        out
    }
}

/// Strongly typed 12-byte XZ Stream Footer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XzStreamFooter {
    /// Stream flags governing this stream (must match Stream Header).
    pub flags: XzStreamFlags,
    /// Real Backward Size in bytes (unencoded, multiple of 4).
    pub backward_size: u64,
}

impl XzStreamFooter {
    /// Creates a new `XzStreamFooter`.
    #[inline]
    pub const fn new(flags: XzStreamFlags, backward_size: u64) -> Self {
        Self {
            flags,
            backward_size,
        }
    }

    /// Parses and validates a 12-byte XZ Stream Footer slice.
    ///
    /// # Layout
    /// - `[0..4]`: CRC32 of Backward Size + Stream Flags (4 bytes, little-endian)
    /// - `[4..8]`: Encoded Backward Size (4 bytes, little-endian, stored as `(real_size / 4) - 1`)
    /// - `[8..10]`: Stream Flags (2 bytes)
    /// - `[10..12]`: Footer Magic (`YZ`)
    pub fn parse(bytes: &[u8; 12]) -> Result<Self, XzError> {
        let magic: &[u8; 2] = bytes[10..12].try_into().unwrap();
        if magic != &XZ_FOOTER_MAGIC {
            return Err(XzError::InvalidFooterMagic {
                expected: XZ_FOOTER_MAGIC,
                actual: *magic,
            });
        }

        let expected_crc = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let actual_crc = crc32_fast(0, &bytes[4..10]);

        if expected_crc != actual_crc {
            return Err(XzError::FooterCrcMismatch {
                expected: expected_crc,
                actual: actual_crc,
            });
        }

        let stored_backward_size = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let backward_size = (stored_backward_size as u64 + 1) * XZ_BACKWARD_SIZE_UNIT;

        let flags_bytes = [bytes[8], bytes[9]];
        let flags = XzStreamFlags::parse(flags_bytes)?;

        Ok(Self {
            flags,
            backward_size,
        })
    }

    /// Parses and validates a 12-byte XZ Stream Footer, checking parity against the provided Stream Header flags.
    pub fn parse_and_verify_header(
        bytes: &[u8; 12],
        header_flags: &XzStreamFlags,
    ) -> Result<Self, XzError> {
        let footer = Self::parse(bytes)?;
        if !compare_stream_flags(header_flags, &footer.flags) {
            return Err(XzError::FlagsMismatch {
                header: *header_flags,
                footer: footer.flags,
            });
        }
        Ok(footer)
    }

    /// Encodes the 12-byte XZ Stream Footer with the specified `backward_size`.
    ///
    /// # Errors
    /// Returns `XzError::InvalidBackwardSize` if `backward_size` is < 4, > 17,179,869,184,
    /// or not an exact multiple of 4.
    pub fn encode(&self, backward_size: u64) -> Result<[u8; 12], XzError> {
        if !(XZ_MIN_BACKWARD_SIZE..=XZ_MAX_BACKWARD_SIZE).contains(&backward_size)
            || !backward_size.is_multiple_of(XZ_BACKWARD_SIZE_UNIT)
        {
            return Err(XzError::InvalidBackwardSize(backward_size));
        }

        let stored_backward_size = ((backward_size / XZ_BACKWARD_SIZE_UNIT) - 1) as u32;
        let flags_bytes = self.flags.encode();

        let mut crc_payload = [0u8; 6];
        crc_payload[0..4].copy_from_slice(&stored_backward_size.to_le_bytes());
        crc_payload[4..6].copy_from_slice(&flags_bytes);

        let crc = crc32_fast(0, &crc_payload);

        let mut out = [0u8; 12];
        out[0..4].copy_from_slice(&crc.to_le_bytes());
        out[4..8].copy_from_slice(&stored_backward_size.to_le_bytes());
        out[8..10].copy_from_slice(&flags_bytes);
        out[10..12].copy_from_slice(&XZ_FOOTER_MAGIC);

        Ok(out)
    }

    /// Encodes the 12-byte XZ Stream Footer using the struct's own `backward_size`.
    #[inline]
    pub fn encode_self(&self) -> Result<[u8; 12], XzError> {
        self.encode(self.backward_size)
    }

    /// Verifies whether the footer's stream flags match the header's stream flags.
    #[inline]
    pub fn verify_flags(&self, header_flags: &XzStreamFlags) -> Result<(), XzError> {
        if !compare_stream_flags(header_flags, &self.flags) {
            Err(XzError::FlagsMismatch {
                header: *header_flags,
                footer: self.flags,
            })
        } else {
            Ok(())
        }
    }
}
