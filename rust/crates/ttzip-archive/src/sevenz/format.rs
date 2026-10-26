// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

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

/// Configuration parameters for 7z AES-256-CBC encryption in the metadata header.
#[derive(Debug, Clone)]
pub struct SevenZAesWriterConfig {
    pub salt: [u8; 16],
    pub iv: [u8; 16],
    pub num_cycles_power: u32,
    pub raw_comp_len: u64,
}

/// Lightweight entry view for 7z header serialization without holding file bytes.
#[derive(Debug, Clone, Copy)]
pub struct HeaderEntryRef<'a> {
    pub rel_path: &'a str,
    pub is_directory: bool,
    pub is_empty_stream: bool,
    pub is_symlink: bool,
    pub posix_mode: u32,
    pub mtime: std::time::SystemTime,
}

fn encode_method_bytes(method_id: u64) -> Vec<u8> {
    let mut method_bytes = Vec::new();
    let mut temp_mid = method_id;
    while temp_mid > 0 {
        method_bytes.push((temp_mid & 0xFF) as u8);
        temp_mid >>= 8;
    }
    if method_bytes.is_empty() {
        method_bytes.push(0);
    }
    method_bytes.reverse();
    method_bytes
}

fn write_coder_descriptor(method_id: u64, coder_props: &[u8], h: &mut Vec<u8>) {
    let method_bytes = encode_method_bytes(method_id);
    let mut flags = (method_bytes.len() as u8) & 0x0F;
    if !coder_props.is_empty() {
        flags |= 0x20;
    }
    h.push(flags);
    h.extend_from_slice(&method_bytes);
    if !coder_props.is_empty() {
        write_varint(coder_props.len() as u64, h);
        h.extend_from_slice(coder_props);
    }
}

fn write_unpack_info_section_with_crc(
    uncompressed_len: u64,
    method_id: u64,
    coder_props: &[u8],
    aes_config: Option<&SevenZAesWriterConfig>,
    folder_crc: Option<u32>,
    h: &mut Vec<u8>,
) {
    h.push(K_UNPACK_INFO);
    h.push(K_FOLDER);
    write_varint(1, h);
    h.push(0);

    if let Some(aes) = aes_config {
        write_varint(2, h);
        let mut aes_props = Vec::with_capacity(34);
        let b0 = 0x80 | 0x40 | ((aes.num_cycles_power as u8) & 0x3F);
        aes_props.push(b0);
        aes_props.push(0xFF);
        aes_props.extend_from_slice(&aes.salt);
        aes_props.extend_from_slice(&aes.iv);

        h.push(4 | 0x20);
        h.extend_from_slice(&[0x06, 0xF1, 0x07, 0x01]);
        write_varint(aes_props.len() as u64, h);
        h.extend_from_slice(&aes_props);

        write_coder_descriptor(method_id, coder_props, h);
        write_varint(1, h);
        write_varint(0, h);

        h.push(K_CODERS_UNPACK_SIZE);
        write_varint(aes.raw_comp_len, h);
        write_varint(uncompressed_len, h);
    } else {
        write_varint(1, h);
        write_coder_descriptor(method_id, coder_props, h);
        h.push(K_CODERS_UNPACK_SIZE);
        write_varint(uncompressed_len, h);
    }

    if let Some(crc) = folder_crc {
        h.push(K_CRC);
        h.push(1);
        h.extend_from_slice(&crc.to_le_bytes());
    }
    h.push(K_END);
}

fn write_unpack_info_section(
    uncompressed_len: u64,
    method_id: u64,
    coder_props: &[u8],
    aes_config: Option<&SevenZAesWriterConfig>,
    h: &mut Vec<u8>,
) {
    write_unpack_info_section_with_crc(uncompressed_len, method_id, coder_props, aes_config, None, h);
}

fn write_substreams_info_section(stream_sizes: &[u64], stream_crcs: &[u32], h: &mut Vec<u8>) {
    if stream_sizes.len() > 1 {
        h.push(K_SUB_STREAMS_INFO);
        h.push(K_NUM_UNPACK_STREAM);
        write_varint(stream_sizes.len() as u64, h);

        h.push(K_SIZE);
        for &sz in &stream_sizes[..stream_sizes.len() - 1] {
            write_varint(sz, h);
        }

        h.push(K_CRC);
        h.push(1);
        for &c in stream_crcs {
            h.extend_from_slice(&c.to_le_bytes());
        }
        h.push(K_END);
    } else if stream_sizes.len() == 1 && !stream_crcs.is_empty() {
        h.push(K_SUB_STREAMS_INFO);
        h.push(K_CRC);
        h.push(1);
        h.extend_from_slice(&stream_crcs[0].to_le_bytes());
        h.push(K_END);
    }
}

fn write_empty_stream_bitmasks(entries: &[HeaderEntryRef<'_>], h: &mut Vec<u8>) {
    if !entries.iter().any(|it| it.is_empty_stream) {
        return;
    }
    h.push(K_EMPTY_STREAM);
    write_varint(entries.len().div_ceil(8) as u64, h);
    for chunk in entries.chunks(8) {
        let mut byte = 0u8;
        for (bit, item) in chunk.iter().enumerate() {
            if item.is_empty_stream {
                byte |= 1 << (7 - bit);
            }
        }
        h.push(byte);
    }

    let empty_streams: Vec<&HeaderEntryRef<'_>> =
        entries.iter().filter(|it| it.is_empty_stream).collect();
    if empty_streams.iter().any(|it| !it.is_directory) {
        h.push(K_EMPTY_FILE);
        write_varint(empty_streams.len().div_ceil(8) as u64, h);
        for chunk in empty_streams.chunks(8) {
            let mut byte = 0u8;
            for (bit, item) in chunk.iter().enumerate() {
                if !item.is_directory {
                    byte |= 1 << (7 - bit);
                }
            }
            h.push(byte);
        }
    }
}

fn encode_7z_win_attribute(item: &HeaderEntryRef<'_>) -> u32 {
    let (default_mode, default_type_bits) = if item.is_symlink {
        (0o120777, 0o120000)
    } else if item.is_directory {
        (0o040755, 0o040000)
    } else {
        (0o100644, 0o100000)
    };
    let raw_mode = if item.posix_mode != 0 { item.posix_mode } else { default_mode };
    let full_mode = if (raw_mode & 0o170000) == 0 {
        default_type_bits | (raw_mode & 0o7777)
    } else {
        raw_mode
    };
    let win_low: u32 = if item.is_symlink {
        0x0420
    } else if item.is_directory {
        0x0010
    } else if (full_mode & 0o222) == 0 {
        0x0021
    } else {
        0x0020
    };
    ((full_mode & 0xFFFF) << 16) | 0x8000 | win_low
}

fn write_files_info_section(entries: &[HeaderEntryRef<'_>], h: &mut Vec<u8>) {
    h.push(K_FILES_INFO);
    write_varint(entries.len() as u64, h);
    write_empty_stream_bitmasks(entries, h);

    h.push(K_NAME);
    let mut names_u16_bytes = Vec::new();
    for item in entries {
        for u in item.rel_path.encode_utf16() {
            names_u16_bytes.extend_from_slice(&u.to_le_bytes());
        }
        names_u16_bytes.extend_from_slice(&0u16.to_le_bytes());
    }
    write_varint((1 + names_u16_bytes.len()) as u64, h);
    h.push(0);
    h.extend_from_slice(&names_u16_bytes);

    h.push(K_MTIME);
    write_varint((2 + (entries.len() * 8)) as u64, h);
    h.extend_from_slice(&[1, 0]);
    for item in entries {
        let nt = crate::sevenz::time::NtTime::from_system_time(item.mtime)
            .unwrap_or(crate::sevenz::time::NtTime(crate::sevenz::time::UNIX_EPOCH_TICKS));
        h.extend_from_slice(&nt.0.to_le_bytes());
    }

    h.push(K_WIN_ATTRIBUTES);
    write_varint((2 + (entries.len() * 4)) as u64, h);
    h.extend_from_slice(&[1, 0]);
    for item in entries {
        h.extend_from_slice(&encode_7z_win_attribute(item).to_le_bytes());
    }
    h.push(K_END);
}

/// Serializes the 7z metadata header stream from entry references and stream metrics.
pub fn build_7z_metadata_header_from_refs(
    entries: &[HeaderEntryRef<'_>],
    stream_sizes: &[u64],
    stream_crcs: &[u32],
    compressed_len: u64,
    uncompressed_len: u64,
    method_id: u64,
    coder_props: &[u8],
    aes_config: Option<&SevenZAesWriterConfig>,
) -> Vec<u8> {
    let mut h = Vec::new();
    h.push(K_HEADER);
    if !stream_sizes.is_empty() || uncompressed_len > 0 {
        h.push(K_MAIN_STREAMS_INFO);
        h.push(K_PACK_INFO);
        write_varint(0, &mut h);
        write_varint(1, &mut h);
        h.push(K_SIZE);
        write_varint(compressed_len, &mut h);
        h.push(K_END);
        write_unpack_info_section(uncompressed_len, method_id, coder_props, aes_config, &mut h);
        write_substreams_info_section(stream_sizes, stream_crcs, &mut h);
        h.push(K_END);
    }
    write_files_info_section(entries, &mut h);
    h.push(K_END);
    h
}

/// Serializes a `kEncodedHeader` (`0x17`) bootstrap descriptor for an encrypted/compressed `kHeader`.
pub fn build_7z_encoded_header_boot(
    pack_pos_from_payload_start: u64,
    packed_header_len: u64,
    unpacked_header_len: u64,
    unpacked_header_crc: u32,
    method_id: u64,
    coder_props: &[u8],
    aes_config: Option<&SevenZAesWriterConfig>,
) -> Vec<u8> {
    let mut boot = Vec::new();
    boot.push(K_ENCODED_HEADER);
    boot.push(K_PACK_INFO);
    write_varint(pack_pos_from_payload_start, &mut boot);
    write_varint(1, &mut boot);
    boot.push(K_SIZE);
    write_varint(packed_header_len, &mut boot);
    boot.push(K_END);
    write_unpack_info_section_with_crc(
        unpacked_header_len,
        method_id,
        coder_props,
        aes_config,
        Some(unpacked_header_crc),
        &mut boot,
    );
    boot.push(K_END);
    boot
}

/// Builds the 32-byte 7z signature header pointing to `header_bytes` after `compressed_len` payload bytes.
pub fn build_sig_header_bytes(compressed_len: u64, header_bytes: &[u8]) -> [u8; 32] {
    let sig_header = SevenZSignatureHeader {
        major_version: 0,
        minor_version: 4,
        start_header_crc: 0,
        next_header_offset: compressed_len,
        next_header_size: header_bytes.len() as u64,
        next_header_crc: crc32_fast(0, header_bytes),
    };
    sig_header.serialize()
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
