// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 7-Zip Solid Stream selective entry decoding with Bounded Memory and Early Termination.

use super::payload::decode_7z_folder_streaming;
use crate::crypto::crc32::crc32_fast;
use crate::sevenz::header::{SevenZHeaderInfo, SevenZSeekIndex};
use crate::types::TTZipStatus;

/// Decompresses and extracts a single entry from 7z solid stream with bounded memory and early termination.
pub fn extract_entry_bytes_stream_bounded(
    mapped: &[u8],
    info: &SevenZHeaderInfo,
    seek_index: &SevenZSeekIndex,
    entry_idx: usize,
    password: Option<&str>,
    max_preceding_budget_bytes: u64,
) -> Result<Vec<u8>, TTZipStatus> {
    let loc = seek_index
        .get_by_index(entry_idx)
        .ok_or(TTZipStatus::ErrInvalidOffset)?;

    if loc.is_directory || loc.is_empty_stream || loc.uncompressed_size == 0 {
        return Ok(Vec::new());
    }

    let target_start = loc.offset_in_folder;
    let target_len = loc.uncompressed_size;
    let target_end = target_start + target_len;

    // Budget guard: If preceding data exceeds budget and budget is set, return ErrSolidBudgetExceeded
    if max_preceding_budget_bytes > 0 && target_start > max_preceding_budget_bytes {
        return Err(TTZipStatus::ErrSolidBudgetExceeded);
    }

    let mut current_offset: u64 = 0;
    let mut result_vec = Vec::with_capacity(target_len as usize);
    let folder_idx = loc.folder_index.unwrap_or(0);

    decode_7z_folder_streaming(mapped, info, folder_idx, password, 1, |chunk| -> Result<(), TTZipStatus> {
        let chunk_start = current_offset;
        let chunk_len = chunk.len() as u64;
        let chunk_end = chunk_start + chunk_len;
        current_offset += chunk_len;

        // Chunk is entirely before target file: discard, zero-alloc
        if chunk_end <= target_start {
            return Ok(());
        }

        // Chunk overlaps with target [target_start, target_end)
        if chunk_start < target_end && chunk_end > target_start {
            let slice_start = (target_start.saturating_sub(chunk_start)) as usize;
            let slice_end = (target_end.min(chunk_end) - chunk_start) as usize;
            result_vec.extend_from_slice(&chunk[slice_start..slice_end]);
        }

        // Early termination once target is fully read
        if current_offset >= target_end {
            return Err(TTZipStatus::Eof);
        }

        Ok(())
    }).or_else(|status| {
        if status == TTZipStatus::Eof {
            Ok(target_len)
        } else {
            Err(status)
        }
    })?;

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

/// Backwards-compatible stream extraction with default 100MB budget.
pub fn extract_entry_bytes_stream(
    mapped: &[u8],
    info: &SevenZHeaderInfo,
    seek_index: &SevenZSeekIndex,
    entry_idx: usize,
    password: Option<&str>,
) -> Result<Vec<u8>, TTZipStatus> {
    extract_entry_bytes_stream_bounded(mapped, info, seek_index, entry_idx, password, 100 * 1024 * 1024)
}
