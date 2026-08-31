// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! XZ Block Header encoding, variable-length header parsing, filter flag chain,
//! and 4-byte alignment padding utilities.
//!
//! Complies strictly with Section 3.1 (Block Header) of the .xz File Format Specification.

use crate::crypto::crc32::crc32_fast;
use crate::xz::types::XzCheckType;
use crate::xz::vli::{decode_vli, encode_vli, vli_size, VliError, VLI_MAX_BYTES};

/// Filter IDs defined in the .xz format specification.
pub const FILTER_ID_DELTA: u64 = 0x03;
pub const FILTER_ID_X86: u64 = 0x04;
pub const FILTER_ID_POWERPC: u64 = 0x05;
pub const FILTER_ID_IA64: u64 = 0x06;
pub const FILTER_ID_ARM: u64 = 0x07;
pub const FILTER_ID_ARMTHUMB: u64 = 0x08;
pub const FILTER_ID_SPARC: u64 = 0x09;
pub const FILTER_ID_ARM64: u64 = 0x0A;
pub const FILTER_ID_RISCV: u64 = 0x0B;
pub const FILTER_ID_LZMA2: u64 = 0x21;

/// Minimum valid XZ Block Header size in bytes (Header Size (1) + Flags (1) + Filter (2) + CRC32 (4)).
pub const MIN_BLOCK_HEADER_SIZE: usize = 8;

/// Maximum valid XZ Block Header size in bytes ((0xFF + 1) * 4).
pub const MAX_BLOCK_HEADER_SIZE: usize = 1024;

/// Maximum number of filters supported in a single XZ Block Header.
pub const MAX_FILTER_COUNT: usize = 4;

/// Error types occurring during XZ Block Header serialization, parsing, or validation.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum XzBlockError {
    /// The input buffer ended prematurely while parsing the Block Header.
    #[error("Unexpected EOF while parsing XZ Block Header")]
    UnexpectedEof,

    /// The parsed Block Header size is invalid (must be 8..=1024 and divisible by 4).
    #[error("Invalid XZ Block Header size: {0} bytes (expected 8..=1024 in multiples of 4)")]
    InvalidHeaderSize(usize),

    /// Header CRC32 checksum mismatch indicating corrupted header data.
    #[error("Block Header CRC32 mismatch: expected 0x{expected:08X}, computed 0x{computed:08X}")]
    Crc32Mismatch {
        /// Expected CRC32 value read from the header.
        expected: u32,
        /// Computed CRC32 value over the header payload and padding.
        computed: u32,
    },

    /// Reserved bits (bits 2..5) in Block Flags are non-zero.
    #[error("Reserved bits in Block Flags are non-zero: 0x{0:02X}")]
    ReservedFlagsSet(u8),

    /// VLI integer parsing or encoding failed.
    #[error("VLI error: {0}")]
    InvalidVli(#[from] VliError),

    /// The filter properties payload was truncated.
    #[error("Filter properties payload truncated")]
    TruncatedFilterProperties,

    /// Non-zero byte detected in Header Padding (LZMA_OPTIONS_ERROR).
    #[error("Block Header padding contains non-zero byte (corrupted header options)")]
    NonZeroHeaderPadding,

    /// Filter count is outside the valid range [1, 4].
    #[error("Invalid filter count: {0} (expected 1..=4)")]
    InvalidFilterCount(usize),

    /// Invalid compressed size in Block Header (must be >= 1 and <= 2^63 - 1 - header_size - check_size).
    #[error("Invalid compressed size in Block Header: {0}")]
    InvalidCompressedSize(u64),

    /// Invalid uncompressed size in Block Header.
    #[error("Invalid uncompressed size in Block Header: {0}")]
    InvalidUncompressedSize(u64),

    /// Total encoded Block Header exceeds the 1024 bytes maximum limit.
    #[error("Block Header size {0} exceeds maximum allowed size of 1024 bytes")]
    HeaderTooLarge(usize),
}

/// Filter configuration containing the 64-bit Filter ID and arbitrary binary properties.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XzFilterConfig {
    /// Numerical filter identifier (e.g. 0x21 for LZMA2).
    pub filter_id: u64,
    /// Arbitrary binary filter properties.
    pub properties: Vec<u8>,
}

impl XzFilterConfig {
    /// Create a new generic filter configuration.
    #[inline]
    pub fn new(filter_id: u64, properties: Vec<u8>) -> Self {
        Self {
            filter_id,
            properties,
        }
    }

    /// Helper to construct an LZMA2 filter configuration with encoded dictionary size.
    pub fn lzma2(dict_size: u32) -> Self {
        let prop = encode_lzma2_dict_size(dict_size);
        Self {
            filter_id: FILTER_ID_LZMA2,
            properties: vec![prop],
        }
    }

    /// Helper to construct an x86 BCJ filter configuration.
    pub fn bcj_x86(start_offset: Option<u32>) -> Self {
        let properties = match start_offset {
            Some(offset) if offset != 0 => offset.to_le_bytes().to_vec(),
            _ => Vec::new(),
        };
        Self {
            filter_id: FILTER_ID_X86,
            properties,
        }
    }

    /// Helper to construct an ARM (32-bit) BCJ filter configuration.
    pub fn bcj_arm(start_offset: Option<u32>) -> Self {
        let properties = match start_offset {
            Some(offset) if offset != 0 => offset.to_le_bytes().to_vec(),
            _ => Vec::new(),
        };
        Self {
            filter_id: FILTER_ID_ARM,
            properties,
        }
    }

    /// Helper to construct an ARM64 (AArch64) BCJ filter configuration.
    pub fn bcj_arm64(start_offset: Option<u32>) -> Self {
        let properties = match start_offset {
            Some(offset) if offset != 0 => offset.to_le_bytes().to_vec(),
            _ => Vec::new(),
        };
        Self {
            filter_id: FILTER_ID_ARM64,
            properties,
        }
    }

    /// Helper to construct a RISC-V BCJ filter configuration.
    pub fn bcj_riscv(start_offset: Option<u32>) -> Self {
        let properties = match start_offset {
            Some(offset) if offset != 0 => offset.to_le_bytes().to_vec(),
            _ => Vec::new(),
        };
        Self {
            filter_id: FILTER_ID_RISCV,
            properties,
        }
    }

    /// Helper to construct a Delta filter configuration with distance property.
    pub fn delta(distance: u8) -> Self {
        Self {
            filter_id: FILTER_ID_DELTA,
            properties: vec![distance.saturating_sub(1)],
        }
    }
}

/// Compute 1-byte LZMA2 dictionary size property according to XZ specification.
fn encode_lzma2_dict_size(dict_size: u32) -> u8 {
    let dict_size = dict_size.max(4096);
    if dict_size >= 0x4000_0000 {
        return 39;
    }
    for i in 0..40u8 {
        let base = 2 | ((i as u32) & 1);
        let shift = ((i as u32) >> 1) + 11;
        let size = base << shift;
        if size >= dict_size {
            return i;
        }
    }
    39
}

/// Compute the number of 4-byte padding bytes required for `size` bytes.
///
/// Returns 0, 1, 2, or 3.
#[inline]
pub const fn pad_to_4(size: u64) -> usize {
    ((4 - (size & 3)) & 3) as usize
}

/// Compute the 4-byte aligned size for `unpadded_size`.
#[inline]
pub const fn aligned_block_size(unpadded_size: u64) -> u64 {
    (unpadded_size + 3) & !3
}

/// In-memory representation of an XZ Block Header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XzBlockHeader {
    /// Real header size in bytes ($[8, 1024]$, always a multiple of 4).
    pub header_size: usize,
    /// Optional compressed size (in bytes, excluding block padding and check).
    pub compressed_size: Option<u64>,
    /// Optional uncompressed size (in bytes).
    pub uncompressed_size: Option<u64>,
    /// Ordered list of filter configurations (1 to 4 filters).
    pub filters: Vec<XzFilterConfig>,
    /// Integrity check type associated with this block's stream.
    pub check_type: XzCheckType,
}

impl XzBlockHeader {
    /// Create a new `XzBlockHeader` with dynamically calculated minimum valid `header_size`.
    pub fn new(filters: Vec<XzFilterConfig>, check_type: XzCheckType) -> Result<Self, XzBlockError> {
        let mut header = Self {
            header_size: 0,
            compressed_size: None,
            uncompressed_size: None,
            filters,
            check_type,
        };
        header.header_size = header.calculate_minimum_header_size()?;
        Ok(header)
    }

    /// Attach optional compressed and uncompressed sizes to the header.
    pub fn with_sizes(
        mut self,
        compressed_size: Option<u64>,
        uncompressed_size: Option<u64>,
    ) -> Result<Self, XzBlockError> {
        self.compressed_size = compressed_size;
        self.uncompressed_size = uncompressed_size;
        self.header_size = self.calculate_minimum_header_size()?;
        Ok(self)
    }

    /// Calculate the minimum required header size in bytes for the current configuration.
    pub fn calculate_minimum_header_size(&self) -> Result<usize, XzBlockError> {
        if self.filters.is_empty() || self.filters.len() > MAX_FILTER_COUNT {
            return Err(XzBlockError::InvalidFilterCount(self.filters.len()));
        }

        // 1 byte (Header Size) + 1 byte (Block Flags)
        let mut raw_len = 2;

        if let Some(cs) = self.compressed_size {
            raw_len += vli_size(cs)?;
        }
        if let Some(us) = self.uncompressed_size {
            raw_len += vli_size(us)?;
        }

        for filter in &self.filters {
            raw_len += vli_size(filter.filter_id)?;
            raw_len += vli_size(filter.properties.len() as u64)?;
            raw_len += filter.properties.len();
        }

        // Add 4 bytes for Header CRC32
        raw_len += 4;

        // Align up to multiple of 4 bytes
        let mut padded_size = (raw_len + 3) & !3;
        if padded_size < MIN_BLOCK_HEADER_SIZE {
            padded_size = MIN_BLOCK_HEADER_SIZE;
        }
        if padded_size > MAX_BLOCK_HEADER_SIZE {
            return Err(XzBlockError::HeaderTooLarge(padded_size));
        }

        Ok(padded_size)
    }

    /// Calculate the unpadded block size: Block Header + Compressed Data + Check.
    #[inline]
    pub fn unpadded_size(&self) -> Option<u64> {
        self.compressed_size.map(|comp| {
            self.header_size as u64 + comp + self.check_type.check_size() as u64
        })
    }

    /// Calculate total block size in the stream: Block Header + Aligned Compressed Data + Check.
    #[inline]
    pub fn total_block_size(&self) -> Option<u64> {
        self.compressed_size.map(|comp| {
            self.header_size as u64 + aligned_block_size(comp) + self.check_type.check_size() as u64
        })
    }

    /// Compute the number of block padding bytes trailing the compressed payload.
    #[inline]
    pub fn block_padding_size(&self) -> Option<usize> {
        self.compressed_size.map(pad_to_4)
    }

    /// Return the check size in bytes for this block.
    #[inline]
    pub fn check_size(&self) -> usize {
        self.check_type.check_size()
    }

    /// Parse an `XzBlockHeader` from a raw byte slice.
    ///
    /// Validates the 1-byte Header Size, Block Flags reserved bits, VLI sizes,
    /// filter properties bounds, zeroed header padding, and CRC32 integrity.
    pub fn parse(input: &[u8], check_type: XzCheckType) -> Result<Self, XzBlockError> {
        if input.len() < MIN_BLOCK_HEADER_SIZE {
            return Err(XzBlockError::UnexpectedEof);
        }

        // 1. Read Header Size byte: real header size = (encoded + 1) * 4
        let encoded_header_size = input[0];
        let header_size = (encoded_header_size as usize + 1) * 4;

        if !(MIN_BLOCK_HEADER_SIZE..=MAX_BLOCK_HEADER_SIZE).contains(&header_size) {
            return Err(XzBlockError::InvalidHeaderSize(header_size));
        }

        if input.len() < header_size {
            return Err(XzBlockError::UnexpectedEof);
        }

        // 2. Validate Header CRC32
        let payload_len = header_size - 4;
        let expected_crc = u32::from_le_bytes(
            input[payload_len..header_size]
                .try_into()
                .map_err(|_| XzBlockError::UnexpectedEof)?,
        );
        let computed_crc = crc32_fast(0, &input[..payload_len]);

        if expected_crc != computed_crc {
            return Err(XzBlockError::Crc32Mismatch {
                expected: expected_crc,
                computed: computed_crc,
            });
        }

        // 3. Parse Block Flags (byte 1)
        let flags = input[1];
        if (flags & 0x3C) != 0 {
            return Err(XzBlockError::ReservedFlagsSet(flags));
        }

        let num_filters = ((flags & 0x03) as usize) + 1;
        let has_compressed_size = (flags & 0x40) != 0;
        let has_uncompressed_size = (flags & 0x80) != 0;

        let mut pos = 2;

        // 4. Parse Optional Compressed Size VLI
        let compressed_size = if has_compressed_size {
            let val = decode_vli(&input[..payload_len], &mut pos)?;
            if val == 0 {
                return Err(XzBlockError::InvalidCompressedSize(0));
            }
            let max_allowed = 0x7FFF_FFFF_FFFF_FFFFu64
                .saturating_sub(header_size as u64)
                .saturating_sub(check_type.check_size() as u64);
            if val > max_allowed {
                return Err(XzBlockError::InvalidCompressedSize(val));
            }
            Some(val)
        } else {
            None
        };

        // 5. Parse Optional Uncompressed Size VLI
        let uncompressed_size = if has_uncompressed_size {
            let val = decode_vli(&input[..payload_len], &mut pos)?;
            if val > 0x7FFF_FFFF_FFFF_FFFFu64 {
                return Err(XzBlockError::InvalidUncompressedSize(val));
            }
            Some(val)
        } else {
            None
        };

        // 6. Parse Filter Flags chain
        let mut filters = Vec::with_capacity(num_filters);
        for _ in 0..num_filters {
            let filter_id = decode_vli(&input[..payload_len], &mut pos)?;
            let props_len_vli = decode_vli(&input[..payload_len], &mut pos)?;

            let props_len = props_len_vli as usize;
            if pos + props_len > payload_len {
                return Err(XzBlockError::TruncatedFilterProperties);
            }

            let properties = input[pos..pos + props_len].to_vec();
            pos += props_len;

            filters.push(XzFilterConfig {
                filter_id,
                properties,
            });
        }

        // 7. Validate Header Padding bytes (all must be 0x00)
        let padding_slice = &input[pos..payload_len];
        if padding_slice.iter().any(|&b| b != 0x00) {
            return Err(XzBlockError::NonZeroHeaderPadding);
        }

        Ok(Self {
            header_size,
            compressed_size,
            uncompressed_size,
            filters,
            check_type,
        })
    }

    /// Encode this `XzBlockHeader` into a byte buffer including padding and CRC32.
    pub fn encode(&self) -> Result<Vec<u8>, XzBlockError> {
        let filter_count = self.filters.len();
        if filter_count == 0 || filter_count > MAX_FILTER_COUNT {
            return Err(XzBlockError::InvalidFilterCount(filter_count));
        }

        let target_size = if self.header_size >= MIN_BLOCK_HEADER_SIZE {
            self.header_size
        } else {
            self.calculate_minimum_header_size()?
        };

        if !(MIN_BLOCK_HEADER_SIZE..=MAX_BLOCK_HEADER_SIZE).contains(&target_size) || (target_size % 4) != 0 {
            return Err(XzBlockError::InvalidHeaderSize(target_size));
        }

        let mut buf = Vec::with_capacity(target_size);

        // Byte 0: Header Size (placeholder, written later)
        let encoded_size_byte = ((target_size / 4) - 1) as u8;
        buf.push(encoded_size_byte);

        // Byte 1: Block Flags
        let mut flags = (filter_count as u8) - 1;
        if self.compressed_size.is_some() {
            flags |= 0x40;
        }
        if self.uncompressed_size.is_some() {
            flags |= 0x80;
        }
        buf.push(flags);

        // Optional Compressed Size VLI
        if let Some(cs) = self.compressed_size {
            let mut vli_buf = [0u8; VLI_MAX_BYTES];
            let mut vpos = 0;
            let len = encode_vli(cs, &mut vli_buf, &mut vpos)?;
            buf.extend_from_slice(&vli_buf[..len]);
        }

        // Optional Uncompressed Size VLI
        if let Some(us) = self.uncompressed_size {
            let mut vli_buf = [0u8; VLI_MAX_BYTES];
            let mut vpos = 0;
            let len = encode_vli(us, &mut vli_buf, &mut vpos)?;
            buf.extend_from_slice(&vli_buf[..len]);
        }

        // Filter Flags chain
        for filter in &self.filters {
            let mut vli_buf = [0u8; VLI_MAX_BYTES];
            let mut id_pos = 0;
            let id_len = encode_vli(filter.filter_id, &mut vli_buf, &mut id_pos)?;
            buf.extend_from_slice(&vli_buf[..id_len]);

            let props_len = filter.properties.len() as u64;
            let mut len_pos = 0;
            let len_len = encode_vli(props_len, &mut vli_buf, &mut len_pos)?;
            buf.extend_from_slice(&vli_buf[..len_len]);

            buf.extend_from_slice(&filter.properties);
        }

        // Header Padding
        let payload_len = target_size - 4;
        if buf.len() > payload_len {
            return Err(XzBlockError::HeaderTooLarge(buf.len() + 4));
        }

        let padding_needed = payload_len - buf.len();
        buf.resize(payload_len, 0x00);

        // Write CRC32 over input[0..payload_len]
        let crc = crc32_fast(0, &buf);
        buf.extend_from_slice(&crc.to_le_bytes());

        debug_assert_eq!(buf.len(), target_size);
        let _ = padding_needed; // ensure padding was applied
        Ok(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_padding_and_alignment_math() {
        assert_eq!(pad_to_4(0), 0);
        assert_eq!(pad_to_4(1), 3);
        assert_eq!(pad_to_4(2), 2);
        assert_eq!(pad_to_4(3), 1);
        assert_eq!(pad_to_4(4), 0);
        assert_eq!(pad_to_4(1025), 3);

        assert_eq!(aligned_block_size(0), 0);
        assert_eq!(aligned_block_size(1), 4);
        assert_eq!(aligned_block_size(2), 4);
        assert_eq!(aligned_block_size(3), 4);
        assert_eq!(aligned_block_size(4), 4);
        assert_eq!(aligned_block_size(5), 8);
        assert_eq!(aligned_block_size(1024), 1024);
        assert_eq!(aligned_block_size(1025), 1028);
    }

    #[test]
    fn test_block_header_basic_roundtrip() {
        let filter = XzFilterConfig::lzma2(8 * 1024 * 1024);
        let header = XzBlockHeader::new(vec![filter], XzCheckType::Crc32)
            .expect("header creation")
            .with_sizes(Some(1234), Some(5678))
            .expect("with sizes");

        let encoded = header.encode().expect("encode");
        assert_eq!(encoded.len() % 4, 0);
        assert!(encoded.len() >= 8);

        let parsed = XzBlockHeader::parse(&encoded, XzCheckType::Crc32).expect("parse");
        assert_eq!(header.filters, parsed.filters);
        assert_eq!(header.compressed_size, parsed.compressed_size);
        assert_eq!(header.uncompressed_size, parsed.uncompressed_size);
        assert_eq!(header.header_size, parsed.header_size);
        assert_eq!(header.check_type, parsed.check_type);
    }
}
