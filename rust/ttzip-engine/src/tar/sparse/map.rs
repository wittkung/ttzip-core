// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! GNU Sparse 0.0/0.1/1.0 and PAX Sparse extent map parser, hole calculator, and validator.
//!
//! Provides unified sparse extent representation (`SparseExtent`), full sparse file map
//! abstraction (`SparseMap`), hole range calculation for hardware-accelerated zero-copy punching
//! (APFS / Linux fallocate/seek_hole), and strict validation against overlapping or corrupted extents.

use std::io::Read;
use thiserror::Error;

use crate::tar::codec::numeric_extended_from;
use crate::tar::header::TarHeader;
use crate::tar::types::{GnuExtSparseHeader, BLOCK_SIZE};

/// Maximum number of sparse entries permitted in GNU Sparse 1.0 header (security guardrail).
pub const MAX_SPARSE_ENTRIES: usize = 1_000_000;

/// Maximum byte size permitted for GNU Sparse 1.0 header stream (64 MiB security guardrail).
pub const MAX_SPARSE_HEADER_BYTES: usize = 64 * 1024 * 1024;

/// Errors arising during TAR sparse header parsing, stream processing, or extent validation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TarSparseError {
    /// The TAR entry typeflag is not sparse (`'S'`).
    #[error("not a sparse TAR entry (typeflag is not 'S')")]
    NotSparseEntry,

    /// Extended sparse headers indicated by `isextended` are missing from the input stream.
    #[error("missing extended sparse header: expected more extended blocks")]
    MissingExtendedHeader,

    /// Invalid numeric field encountered in sparse header.
    #[error("invalid numeric field in sparse header: {0}")]
    InvalidNumeric(String),

    /// PAX sparse 0.1 map string is invalid or malformed.
    #[error("invalid PAX sparse 0.1 map string: {0}")]
    InvalidPaxMap(String),

    /// GNU sparse 1.0 stream text map is invalid or malformed.
    #[error("invalid GNU sparse 1.0 stream map: {0}")]
    InvalidStreamMap(String),

    /// A sparse extent exceeds the declared real uncompressed file size.
    #[error("sparse extent ({offset}..{}) exceeds declared real size {real_size}", .offset.saturating_add(*.numbytes))]
    ExceedsRealSize {
        offset: u64,
        numbytes: u64,
        real_size: u64,
    },

    /// Two sparse extents overlap within the file space.
    #[error("overlapping sparse extents: [{first_offset}..{first_end}) overlaps with [{second_offset}..{second_end})")]
    OverlappingExtents {
        first_offset: u64,
        first_end: u64,
        second_offset: u64,
        second_end: u64,
    },

    /// Sparse extents are not arranged in strictly non-decreasing offset order.
    #[error("disordered sparse extents: extent at offset {next_offset} appears after offset {prev_offset}")]
    DisorderedExtents {
        prev_offset: u64,
        next_offset: u64,
    },

    /// Arithmetic overflow occurred while calculating extent boundaries.
    #[error("integer overflow occurred in sparse extent calculation")]
    IntegerOverflow,

    /// Header size or entry count exceeds configured security bounds.
    #[error("sparse header size exceeds security limit of {max} bytes (found {size} bytes)")]
    HeaderTooLarge {
        size: usize,
        max: usize,
    },

    /// Generic I/O error during streaming sparse operations.
    #[error("I/O error during sparse stream parsing: {0}")]
    Io(String),
}

impl From<std::io::Error> for TarSparseError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

/// Continuous data extent descriptor inside a sparse file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SparseExtent {
    /// Logical byte offset within the unsparse target file.
    pub offset: u64,
    /// Length in bytes of the physical data block.
    pub numbytes: u64,
}

impl SparseExtent {
    /// Constructs a new `SparseExtent`.
    #[inline]
    pub const fn new(offset: u64, numbytes: u64) -> Self {
        Self { offset, numbytes }
    }

    /// Returns the logical end offset (`offset + numbytes`) with saturation.
    #[inline]
    pub const fn end_offset(&self) -> u64 {
        self.offset.saturating_add(self.numbytes)
    }

    /// Returns `true` if this extent contains zero bytes.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.numbytes == 0
    }
}

/// Complete logical map representing all data extents and real size of a sparse file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SparseMap {
    /// True logical unsparse file size in bytes.
    pub real_size: u64,
    /// Ordered list of contiguous data extents.
    pub extents: Vec<SparseExtent>,
}

impl SparseMap {
    /// Constructs a new `SparseMap` with the given `real_size` and `extents`.
    #[inline]
    pub fn new(real_size: u64, extents: Vec<SparseExtent>) -> Self {
        Self { real_size, extents }
    }

    /// Computes the total physical data bytes across all extents.
    #[inline]
    pub fn total_data_bytes(&self) -> u64 {
        self.extents.iter().map(|e| e.numbytes).sum()
    }

    /// Computes the total hole bytes in the sparse file.
    #[inline]
    pub fn total_hole_bytes(&self) -> u64 {
        self.real_size.saturating_sub(self.total_data_bytes())
    }

    /// Returns `true` if the map contains at least one hole.
    #[inline]
    pub fn has_holes(&self) -> bool {
        if self.real_size == 0 {
            return false;
        }
        if self.extents.is_empty() {
            return true;
        }
        if self.extents.len() == 1
            && self.extents[0].offset == 0
            && self.extents[0].numbytes >= self.real_size
        {
            return false;
        }
        self.total_data_bytes() < self.real_size
    }

    /// Calculates the list of unallocated hole ranges `(offset, length)` across the file.
    ///
    /// The resulting list can be directly utilized for fast zero-copy hole punching
    /// via OS primitives (`fcntl(F_PUNCHHOLE)`, `fallocate(FALLOC_FL_PUNCH_HOLE)`, etc.).
    pub fn calculate_hole_ranges(&self) -> Vec<(u64, u64)> {
        if self.real_size == 0 {
            return Vec::new();
        }
        if self.extents.is_empty() {
            return vec![(0, self.real_size)];
        }

        let mut holes = Vec::new();
        let mut current_offset: u64 = 0;

        for ext in &self.extents {
            if ext.offset > current_offset {
                holes.push((current_offset, ext.offset - current_offset));
            }
            current_offset = ext.offset.saturating_add(ext.numbytes);
        }

        if current_offset < self.real_size {
            holes.push((current_offset, self.real_size - current_offset));
        }

        holes
    }

    /// Formats GNU Sparse 0.1 comma-separated map string (`"offset,length,offset,length..."`).
    pub fn to_gnu_0_1_map_string(&self) -> String {
        let mut result = String::new();
        for (i, extent) in self.extents.iter().enumerate() {
            if i > 0 {
                result.push(',');
            }
            result.push_str(&extent.offset.to_string());
            result.push(',');
            result.push_str(&extent.numbytes.to_string());
        }
        result
    }

    /// Formats GNU Sparse 1.0 text-based header block (padded to 512 bytes).
    pub fn to_gnu_1_0_map_block(&self) -> Vec<u8> {
        let mut text = format!("{}\n", self.extents.len());
        for extent in &self.extents {
            text.push_str(&format!("{}\n{}\n", extent.offset, extent.numbytes));
        }

        let mut bytes = text.into_bytes();
        let remainder = bytes.len() % BLOCK_SIZE;
        if remainder != 0 {
            let pad = BLOCK_SIZE - remainder;
            bytes.resize(bytes.len() + pad, 0);
        }
        bytes
    }

    /// Validates the sparse map for integrity, monotonicity, non-overlapping bounds,
    /// and ensures no extents exceed `real_size`.
    pub fn validate_sparse_map(&self) -> Result<(), TarSparseError> {
        let mut prev_end: u64 = 0;

        for (i, ext) in self.extents.iter().enumerate() {
            // Check for integer overflow
            let end = ext
                .offset
                .checked_add(ext.numbytes)
                .ok_or(TarSparseError::IntegerOverflow)?;

            if end > self.real_size {
                return Err(TarSparseError::ExceedsRealSize {
                    offset: ext.offset,
                    numbytes: ext.numbytes,
                    real_size: self.real_size,
                });
            }

            if i > 0 {
                let prev = &self.extents[i - 1];
                if ext.offset < prev.offset {
                    return Err(TarSparseError::DisorderedExtents {
                        prev_offset: prev.offset,
                        next_offset: ext.offset,
                    });
                }
                if ext.offset < prev_end {
                    return Err(TarSparseError::OverlappingExtents {
                        first_offset: prev.offset,
                        first_end: prev_end,
                        second_offset: ext.offset,
                        second_end: end,
                    });
                }
            }

            prev_end = end;
        }

        Ok(())
    }
}

/// Parses GNU Sparse 0.0 or 0.1 sparse map from standard TAR header and optional extended headers.
///
/// In GNU 0.0 format, up to 4 sparse extents are embedded directly in the 512-byte header.
/// In GNU 0.1 format, if `isextended` is set (`1` or `'1'`), chained `GnuExtSparseHeader` blocks
/// (each containing up to 21 sparse extents) follow before the data payload.
pub fn parse_gnu_sparse_0_x(
    header: &TarHeader,
    ext_headers: &[GnuExtSparseHeader],
) -> Result<SparseMap, TarSparseError> {
    if !header.entry_type().is_sparse() {
        return Err(TarSparseError::NotSparseEntry);
    }

    let gnu = header.as_gnu_header();
    let real_size = numeric_extended_from(&gnu.realsize);

    let mut extents = Vec::new();

    // 1. Process 4 embedded sparse extents in the primary 512-byte GNU header
    for slot in &gnu.sparse {
        let numbytes = numeric_extended_from(&slot.numbytes);
        if numbytes > 0 {
            let offset = numeric_extended_from(&slot.offset);
            extents.push(SparseExtent::new(offset, numbytes));
        }
    }

    // 2. Check if extended sparse headers follow
    let mut is_extended = gnu.isextended == 1 || gnu.isextended == b'1';
    let mut ext_idx = 0;

    while is_extended {
        if ext_idx >= ext_headers.len() {
            return Err(TarSparseError::MissingExtendedHeader);
        }

        let ext_header = &ext_headers[ext_idx];
        ext_idx += 1;

        for slot in &ext_header.sparse {
            let numbytes = numeric_extended_from(&slot.numbytes);
            if numbytes > 0 {
                let offset = numeric_extended_from(&slot.offset);
                extents.push(SparseExtent::new(offset, numbytes));
            }
        }

        is_extended = ext_header.isextended == 1 || ext_header.isextended == b'1';
    }

    let map = SparseMap::new(real_size, extents);
    map.validate_sparse_map()?;
    Ok(map)
}

/// Parses PAX Sparse 0.1 map string (`GNU.sparse.map = "offset1,numbytes1,offset2,numbytes2,..."`).
pub fn parse_pax_sparse_0_1(
    pax_map_str: &str,
    real_size: u64,
) -> Result<SparseMap, TarSparseError> {
    let trimmed = pax_map_str.trim();
    if trimmed.is_empty() {
        let map = SparseMap::new(real_size, Vec::new());
        map.validate_sparse_map()?;
        return Ok(map);
    }

    let tokens: Vec<&str> = trimmed
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if !tokens.len().is_multiple_of(2) {
        return Err(TarSparseError::InvalidPaxMap(format!(
            "odd number of values in sparse map (found {} items)",
            tokens.len()
        )));
    }

    let mut extents = Vec::with_capacity(tokens.len() / 2);

    for chunk in tokens.chunks_exact(2) {
        let offset = chunk[0].parse::<u64>().map_err(|e| {
            TarSparseError::InvalidPaxMap(format!("invalid offset '{}': {}", chunk[0], e))
        })?;
        let numbytes = chunk[1].parse::<u64>().map_err(|e| {
            TarSparseError::InvalidPaxMap(format!("invalid numbytes '{}': {}", chunk[1], e))
        })?;

        if numbytes > 0 {
            extents.push(SparseExtent::new(offset, numbytes));
        }
    }

    let map = SparseMap::new(real_size, extents);
    map.validate_sparse_map()?;
    Ok(map)
}

/// Parses GNU Sparse 1.0 stream prefix (`num_entries\noffset\nnumbytes...`) from reader.
///
/// Returns the parsed `SparseMap` and the exact total number of bytes consumed from `reader`
/// (which is rounded up to a 512-byte sector multiple).
pub fn parse_gnu_sparse_1_0_stream<R: Read>(
    reader: &mut R,
    real_size: u64,
) -> Result<(SparseMap, usize), TarSparseError> {
    let mut sector_buf = [0u8; BLOCK_SIZE];
    let mut raw_bytes = Vec::new();
    let mut total_sectors_read = 0usize;

    let mut num_entries: Option<usize> = None;
    let mut lines = Vec::new();
    let mut current_line_start = 0usize;

    loop {
        reader.read_exact(&mut sector_buf).map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                TarSparseError::InvalidStreamMap(
                    "unexpected EOF while reading sparse 1.0 header".to_string(),
                )
            } else {
                TarSparseError::from(e)
            }
        })?;

        total_sectors_read += 1;
        raw_bytes.extend_from_slice(&sector_buf);

        if raw_bytes.len() > MAX_SPARSE_HEADER_BYTES {
            return Err(TarSparseError::HeaderTooLarge {
                size: raw_bytes.len(),
                max: MAX_SPARSE_HEADER_BYTES,
            });
        }

        while let Some(nl_pos) = raw_bytes[current_line_start..]
            .iter()
            .position(|&b| b == b'\n')
        {
            let line_end = current_line_start + nl_pos;
            let line_bytes = &raw_bytes[current_line_start..line_end];
            let line_str = std::str::from_utf8(line_bytes)
                .map_err(|e| {
                    TarSparseError::InvalidStreamMap(format!(
                        "invalid UTF-8 sequence in stream map: {}",
                        e
                    ))
                })?
                .trim();

            lines.push(line_str.to_string());
            current_line_start = line_end + 1;

            if num_entries.is_none() {
                let count = lines[0].parse::<usize>().map_err(|e| {
                    TarSparseError::InvalidStreamMap(format!(
                        "invalid num_entries count '{}': {}",
                        lines[0], e
                    ))
                })?;
                if count > MAX_SPARSE_ENTRIES {
                    return Err(TarSparseError::HeaderTooLarge {
                        size: count,
                        max: MAX_SPARSE_ENTRIES,
                    });
                }
                num_entries = Some(count);
            }

            if let Some(target_entries) = num_entries {
                let required_lines = 1 + target_entries * 2;
                if lines.len() == required_lines {
                    let mut extents = Vec::with_capacity(target_entries);
                    for i in 0..target_entries {
                        let offset_idx = 1 + i * 2;
                        let numbytes_idx = 1 + i * 2 + 1;
                        let offset = lines[offset_idx].parse::<u64>().map_err(|e| {
                            TarSparseError::InvalidStreamMap(format!(
                                "invalid offset '{}' at entry {}: {}",
                                lines[offset_idx], i, e
                            ))
                        })?;
                        let numbytes = lines[numbytes_idx].parse::<u64>().map_err(|e| {
                            TarSparseError::InvalidStreamMap(format!(
                                "invalid numbytes '{}' at entry {}: {}",
                                lines[numbytes_idx], i, e
                            ))
                        })?;
                        if numbytes > 0 {
                            extents.push(SparseExtent::new(offset, numbytes));
                        }
                    }

                    let map = SparseMap::new(real_size, extents);
                    map.validate_sparse_map()?;
                    let total_consumed = total_sectors_read * BLOCK_SIZE;
                    return Ok((map, total_consumed));
                }
            }
        }
    }
}
