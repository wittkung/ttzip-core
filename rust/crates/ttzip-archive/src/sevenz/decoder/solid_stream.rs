// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! High-performance 7-Zip Solid Stream selective extraction engine with O(1) index jump tables
//! and 4MB micro-buffer sliding window early-exit decompression.

use std::collections::HashMap;
use std::io::Write;

use super::payload::decode_7z_folder_streaming;
use crate::crypto::crc32::crc32_fast;
use crate::sevenz::header::SevenZHeaderInfo;
use crate::types::TTZipStatus;
use ttzip_types::options::ProgressAtomicSheet;


/// 4MB sliding-window micro-buffer chunk size for 7z solid stream decompression.
pub const SOLID_MICRO_BUFFER_CHUNK_SIZE: usize = 4 * 1024 * 1024;

/// Maximum allowable uncompressed payload size allocated in contiguous memory (64MB ceiling).
pub const MAX_IN_MEMORY_DECOMPRESS_BYTES: usize = 64 * 1024 * 1024;

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
        let mut path_map = HashMap::with_capacity(num_files);

        let mut folders: Vec<SolidFolderTable> = Vec::with_capacity(info.folders.len().max(1));
        if !info.folders.is_empty() {
            for (f_idx, f) in info.folders.iter().enumerate() {
                let total_uncomp = super::payload::folder_final_unpack_size(f);
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

                let sz = super::payload::resolve_folder_stream_size(
                    info,
                    cur_folder_idx,
                    global_stream_idx,
                    streams_in_cur_folder,
                    current_folder_offset,
                );

                let crc = info.stream_crcs.get(global_stream_idx).copied();
                let offset_start = current_folder_offset;
                let offset_end = current_folder_offset.saturating_add(sz);

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

            // Populate path map with zero redundant heap allocations for canonical paths
            let raw_path = f.rel_path.as_str();
            let trimmed = raw_path.trim_start_matches('/');
            let needs_slash_norm = trimmed.contains('\\');
            let has_leading_slash = trimmed.len() != raw_path.len();

            path_map.insert(f.rel_path.clone(), file_idx);
            if needs_slash_norm || has_leading_slash {
                let normalized = if needs_slash_norm {
                    trimmed.replace('\\', "/")
                } else {
                    trimmed.to_string()
                };
                if normalized != *raw_path {
                    path_map.insert(normalized, file_idx);
                }
            }
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
        let trimmed = path.trim_start_matches('/');
        if trimmed != path {
            if let Some(&idx) = self.path_map.get(trimmed) {
                return self.entries.get(idx);
            }
        }
        if trimmed.contains('\\') {
            let normalized = trimmed.replace('\\', "/");
            if let Some(&idx) = self.path_map.get(normalized.as_str()) {
                return self.entries.get(idx);
            }
        }
        let normalized_trimmed = if trimmed.contains('\\') {
            trimmed.replace('\\', "/")
        } else {
            trimmed.to_string()
        };
        self.entries.iter().find(|e| {
            let e_norm = if e.rel_path.contains('\\') {
                e.rel_path.replace('\\', "/")
            } else {
                e.rel_path.clone()
            };
            let e_norm_trimmed = e_norm.trim_start_matches('/');
            e_norm_trimmed == normalized_trimmed
                || e.rel_path == path
                || e_norm_trimmed.ends_with(&normalized_trimmed)
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
    sheet: Option<&'a ProgressAtomicSheet>,
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
            sheet: None,
        }
    }

    /// Attaches an atomic progress sheet for non-blocking cancellation and progress tracking.
    pub fn with_atomic_sheet(mut self, sheet: &'a ProgressAtomicSheet) -> Self {
        self.sheet = Some(sheet);
        self
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
        if (loc.uncompressed_size as usize) > MAX_IN_MEMORY_DECOMPRESS_BYTES {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
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
        let target_len = loc.uncompressed_size;
        let target_end = target_start
            .checked_add(target_len)
            .ok_or(TTZipStatus::ErrCorruptHeader)?;

        if self.max_preceding_budget_bytes > 0 && target_start > self.max_preceding_budget_bytes {
            return Err(TTZipStatus::ErrSolidBudgetExceeded);
        }

        if let Some(sheet) = self.sheet {
            if sheet.is_cancelled() {
                return Err(TTZipStatus::Cancelled);
            }
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
                if let Some(sheet) = self.sheet {
                    if sheet.is_cancelled() {
                        return Err(TTZipStatus::Cancelled);
                    }
                }

                let chunk_start = current_offset;
                let chunk_len = chunk.len() as u64;
                let chunk_end = chunk_start
                    .checked_add(chunk_len)
                    .ok_or(TTZipStatus::ErrCorruptHeader)?;
                current_offset = chunk_end;
                decompressed_total = decompressed_total.saturating_add(chunk_len);

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
                        if let Some(sheet) = self.sheet {
                            sheet.add_processed(target_slice.len() as u64);
                        }
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

        let crc_matched = match loc.crc {
            Some(exp) => running_crc == exp,
            None => true,
        };

        if extracted_bytes != target_len || !crc_matched {
            return Err(if self.header_info.is_encrypted {
                TTZipStatus::ErrInvalidPassword
            } else {
                TTZipStatus::ErrCorruptHeader
            });
        }

        let early_exit_triggered =
            early_exit_signal_sent || (target_end < folder_total && decompressed_total < folder_total);

        if let Some(sheet) = self.sheet {
            sheet.mark_finished();
        }

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


#[cfg(test)]
mod tests {
    use super::*;
    use crate::sevenz::header::{SevenZCoder, SevenZFileMeta, SevenZFolder, SevenZHeaderInfo};

    #[test]
    fn test_solid_folder_index_empty_header() {
        let info = SevenZHeaderInfo::default();
        let index = SolidFolderIndex::build(&info);
        assert!(index.is_empty());
        assert_eq!(index.file_count(), 0);
        assert_eq!(index.folder_count(), 1);
        assert_eq!(index.lookup(0), None);
        assert_eq!(index.lookup_by_path("file.txt"), None);
    }

    #[test]
    fn test_solid_folder_index_multi_stream_offsets_and_lookup() {
        let info = SevenZHeaderInfo {
            folders: vec![SevenZFolder {
                coders: vec![SevenZCoder {
                    method_id: 0x21,
                    num_in_streams: 1,
                    num_out_streams: 1,
                    properties: vec![0x10],
                }],
                bind_pairs: Vec::new(),
                packed_streams: Vec::new(),
                unpack_sizes: vec![1000],
                crc: Some(0x12345678),
                num_unpack_streams: 3,
                packed_offset: 32,
                packed_len: 400,
            }],
            stream_sizes: vec![200, 300, 500],
            stream_crcs: vec![0xAAAA, 0xBBBB, 0xCCCC],
            files: vec![
                SevenZFileMeta {
                    rel_path: "folder/dir".to_string(),
                    is_directory: true,
                    is_empty_stream: false,
                    mtime_epoch_secs: None,
                    mode: 0,
                },
                SevenZFileMeta {
                    rel_path: "folder/a.txt".to_string(),
                    is_directory: false,
                    is_empty_stream: false,
                    mtime_epoch_secs: None,
                    mode: 0,
                },
                SevenZFileMeta {
                    rel_path: "folder/b.txt".to_string(),
                    is_directory: false,
                    is_empty_stream: false,
                    mtime_epoch_secs: None,
                    mode: 0,
                },
                SevenZFileMeta {
                    rel_path: "c.txt".to_string(),
                    is_directory: false,
                    is_empty_stream: false,
                    mtime_epoch_secs: None,
                    mode: 0,
                },
            ],
            ..Default::default()
        };

        let index = SolidFolderIndex::build(&info);
        assert_eq!(index.file_count(), 4);
        assert_eq!(index.folder_count(), 1);

        // Directory entry
        let dir_entry = index.lookup(0).unwrap();
        assert!(dir_entry.is_directory);
        assert_eq!(dir_entry.uncompressed_size, 0);

        // File 1: folder/a.txt (size 200, offset 0..200)
        let f1 = index.lookup(1).unwrap();
        assert_eq!(f1.offset_start, 0);
        assert_eq!(f1.offset_end, 200);
        assert_eq!(f1.uncompressed_size, 200);
        assert_eq!(f1.crc, Some(0xAAAA));

        // File 2: folder/b.txt (size 300, offset 200..500)
        let f2 = index.lookup(2).unwrap();
        assert_eq!(f2.offset_start, 200);
        assert_eq!(f2.offset_end, 500);
        assert_eq!(f2.uncompressed_size, 300);
        assert_eq!(f2.crc, Some(0xBBBB));

        // File 3: c.txt (size 500, offset 500..1000)
        let f3 = index.lookup(3).unwrap();
        assert_eq!(f3.offset_start, 500);
        assert_eq!(f3.offset_end, 1000);
        assert_eq!(f3.uncompressed_size, 500);
        assert_eq!(f3.crc, Some(0xCCCC));

        // Path lookup with backslash and normalized slashes
        assert_eq!(
            index.lookup_by_path("folder\\a.txt").map(|e| e.file_index),
            Some(1)
        );
        assert_eq!(
            index.lookup_by_path("/folder/b.txt").map(|e| e.file_index),
            Some(2)
        );
        assert_eq!(index.lookup_by_path("c.txt").map(|e| e.file_index), Some(3));

        // Folder stream range check
        assert_eq!(index.folder_stream_range(0, 0), Some((0, 200)));
        assert_eq!(index.folder_stream_range(0, 1), Some((200, 500)));
        assert_eq!(index.folder_stream_range(0, 2), Some((500, 1000)));
        assert_eq!(index.folder_stream_range(0, 3), None);
    }

    #[test]
    fn test_solid_folder_index_backslash_and_slash_normalization() {
        let info = SevenZHeaderInfo {
            folders: Vec::new(),
            stream_sizes: Vec::new(),
            stream_crcs: Vec::new(),
            files: vec![
                SevenZFileMeta {
                    rel_path: r"nested\deep\file.txt".to_string(),
                    is_directory: false,
                    is_empty_stream: true,
                    mtime_epoch_secs: None,
                    mode: 0,
                },
                SevenZFileMeta {
                    rel_path: "/root/level.bin".to_string(),
                    is_directory: false,
                    is_empty_stream: true,
                    mtime_epoch_secs: None,
                    mode: 0,
                },
            ],
            ..Default::default()
        };

        let index = SolidFolderIndex::build(&info);
        // Lookup by raw path
        assert_eq!(index.lookup_by_path(r"nested\deep\file.txt").map(|e| e.file_index), Some(0));
        // Lookup by normalized path
        assert_eq!(index.lookup_by_path("nested/deep/file.txt").map(|e| e.file_index), Some(0));

        // Lookup by raw leading slash
        assert_eq!(index.lookup_by_path("/root/level.bin").map(|e| e.file_index), Some(1));
        // Lookup by trimmed slash
        assert_eq!(index.lookup_by_path("root/level.bin").map(|e| e.file_index), Some(1));
    }

    #[test]
    fn test_solid_early_exit_cancellation() {
        let payload = vec![0x11u8; 1024];
        let folder = SevenZFolder {
            coders: vec![SevenZCoder {
                method_id: crate::sevenz::format::METHOD_COPY,
                properties: Vec::new(),
                num_in_streams: 1,
                num_out_streams: 1,
            }],
            bind_pairs: Vec::new(),
            packed_streams: Vec::new(),
            unpack_sizes: vec![1024],
            crc: None,
            num_unpack_streams: 1,
            packed_offset: 0,
            packed_len: 1024,
        };
        let info = SevenZHeaderInfo {
            folders: vec![folder],
            stream_sizes: vec![1024],
            stream_crcs: vec![0],
            files: vec![SevenZFileMeta {
                rel_path: "item.bin".to_string(),
                is_directory: false,
                is_empty_stream: false,
                mtime_epoch_secs: None,
                mode: 0,
            }],
            payload_offset: 0,
            payload_len: 1024,
            ..Default::default()
        };
        let index = SolidFolderIndex::build(&info);
        let sheet = ProgressAtomicSheet::with_total(1024);
        sheet.cancel();

        let extractor = SolidEarlyExitExtractor::new(&payload, &info, &index).with_atomic_sheet(&sheet);
        let res = extractor.extract_to_vec(0, None);
        assert_eq!(res.err(), Some(TTZipStatus::Cancelled));
    }
}


