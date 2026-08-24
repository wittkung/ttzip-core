// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

//! Zero-copy 7z Archive reader and extraction engine.

use super::payload::decode_7z_solid_streaming;
use super::stream::extract_entry_bytes_stream;
use crate::crypto::crc32::crc32_fast;
use crate::fs::safe_extract::{sanitize_and_validate_path, SafeExtractEngine};
use crate::sevenz::header::{parse_7z_metadata, SevenZFileMeta, SevenZHeaderInfo, SevenZSeekIndex};
use crate::types::{TTZipExtractOptions, TTZipStatus};
use crate::zip::reader::ZipExtractReport;
use std::ffi::CString;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Zero-copy 7z Archive reader and extractor.
pub struct SevenZArchive<'a> {
    data: &'a [u8],
    info: SevenZHeaderInfo,
    seek_index: SevenZSeekIndex,
}

impl<'a> SevenZArchive<'a> {
    /// Opens and parses a 7z archive from in-memory slice with optional password.
    pub fn open_slice_with_password(data: &'a [u8], password: Option<&str>) -> Result<Self, TTZipStatus> {
        let info = parse_7z_metadata(data, password)?;
        let seek_index = SevenZSeekIndex::build(&info);
        Ok(Self {
            data,
            info,
            seek_index,
        })
    }

    /// Opens and parses a 7z archive from in-memory slice.
    pub fn open_slice(data: &'a [u8]) -> Result<Self, TTZipStatus> {
        Self::open_slice_with_password(data, None)
    }

    /// Returns reference to parsed 7z metadata header.
    #[inline]
    pub fn info(&self) -> &SevenZHeaderInfo {
        &self.info
    }

    /// Returns reference to pre-built random-access seek index.
    #[inline]
    pub fn seek_index(&self) -> &SevenZSeekIndex {
        &self.seek_index
    }

    /// Returns list of files in the 7z archive.
    #[inline]
    pub fn files(&self) -> &[SevenZFileMeta] {
        &self.info.files
    }

    /// Returns the number of files in the archive.
    #[inline]
    pub fn len(&self) -> usize {
        self.info.files.len()
    }

    /// Returns true if the archive is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.info.files.is_empty()
    }

    /// Decompresses and extracts a single file from the 7z solid stream using Early Termination.
    #[inline]
    pub fn extract_entry_bytes_stream(
        &self,
        entry_idx: usize,
        password: Option<&str>,
    ) -> Result<Vec<u8>, TTZipStatus> {
        extract_entry_bytes_stream(self.data, &self.info, &self.seek_index, entry_idx, password)
    }

    /// Decompresses and extracts a single file from the 7z solid stream.
    #[inline]
    pub fn extract_entry_bytes(
        &self,
        entry_idx: usize,
        password: Option<&str>,
    ) -> Result<Vec<u8>, TTZipStatus> {
        self.extract_entry_bytes_stream(entry_idx, password)
    }

    /// Extracts all files in the 7z archive to the destination directory.
    pub fn extract_all(
        &self,
        dest_dir: &Path,
        options: &TTZipExtractOptions,
    ) -> Result<ZipExtractReport, TTZipStatus> {
        let start_time = std::time::Instant::now();
        fs::create_dir_all(dest_dir).map_err(|_| TTZipStatus::ErrOpenFailed)?;

        let password_str = if !options.password.is_null() {
            unsafe { std::ffi::CStr::from_ptr(options.password) }
                .to_str()
                .ok()
        } else {
            None
        };

        let mut engine = SafeExtractEngine::new();
        let mut total_uncomp_bytes = 0u64;

        for file in &self.info.files {
            let safe_path = sanitize_and_validate_path(dest_dir, &file.rel_path)?;
            engine.register_entry(
                safe_path.clone(),
                file.mode,
                file.mtime_epoch_secs.unwrap_or(0),
                0,
                file.is_directory,
            );

            if file.is_directory {
                fs::create_dir_all(&safe_path).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
            }
        }

        for &sz in &self.info.stream_sizes {
            total_uncomp_bytes += sz;
        }

        if options.dry_run {
            return Ok(ZipExtractReport {
                processed_entries_count: self.info.files.len(),
                total_uncompressed_bytes: total_uncomp_bytes,
                total_compressed_bytes: self.info.payload_len as u64,
                duration_ms: start_time.elapsed().as_millis() as u64,
            });
        }

        struct PendingFileItem {
            safe_path: std::path::PathBuf,
            rel_path: String,
            size: u64,
            expected_crc: u32,
        }

        let mut pending_files = Vec::new();
        let mut stream_idx = 0usize;

        for file in &self.info.files {
            if file.is_directory {
                continue;
            }
            let safe_path = sanitize_and_validate_path(dest_dir, &file.rel_path)?;
            if let Some(parent) = safe_path.parent() {
                fs::create_dir_all(parent).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
            }

            if file.is_empty_stream {
                File::create(&safe_path).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                continue;
            }

            let fsize = if stream_idx < self.info.stream_sizes.len() {
                self.info.stream_sizes[stream_idx]
            } else {
                0
            };
            let exp_crc = self.info.stream_crcs.get(stream_idx).copied().unwrap_or(0);
            stream_idx += 1;

            pending_files.push(PendingFileItem {
                safe_path,
                rel_path: file.rel_path.clone(),
                size: fsize,
                expected_crc: exp_crc,
            });
        }

        let mut current_file_idx = 0usize;
        let mut current_file_handle: Option<(File, u64, u32)> = None; // (File, bytes_written_for_this_file, running_crc)
        let processed_bytes = Arc::new(AtomicU64::new(0));
        let is_encrypted = self.info.is_encrypted;
        let progress_cb = options.progress_callback;
        let user_data = options.user_data;

        let _ = decode_7z_solid_streaming(
            self.data,
            &self.info,
            password_str,
            options.thread_budget.max(1),
            |mut chunk| -> Result<(), TTZipStatus> {
                while !chunk.is_empty() && current_file_idx < pending_files.len() {
                    let target_info = &pending_files[current_file_idx];
                    
                    if current_file_handle.is_none() {
                        let f = File::create(&target_info.safe_path)
                            .map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                        current_file_handle = Some((f, 0, 0));
                    }

                    let (ref mut file, ref mut written, ref mut running_crc) = current_file_handle.as_mut().unwrap();
                    let file_remaining = target_info.size.saturating_sub(*written);
                    let to_write = (chunk.len() as u64).min(file_remaining) as usize;

                    if to_write > 0 {
                        let slice = &chunk[..to_write];
                        file.write_all(slice).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                        *running_crc = crc32_fast(*running_crc, slice);
                        *written += to_write as u64;
                        let total_done = processed_bytes.fetch_add(to_write as u64, Ordering::Relaxed) + to_write as u64;

                        if let Some(cb) = progress_cb {
                            let c_path = CString::new(target_info.rel_path.as_str()).unwrap_or_default();
                            let keep_going = unsafe {
                                cb(total_done, total_uncomp_bytes, c_path.as_ptr(), user_data)
                            };
                            if !keep_going {
                                return Err(TTZipStatus::Cancelled);
                            }
                        }

                        chunk = &chunk[to_write..];
                    }

                    if *written >= target_info.size {
                        if target_info.expected_crc != 0 && *running_crc != target_info.expected_crc && is_encrypted {
                            return Err(TTZipStatus::ErrInvalidPassword);
                        }
                        current_file_handle = None;
                        current_file_idx += 1;
                    }
                }
                Ok(())
            },
        )?;

        if options.preserve_permissions {
            engine.apply_all()?;
        }

        Ok(ZipExtractReport {
            processed_entries_count: self.info.files.len(),
            total_uncompressed_bytes: total_uncomp_bytes,
            total_compressed_bytes: self.info.payload_len as u64,
            duration_ms: start_time.elapsed().as_millis() as u64,
        })
    }
}
