// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 7-Zip in-place archive mutation engine with non-solid append and header index reconstruction.

use super::InPlaceAction;
use crate::sevenz::{create_7z_solid_archive_bytes, decode_7z_solid_payload, SevenZArchive};
use crate::types::{record_execution_provenance, TTZipEngineTag, TTZipExecutionProvenance, TTZipStatus};
use crate::zip::writer::ZipInputItem;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Modifies a 7z archive in-place with non-solid append and full header index reconstruction.
pub fn in_place_edit_sevenz(
    archive_path: &Path,
    shadow_path: &Path,
    actions: &[InPlaceAction],
) -> Result<(), TTZipStatus> {
    let source = crate::archive::source::open_archive_source(archive_path)?;
    let mapped = source.as_slice().ok_or(TTZipStatus::ErrOpenFailed)?;

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

    let mut items: Vec<ZipInputItem> = Vec::new();
    let mut handled_replaced = HashSet::new();

    let is_match = |entry_key: &str, target: &str| -> bool {
        let ek = entry_key.trim_start_matches('/');
        let tg = target.trim_start_matches('/');
        ek == tg || ek.ends_with(&format!("/{}", tg)) || tg.ends_with(&format!("/{}", ek))
    };

    if let Ok(archive) = SevenZArchive::open_slice(mapped) {
        let solid_buf = if archive.info().payload_len > 0 {
            decode_7z_solid_payload(mapped, archive.info(), None, 1).ok()
        } else {
            None
        };

        for i in 0..archive.len() {
            let meta = &archive.files()[i];
            let key = meta.rel_path.trim_start_matches('/').to_string();

            if deleted.iter().any(|d| is_match(&key, d)) {
                continue;
            }

            if let Some((rep_k, src_path)) = replaced.iter().find(|(k, _)| is_match(&key, k)) {
                handled_replaced.insert(rep_k.clone());
                items.push(read_file_to_zip_item(&meta.rel_path, src_path)?);
            } else {
                let data = if meta.is_directory {
                    Vec::new()
                } else if let (Some(buf), Some(loc)) = (&solid_buf, archive.seek_index().get_by_index(i)) {
                    let offset = loc.offset_in_folder as usize;
                    let end = (offset + loc.uncompressed_size as usize).min(buf.len());
                    if offset <= end {
                        buf[offset..end].to_vec()
                    } else {
                        Vec::new()
                    }
                } else {
                    archive.extract_entry_bytes_stream(i, None).unwrap_or_default()
                };
                items.push(ZipInputItem {
                    rel_path: meta.rel_path.clone(),
                    data,
                    mtime_epoch_secs: meta.mtime_epoch_secs.map(|t| t as u32).unwrap_or(0),
                    mode: meta.mode,
                    is_directory: meta.is_directory,
                });
            }
        }
    }

    for (rel_path, src_path) in &replaced {
        if !handled_replaced.contains(rel_path) {
            items.push(read_file_to_zip_item(rel_path, src_path)?);
        }
    }

    for (rel_path, src_path) in &appended {
        items.push(read_file_to_zip_item(rel_path, src_path)?);
    }

    if items.is_empty() {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let bytes = create_7z_solid_archive_bytes(&items, 3, 2)?;
    let total_len = bytes.len() as u64;
    fs::write(shadow_path, bytes).map_err(|_| TTZipStatus::ErrOpenFailed)?;

    let mut prov = TTZipExecutionProvenance::default();
    prov.engine_tag = TTZipEngineTag::RustInPlaceSevenZip;
    prov.compressed_bytes = total_len;
    prov.is_fallback = false;
    record_execution_provenance(prov);

    Ok(())
}

fn read_file_to_zip_item(rel_path: &str, src_path: &Path) -> Result<ZipInputItem, TTZipStatus> {
    let meta_src = fs::symlink_metadata(src_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let is_dir = meta_src.is_dir();
    let data = if is_dir { Vec::new() } else { fs::read(src_path).map_err(|_| TTZipStatus::ErrFileNotFound)? };
    let mode = meta_src.permissions().mode();
    let mtime = meta_src
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0);

    Ok(ZipInputItem {
        rel_path: rel_path.to_string(),
        data,
        mtime_epoch_secs: mtime,
        mode: if mode != 0 { mode } else if is_dir { 0o755 } else { 0o644 },
        is_directory: is_dir,
    })
}
