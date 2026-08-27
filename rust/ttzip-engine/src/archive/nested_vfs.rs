// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! In-Memory Nested Archive Recursion, Drill-Down, and Virtual File Stream.

#[path = "nested_vfs/buffer.rs"]
mod buffer;
#[path = "nested_vfs/stream.rs"]
mod stream;

use std::sync::Arc;

pub use buffer::*;
pub use stream::*;

use crate::archive::tar::reader::TarArchive;
use crate::uniffi_api::types::{TTZipError, UniFFIEntryMetadata};
use crate::zip::reader::ZipArchive;

pub fn drill_down_buffer(
    root_bytes: &[u8],
    drill_path: &[String],
    password: Option<&str>,
) -> Result<Vec<UniFFIEntryMetadata>, TTZipError> {
    let mut curr = root_bytes.to_vec();
    for seg in drill_path {
        let clean = seg.trim();
        if !clean.is_empty() {
            curr = extract_entry_from_buffer(&curr, clean, password)?;
        }
    }
    inspect_entries_from_buffer(&curr, password)
}

pub fn extract_nested_entry_buffer(
    root_bytes: &[u8],
    drill_path: &[String],
    target: &str,
    password: Option<&str>,
) -> Result<Vec<u8>, TTZipError> {
    let (drill, eff_target) = parse_nested_specifier(drill_path, target);
    let mut curr = root_bytes.to_vec();
    for seg in &drill {
        let clean = seg.trim();
        if !clean.is_empty() {
            curr = extract_entry_from_buffer(&curr, clean, password)?;
        }
    }
    extract_entry_from_buffer(&curr, &eff_target, password)
}

pub fn drill_down_nested_archive(
    archive_path: &str,
    drill_path: &[String],
    password: Option<&str>,
) -> Result<Vec<UniFFIEntryMetadata>, TTZipError> {
    let p = std::path::Path::new(archive_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path.to_string() });
    }
    let source = crate::archive::source::open_archive_source(p)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    let mapped = source.as_slice().ok_or_else(|| TTZipError::IoError {
        message: "Failed to map archive bytes".to_string(),
    })?;
    drill_down_buffer(mapped, drill_path, password)
}

pub fn open_virtual_file_stream(
    archive_path: &str,
    drill_path: &[String],
    target_entry: &str,
    password: Option<&str>,
) -> Result<Arc<VirtualFileStream>, TTZipError> {
    let p = std::path::Path::new(archive_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path.to_string() });
    }
    let source = Arc::new(crate::archive::source::open_archive_source(p)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?);
    let mapped = source.as_slice().ok_or_else(|| TTZipError::IoError {
        message: "Failed to map archive bytes".to_string(),
    })?;
    let (drill, eff_target) = parse_nested_specifier(drill_path, target_entry);
    if drill.is_empty() {
        if (mapped.starts_with(b"PK\x03\x04") || mapped.starts_with(b"PK\x05\x06")) && password.is_none() {
            if let Ok(zip) = ZipArchive::open_slice(mapped) {
                if let Some(entry) = zip.entries().iter().find(|e| matches_entry_path(&e.rel_path, &eff_target)) {
                    if entry.actual_method == 0 && !entry.is_encrypted {
                        if let Ok((payload_off, _)) = crate::zip::parser::parse_local_file_header(mapped, entry.lfh_offset as usize) {
                            let total_size = entry.uncompressed_size;
                            let chunk_size = calculate_chunk_size(total_size);
                            let source_arc = Arc::clone(&source);
                            let base_off = payload_off as u64;
                            let loader = Arc::new(move |offset: u64, len: usize| {
                                let mut buf = vec![0u8; len];
                                let n = source_arc.read_at(&mut buf, base_off + offset)
                                    .map_err(|e| TTZipError::IoError { message: e.to_string() })?;
                                buf.truncate(n);
                                Ok(buf)
                            });
                            return Ok(Arc::new(VirtualFileStream::new(VirtualChunkedStream::new(total_size, chunk_size, loader))));
                        }
                    }
                }
            }
        }
        if let Ok(tar) = TarArchive::open_slice(mapped) {
            if let Some(entry) = tar.entries().iter().find(|e| matches_entry_path(e.path.as_ref(), &eff_target)) {
                let total_size = entry.size;
                let chunk_size = calculate_chunk_size(total_size);
                let source_arc = Arc::clone(&source);
                let base_off = entry.data_offset as u64;
                let loader = Arc::new(move |offset: u64, len: usize| {
                    let mut buf = vec![0u8; len];
                    let n = source_arc.read_at(&mut buf, base_off + offset)
                        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;
                    buf.truncate(n);
                    Ok(buf)
                });
                return Ok(Arc::new(VirtualFileStream::new(VirtualChunkedStream::new(total_size, chunk_size, loader))));
            }
        }
    }
    let payload = extract_nested_entry_buffer(mapped, drill_path, target_entry, password)?;
    Ok(Arc::new(VirtualFileStream::from_vec(payload)))
}
