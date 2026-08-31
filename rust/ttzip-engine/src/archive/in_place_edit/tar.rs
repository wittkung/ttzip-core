// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! POSIX TAR in-place atomic archive mutation engine with 512-byte aligned block overwrites and appends.

use super::InPlaceAction;
use crate::archive::tar::header::TAR_BLOCK_SIZE;
use crate::archive::tar::scanner::TarSeekScanner;
use crate::archive::tar::writer::TarWriter;
use crate::types::{record_execution_provenance, TTZipEngineTag, TTZipExecutionProvenance, TTZipStatus};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Modifies a POSIX TAR archive in-place with 512-byte aligned block overwrites and appends.
pub fn in_place_edit_tar(
    archive_path: &Path,
    shadow_path: &Path,
    actions: &[InPlaceAction],
) -> Result<(), TTZipStatus> {
    let source = crate::archive::source::open_archive_source(archive_path)?;
    let mapped = source.as_slice().ok_or(TTZipStatus::ErrOpenFailed)?;

    let new_tar_bytes = mutate_tar_bytes(mapped, actions)?;
    let total_len = new_tar_bytes.len() as u64;
    fs::write(shadow_path, new_tar_bytes).map_err(|_| TTZipStatus::ErrOpenFailed)?;

    let prov = TTZipExecutionProvenance {
        engine_tag: TTZipEngineTag::RustTarStreamEngine,
        compressed_bytes: total_len,
        is_fallback: false,
        ..Default::default()
    };
    record_execution_provenance(prov);

    Ok(())
}

pub(crate) fn mutate_tar_bytes(mapped: &[u8], actions: &[InPlaceAction]) -> Result<Vec<u8>, TTZipStatus> {
    let mut scanner = TarSeekScanner::new(mapped);
    let entries = scanner.scan_all()?;

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

    let mut out = Vec::with_capacity(mapped.len() + 65536);
    let mut processed_replaced = HashSet::new();

    for entry in &entries {
        let key = entry.path.trim_start_matches('/').to_string();
        if deleted.contains(&key) {
            continue;
        }

        if let Some(src_path) = replaced.get(&key) {
            processed_replaced.insert(key);
            write_tar_entry_from_file(&mut out, &entry.path, src_path)?;
        } else {
            let block_start = entry.header_offset;
            let payload_size = entry.size as usize;
            let payload_blocks = payload_size.div_ceil(TAR_BLOCK_SIZE) * TAR_BLOCK_SIZE;
            let block_end = entry.data_offset + payload_blocks;
            if block_end <= mapped.len() && block_start <= block_end {
                out.extend_from_slice(&mapped[block_start..block_end]);
            }
        }
    }

    for (key, src_path) in &replaced {
        if !processed_replaced.contains(key) {
            write_tar_entry_from_file(&mut out, key, src_path)?;
        }
    }

    for (rel_path, src_path) in &appended {
        write_tar_entry_from_file(&mut out, rel_path, src_path)?;
    }

    out.extend_from_slice(&[0u8; 1024]);
    Ok(out)
}

pub(crate) fn write_tar_entry_from_file(out: &mut Vec<u8>, rel_path: &str, src_path: &Path) -> Result<(), TTZipStatus> {
    let meta = fs::symlink_metadata(src_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let is_dir = meta.is_dir();
    let mode = meta.permissions().mode();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let mut writer = TarWriter::new(out);
    if is_dir {
        writer.append_dir(rel_path, mode, mtime)?;
    } else {
        let data = fs::read(src_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
        writer.append_file(rel_path, &data, mode, mtime)?;
    }
    Ok(())
}
