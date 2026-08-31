// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! DataStreamAlignment sector and virtual memory page alignment for ZIP archives.
//!
//! Provides zero-copy memory mapping (`mmap`) support for archive payloads by aligning
//! the physical file offset of compressed/stored data streams using the standard ZIP
//! Extra Field tag `0xa11e` (`TAG_DATA_STREAM_ALIGNMENT`).
//!
//! # Specification
//! - Extra Field Tag: `0xa11e` (u16 little-endian: `[0x1e, 0xa1]`).
//! - Minimum extra field size: 6 bytes (Tag 2B + Size 2B + Alignment 2B).
//! - Target alignments: typically 4096 (4KB x86/POSIX page), 16384 (16KB Apple Silicon page),
//!   or 65536 (64KB large sector / GPU direct I/O).
//! - Local File Header (LFH) specific: stripped from Central Directory File Headers (CDFH)
//!   to eliminate catalog space inflation.

/// ZIP Extra Field tag for data stream alignment (`0xa11e`).
pub const TAG_DATA_STREAM_ALIGNMENT: u16 = 0xa11e;

/// Minimum byte size of a valid `0xa11e` alignment Extra Field:
/// 2 bytes (Tag) + 2 bytes (Size) + 2 bytes (Alignment) = 6 bytes.
pub const MIN_ALIGNMENT_EXTRA_FIELD_LEN: usize = 6;

/// Fixed header size of a ZIP Local File Header (LFH) before variable fields.
pub const LFH_FIXED_HEADER_SIZE: usize = 30;

/// Calculator for determining the required padding bytes to align payload data streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AlignmentPaddingCalculator;

impl AlignmentPaddingCalculator {
    /// Computes the padding byte length required so that the payload's physical file offset
    /// satisfies `data_start % target_alignment == 0`.
    ///
    /// # Arguments
    /// * `header_start` - Physical file offset where the Local File Header begins.
    /// * `file_name_len` - Length in bytes of the file name.
    /// * `existing_extra_len` - Length in bytes of existing extra fields in LFH.
    /// * `target_alignment` - Target alignment boundary in bytes (e.g. 4096, 16384, 65536).
    ///
    /// # Returns
    /// * `0` if `target_alignment <= 1` or if `data_start` is already naturally aligned.
    /// * An exact padding length (`>= 6`) ensuring `(unpadded_data_start + padding) % target_alignment == 0`.
    ///   If the mathematical remainder requires 1..5 bytes of padding, a full alignment cycle
    ///   is accumulated (`needed += target_alignment`) because the minimal `0xa11e` extra field
    ///   requires at least 6 bytes (Tag 2B + Size 2B + Align 2B).
    pub fn calculate(
        header_start: u64,
        file_name_len: usize,
        existing_extra_len: usize,
        target_alignment: u16,
    ) -> usize {
        if target_alignment <= 1 {
            return 0;
        }

        let align = target_alignment as u64;
        let unpadded_data_start = header_start
            .saturating_add(LFH_FIXED_HEADER_SIZE as u64)
            .saturating_add(file_name_len as u64)
            .saturating_add(existing_extra_len as u64);

        let rem = unpadded_data_start % align;
        if rem == 0 {
            return 0;
        }

        let mut needed = align - rem;
        if needed < MIN_ALIGNMENT_EXTRA_FIELD_LEN as u64 {
            needed = needed.saturating_add(align);
        }

        needed as usize
    }

    /// Computes the final aligned data start physical offset.
    pub fn calculate_data_start(
        header_start: u64,
        file_name_len: usize,
        existing_extra_len: usize,
        target_alignment: u16,
    ) -> u64 {
        let pad = Self::calculate(
            header_start,
            file_name_len,
            existing_extra_len,
            target_alignment,
        );
        header_start
            .saturating_add(LFH_FIXED_HEADER_SIZE as u64)
            .saturating_add(file_name_len as u64)
            .saturating_add(existing_extra_len as u64)
            .saturating_add(pad as u64)
    }

    /// Checks whether a given physical offset satisfies the alignment boundary.
    pub fn is_aligned(offset: u64, target_alignment: u16) -> bool {
        if target_alignment <= 1 {
            true
        } else {
            offset.is_multiple_of(target_alignment as u64)
        }
    }
}

/// Serializes a `TAG_DATA_STREAM_ALIGNMENT` (`0xa11e`) Extra Field record.
///
/// # Arguments
/// * `pad_len` - Total byte size of the extra field to generate (including tag and size headers).
/// * `alignment` - Alignment value recorded in the payload.
///
/// # Returns
/// * Empty `Vec<u8>` if `pad_len == 0`.
/// * A byte buffer of length `pad_len` structured as:
///   - `[0..2]`: Header ID `0xa11e` (little-endian: `0x1e, 0xa1`)
///   - `[2..4]`: Data Size `(pad_len - 4) as u16` (little-endian)
///   - `[4..6]`: Alignment value `alignment` (little-endian)
///   - `[6..pad_len]`: Zero padding bytes (all `0x00`)
pub fn build_alignment_extra_field(pad_len: usize, alignment: u16) -> Vec<u8> {
    if pad_len == 0 {
        return Vec::new();
    }
    if pad_len < MIN_ALIGNMENT_EXTRA_FIELD_LEN {
        return Vec::new();
    }

    let data_size = (pad_len - 4) as u16;
    let mut out = Vec::with_capacity(pad_len);
    out.extend_from_slice(&TAG_DATA_STREAM_ALIGNMENT.to_le_bytes());
    out.extend_from_slice(&data_size.to_le_bytes());
    out.extend_from_slice(&alignment.to_le_bytes());
    out.resize(pad_len, 0u8);
    out
}

/// Strips all `TAG_DATA_STREAM_ALIGNMENT` (`0xa11e`) extra fields from an extra field byte buffer.
///
/// Enforces Central Directory isolation: alignment padding extra fields are Local File Header (LFH)
/// specific and must be omitted from Central Directory File Headers (CDFH) to eliminate catalog space waste.
pub fn strip_alignment_extra_fields(extra_data: &[u8]) -> Vec<u8> {
    if extra_data.len() < 4 {
        return extra_data.to_vec();
    }

    let mut out = Vec::with_capacity(extra_data.len());
    let mut offset = 0;

    while offset + 4 <= extra_data.len() {
        let tag = u16::from_le_bytes([extra_data[offset], extra_data[offset + 1]]);
        let data_size =
            u16::from_le_bytes([extra_data[offset + 2], extra_data[offset + 3]]) as usize;
        let total_field_len = 4 + data_size;

        if offset + total_field_len > extra_data.len() {
            out.extend_from_slice(&extra_data[offset..]);
            break;
        }

        if tag != TAG_DATA_STREAM_ALIGNMENT {
            out.extend_from_slice(&extra_data[offset..offset + total_field_len]);
        }

        offset += total_field_len;
    }

    out
}

/// Checks if an extra field byte buffer contains a `TAG_DATA_STREAM_ALIGNMENT` record.
pub fn contains_alignment_extra_field(extra_data: &[u8]) -> bool {
    if extra_data.len() < 4 {
        return false;
    }

    let mut offset = 0;
    while offset + 4 <= extra_data.len() {
        let tag = u16::from_le_bytes([extra_data[offset], extra_data[offset + 1]]);
        let data_size =
            u16::from_le_bytes([extra_data[offset + 2], extra_data[offset + 3]]) as usize;
        let total_field_len = 4 + data_size;

        if tag == TAG_DATA_STREAM_ALIGNMENT {
            return true;
        }

        if offset + total_field_len > extra_data.len() {
            break;
        }
        offset += total_field_len;
    }

    false
}

/// Parses the target alignment value from a `TAG_DATA_STREAM_ALIGNMENT` extra field payload.
pub fn parse_alignment_extra_field(payload: &[u8]) -> Option<u16> {
    if payload.len() < 2 {
        return None;
    }
    Some(u16::from_le_bytes([payload[0], payload[1]]))
}
