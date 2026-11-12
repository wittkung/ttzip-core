// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Domain types and sliding-window state machine for 7-Zip solid stream selective extraction.

use crate::crypto::crc32::crc32_fast;
use crate::sevenz::header::SevenZHeaderInfo;
use crate::types::TTZipStatus;
use ttzip_types::options::ProgressAtomicSheet;

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

pub(crate) fn build_initial_folder_tables(info: &SevenZHeaderInfo) -> Vec<SolidFolderTable> {
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
    folders
}

pub(crate) struct SolidStreamWindowState<'a> {
    pub(crate) target_start: u64,
    pub(crate) target_end: u64,
    pub(crate) current_offset: u64,
    pub(crate) decompressed_total: u64,
    pub(crate) skipped_preceding: u64,
    pub(crate) extracted_bytes: u64,
    pub(crate) running_crc: u32,
    pub(crate) early_exit_signal_sent: bool,
    pub(crate) sheet: Option<&'a ProgressAtomicSheet>,
}

impl<'a> SolidStreamWindowState<'a> {
    pub(crate) fn new(
        target_start: u64,
        target_end: u64,
        sheet: Option<&'a ProgressAtomicSheet>,
    ) -> Self {
        Self {
            target_start,
            target_end,
            current_offset: 0,
            decompressed_total: 0,
            skipped_preceding: 0,
            extracted_bytes: 0,
            running_crc: 0,
            early_exit_signal_sent: false,
            sheet,
        }
    }

    pub(crate) fn consume_chunk<F>(&mut self, chunk: &[u8], sink: &mut F) -> Result<(), TTZipStatus>
    where
        F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
    {
        if self.sheet.is_some_and(ProgressAtomicSheet::is_cancelled) {
            return Err(TTZipStatus::Cancelled);
        }

        let chunk_start = self.current_offset;
        let chunk_len = chunk.len() as u64;
        let chunk_end = chunk_start
            .checked_add(chunk_len)
            .ok_or(TTZipStatus::ErrCorruptHeader)?;
        self.current_offset = chunk_end;
        self.decompressed_total = self.decompressed_total.saturating_add(chunk_len);

        if chunk_end <= self.target_start {
            self.skipped_preceding += chunk_len;
            return Ok(());
        }

        if chunk_start < self.target_end && chunk_end > self.target_start {
            if chunk_start < self.target_start {
                self.skipped_preceding += self.target_start - chunk_start;
            }
            let slice_start = (self.target_start.saturating_sub(chunk_start)) as usize;
            let slice_end = (self.target_end.min(chunk_end) - chunk_start) as usize;
            let target_slice = &chunk[slice_start..slice_end];
            if !target_slice.is_empty() {
                sink(target_slice)?;
                self.running_crc = crc32_fast(self.running_crc, target_slice);
                self.extracted_bytes += target_slice.len() as u64;
                if let Some(sheet) = self.sheet {
                    sheet.add_processed(target_slice.len() as u64);
                }
            }
        }

        if self.current_offset >= self.target_end {
            self.early_exit_signal_sent = true;
            return Err(TTZipStatus::Eof);
        }

        Ok(())
    }

    pub(crate) fn finalize(
        self,
        folder_idx: usize,
        target_len: u64,
        folder_total: u64,
        expected_crc: Option<u32>,
        is_encrypted: bool,
    ) -> Result<SolidExtractionStats, TTZipStatus> {
        let crc_matched = expected_crc.is_none_or(|exp| self.running_crc == exp);
        if self.extracted_bytes != target_len || !crc_matched {
            return Err(if is_encrypted {
                TTZipStatus::ErrInvalidPassword
            } else {
                TTZipStatus::ErrCorruptHeader
            });
        }

        let early_exit_triggered = self.early_exit_signal_sent
            || (self.target_end < folder_total && self.decompressed_total < folder_total);

        if let Some(sheet) = self.sheet {
            sheet.mark_finished();
        }

        Ok(SolidExtractionStats {
            folder_index: folder_idx,
            target_offset_start: self.target_start,
            target_offset_end: self.target_end,
            decompressed_bytes_total: self.decompressed_total,
            skipped_preceding_bytes: self.skipped_preceding,
            extracted_target_bytes: self.extracted_bytes,
            early_exit_triggered,
            computed_crc: self.running_crc,
            expected_crc,
            crc_matched,
        })
    }
}
