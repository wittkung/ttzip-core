// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe Pure-Rust implementation of RFC 7932 Brotli 121 Static Transforms and UTF-8 Word Restructurer.
//!
//! Provides zero-copy, zero-allocation transformations of Brotli static dictionary words
//! with complete boundary safety, zero panic, and strict RFC 7932 parity.

pub use super::error::BrotliError;

/// Word transformation operations defined in RFC 7932 Section 8.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum WordTransformOp {
    /// Identity: leaves the word unchanged.
    Identity = 0,
    /// Omit last 1 byte.
    OmitLast1 = 1,
    /// Omit last 2 bytes.
    OmitLast2 = 2,
    /// Omit last 3 bytes.
    OmitLast3 = 3,
    /// Omit last 4 bytes.
    OmitLast4 = 4,
    /// Omit last 5 bytes.
    OmitLast5 = 5,
    /// Omit last 6 bytes.
    OmitLast6 = 6,
    /// Omit last 7 bytes.
    OmitLast7 = 7,
    /// Omit last 8 bytes.
    OmitLast8 = 8,
    /// Omit last 9 bytes.
    OmitLast9 = 9,
    /// Transforms the first UTF-8 codepoint to uppercase.
    UppercaseFirst = 10,
    /// Transforms all UTF-8 codepoints to uppercase.
    UppercaseAll = 11,
    /// Omit first 1 byte.
    OmitFirst1 = 12,
    /// Omit first 2 bytes.
    OmitFirst2 = 13,
    /// Omit first 3 bytes.
    OmitFirst3 = 14,
    /// Omit first 4 bytes.
    OmitFirst4 = 15,
    /// Omit first 5 bytes.
    OmitFirst5 = 16,
    /// Omit first 6 bytes.
    OmitFirst6 = 17,
    /// Omit first 7 bytes.
    OmitFirst7 = 18,
    /// Omit first 8 bytes.
    OmitFirst8 = 19,
    /// Omit first 9 bytes.
    OmitFirst9 = 20,
    /// Shifts the first UTF-8 codepoint with parameter scalar.
    ShiftFirst = 21,
    /// Shifts all UTF-8 codepoints with parameter scalar.
    ShiftAll = 22,
}

impl WordTransformOp {
    /// Converts a raw `u8` integer to the corresponding `WordTransformOp`.
    #[inline]
    pub const fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::Identity),
            1 => Some(Self::OmitLast1),
            2 => Some(Self::OmitLast2),
            3 => Some(Self::OmitLast3),
            4 => Some(Self::OmitLast4),
            5 => Some(Self::OmitLast5),
            6 => Some(Self::OmitLast6),
            7 => Some(Self::OmitLast7),
            8 => Some(Self::OmitLast8),
            9 => Some(Self::OmitLast9),
            10 => Some(Self::UppercaseFirst),
            11 => Some(Self::UppercaseAll),
            12 => Some(Self::OmitFirst1),
            13 => Some(Self::OmitFirst2),
            14 => Some(Self::OmitFirst3),
            15 => Some(Self::OmitFirst4),
            16 => Some(Self::OmitFirst5),
            17 => Some(Self::OmitFirst6),
            18 => Some(Self::OmitFirst7),
            19 => Some(Self::OmitFirst8),
            20 => Some(Self::OmitFirst9),
            21 => Some(Self::ShiftFirst),
            22 => Some(Self::ShiftAll),
            _ => None,
        }
    }

    /// Returns the raw `u8` representation of this transform operation.
    #[inline]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// A transform triplet consisting of prefix ID, word operation, and suffix ID.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TransformTriplet {
    /// Prefix string identifier in `PREFIX_SUFFIX_MAP` (0..=49).
    pub prefix_id: u8,
    /// Word transformation operation.
    pub op: WordTransformOp,
    /// Suffix string identifier in `PREFIX_SUFFIX_MAP` (0..=49).
    pub suffix_id: u8,
}

impl TransformTriplet {
    /// Creates a new transform triplet.
    #[inline]
    pub const fn new(prefix_id: u8, op: WordTransformOp, suffix_id: u8) -> Self {
        Self {
            prefix_id,
            op,
            suffix_id,
        }
    }
}

/// Compact storage pool of length-prefixed strings used for Brotli transforms (217 bytes).
/// Each entry starts with a 1-byte length followed by UTF-8 bytes.
pub static PREFIX_SUFFIX_STORAGE: &[u8; 217] = b"\x01 \x02, \x08 of the \x04 of \x02s \x01.\x05 and \x04 in \x01\"\x04 to \x02\">\x01\n\x02. \x01]\x05 for \x03 a \x06 that \x01\'\x06 with \x06 from \x04 by \x01(\x06. The \x04 on \x04 as \x04 is \x04ing \x02\n\t\x01:\x03ed \x02=\"\x04 at \x03ly \x01,\x02=\'\x05.com/\x07. This \x05 not \x03er \x03al \x04ful \x04ive \x05less \x04est \x04ize \x02\xc2\xa0\x04ous \x05 the \x02e \x00";

/// Offset mapping array into `PREFIX_SUFFIX_STORAGE` for 50 distinct prefix/suffix entries.
pub static PREFIX_SUFFIX_MAP: [u16; 50] = [
    0x00, 0x02, 0x05, 0x0E, 0x13, 0x16, 0x18, 0x1E, 0x23, 0x25,
    0x2A, 0x2D, 0x2F, 0x32, 0x34, 0x3A, 0x3E, 0x45, 0x47, 0x4E,
    0x55, 0x5A, 0x5C, 0x63, 0x68, 0x6D, 0x72, 0x77, 0x7A, 0x7C,
    0x80, 0x83, 0x88, 0x8C, 0x8E, 0x91, 0x97, 0x9F, 0xA5, 0xA9,
    0xAD, 0xB2, 0xB7, 0xBD, 0xC2, 0xC7, 0xCA, 0xCF, 0xD5, 0xD8,
];

/// Fast cutoff transform indices corresponding to omit-last transforms 0..=9.
/// Index 0 represents identity (`["", Identity, ""]`), index 1 represents `["", OmitLast1, ""]`, etc.
pub static CUTOFF_TRANSFORMS: [usize; 10] = [0, 12, 27, 23, 42, 63, 56, 48, 59, 64];

/// Retrieves a slice of prefix or suffix bytes for the given identifier (0..=49).
/// Returns an empty slice `b""` if `id` is invalid or refers to entry 49.
#[inline]
pub fn get_prefix_suffix(id: u8) -> &'static [u8] {
    let id_idx = id as usize;
    if id_idx >= PREFIX_SUFFIX_MAP.len() {
        return &[];
    }
    let offset = PREFIX_SUFFIX_MAP[id_idx] as usize;
    if offset >= PREFIX_SUFFIX_STORAGE.len() {
        return &[];
    }
    let len = PREFIX_SUFFIX_STORAGE[offset] as usize;
    let start = offset + 1;
    let end = start + len;
    if end <= PREFIX_SUFFIX_STORAGE.len() {
        &PREFIX_SUFFIX_STORAGE[start..end]
    } else {
        &[]
    }
}

/// All 121 official RFC 7932 static dictionary word transformation triplets.
pub static TRANSFORMS_TABLE: [TransformTriplet; 121] = [
    TransformTriplet::new(49, WordTransformOp::Identity, 49),
    TransformTriplet::new(49, WordTransformOp::Identity, 0),
    TransformTriplet::new(0, WordTransformOp::Identity, 0),
    TransformTriplet::new(49, WordTransformOp::OmitFirst1, 49),
    TransformTriplet::new(49, WordTransformOp::UppercaseFirst, 0),
    TransformTriplet::new(49, WordTransformOp::Identity, 47),
    TransformTriplet::new(0, WordTransformOp::Identity, 49),
    TransformTriplet::new(4, WordTransformOp::Identity, 0),
    TransformTriplet::new(49, WordTransformOp::Identity, 3),
    TransformTriplet::new(49, WordTransformOp::UppercaseFirst, 49),
    TransformTriplet::new(49, WordTransformOp::Identity, 6),
    TransformTriplet::new(49, WordTransformOp::OmitFirst2, 49),
    TransformTriplet::new(49, WordTransformOp::OmitLast1, 49),
    TransformTriplet::new(1, WordTransformOp::Identity, 0),
    TransformTriplet::new(49, WordTransformOp::Identity, 1),
    TransformTriplet::new(0, WordTransformOp::UppercaseFirst, 0),
    TransformTriplet::new(49, WordTransformOp::Identity, 7),
    TransformTriplet::new(49, WordTransformOp::Identity, 9),
    TransformTriplet::new(48, WordTransformOp::Identity, 0),
    TransformTriplet::new(49, WordTransformOp::Identity, 8),
    TransformTriplet::new(49, WordTransformOp::Identity, 5),
    TransformTriplet::new(49, WordTransformOp::Identity, 10),
    TransformTriplet::new(49, WordTransformOp::Identity, 11),
    TransformTriplet::new(49, WordTransformOp::OmitLast3, 49),
    TransformTriplet::new(49, WordTransformOp::Identity, 13),
    TransformTriplet::new(49, WordTransformOp::Identity, 14),
    TransformTriplet::new(49, WordTransformOp::OmitFirst3, 49),
    TransformTriplet::new(49, WordTransformOp::OmitLast2, 49),
    TransformTriplet::new(49, WordTransformOp::Identity, 15),
    TransformTriplet::new(49, WordTransformOp::Identity, 16),
    TransformTriplet::new(0, WordTransformOp::UppercaseFirst, 49),
    TransformTriplet::new(49, WordTransformOp::Identity, 12),
    TransformTriplet::new(5, WordTransformOp::Identity, 49),
    TransformTriplet::new(0, WordTransformOp::Identity, 1),
    TransformTriplet::new(49, WordTransformOp::OmitFirst4, 49),
    TransformTriplet::new(49, WordTransformOp::Identity, 18),
    TransformTriplet::new(49, WordTransformOp::Identity, 17),
    TransformTriplet::new(49, WordTransformOp::Identity, 19),
    TransformTriplet::new(49, WordTransformOp::Identity, 20),
    TransformTriplet::new(49, WordTransformOp::OmitFirst5, 49),
    TransformTriplet::new(49, WordTransformOp::OmitFirst6, 49),
    TransformTriplet::new(47, WordTransformOp::Identity, 49),
    TransformTriplet::new(49, WordTransformOp::OmitLast4, 49),
    TransformTriplet::new(49, WordTransformOp::Identity, 22),
    TransformTriplet::new(49, WordTransformOp::UppercaseAll, 49),
    TransformTriplet::new(49, WordTransformOp::Identity, 23),
    TransformTriplet::new(49, WordTransformOp::Identity, 24),
    TransformTriplet::new(49, WordTransformOp::Identity, 25),
    TransformTriplet::new(49, WordTransformOp::OmitLast7, 49),
    TransformTriplet::new(49, WordTransformOp::OmitLast1, 26),
    TransformTriplet::new(49, WordTransformOp::Identity, 27),
    TransformTriplet::new(49, WordTransformOp::Identity, 28),
    TransformTriplet::new(0, WordTransformOp::Identity, 12),
    TransformTriplet::new(49, WordTransformOp::Identity, 29),
    TransformTriplet::new(49, WordTransformOp::OmitFirst9, 49),
    TransformTriplet::new(49, WordTransformOp::OmitFirst7, 49),
    TransformTriplet::new(49, WordTransformOp::OmitLast6, 49),
    TransformTriplet::new(49, WordTransformOp::Identity, 21),
    TransformTriplet::new(49, WordTransformOp::UppercaseFirst, 1),
    TransformTriplet::new(49, WordTransformOp::OmitLast8, 49),
    TransformTriplet::new(49, WordTransformOp::Identity, 31),
    TransformTriplet::new(49, WordTransformOp::Identity, 32),
    TransformTriplet::new(47, WordTransformOp::Identity, 3),
    TransformTriplet::new(49, WordTransformOp::OmitLast5, 49),
    TransformTriplet::new(49, WordTransformOp::OmitLast9, 49),
    TransformTriplet::new(0, WordTransformOp::UppercaseFirst, 1),
    TransformTriplet::new(49, WordTransformOp::UppercaseFirst, 8),
    TransformTriplet::new(5, WordTransformOp::Identity, 21),
    TransformTriplet::new(49, WordTransformOp::UppercaseAll, 0),
    TransformTriplet::new(49, WordTransformOp::UppercaseFirst, 10),
    TransformTriplet::new(49, WordTransformOp::Identity, 30),
    TransformTriplet::new(0, WordTransformOp::Identity, 5),
    TransformTriplet::new(35, WordTransformOp::Identity, 49),
    TransformTriplet::new(47, WordTransformOp::Identity, 2),
    TransformTriplet::new(49, WordTransformOp::UppercaseFirst, 17),
    TransformTriplet::new(49, WordTransformOp::Identity, 36),
    TransformTriplet::new(49, WordTransformOp::Identity, 33),
    TransformTriplet::new(5, WordTransformOp::Identity, 0),
    TransformTriplet::new(49, WordTransformOp::UppercaseFirst, 21),
    TransformTriplet::new(49, WordTransformOp::UppercaseFirst, 5),
    TransformTriplet::new(49, WordTransformOp::Identity, 37),
    TransformTriplet::new(0, WordTransformOp::Identity, 30),
    TransformTriplet::new(49, WordTransformOp::Identity, 38),
    TransformTriplet::new(0, WordTransformOp::UppercaseAll, 0),
    TransformTriplet::new(49, WordTransformOp::Identity, 39),
    TransformTriplet::new(0, WordTransformOp::UppercaseAll, 49),
    TransformTriplet::new(49, WordTransformOp::Identity, 34),
    TransformTriplet::new(49, WordTransformOp::UppercaseAll, 8),
    TransformTriplet::new(49, WordTransformOp::UppercaseFirst, 12),
    TransformTriplet::new(0, WordTransformOp::Identity, 21),
    TransformTriplet::new(49, WordTransformOp::Identity, 40),
    TransformTriplet::new(0, WordTransformOp::UppercaseFirst, 12),
    TransformTriplet::new(49, WordTransformOp::Identity, 41),
    TransformTriplet::new(49, WordTransformOp::Identity, 42),
    TransformTriplet::new(49, WordTransformOp::UppercaseAll, 17),
    TransformTriplet::new(49, WordTransformOp::Identity, 43),
    TransformTriplet::new(0, WordTransformOp::UppercaseFirst, 5),
    TransformTriplet::new(49, WordTransformOp::UppercaseAll, 10),
    TransformTriplet::new(0, WordTransformOp::Identity, 34),
    TransformTriplet::new(49, WordTransformOp::UppercaseFirst, 33),
    TransformTriplet::new(49, WordTransformOp::Identity, 44),
    TransformTriplet::new(49, WordTransformOp::UppercaseAll, 5),
    TransformTriplet::new(45, WordTransformOp::Identity, 49),
    TransformTriplet::new(0, WordTransformOp::Identity, 33),
    TransformTriplet::new(49, WordTransformOp::UppercaseFirst, 30),
    TransformTriplet::new(49, WordTransformOp::UppercaseAll, 30),
    TransformTriplet::new(49, WordTransformOp::Identity, 46),
    TransformTriplet::new(49, WordTransformOp::UppercaseAll, 1),
    TransformTriplet::new(49, WordTransformOp::UppercaseFirst, 34),
    TransformTriplet::new(0, WordTransformOp::UppercaseFirst, 33),
    TransformTriplet::new(0, WordTransformOp::UppercaseAll, 30),
    TransformTriplet::new(0, WordTransformOp::UppercaseAll, 1),
    TransformTriplet::new(49, WordTransformOp::UppercaseAll, 33),
    TransformTriplet::new(49, WordTransformOp::UppercaseAll, 21),
    TransformTriplet::new(49, WordTransformOp::UppercaseAll, 12),
    TransformTriplet::new(0, WordTransformOp::UppercaseAll, 5),
    TransformTriplet::new(49, WordTransformOp::UppercaseAll, 34),
    TransformTriplet::new(0, WordTransformOp::UppercaseAll, 12),
    TransformTriplet::new(0, WordTransformOp::UppercaseFirst, 30),
    TransformTriplet::new(0, WordTransformOp::UppercaseAll, 34),
    TransformTriplet::new(0, WordTransformOp::UppercaseFirst, 34),
];

/// Converts the leading UTF-8 codepoint in `dst` to uppercase according to RFC 7932 Section 8.
/// Returns the number of consumed bytes (1, 2, or 3), or 0 if `dst` is empty.
/// Guarantees zero panic even on malformed or truncated byte slices.
#[inline]
pub fn to_uppercase_utf8(dst: &mut [u8]) -> usize {
    if dst.is_empty() {
        return 0;
    }
    if dst[0] < 0xC0 {
        if dst[0] >= b'a' && dst[0] <= b'z' {
            dst[0] ^= 32;
        }
        1
    } else if dst[0] < 0xE0 {
        if dst.len() >= 2 {
            dst[1] ^= 32;
            2
        } else {
            1
        }
    } else {
        if dst.len() >= 3 {
            dst[2] ^= 5;
            3
        } else {
            dst.len()
        }
    }
}

/// Shifts the leading UTF-8 codepoint in `dst` by the given 16-bit scalar parameter.
/// Returns the number of consumed bytes (1..=4), or 0 if `dst` is empty.
#[inline]
pub fn shift_utf8(dst: &mut [u8], param: u16) -> usize {
    if dst.is_empty() {
        return 0;
    }
    let scalar_base = (param as u32 & 0x7FFF) + (0x1000000 - (param as u32 & 0x8000));
    let first = dst[0];

    if first < 0x80 {
        let scalar = scalar_base + (first as u32);
        dst[0] = (scalar & 0x7F) as u8;
        1
    } else if first < 0xC0 {
        1
    } else if first < 0xE0 {
        if dst.len() < 2 {
            return 1;
        }
        let scalar = scalar_base + (((dst[1] & 0x3F) as u32) | (((first & 0x1F) as u32) << 6));
        dst[0] = 0xC0 | (((scalar >> 6) & 0x1F) as u8);
        dst[1] = (dst[1] & 0xC0) | ((scalar & 0x3F) as u8);
        2
    } else if first < 0xF0 {
        if dst.len() < 3 {
            return dst.len();
        }
        let scalar = scalar_base
            + (((dst[2] & 0x3F) as u32)
                | (((dst[1] & 0x3F) as u32) << 6)
                | (((first & 0x0F) as u32) << 12));
        dst[0] = 0xE0 | (((scalar >> 12) & 0x0F) as u8);
        dst[1] = (dst[1] & 0xC0) | (((scalar >> 6) & 0x3F) as u8);
        dst[2] = (dst[2] & 0xC0) | ((scalar & 0x3F) as u8);
        3
    } else if first < 0xF8 {
        if dst.len() < 4 {
            return dst.len();
        }
        let scalar = scalar_base
            + (((dst[3] & 0x3F) as u32)
                | (((dst[2] & 0x3F) as u32) << 6)
                | (((dst[1] & 0x3F) as u32) << 12)
                | (((first & 0x07) as u32) << 18));
        dst[0] = 0xF0 | (((scalar >> 18) & 0x07) as u8);
        dst[1] = (dst[1] & 0xC0) | (((scalar >> 12) & 0x3F) as u8);
        dst[2] = (dst[2] & 0xC0) | (((scalar >> 6) & 0x3F) as u8);
        dst[3] = (dst[3] & 0xC0) | ((scalar & 0x3F) as u8);
        4
    } else {
        1
    }
}

/// Applies an RFC 7932 static dictionary word transformation in-place into `dst`.
///
/// Returns the total number of bytes written to `dst` on success.
///
/// # Errors
/// Returns [`BrotliError::InvalidTransformIndex`] if `transform_idx >= 121`.
/// Returns [`BrotliError::BufferTooSmall`] if `dst` does not have enough capacity.
pub fn transform_dictionary_word(
    dst: &mut [u8],
    word: &[u8],
    transform_idx: usize,
) -> Result<usize, BrotliError> {
    if transform_idx >= TRANSFORMS_TABLE.len() {
        return Err(BrotliError::InvalidTransformIndex(transform_idx));
    }

    let triplet = TRANSFORMS_TABLE[transform_idx];
    let prefix = get_prefix_suffix(triplet.prefix_id);
    let suffix = get_prefix_suffix(triplet.suffix_id);
    let op = triplet.op;

    let word_slice: &[u8] = match op {
        WordTransformOp::OmitLast1 => &word[..word.len().saturating_sub(1)],
        WordTransformOp::OmitLast2 => &word[..word.len().saturating_sub(2)],
        WordTransformOp::OmitLast3 => &word[..word.len().saturating_sub(3)],
        WordTransformOp::OmitLast4 => &word[..word.len().saturating_sub(4)],
        WordTransformOp::OmitLast5 => &word[..word.len().saturating_sub(5)],
        WordTransformOp::OmitLast6 => &word[..word.len().saturating_sub(6)],
        WordTransformOp::OmitLast7 => &word[..word.len().saturating_sub(7)],
        WordTransformOp::OmitLast8 => &word[..word.len().saturating_sub(8)],
        WordTransformOp::OmitLast9 => &word[..word.len().saturating_sub(9)],
        WordTransformOp::OmitFirst1 => {
            let skip = 1.min(word.len());
            &word[skip..]
        }
        WordTransformOp::OmitFirst2 => {
            let skip = 2.min(word.len());
            &word[skip..]
        }
        WordTransformOp::OmitFirst3 => {
            let skip = 3.min(word.len());
            &word[skip..]
        }
        WordTransformOp::OmitFirst4 => {
            let skip = 4.min(word.len());
            &word[skip..]
        }
        WordTransformOp::OmitFirst5 => {
            let skip = 5.min(word.len());
            &word[skip..]
        }
        WordTransformOp::OmitFirst6 => {
            let skip = 6.min(word.len());
            &word[skip..]
        }
        WordTransformOp::OmitFirst7 => {
            let skip = 7.min(word.len());
            &word[skip..]
        }
        WordTransformOp::OmitFirst8 => {
            let skip = 8.min(word.len());
            &word[skip..]
        }
        WordTransformOp::OmitFirst9 => {
            let skip = 9.min(word.len());
            &word[skip..]
        }
        WordTransformOp::Identity
        | WordTransformOp::UppercaseFirst
        | WordTransformOp::UppercaseAll
        | WordTransformOp::ShiftFirst
        | WordTransformOp::ShiftAll => word,
    };

    let required_len = prefix.len() + word_slice.len() + suffix.len();
    if dst.len() < required_len {
        return Err(BrotliError::BufferTooSmall {
            required: required_len,
            available: dst.len(),
        });
    }

    // 1. Copy prefix
    let mut out_pos = 0;
    if !prefix.is_empty() {
        dst[out_pos..out_pos + prefix.len()].copy_from_slice(prefix);
        out_pos += prefix.len();
    }

    // 2. Copy and transform word body
    let word_start = out_pos;
    let word_end = word_start + word_slice.len();
    if !word_slice.is_empty() {
        dst[word_start..word_end].copy_from_slice(word_slice);
        let word_mut = &mut dst[word_start..word_end];

        match op {
            WordTransformOp::UppercaseFirst => {
                to_uppercase_utf8(word_mut);
            }
            WordTransformOp::UppercaseAll => {
                let mut cursor = 0;
                while cursor < word_mut.len() {
                    let step = to_uppercase_utf8(&mut word_mut[cursor..]);
                    if step == 0 {
                        break;
                    }
                    cursor += step;
                }
            }
            WordTransformOp::ShiftFirst => {
                shift_utf8(word_mut, 0);
            }
            WordTransformOp::ShiftAll => {
                let mut cursor = 0;
                while cursor < word_mut.len() {
                    let step = shift_utf8(&mut word_mut[cursor..], 0);
                    if step == 0 {
                        break;
                    }
                    cursor += step;
                }
            }
            _ => {}
        }
        out_pos = word_end;
    }

    // 3. Copy suffix
    if !suffix.is_empty() {
        dst[out_pos..out_pos + suffix.len()].copy_from_slice(suffix);
        out_pos += suffix.len();
    }

    Ok(out_pos)
}
