// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 7-Zip (7z) binary format definitions, Varint encoding/decoding, and Signature Header.

use crate::crypto::crc32::crc32_fast;
use crate::types::TTZipStatus;

pub const SEVENZ_SIGNATURE: [u8; 6] = [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]; // '7', 'z', 0xBC, 0xAF, 0x27, 0x1C

// 7z Property Tags (NIDs re-exported from varint module)
pub use crate::sevenz::varint::{
    K_ADDITIONAL_STREAMS_INFO, K_ANTI, K_ARCHIVE_PROPERTIES, K_ATIME, K_CODERS_UNPACK_SIZE,
    K_COMMENT, K_CRC, K_CTIME, K_DUMMY, K_EMPTY_FILE, K_EMPTY_STREAM, K_ENCODED_HEADER,
    K_END, K_FILES_INFO, K_FOLDER, K_HEADER, K_MAIN_STREAMS_INFO, K_MTIME, K_NAME,
    K_NUM_UNPACK_STREAM, K_PACK_INFO, K_SIZE, K_START_EDIT_HEADER, K_SUB_STREAMS_INFO,
    K_UNPACK_INFO, K_WIN_ATTRIBUTES,
};

// 7z Coder Method IDs
pub const METHOD_COPY: u64 = 0x00;
pub const METHOD_DELTA: u64 = 0x03;
pub const METHOD_ARM64: u64 = 0x0A;
pub const METHOD_LZMA2: u64 = 0x21;
pub const METHOD_LZMA: u64 = 0x030101;
pub const METHOD_BCJ_X86: u64 = 0x03030103;
pub const METHOD_BCJ2: u64 = 0x0303011B;
pub const METHOD_ARM64_ALT: u64 = 0x03030701;
pub const METHOD_PPMD: u64 = 0x030401;
pub const METHOD_DEFLATE: u64 = 0x040108;
pub const METHOD_BZIP2: u64 = 0x040202;
pub const METHOD_BROTLI: u64 = 0x04F71102;
pub const METHOD_LZ4: u64 = 0x04F71104;
pub const METHOD_AES: u64 = 0x06F10701;

/// Parsed 7z Signature Header (32 bytes at offset 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SevenZSignatureHeader {
    pub major_version: u8,
    pub minor_version: u8,
    pub start_header_crc: u32,
    pub next_header_offset: u64,
    pub next_header_size: u64,
    pub next_header_crc: u32,
}

impl SevenZSignatureHeader {
    /// Parses the 32-byte 7z signature header from mapped buffer.
    pub fn parse(mapped: &[u8]) -> Result<Self, TTZipStatus> {
        if mapped.len() < 32 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        if mapped[0..6] != SEVENZ_SIGNATURE {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let major_version = mapped[6];
        let minor_version = mapped[7];
        let start_header_crc = u32::from_le_bytes(mapped[8..12].try_into().unwrap());
        let next_header_offset = u64::from_le_bytes(mapped[12..20].try_into().unwrap());
        let next_header_size = u64::from_le_bytes(mapped[20..28].try_into().unwrap());
        let next_header_crc = u32::from_le_bytes(mapped[28..32].try_into().unwrap());

        // Validate start header CRC (over bytes 12..32)
        let computed_start_crc = crc32_fast(0, &mapped[12..32]);
        if computed_start_crc != start_header_crc {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        Ok(Self {
            major_version,
            minor_version,
            start_header_crc,
            next_header_offset,
            next_header_size,
            next_header_crc,
        })
    }

    /// Serializes 32-byte signature header with accurate start_header_crc.
    pub fn serialize(&self) -> [u8; 32] {
        let mut header = [0u8; 32];
        header[0..6].copy_from_slice(&SEVENZ_SIGNATURE);
        header[6] = self.major_version;
        header[7] = self.minor_version;

        header[12..20].copy_from_slice(&self.next_header_offset.to_le_bytes());
        header[20..28].copy_from_slice(&self.next_header_size.to_le_bytes());
        header[28..32].copy_from_slice(&self.next_header_crc.to_le_bytes());

        let start_crc = crc32_fast(0, &header[12..32]);
        header[8..12].copy_from_slice(&start_crc.to_le_bytes());

        header
    }
}

pub use crate::sevenz::varint::{
    decode_7z_varint, encode_7z_varint, encode_7z_varint_vec, try_encode_7z_varint,
    varint_size_7z, VarintError, MAX_VARINT_LEN_7Z,
};

/// Reads a 7z variable-length integer (Varint) from slice.
/// Returns `(value, bytes_consumed)` or `None` if slice is too short or invalid.
#[inline]
pub fn read_varint(buf: &[u8]) -> Option<(u64, usize)> {
    decode_7z_varint(buf).ok()
}

/// Writes a 7z variable-length integer (Varint) to a vector.
#[inline]
pub fn write_varint(val: u64, out: &mut Vec<u8>) {
    encode_7z_varint_vec(val, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_7z_varint_roundtrip() {
        let test_values = [
            0u64, 1, 127, 128, 255, 256, 16383, 16384, 65535, 65536,
            0xFFFFFF, 0xFFFFFFFF, 0x123456789ABCDEF0,
        ];

        for &val in &test_values {
            let mut encoded = Vec::new();
            write_varint(val, &mut encoded);
            assert!(!encoded.is_empty());

            let (decoded, consumed) = read_varint(&encoded).expect("varint decode failed");
            assert_eq!(decoded, val);
            assert_eq!(consumed, encoded.len());
        }
    }

    #[test]
    fn test_7z_signature_header_serialize_and_parse() {
        let sig = SevenZSignatureHeader {
            major_version: 0,
            minor_version: 4,
            start_header_crc: 0, // Calculated automatically
            next_header_offset: 1024,
            next_header_size: 256,
            next_header_crc: 0x12345678,
        };

        let raw = sig.serialize();
        let parsed = SevenZSignatureHeader::parse(&raw).expect("parse sig header failed");
        assert_eq!(parsed.major_version, 0);
        assert_eq!(parsed.minor_version, 4);
        assert_eq!(parsed.next_header_offset, 1024);
        assert_eq!(parsed.next_header_size, 256);
        assert_eq!(parsed.next_header_crc, 0x12345678);
    }
}
