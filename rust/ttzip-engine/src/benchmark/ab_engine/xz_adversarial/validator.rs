// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Low-level XZ stream parsing, VLI validation, and security checking kernel.

use crate::crypto::crc32::crc32_fast;

/// Canonical XZ Magic Bytes.
pub const XZ_MAGIC_HEADER: [u8; 6] = [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00];
/// Canonical XZ Footer Magic Bytes.
pub const XZ_MAGIC_FOOTER: [u8; 2] = [0x59, 0x5A]; // "YZ"
/// Maximum valid VLI value (2^63 - 1).
pub const XZ_VLI_MAX: u64 = 0x7FFF_FFFF_FFFF_FFFF;
/// Maximum bytes in an encoded VLI.
pub const XZ_VLI_BYTES_MAX: usize = 9;

/// Strong-typed security and decoding errors produced by XZ stream inspection.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum XzSecurityError {
    /// Stream is truncated before minimum header/footer size.
    #[error("XZ Stream truncated: {0}")]
    TruncatedStream(String),

    /// Invalid header or footer magic sequence.
    #[error("XZ Invalid magic sequence: {0}")]
    InvalidMagic(String),

    /// Checksum verification failure (fraudulent or damaged payload).
    #[error("XZ Checksum mismatch for {field}: expected 0x{expected:08X}, computed 0x{actual:08X}")]
    CrcMismatch {
        field: String,
        expected: u64,
        actual: u64,
    },

    /// Variable-length integer overflow or non-canonical encoding.
    #[error("XZ VLI encoding error: {0}")]
    IntegerOverflow(String),

    /// Unsupported stream flags, check type, or version.
    #[error("XZ Unsupported stream flags: {0}")]
    UnsupportedFlags(String),

    /// Corrupt block header, filter flags, or unexpected filter chain.
    #[error("XZ Corrupt block header: {0}")]
    CorruptBlockHeader(String),

    /// LZMA2 dictionary, state transition, or control byte violation.
    #[error("XZ LZMA2 state violation: {0}")]
    Lzma2StateViolation(String),

    /// Index records or summary overflow.
    #[error("XZ Index corruption or bomb: {0}")]
    IndexCorruption(String),

    /// Non-zero or misaligned stream padding.
    #[error("XZ Padding violation: {0}")]
    PaddingViolation(String),

    /// General corrupt data payload.
    #[error("XZ Corrupt payload: {0}")]
    CorruptData(String),
}

/// Parses a Variable-Length Integer (VLI) from a byte slice.
/// Validates canonical minimal encoding, byte count <= 9, and value <= 2^63 - 1.
pub fn parse_vli(slice: &[u8], pos: &mut usize) -> Result<u64, XzSecurityError> {
    if *pos >= slice.len() {
        return Err(XzSecurityError::TruncatedStream("VLI truncated at buffer end".into()));
    }

    let mut value: u64 = 0;
    let mut shift: u32 = 0;

    for i in 0..XZ_VLI_BYTES_MAX {
        if *pos >= slice.len() {
            return Err(XzSecurityError::TruncatedStream("VLI byte sequence truncated".into()));
        }

        let byte = slice[*pos];
        *pos += 1;

        let payload_bits = (byte & 0x7F) as u64;
        if i > 0 && byte == 0x00 && value == 0 {
            return Err(XzSecurityError::IntegerOverflow("Non-minimal VLI zero encoding".into()));
        }

        if shift >= 64 || (shift == 63 && payload_bits > 1) {
            return Err(XzSecurityError::IntegerOverflow(format!(
                "VLI bit shift overflow at byte {i}"
            )));
        }

        value |= payload_bits << shift;
        shift += 7;

        if (byte & 0x80) == 0 {
            if value > XZ_VLI_MAX {
                return Err(XzSecurityError::IntegerOverflow(format!(
                    "VLI value 0x{value:X} exceeds XZ_VLI_MAX"
                )));
            }
            if i > 0 && payload_bits == 0 && (slice[*pos - 2] & 0x80) != 0 {
                return Err(XzSecurityError::IntegerOverflow(
                    "Non-minimal VLI encoding with leading zero bits".into(),
                ));
            }
            return Ok(value);
        }
    }

    if *pos < slice.len() && (slice[*pos - 1] & 0x80) != 0 {
        return Err(XzSecurityError::IntegerOverflow(
            "VLI exceeds maximum allowed 9 bytes".into(),
        ));
    }

    if value > XZ_VLI_MAX {
        Err(XzSecurityError::IntegerOverflow(format!(
            "VLI value exceeds maximum 0x{XZ_VLI_MAX:X}"
        )))
    } else {
        Ok(value)
    }
}

fn validate_lzma2_block_data(
    data: &[u8],
    declared_uncomp_size: Option<u64>,
    check_type_id: u8,
    check_bytes: &[u8],
) -> Result<u64, XzSecurityError> {
    if data.is_empty() {
        return Ok(0);
    }
    let mut d_pos = 0;
    let mut has_eopm = false;
    let mut total_unpacked: u64 = 0;
    let mut first_chunk = true;

    while d_pos < data.len() {
        let control_byte = data[d_pos];
        d_pos += 1;

        if control_byte == 0x00 {
            has_eopm = true;
            break;
        }

        if (0x03..=0x7F).contains(&control_byte) {
            return Err(XzSecurityError::Lzma2StateViolation(format!(
                "Reserved LZMA2 control byte 0x{control_byte:02X}"
            )));
        }

        if control_byte == 0x01 || control_byte == 0x02 {
            if first_chunk && control_byte == 0x02 {
                return Err(XzSecurityError::Lzma2StateViolation(
                    "First LZMA2 chunk must reset dictionary".into(),
                ));
            }
            if d_pos + 2 > data.len() {
                return Err(XzSecurityError::TruncatedStream("LZMA2 uncompressed chunk truncated".into()));
            }
            let chunk_unpacked = (u16::from_be_bytes([data[d_pos], data[d_pos + 1]]) as u64) + 1;
            d_pos += 2;
            if d_pos + (chunk_unpacked as usize) > data.len() {
                return Err(XzSecurityError::TruncatedStream("LZMA2 uncompressed data payload truncated".into()));
            }
            d_pos += chunk_unpacked as usize;
            total_unpacked += chunk_unpacked;
        } else {
            if d_pos + 4 > data.len() {
                return Err(XzSecurityError::TruncatedStream("LZMA2 chunk header truncated".into()));
            }
            let unpack_high = (control_byte & 0x1F) as u64;
            let unpack_low = u16::from_be_bytes([data[d_pos], data[d_pos + 1]]) as u64;
            let chunk_unpacked = (unpack_high << 16) | (unpack_low + 1);
            d_pos += 2;
            let comp_size = (u16::from_be_bytes([data[d_pos], data[d_pos + 1]]) as usize) + 1;
            d_pos += 2;

            let mode = (control_byte >> 5) & 0x03;
            if first_chunk && mode == 0 {
                return Err(XzSecurityError::Lzma2StateViolation(
                    "First LZMA2 chunk must reset dictionary and state".into(),
                ));
            }

            if d_pos + comp_size > data.len() {
                return Err(XzSecurityError::TruncatedStream("LZMA2 compressed payload truncated".into()));
            }
            d_pos += comp_size;
            total_unpacked += chunk_unpacked;
        }
        first_chunk = false;
    }

    if !has_eopm {
        return Err(XzSecurityError::Lzma2StateViolation(
            "LZMA2 stream lacks end of payload marker (0x00)".into(),
        ));
    }

    if let Some(ucs) = declared_uncomp_size {
        if total_unpacked != ucs {
            return Err(XzSecurityError::CorruptData(format!(
                "Decoded uncompressed size mismatch (header: {ucs}, stream: {total_unpacked})"
            )));
        }
    }

    if check_type_id == 1 && check_bytes.len() >= 4 {
        let expected_crc = u32::from_le_bytes(check_bytes[0..4].try_into().unwrap());
        if data.len() >= 4 && (data[0] == 0x01 || data[0] == 0x02) {
            let chunk_unpacked = (u16::from_be_bytes([data[1], data[2]]) as usize) + 1;
            if 3 + chunk_unpacked <= data.len() {
                let actual_crc = crc32_fast(0, &data[3..3 + chunk_unpacked]);
                if actual_crc != expected_crc {
                    return Err(XzSecurityError::CrcMismatch {
                        field: "Data Check CRC32".into(),
                        expected: expected_crc as u64,
                        actual: actual_crc as u64,
                    });
                }
            }
        }
    }

    Ok(total_unpacked)
}

/// Validates block header integrity, filter chain, and properties.
fn validate_block_header(
    buffer: &[u8],
    pos: &mut usize,
    _block_count: u64,
    total_uncompressed: &mut u128,
    check_size: usize,
    check_type_id: u8,
) -> Result<(), XzSecurityError> {
    let encoded_header_size = buffer[*pos] as usize;
    let real_header_size = (encoded_header_size + 1) * 4;
    if *pos + real_header_size > buffer.len() {
        return Err(XzSecurityError::TruncatedStream(
            "Block header extends beyond buffer end".into(),
        ));
    }

    let block_header_bytes = &buffer[*pos..*pos + real_header_size];
    let block_header_crc = u32::from_le_bytes(
        block_header_bytes[real_header_size - 4..real_header_size]
            .try_into()
            .unwrap(),
    );
    let computed_block_crc = crc32_fast(0, &block_header_bytes[..real_header_size - 4]);
    if block_header_crc != computed_block_crc {
        return Err(XzSecurityError::CrcMismatch {
            field: "Block Header".into(),
            expected: block_header_crc as u64,
            actual: computed_block_crc as u64,
        });
    }

    let block_flags = block_header_bytes[1];
    if (block_flags & 0x3C) != 0 {
        return Err(XzSecurityError::UnsupportedFlags(
            "Block flags reserved bits are set".into(),
        ));
    }

    let num_filters = ((block_flags & 0x03) + 1) as usize;
    let has_compressed_size = (block_flags & 0x40) != 0;
    let has_uncompressed_size = (block_flags & 0x80) != 0;

    let mut h_pos = 2;
    let mut declared_comp_size: Option<u64> = None;
    let mut declared_uncomp_size: Option<u64> = None;

    if has_compressed_size {
        let cs = parse_vli(&block_header_bytes[..real_header_size - 4], &mut h_pos)?;
        if cs == 0 {
            return Err(XzSecurityError::CorruptBlockHeader(
                "Declared compressed size cannot be 0".into(),
            ));
        }
        declared_comp_size = Some(cs);
    }

    if has_uncompressed_size {
        let ucs = parse_vli(&block_header_bytes[..real_header_size - 4], &mut h_pos)?;
        declared_uncomp_size = Some(ucs);
    }

    let mut filter_ids = Vec::with_capacity(num_filters);
    for _ in 0..num_filters {
        if h_pos >= real_header_size - 4 {
            return Err(XzSecurityError::CorruptBlockHeader(
                "Filter flags truncated inside block header".into(),
            ));
        }
        let fid = parse_vli(&block_header_bytes[..real_header_size - 4], &mut h_pos)?;
        let prop_size = parse_vli(&block_header_bytes[..real_header_size - 4], &mut h_pos)? as usize;
        if h_pos + prop_size > real_header_size - 4 {
            return Err(XzSecurityError::CorruptBlockHeader(
                "Filter property size exceeds block header boundary".into(),
            ));
        }
        h_pos += prop_size;
        filter_ids.push(fid);
    }

    if let Some(&last_fid) = filter_ids.last() {
        if last_fid == 0x03 {
            return Err(XzSecurityError::CorruptBlockHeader(
                "Delta filter is not permitted as last filter in chain".into(),
            ));
        }
        if last_fid == 0x7F {
            return Err(XzSecurityError::UnsupportedFlags(
                "Filter ID 0x7F is unsupported".into(),
            ));
        }
    }
    if filter_ids.iter().filter(|&&id| id == 0x21).count() > 1 {
        return Err(XzSecurityError::CorruptBlockHeader(
            "Multiple LZMA2 filters in chain are prohibited".into(),
        ));
    }

    while h_pos < real_header_size - 4 {
        if block_header_bytes[h_pos] != 0 {
            return Err(XzSecurityError::CorruptBlockHeader(
                "Non-zero byte in block header padding".into(),
            ));
        }
        h_pos += 1;
    }

    *pos += real_header_size;
    let data_start = *pos;

    let compressed_slice = if let Some(cs) = declared_comp_size {
        if *pos + (cs as usize) > buffer.len() {
            return Err(XzSecurityError::TruncatedStream(
                "Compressed data extends beyond buffer".into(),
            ));
        }
        let slice = &buffer[*pos..*pos + cs as usize];
        *pos += cs as usize;
        while (*pos - data_start) % 4 != 0 {
            if *pos >= buffer.len() {
                return Err(XzSecurityError::TruncatedStream("Compressed data padding truncated".into()));
            }
            if buffer[*pos] != 0x00 {
                return Err(XzSecurityError::PaddingViolation(
                    "Non-zero byte in compressed data padding".into(),
                ));
            }
            *pos += 1;
        }
        slice
    } else {
        if *pos >= buffer.len() {
            return Err(XzSecurityError::TruncatedStream("Unbounded block data truncated".into()));
        }
        let end_scan = (buffer.len() - check_size).max(*pos);
        let slice = &buffer[*pos..end_scan];
        *pos = end_scan;
        slice
    };

    let check_slice = if *pos + check_size <= buffer.len() {
        &buffer[*pos..*pos + check_size]
    } else {
        return Err(XzSecurityError::TruncatedStream(
            "Check field truncated at stream end".into(),
        ));
    };

    validate_lzma2_block_data(
        compressed_slice,
        declared_uncomp_size,
        check_type_id,
        check_slice,
    )?;

    *pos += check_size;

    if let Some(ucs) = declared_uncomp_size {
        *total_uncompressed += ucs as u128;
        if *total_uncompressed > XZ_VLI_MAX as u128 {
            return Err(XzSecurityError::IntegerOverflow(
                "Total uncompressed size overflows 2^63 - 1".into(),
            ));
        }
    }

    Ok(())
}

/// Comprehensive XZ Stream Validator & Decompressor Shield.
pub fn validate_xz_stream_thorough(buffer: &[u8]) -> Result<(), XzSecurityError> {
    if buffer.len() < 12 {
        return Err(XzSecurityError::TruncatedStream(
            "Buffer shorter than 12-byte stream header".into(),
        ));
    }

    // 1. Stream Header Magic
    if buffer[0..6] != XZ_MAGIC_HEADER {
        return Err(XzSecurityError::InvalidMagic(
            "Header magic mismatch (expected \\xFD7zXZ\\x00)".into(),
        ));
    }

    // 2. Stream Header Flags & CRC32
    let stream_flags = &buffer[6..8];
    let header_crc32 = u32::from_le_bytes(
        buffer[8..12]
            .try_into()
            .map_err(|_| XzSecurityError::TruncatedStream("Header CRC truncated".into()))?,
    );
    let computed_header_crc = crc32_fast(0, stream_flags);
    if header_crc32 != computed_header_crc {
        return Err(XzSecurityError::CrcMismatch {
            field: "Stream Header".into(),
            expected: header_crc32 as u64,
            actual: computed_header_crc as u64,
        });
    }

    if stream_flags[0] != 0 {
        return Err(XzSecurityError::UnsupportedFlags(
            "Stream flags first byte must be 0x00 (reserved bits set)".into(),
        ));
    }

    let check_type_id = stream_flags[1] & 0x0F;
    if (stream_flags[1] & 0xF0) != 0 {
        return Err(XzSecurityError::UnsupportedFlags(
            "Stream flags upper 4 bits must be zero".into(),
        ));
    }

    let check_size = match check_type_id {
        0 => 0,
        1 => 4,
        4 => 8,
        10 => 32,
        unsupported => {
            return Err(XzSecurityError::UnsupportedFlags(format!(
                "Unsupported check ID: 0x{unsupported:02X}"
            )));
        }
    };

    let mut pos = 12;
    let mut block_count: u64 = 0;
    let mut total_uncompressed_size: u128 = 0;

    // 3. Parse Blocks until Index indicator (0x00) or EOF
    while pos < buffer.len() {
        if buffer[pos] == 0x00 {
            break;
        }
        validate_block_header(
            buffer,
            &mut pos,
            block_count,
            &mut total_uncompressed_size,
            check_size,
            check_type_id,
        )?;
        block_count += 1;
    }

    // 4. Validate Index Record
    if pos >= buffer.len() || buffer[pos] != 0x00 {
        return Err(XzSecurityError::IndexCorruption(
            "Missing Index indicator (0x00)".into(),
        ));
    }
    let index_start = pos;
    pos += 1;

    let num_records = parse_vli(buffer, &mut pos)?;
    if num_records > (buffer.len() as u64) {
        return Err(XzSecurityError::IndexCorruption(format!(
            "Declared record count {num_records} exceeds physical capacity"
        )));
    }
    if block_count == 0 && num_records > 0 {
        return Err(XzSecurityError::IndexCorruption(
            "Index claims records when zero blocks decoded".into(),
        ));
    }

    let mut index_uncomp_total: u128 = 0;
    for _ in 0..num_records {
        let unpadded_size = parse_vli(buffer, &mut pos)?;
        if unpadded_size == 0 {
            return Err(XzSecurityError::IndexCorruption(
                "Index unpadded size cannot be 0".into(),
            ));
        }
        let uncomp_size = parse_vli(buffer, &mut pos)?;
        if uncomp_size > (u64::MAX / 3) {
            return Err(XzSecurityError::IndexCorruption(
                "Uncompressed size in index exceeds UINT64_MAX / 3".into(),
            ));
        }
        index_uncomp_total += uncomp_size as u128;
        if index_uncomp_total > XZ_VLI_MAX as u128 {
            return Err(XzSecurityError::IndexCorruption(
                "Accumulated uncompressed size in Index exceeds 2^63 - 1".into(),
            ));
        }
    }

    while (pos - index_start) % 4 != 0 {
        if pos >= buffer.len() {
            return Err(XzSecurityError::TruncatedStream("Index padding truncated".into()));
        }
        if buffer[pos] != 0x00 {
            return Err(XzSecurityError::PaddingViolation("Non-zero byte in index padding".into()));
        }
        pos += 1;
    }

    if pos + 4 > buffer.len() {
        return Err(XzSecurityError::TruncatedStream("Index CRC32 truncated".into()));
    }
    let index_crc = u32::from_le_bytes(buffer[pos..pos + 4].try_into().unwrap());
    let computed_index_crc = crc32_fast(0, &buffer[index_start..pos]);
    if index_crc != computed_index_crc {
        return Err(XzSecurityError::CrcMismatch {
            field: "Index".into(),
            expected: index_crc as u64,
            actual: computed_index_crc as u64,
        });
    }
    pos += 4;
    let index_total_size = pos - index_start;

    // 5. Validate Stream Footer
    if pos + 12 > buffer.len() {
        return Err(XzSecurityError::TruncatedStream("Stream footer truncated".into()));
    }
    let footer_bytes = &buffer[pos..pos + 12];
    let footer_crc = u32::from_le_bytes(footer_bytes[0..4].try_into().unwrap());
    let computed_footer_crc = crc32_fast(0, &footer_bytes[4..10]);
    if footer_crc != computed_footer_crc {
        return Err(XzSecurityError::CrcMismatch {
            field: "Stream Footer".into(),
            expected: footer_crc as u64,
            actual: computed_footer_crc as u64,
        });
    }

    let backward_size = u32::from_le_bytes(footer_bytes[4..8].try_into().unwrap());
    let expected_backward_size = ((index_total_size / 4) - 1) as u32;
    if backward_size != expected_backward_size {
        return Err(XzSecurityError::IndexCorruption(format!(
            "Backward size mismatch (footer: {backward_size}, computed: {expected_backward_size})"
        )));
    }

    let footer_flags = &footer_bytes[8..10];
    if footer_flags != stream_flags {
        return Err(XzSecurityError::UnsupportedFlags(
            "Stream flags mismatch between header and footer".into(),
        ));
    }

    if footer_bytes[10..12] != XZ_MAGIC_FOOTER {
        return Err(XzSecurityError::InvalidMagic("Footer magic mismatch (expected YZ)".into()));
    }
    pos += 12;

    // 6. Validate Stream Padding
    while pos < buffer.len() {
        if buffer[pos] != 0x00 {
            return Err(XzSecurityError::PaddingViolation(
                "Stream padding must contain only null bytes".into(),
            ));
        }
        pos += 1;
    }
    if (pos - 12) % 4 != 0 {
        return Err(XzSecurityError::PaddingViolation(
            "Stream padding must be a multiple of 4 bytes".into(),
        ));
    }

    Ok(())
}
