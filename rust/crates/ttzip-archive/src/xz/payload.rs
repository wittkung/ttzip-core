// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! XZ Block payload decompression pipeline, LZMA2 stream decoding, and integrity verification.

use crate::codecs::lzma2::{
    Lzma2ChunkHeader, Lzma2StreamDecoder, LZMA2_DEFAULT_DICT_SIZE,
};
use crate::xz::block::{XzBlockHeader, FILTER_ID_LZMA2};
use crate::xz::checksum::{XzChecksumEngine, XzChecksumType};
use crate::xz::filters::apply_filters_decode;
use crate::xz::types::{XzCheckType, XzError};

/// Compute LZMA2 dictionary size from 1-byte property value.
#[inline]
pub fn lzma2_dict_size_from_prop(prop: u8) -> u64 {
    if prop > 39 {
        return 4096;
    }
    let base = 2 | ((prop as u64) & 1);
    let shift = ((prop as u64) >> 1) + 11;
    base << shift
}

/// Decompresses raw LZMA2 chunk stream into uncompressed byte vector.
pub fn decompress_lzma2_payload(
    compressed: &[u8],
    _dict_prop: Option<u8>,
    dict_size: usize,
) -> Result<Vec<u8>, XzError> {
    if compressed.is_empty() || compressed == [0x00] {
        return Ok(Vec::new());
    }

    validate_first_lzma2_control_byte(compressed[0])?;

    let mut safe_decoder = Lzma2StreamDecoder::new(dict_size.max(LZMA2_DEFAULT_DICT_SIZE));
    let mut out = Vec::with_capacity(compressed.len().saturating_mul(2));
    let mut pos = 0;

    while pos < compressed.len() && !safe_decoder.is_eos() {
        decode_next_lzma2_chunk(compressed, &mut pos, &mut safe_decoder, &mut out)?;
    }

    if !safe_decoder.is_eos() {
        return Err(XzError::TruncatedData {
            expected: pos + 1,
            actual: compressed.len(),
        });
    }

    validate_trailing_zeros(&compressed[pos..])?;
    Ok(out)
}

fn validate_first_lzma2_control_byte(first_ctrl: u8) -> Result<(), XzError> {
    let invalid = (0x03..=0x7F).contains(&first_ctrl)
        || first_ctrl == 0x02
        || (0x80..0xE0).contains(&first_ctrl);
    if invalid {
        return Err(XzError::DecompressError(format!(
            "Invalid first LZMA2 chunk control byte 0x{first_ctrl:02X} (dictionary reset required)"
        )));
    }
    Ok(())
}

fn decode_next_lzma2_chunk(
    compressed: &[u8],
    pos: &mut usize,
    safe_decoder: &mut Lzma2StreamDecoder,
    out: &mut Vec<u8>,
) -> Result<(), XzError> {
    let (header, hdr_len) = Lzma2ChunkHeader::parse(&compressed[*pos..])
        .map_err(|e| XzError::DecompressError(format!("LZMA2 header parse error: {e:?}")))?
        .ok_or(XzError::TruncatedData {
            expected: *pos + 1,
            actual: compressed.len(),
        })?;
    *pos += hdr_len;
    let pack = header.pack_size();
    if *pos + pack > compressed.len() && !header.is_eos() {
        return Err(XzError::TruncatedData {
            expected: *pos + pack,
            actual: compressed.len(),
        });
    }
    let payload = if pack > 0 {
        &compressed[*pos..*pos + pack]
    } else {
        &[]
    };
    *pos += pack;
    safe_decoder
        .decode_chunk(&header, payload, out)
        .map(|_| ())
        .map_err(|e| XzError::DecompressError(format!("LZMA2 safe decode error: {e:?}")))
}

fn validate_trailing_zeros(remainder: &[u8]) -> Result<(), XzError> {
    if remainder.iter().any(|&b| b != 0x00) {
        return Err(XzError::DecompressError(format!(
            "Unexpected trailing non-zero data after LZMA2 EOS marker ({} bytes)",
            remainder.len()
        )));
    }
    Ok(())
}

/// Decompresses an entire XZ Block's payload and validates its checksum.
pub fn decompress_block_payload(
    compressed_payload: &[u8],
    header: &XzBlockHeader,
    check_type: XzCheckType,
    expected_check: &[u8],
    memlimit: u64,
) -> Result<Vec<u8>, XzError> {
    let Some(lzma2_filter) = header.filters.last() else {
        return Err(XzError::DecompressError("Empty filter chain".to_string()));
    };
    if lzma2_filter.filter_id != FILTER_ID_LZMA2 {
        return Err(XzError::UnsupportedFilter(lzma2_filter.filter_id));
    }

    if header.filters[..header.filters.len() - 1]
        .iter()
        .any(|f| f.filter_id == FILTER_ID_LZMA2)
    {
        return Err(XzError::UnsupportedFilter(FILTER_ID_LZMA2));
    }

    let dict_prop = lzma2_filter.properties.first().copied();
    let dict_size = dict_prop.map(lzma2_dict_size_from_prop).unwrap_or(4096);
    if dict_size > memlimit {
        return Err(XzError::SizeOverflow("Memory limit exceeded for LZMA2 dictionary"));
    }

    let mut uncompressed = decompress_lzma2_payload(
        compressed_payload,
        dict_prop,
        dict_size as usize,
    )?;

    if let Some(expected_unpack) = header.uncompressed_size {
        if uncompressed.len() as u64 != expected_unpack {
            return Err(XzError::DecompressError(format!(
                "Uncompressed size mismatch: header specifies {}, got {}",
                expected_unpack,
                uncompressed.len()
            )));
        }
    }

    // Apply pre-filters in reverse order
    apply_filters_decode(&header.filters, &mut uncompressed)?;

    // Verify Checksum
    let check_enum = match check_type {
        XzCheckType::None => XzChecksumType::None,
        XzCheckType::Crc32 => XzChecksumType::Crc32,
        XzCheckType::Crc64 => XzChecksumType::Crc64,
        XzCheckType::Sha256 => XzChecksumType::Sha256,
    };

    let mut checksum_engine = XzChecksumEngine::new(check_enum);
    checksum_engine.update(&uncompressed);

    if check_type != XzCheckType::None {
        checksum_engine.verify(expected_check)?;
    }

    Ok(uncompressed)
}
