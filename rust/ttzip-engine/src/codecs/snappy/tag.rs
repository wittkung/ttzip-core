// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Snappy bytecode Tag encoding, decoding, and zero-branch lookup tables.
//!
//! Implements the 4 core Snappy element types (Literal, Copy 1-byte, Copy 2-byte, Copy 4-byte)
//! alongside precomputed constant tables for branchless decompressor pipelines.

use super::error::SnappyError;

/// Strong enumeration of the 4 fundamental Snappy element bytecode tag types.
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum SnappyTagType {
    /// Uncompressed literal data directly embedded in the byte stream (bits `0b00`).
    Literal = 0b00,
    /// Copy backreference with 1-byte offset in trailer (bits `0b01`).
    Copy1Byte = 0b01,
    /// Copy backreference with 2-byte offset in trailer (bits `0b10`).
    Copy2Byte = 0b10,
    /// Copy backreference with 4-byte offset in trailer (bits `0b11`).
    Copy4Byte = 0b11,
}

impl SnappyTagType {
    /// Extracts the tag type from the lowest 2 bits of a tag opcode byte.
    #[inline]
    #[must_use]
    pub const fn from_tag_byte(tag: u8) -> Self {
        match tag & 0x03 {
            0b00 => Self::Literal,
            0b01 => Self::Copy1Byte,
            0b10 => Self::Copy2Byte,
            0b11 => Self::Copy4Byte,
            _ => unreachable!(),
        }
    }

    /// Returns the 2-bit tag identifier as a raw `u8` integer (`0b00`..=`0b11`).
    #[inline]
    #[must_use]
    pub const fn tag_bits(self) -> u8 {
        self as u8
    }
}

/// Parsed metadata header for a Snappy bitstream element.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SnappyTagHeader {
    /// Element category (Literal or Copy variant).
    pub tag_type: SnappyTagType,
    /// Length of uncompressed data represented or copied.
    pub length: u32,
    /// Backreference offset distance (0 for Literal).
    pub offset: u32,
    /// Total number of bytes consumed by the tag and extra length/offset bytes.
    pub header_len: usize,
}

/// Strongly-typed zero-copy representation of a parsed Snappy element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnappyElement<'a> {
    /// Uncompressed literal slice.
    Literal {
        /// Borrowed slice of raw uncompressed payload bytes.
        data: &'a [u8],
    },
    /// Backreference copy instruction.
    Copy {
        /// The specific copy variant format.
        tag_type: SnappyTagType,
        /// Relative backreference distance in bytes (must be > 0 and <= uncompressed history).
        offset: u32,
        /// Number of bytes to copy from historical decompressed buffer.
        length: u32,
    },
}

impl<'a> SnappyElement<'a> {
    /// Returns `true` if this element is an uncompressed literal.
    #[inline]
    #[must_use]
    pub const fn is_literal(&self) -> bool {
        matches!(self, Self::Literal { .. })
    }

    /// Returns `true` if this element is a backreference copy.
    #[inline]
    #[must_use]
    pub const fn is_copy(&self) -> bool {
        matches!(self, Self::Copy { .. })
    }

    /// Returns the uncompressed output length produced by this element.
    #[inline]
    #[must_use]
    pub fn length(&self) -> usize {
        match self {
            Self::Literal { data } => data.len(),
            Self::Copy { length, .. } => *length as usize,
        }
    }

    /// Returns the backreference offset distance if this is a copy element.
    #[inline]
    #[must_use]
    pub const fn offset(&self) -> Option<u32> {
        match self {
            Self::Literal { .. } => None,
            Self::Copy { offset, .. } => Some(*offset),
        }
    }
}

#[inline]
const fn make_entry(len: i16, offset: i16) -> i16 {
    len - (offset << 8)
}

#[inline]
const fn compute_length_minus_offset(tag: u8) -> i16 {
    let data = (tag >> 2) as i16;
    let tag_type = tag & 0x03;
    match tag_type {
        0b11 => 0x00FF,
        0b10 => make_entry(data + 1, 0),
        0b01 => make_entry((data & 7) + 4, data >> 3),
        0b00 => {
            if data < 60 {
                make_entry(data + 1, 1)
            } else {
                0x00FF
            }
        }
        _ => unreachable!(),
    }
}

/// Precomputed 256-entry lookup table mapping `tag -> (length - (tag_offset << 8))`.
///
/// Designed for branchless decompressor inner loops, folding length calculation,
/// tag offset extraction, and boundary verification into minimal CPU instructions.
/// Exactly matches Google Snappy `kLengthMinusOffset` specification.
pub const LENGTH_MINUS_OFFSET_TABLE: [i16; 256] = {
    let mut table = [0i16; 256];
    let mut i = 0;
    while i < 256 {
        table[i] = compute_length_minus_offset(i as u8);
        i += 1;
    }
    table
};

/// Encodes a Snappy Literal tag header for the specified payload length into `dst`.
///
/// Returns the number of header bytes written (1 to 5).
pub fn emit_literal_tag(len: usize, dst: &mut [u8]) -> Result<usize, SnappyError> {
    if len == 0 {
        return Err(SnappyError::InvalidParam(
            "Literal length must be at least 1 byte".to_string(),
        ));
    }

    if len <= 60 {
        if dst.is_empty() {
            return Err(SnappyError::BufferTooSmall {
                required: 1,
                available: 0,
            });
        }
        dst[0] = ((len - 1) as u8) << 2;
        Ok(1)
    } else if len <= 256 {
        if dst.len() < 2 {
            return Err(SnappyError::BufferTooSmall {
                required: 2,
                available: dst.len(),
            });
        }
        dst[0] = 60 << 2;
        dst[1] = (len - 1) as u8;
        Ok(2)
    } else if len <= 65536 {
        if dst.len() < 3 {
            return Err(SnappyError::BufferTooSmall {
                required: 3,
                available: dst.len(),
            });
        }
        dst[0] = 61 << 2;
        let v = ((len - 1) as u16).to_le_bytes();
        dst[1] = v[0];
        dst[2] = v[1];
        Ok(3)
    } else if len <= 16777216 {
        if dst.len() < 4 {
            return Err(SnappyError::BufferTooSmall {
                required: 4,
                available: dst.len(),
            });
        }
        dst[0] = 62 << 2;
        let v = (len - 1) as u32;
        dst[1] = v as u8;
        dst[2] = (v >> 8) as u8;
        dst[3] = (v >> 16) as u8;
        Ok(4)
    } else {
        if len > (u32::MAX as usize) + 1 {
            return Err(SnappyError::LiteralLengthExceeded {
                length: len,
                max: u32::MAX as usize,
            });
        }
        if dst.len() < 5 {
            return Err(SnappyError::BufferTooSmall {
                required: 5,
                available: dst.len(),
            });
        }
        dst[0] = 63 << 2;
        let v = ((len - 1) as u32).to_le_bytes();
        dst[1..5].copy_from_slice(&v);
        Ok(5)
    }
}

/// Encodes a Copy 1-Byte offset tag into `dst`.
///
/// Encodes lengths `[4..=11]` and offsets `[1..=2047]`. Consumes exactly 2 bytes in `dst`.
pub fn emit_copy1_tag(len: usize, offset: u32, dst: &mut [u8]) -> Result<usize, SnappyError> {
    if !(4..=11).contains(&len) {
        return Err(SnappyError::InvalidParam(format!(
            "Copy1 length must be within [4..11], got {len}"
        )));
    }
    if offset == 0 || offset > 2047 {
        return Err(SnappyError::InvalidOffset {
            offset,
            position: 0,
        });
    }
    if dst.len() < 2 {
        return Err(SnappyError::BufferTooSmall {
            required: 2,
            available: dst.len(),
        });
    }

    let len_bits = ((len - 4) as u8) << 2;
    let offset_hi = ((offset >> 8) as u8) << 5;
    dst[0] = SnappyTagType::Copy1Byte.tag_bits() | len_bits | offset_hi;
    dst[1] = (offset & 0xFF) as u8;
    Ok(2)
}

/// Encodes a Copy 2-Byte offset tag into `dst`.
///
/// Encodes lengths `[1..=64]` and offsets `[1..=65535]`. Consumes exactly 3 bytes in `dst`.
pub fn emit_copy2_tag(len: usize, offset: u32, dst: &mut [u8]) -> Result<usize, SnappyError> {
    if !(1..=64).contains(&len) {
        return Err(SnappyError::InvalidParam(format!(
            "Copy2 length must be within [1..64], got {len}"
        )));
    }
    if offset == 0 || offset > 65535 {
        return Err(SnappyError::InvalidOffset {
            offset,
            position: 0,
        });
    }
    if dst.len() < 3 {
        return Err(SnappyError::BufferTooSmall {
            required: 3,
            available: dst.len(),
        });
    }

    let len_bits = ((len - 1) as u8) << 2;
    dst[0] = SnappyTagType::Copy2Byte.tag_bits() | len_bits;
    let off_bytes = (offset as u16).to_le_bytes();
    dst[1] = off_bytes[0];
    dst[2] = off_bytes[1];
    Ok(3)
}

/// Encodes a Copy 4-Byte offset tag into `dst`.
///
/// Encodes lengths `[1..=64]` and 32-bit offsets `[1..=u32::MAX]`. Consumes exactly 5 bytes in `dst`.
pub fn emit_copy4_tag(len: usize, offset: u32, dst: &mut [u8]) -> Result<usize, SnappyError> {
    if !(1..=64).contains(&len) {
        return Err(SnappyError::InvalidParam(format!(
            "Copy4 length must be within [1..64], got {len}"
        )));
    }
    if offset == 0 {
        return Err(SnappyError::InvalidOffset {
            offset: 0,
            position: 0,
        });
    }
    if dst.len() < 5 {
        return Err(SnappyError::BufferTooSmall {
            required: 5,
            available: dst.len(),
        });
    }

    let len_bits = ((len - 1) as u8) << 2;
    dst[0] = SnappyTagType::Copy4Byte.tag_bits() | len_bits;
    let off_bytes = offset.to_le_bytes();
    dst[1..5].copy_from_slice(&off_bytes);
    Ok(5)
}

/// Parses the element tag header from `src`, returning the tag metadata and consumed header bytes.
pub fn parse_tag_header(src: &[u8]) -> Result<(SnappyTagHeader, usize), SnappyError> {
    if src.is_empty() {
        return Err(SnappyError::UnexpectedEof);
    }

    let tag = src[0];
    let tag_type = SnappyTagType::from_tag_byte(tag);

    match tag_type {
        SnappyTagType::Literal => {
            let len_tag = (tag >> 2) as usize;
            match len_tag {
                0..=59 => Ok((
                    SnappyTagHeader {
                        tag_type,
                        length: (len_tag + 1) as u32,
                        offset: 0,
                        header_len: 1,
                    },
                    1,
                )),
                60 => {
                    if src.len() < 2 {
                        return Err(SnappyError::UnexpectedEof);
                    }
                    let len = (src[1] as u32) + 1;
                    Ok((
                        SnappyTagHeader {
                            tag_type,
                            length: len,
                            offset: 0,
                            header_len: 2,
                        },
                        2,
                    ))
                }
                61 => {
                    if src.len() < 3 {
                        return Err(SnappyError::UnexpectedEof);
                    }
                    let len = (u16::from_le_bytes([src[1], src[2]]) as u32) + 1;
                    Ok((
                        SnappyTagHeader {
                            tag_type,
                            length: len,
                            offset: 0,
                            header_len: 3,
                        },
                        3,
                    ))
                }
                62 => {
                    if src.len() < 4 {
                        return Err(SnappyError::UnexpectedEof);
                    }
                    let len = (src[1] as u32
                        | ((src[2] as u32) << 8)
                        | ((src[3] as u32) << 16))
                        + 1;
                    Ok((
                        SnappyTagHeader {
                            tag_type,
                            length: len,
                            offset: 0,
                            header_len: 4,
                        },
                        4,
                    ))
                }
                63 => {
                    if src.len() < 5 {
                        return Err(SnappyError::UnexpectedEof);
                    }
                    let raw_len = u32::from_le_bytes([src[1], src[2], src[3], src[4]]);
                    let len = raw_len.checked_add(1).ok_or(SnappyError::LiteralLengthExceeded {
                        length: u32::MAX as usize,
                        max: u32::MAX as usize,
                    })?;
                    Ok((
                        SnappyTagHeader {
                            tag_type,
                            length: len,
                            offset: 0,
                            header_len: 5,
                        },
                        5,
                    ))
                }
                _ => unreachable!(),
            }
        }
        SnappyTagType::Copy1Byte => {
            if src.len() < 2 {
                return Err(SnappyError::UnexpectedEof);
            }
            let length = (((tag >> 2) & 0x07) as u32) + 4;
            let offset_hi = ((tag >> 5) as u32) << 8;
            let offset_lo = src[1] as u32;
            let offset = offset_hi | offset_lo;
            if offset == 0 {
                return Err(SnappyError::InvalidOffset {
                    offset: 0,
                    position: 0,
                });
            }
            Ok((
                SnappyTagHeader {
                    tag_type,
                    length,
                    offset,
                    header_len: 2,
                },
                2,
            ))
        }
        SnappyTagType::Copy2Byte => {
            if src.len() < 3 {
                return Err(SnappyError::UnexpectedEof);
            }
            let length = ((tag >> 2) as u32) + 1;
            let offset = u16::from_le_bytes([src[1], src[2]]) as u32;
            if offset == 0 {
                return Err(SnappyError::InvalidOffset {
                    offset: 0,
                    position: 0,
                });
            }
            Ok((
                SnappyTagHeader {
                    tag_type,
                    length,
                    offset,
                    header_len: 3,
                },
                3,
            ))
        }
        SnappyTagType::Copy4Byte => {
            if src.len() < 5 {
                return Err(SnappyError::UnexpectedEof);
            }
            let length = ((tag >> 2) as u32) + 1;
            let offset = u32::from_le_bytes([src[1], src[2], src[3], src[4]]);
            if offset == 0 {
                return Err(SnappyError::InvalidOffset {
                    offset: 0,
                    position: 0,
                });
            }
            Ok((
                SnappyTagHeader {
                    tag_type,
                    length,
                    offset,
                    header_len: 5,
                },
                5,
            ))
        }
    }
}

/// Parses a full `SnappyElement` from `src`, including literal payload slices when applicable.
///
/// Returns `Ok((element, total_bytes_consumed))` on success.
pub fn parse_element<'a>(src: &'a [u8]) -> Result<(SnappyElement<'a>, usize), SnappyError> {
    let (header, header_len) = parse_tag_header(src)?;
    match header.tag_type {
        SnappyTagType::Literal => {
            let lit_len = header.length as usize;
            let total_len = header_len + lit_len;
            if src.len() < total_len {
                return Err(SnappyError::UnexpectedEof);
            }
            let literal_slice = &src[header_len..total_len];
            Ok((
                SnappyElement::Literal {
                    data: literal_slice,
                },
                total_len,
            ))
        }
        tag_type => Ok((
            SnappyElement::Copy {
                tag_type,
                offset: header.offset,
                length: header.length,
            },
            header_len,
        )),
    }
}
