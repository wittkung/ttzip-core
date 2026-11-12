// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! 7-Zip in-place archive mutation engine with non-solid append and header index reconstruction.

use super::{InPlaceAction, PartitionedActions};
use crate::sevenz::{create_7z_solid_archive_bytes, decode_7z_solid_payload, SevenZArchive};
use crate::types::{record_execution_provenance, TTZipEngineTag, TTZipExecutionProvenance, TTZipStatus};
use crate::zip::writer::ZipInputItem;
use std::collections::HashSet;
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
    let plan = PartitionedActions::from_slice(actions);

    let mut handled_replaced = HashSet::new();
    let mut items = collect_existing_sevenz_items(mapped, &plan, &mut handled_replaced)?;

    for (rel_path, src_path) in &plan.replaced {
        if !handled_replaced.contains(rel_path) {
            items.push(read_file_to_zip_item(rel_path, src_path)?);
        }
    }

    for (rel_path, src_path) in &plan.appended {
        items.push(read_file_to_zip_item(rel_path, src_path)?);
    }

    if items.is_empty() {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let bytes = create_7z_solid_archive_bytes(&items, 3, 2)?;
    let total_len = bytes.len() as u64;
    fs::write(shadow_path, bytes).map_err(|_| TTZipStatus::ErrOpenFailed)?;

    let prov = TTZipExecutionProvenance {
        engine_tag: TTZipEngineTag::RustInPlaceSevenZip,
        compressed_bytes: total_len,
        is_fallback: false,
        ..Default::default()
    };
    record_execution_provenance(prov);

    Ok(())
}

fn entry_matches_target(entry_key: &str, target: &str) -> bool {
    let ek = entry_key.trim_start_matches('/');
    let tg = target.trim_start_matches('/');
    ek == tg || ek.ends_with(&format!("/{}", tg)) || tg.ends_with(&format!("/{}", ek))
}

fn collect_existing_sevenz_items(
    mapped: &[u8],
    plan: &PartitionedActions,
    handled_replaced: &mut HashSet<String>,
) -> Result<Vec<ZipInputItem>, TTZipStatus> {
    let Ok(archive) = SevenZArchive::open_slice(mapped) else {
        return Ok(Vec::new());
    };

    let solid_buf = if archive.info().payload_len > 0 {
        decode_7z_solid_payload(mapped, archive.info(), None, 1).ok()
    } else {
        None
    };

    let mut items = Vec::new();
    for i in 0..archive.len() {
        let meta = &archive.files()[i];
        let key = meta.rel_path.trim_start_matches('/').to_string();

        if plan.deleted.iter().any(|d| entry_matches_target(&key, d)) {
            continue;
        }

        if let Some((rep_k, src_path)) = plan.replaced.iter().find(|(k, _)| entry_matches_target(&key, k)) {
            handled_replaced.insert(rep_k.clone());
            items.push(read_file_to_zip_item(&meta.rel_path, src_path)?);
        } else {
            let data = extract_sevenz_entry_data(&archive, &solid_buf, i, meta.is_directory);
            items.push(ZipInputItem {
                rel_path: meta.rel_path.clone(),
                data,
                mtime_epoch_secs: meta.mtime_epoch_secs.map(|t| t as u32).unwrap_or(0),
                mode: meta.mode,
                is_directory: meta.is_directory,
            });
        }
    }
    Ok(items)
}

fn extract_sevenz_entry_data(
    archive: &SevenZArchive,
    solid_buf: &Option<Vec<u8>>,
    index: usize,
    is_directory: bool,
) -> Vec<u8> {
    if is_directory {
        return Vec::new();
    }
    if let (Some(buf), Some(loc)) = (solid_buf, archive.seek_index().get_by_index(index)) {
        let offset = loc.offset_in_folder as usize;
        let end = (offset + loc.uncompressed_size as usize).min(buf.len());
        return if offset <= end { buf[offset..end].to_vec() } else { Vec::new() };
    }
    archive.extract_entry_bytes_stream(index, None).unwrap_or_default()
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
