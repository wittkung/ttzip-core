// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, high-throughput Snappy Varint-32 (LEB128) encoding and decoding routines.
//!
//! Provides zero-allocation, boundary-hardened decoding with strict defenses against
//! malicious 32-bit overflow attacks and truncated inputs.

use super::error::SnappyError;

/// Maximum number of bytes required to represent an unsigned 32-bit integer in Snappy varint encoding.
pub const MAX_VARINT32_BYTES: usize = 5;

/// Returns the exact byte length required to encode a 32-bit integer as a Snappy varint.
///
/// # Examples
/// ```
/// use ttzip_engine::codecs::snappy::varint::varint32_len;
///
/// assert_eq!(varint32_len(0), 1);
/// assert_eq!(varint32_len(127), 1);
/// assert_eq!(varint32_len(128), 2);
/// assert_eq!(varint32_len(16383), 2);
/// assert_eq!(varint32_len(16384), 3);
/// assert_eq!(varint32_len(2097151), 3);
/// assert_eq!(varint32_len(2097152), 4);
/// assert_eq!(varint32_len(268435455), 4);
/// assert_eq!(varint32_len(268435456), 5);
/// assert_eq!(varint32_len(u32::MAX), 5);
/// ```
#[inline]
#[must_use]
pub const fn varint32_len(val: u32) -> usize {
    if val < (1 << 7) {
        1
    } else if val < (1 << 14) {
        2
    } else if val < (1 << 21) {
        3
    } else if val < (1 << 28) {
        4
    } else {
        5
    }
}

/// Encodes an unsigned 32-bit integer into the destination byte slice using standard Snappy Varint-32 format.
///
/// Returns the number of bytes written to `dst` (between 1 and 5).
///
/// # Panics
/// Panics in debug/release if `dst.len() < varint32_len(val)`.
#[inline]
pub fn encode_varint32(val: u32, dst: &mut [u8]) -> usize {
    let needed = varint32_len(val);
    assert!(
        dst.len() >= needed,
        "destination buffer too small for varint-32 encoding"
    );

    let mut v = val;
    let mut i = 0;
    while v >= 0x80 {
        dst[i] = (v as u8 & 0x7F) | 0x80;
        v >>= 7;
        i += 1;
    }
    dst[i] = v as u8;
    i + 1
}

/// Decodes an unsigned 32-bit integer from the provided byte slice.
///
/// Returns `Ok((decoded_value, bytes_consumed))` on success.
///
/// # Security & Overflow Defenses
/// 1. Intercepts any varint sequence extending beyond `MAX_VARINT32_BYTES` (5 bytes) with `SnappyError::VarintOverflow`.
/// 2. Intercepts any 5th byte where the high 4 bits (bits 4..7) are non-zero with `SnappyError::VarintOverflow`,
///    as this represents values strictly greater than `u32::MAX` (2^32 - 1).
/// 3. Returns `SnappyError::UnexpectedEof` when the input slice is truncated before a terminating byte (`MSB == 0`).
#[inline]
pub fn decode_varint32(src: &[u8]) -> Result<(u32, usize), SnappyError> {
    if src.is_empty() {
        return Err(SnappyError::UnexpectedEof);
    }

    let mut result: u32 = 0;
    let mut shift = 0;
    let limit = src.len().min(MAX_VARINT32_BYTES);

    for i in 0..limit {
        let b = src[i];
        if i == 4 {
            // In the 5th byte, bit 7 (continuation) must be 0 and bits 4..6 must be 0
            // since 4 * 7 = 28 bits have already been read, leaving at most 4 bits for a 32-bit integer.
            if b > 0x0F {
                return Err(SnappyError::VarintOverflow);
            }
            result |= (b as u32) << shift;
            return Ok((result, 5));
        }

        let val = (b & 0x7F) as u32;
        result |= val << shift;
        if (b & 0x80) == 0 {
            return Ok((result, i + 1));
        }
        shift += 7;
    }

    if src.len() < MAX_VARINT32_BYTES {
        Err(SnappyError::UnexpectedEof)
    } else {
        Err(SnappyError::VarintOverflow)
    }
}
