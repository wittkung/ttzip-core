// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! ZIP in-place atomic archive mutation engine with zero recompression of untouched entries.

use super::InPlaceAction;
use crate::codecs::deflate::{deflate_compress, deflate_compress_bound};
use crate::crypto::crc32_fast;
use crate::types::{record_execution_provenance, TTZipEngineTag, TTZipExecutionProvenance, TTZipStatus};
use crate::zip::parser::{parse_all_entries, parse_local_file_header};
use crate::zip::writer::{assemble_zip_archive, ZipCompressedItem};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Modifies a ZIP archive in-place, preserving raw compressed streams of untouched entries.
pub fn in_place_edit_zip(
    archive_path: &Path,
    shadow_path: &Path,
    actions: &[InPlaceAction],
) -> Result<(), TTZipStatus> {
    let source = crate::archive::source::open_archive_source(archive_path)?;
    let mapped = source.as_slice().ok_or(TTZipStatus::ErrOpenFailed)?;
    let entries = parse_all_entries(mapped)?;

    let mut deleted = HashSet::new();
    let mut replaced = HashMap::new();
    let mut appended = Vec::new();

    for action in actions {
        match action {
            InPlaceAction::Delete { entry_path } => {
                deleted.insert(entry_path.trim_start_matches('/').to_string());
            }
            InPlaceAction::Replace { entry_path, source_path } => {
                replaced.insert(entry_path.trim_start_matches('/').to_string(), source_path.clone());
            }
            InPlaceAction::Append { entry_path, source_path } => {
                appended.push((entry_path.trim_start_matches('/').to_string(), source_path.clone()));
            }
        }
    }

    let mut new_items: Vec<ZipCompressedItem> = Vec::with_capacity(entries.len() + appended.len());

    for entry in &entries {
        let key = entry.rel_path.trim_start_matches('/').to_string();
        if deleted.contains(&key) {
            continue;
        }

        if let Some(src_path) = replaced.get(&key) {
            let item = compress_file_for_zip(&entry.rel_path, src_path)?;
            new_items.push(item);
        } else {
            let (payload_offset, _header_size) = parse_local_file_header(mapped, entry.lfh_offset as usize)?;
            let payload_start = payload_offset;
            let payload_end = payload_start + entry.compressed_size as usize;
            if payload_end > mapped.len() {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            let payload = mapped[payload_start..payload_end].to_vec();

            new_items.push(ZipCompressedItem {
                rel_path: entry.rel_path.clone(),
                uncompressed_size: entry.uncompressed_size,
                compressed_size: entry.compressed_size,
                crc32: entry.crc32,
                compression_method: entry.compression_method,
                actual_method: entry.actual_method,
                aes_strength: entry.aes_strength,
                payload,
                mtime_epoch_secs: entry.mtime_epoch_secs as u32,
                mode: entry.mode,
                is_directory: entry.is_directory,
                is_encrypted: entry.is_encrypted,
            });
        }
    }

    for (rel_path, src_path) in &appended {
        let item = compress_file_for_zip(rel_path, src_path)?;
        new_items.push(item);
    }

    let assembled = assemble_zip_archive(&new_items)?;
    let total_len = assembled.len() as u64;
    fs::write(shadow_path, assembled).map_err(|_| TTZipStatus::ErrOpenFailed)?;

    let mut prov = TTZipExecutionProvenance::default();
    prov.engine_tag = TTZipEngineTag::RustInPlaceZip;
    prov.compressed_bytes = total_len;
    prov.is_fallback = false;
    record_execution_provenance(prov);

    Ok(())
}

fn compress_file_for_zip(rel_path: &str, src_path: &Path) -> Result<ZipCompressedItem, TTZipStatus> {
    let meta = fs::symlink_metadata(src_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let is_dir = meta.is_dir();
    let mode = meta.permissions().mode();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0);

    if is_dir {
        let mut dir_path = rel_path.to_string();
        if !dir_path.ends_with('/') {
            dir_path.push('/');
        }
        return Ok(ZipCompressedItem {
            rel_path: dir_path,
            uncompressed_size: 0,
            compressed_size: 0,
            crc32: 0,
            compression_method: 0,
            actual_method: 0,
            aes_strength: 0,
            payload: Vec::new(),
            mtime_epoch_secs: mtime,
            mode: if mode != 0 { mode } else { 0o755 },
            is_directory: true,
            is_encrypted: false,
        });
    }

    let raw_data = fs::read(src_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let uncompressed_size = raw_data.len() as u64;
    let crc = crc32_fast(0, &raw_data);

    if raw_data.is_empty() {
        return Ok(ZipCompressedItem {
            rel_path: rel_path.to_string(),
            uncompressed_size: 0,
            compressed_size: 0,
            crc32: 0,
            compression_method: 0,
            actual_method: 0,
            aes_strength: 0,
            payload: Vec::new(),
            mtime_epoch_secs: mtime,
            mode: if mode != 0 { mode } else { 0o644 },
            is_directory: false,
            is_encrypted: false,
        });
    }

    let max_bound = deflate_compress_bound(raw_data.len(), 6);
    let mut comp_buf = vec![0u8; max_bound];
    match deflate_compress(&raw_data, &mut comp_buf, 6) {
        Ok(comp_len) if comp_len < raw_data.len() => {
            comp_buf.truncate(comp_len);
            Ok(ZipCompressedItem {
                rel_path: rel_path.to_string(),
                uncompressed_size,
                compressed_size: comp_len as u64,
                crc32: crc,
                compression_method: 8,
                actual_method: 8,
                aes_strength: 0,
                payload: comp_buf,
                mtime_epoch_secs: mtime,
                mode: if mode != 0 { mode } else { 0o644 },
                is_directory: false,
                is_encrypted: false,
            })
        }
        _ => Ok(ZipCompressedItem {
            rel_path: rel_path.to_string(),
            uncompressed_size,
            compressed_size: uncompressed_size,
            crc32: crc,
            compression_method: 0,
            actual_method: 0,
            aes_strength: 0,
            payload: raw_data,
            mtime_epoch_secs: mtime,
            mode: if mode != 0 { mode } else { 0o644 },
            is_directory: false,
            is_encrypted: false,
        }),
    }
}
