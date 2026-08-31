// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! PKWARE Data Descriptor 16-byte / 24-byte streaming write & parse pipeline.
//!
//! Provides support for non-seekable streaming ZIP writes where CRC-32 and file sizes
//! are unknown upfront and emitted as trailing descriptors (General Purpose Bit 3).
//! Integrates ZipCrypto check byte linkage (DOS time high byte in streaming mode vs CRC-32 high byte).

use std::io::{self, Read, Write};

/// PKWARE Data Descriptor signature (`0x08074B50` / `PK\x07\x08`).
pub const MAGIC_DATA_DESCRIPTOR: u32 = 0x08074B50;

/// General Purpose Bit Flag: Bit 3 = 1 indicates CRC-32 & sizes in Data Descriptor.
pub const FLAG_DATA_DESCRIPTOR: u16 = 0x0008;

/// General Purpose Bit Flag: Bit 0 = 1 indicates encryption enabled.
pub const FLAG_ENCRYPTED: u16 = 0x0001;

/// General Purpose Bit Flag: Bit 11 = 1 indicates UTF-8 encoded filename and comments.
pub const FLAG_UTF8: u16 = 0x0800;

/// Standard 16-byte PKWARE Data Descriptor.
///
/// Binary Layout:
/// - `magic`: 4 bytes (`0x08074B50`)
/// - `crc32`: 4 bytes
/// - `compressed_size`: 4 bytes
/// - `uncompressed_size`: 4 bytes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZipDataDescriptor32 {
    pub magic: u32,
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
}

impl ZipDataDescriptor32 {
    /// Creates a new 32-bit Data Descriptor with standard magic signature.
    #[inline]
    pub const fn new(crc32: u32, compressed_size: u32, uncompressed_size: u32) -> Self {
        Self {
            magic: MAGIC_DATA_DESCRIPTOR,
            crc32,
            compressed_size,
            uncompressed_size,
        }
    }

    /// Serializes to a 16-byte little-endian array.
    #[inline]
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut out = [0u8; 16];
        out[0..4].copy_from_slice(&self.magic.to_le_bytes());
        out[4..8].copy_from_slice(&self.crc32.to_le_bytes());
        out[8..12].copy_from_slice(&self.compressed_size.to_le_bytes());
        out[12..16].copy_from_slice(&self.uncompressed_size.to_le_bytes());
        out
    }

    /// Deserializes from a 16-byte slice.
    pub fn from_bytes(bytes: &[u8; 16]) -> io::Result<Self> {
        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if magic != MAGIC_DATA_DESCRIPTOR {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid Data Descriptor magic signature",
            ));
        }
        let crc32 = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let compressed_size = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let uncompressed_size = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);

        Ok(Self {
            magic,
            crc32,
            compressed_size,
            uncompressed_size,
        })
    }

    /// Writes the 16-byte descriptor to a writer stream.
    #[inline]
    pub fn write<W: Write>(&self, writer: &mut W) -> io::Result<usize> {
        writer.write_all(&self.to_bytes())?;
        Ok(16)
    }

    /// Reads the 16-byte descriptor from a reader stream.
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut buf = [0u8; 16];
        reader.read_exact(&mut buf)?;
        Self::from_bytes(&buf)
    }
}

/// 24-byte Zip64 PKWARE Data Descriptor.
///
/// Binary Layout:
/// - `magic`: 4 bytes (`0x08074B50`)
/// - `crc32`: 4 bytes
/// - `compressed_size`: 8 bytes
/// - `uncompressed_size`: 8 bytes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZipDataDescriptor64 {
    pub magic: u32,
    pub crc32: u32,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
}

impl ZipDataDescriptor64 {
    /// Creates a new 64-bit Zip64 Data Descriptor with standard magic signature.
    #[inline]
    pub const fn new(crc32: u32, compressed_size: u64, uncompressed_size: u64) -> Self {
        Self {
            magic: MAGIC_DATA_DESCRIPTOR,
            crc32,
            compressed_size,
            uncompressed_size,
        }
    }

    /// Serializes to a 24-byte little-endian array.
    #[inline]
    pub fn to_bytes(&self) -> [u8; 24] {
        let mut out = [0u8; 24];
        out[0..4].copy_from_slice(&self.magic.to_le_bytes());
        out[4..8].copy_from_slice(&self.crc32.to_le_bytes());
        out[8..16].copy_from_slice(&self.compressed_size.to_le_bytes());
        out[16..24].copy_from_slice(&self.uncompressed_size.to_le_bytes());
        out
    }

    /// Deserializes from a 24-byte slice.
    pub fn from_bytes(bytes: &[u8; 24]) -> io::Result<Self> {
        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if magic != MAGIC_DATA_DESCRIPTOR {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid Zip64 Data Descriptor magic signature",
            ));
        }
        let crc32 = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let compressed_size = u64::from_le_bytes([
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ]);
        let uncompressed_size = u64::from_le_bytes([
            bytes[16], bytes[17], bytes[18], bytes[19], bytes[20], bytes[21], bytes[22], bytes[23],
        ]);

        Ok(Self {
            magic,
            crc32,
            compressed_size,
            uncompressed_size,
        })
    }

    /// Writes the 24-byte descriptor to a writer stream.
    #[inline]
    pub fn write<W: Write>(&self, writer: &mut W) -> io::Result<usize> {
        writer.write_all(&self.to_bytes())?;
        Ok(24)
    }

    /// Reads the 24-byte descriptor from a reader stream.
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut buf = [0u8; 24];
        reader.read_exact(&mut buf)?;
        Self::from_bytes(&buf)
    }
}

/// Writes a PKWARE Data Descriptor (16-byte standard or 24-byte Zip64) to a writer stream.
pub fn write_data_descriptor<W: Write>(
    writer: &mut W,
    crc32: u32,
    comp_size: u64,
    uncomp_size: u64,
    force_zip64: bool,
) -> io::Result<usize> {
    let is_zip64 = force_zip64 || comp_size >= 0xFFFF_FFFF || uncomp_size >= 0xFFFF_FFFF;
    if is_zip64 {
        let desc = ZipDataDescriptor64::new(crc32, comp_size, uncomp_size);
        desc.write(writer)
    } else {
        let desc = ZipDataDescriptor32::new(crc32, comp_size as u32, uncomp_size as u32);
        desc.write(writer)
    }
}

/// Parses a PKWARE Data Descriptor from a reader stream, handling optional magic and Zip64.
///
/// Returns `(crc32, compressed_size, uncompressed_size)`.
pub fn parse_data_descriptor<R: Read>(reader: &mut R, is_zip64: bool) -> io::Result<(u32, u64, u64)> {
    let mut head = [0u8; 4];
    reader.read_exact(&mut head)?;
    let sig = u32::from_le_bytes(head);

    let crc32 = if sig == MAGIC_DATA_DESCRIPTOR {
        let mut crc_buf = [0u8; 4];
        reader.read_exact(&mut crc_buf)?;
        u32::from_le_bytes(crc_buf)
    } else {
        // Optional magic signature omitted in some legacy encoders: first 4 bytes is CRC-32
        sig
    };

    if is_zip64 {
        let mut size_buf = [0u8; 16];
        reader.read_exact(&mut size_buf)?;
        let comp_size = u64::from_le_bytes([
            size_buf[0], size_buf[1], size_buf[2], size_buf[3],
            size_buf[4], size_buf[5], size_buf[6], size_buf[7],
        ]);
        let uncomp_size = u64::from_le_bytes([
            size_buf[8], size_buf[9], size_buf[10], size_buf[11],
            size_buf[12], size_buf[13], size_buf[14], size_buf[15],
        ]);
        Ok((crc32, comp_size, uncomp_size))
    } else {
        let mut size_buf = [0u8; 8];
        reader.read_exact(&mut size_buf)?;
        let comp_size = u32::from_le_bytes([size_buf[0], size_buf[1], size_buf[2], size_buf[3]]) as u64;
        let uncomp_size = u32::from_le_bytes([size_buf[4], size_buf[5], size_buf[6], size_buf[7]]) as u64;
        Ok((crc32, comp_size, uncomp_size))
    }
}

/// Builds a PKWARE Data Descriptor tail block as a byte vector.
pub fn build_data_descriptor(
    crc32: u32,
    compressed_size: u64,
    uncompressed_size: u64,
    is_zip64: bool,
) -> Vec<u8> {
    if is_zip64 || compressed_size >= 0xFFFF_FFFF || uncompressed_size >= 0xFFFF_FFFF {
        ZipDataDescriptor64::new(crc32, compressed_size, uncompressed_size).to_bytes().to_vec()
    } else {
        ZipDataDescriptor32::new(crc32, compressed_size as u32, uncompressed_size as u32).to_bytes().to_vec()
    }
}

/// Injects General Purpose Bit 3 (`FLAG_DATA_DESCRIPTOR`) and Bit 11 (`FLAG_UTF8`) into flags.
#[inline]
pub const fn inject_data_descriptor_flag(base_flag: u16) -> u16 {
    base_flag | FLAG_DATA_DESCRIPTOR | FLAG_UTF8
}

/// Checks if General Purpose Bit 3 (`FLAG_DATA_DESCRIPTOR`) is enabled.
#[inline]
pub const fn has_data_descriptor(flag: u16) -> bool {
    (flag & FLAG_DATA_DESCRIPTOR) != 0
}

/// Computes the 12th byte check byte for ZipCrypto encryption headers according to PKWARE spec.
///
/// - When Bit 3 Data Descriptor is used (`bit3_data_descriptor == true`):
///   The CRC-32 is not known before streaming payload, so the high byte of MS-DOS
///   last modification time `(dos_time >> 8) as u8` is used as verification byte.
/// - When standard seeking mode is used (`bit3_data_descriptor == false`):
///   The high byte of CRC-32 `(crc32 >> 24) as u8` is used as verification byte.
#[inline]
pub const fn compute_zipcrypto_check_byte(crc32: u32, dos_time: u16, bit3_data_descriptor: bool) -> u8 {
    if bit3_data_descriptor {
        ((dos_time >> 8) & 0xFF) as u8
    } else {
        ((crc32 >> 24) & 0xFF) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zip_data_descriptor_32_roundtrip() {
        let desc = ZipDataDescriptor32::new(0x12345678, 1024, 4096);
        let bytes = desc.to_bytes();
        assert_eq!(bytes.len(), 16);

        let parsed = ZipDataDescriptor32::from_bytes(&bytes).expect("parse failed");
        assert_eq!(desc, parsed);

        let mut cursor = io::Cursor::new(bytes);
        let read_desc = ZipDataDescriptor32::read(&mut cursor).expect("read failed");
        assert_eq!(desc, read_desc);
    }

    #[test]
    fn test_zip_data_descriptor_64_roundtrip() {
        let desc = ZipDataDescriptor64::new(0x87654321, 0x1_0000_1000, 0x2_0000_2000);
        let bytes = desc.to_bytes();
        assert_eq!(bytes.len(), 24);

        let parsed = ZipDataDescriptor64::from_bytes(&bytes).expect("parse failed");
        assert_eq!(desc, parsed);

        let mut cursor = io::Cursor::new(bytes);
        let read_desc = ZipDataDescriptor64::read(&mut cursor).expect("read failed");
        assert_eq!(desc, read_desc);
    }

    #[test]
    fn test_write_and_parse_data_descriptor_with_and_without_magic() {
        // 32-bit with magic
        let mut buf32 = Vec::new();
        let written = write_data_descriptor(&mut buf32, 0xDEADBEEF, 500, 1000, false).unwrap();
        assert_eq!(written, 16);
        let (crc, comp, uncomp) = parse_data_descriptor(&mut io::Cursor::new(&buf32), false).unwrap();
        assert_eq!(crc, 0xDEADBEEF);
        assert_eq!(comp, 500);
        assert_eq!(uncomp, 1000);

        // 64-bit with magic
        let mut buf64 = Vec::new();
        let written64 = write_data_descriptor(&mut buf64, 0xCAFEBABE, 0x10_0000_0000, 0x20_0000_0000, true).unwrap();
        assert_eq!(written64, 24);
        let (crc, comp, uncomp) = parse_data_descriptor(&mut io::Cursor::new(&buf64), true).unwrap();
        assert_eq!(crc, 0xCAFEBABE);
        assert_eq!(comp, 0x10_0000_0000);
        assert_eq!(uncomp, 0x20_0000_0000);

        // Without magic signature (legacy stream)
        let mut raw_buf = Vec::new();
        raw_buf.extend_from_slice(&0xAABBCCDDu32.to_le_bytes());
        raw_buf.extend_from_slice(&123u32.to_le_bytes());
        raw_buf.extend_from_slice(&456u32.to_le_bytes());
        let (crc, comp, uncomp) = parse_data_descriptor(&mut io::Cursor::new(&raw_buf), false).unwrap();
        assert_eq!(crc, 0xAABBCCDD);
        assert_eq!(comp, 123);
        assert_eq!(uncomp, 456);
    }

    #[test]
    fn test_zipcrypto_check_byte_computation() {
        let crc = 0x89ABCDEF;
        let dos_time = 0x5678;

        // Non-streaming (Bit 3 = 0): uses CRC-32 high byte (0x89)
        assert_eq!(compute_zipcrypto_check_byte(crc, dos_time, false), 0x89);

        // Streaming (Bit 3 = 1): uses DOS time high byte (0x56)
        assert_eq!(compute_zipcrypto_check_byte(crc, dos_time, true), 0x56);
    }

    #[test]
    fn test_flag_helpers() {
        let flag = inject_data_descriptor_flag(0);
        assert_eq!(flag, 0x0808);
        assert!(has_data_descriptor(flag));
        assert!(!has_data_descriptor(0x0800));
    }
}
