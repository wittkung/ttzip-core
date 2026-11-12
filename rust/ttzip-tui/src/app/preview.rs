// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! File preview extraction and syntax highlighting integration.

use super::state::AppState;
use crate::cli::format::{
    detect_archive_format, parse_archive_entries, read_archive_data_auto, ContainerFormat,
};
use crate::preview::{
    generate_preview, PreviewData, MAX_PREVIEW_BYTES, MAX_RESIDENT_MEMORY_LIMIT,
};
use std::io::{self, Read, Write};
use std::path::Path;
use ttzip_engine::archive::tar::TarArchive;
use ttzip_engine::archive::unified::extract_single_entry_memory;
use ttzip_engine::codecs::deflate::LibdeflateReader;
use ttzip_engine::codecs::zstd::ZstdStreamReader;
use ttzip_engine::sevenz::{extract_entry_bytes_stream_bounded, SevenZArchive};
use ttzip_engine::types::TTZipStatus;
use ttzip_engine::zip::parser::parse_local_file_header;
use ttzip_engine::zip::ZipArchive;

struct BoundedPreviewSink {
    buf: Vec<u8>,
    max_bytes: usize,
    reached_limit: bool,
}

impl BoundedPreviewSink {
    fn new(max_bytes: usize) -> Self {
        Self {
            buf: Vec::with_capacity(max_bytes),
            max_bytes,
            reached_limit: false,
        }
    }
}

impl Write for BoundedPreviewSink {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let remaining = self.max_bytes.saturating_sub(self.buf.len());
        let take = remaining.min(data.len());
        self.buf.extend_from_slice(&data[..take]);
        if self.buf.len() >= self.max_bytes {
            self.reached_limit = true;
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "preview byte cap reached",
            ));
        }
        Ok(take)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn decode_unencrypted_zip_slice_bounded(
    raw_data: &[u8],
    lfh_offset: u64,
    compressed_size: u64,
    actual_method: u16,
    max_bytes: usize,
) -> Option<Result<Vec<u8>, TTZipStatus>> {
    let offset = usize::try_from(lfh_offset).ok()?;
    let (payload_offset, _) = match parse_local_file_header(raw_data, offset) {
        Ok(res) => res,
        Err(e) => return Some(Err(e)),
    };
    let comp_size = usize::try_from(compressed_size).ok()?;
    let payload_end = payload_offset.checked_add(comp_size)?;
    if payload_end > raw_data.len() {
        return Some(Err(TTZipStatus::ErrCorruptHeader));
    }
    let raw_payload = &raw_data[payload_offset..payload_end];

    match actual_method {
        0 => {
            let take = raw_payload.len().min(max_bytes);
            Some(Ok(raw_payload[..take].to_vec()))
        }
        8 => {
            let reader = match LibdeflateReader::new_raw(raw_payload) {
                Ok(r) => r,
                Err(e) => return Some(Err(e)),
            };
            let mut out = Vec::with_capacity(max_bytes);
            let res = reader
                .take(max_bytes as u64)
                .read_to_end(&mut out)
                .map(|_| out)
                .map_err(|_| TTZipStatus::ErrCorruptHeader);
            Some(res)
        }
        93 => {
            let reader = match ZstdStreamReader::new(raw_payload) {
                Ok(r) => r,
                Err(e) => return Some(Err(e)),
            };
            let mut out = Vec::with_capacity(max_bytes);
            let res = reader
                .take(max_bytes as u64)
                .read_to_end(&mut out)
                .map(|_| out)
                .map_err(|_| TTZipStatus::ErrCorruptHeader);
            Some(res)
        }
        _ => None,
    }
}

/// Extracts at most `max_bytes` from a ZIP entry without decompressing the entire entry when bounded.
pub(crate) fn extract_zip_entry_bounded(
    raw_data: &[u8],
    archive: &ZipArchive<'_>,
    idx: usize,
    password: Option<&str>,
    max_bytes: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    let entry = archive
        .entries()
        .get(idx)
        .ok_or(TTZipStatus::ErrInvalidOffset)?;
    if entry.is_directory || entry.uncompressed_size <= max_bytes as u64 {
        return archive.extract_entry_bytes(idx, password);
    }

    if !entry.is_encrypted {
        if let Some(res) = decode_unencrypted_zip_slice_bounded(
            raw_data,
            entry.lfh_offset,
            entry.compressed_size,
            entry.actual_method,
            max_bytes,
        ) {
            return res;
        }
    }

    let mut sink = BoundedPreviewSink::new(max_bytes);
    match archive.extract_entry_to_sink(idx, password, &mut sink) {
        Ok(_) => Ok(sink.buf),
        Err(_) if sink.reached_limit => Ok(sink.buf),
        Err(e) => Err(e),
    }
}

/// Extracts at most `max_bytes` from a 7z entry using bounded solid stream decoding and early exit.
pub(crate) fn extract_sevenz_entry_bounded(
    raw_data: &[u8],
    archive: &SevenZArchive<'_>,
    idx: usize,
    password: Option<&str>,
    max_bytes: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    let uncomp_size = archive
        .solid_index()
        .lookup(idx)
        .map(|r| r.uncompressed_size)
        .unwrap_or(0);
    if uncomp_size <= max_bytes as u64 {
        return extract_entry_bytes_stream_bounded(
            raw_data,
            archive.info(),
            archive.seek_index(),
            idx,
            password,
            MAX_RESIDENT_MEMORY_LIMIT as u64,
        );
    }

    let mut out = Vec::with_capacity(max_bytes);
    let mut reached_limit = false;
    let res = archive
        .solid_extractor()
        .with_preceding_budget(MAX_RESIDENT_MEMORY_LIMIT as u64)
        .extract_streaming(idx, password, |chunk| {
            let remaining = max_bytes.saturating_sub(out.len());
            let take = remaining.min(chunk.len());
            out.extend_from_slice(&chunk[..take]);
            if out.len() >= max_bytes {
                reached_limit = true;
                return Err(TTZipStatus::Cancelled);
            }
            Ok(())
        });

    match res {
        Ok(_) => Ok(out),
        Err(_) if reached_limit => Ok(out),
        Err(e) => Err(e),
    }
}

/// Outcome of single-pass bounded entry extraction for CLI/MCP `cat`.
pub(crate) enum BoundedCatOutcome {
    Extracted { bytes: Vec<u8>, total_size: usize },
    Directory,
    TooLarge { uncompressed_size: u64, max_allowed: u64 },
    NotFound,
    ReadFailed(String),
    ParseFailed(String),
    StatusErr(TTZipStatus),
    UnsupportedFormat,
}

fn entry_matches_query(rel_path: &str, query: &str) -> bool {
    if rel_path == query {
        return true;
    }
    Path::new(rel_path)
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|name| name == query)
}

fn cat_zip_slice_bounded(
    raw_data: &[u8],
    entry_path: &str,
    password: Option<&str>,
    max_bytes: usize,
    max_in_memory_bytes: u64,
) -> BoundedCatOutcome {
    let archive = match ZipArchive::open_slice(raw_data) {
        Ok(a) => a,
        Err(st) => return BoundedCatOutcome::StatusErr(st),
    };
    let Some((idx, entry)) = archive
        .entries()
        .iter()
        .enumerate()
        .find(|(_, e)| entry_matches_query(&e.rel_path, entry_path))
    else {
        return BoundedCatOutcome::NotFound;
    };
    if entry.is_directory {
        return BoundedCatOutcome::Directory;
    }
    if entry.uncompressed_size > max_in_memory_bytes {
        return BoundedCatOutcome::TooLarge {
            uncompressed_size: entry.uncompressed_size,
            max_allowed: max_in_memory_bytes,
        };
    }
    let total_size = entry.uncompressed_size as usize;
    match extract_zip_entry_bounded(raw_data, &archive, idx, password, max_bytes) {
        Ok(bytes) => BoundedCatOutcome::Extracted { bytes, total_size },
        Err(st) => BoundedCatOutcome::StatusErr(st),
    }
}

fn cat_sevenz_slice_bounded(
    raw_data: &[u8],
    entry_path: &str,
    password: Option<&str>,
    max_bytes: usize,
    max_in_memory_bytes: u64,
) -> BoundedCatOutcome {
    let archive = match SevenZArchive::open_slice_with_password(raw_data, password) {
        Ok(a) => a,
        Err(st) => return BoundedCatOutcome::StatusErr(st),
    };
    let Some((idx, file)) = archive
        .files()
        .iter()
        .enumerate()
        .find(|(_, f)| entry_matches_query(&f.rel_path, entry_path))
    else {
        return BoundedCatOutcome::NotFound;
    };
    if file.is_directory {
        return BoundedCatOutcome::Directory;
    }
    let uncomp_size = archive
        .solid_index()
        .lookup(idx)
        .map(|r| r.uncompressed_size)
        .unwrap_or(0);
    if uncomp_size > max_in_memory_bytes {
        return BoundedCatOutcome::TooLarge {
            uncompressed_size: uncomp_size,
            max_allowed: max_in_memory_bytes,
        };
    }
    match extract_sevenz_entry_bounded(raw_data, &archive, idx, password, max_bytes) {
        Ok(bytes) => BoundedCatOutcome::Extracted {
            bytes,
            total_size: uncomp_size as usize,
        },
        Err(st) => BoundedCatOutcome::StatusErr(st),
    }
}

fn cat_tar_slice_bounded(
    raw_data: &[u8],
    entry_path: &str,
    max_bytes: usize,
    max_in_memory_bytes: u64,
) -> BoundedCatOutcome {
    let archive = match TarArchive::open_slice(raw_data) {
        Ok(a) => a,
        Err(st) => return BoundedCatOutcome::StatusErr(st),
    };
    let Some((idx, entry)) = archive
        .entries()
        .iter()
        .enumerate()
        .find(|(_, e)| entry_matches_query(e.path.as_ref(), entry_path))
    else {
        return BoundedCatOutcome::NotFound;
    };
    if entry.is_directory {
        return BoundedCatOutcome::Directory;
    }
    if entry.size > max_in_memory_bytes {
        return BoundedCatOutcome::TooLarge {
            uncompressed_size: entry.size,
            max_allowed: max_in_memory_bytes,
        };
    }
    match archive.extract_entry_bytes(idx) {
        Ok(slice) => {
            let take = slice.len().min(max_bytes);
            BoundedCatOutcome::Extracted {
                bytes: slice[..take].to_vec(),
                total_size: slice.len(),
            }
        }
        Err(st) => BoundedCatOutcome::StatusErr(st),
    }
}

fn cat_fallback_entry_bounded(
    archive_path: &Path,
    data: &[u8],
    entry_path: &str,
    password: Option<&str>,
    max_bytes: usize,
    max_in_memory_bytes: u64,
) -> BoundedCatOutcome {
    let (_fmt, entries) = match parse_archive_entries(archive_path, data) {
        Ok(res) => res,
        Err(e) => return BoundedCatOutcome::ParseFailed(e),
    };
    let Some(entry) = entries
        .iter()
        .find(|e| e.relative_path == entry_path || e.name == entry_path)
    else {
        return BoundedCatOutcome::NotFound;
    };
    if entry.is_directory {
        return BoundedCatOutcome::Directory;
    }
    if entry.uncompressed_size > max_in_memory_bytes {
        return BoundedCatOutcome::TooLarge {
            uncompressed_size: entry.uncompressed_size,
            max_allowed: max_in_memory_bytes,
        };
    }
    match extract_single_entry_memory(archive_path, Some(&entry.relative_path), -1, password) {
        Ok(mut bytes) => {
            let total_size = bytes.len();
            bytes.truncate(max_bytes);
            BoundedCatOutcome::Extracted { bytes, total_size }
        }
        Err(st) => BoundedCatOutcome::StatusErr(st),
    }
}

/// Single-pass bounded entry extraction for ZIP, 7z, and TAR slices without double header parsing,
/// falling back to `extract_single_entry_memory` for multi-volume or stream containers.
pub(crate) fn extract_cat_entry_bounded(
    archive_path: &Path,
    entry_path: &str,
    password: Option<&str>,
    max_bytes: usize,
    max_in_memory_bytes: u64,
) -> BoundedCatOutcome {
    let (volumes, data) = match read_archive_data_auto(archive_path) {
        Ok(res) => res,
        Err(e) => return BoundedCatOutcome::ReadFailed(e),
    };

    if volumes.len() <= 1 {
        let single_outcome = match detect_archive_format(archive_path, &data) {
            ContainerFormat::Zip => {
                cat_zip_slice_bounded(&data, entry_path, password, max_bytes, max_in_memory_bytes)
            }
            ContainerFormat::SevenZip => cat_sevenz_slice_bounded(
                &data,
                entry_path,
                password,
                max_bytes,
                max_in_memory_bytes,
            ),
            ContainerFormat::Tar => {
                cat_tar_slice_bounded(&data, entry_path, max_bytes, max_in_memory_bytes)
            }
            _ => BoundedCatOutcome::UnsupportedFormat,
        };
        if !matches!(single_outcome, BoundedCatOutcome::UnsupportedFormat) {
            return single_outcome;
        }
    }

    cat_fallback_entry_bounded(
        archive_path,
        &data,
        entry_path,
        password,
        max_bytes,
        max_in_memory_bytes,
    )
}

/// Extracts bounded preview bytes (up to `max_bytes`) for the specified archive entry.
pub(crate) fn extract_bounded_entry_bytes(
    raw_data: &[u8],
    archive_format: &str,
    rel_path: &str,
    password: Option<&str>,
    max_bytes: usize,
) -> Vec<u8> {
    if archive_format == "ZIP" {
        if let Ok(archive) = ZipArchive::open_slice(raw_data) {
            if let Some(idx) = archive.entries().iter().position(|e| e.rel_path == rel_path) {
                return extract_zip_entry_bounded(raw_data, &archive, idx, password, max_bytes)
                    .unwrap_or_default();
            }
        }
    } else if archive_format == "7-Zip" {
        if let Ok(archive) = SevenZArchive::open_slice_with_password(raw_data, password) {
            if let Some(idx) = archive.files().iter().position(|f| f.rel_path == rel_path) {
                return extract_sevenz_entry_bounded(raw_data, &archive, idx, password, max_bytes)
                    .unwrap_or_default();
            }
        }
    }
    Vec::new()
}

impl AppState {
    /// Updates preview buffer for the currently selected item, bounded to `MAX_PREVIEW_BYTES` (64 KiB).
    pub fn update_preview_content(&mut self) {
        let node_info = {
            let visible = self.vfs.flatten_visible();
            visible.get(self.selected_index).map(|item| {
                (
                    item.node.is_dir,
                    item.node.name.clone(),
                    item.node.uncompressed_size,
                    item.node.relative_path.clone(),
                )
            })
        };

        if let Some((is_dir, filename, full_size, rel_path)) = node_info {
            if is_dir {
                self.preview_content = Some(PreviewData::Unsupported {
                    reason: "Directories cannot be previewed".to_string(),
                    file_size_bytes: 0,
                });
                self.preview_scroll = 0;
                return;
            }

            let preview_bytes = extract_bounded_entry_bytes(
                &self.archive_raw_data,
                &self.archive_format,
                &rel_path,
                self.recovered_password.as_deref(),
                MAX_PREVIEW_BYTES,
            );

            let preview = generate_preview(&filename, &preview_bytes, full_size, &self.highlighter);
            self.preview_content = Some(preview);
            self.preview_scroll = 0;
        }
    }
}
