// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Standard LZ4 Frame format constants, magic tables, bitfield descriptors, and checksum routines.
//!
//! Conforms strictly to the official LZ4 Framing Format specification (v1.6.2+):
//! - Magic numbers (Standard Frame, Skippable Frames, Legacy format)
//! - Frame Descriptor (FLG, BD, optional ContentSize & DictID, Header Checksum)
//! - Bitfield serializing and parsing with zero-allocation defensive validation

use crate::checksum::xxh32;
use crate::types::TTZipStatus;

// MARK: - Magic Number Constants

/// Standard LZ4 Frame magic number (0x184D2204, Little-Endian: `04 22 4D 18`).
pub const LZ4F_MAGICNUMBER: u32 = 0x184D_2204;

/// Lower bound of LZ4 skippable frame magic range (0x184D2A50).
pub const LZ4F_MAGIC_SKIPPABLE_START: u32 = 0x184D_2A50;

/// Upper bound of LZ4 skippable frame magic range (0x184D2A5F).
pub const LZ4F_MAGIC_SKIPPABLE_END: u32 = 0x184D_2A5F;

/// Bitmask for identifying LZ4 skippable frames (0xFFFFFFF0).
pub const LZ4F_MAGIC_SKIPPABLE_MASK: u32 = 0xFFFF_FFF0;

/// Legacy LZ4 format magic number (0x184C2102, Little-Endian: `02 21 4C 18`).
pub const LZ4F_MAGIC_LEGACY: u32 = 0x184C_2102;

/// Frame version supported by the LZ4 Framing specification (Version 1).
pub const LZ4F_VERSION_1: u8 = 1;

// MARK: - Magic Identification Helpers

/// Checks whether the provided 32-bit integer matches the standard LZ4 Frame magic number.
#[inline(always)]
pub const fn is_lz4_frame_magic(magic: u32) -> bool {
    magic == LZ4F_MAGICNUMBER
}

/// Checks whether the provided 32-bit integer falls within the 16 skippable frame magic values (0x184D2A50..=0x184D2A5F).
#[inline(always)]
pub const fn is_lz4_skippable_magic(magic: u32) -> bool {
    (magic & LZ4F_MAGIC_SKIPPABLE_MASK) == LZ4F_MAGIC_SKIPPABLE_START
}

/// Checks whether the provided 32-bit integer matches the legacy LZ4 format magic number.
#[inline(always)]
pub const fn is_lz4_legacy_magic(magic: u32) -> bool {
    magic == LZ4F_MAGIC_LEGACY
}

// MARK: - Block Max Size

/// Maximum uncompressed size of individual data blocks within an LZ4 frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum BlockMaxSize {
    /// 64 KB maximum block size (id = 4).
    #[default]
    Max64KB = 4,
    /// 256 KB maximum block size (id = 5).
    Max256KB = 5,
    /// 1 MB maximum block size (id = 6).
    Max1MB = 6,
    /// 4 MB maximum block size (id = 7).
    Max4MB = 7,
}

impl BlockMaxSize {
    /// Parses a 3-bit block maximum size ID from the BD byte (values 4..=7).
    pub const fn from_id(id: u8) -> Result<Self, TTZipStatus> {
        match id {
            4 => Ok(Self::Max64KB),
            5 => Ok(Self::Max256KB),
            6 => Ok(Self::Max1MB),
            7 => Ok(Self::Max4MB),
            _ => Err(TTZipStatus::ErrCorruptHeader),
        }
    }

    /// Returns the raw numeric 3-bit ID (4..=7).
    #[inline(always)]
    pub const fn to_id(self) -> u8 {
        self as u8
    }

    /// Returns the maximum block size in uncompressed bytes.
    #[inline(always)]
    pub const fn max_bytes(self) -> usize {
        match self {
            Self::Max64KB => 64 * 1024,
            Self::Max256KB => 256 * 1024,
            Self::Max1MB => 1024 * 1024,
            Self::Max4MB => 4 * 1024 * 1024,
        }
    }
}

// MARK: - Block Independence

/// Block dependency mode within an LZ4 frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum BlockIndependence {
    /// Linked blocks: blocks depend on data from previous blocks (up to 64KB history window).
    Linked = 0,
    /// Independent blocks: each block can be decompressed independently in parallel.
    #[default]
    Independent = 1,
}

impl BlockIndependence {
    /// Converts a boolean flag (`true` for independent, `false` for linked) to `BlockIndependence`.
    #[inline(always)]
    pub const fn from_flag(flag: bool) -> Self {
        if flag {
            Self::Independent
        } else {
            Self::Linked
        }
    }

    /// Returns `true` if blocks are independent.
    #[inline(always)]
    pub const fn is_independent(self) -> bool {
        matches!(self, Self::Independent)
    }
}

// MARK: - Frame Descriptor

/// Strongly-typed representation of an LZ4 Frame Descriptor header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameDescriptor {
    /// Frame format version (must be 1).
    pub version: u8,
    /// Block independence mode (Linked vs Independent).
    pub block_independence: BlockIndependence,
    /// Indicates whether each data block is followed by a 4-byte XXH32 block checksum.
    pub block_checksum: bool,
    /// Indicates whether the entire frame is followed by a 4-byte XXH32 content checksum.
    pub content_checksum: bool,
    /// Optional uncompressed content size in bytes (8-byte Little-Endian u64).
    pub content_size: Option<u64>,
    /// Optional dictionary ID (4-byte Little-Endian u32).
    pub dict_id: Option<u32>,
    /// Maximum uncompressed size for data blocks.
    pub block_max_size: BlockMaxSize,
}

impl Default for FrameDescriptor {
    fn default() -> Self {
        Self {
            version: LZ4F_VERSION_1,
            block_independence: BlockIndependence::Independent,
            block_checksum: false,
            content_checksum: false,
            content_size: None,
            dict_id: None,
            block_max_size: BlockMaxSize::Max64KB,
        }
    }
}

// MARK: - Header Checksum Calculation

/// Computes the 1-byte LZ4 Frame Header Checksum (HC) over the descriptor byte sequence.
///
/// According to the LZ4 Frame spec:
/// `HC = ((xxh32(desc, len, 0) >> 8) & 0xFF) as u8`
#[inline(always)]
pub fn header_checksum(descriptor_bytes: &[u8]) -> u8 {
    ((xxh32(descriptor_bytes, 0) >> 8) & 0xFF) as u8
}

// MARK: - Descriptor Parsing and Emission

impl FrameDescriptor {
    /// Minimum header descriptor size in bytes (FLG + BD + HC = 3 bytes).
    pub const MIN_HEADER_SIZE: usize = 3;

    /// Maximum header descriptor size in bytes (FLG + BD + 8-byte ContentSize + 4-byte DictID + HC = 15 bytes).
    pub const MAX_HEADER_SIZE: usize = 15;

    /// Computes the raw descriptor payload length (excluding Header Checksum byte).
    #[inline]
    pub const fn descriptor_payload_len(&self) -> usize {
        let mut len = 2; // FLG + BD
        if self.content_size.is_some() {
            len += 8;
        }
        if self.dict_id.is_some() {
            len += 4;
        }
        len
    }

    /// Computes total header size in bytes including the 1-byte Header Checksum (HC).
    #[inline]
    pub const fn total_header_size(&self) -> usize {
        self.descriptor_payload_len() + 1
    }

    /// Parses a `FrameDescriptor` from a byte slice starting at the `FLG` byte.
    ///
    /// Returns `Ok((FrameDescriptor, bytes_consumed))` on success, or an explicit `TTZipStatus` error
    /// if the header is corrupt, truncated, or contains illegal reserved bits / versions.
    pub fn parse(bytes: &[u8]) -> Result<(Self, usize), TTZipStatus> {
        if bytes.len() < Self::MIN_HEADER_SIZE {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let flg = bytes[0];
        let bd = bytes[1];

        // 1. FLG Bitfield Validation
        let version = (flg >> 6) & 0x03;
        if version != LZ4F_VERSION_1 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let b_indep_flag = ((flg >> 5) & 0x01) != 0;
        let block_checksum = ((flg >> 4) & 0x01) != 0;
        let has_content_size = ((flg >> 3) & 0x01) != 0;
        let content_checksum = ((flg >> 2) & 0x01) != 0;
        let flg_reserved = ((flg >> 1) & 0x01) != 0;
        let has_dict_id = (flg & 0x01) != 0;

        // Reserved bit 1 of FLG must be 0
        if flg_reserved {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        // 2. BD Bitfield Validation
        let bd_reserved_high = (bd & 0x80) != 0;
        let bd_reserved_low = (bd & 0x0F) != 0;
        if bd_reserved_high || bd_reserved_low {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let block_size_id = (bd >> 4) & 0x07;
        let block_max_size = BlockMaxSize::from_id(block_size_id)?;

        // 3. Compute expected descriptor length
        let mut expected_payload_len = 2usize; // FLG + BD
        if has_content_size {
            expected_payload_len += 8;
        }
        if has_dict_id {
            expected_payload_len += 4;
        }

        let total_required_len = expected_payload_len + 1; // + 1 for HC
        if bytes.len() < total_required_len {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        // 4. Verify Header Checksum (HC)
        let desc_payload = &bytes[..expected_payload_len];
        let expected_hc = bytes[expected_payload_len];
        let actual_hc = header_checksum(desc_payload);
        if expected_hc != actual_hc {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        // 5. Parse optional fields
        let mut cursor = 2usize;
        let content_size = if has_content_size {
            let cs_bytes: [u8; 8] = match bytes[cursor..cursor + 8].try_into() {
                Ok(b) => b,
                Err(_) => return Err(TTZipStatus::ErrCorruptHeader),
            };
            cursor += 8;
            Some(u64::from_le_bytes(cs_bytes))
        } else {
            None
        };

        let dict_id = if has_dict_id {
            let dict_bytes: [u8; 4] = match bytes[cursor..cursor + 4].try_into() {
                Ok(b) => b,
                Err(_) => return Err(TTZipStatus::ErrCorruptHeader),
            };
            Some(u32::from_le_bytes(dict_bytes))
        } else {
            None
        };

        let descriptor = Self {
            version,
            block_independence: BlockIndependence::from_flag(b_indep_flag),
            block_checksum,
            content_checksum,
            content_size,
            dict_id,
            block_max_size,
        };

        Ok((descriptor, total_required_len))
    }

    /// Parses a `FrameDescriptor` that is prefixed with the 4-byte `LZ4F_MAGICNUMBER`.
    pub fn parse_with_magic(bytes: &[u8]) -> Result<(Self, usize), TTZipStatus> {
        if bytes.len() < 4 + Self::MIN_HEADER_SIZE {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let magic_bytes: [u8; 4] = match bytes[..4].try_into() {
            Ok(b) => b,
            Err(_) => return Err(TTZipStatus::ErrCorruptHeader),
        };
        let magic = u32::from_le_bytes(magic_bytes);
        if !is_lz4_frame_magic(magic) {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let (desc, consumed) = Self::parse(&bytes[4..])?;
        Ok((desc, 4 + consumed))
    }

    /// Emits the descriptor header bytes (FLG, BD, optional fields, and HC) into `dst`.
    ///
    /// Returns the total number of header bytes written.
    pub fn emit(&self, dst: &mut [u8]) -> Result<usize, TTZipStatus> {
        if self.version != LZ4F_VERSION_1 {
            return Err(TTZipStatus::ErrInvalidParam);
        }

        let total_size = self.total_header_size();
        if dst.len() < total_size {
            return Err(TTZipStatus::ErrInvalidParam);
        }

        // Encode FLG
        let mut flg = (LZ4F_VERSION_1 & 0x03) << 6;
        if self.block_independence.is_independent() {
            flg |= 0x20;
        }
        if self.block_checksum {
            flg |= 0x10;
        }
        if self.content_size.is_some() {
            flg |= 0x08;
        }
        if self.content_checksum {
            flg |= 0x04;
        }
        if self.dict_id.is_some() {
            flg |= 0x01;
        }

        // Encode BD
        let bd = (self.block_max_size.to_id() & 0x07) << 4;

        dst[0] = flg;
        dst[1] = bd;

        let mut offset = 2usize;
        if let Some(cs) = self.content_size {
            dst[offset..offset + 8].copy_from_slice(&cs.to_le_bytes());
            offset += 8;
        }
        if let Some(dict) = self.dict_id {
            dst[offset..offset + 4].copy_from_slice(&dict.to_le_bytes());
            offset += 4;
        }

        let hc = header_checksum(&dst[..offset]);
        dst[offset] = hc;
        offset += 1;

        Ok(offset)
    }

    /// Emits the 4-byte magic number followed by descriptor header bytes into `dst`.
    pub fn emit_with_magic(&self, dst: &mut [u8]) -> Result<usize, TTZipStatus> {
        let total_size = 4 + self.total_header_size();
        if dst.len() < total_size {
            return Err(TTZipStatus::ErrInvalidParam);
        }

        dst[..4].copy_from_slice(&LZ4F_MAGICNUMBER.to_le_bytes());
        let desc_len = self.emit(&mut dst[4..])?;
        Ok(4 + desc_len)
    }

    /// Emits header bytes into a newly allocated `Vec<u8>`.
    pub fn emit_to_vec(&self, include_magic: bool) -> Result<Vec<u8>, TTZipStatus> {
        let size = if include_magic {
            4 + self.total_header_size()
        } else {
            self.total_header_size()
        };

        let mut buf = vec![0u8; size];
        if include_magic {
            self.emit_with_magic(&mut buf)?;
        } else {
            self.emit(&mut buf)?;
        }
        Ok(buf)
    }
}

// MARK: - Free Function Wrappers

/// Parses an LZ4 Frame descriptor from a byte slice starting at `FLG`.
#[inline]
pub fn parse_header(bytes: &[u8]) -> Result<(FrameDescriptor, usize), TTZipStatus> {
    FrameDescriptor::parse(bytes)
}

/// Parses an LZ4 Frame descriptor prefixed with the 4-byte `LZ4F_MAGICNUMBER`.
#[inline]
pub fn parse_frame_header(bytes: &[u8]) -> Result<(FrameDescriptor, usize), TTZipStatus> {
    FrameDescriptor::parse_with_magic(bytes)
}

/// Emits an LZ4 Frame descriptor header into a destination buffer.
#[inline]
pub fn emit_header(descriptor: &FrameDescriptor, dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    descriptor.emit(dst)
}
