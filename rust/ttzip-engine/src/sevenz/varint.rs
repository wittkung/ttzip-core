// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! High-performance, branchless 7-Zip (`Real_UINT64`) Variable-Length Integer (Varint) Codec
//! and 7z Property Tag (NID) namespace definitions.
//!
//! Provides zero-allocation, boundary-checked serialization and deserialization for
//! 7z variable-length unsigned integers (0 to 64-bit integer values) across memory buffers
//! and streaming I/O interfaces (`Read`/`Write`).

use std::io::{self, Read, Write};
use thiserror::Error;

// ============================================================================
// 7z Property Tags (NID Namespace Constants)
// ============================================================================

/// Marks the end of a header or property list (0x00).
pub const K_END: u8 = 0x00;
/// Archive header marker (0x01).
pub const K_HEADER: u8 = 0x01;
/// Archive properties metadata (0x02).
pub const K_ARCHIVE_PROPERTIES: u8 = 0x02;
/// Additional streams info (0x03).
pub const K_ADDITIONAL_STREAMS_INFO: u8 = 0x03;
/// Main streams information block (0x04).
pub const K_MAIN_STREAMS_INFO: u8 = 0x04;
/// Files metadata information block (0x05).
pub const K_FILES_INFO: u8 = 0x05;
/// Pack info block containing stream pack sizes and CRCs (0x06).
pub const K_PACK_INFO: u8 = 0x06;
/// Unpack info block containing folders, coders, and unpack sizes (0x07).
pub const K_UNPACK_INFO: u8 = 0x07;
/// Sub-streams info block containing unpack sizes and CRCs per file (0x08).
pub const K_SUB_STREAMS_INFO: u8 = 0x08;
/// Size property tag (0x09).
pub const K_SIZE: u8 = 0x09;
/// CRC-32 checksum property tag (0x0A).
pub const K_CRC: u8 = 0x0A;
/// Folder definition tag (0x0B).
pub const K_FOLDER: u8 = 0x0B;
/// Coders unpack size tag (0x0C).
pub const K_CODERS_UNPACK_SIZE: u8 = 0x0C;
/// Number of unpack streams per folder (0x0D).
pub const K_NUM_UNPACK_STREAM: u8 = 0x0D;
/// Empty stream flag bit-vector (0x0E).
pub const K_EMPTY_STREAM: u8 = 0x0E;
/// Empty file (0-byte file) flag bit-vector (0x0F).
pub const K_EMPTY_FILE: u8 = 0x0F;
/// Anti-item flag bit-vector for differential archives (0x10).
pub const K_ANTI: u8 = 0x10;
/// UTF-16LE file names vector (0x11).
pub const K_NAME: u8 = 0x11;
/// Windows FILETIME creation timestamp vector (0x12).
pub const K_CTIME: u8 = 0x12;
/// Windows FILETIME access timestamp vector (0x13).
pub const K_ATIME: u8 = 0x13;
/// Windows FILETIME modification timestamp vector (0x14).
pub const K_MTIME: u8 = 0x14;
/// Win32 file attribute flags vector (0x15).
pub const K_WIN_ATTRIBUTES: u8 = 0x15;
/// Comment property tag (0x16).
pub const K_COMMENT: u8 = 0x16;
/// Encoded header stream marker (0x17).
pub const K_ENCODED_HEADER: u8 = 0x17;
/// Start edit header marker (0x18).
pub const K_START_EDIT_HEADER: u8 = 0x18;
/// Dummy padding byte tag (0x19).
pub const K_DUMMY: u8 = 0x19;

// ============================================================================
// Varint Core Constants & Types
// ============================================================================

/// Maximum byte length required to store any 64-bit 7z Varint (1 marker byte + 8 payload bytes).
pub const MAX_VARINT_LEN_7Z: usize = 9;

/// Error conditions that can occur during 7z Varint encoding or decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum VarintError {
    /// Buffer reached EOF prematurely while decoding a multi-byte Varint.
    #[error("unexpected end of buffer while decoding 7z varint (needed {needed} bytes, have {available})")]
    UnexpectedEof {
        /// Expected byte length of the varint based on the prefix.
        needed: usize,
        /// Actual available bytes in the input buffer.
        available: usize,
    },
    /// Provided output buffer is too small to store the encoded varint.
    #[error("output buffer too small for 7z varint (needed {needed} bytes, have {available})")]
    BufferTooSmall {
        /// Minimum buffer size required for the value.
        needed: usize,
        /// Actual capacity of the provided output slice.
        available: usize,
    },
    /// Arithmetic overflow occurred during value reconstruction.
    #[error("varint arithmetic overflow during reconstruction")]
    Overflow,
}

/// Returns the exact byte length (1..=9) required to encode `val` as a 7z Varint.
#[inline(always)]
pub const fn varint_size_7z(val: u64) -> usize {
    if val < (1 << 7) {
        1
    } else if val < (1 << 14) {
        2
    } else if val < (1 << 21) {
        3
    } else if val < (1 << 28) {
        4
    } else if val < (1 << 35) {
        5
    } else if val < (1 << 42) {
        6
    } else if val < (1 << 49) {
        7
    } else if val < (1 << 56) {
        8
    } else {
        9
    }
}

/// Decodes a 7-Zip `Real_UINT64` variable-length integer from a byte slice.
///
/// Returns `Ok((value, bytes_consumed))` on success, or `Err(VarintError)` if the
/// buffer is truncated or malformed.
///
/// # Format Specification
/// - If first byte has 0 leading 1s (`0xxxxxxx`): 1 byte, `val = b0` (0..=127).
/// - If first byte has $k$ leading 1s ($1 \le k \le 7$): $1 + k$ bytes. The first byte
///   stores the top $(7 - k)$ bits masked by `(1 << (7 - k)) - 1`, followed by $k$
///   little-endian low-order bytes.
/// - If first byte is `0xFF` ($k = 8$ leading 1s): 9 bytes. The first byte is a marker
///   and the following 8 bytes represent a raw 64-bit integer in little-endian order.
#[inline]
pub fn decode_7z_varint(buf: &[u8]) -> Result<(u64, usize), VarintError> {
    if buf.is_empty() {
        return Err(VarintError::UnexpectedEof {
            needed: 1,
            available: 0,
        });
    }

    let first = buf[0];
    let k = first.leading_ones() as usize;

    if k == 0 {
        return Ok((first as u64, 1));
    }

    let total_len = 1 + k;
    if buf.len() < total_len {
        return Err(VarintError::UnexpectedEof {
            needed: total_len,
            available: buf.len(),
        });
    }

    if k == 8 {
        let mut raw = [0u8; 8];
        raw.copy_from_slice(&buf[1..9]);
        let val = u64::from_le_bytes(raw);
        return Ok((val, 9));
    }

    // 1 <= k <= 7
    let mask = (1u8 << (7 - k)) - 1;
    let high = ((first & mask) as u64) << (k * 8);

    let mut low_bytes = [0u8; 8];
    low_bytes[..k].copy_from_slice(&buf[1..total_len]);
    let low = u64::from_le_bytes(low_bytes);

    Ok((high | low, total_len))
}

/// Encodes a 64-bit unsigned integer into 7-Zip `Real_UINT64` varint format.
///
/// Returns the number of bytes written (1..=9).
///
/// # Panics
/// Panics if `out.len() < varint_size_7z(val)`. Use [`try_encode_7z_varint`] for a non-panicking variant.
#[inline]
pub fn encode_7z_varint(val: u64, out: &mut [u8]) -> usize {
    let size = varint_size_7z(val);
    assert!(
        out.len() >= size,
        "output buffer too small for 7z varint (needed {} bytes, have {})",
        size,
        out.len()
    );

    if size == 1 {
        out[0] = val as u8;
        return 1;
    }

    if size == 9 {
        out[0] = 0xFF;
        out[1..9].copy_from_slice(&val.to_le_bytes());
        return 9;
    }

    // 2 <= size <= 8 (k = size - 1, 1 <= k <= 7)
    let k = size - 1;
    let prefix = !((1u8 << (8 - k)) - 1);
    let high = (val >> (8 * k)) as u8;
    out[0] = prefix | high;
    out[1..size].copy_from_slice(&val.to_le_bytes()[..k]);
    size
}

/// Fallible encoder returning `Result<usize, VarintError>` if the output buffer is insufficient.
#[inline]
pub fn try_encode_7z_varint(val: u64, out: &mut [u8]) -> Result<usize, VarintError> {
    let size = varint_size_7z(val);
    if out.len() < size {
        return Err(VarintError::BufferTooSmall {
            needed: size,
            available: out.len(),
        });
    }
    Ok(encode_7z_varint(val, out))
}

/// Encodes a 7z varint and appends the serialized bytes to a `Vec<u8>`.
#[inline]
pub fn encode_7z_varint_vec(val: u64, out: &mut Vec<u8>) -> usize {
    let mut tmp = [0u8; MAX_VARINT_LEN_7Z];
    let written = encode_7z_varint(val, &mut tmp);
    out.extend_from_slice(&tmp[..written]);
    written
}

// ============================================================================
// Streaming I/O Primitives
// ============================================================================

/// Reads a 7-Zip variable-length integer (`Real_UINT64`) from a streaming reader.
///
/// Decodes 1 to 9 bytes according to the standard 7z Varint encoding rule.
/// Returns `io::ErrorKind::UnexpectedEof` if the stream terminates prematurely.
#[inline]
pub fn read_variable_u64<R: Read + ?Sized>(reader: &mut R) -> io::Result<u64> {
    let mut first_buf = [0u8; 1];
    reader.read_exact(&mut first_buf)?;
    let first = first_buf[0];
    let k = first.leading_ones() as usize;

    if k == 0 {
        return Ok(first as u64);
    }

    if k == 8 {
        let mut raw = [0u8; 8];
        reader.read_exact(&mut raw)?;
        return Ok(u64::from_le_bytes(raw));
    }

    // 1 <= k <= 7
    let mask = (1u8 << (7 - k)) - 1;
    let high = ((first & mask) as u64) << (k * 8);

    let mut low_bytes = [0u8; 8];
    reader.read_exact(&mut low_bytes[..k])?;
    let low = u64::from_le_bytes(low_bytes);

    Ok(high | low)
}

/// Writes a 64-bit integer into a streaming writer using the 7z Varint format.
///
/// Returns the number of bytes written (1 to 9).
#[inline]
pub fn write_variable_u64<W: Write + ?Sized>(writer: &mut W, val: u64) -> io::Result<usize> {
    let mut buf = [0u8; MAX_VARINT_LEN_7Z];
    let size = encode_7z_varint(val, &mut buf);
    writer.write_all(&buf[..size])?;
    Ok(size)
}

/// Reads a 7z variable-length integer as `usize`, safely clamping to `usize::MAX` on 32-bit platforms.
#[inline]
pub fn read_variable_usize<R: Read + ?Sized>(reader: &mut R) -> io::Result<usize> {
    let val = read_variable_u64(reader)?;
    Ok(val.min(usize::MAX as u64) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_basic_roundtrips() {
        let test_cases = [
            0u64,
            1,
            127,
            128,
            255,
            256,
            16383,
            16384,
            0x1FFFFF,
            0x200000,
            0x0FFFFFFF,
            0x10000000,
            0x00000007_FFFFFFFF,
            0x00000008_00000000,
            0x000003FF_FFFFFFFF,
            0x00000400_00000000,
            0x0001FFFF_FFFFFFFF,
            0x00020000_00000000,
            0x00FFFFFF_FFFFFFFF,
            0x01000000_00000000,
            0x7FFFFFFF_FFFFFFFF,
            0x80000000_00000000,
            u64::MAX,
        ];

        for &val in &test_cases {
            let expected_size = varint_size_7z(val);
            let mut buf = [0u8; MAX_VARINT_LEN_7Z];
            let written = encode_7z_varint(val, &mut buf);
            assert_eq!(written, expected_size);

            let (decoded, consumed) =
                decode_7z_varint(&buf[..written]).expect("decode must succeed");
            assert_eq!(decoded, val);
            assert_eq!(consumed, expected_size);
        }
    }

    #[test]
    fn test_streaming_io_primitives_roundtrip() {
        let test_cases = [
            0u64,
            1,
            127,
            128,
            16383,
            16384,
            0x1FFFFF,
            0x200000,
            u32::MAX as u64,
            0x01000000_00000000,
            u64::MAX,
        ];

        for &val in &test_cases {
            let mut stream = Vec::new();
            let written = write_variable_u64(&mut stream, val).expect("write_variable_u64 failed");
            assert_eq!(written, varint_size_7z(val));

            let mut cursor = std::io::Cursor::new(&stream);
            let decoded = read_variable_u64(&mut cursor).expect("read_variable_u64 failed");
            assert_eq!(decoded, val);
            assert_eq!(cursor.position() as usize, written);

            let mut cursor2 = std::io::Cursor::new(&stream);
            let decoded_usize = read_variable_usize(&mut cursor2).expect("read_variable_usize failed");
            assert_eq!(decoded_usize, val.min(usize::MAX as u64) as usize);
        }
    }

    #[test]
    fn test_nid_constants_values() {
        assert_eq!(K_END, 0x00);
        assert_eq!(K_HEADER, 0x01);
        assert_eq!(K_ARCHIVE_PROPERTIES, 0x02);
        assert_eq!(K_ADDITIONAL_STREAMS_INFO, 0x03);
        assert_eq!(K_MAIN_STREAMS_INFO, 0x04);
        assert_eq!(K_FILES_INFO, 0x05);
        assert_eq!(K_PACK_INFO, 0x06);
        assert_eq!(K_UNPACK_INFO, 0x07);
        assert_eq!(K_SUB_STREAMS_INFO, 0x08);
        assert_eq!(K_SIZE, 0x09);
        assert_eq!(K_CRC, 0x0A);
        assert_eq!(K_FOLDER, 0x0B);
        assert_eq!(K_CODERS_UNPACK_SIZE, 0x0C);
        assert_eq!(K_NUM_UNPACK_STREAM, 0x0D);
        assert_eq!(K_EMPTY_STREAM, 0x0E);
        assert_eq!(K_EMPTY_FILE, 0x0F);
        assert_eq!(K_ANTI, 0x10);
        assert_eq!(K_NAME, 0x11);
        assert_eq!(K_CTIME, 0x12);
        assert_eq!(K_ATIME, 0x13);
        assert_eq!(K_MTIME, 0x14);
        assert_eq!(K_WIN_ATTRIBUTES, 0x15);
        assert_eq!(K_COMMENT, 0x16);
        assert_eq!(K_ENCODED_HEADER, 0x17);
        assert_eq!(K_START_EDIT_HEADER, 0x18);
        assert_eq!(K_DUMMY, 0x19);
    }

    #[test]
    fn test_truncated_buffer_errors() {
        let mut buf = [0u8; MAX_VARINT_LEN_7Z];
        let written = encode_7z_varint(u64::MAX, &mut buf);
        assert_eq!(written, 9);

        // Test truncated inputs from 0..8 bytes
        for len in 0..written {
            let res = decode_7z_varint(&buf[..len]);
            assert!(
                matches!(res, Err(VarintError::UnexpectedEof { needed, available }) if needed == (if len == 0 { 1 } else { 9 }) && available == len),
                "expected unexpected eof error for truncated length {}",
                len
            );

            let mut cursor = std::io::Cursor::new(&buf[..len]);
            let stream_res = read_variable_u64(&mut cursor);
            assert!(stream_res.is_err());
            assert_eq!(stream_res.unwrap_err().kind(), std::io::ErrorKind::UnexpectedEof);
        }
    }

    #[test]
    fn test_buffer_too_small_error() {
        let mut tiny_buf = [0u8; 1];
        let res = try_encode_7z_varint(16384, &mut tiny_buf);
        assert_eq!(
            res,
            Err(VarintError::BufferTooSmall {
                needed: 3,
                available: 1
            })
        );
    }
}
