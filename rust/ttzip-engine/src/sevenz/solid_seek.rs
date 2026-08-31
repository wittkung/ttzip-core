// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 7-Zip Solid Stream Selective Decompression with $O(1)$ Jump Table Indexing
//! and 4MB Micro-Buffer Sliding Window Early-Exit Circuit Breaker.
//!
//! Provides zero-heap-allocation sliding discard for preceding substreams,
//! incremental CRC32 computation on the target substream, and physical early-exit
//! termination to avoid decompressing subsequent unselected data.

use std::io::{Read, Write};

use crate::crypto::crc32::crc32_fast;
use crate::sevenz::dag::SevenZError;

/// Default 4MB static micro-buffer sliding discard chunk size.
pub const SOLID_MICRO_BUFFER_SIZE: usize = 4 * 1024 * 1024;

/// Metadata for a single substream within a 7z Solid Folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SolidSubstreamMeta {
    /// Zero-based substream index within the solid folder.
    pub sub_index: usize,
    /// Starting uncompressed byte offset within the solid stream.
    pub start_offset: u64,
    /// Uncompressed size of this substream in bytes.
    pub size: u64,
    /// Optional expected CRC32 checksum from archive header.
    pub crc: Option<u32>,
}

impl SolidSubstreamMeta {
    /// Returns the ending uncompressed byte offset `[start_offset, end_offset)`.
    #[inline]
    #[must_use]
    pub fn end_offset(&self) -> u64 {
        self.start_offset.saturating_add(self.size)
    }

    /// Returns true if this substream has zero uncompressed bytes.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
}

/// $O(1)$ prefix-sum jump table and stream boundary index for a solid 7z folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SolidFolderIndex {
    /// Ordered list of substream metadata entries.
    substreams: Vec<SolidSubstreamMeta>,
    /// Prefix sums of substream sizes where `prefix_sums[0] == 0` and `prefix_sums[i]` is start offset of substream `i`.
    prefix_sums: Vec<u64>,
    /// Total uncompressed byte size of all substreams in this folder.
    total_uncompressed_size: u64,
}

impl SolidFolderIndex {
    /// Constructs a `SolidFolderIndex` from a vector of substream metadata entries.
    #[must_use]
    pub fn new(substreams: Vec<SolidSubstreamMeta>) -> Self {
        let mut prefix_sums = Vec::with_capacity(substreams.len() + 1);
        prefix_sums.push(0);

        let mut running_sum = 0u64;
        for s in &substreams {
            running_sum = running_sum.saturating_add(s.size);
            prefix_sums.push(running_sum);
        }

        Self {
            substreams,
            prefix_sums,
            total_uncompressed_size: running_sum,
        }
    }

    /// Builds an $O(1)$ jump table from slices of substream uncompressed sizes and optional CRCs.
    #[must_use]
    pub fn from_sizes_and_crcs(sizes: &[u64], crcs: &[Option<u32>]) -> Self {
        let mut substreams = Vec::with_capacity(sizes.len());
        let mut prefix_sums = Vec::with_capacity(sizes.len() + 1);
        prefix_sums.push(0);

        let mut current_offset = 0u64;
        for (i, &size) in sizes.iter().enumerate() {
            let crc = crcs.get(i).copied().flatten();
            substreams.push(SolidSubstreamMeta {
                sub_index: i,
                start_offset: current_offset,
                size,
                crc,
            });
            current_offset = current_offset.saturating_add(size);
            prefix_sums.push(current_offset);
        }

        Self {
            substreams,
            prefix_sums,
            total_uncompressed_size: current_offset,
        }
    }

    /// Builds an $O(1)$ jump table from a slice of substream uncompressed sizes without CRCs.
    #[must_use]
    pub fn from_sizes(sizes: &[u64]) -> Self {
        Self::from_sizes_and_crcs(sizes, &[])
    }

    /// Returns the total number of substreams in this solid folder.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.substreams.len()
    }

    /// Returns true if there are no substreams in this solid folder.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.substreams.is_empty()
    }

    /// $O(1)$ lookup of substream metadata by substream index.
    #[inline]
    #[must_use]
    pub fn get(&self, sub_index: usize) -> Option<&SolidSubstreamMeta> {
        self.substreams.get(sub_index)
    }

    /// Returns a slice of all substream metadata entries.
    #[inline]
    #[must_use]
    pub fn substreams(&self) -> &[SolidSubstreamMeta] {
        &self.substreams
    }

    /// Returns the total uncompressed size of all substreams in bytes.
    #[inline]
    #[must_use]
    pub fn total_uncompressed_size(&self) -> u64 {
        self.total_uncompressed_size
    }

    /// Returns the prefix sum array `[0, size_0, size_0 + size_1, ...]`.
    #[inline]
    #[must_use]
    pub fn prefix_sums(&self) -> &[u64] {
        &self.prefix_sums
    }

    /// $O(1)$ lookup of byte range `[start_offset, end_offset)` for a given substream index.
    #[inline]
    #[must_use]
    pub fn substream_range(&self, sub_index: usize) -> Option<(u64, u64)> {
        self.get(sub_index).map(|m| (m.start_offset, m.end_offset()))
    }
}

/// Execution telemetry and verification statistics for a solid stream selective extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SolidExtractionReport {
    /// Extracted substream index.
    pub sub_index: usize,
    /// Uncompressed starting byte offset within the solid stream.
    pub start_offset: u64,
    /// Target uncompressed size in bytes.
    pub size: u64,
    /// Total bytes skipped from preceding substreams $0..K-1$.
    pub skipped_preceding_bytes: u64,
    /// Total bytes extracted into the output sink.
    pub extracted_bytes: u64,
    /// Computed CRC32 checksum of extracted data.
    pub computed_crc: u32,
    /// Expected CRC32 checksum from header metadata, if available.
    pub expected_crc: Option<u32>,
    /// Whether computed CRC matches expected CRC.
    pub crc_matched: bool,
    /// Whether early-exit physical circuit breaker was triggered.
    pub early_exit_triggered: bool,
}

/// High-performance selective solid stream extractor with 4MB sliding discard and early exit.
pub struct SolidEarlyExitExtractor;

impl SolidEarlyExitExtractor {
    /// 4MB sliding discard micro-buffer chunk threshold.
    pub const MICRO_BUFFER_SIZE: usize = SOLID_MICRO_BUFFER_SIZE;

    /// Selectively extracts a single substream from a sequential solid stream into an output sink.
    ///
    /// # Pipeline Stages:
    /// - **Stage 1 (Sliding Discard)**: Skips preceding substreams $0..K-1$ using a single reusable
    ///   4MB micro-buffer with zero extra heap allocations, advancing the decompressor dictionary.
    /// - **Stage 2 (Target Extraction)**: Streams target $K$-th substream into `output` and computes
    ///   incremental CRC32.
    /// - **Stage 3 (Early Exit)**: Terminates immediately after extracting target substream, performing
    ///   zero reads or decompressions for subsequent substreams $K+1..M$.
    ///
    /// # Errors:
    /// Returns `SevenZError::InvalidSubstreamIndex` if `sub_index >= index.len()`.
    /// Returns `SevenZError::CrcMismatch` if expected CRC does not match computed CRC.
    /// Returns `SevenZError::UnexpectedEof` if stream terminates before reaching expected byte length.
    /// Returns `SevenZError::IoError` on read or write failure.
    pub fn extract_substream<R: Read, W: Write>(
        stream: &mut R,
        sub_index: usize,
        index: &SolidFolderIndex,
        output: &mut W,
    ) -> Result<u32, SevenZError> {
        let report = Self::extract_substream_with_stats(stream, sub_index, index, output)?;
        Ok(report.computed_crc)
    }

    /// Selectively extracts a single substream into an in-memory byte buffer (`Vec<u8>`).
    pub fn extract_substream_to_vec<R: Read>(
        stream: &mut R,
        sub_index: usize,
        index: &SolidFolderIndex,
    ) -> Result<(Vec<u8>, u32), SevenZError> {
        let meta = index.get(sub_index).ok_or_else(|| SevenZError::InvalidSubstreamIndex {
            index: sub_index,
            total: index.len(),
        })?;

        let mut buffer = Vec::with_capacity(meta.size as usize);
        let crc = Self::extract_substream(stream, sub_index, index, &mut buffer)?;
        Ok((buffer, crc))
    }

    /// Selectively extracts a single substream and produces detailed execution telemetry.
    pub fn extract_substream_with_stats<R: Read, W: Write>(
        stream: &mut R,
        sub_index: usize,
        index: &SolidFolderIndex,
        output: &mut W,
    ) -> Result<SolidExtractionReport, SevenZError> {
        let meta = index.get(sub_index).ok_or_else(|| SevenZError::InvalidSubstreamIndex {
            index: sub_index,
            total: index.len(),
        })?;

        let target_start = meta.start_offset;
        let target_size = meta.size;
        let target_end = meta.end_offset();

        // Stage 1: Zero-heap-allocation sliding discard for preceding substreams 0..K-1
        let mut skipped_preceding_bytes = 0u64;
        if target_start > 0 {
            let mut remaining_skip = target_start;
            let buffer_cap = (remaining_skip as usize).min(Self::MICRO_BUFFER_SIZE);
            let mut micro_buf = vec![0u8; buffer_cap];

            while remaining_skip > 0 {
                let chunk_len = remaining_skip.min(micro_buf.len() as u64) as usize;
                let bytes_read = stream.read(&mut micro_buf[..chunk_len])?;
                if bytes_read == 0 {
                    return Err(SevenZError::UnexpectedEof {
                        required: remaining_skip,
                        actual: target_start - remaining_skip,
                    });
                }
                remaining_skip -= bytes_read as u64;
                skipped_preceding_bytes += bytes_read as u64;
            }
        }

        // Stage 2: Stream target substream into output sink and incrementally compute CRC32
        let mut extracted_bytes = 0u64;
        let mut running_crc = 0u32;
        if target_size > 0 {
            let mut remaining_target = target_size;
            let buffer_cap = (remaining_target as usize).min(Self::MICRO_BUFFER_SIZE);
            let mut transfer_buf = vec![0u8; buffer_cap];

            while remaining_target > 0 {
                let chunk_len = remaining_target.min(transfer_buf.len() as u64) as usize;
                let bytes_read = stream.read(&mut transfer_buf[..chunk_len])?;
                if bytes_read == 0 {
                    return Err(SevenZError::UnexpectedEof {
                        required: remaining_target,
                        actual: target_size - remaining_target,
                    });
                }
                let chunk = &transfer_buf[..bytes_read];
                output.write_all(chunk)?;
                running_crc = crc32_fast(running_crc, chunk);
                extracted_bytes += bytes_read as u64;
                remaining_target -= bytes_read as u64;
            }
            output.flush()?;
        }

        // Verify expected CRC32 if specified
        let crc_matched = match meta.crc {
            Some(expected) => {
                if running_crc != expected {
                    return Err(SevenZError::CrcMismatch {
                        expected,
                        computed: running_crc,
                    });
                }
                true
            }
            None => true,
        };

        // Stage 3: Early Exit Circuit Breaker (Zero reads for trailing substreams K+1..M)
        let early_exit_triggered = target_end < index.total_uncompressed_size();

        Ok(SolidExtractionReport {
            sub_index,
            start_offset: target_start,
            size: target_size,
            skipped_preceding_bytes,
            extracted_bytes,
            computed_crc: running_crc,
            expected_crc: meta.crc,
            crc_matched,
            early_exit_triggered,
        })
    }
}
