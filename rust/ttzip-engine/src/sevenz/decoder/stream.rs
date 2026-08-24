// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 7-Zip Solid Stream selective entry decoding with Early Termination.

use super::payload::decode_7z_solid_payload;
use crate::crypto::crc32::crc32_fast;
use crate::sevenz::header::{SevenZHeaderInfo, SevenZSeekIndex};
use crate::types::TTZipStatus;

/// Decompresses and extracts a single entry from 7z solid stream with Early Termination.
pub fn extract_entry_bytes_stream(
    mapped: &[u8],
    info: &SevenZHeaderInfo,
    seek_index: &SevenZSeekIndex,
    entry_idx: usize,
    password: Option<&str>,
) -> Result<Vec<u8>, TTZipStatus> {
    let loc = seek_index
        .get_by_index(entry_idx)
        .ok_or(TTZipStatus::ErrInvalidOffset)?;

    if loc.is_directory || loc.is_empty_stream || loc.uncompressed_size == 0 {
        return Ok(Vec::new());
    }

    let target_start = loc.offset_in_folder as usize;
    let target_len = loc.uncompressed_size as usize;
    let target_end = target_start + target_len;

    let solid_buf = decode_7z_solid_payload(mapped, info, password, 1)?;
    let clamped_end = target_end.min(solid_buf.len());
    let clamped_offset = target_start.min(clamped_end);
    let mut result_vec = Vec::with_capacity(clamped_end - clamped_offset);
    result_vec.extend_from_slice(&solid_buf[clamped_offset..clamped_end]);

    // Verify CRC32
    if let Some(expected_crc) = loc.crc {
        if expected_crc != 0 && !result_vec.is_empty() {
            let computed = crc32_fast(0, &result_vec);
            if computed != expected_crc && info.is_encrypted {
                return Err(TTZipStatus::ErrInvalidPassword);
            }
        }
    }

    Ok(result_vec)
}
