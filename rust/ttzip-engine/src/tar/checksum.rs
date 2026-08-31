// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! TAR header checksum calculation, dual-mode signed/unsigned verification,
//! and fault-tolerant parser state machine.
//!
//! Handles POSIX ustar standard unsigned checksums as well as historical SunOS / BSD
//! signed-char overflow quirks where bytes >= 0x80 were sign-extended during addition.

use thiserror::Error;

/// Offset of the 8-byte checksum field within a standard 512-byte TAR header.
pub const CHKSUM_OFFSET: usize = 148;

/// Length of the checksum field in bytes.
pub const CHKSUM_LEN: usize = 8;

/// Error variants encountered during TAR header checksum validation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TarChecksumError {
    /// The provided byte buffer is shorter than the required 512-byte TAR block.
    #[error("truncated header block: expected 512 bytes, found {found}")]
    TruncatedHeader { found: usize },

    /// The checksum field does not contain valid octal digits or expected delimiters.
    #[error("invalid octal checksum sequence in header: raw bytes {raw:?}")]
    InvalidOctal { raw: [u8; CHKSUM_LEN] },

    /// The header block checksum does not match either unsigned or signed computation.
    #[error("checksum mismatch: expected {expected:#o} ({expected}), calculated unsigned {actual_unsigned:#o} ({actual_unsigned}), calculated signed {actual_signed:#o} ({actual_signed})")]
    Mismatch {
        expected: u32,
        actual_unsigned: u32,
        actual_signed: i32,
    },
}

/// Computes the standard unsigned TAR checksum over a 512-byte header block.
///
/// The 8 bytes at indices `148..156` (`CHKSUM_OFFSET..CHKSUM_OFFSET + CHKSUM_LEN`)
/// are treated as ASCII spaces (`0x20`), and every byte of the 512-byte block is
/// accumulated as an unsigned 32-bit integer.
#[inline]
pub fn calculate_unsigned_checksum(header_512: &[u8; 512]) -> u32 {
    let mut sum: u32 = 0;
    for (i, &byte) in header_512.iter().enumerate() {
        let val = if (CHKSUM_OFFSET..CHKSUM_OFFSET + CHKSUM_LEN).contains(&i) {
            0x20u8
        } else {
            byte
        };
        sum += val as u32;
    }
    sum
}

/// Computes the signed TAR checksum over a 512-byte header block.
///
/// Historical implementations on SunOS and BSD (C compilers where `char` defaults to `signed char`)
/// accumulated header bytes with sign extension. Bytes `>= 0x80` become negative values (`-128..=-1`),
/// producing a different checksum for non-ASCII headers.
#[inline]
pub fn calculate_signed_checksum(header_512: &[u8; 512]) -> i32 {
    let mut sum: i32 = 0;
    for (i, &byte) in header_512.iter().enumerate() {
        let val = if (CHKSUM_OFFSET..CHKSUM_OFFSET + CHKSUM_LEN).contains(&i) {
            0x20u8
        } else {
            byte
        };
        sum += (val as i8) as i32;
    }
    sum
}

/// Parses the 8-byte checksum field from a TAR header block.
///
/// Supports standard formats:
/// - 6 octal digits + NUL + Space
/// - 6 octal digits + Space + NUL
/// - 7 octal digits + Space / NUL
/// - Leading and trailing whitespace / null padding
pub fn parse_checksum_field(raw: &[u8; CHKSUM_LEN]) -> Result<u32, TarChecksumError> {
    let mut slice = &raw[..];
    // Trim leading whitespace and nulls
    while !slice.is_empty() && (slice[0] == b' ' || slice[0] == 0) {
        slice = &slice[1..];
    }
    // Trim trailing whitespace and nulls
    while !slice.is_empty() && (slice[slice.len() - 1] == b' ' || slice[slice.len() - 1] == 0) {
        slice = &slice[..slice.len() - 1];
    }

    if slice.is_empty() {
        return Ok(0);
    }

    let mut result: u32 = 0;
    for &b in slice {
        if !(b'0'..=b'7').contains(&b) {
            return Err(TarChecksumError::InvalidOctal { raw: *raw });
        }
        match result.checked_mul(8).and_then(|r| r.checked_add((b - b'0') as u32)) {
            Some(next) => result = next,
            None => return Err(TarChecksumError::InvalidOctal { raw: *raw }),
        }
    }

    Ok(result)
}

/// Verifies whether the TAR header block contains a valid checksum.
///
/// It first attempts to match the header's recorded checksum against the standard unsigned checksum.
/// If that does not match, it falls back to matching against the historical signed checksum.
/// Returns `Ok(checksum)` on success, or `Err(TarChecksumError)` on validation failure.
pub fn verify_header_checksum(header_512: &[u8; 512]) -> Result<u32, TarChecksumError> {
    let raw_field: [u8; CHKSUM_LEN] = {
        let mut arr = [0u8; CHKSUM_LEN];
        arr.copy_from_slice(&header_512[CHKSUM_OFFSET..CHKSUM_OFFSET + CHKSUM_LEN]);
        arr
    };

    let expected = parse_checksum_field(&raw_field)?;
    let actual_unsigned = calculate_unsigned_checksum(header_512);
    let actual_signed = calculate_signed_checksum(header_512);

    if expected == actual_unsigned {
        Ok(actual_unsigned)
    } else if (expected as i32) == actual_signed {
        Ok(expected)
    } else {
        Err(TarChecksumError::Mismatch {
            expected,
            actual_unsigned,
            actual_signed,
        })
    }
}

/// Verifies a byte slice as a TAR header checksum block.
///
/// If `slice` is shorter than 512 bytes, returns `Err(TarChecksumError::TruncatedHeader)`.
/// Otherwise, validates the first 512 bytes using `verify_header_checksum`.
pub fn verify_header_checksum_slice(slice: &[u8]) -> Result<u32, TarChecksumError> {
    if slice.len() < 512 {
        return Err(TarChecksumError::TruncatedHeader {
            found: slice.len(),
        });
    }

    let mut header_512 = [0u8; 512];
    header_512.copy_from_slice(&slice[..512]);
    verify_header_checksum(&header_512)
}

/// Writes the standard unsigned checksum into the 8-byte checksum field of a 512-byte header.
///
/// Formats the checksum as 6 ASCII octal digits (zero-padded), followed by a NUL byte and an ASCII space.
/// Example: `001234\0 ` (standard POSIX ustar format).
pub fn write_header_checksum(header_512: &mut [u8; 512]) {
    // Fill checksum field with spaces to prepare for calculation
    header_512[CHKSUM_OFFSET..CHKSUM_OFFSET + CHKSUM_LEN].fill(b' ');

    let checksum = calculate_unsigned_checksum(header_512);

    // Format as 6 octal digits + \0 + ' '
    let mut formatted = [b'0'; 6];
    let mut val = checksum;
    for i in (0..6).rev() {
        formatted[i] = b'0' + (val % 8) as u8;
        val /= 8;
    }

    header_512[CHKSUM_OFFSET..CHKSUM_OFFSET + 6].copy_from_slice(&formatted);
    header_512[CHKSUM_OFFSET + 6] = 0;
    header_512[CHKSUM_OFFSET + 7] = b' ';
}
