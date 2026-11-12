// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! High-performance 7-Zip Solid Stream selective extraction engine with O(1) index jump tables
//! and 4MB micro-buffer sliding window early-exit decompression.

use std::collections::HashMap;
use std::io::Write;

use super::payload::decode_7z_folder_streaming;
pub use super::solid_stream_state::{SolidExtractionStats, SolidFileRange, SolidFolderTable};
use super::solid_stream_state::{build_initial_folder_tables, SolidStreamWindowState};
use crate::sevenz::header::SevenZHeaderInfo;
use crate::types::TTZipStatus;
use ttzip_types::options::ProgressAtomicSheet;

/// 4MB sliding-window micro-buffer chunk size for 7z solid stream decompression.
pub const SOLID_MICRO_BUFFER_CHUNK_SIZE: usize = 4 * 1024 * 1024;

/// Maximum allowable uncompressed payload size allocated in contiguous memory (64MB ceiling).
pub const MAX_IN_MEMORY_DECOMPRESS_BYTES: usize = 64 * 1024 * 1024;

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

fn advance_folder_stream_cursor(
    info: &SevenZHeaderInfo,
    cur_folder_idx: &mut usize,
    streams_in_cur_folder: &mut usize,
    current_folder_offset: &mut u64,
) {
    while *cur_folder_idx < info.folders.len() {
        let folder_streams = info.folders[*cur_folder_idx].num_unpack_streams.max(1);
        if *streams_in_cur_folder < folder_streams {
            break;
        }
        if *cur_folder_idx + 1 < info.folders.len() {
            *cur_folder_idx += 1;
            *streams_in_cur_folder = 0;
            *current_folder_offset = 0;
        } else {
            break;
        }
    }
}

fn insert_normalized_path_entry(
    path_map: &mut HashMap<String, usize>,
    rel_path: &str,
    file_idx: usize,
) {
    let trimmed = rel_path.trim_start_matches('/');
    let needs_slash_norm = trimmed.contains('\\');
    let has_leading_slash = trimmed.len() != rel_path.len();

    path_map.insert(rel_path.to_string(), file_idx);
    if needs_slash_norm || has_leading_slash {
        let normalized = if needs_slash_norm {
            trimmed.replace('\\', "/")
        } else {
            trimmed.to_string()
        };
        if normalized != rel_path {
            path_map.insert(normalized, file_idx);
        }
    }
}

fn build_solid_file_range(
    info: &SevenZHeaderInfo,
    folders: &mut [SolidFolderTable],
    file_idx: usize,
    f: &crate::sevenz::header::SevenZFileMeta,
    global_stream_idx: &mut usize,
    cur_folder_idx: &mut usize,
    streams_in_cur_folder: &mut usize,
    current_folder_offset: &mut u64,
) -> SolidFileRange {
    if f.is_directory || f.is_empty_stream {
        return SolidFileRange {
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
    }

    advance_folder_stream_cursor(
        info,
        cur_folder_idx,
        streams_in_cur_folder,
        current_folder_offset,
    );
    let folder_idx = if *cur_folder_idx < info.folders.len() {
        *cur_folder_idx
    } else {
        0
    };
    let sz = super::payload::resolve_folder_stream_size(
        info,
        *cur_folder_idx,
        *global_stream_idx,
        *streams_in_cur_folder,
        *current_folder_offset,
    );
    let crc = info.stream_crcs.get(*global_stream_idx).copied();
    let offset_start = *current_folder_offset;
    let offset_end = current_folder_offset.saturating_add(sz);

    if let Some(ft) = folders.get_mut(folder_idx) {
        ft.global_stream_indices.push(*global_stream_idx);
        ft.substream_to_file_index.push(file_idx);
        ft.stream_prefix_sums.push(offset_end);
    }

    let substream_idx = *streams_in_cur_folder;
    let stream_idx = *global_stream_idx;
    *current_folder_offset = offset_end;
    *streams_in_cur_folder += 1;
    *global_stream_idx += 1;

    SolidFileRange {
        file_index: file_idx,
        rel_path: f.rel_path.clone(),
        is_directory: false,
        is_empty_stream: false,
        folder_index: Some(folder_idx),
        substream_index: Some(substream_idx),
        global_stream_index: Some(stream_idx),
        offset_start,
        offset_end,
        uncompressed_size: sz,
        crc,
    }
}

impl SolidFolderIndex {
    /// Builds an O(1) random-access seek and prefix sum jump table from parsed 7z metadata header.
    pub fn build(info: &SevenZHeaderInfo) -> Self {
        let num_files = info.files.len();
        let mut entries = Vec::with_capacity(num_files);
        let mut path_map = HashMap::with_capacity(num_files);
        let mut folders = build_initial_folder_tables(info);

        let mut global_stream_idx = 0usize;
        let mut cur_folder_idx = 0usize;
        let mut streams_in_cur_folder = 0usize;
        let mut current_folder_offset = 0u64;

        for (file_idx, f) in info.files.iter().enumerate() {
            entries.push(build_solid_file_range(
                info,
                &mut folders,
                file_idx,
                f,
                &mut global_stream_idx,
                &mut cur_folder_idx,
                &mut streams_in_cur_folder,
                &mut current_folder_offset,
            ));
            insert_normalized_path_entry(&mut path_map, &f.rel_path, file_idx);
        }

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

        if loc.is_directory || loc.is_empty_stream || loc.uncompressed_size == 0 {
            return Ok(SolidExtractionStats {
                folder_index: loc.folder_index.unwrap_or(0),
                expected_crc: loc.crc,
                crc_matched: true,
                ..Default::default()
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
        if self.sheet.is_some_and(ProgressAtomicSheet::is_cancelled) {
            return Err(TTZipStatus::Cancelled);
        }

        let folder_total = self.folder_index.folder_total_size(folder_idx).unwrap_or(target_end);
        let mut window = SolidStreamWindowState::new(target_start, target_end, self.sheet);

        let decode_res = decode_7z_folder_streaming(
            self.archive_data,
            self.header_info,
            folder_idx,
            password,
            self.thread_budget,
            |chunk| window.consume_chunk(chunk, &mut sink),
        );

        match decode_res {
            Ok(_) | Err(TTZipStatus::Eof) => {}
            Err(e) => return Err(e),
        }

        window.finalize(
            folder_idx,
            target_len,
            folder_total,
            loc.crc,
            self.header_info.is_encrypted,
        )
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


