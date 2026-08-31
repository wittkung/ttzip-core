// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! High-performance, branchless XZ Variable-Length Integer (VLI) Codec
//! and 64-bit `VLI_MAX` overflow circuit breaker.
//!
//! Provides zero-allocation, boundary-checked serialization and deserialization for
//! XZ variable-length unsigned integers (0 to $2^{63}-1$) across memory buffers
//! and streaming I/O interfaces (`Read`/`Write`).

use std::io::{self, Read, Write};
use thiserror::Error;

/// Maximum valid numerical value representable by an XZ VLI integer ($2^{63} - 1$).
pub const XZ_VLI_MAX: u64 = 0x7FFF_FFFF_FFFF_FFFF;

/// Alias for [`XZ_VLI_MAX`] for backward compatibility.
pub const VLI_MAX: u64 = XZ_VLI_MAX;

/// Special sentinel value indicating an unknown or unspecified VLI size (`u64::MAX`).
pub const XZ_VLI_UNKNOWN: u64 = u64::MAX;

/// Maximum byte length required to store any valid 64-bit XZ VLI integer (9 bytes).
pub const XZ_VLI_BYTES_MAX: usize = 9;

/// Alias for [`XZ_VLI_BYTES_MAX`] for backward compatibility.
pub const VLI_MAX_BYTES: usize = XZ_VLI_BYTES_MAX;

/// Error conditions that can occur during XZ VLI encoding or decoding operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum XzVliError {
    /// Integer value exceeds the maximum allowable XZ VLI limit (`XZ_VLI_MAX` = $2^{63}-1$).
    #[error("VLI value {val:#X} exceeds maximum allowed value XZ_VLI_MAX (0x7FFF_FFFF_FFFF_FFFF)")]
    ValueTooLarge {
        /// The offending integer value.
        val: u64,
    },

    /// Buffer reached EOF prematurely while decoding a multi-byte VLI sequence.
    #[error("unexpected end of buffer while decoding VLI (needed at least {needed} additional bytes, have {available})")]
    UnexpectedEof {
        /// Minimum additional bytes needed.
        needed: usize,
        /// Actual remaining bytes available.
        available: usize,
    },

    /// Provided output buffer is too small to store the serialized VLI sequence.
    #[error("output buffer too small for VLI encoding (needed {needed} bytes, have {available})")]
    BufferTooSmall {
        /// Minimum buffer size required for the value.
        needed: usize,
        /// Actual capacity of the provided output slice.
        available: usize,
    },

    /// VLI sequence exceeds the maximum allowed length of 9 bytes or high bit is set on byte 9.
    #[error("VLI sequence exceeds maximum allowed byte length of 9 bytes")]
    SequenceTooLong,

    /// Non-canonical multi-byte VLI encoding detected (e.g. leading zero payload).
    #[error("non-canonical multi-byte VLI encoding detected: byte at index {byte_index} has value 0x00")]
    NonCanonical {
        /// 0-based index of the invalid terminating byte.
        byte_index: usize,
    },

    /// General I/O error encountered during streaming decode/encode.
    #[error("VLI I/O error: {0}")]
    Io(String),
}

/// Backward-compatible type alias for [`XzVliError`].
pub type VliError = XzVliError;

impl From<io::Error> for XzVliError {
    fn from(err: io::Error) -> Self {
        if err.kind() == io::ErrorKind::UnexpectedEof {
            XzVliError::UnexpectedEof {
                needed: 1,
                available: 0,
            }
        } else {
            XzVliError::Io(err.to_string())
        }
    }
}

/// Computes the exact byte length (1..=9) required to encode `val` as an XZ VLI.
///
/// Uses an $O(1)$ branchless formula leveraging hardware leading zero count (`CLZ`).
///
/// # Errors
/// Returns [`XzVliError::ValueTooLarge`] if `val > XZ_VLI_MAX`.
#[inline(always)]
pub const fn vli_size(val: u64) -> Result<usize, XzVliError> {
    if val > XZ_VLI_MAX {
        return Err(XzVliError::ValueTooLarge { val });
    }
    let bits = 64 - if val == 0 { 63 } else { val.leading_zeros() } as usize;
    Ok(bits.div_ceil(7))
}

/// Decodes an XZ Variable-Length Integer (VLI) from a byte slice starting at `*pos`.
///
/// Updates `*pos` to point after the consumed bytes upon successful decode.
/// If an error occurs, `*pos` remains unchanged.
///
/// # Defense Gates
/// 1. **Truncation / Exceeding Max Bytes**: Rejects sequences longer than 9 bytes.
/// 2. **9th Byte High-Bit Overflow**: Enforces that the 9th byte has bit 7 cleared (`0x80 == 0`).
/// 3. **Non-Canonical Zero Padding**: Rejects multi-byte encodings whose terminating byte is `0x00`.
///
/// # Errors
/// - [`XzVliError::UnexpectedEof`] if the buffer ends prematurely.
/// - [`XzVliError::SequenceTooLong`] if the sequence exceeds 9 bytes.
/// - [`XzVliError::NonCanonical`] if a multi-byte sequence contains a trailing zero payload.
#[inline]
pub fn decode_vli(input: &[u8], pos: &mut usize) -> Result<u64, XzVliError> {
    let start_pos = *pos;
    if start_pos >= input.len() {
        return Err(XzVliError::UnexpectedEof {
            needed: 1,
            available: 0,
        });
    }

    let mut val = 0u64;

    for i in 0..XZ_VLI_BYTES_MAX {
        let curr_idx = start_pos + i;
        if curr_idx >= input.len() {
            return Err(XzVliError::UnexpectedEof {
                needed: 1,
                available: input.len().saturating_sub(start_pos),
            });
        }

        let b = input[curr_idx];

        if i == XZ_VLI_BYTES_MAX - 1 && (b & 0x80 != 0) {
            return Err(XzVliError::SequenceTooLong);
        }

        val |= ((b & 0x7F) as u64) << (i * 7);

        if b & 0x80 == 0 {
            if i > 0 && b == 0x00 {
                return Err(XzVliError::NonCanonical { byte_index: i });
            }
            *pos = curr_idx + 1;
            return Ok(val);
        }
    }

    Err(XzVliError::SequenceTooLong)
}

/// Reads and decodes an XZ VLI integer from a streaming reader.
///
/// Consumes 1 to 9 bytes from `reader`.
///
/// # Errors
/// - [`XzVliError::UnexpectedEof`] or [`XzVliError::Io`] if reading from `reader` fails.
/// - [`XzVliError::SequenceTooLong`] if the sequence exceeds 9 bytes.
/// - [`XzVliError::NonCanonical`] if a multi-byte sequence contains a trailing zero payload.
#[inline]
pub fn decode_vli_stream<R: Read + ?Sized>(reader: &mut R) -> Result<u64, XzVliError> {
    let mut val = 0u64;
    let mut byte_buf = [0u8; 1];

    for i in 0..XZ_VLI_BYTES_MAX {
        reader.read_exact(&mut byte_buf)?;
        let b = byte_buf[0];

        if i == XZ_VLI_BYTES_MAX - 1 && (b & 0x80 != 0) {
            return Err(XzVliError::SequenceTooLong);
        }

        val |= ((b & 0x7F) as u64) << (i * 7);

        if b & 0x80 == 0 {
            if i > 0 && b == 0x00 {
                return Err(XzVliError::NonCanonical { byte_index: i });
            }
            return Ok(val);
        }
    }

    Err(XzVliError::SequenceTooLong)
}

/// Encodes a 64-bit unsigned integer into the XZ VLI format within the given byte buffer starting at `pos`.
///
/// Updates `*pos` to point to the byte immediately following the encoded VLI.
/// Returns the number of bytes written (1..=9) on success.
///
/// # Errors
/// - [`XzVliError::ValueTooLarge`] if `val > XZ_VLI_MAX`.
/// - [`XzVliError::BufferTooSmall`] if the remaining buffer starting at `*pos` is insufficient.
#[inline]
pub fn encode_vli(val: u64, out: &mut [u8], pos: &mut usize) -> Result<usize, XzVliError> {
    let size = vli_size(val)?;
    let start_pos = *pos;
    let available = out.len().saturating_sub(start_pos);
    if available < size {
        return Err(XzVliError::BufferTooSmall {
            needed: size,
            available,
        });
    }

    let mut v = val;
    let mut curr = start_pos;
    while v >= 0x80 {
        out[curr] = (v as u8 & 0x7F) | 0x80;
        curr += 1;
        v >>= 7;
    }
    out[curr] = v as u8 & 0x7F;
    curr += 1;

    *pos = curr;
    Ok(curr - start_pos)
}

/// Encodes an integer value into a newly allocated `Vec<u8>`.
///
/// # Errors
/// - [`XzVliError::ValueTooLarge`] if `val > XZ_VLI_MAX`.
#[inline]
pub fn encode_vli_vec(val: u64) -> Result<Vec<u8>, XzVliError> {
    let size = vli_size(val)?;
    let mut buf = vec![0u8; size];
    let mut pos = 0;
    encode_vli(val, &mut buf, &mut pos)?;
    Ok(buf)
}

/// Writes an XZ VLI integer directly to a streaming writer.
///
/// Returns the number of bytes written (1..=9).
///
/// # Errors
/// - [`XzVliError::ValueTooLarge`] if `val > XZ_VLI_MAX`.
/// - [`XzVliError::Io`] if the writer fails.
#[inline]
pub fn encode_vli_stream<W: Write + ?Sized>(val: u64, writer: &mut W) -> Result<usize, XzVliError> {
    let mut buf = [0u8; XZ_VLI_BYTES_MAX];
    let mut pos = 0;
    encode_vli(val, &mut buf, &mut pos)?;
    writer.write_all(&buf[..pos]).map_err(|e| XzVliError::Io(e.to_string()))?;
    Ok(pos)
}
