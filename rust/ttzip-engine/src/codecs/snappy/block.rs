// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Rust Google Snappy block compression, decompression, and length probe.
//!
//! Uses zero-copy buffer operations backed by the `snap::raw` engine.

use crate::types::TTZipStatus;
use snap::raw::{max_compress_len, Decoder, Encoder};

/// Computes upper bound on compressed bytes for a given raw input size.
#[inline]
pub fn snappy_compress_bound(src_size: usize) -> usize {
    max_compress_len(src_size)
}

/// Safely parses an unsigned varint (LEB128) from byte slice without panic or overflow.
///
/// Returns `Some((uncompressed_len, bytes_consumed))` on success. Max 5 bytes (u32 range) per Snappy spec.
#[inline]
pub fn parse_varint(src: &[u8]) -> Option<(usize, usize)> {
    if src.is_empty() {
        return None;
    }
    let mut result: usize = 0;
    let mut shift = 0;
    let max_bytes = 5.min(src.len());
    for (i, &byte) in src[..max_bytes].iter().enumerate() {
        let val = (byte & 0x7F) as usize;
        // Check 32-bit overflow boundary
        if shift >= 32 || (shift == 28 && val > 0x0F) {
            return None;
        }
        result |= val << shift;
        if (byte & 0x80) == 0 {
            return Some((result, i + 1));
        }
        shift += 7;
    }
    None
}

/// Parses uncompressed length from a raw Snappy varint header safely.
pub fn snappy_uncompressed_length(src: &[u8]) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    match parse_varint(src) {
        Some((len, _)) => Ok(len),
        None => Err(TTZipStatus::ErrCorruptHeader),
    }
}

/// Validates integrity of a Snappy compressed buffer with bounded uncompressed length limit in O(1) memory.
///
/// Executes a pure-Rust bytecode verification state machine with ZERO heap allocations.
pub fn snappy_validate_bounded(src: &[u8], max_uncompressed_len: usize) -> bool {
    if src.is_empty() {
        return true;
    }

    let (expected_len, header_len) = match parse_varint(src) {
        Some(res) => res,
        None => return false,
    };

    if expected_len > max_uncompressed_len {
        return false;
    }

    if expected_len == 0 {
        return header_len == src.len();
    }

    let mut src_pos = header_len;
    let mut uncompressed_pos = 0usize;

    while src_pos < src.len() {
        if uncompressed_pos >= expected_len {
            // Trailing extraneous compressed bytes when uncompressed target already met
            return false;
        }

        let tag = src[src_pos];
        src_pos += 1;

        match tag & 0x03 {
            0b00 => {
                // Literal element
                let len_tag = (tag >> 2) as usize;
                let literal_len = match len_tag {
                    0..=59 => len_tag + 1,
                    60 => {
                        if src_pos >= src.len() {
                            return false;
                        }
                        let l = (src[src_pos] as usize) + 1;
                        src_pos += 1;
                        l
                    }
                    61 => {
                        if src_pos + 2 > src.len() {
                            return false;
                        }
                        let l = (u16::from_le_bytes([src[src_pos], src[src_pos + 1]]) as usize) + 1;
                        src_pos += 2;
                        l
                    }
                    62 => {
                        if src_pos + 3 > src.len() {
                            return false;
                        }
                        let l = (src[src_pos] as usize
                            | ((src[src_pos + 1] as usize) << 8)
                            | ((src[src_pos + 2] as usize) << 16))
                            + 1;
                        src_pos += 3;
                        l
                    }
                    63 => {
                        if src_pos + 4 > src.len() {
                            return false;
                        }
                        let l = (u32::from_le_bytes([
                            src[src_pos],
                            src[src_pos + 1],
                            src[src_pos + 2],
                            src[src_pos + 3],
                        ]) as usize)
                            + 1;
                        src_pos += 4;
                        l
                    }
                    _ => return false,
                };

                if src_pos.checked_add(literal_len).map_or(true, |end| end > src.len()) {
                    return false;
                }
                src_pos += literal_len;

                if uncompressed_pos
                    .checked_add(literal_len)
                    .map_or(true, |end| end > expected_len)
                {
                    return false;
                }
                uncompressed_pos += literal_len;
            }
            0b01 => {
                // Copy with 1-byte offset
                if src_pos >= src.len() {
                    return false;
                }
                let copy_len = (((tag >> 2) & 0x07) as usize) + 4;
                let offset_high = (tag >> 5) as usize;
                let offset_low = src[src_pos] as usize;
                src_pos += 1;
                let offset = (offset_high << 8) | offset_low;

                if offset == 0 || offset > uncompressed_pos {
                    return false;
                }
                if uncompressed_pos
                    .checked_add(copy_len)
                    .map_or(true, |end| end > expected_len)
                {
                    return false;
                }
                uncompressed_pos += copy_len;
            }
            0b10 => {
                // Copy with 2-byte offset
                if src_pos + 2 > src.len() {
                    return false;
                }
                let copy_len = ((tag >> 2) as usize) + 1;
                let offset = u16::from_le_bytes([src[src_pos], src[src_pos + 1]]) as usize;
                src_pos += 2;

                if offset == 0 || offset > uncompressed_pos {
                    return false;
                }
                if uncompressed_pos
                    .checked_add(copy_len)
                    .map_or(true, |end| end > expected_len)
                {
                    return false;
                }
                uncompressed_pos += copy_len;
            }
            0b11 => {
                // Copy with 4-byte offset
                if src_pos + 4 > src.len() {
                    return false;
                }
                let copy_len = ((tag >> 2) as usize) + 1;
                let offset = u32::from_le_bytes([
                    src[src_pos],
                    src[src_pos + 1],
                    src[src_pos + 2],
                    src[src_pos + 3],
                ]) as usize;
                src_pos += 4;

                if offset == 0 || offset > uncompressed_pos {
                    return false;
                }
                if uncompressed_pos
                    .checked_add(copy_len)
                    .map_or(true, |end| end > expected_len)
                {
                    return false;
                }
                uncompressed_pos += copy_len;
            }
            _ => return false,
        }
    }

    src_pos == src.len() && uncompressed_pos == expected_len
}

/// Validates integrity of a Snappy compressed buffer without decompressing or allocating heap memory.
#[inline]
pub fn snappy_validate(src: &[u8]) -> bool {
    snappy_validate_bounded(src, usize::MAX)
}

/// Compresses a memory block using pure Rust Google Snappy raw format.
pub fn snappy_compress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    let mut enc = Encoder::new();
    enc.compress(src, dst).map_err(|_| TTZipStatus::ErrCompressionFailed)
}

/// Decompresses a raw Snappy compressed block into a pre-allocated destination buffer.
pub fn snappy_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    let mut dec = Decoder::new();
    dec.decompress(src, dst).map_err(|_| TTZipStatus::ErrCorruptHeader)
}

/// Compresses a memory slice into a newly allocated `Vec<u8>`.
pub fn snappy_compress_to_vec(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() {
        return Ok(Vec::new());
    }
    let mut enc = Encoder::new();
    enc.compress_vec(src).map_err(|_| TTZipStatus::ErrCompressionFailed)
}

/// Decompresses a raw Snappy slice into a newly allocated `Vec<u8>`.
pub fn snappy_decompress_to_vec(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() {
        return Ok(Vec::new());
    }
    let mut dec = Decoder::new();
    dec.decompress_vec(src).map_err(|_| TTZipStatus::ErrCorruptHeader)
}
