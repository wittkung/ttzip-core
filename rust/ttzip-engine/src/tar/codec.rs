// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! TAR octal ASCII codecs and GNU Base-256 binary encoding/decoding.

/// Parses an ASCII octal byte slice into a `u64`.
///
/// Leading and trailing spaces (`' '`) and nulls (`\0`) are safely trimmed.
/// Returns `Some(0)` for empty/all-null fields, or `None` if invalid octal digits are present.
pub fn octal_from(bytes: &[u8]) -> Option<u64> {
    let mut trimmed = bytes;
    while !trimmed.is_empty() && (trimmed[0] == b' ' || trimmed[0] == 0) {
        trimmed = &trimmed[1..];
    }
    while !trimmed.is_empty() && (trimmed[trimmed.len() - 1] == b' ' || trimmed[trimmed.len() - 1] == 0) {
        trimmed = &trimmed[..trimmed.len() - 1];
    }
    if trimmed.is_empty() {
        return Some(0);
    }

    let mut result: u64 = 0;
    for &b in trimmed {
        if !(b'0'..=b'7').contains(&b) {
            return None;
        }
        result = result.checked_mul(8)?.checked_add((b - b'0') as u64)?;
    }
    Some(result)
}

/// Formats a `u64` into a destination slice using standard null-terminated octal ASCII.
///
/// Digits are zero-padded and right-aligned with a trailing null terminator at `dst.len() - 1`.
pub fn octal_into(dst: &mut [u8], val: u64) {
    let len = dst.len();
    if len == 0 {
        return;
    }
    dst.fill(b'0');
    dst[len - 1] = 0; // null terminator

    if len == 1 {
        return;
    }

    let mut curr = val;
    let mut idx = len - 2;
    loop {
        dst[idx] = b'0' + (curr % 8) as u8;
        curr /= 8;
        if curr == 0 || idx == 0 {
            break;
        }
        idx -= 1;
    }
}

/// Decodes a big-endian binary base-256 integer from a byte slice.
///
/// Bit 7 of `bytes[0]` is the positive binary indicator (0x80) used in GNU TAR.
pub fn base256_from(bytes: &[u8]) -> Option<u64> {
    if bytes.is_empty() {
        return Some(0);
    }
    if (bytes[0] & 0x80) == 0 {
        return None;
    }

    let mut val: u64 = (bytes[0] & 0x7F) as u64;
    for &b in &bytes[1..] {
        val = (val.checked_shl(8)?) | (b as u64);
    }
    Some(val)
}

/// Encodes a `u64` value into a byte slice using GNU Base-256 big-endian format.
///
/// Sets `dst[0] = 0x80` and stores `val` across the remaining bytes in big-endian order.
pub fn base256_into(dst: &mut [u8], val: u64) {
    let len = dst.len();
    if len == 0 {
        return;
    }
    dst.fill(0);
    dst[0] = 0x80;
    let mut curr = val;
    for i in (1..len).rev() {
        dst[i] = (curr & 0xFF) as u8;
        curr >>= 8;
    }
}

/// Decodes a numeric field supporting standard octal and GNU Base-256 binary formats.
///
/// Automatically inspects the first byte for the GNU Base-256 marker `0x80`.
/// Returns `0` on invalid or malformed data with zero panics.
pub fn numeric_extended_from(bytes: &[u8]) -> u64 {
    if bytes.is_empty() {
        return 0;
    }

    if (bytes[0] & 0x80) != 0 {
        // GNU base-256 binary encoding
        let mut val: u64 = (bytes[0] & 0x7F) as u64;
        for &b in &bytes[1..] {
            val = (val << 8) | (b as u64);
        }
        val
    } else {
        octal_from(bytes).unwrap_or(0)
    }
}

/// Encodes a `u64` into a destination slice using standard octal if it fits,
/// or GNU Base-256 binary format if `val` exceeds the octal capacity of `dst`.
pub fn numeric_extended_into(dst: &mut [u8], val: u64) {
    let len = dst.len();
    if len == 0 {
        return;
    }

    // Maximum octal value that can be represented with (len - 1) octal digits
    let octal_digits = len.saturating_sub(1);
    let max_octal = if octal_digits >= 22 {
        u64::MAX
    } else if octal_digits == 0 {
        0
    } else {
        (1u64.checked_shl((octal_digits * 3) as u32)).map(|v| v.saturating_sub(1)).unwrap_or(u64::MAX)
    };

    if val <= max_octal {
        octal_into(dst, val);
    } else {
        base256_into(dst, val);
    }
}

/// Extracts a null-trimmed UTF-8 string slice from a byte slice.
#[inline]
pub fn null_trimmed_str(bytes: &[u8]) -> &str {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    std::str::from_utf8(&bytes[..end]).unwrap_or("")
}
