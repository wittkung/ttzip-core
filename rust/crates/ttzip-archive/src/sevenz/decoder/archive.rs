// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Zero-copy 7z Archive reader and extraction engine.

use super::payload::decode_7z_solid_streaming;
use super::stream::extract_entry_bytes_stream;
use super::solid_stream::{SolidEarlyExitExtractor, SolidFolderIndex};
use crate::crypto::crc32::crc32_fast;
use crate::fs::abort_guard::InFlightJailFile;
use crate::fs::safe_extract::{sanitize_and_validate_path, DirJail, SafeExtractEngine};
use crate::sevenz::header::{parse_7z_metadata, SevenZFileMeta, SevenZHeaderInfo, SevenZSeekIndex};
use crate::types::{ProgressAtomicSheet, TTZipExtractOptions, TTZipStatus};
use crate::zip::reader::ZipExtractReport;
use std::ffi::CString;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::Ordering;



/// Zero-copy 7z Archive reader and extractor.
pub struct SevenZArchive<'a> {
    data: &'a [u8],
    info: SevenZHeaderInfo,
    seek_index: SevenZSeekIndex,
    folder_index: SolidFolderIndex,
}

/// Type alias for `SevenZArchive` providing unified reader naming convention.
pub type SevenZReader<'a> = SevenZArchive<'a>;

impl<'a> SevenZArchive<'a> {
    /// Opens and parses a 7z archive from in-memory slice with optional password.
    pub fn open_slice_with_password(data: &'a [u8], password: Option<&str>) -> Result<Self, TTZipStatus> {
        let info = parse_7z_metadata(data, password)?;
        let seek_index = SevenZSeekIndex::build(&info);
        let folder_index = SolidFolderIndex::build(&info);
        Ok(Self {
            data,
            info,
            seek_index,
            folder_index,
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

    /// Returns reference to pre-built solid folder index.
    #[inline]
    pub fn folder_index(&self) -> &SolidFolderIndex {
        &self.folder_index
    }

    /// Returns reference to pre-built solid folder index.
    #[inline]
    pub fn solid_index(&self) -> &SolidFolderIndex {
        &self.folder_index
    }

    /// Creates a 4MB micro-buffer early-exit solid extractor bound to this archive.
    #[inline]
    pub fn solid_extractor(&self) -> SolidEarlyExitExtractor<'_> {
        SolidEarlyExitExtractor::new(self.data, &self.info, &self.folder_index)
    }

    /// Creates a 4MB micro-buffer early-exit solid extractor bound to this archive and an atomic progress sheet.
    #[inline]
    pub fn solid_extractor_with_sheet<'s>(&'s self, sheet: &'s ProgressAtomicSheet) -> SolidEarlyExitExtractor<'s> {
        SolidEarlyExitExtractor::new(self.data, &self.info, &self.folder_index).with_atomic_sheet(sheet)
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

    /// Decompresses and extracts a single file from the 7z solid stream using Early Termination and optional sheet.
    #[inline]
    pub fn extract_entry_bytes_stream_with_sheet(
        &self,
        entry_idx: usize,
        password: Option<&str>,
        sheet: Option<&ProgressAtomicSheet>,
    ) -> Result<Vec<u8>, TTZipStatus> {
        if let Some(s) = sheet {
            let (vec, _) = self.solid_extractor_with_sheet(s).extract_to_vec(entry_idx, password)?;
            Ok(vec)
        } else {
            self.extract_entry_bytes_stream(entry_idx, password)
        }
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
    #[inline]
    pub fn extract_all(
        &self,
        dest_dir: &Path,
        options: &TTZipExtractOptions,
    ) -> Result<ZipExtractReport, TTZipStatus> {
        self.extract_all_with_sheet(dest_dir, options, None)
    }

    /// Extracts all files in the 7z archive to the destination directory with optional atomic progress sheet.
    pub fn extract_all_with_sheet(
        &self,
        dest_dir: &Path,
        options: &TTZipExtractOptions,
        sheet: Option<&ProgressAtomicSheet>,
    ) -> Result<ZipExtractReport, TTZipStatus> {
        let start_time = std::time::Instant::now();
        fs::create_dir_all(dest_dir).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let jail = DirJail::open(dest_dir)?;
        let password_str = extract_options_password(options);

        let total_uncomp_bytes = compute_7z_total_uncomp_bytes(&self.info);
        let local_sheet = ProgressAtomicSheet::with_total(total_uncomp_bytes);
        let active_sheet = prepare_active_sheet(sheet, &local_sheet, total_uncomp_bytes)?;

        let mut engine = SafeExtractEngine::for_root(dest_dir);
        let pending_files = prepare_pending_extraction_files(
            &self.info,
            dest_dir,
            &jail,
            &mut engine,
            options.dry_run,
            active_sheet,
        )?;

        if options.dry_run {
            active_sheet.mark_finished();
            return Ok(build_sevenz_extract_report(&self.info, total_uncomp_bytes, start_time));
        }

        let mut cur_idx = 0usize;
        flush_zero_size_pending_files(&pending_files, &mut cur_idx, &jail)?;
        let mut cur_handle: Option<ActiveSevenZSink> = None;

        let mut cursor = std::io::Cursor::new(self.data);
        let mut sink = |chunk: &[u8]| -> Result<(), TTZipStatus> {
            consume_extracted_stream_chunk(
                chunk,
                &pending_files,
                &mut cur_idx,
                &mut cur_handle,
                &jail,
                self.info.is_encrypted,
                options,
                active_sheet,
            )
        };

        if !self.info.folders.is_empty() {
            for (f_idx, folder) in self.info.folders.iter().enumerate() {
                match stream_7z_folder_from_reader(
                    &mut cursor,
                    &self.info,
                    folder,
                    password_str,
                    options.thread_budget.max(1),
                    &mut sink,
                ) {
                    Ok(_) => {}
                    Err(TTZipStatus::ErrUnsupportedFeature) => {
                        super::payload::decode_7z_folder_streaming(
                            self.data,
                            &self.info,
                            f_idx,
                            password_str,
                            options.thread_budget.max(1),
                            &mut sink,
                        )?;
                    }
                    Err(e) => return Err(e),
                }
            }
        } else {
            decode_7z_solid_streaming(
                self.data,
                &self.info,
                password_str,
                options.thread_budget.max(1),
                &mut sink,
            )?;
        }

        finalize_pending_extraction_state(
            &pending_files,
            &mut cur_idx,
            &cur_handle,
            &jail,
            self.info.is_encrypted,
        )?;
        active_sheet.mark_finished();
        engine.apply_deferred_metadata(options.preserve_permissions)?;
        Ok(build_sevenz_extract_report(&self.info, total_uncomp_bytes, start_time))
    }
}

pub use super::stream::extract_7z_single_entry_from_reader;
use super::stream::stream_7z_folder_from_reader;

struct PendingSevenZItem {
    rel_path: String,
    size: u64,
    expected_crc: Option<u32>,
    is_symlink: bool,
}

enum ActiveSevenZSink {
    Regular {
        inflight: InFlightJailFile,
        written: u64,
        running_crc: u32,
    },
    Symlink {
        target_bytes: Vec<u8>,
        written: u64,
        running_crc: u32,
    },
}

fn extract_options_password(options: &TTZipExtractOptions) -> Option<&str> {
    if !options.password.is_null() {
        unsafe { std::ffi::CStr::from_ptr(options.password) }.to_str().ok()
    } else {
        None
    }
}

fn prepare_active_sheet<'s>(
    sheet: Option<&'s ProgressAtomicSheet>,
    local_sheet: &'s ProgressAtomicSheet,
    total_uncomp_bytes: u64,
) -> Result<&'s ProgressAtomicSheet, TTZipStatus> {
    let active_sheet = sheet.unwrap_or(local_sheet);
    if active_sheet.total_bytes.load(Ordering::Relaxed) == 0 && total_uncomp_bytes > 0 {
        active_sheet.set_total(total_uncomp_bytes);
    }
    if active_sheet.is_cancelled() {
        return Err(TTZipStatus::Cancelled);
    }
    Ok(active_sheet)
}

fn build_sevenz_extract_report(
    info: &SevenZHeaderInfo,
    total_uncompressed_bytes: u64,
    start_time: std::time::Instant,
) -> ZipExtractReport {
    ZipExtractReport {
        processed_entries_count: info.files.len(),
        total_uncompressed_bytes,
        total_compressed_bytes: info.payload_len as u64,
        duration_ms: start_time.elapsed().as_millis() as u64,
    }
}

fn compute_7z_total_uncomp_bytes(info: &SevenZHeaderInfo) -> u64 {
    let has_non_trailing_final = info.folders.iter().any(|f| {
        super::payload::folder_final_unpack_size(f) != f.unpack_sizes.last().copied().unwrap_or(0)
    });
    if has_non_trailing_final {
        let folder_sum: u64 = info
            .folders
            .iter()
            .map(super::payload::folder_final_unpack_size)
            .sum();
        if folder_sum > 0 {
            return folder_sum;
        }
    }
    let mut total: u64 = info.stream_sizes.iter().sum();
    if total == 0 {
        total = info
            .folders
            .iter()
            .map(super::payload::folder_final_unpack_size)
            .sum();
    }
    if total == 0 {
        total = info.payload_len as u64;
    }
    total
}

fn prepare_pending_extraction_files(
    info: &SevenZHeaderInfo,
    dest_dir: &Path,
    jail: &DirJail,
    engine: &mut SafeExtractEngine,
    dry_run: bool,
    active_sheet: &ProgressAtomicSheet,
) -> Result<Vec<PendingSevenZItem>, TTZipStatus> {
    let mut pending_files = Vec::new();
    let mut stream_idx = 0usize;
    let mut cur_folder_idx = 0usize;
    let mut streams_in_cur_folder = 0usize;
    let mut current_folder_offset = 0u64;
    for file in &info.files {
        if active_sheet.is_cancelled() {
            return Err(TTZipStatus::Cancelled);
        }
        let safe_path = sanitize_and_validate_path(dest_dir, &file.rel_path)?;
        let is_symlink = (file.mode & 0o170000) == 0o120000;
        engine.register_entry_typed(
            safe_path,
            file.mode & 0o7777,
            file.mtime_epoch_secs.unwrap_or(0),
            0,
            file.is_directory,
            is_symlink,
        );
        if file.is_directory {
            if !dry_run {
                jail.create_dir_all_in_jail(&file.rel_path)?;
            }
            continue;
        }
        if file.is_empty_stream {
            pending_files.push(PendingSevenZItem {
                rel_path: file.rel_path.clone(),
                size: 0,
                expected_crc: None,
                is_symlink,
            });
            continue;
        }
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
        let fsize = super::payload::resolve_folder_stream_size(
            info,
            cur_folder_idx,
            stream_idx,
            streams_in_cur_folder,
            current_folder_offset,
        );
        let exp_crc = info.stream_crcs.get(stream_idx).copied();
        current_folder_offset = current_folder_offset.saturating_add(fsize);
        streams_in_cur_folder += 1;
        stream_idx += 1;
        pending_files.push(PendingSevenZItem {
            rel_path: file.rel_path.clone(),
            size: fsize,
            expected_crc: exp_crc,
            is_symlink,
        });
    }
    Ok(pending_files)
}

fn flush_zero_size_pending_files(
    pending_files: &[PendingSevenZItem],
    cur_idx: &mut usize,
    jail: &DirJail,
) -> Result<(), TTZipStatus> {
    while *cur_idx < pending_files.len() && pending_files[*cur_idx].size == 0 {
        let item = &pending_files[*cur_idx];
        if item.is_symlink {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        if let Some(exp_crc) = item.expected_crc {
            if exp_crc != 0 {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
        }
        InFlightJailFile::open(jail, &item.rel_path, true)?.commit(jail)?;
        *cur_idx += 1;
    }
    Ok(())
}

fn open_sevenz_sink_for_item(
    item: &PendingSevenZItem,
    jail: &DirJail,
) -> Result<ActiveSevenZSink, TTZipStatus> {
    if item.is_symlink {
        if item.size > 4096 {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        Ok(ActiveSevenZSink::Symlink {
            target_bytes: Vec::with_capacity(item.size as usize),
            written: 0,
            running_crc: 0,
        })
    } else {
        let inflight = InFlightJailFile::open(jail, &item.rel_path, true)?;
        Ok(ActiveSevenZSink::Regular {
            inflight,
            written: 0,
            running_crc: 0,
        })
    }
}

fn write_sevenz_sink_slice(
    sink: &mut ActiveSevenZSink,
    item: &PendingSevenZItem,
    chunk: &mut &[u8],
    active_sheet: &ProgressAtomicSheet,
    options: &TTZipExtractOptions,
) -> Result<(u64, u32), TTZipStatus> {
    match sink {
        ActiveSevenZSink::Regular {
            inflight,
            written,
            running_crc,
        } => {
            let to_write = (chunk.len() as u64).min(item.size.saturating_sub(*written)) as usize;
            if to_write > 0 {
                let slice = &chunk[..to_write];
                inflight
                    .file_mut()
                    .write_all(slice)
                    .map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                *running_crc = crc32_fast(*running_crc, slice);
                *written += to_write as u64;
                report_extract_progress(active_sheet, options, &item.rel_path, to_write as u64)?;
                *chunk = &chunk[to_write..];
            }
            Ok((*written, *running_crc))
        }
        ActiveSevenZSink::Symlink {
            target_bytes,
            written,
            running_crc,
        } => {
            let to_write = (chunk.len() as u64).min(item.size.saturating_sub(*written)) as usize;
            if to_write > 0 {
                let slice = &chunk[..to_write];
                target_bytes.extend_from_slice(slice);
                *running_crc = crc32_fast(*running_crc, slice);
                *written += to_write as u64;
                report_extract_progress(active_sheet, options, &item.rel_path, to_write as u64)?;
                *chunk = &chunk[to_write..];
            }
            Ok((*written, *running_crc))
        }
    }
}

fn finalize_completed_sevenz_item(
    item: &PendingSevenZItem,
    crc_now: u32,
    cur_handle: &mut Option<ActiveSevenZSink>,
    jail: &DirJail,
    is_encrypted: bool,
) -> Result<(), TTZipStatus> {
    if let Some(expected_crc) = item.expected_crc {
        if crc_now != expected_crc {
            let _ = cur_handle.take();
            return Err(if is_encrypted {
                TTZipStatus::ErrInvalidPassword
            } else {
                TTZipStatus::ErrCorruptHeader
            });
        }
    }
    match cur_handle.take() {
        Some(ActiveSevenZSink::Regular { inflight, .. }) => {
            inflight.commit(jail)?;
        }
        Some(ActiveSevenZSink::Symlink { target_bytes, .. }) => {
            let target_str =
                std::str::from_utf8(&target_bytes).map_err(|_| TTZipStatus::ErrSecurityViolation)?;
            jail.create_symlink_in_jail(&item.rel_path, target_str)?;
        }
        None => {}
    }
    Ok(())
}

fn ensure_no_excess_stream_bytes(
    chunk: &[u8],
    remaining_files: &[PendingSevenZItem],
    cur_handle: &mut Option<ActiveSevenZSink>,
    is_encrypted: bool,
) -> Result<(), TTZipStatus> {
    if !chunk.is_empty() && remaining_files.iter().all(|f| f.size == 0) {
        let _ = cur_handle.take();
        return Err(if is_encrypted {
            TTZipStatus::ErrInvalidPassword
        } else {
            TTZipStatus::ErrCorruptHeader
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments, reason = "Internal stateful sink helper avoids closure complexity")]
fn consume_extracted_stream_chunk(
    mut chunk: &[u8],
    pending_files: &[PendingSevenZItem],
    cur_idx: &mut usize,
    cur_handle: &mut Option<ActiveSevenZSink>,
    jail: &DirJail,
    is_encrypted: bool,
    options: &TTZipExtractOptions,
    active_sheet: &ProgressAtomicSheet,
) -> Result<(), TTZipStatus> {
    if active_sheet.is_cancelled() {
        return Err(TTZipStatus::Cancelled);
    }
    flush_zero_size_pending_files(pending_files, cur_idx, jail)?;
    while !chunk.is_empty() && *cur_idx < pending_files.len() {
        if active_sheet.is_cancelled() {
            return Err(TTZipStatus::Cancelled);
        }
        let item = &pending_files[*cur_idx];
        if cur_handle.is_none() {
            *cur_handle = Some(open_sevenz_sink_for_item(item, jail)?);
        }
        let sink = cur_handle.as_mut().unwrap();
        let (written_now, crc_now) =
            write_sevenz_sink_slice(sink, item, &mut chunk, active_sheet, options)?;

        if written_now >= item.size {
            ensure_no_excess_stream_bytes(chunk, &pending_files[*cur_idx + 1..], cur_handle, is_encrypted)?;
            finalize_completed_sevenz_item(item, crc_now, cur_handle, jail, is_encrypted)?;
            *cur_idx += 1;
            flush_zero_size_pending_files(pending_files, cur_idx, jail)?;
        }
    }
    ensure_no_excess_stream_bytes(chunk, &[], cur_handle, is_encrypted)
}

fn report_extract_progress(
    active_sheet: &ProgressAtomicSheet,
    options: &TTZipExtractOptions,
    rel_path: &str,
    delta: u64,
) -> Result<(), TTZipStatus> {
    let done = active_sheet.add_processed(delta);
    if let Some(cb) = options.progress_callback {
        let c_path = CString::new(rel_path).unwrap_or_default();
        let target = active_sheet.total_bytes.load(Ordering::Relaxed).max(done);
        if !unsafe { cb(done, target, c_path.as_ptr(), options.user_data) } {
            active_sheet.cancel();
            return Err(TTZipStatus::Cancelled);
        }
    }
    Ok(())
}

/// Extracts all entries from a seekable 7z reader (such as `VirtualMultiVolumeReader`) with bounded memory.
pub fn extract_7z_from_reader<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    info: &SevenZHeaderInfo,
    dest_dir: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<u64, TTZipStatus> {
    fs::create_dir_all(dest_dir).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let jail = DirJail::open(dest_dir)?;
    let password_str = extract_options_password(options);
    let total_uncomp_bytes = compute_7z_total_uncomp_bytes(info);
    let local_sheet = ProgressAtomicSheet::with_total(total_uncomp_bytes);
    let active_sheet = prepare_active_sheet(sheet, &local_sheet, total_uncomp_bytes)?;

    let mut engine = SafeExtractEngine::for_root(dest_dir);
    let pending_files = prepare_pending_extraction_files(
        info,
        dest_dir,
        &jail,
        &mut engine,
        options.dry_run,
        active_sheet,
    )?;
    if options.dry_run {
        return Ok(total_uncomp_bytes);
    }

    let mut cur_idx = 0usize;
    flush_zero_size_pending_files(&pending_files, &mut cur_idx, &jail)?;
    let mut cur_handle: Option<ActiveSevenZSink> = None;
    let mut sink = |chunk: &[u8]| -> Result<(), TTZipStatus> {
        consume_extracted_stream_chunk(
            chunk,
            &pending_files,
            &mut cur_idx,
            &mut cur_handle,
            &jail,
            info.is_encrypted,
            options,
            active_sheet,
        )
    };

    for folder in &info.folders {
        stream_7z_folder_from_reader(reader, info, folder, password_str, options.thread_budget.max(1), &mut sink)?;
    }
    finalize_pending_extraction_state(
        &pending_files,
        &mut cur_idx,
        &cur_handle,
        &jail,
        info.is_encrypted,
    )?;
    if options.preserve_permissions {
        engine.apply_all()?;
    }
    Ok(total_uncomp_bytes)
}

fn finalize_pending_extraction_state(
    pending_files: &[PendingSevenZItem],
    cur_idx: &mut usize,
    cur_handle: &Option<ActiveSevenZSink>,
    jail: &DirJail,
    is_encrypted: bool,
) -> Result<(), TTZipStatus> {
    flush_zero_size_pending_files(pending_files, cur_idx, jail)?;
    if cur_handle.is_some() || *cur_idx < pending_files.len() {
        return Err(if is_encrypted {
            TTZipStatus::ErrInvalidPassword
        } else {
            TTZipStatus::ErrCorruptHeader
        });
    }
    jail.verify_all_jail_symlinks()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_7z_extract_all_cancellation_with_sheet() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join("out");
        let sheet = ProgressAtomicSheet::with_total(100);
        sheet.cancel();

        let dummy_info = SevenZHeaderInfo {
            files: vec![SevenZFileMeta {
                rel_path: "test.txt".to_string(),
                is_directory: false,
                is_empty_stream: false,
                mtime_epoch_secs: None,
                mode: 0,
            }],
            stream_sizes: vec![100],
            ..Default::default()
        };

        let archive = SevenZArchive {
            data: &[],
            info: dummy_info,
            seek_index: SevenZSeekIndex::default(),
            folder_index: SolidFolderIndex::default(),
        };

        let opts = TTZipExtractOptions::default();
        let res = archive.extract_all_with_sheet(&dest, &opts, Some(&sheet));
        assert_eq!(res.err(), Some(TTZipStatus::Cancelled));
    }
}

