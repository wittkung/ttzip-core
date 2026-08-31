// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Second-order context modeling and static lookup tables for Brotli compression.
//!
//! Compliant with RFC 7932 Section 7.3 and Google Brotli reference implementation.
//! Provides zero-branch, 2048-byte compile-time static LUT lookup for literal contexts
//! across the 4 standard context modes (`LSB6`, `MSB6`, `UTF8`, `SIGNED`), distance
//! context derivation, and context map management structures.

/// The 4 standard context modeling modes defined in RFC 7932 Section 7.3.
#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum BrotliContextMode {
    /// Context ID is the 6 least significant bits of the previous byte (`p1 & 0x3F`).
    #[default]
    Lsb6 = 0,
    /// Context ID is the 6 most significant bits of the previous byte (`p1 >> 2`).
    Msb6 = 1,
    /// Second-order context model tuned for UTF-8 encoded text using `(p1, p2)`.
    Utf8 = 2,
    /// Second-order context model tuned for signed integers using `(p1, p2)`.
    Signed = 3,
}

impl BrotliContextMode {
    /// Convert an integer mode value (0..=3) to `BrotliContextMode`.
    #[inline(always)]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Lsb6),
            1 => Some(Self::Msb6),
            2 => Some(Self::Utf8),
            3 => Some(Self::Signed),
            _ => None,
        }
    }

    /// Convert an integer mode value (0..=3) to `BrotliContextMode`, clamping invalid values to `Lsb6`.
    #[inline(always)]
    pub const fn from_u8_clamped(value: u8) -> Self {
        match value {
            0 => Self::Lsb6,
            1 => Self::Msb6,
            2 => Self::Utf8,
            3 => Self::Signed,
            _ => Self::Lsb6,
        }
    }
}

/// 2048-byte static context lookup table defined in RFC 7932 and Brotli C codebase.
///
/// Layout:
/// - `0..512`: `CONTEXT_LSB6` (256 bytes for `p1`, 256 bytes of zeros for `p2`)
/// - `512..1024`: `CONTEXT_MSB6` (256 bytes for `p1`, 256 bytes of zeros for `p2`)
/// - `1024..1536`: `CONTEXT_UTF8` (256 bytes for `p1`, 256 bytes for `p2`)
/// - `1536..2048`: `CONTEXT_SIGNED` (256 bytes for `p1`, 256 bytes for `p2`)
#[rustfmt::skip]
pub static BROTLI_CONTEXT_LOOKUP_TABLE: [u8; 2048] = [
    // --- 0..512: CONTEXT_LSB6 ---
    // CONTEXT_LSB6, last byte (p1)
     0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, 15,
    16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
    32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47,
    48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63,
     0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, 15,
    16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
    32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47,
    48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63,
     0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, 15,
    16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
    32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47,
    48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63,
     0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, 15,
    16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
    32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47,
    48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63,
    // CONTEXT_LSB6, second last byte (p2)
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,

    // --- 512..1024: CONTEXT_MSB6 ---
    // CONTEXT_MSB6, last byte (p1)
     0,  0,  0,  0,  1,  1,  1,  1,  2,  2,  2,  2,  3,  3,  3,  3,
     4,  4,  4,  4,  5,  5,  5,  5,  6,  6,  6,  6,  7,  7,  7,  7,
     8,  8,  8,  8,  9,  9,  9,  9, 10, 10, 10, 10, 11, 11, 11, 11,
    12, 12, 12, 12, 13, 13, 13, 13, 14, 14, 14, 14, 15, 15, 15, 15,
    16, 16, 16, 16, 17, 17, 17, 17, 18, 18, 18, 18, 19, 19, 19, 19,
    20, 20, 20, 20, 21, 21, 21, 21, 22, 22, 22, 22, 23, 23, 23, 23,
    24, 24, 24, 24, 25, 25, 25, 25, 26, 26, 26, 26, 27, 27, 27, 27,
    28, 28, 28, 28, 29, 29, 29, 29, 30, 30, 30, 30, 31, 31, 31, 31,
    32, 32, 32, 32, 33, 33, 33, 33, 34, 34, 34, 34, 35, 35, 35, 35,
    36, 36, 36, 36, 37, 37, 37, 37, 38, 38, 38, 38, 39, 39, 39, 39,
    40, 40, 40, 40, 41, 41, 41, 41, 42, 42, 42, 42, 43, 43, 43, 43,
    44, 44, 44, 44, 45, 45, 45, 45, 46, 46, 46, 46, 47, 47, 47, 47,
    48, 48, 48, 48, 49, 49, 49, 49, 50, 50, 50, 50, 51, 51, 51, 51,
    52, 52, 52, 52, 53, 53, 53, 53, 54, 54, 54, 54, 55, 55, 55, 55,
    56, 56, 56, 56, 57, 57, 57, 57, 58, 58, 58, 58, 59, 59, 59, 59,
    60, 60, 60, 60, 61, 61, 61, 61, 62, 62, 62, 62, 63, 63, 63, 63,
    // CONTEXT_MSB6, second last byte (p2)
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,

    // --- 1024..1536: CONTEXT_UTF8 ---
    // CONTEXT_UTF8, last byte (p1)
    // ASCII range (0..127)
     0,  0,  0,  0,  0,  0,  0,  0,  0,  4,  4,  0,  0,  4,  0,  0,
     0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,
     8, 12, 16, 12, 12, 20, 12, 16, 24, 28, 12, 12, 32, 12, 36, 12,
    44, 44, 44, 44, 44, 44, 44, 44, 44, 44, 32, 32, 24, 40, 28, 12,
    12, 48, 52, 52, 52, 48, 52, 52, 52, 48, 52, 52, 52, 52, 52, 48,
    52, 52, 52, 52, 52, 48, 52, 52, 52, 52, 52, 24, 12, 28, 12, 12,
    12, 56, 60, 60, 60, 56, 60, 60, 60, 56, 60, 60, 60, 60, 60, 56,
    60, 60, 60, 60, 60, 56, 60, 60, 60, 60, 60, 24, 12, 28, 12,  0,
    // UTF-8 continuation byte range (128..191)
    0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1,
    0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1,
    0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1,
    0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1,
    // UTF-8 lead byte range (192..255)
    2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3,
    2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3,
    2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3,
    2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3,
    // CONTEXT_UTF8, second last byte (p2)
    // ASCII range (0..127)
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1,
    1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1,
    1, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 1, 1, 1, 1, 0,
    // UTF-8 continuation byte range (128..191)
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    // UTF-8 lead byte range (192..255)
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,

    // --- 1536..2048: CONTEXT_SIGNED ---
    // CONTEXT_SIGNED, last byte (p1)
     0,  8,  8,  8,  8,  8,  8,  8,  8,  8,  8,  8,  8,  8,  8,  8,
    16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
    16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
    16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
    24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24,
    24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24,
    24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24,
    24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24,
    32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32,
    32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32,
    32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32,
    32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32,
    40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40,
    40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40,
    40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40,
    48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 56,
    // CONTEXT_SIGNED, second last byte (p2)
    0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
    5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
    5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 7,
];

/// Compute the literal context ID (0..63) given the previous two bytes and mode.
///
/// Complies with RFC 7932 Section 7.3:
/// `context_id = LUT[offset + p1] | LUT[offset + 256 + p2]`
///
/// Marked `#[inline(always)]` for zero-branch inlined assembly code generation.
#[inline(always)]
pub const fn get_context_id(p1: u8, p2: u8, mode: BrotliContextMode) -> usize {
    let offset = (mode as usize) * 512;
    // Guaranteed by construction: offset <= 1536, offset + p1 <= 1791 < 2048,
    // offset + 256 + p2 <= 2047 < 2048.
    let c1 = BROTLI_CONTEXT_LOOKUP_TABLE[offset + (p1 as usize)];
    let c2 = BROTLI_CONTEXT_LOOKUP_TABLE[offset + 256 + (p2 as usize)];
    (c1 | c2) as usize
}

/// Compute the distance context ID (0..3) based on backward reference copy length.
///
/// Complies with RFC 7932 Section 7.3:
/// - `copy_len <= 2 -> 0`
/// - `copy_len == 3 -> 1`
/// - `copy_len == 4 -> 2`
/// - `copy_len > 4  -> 3`
#[inline(always)]
pub const fn get_distance_context(copy_len: usize) -> usize {
    match copy_len {
        0..=2 => 0,
        3 => 1,
        4 => 2,
        _ => 3,
    }
}

/// Returns a reference to the 512-byte LUT segment for the specified context mode.
#[inline(always)]
pub fn context_lut_slice(mode: BrotliContextMode) -> &'static [u8] {
    let offset = (mode as usize) * 512;
    &BROTLI_CONTEXT_LOOKUP_TABLE[offset..offset + 512]
}

/// Context map managing the assignment of `(block_type, context_id)` to Huffman tree indices.
///
/// In Brotli:
/// - Literals use 64 contexts per block type (`block_type << 6 | context_id`).
/// - Backward copy distances use 4 contexts per block type (`block_type << 2 | dist_context`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrotliContextMap {
    /// Total number of distinct Huffman trees (alphabet trees) referenced.
    num_trees: usize,
    /// Context-to-tree lookup table.
    map: Vec<u8>,
}

impl BrotliContextMap {
    /// Construct a new `BrotliContextMap` with the given tree count and mapping slice.
    pub fn new(num_trees: usize, map: Vec<u8>) -> Self {
        Self { num_trees, map }
    }

    /// Construct a trivial single-tree context map where all contexts map to tree 0.
    pub fn single_tree(context_count: usize) -> Self {
        Self {
            num_trees: 1,
            map: vec![0u8; context_count],
        }
    }

    /// Construct an identity context map where context `i` maps to tree `i`.
    pub fn identity(context_count: usize) -> Self {
        let mut map = Vec::with_capacity(context_count);
        for i in 0..context_count {
            map.push((i & 0xFF) as u8);
        }
        Self {
            num_trees: context_count,
            map,
        }
    }

    /// Construct a default 64-entry literal context map for a single block type.
    pub fn default_literal_map() -> Self {
        Self::single_tree(64)
    }

    /// Construct a default 4-entry distance context map for a single block type.
    pub fn default_distance_map() -> Self {
        Self::single_tree(4)
    }

    /// Retrieve the total number of distinct Huffman trees in this map.
    #[inline(always)]
    pub fn num_trees(&self) -> usize {
        self.num_trees
    }

    /// Retrieve the total number of context entries in this map.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Check if the context map is empty.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Query the Huffman tree index for a given raw context index.
    #[inline(always)]
    pub fn get(&self, index: usize) -> usize {
        if let Some(&tree_idx) = self.map.get(index) {
            tree_idx as usize
        } else {
            0
        }
    }

    /// Query the Huffman tree index for a literal given `block_type` and `context_id` (0..63).
    #[inline(always)]
    pub fn get_literal_tree(&self, block_type: usize, context_id: usize) -> usize {
        let idx = (block_type << 6) + (context_id & 63);
        self.get(idx)
    }

    /// Query the Huffman tree index for a distance given `block_type` and `distance_context` (0..3).
    #[inline(always)]
    pub fn get_distance_tree(&self, block_type: usize, distance_context: usize) -> usize {
        let idx = (block_type << 2) + (distance_context & 3);
        self.get(idx)
    }

    /// View the underlying mapping slice.
    #[inline(always)]
    pub fn as_slice(&self) -> &[u8] {
        &self.map
    }

    /// Check if the context map is trivial (all contexts map to tree 0 with `num_trees <= 1`).
    pub fn is_trivial(&self) -> bool {
        self.num_trees <= 1 && self.map.iter().all(|&x| x == 0)
    }

    /// Apply RFC 7932 inverse Move-To-Front (MTF) transform to decode a serialized context map.
    pub fn inverse_move_to_front(map: &mut [u8]) {
        let mut mtf = [0u8; 256];
        for (i, item) in mtf.iter_mut().enumerate() {
            *item = i as u8;
        }

        for val in map.iter_mut() {
            let index = *val as usize;
            let symbol = mtf[index];
            *val = symbol;
            // Shift elements right
            for j in (1..=index).rev() {
                mtf[j] = mtf[j - 1];
            }
            mtf[0] = symbol;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_mode_representation() {
        assert_eq!(BrotliContextMode::Lsb6 as usize, 0);
        assert_eq!(BrotliContextMode::Msb6 as usize, 1);
        assert_eq!(BrotliContextMode::Utf8 as usize, 2);
        assert_eq!(BrotliContextMode::Signed as usize, 3);

        assert_eq!(BrotliContextMode::from_u8(0), Some(BrotliContextMode::Lsb6));
        assert_eq!(BrotliContextMode::from_u8(1), Some(BrotliContextMode::Msb6));
        assert_eq!(BrotliContextMode::from_u8(2), Some(BrotliContextMode::Utf8));
        assert_eq!(BrotliContextMode::from_u8(3), Some(BrotliContextMode::Signed));
        assert_eq!(BrotliContextMode::from_u8(4), None);
    }

    #[test]
    fn test_lut_table_size() {
        assert_eq!(BROTLI_CONTEXT_LOOKUP_TABLE.len(), 2048);
    }

    #[test]
    fn test_lsb6_context_computation() {
        for p1 in 0..=255u8 {
            for p2 in [0u8, 42u8, 128u8, 255u8] {
                let ctx = get_context_id(p1, p2, BrotliContextMode::Lsb6);
                assert_eq!(ctx, (p1 & 0x3F) as usize);
                assert!(ctx < 64);
            }
        }
    }

    #[test]
    fn test_msb6_context_computation() {
        for p1 in 0..=255u8 {
            for p2 in [0u8, 42u8, 128u8, 255u8] {
                let ctx = get_context_id(p1, p2, BrotliContextMode::Msb6);
                assert_eq!(ctx, (p1 >> 2) as usize);
                assert!(ctx < 64);
            }
        }
    }

    #[test]
    fn test_distance_context() {
        assert_eq!(get_distance_context(0), 0);
        assert_eq!(get_distance_context(1), 0);
        assert_eq!(get_distance_context(2), 0);
        assert_eq!(get_distance_context(3), 1);
        assert_eq!(get_distance_context(4), 2);
        assert_eq!(get_distance_context(5), 3);
        assert_eq!(get_distance_context(100), 3);
        assert_eq!(get_distance_context(1024), 3);
    }

    #[test]
    fn test_context_map_basic() {
        let default_lit = BrotliContextMap::default_literal_map();
        assert_eq!(default_lit.len(), 64);
        assert_eq!(default_lit.num_trees(), 1);
        assert!(default_lit.is_trivial());
        assert_eq!(default_lit.get_literal_tree(0, 15), 0);

        let default_dist = BrotliContextMap::default_distance_map();
        assert_eq!(default_dist.len(), 4);
        assert_eq!(default_dist.num_trees(), 1);
        assert!(default_dist.is_trivial());
        assert_eq!(default_dist.get_distance_tree(0, 2), 0);
    }

    #[test]
    fn test_context_map_inverse_mtf() {
        let mut data = vec![0u8, 1, 0, 2, 1];
        // MTF initially: [0, 1, 2, 3, 4, ...]
        // 0 -> 0 (mtf: 0, 1, 2, ...)
        // 1 -> 1 (mtf: 1, 0, 2, ...)
        // 0 -> 1 (mtf: 1, 0, 2, ...)
        // 2 -> 2 (mtf: 2, 1, 0, ...)
        // 1 -> 1 (mtf: 1, 2, 0, ...)
        BrotliContextMap::inverse_move_to_front(&mut data);
        assert_eq!(data, vec![0, 1, 1, 2, 1]);
    }
}
