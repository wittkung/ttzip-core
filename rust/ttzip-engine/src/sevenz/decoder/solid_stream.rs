// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! High-performance 7-Zip Solid Stream selective extraction engine with O(1) index jump tables
//! and 4MB micro-buffer sliding window early-exit decompression.

use std::collections::HashMap;
use std::io::Write;

use super::payload::decode_7z_folder_streaming;
use crate::crypto::crc32::crc32_fast;
use crate::sevenz::header::SevenZHeaderInfo;
use crate::types::TTZipStatus;

/// 4MB sliding-window micro-buffer chunk size for 7z solid stream decompression.
pub const SOLID_MICRO_BUFFER_CHUNK_SIZE: usize = 4 * 1024 * 1024;

/// Spatial offset range and metadata for a single file entry in a solid 7z stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolidFileRange {
    pub file_index: usize,
    pub rel_path: String,
    pub is_directory: bool,
    pub is_empty_stream: bool,
    pub folder_index: Option<usize>,
    pub substream_index: Option<usize>,
    pub global_stream_index: Option<usize>,
    pub offset_start: u64,
    pub offset_end: u64,
    pub uncompressed_size: u64,
    pub crc: Option<u32>,
}

/// Pre-computed prefix-sum jump table and stream boundaries for a single 7z Folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolidFolderTable {
    pub folder_index: usize,
    /// Prefix sums of sub-stream uncompressed lengths:
    /// `stream_prefix_sums[0] == 0`,
    /// `stream_prefix_sums[i]` is the byte offset of sub-stream `i` in uncompressed folder stream,
    /// `stream_prefix_sums[N]` is the total uncompressed size of all sub-streams in this folder.
    pub stream_prefix_sums: Vec<u64>,
    /// Global stream index corresponding to each local sub-stream in this folder.
    pub global_stream_indices: Vec<usize>,
    /// File index corresponding to each local sub-stream in this folder.
    pub substream_to_file_index: Vec<usize>,
    /// Total uncompressed size of this folder (from coders unpack_sizes or sum of sub-streams).
    pub total_uncompressed_size: u64,
    /// Byte offset in archive where this folder's packed payload begins.
    pub packed_offset: usize,
    /// Packed byte length of this folder in archive.
    pub packed_len: usize,
    /// CRC of the folder if present.
    pub folder_crc: Option<u32>,
}

/// Pre-parsed O(1) jump table index for 7-Zip solid stream archives.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SolidFolderIndex {
    /// Jump tables for each folder in the 7z archive.
    pub folders: Vec<SolidFolderTable>,
    /// O(1) index array mapping `file_index` -> `SolidFileRange`.
    pub entries: Vec<SolidFileRange>,
    /// Fast lookup mapping normalized path -> `file_index`.
    path_map: HashMap<String, usize>,
}

impl SolidFolderIndex {
    /// Builds an O(1) random-access seek and prefix sum jump table from parsed 7z metadata header.
    pub fn build(info: &SevenZHeaderInfo) -> Self {
        let num_files = info.files.len();
        let mut entries = Vec::with_capacity(num_files);
        let mut path_map = HashMap::with_capacity(num_files * 2);

        let mut folders: Vec<SolidFolderTable> = Vec::with_capacity(info.folders.len().max(1));
        if !info.folders.is_empty() {
            for (f_idx, f) in info.folders.iter().enumerate() {
                let total_uncomp = f.unpack_sizes.last().copied().unwrap_or(0);
                folders.push(SolidFolderTable {
                    folder_index: f_idx,
                    stream_prefix_sums: vec![0],
                    global_stream_indices: Vec::new(),
                    substream_to_file_index: Vec::new(),
                    total_uncompressed_size: total_uncomp,
                    packed_offset: f.packed_offset,
                    packed_len: f.packed_len,
                    folder_crc: f.crc,
                });
            }
        } else {
            let total_sz = if !info.stream_sizes.is_empty() {
                info.stream_sizes.iter().sum()
            } else {
                info.payload_len as u64
            };
            folders.push(SolidFolderTable {
                folder_index: 0,
                stream_prefix_sums: vec![0],
                global_stream_indices: Vec::new(),
                substream_to_file_index: Vec::new(),
                total_uncompressed_size: total_sz,
                packed_offset: info.payload_offset,
                packed_len: info.payload_len,
                folder_crc: None,
            });
        }

        let mut global_stream_idx = 0usize;
        let mut cur_folder_idx = 0usize;
        let mut streams_in_cur_folder = 0usize;
        let mut current_folder_offset = 0u64;

        for (file_idx, f) in info.files.iter().enumerate() {
            if f.is_directory || f.is_empty_stream {
                let range = SolidFileRange {
                    file_index: file_idx,
                    rel_path: f.rel_path.clone(),
                    is_directory: f.is_directory,
                    is_empty_stream: f.is_empty_stream,
                    folder_index: None,
                    substream_index: None,
                    global_stream_index: None,
                    offset_start: 0,
                    offset_end: 0,
                    uncompressed_size: 0,
                    crc: None,
                };
                entries.push(range);
            } else {
                // Advance to next folder when current folder's stream budget is exhausted
                while cur_folder_idx < info.folders.len() {
                    let folder_streams = info.folders[cur_folder_idx].num_unpack_streams.max(1);
                    if streams_in_cur_folder < folder_streams {
                        break;
                    }
                    if cur_folder_idx + 1 < info.folders.len() {
                        cur_folder_idx += 1;
                        streams_in_cur_folder = 0;
                        current_folder_offset = 0;
                    } else {
                        break;
                    }
                }

                let folder_idx = if !info.folders.is_empty() && cur_folder_idx < info.folders.len() {
                    cur_folder_idx
                } else {
                    0
                };

                let sz = if global_stream_idx < info.stream_sizes.len() {
                    info.stream_sizes[global_stream_idx]
                } else if cur_folder_idx < info.folders.len() {
                    info.folders[cur_folder_idx]
                        .unpack_sizes
                        .last()
                        .copied()
                        .unwrap_or(0)
                        .saturating_sub(current_folder_offset)
                } else {
                    0
                };

                let crc = info.stream_crcs.get(global_stream_idx).copied();
                let offset_start = current_folder_offset;
                let offset_end = current_folder_offset + sz;

                if folder_idx < folders.len() {
                    let ft = &mut folders[folder_idx];
                    ft.global_stream_indices.push(global_stream_idx);
                    ft.substream_to_file_index.push(file_idx);
                    ft.stream_prefix_sums.push(offset_end);
                }

                let range = SolidFileRange {
                    file_index: file_idx,
                    rel_path: f.rel_path.clone(),
                    is_directory: false,
                    is_empty_stream: false,
                    folder_index: Some(folder_idx),
                    substream_index: Some(streams_in_cur_folder),
                    global_stream_index: Some(global_stream_idx),
                    offset_start,
                    offset_end,
                    uncompressed_size: sz,
                    crc,
                };
                entries.push(range);

                current_folder_offset = offset_end;
                streams_in_cur_folder += 1;
                global_stream_idx += 1;
            }

            // Populate path map
            path_map.insert(f.rel_path.clone(), file_idx);
            let normalized = f.rel_path.replace('\\', "/");
            let normalized_trimmed = normalized.trim_start_matches('/').to_string();
            path_map.insert(normalized_trimmed, file_idx);
        }

        // Ensure each folder table has updated total uncompressed size from prefix sums if 0
        for ft in &mut folders {
            if ft.total_uncompressed_size == 0 {
                ft.total_uncompressed_size = ft.stream_prefix_sums.last().copied().unwrap_or(0);
            }
        }

        Self {
            folders,
            entries,
            path_map,
        }
    }

    /// O(1) instant lookup of file range and folder location by file index.
    #[inline]
    pub fn lookup(&self, file_index: usize) -> Option<&SolidFileRange> {
        self.entries.get(file_index)
    }

    /// Fast lookup of file range by archive relative path.
    #[inline]
    pub fn lookup_by_path(&self, path: &str) -> Option<&SolidFileRange> {
        if let Some(&idx) = self.path_map.get(path) {
            return self.entries.get(idx);
        }
        let normalized = path.replace('\\', "/");
        let normalized_trimmed = normalized.trim_start_matches('/');
        if let Some(&idx) = self.path_map.get(normalized_trimmed) {
            return self.entries.get(idx);
        }
        self.entries.iter().find(|e| {
            let e_norm = e.rel_path.replace('\\', "/");
            let e_norm_trimmed = e_norm.trim_start_matches('/');
            e_norm_trimmed == normalized_trimmed
                || e.rel_path == path
                || e_norm_trimmed.ends_with(normalized_trimmed)
                || normalized_trimmed.ends_with(e_norm_trimmed)
        })
    }

    /// Returns the total number of folders in the archive.
    #[inline]
    pub fn folder_count(&self) -> usize {
        self.folders.len()
    }

    /// Returns the total number of file entries in the archive.
    #[inline]
    pub fn file_count(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the archive contains no files.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns reference to folder jump table by folder index.
    #[inline]
    pub fn folder(&self, folder_idx: usize) -> Option<&SolidFolderTable> {
        self.folders.get(folder_idx)
    }

    /// Returns the total uncompressed byte size of a folder.
    #[inline]
    pub fn folder_total_size(&self, folder_idx: usize) -> Option<u64> {
        self.folders.get(folder_idx).map(|f| f.total_uncompressed_size)
    }

    /// Returns the number of sub-streams in a folder.
    #[inline]
    pub fn folder_stream_count(&self, folder_idx: usize) -> Option<usize> {
        self.folders.get(folder_idx).map(|f| f.substream_to_file_index.len())
    }

    /// O(1) calculation of byte offset range `[offset_start, offset_end)` for a sub-stream inside a folder.
    #[inline]
    pub fn folder_stream_range(&self, folder_idx: usize, substream_idx: usize) -> Option<(u64, u64)> {
        let folder = self.folders.get(folder_idx)?;
        if substream_idx + 1 < folder.stream_prefix_sums.len() {
            let start = folder.stream_prefix_sums[substream_idx];
            let end = folder.stream_prefix_sums[substream_idx + 1];
            Some((start, end))
        } else {
            None
        }
    }
}

/// Statistics and execution report for solid stream selective extraction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SolidExtractionStats {
    pub folder_index: usize,
    pub target_offset_start: u64,
    pub target_offset_end: u64,
    pub decompressed_bytes_total: u64,
    pub skipped_preceding_bytes: u64,
    pub extracted_target_bytes: u64,
    pub early_exit_triggered: bool,
    pub computed_crc: u32,
    pub expected_crc: Option<u32>,
    pub crc_matched: bool,
}

/// High-performance 4MB micro-buffer sliding window solid extractor with Early Exit Termination.
pub struct SolidEarlyExitExtractor<'a> {
    archive_data: &'a [u8],
    header_info: &'a SevenZHeaderInfo,
    folder_index: &'a SolidFolderIndex,
    thread_budget: u32,
    max_preceding_budget_bytes: u64,
}

impl<'a> SolidEarlyExitExtractor<'a> {
    /// Creates a new early exit solid extractor bound to archive data and index.
    pub fn new(
        archive_data: &'a [u8],
        header_info: &'a SevenZHeaderInfo,
        folder_index: &'a SolidFolderIndex,
    ) -> Self {
        Self {
            archive_data,
            header_info,
            folder_index,
            thread_budget: 1,
            max_preceding_budget_bytes: 0,
        }
    }

    /// Sets decompression thread parallelism budget (defaults to 1).
    pub fn with_threads(mut self, threads: u32) -> Self {
        self.thread_budget = threads.max(1);
        self
    }

    /// Sets maximum preceding discarded byte budget (0 = unlimited).
    pub fn with_preceding_budget(mut self, max_bytes: u64) -> Self {
        self.max_preceding_budget_bytes = max_bytes;
        self
    }

    /// Extracts entry into an in-memory buffer (`Vec<u8>`) with early exit and CRC32 verification.
    pub fn extract_to_vec(
        &self,
        file_idx: usize,
        password: Option<&str>,
    ) -> Result<(Vec<u8>, SolidExtractionStats), TTZipStatus> {
        let loc = self.folder_index.lookup(file_idx).ok_or(TTZipStatus::ErrInvalidOffset)?;
        let mut buffer = Vec::with_capacity(loc.uncompressed_size as usize);
        let stats = self.extract_streaming(file_idx, password, |chunk| {
            buffer.extend_from_slice(chunk);
            Ok(())
        })?;
        Ok((buffer, stats))
    }

    /// Extracts entry by relative path into an in-memory buffer (`Vec<u8>`).
    pub fn extract_by_path_to_vec(
        &self,
        path: &str,
        password: Option<&str>,
    ) -> Result<(Vec<u8>, SolidExtractionStats), TTZipStatus> {
        let loc = self.folder_index.lookup_by_path(path).ok_or(TTZipStatus::ErrFileNotFound)?;
        self.extract_to_vec(loc.file_index, password)
    }

    /// Extracts entry directly into any `std::io::Write` destination with zero intermediate memory allocations.
    pub fn extract_to_writer<W: Write>(
        &self,
        file_idx: usize,
        password: Option<&str>,
        writer: &mut W,
    ) -> Result<SolidExtractionStats, TTZipStatus> {
        self.extract_streaming(file_idx, password, |chunk| {
            writer.write_all(chunk).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
            Ok(())
        })
    }

    /// Streams extracted entry chunks into a custom callback with 4MB sliding-window micro-buffering and early exit.
    pub fn extract_streaming<F>(
        &self,
        file_idx: usize,
        password: Option<&str>,
        mut sink: F,
    ) -> Result<SolidExtractionStats, TTZipStatus>
    where
        F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
    {
        let loc = self.folder_index.lookup(file_idx).ok_or(TTZipStatus::ErrInvalidOffset)?;

        // Short-circuit directory, empty stream, or zero-byte file
        if loc.is_directory || loc.is_empty_stream || loc.uncompressed_size == 0 {
            return Ok(SolidExtractionStats {
                folder_index: loc.folder_index.unwrap_or(0),
                target_offset_start: 0,
                target_offset_end: 0,
                decompressed_bytes_total: 0,
                skipped_preceding_bytes: 0,
                extracted_target_bytes: 0,
                early_exit_triggered: false,
                computed_crc: 0,
                expected_crc: loc.crc,
                crc_matched: true,
            });
        }

        let folder_idx = loc.folder_index.ok_or(TTZipStatus::ErrCorruptHeader)?;
        let target_start = loc.offset_start;
        let target_end = loc.offset_end;
        let target_len = loc.uncompressed_size;

        if self.max_preceding_budget_bytes > 0 && target_start > self.max_preceding_budget_bytes {
            return Err(TTZipStatus::ErrSolidBudgetExceeded);
        }

        let folder_total = self.folder_index.folder_total_size(folder_idx).unwrap_or(target_end);

        let mut current_offset: u64 = 0;
        let mut decompressed_total: u64 = 0;
        let mut skipped_preceding: u64 = 0;
        let mut extracted_bytes: u64 = 0;
        let mut running_crc: u32 = 0;
        let mut early_exit_signal_sent = false;

        let decode_res = decode_7z_folder_streaming(
            self.archive_data,
            self.header_info,
            folder_idx,
            password,
            self.thread_budget,
            |chunk| -> Result<(), TTZipStatus> {
                let chunk_start = current_offset;
                let chunk_len = chunk.len() as u64;
                let chunk_end = chunk_start + chunk_len;
                current_offset += chunk_len;
                decompressed_total += chunk_len;

                // 1. Preceding range [0, target_start): micro-buffer sliding discard
                if chunk_end <= target_start {
                    skipped_preceding += chunk_len;
                    return Ok(());
                }

                // 2. Overlapping / Target range [target_start, target_end)
                if chunk_start < target_end && chunk_end > target_start {
                    if chunk_start < target_start {
                        skipped_preceding += target_start - chunk_start;
                    }
                    let slice_start = (target_start.saturating_sub(chunk_start)) as usize;
                    let slice_end = (target_end.min(chunk_end) - chunk_start) as usize;
                    let target_slice = &chunk[slice_start..slice_end];
                    if !target_slice.is_empty() {
                        sink(target_slice)?;
                        running_crc = crc32_fast(running_crc, target_slice);
                        extracted_bytes += target_slice.len() as u64;
                    }
                }

                // 3. Trailing range >= target_end: Early-Exit Termination
                if current_offset >= target_end {
                    early_exit_signal_sent = true;
                    return Err(TTZipStatus::Eof);
                }

                Ok(())
            },
        );

        match decode_res {
            Ok(_) | Err(TTZipStatus::Eof) => {}
            Err(e) => return Err(e),
        }

        if extracted_bytes != target_len {
            return Err(TTZipStatus::ErrExtractionFailed);
        }

        let crc_matched = match loc.crc {
            Some(exp) => {
                if exp == 0 && extracted_bytes == 0 {
                    true
                } else {
                    running_crc == exp
                }
            }
            None => true,
        };

        if !crc_matched {
            if self.header_info.is_encrypted {
                return Err(TTZipStatus::ErrInvalidPassword);
            } else {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
        }

        let early_exit_triggered =
            early_exit_signal_sent || (target_end < folder_total && decompressed_total < folder_total);

        Ok(SolidExtractionStats {
            folder_index: folder_idx,
            target_offset_start: target_start,
            target_offset_end: target_end,
            decompressed_bytes_total: decompressed_total,
            skipped_preceding_bytes: skipped_preceding,
            extracted_target_bytes: extracted_bytes,
            early_exit_triggered,
            computed_crc: running_crc,
            expected_crc: loc.crc,
            crc_matched,
        })
    }
}
