// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use std::path::Path;
use std::sync::Arc;

use crate::archive::source::{
    ArchiveSource, open_archive_source, try_open_local_regular_archive_mmap,
};
use crate::sevenz::format::{K_HEADER, METHOD_LZMA2, SEVENZ_SIGNATURE, SevenZSignatureHeader};
use crate::sevenz::header::{SevenZEntryLocation, SevenZFolder, SevenZHeaderInfo};

use super::{
    CancellationToken, TTZipError, TTZipStatus, enforce_7z_entry_budget,
    extract_single_entry_split_guarded, map_single_entry_status, open_sevenz_slice_for_preview,
    resolve_7z_entry_index, solid_preview,
};

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum ArchiveEntryPreviewError {
    #[error(transparent)]
    Legacy(#[from] TTZipError),
    #[error("Archive preview stopped: {0}")]
    Terminal(#[source] TTZipError),
}

impl ArchiveEntryPreviewError {
    pub(super) fn into_archive_error(self) -> TTZipError {
        match self {
            Self::Legacy(error) | Self::Terminal(error) => error,
        }
    }
}

/// Reads one preview entry with optional bounded continuation of a local plaintext 7z stream.
/// A terminal failure precludes another extraction or a catalog retry.
pub fn extract_single_entry_for_preview(
    archive_path: String,
    entry_path: String,
    password: Option<String>,
    token: &CancellationToken,
) -> Result<Vec<u8>, ArchiveEntryPreviewError> {
    let cancelled = || token.is_cancelled();
    let result = extract_single_entry(archive_path, entry_path, password, Some(&cancelled));
    check_cancelled(Some(&cancelled))?;
    result
}

#[uniffi::export]
pub fn read_archive_preview_entry(
    archive_path: String,
    entry_path: String,
    password: Option<String>,
    token: Arc<CancellationToken>,
) -> Result<Vec<u8>, ArchiveEntryPreviewError> {
    extract_single_entry_for_preview(archive_path, entry_path, password, &token)
}

pub(super) fn extract_single_entry(
    archive_path: String,
    entry_path: String,
    password: Option<String>,
    cancellation: Option<&dyn Fn() -> bool>,
) -> Result<Vec<u8>, ArchiveEntryPreviewError> {
    check_cancelled(cancellation)?;
    let secure_pwd = password.map(crate::crypto::ZeroizingString::new);
    let p = std::path::Path::new(&archive_path);
    let volume_chain =
        crate::archive::split::detect_volume_chain(p).map_err(|_| TTZipError::FileNotFound {
            path: archive_path.clone(),
        })?;
    if volume_chain.len() > 1 {
        return extract_single_entry_split_guarded(
            &volume_chain,
            p,
            Some(&entry_path),
            -1,
            secure_pwd.as_deref(),
            64,
        )
        .map_err(Into::into);
    }
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path }.into());
    }

    let local_mapping = match cancellation {
        Some(_) if passwordless_sevenz(p, secure_pwd.as_deref()) => {
            try_open_local_regular_archive_mmap(p).map_err(|status| {
                ArchiveEntryPreviewError::Terminal(map_single_entry_status(status, &archive_path))
            })?
        }
        _ => None,
    };
    let source = match local_mapping {
        Some(source) => {
            return extract_from_mapped_source(
                &source,
                p,
                &entry_path,
                secure_pwd.as_deref(),
                cancellation,
            );
        }
        None => open_archive_source(p).map_err(|e| TTZipError::IoError {
            message: e.to_string(),
        })?,
    };
    extract_from_mapped_source(source.as_ref(), p, &entry_path, secure_pwd.as_deref(), None)
}

fn check_cancelled(
    cancellation: Option<&dyn Fn() -> bool>,
) -> Result<(), ArchiveEntryPreviewError> {
    if cancellation.is_some_and(|cancelled| cancelled()) {
        return Err(ArchiveEntryPreviewError::Terminal(TTZipError::Cancelled));
    }
    Ok(())
}

fn passwordless_sevenz(path: &Path, password: Option<&str>) -> bool {
    password.is_none()
        && path
            .extension()
            .is_some_and(|suffix| suffix.eq_ignore_ascii_case("7z"))
}

fn extract_from_mapped_source(
    source: &dyn ArchiveSource,
    path: &Path,
    entry_path: &str,
    password: Option<&str>,
    cancellation: Option<&dyn Fn() -> bool>,
) -> Result<Vec<u8>, ArchiveEntryPreviewError> {
    check_cancelled(cancellation)?;
    let mapped = source.as_slice().ok_or_else(|| TTZipError::IoError {
        message: "Failed to map archive bytes".to_string(),
    })?;

    const DEFAULT_EXTRACTION_BUDGET_BYTES: u64 = 64 * 1024 * 1024;
    if mapped.starts_with(&SEVENZ_SIGNATURE) {
        extract_7z_entry_by_path_bounded(
            mapped,
            entry_path,
            password,
            DEFAULT_EXTRACTION_BUDGET_BYTES,
            cancellation,
        )
    } else if let Ok(zip_archive) = crate::zip::reader::ZipArchive::open_slice(mapped) {
        extract_zip_entry_by_path_bounded(
            &zip_archive,
            entry_path.trim_start_matches('/'),
            entry_path,
            password,
            DEFAULT_EXTRACTION_BUDGET_BYTES,
        )
        .map_err(Into::into)
    } else {
        crate::archive::unified::extract_single::extract_single_entry_memory(
            path,
            Some(entry_path),
            -1,
            password,
        )
        .map_err(|status| map_single_entry_status(status, entry_path).into())
    }
}

fn is_late_plain_lzma2(
    mapped: &[u8],
    info: &SevenZHeaderInfo,
    location: &SevenZEntryLocation,
    budget_bytes: u64,
) -> Result<bool, TTZipError> {
    if location.offset_in_folder <= budget_bytes || info.is_encrypted {
        return Ok(false);
    }
    let ([folder], Some(0), [_]) = (
        info.folders.as_slice(),
        location.folder_index,
        info.pack_sizes.as_slice(),
    ) else {
        return Ok(false);
    };
    if !matches!(folder.coders.as_slice(), [coder]
        if coder.method_id == METHOD_LZMA2
            && coder.num_in_streams == 1
            && coder.num_out_streams == 1)
    {
        return Ok(false);
    }

    let signature = SevenZSignatureHeader::parse(mapped)
        .map_err(|status| map_single_entry_status(status, "7z signature"))?;
    let plain = usize::try_from(signature.next_header_offset)
        .ok()
        .and_then(|offset| offset.checked_add(32))
        .and_then(|start| mapped.get(start))
        == Some(&K_HEADER);
    Ok(plain)
}

fn extract_zip_entry_by_path_bounded(
    zip_archive: &crate::zip::reader::ZipArchive<'_>,
    clean_target: &str,
    entry_path: &str,
    password: Option<&str>,
    budget_bytes: u64,
) -> Result<Vec<u8>, TTZipError> {
    let suffix = format!("/{clean_target}");
    let found_idx = zip_archive
        .entries()
        .iter()
        .position(|e| e.rel_path == clean_target || e.rel_path.ends_with(&suffix))
        .ok_or_else(|| TTZipError::FileNotFound {
            path: entry_path.to_string(),
        })?;
    let entry = &zip_archive.entries()[found_idx];
    if entry.uncompressed_size > budget_bytes {
        return Err(TTZipError::OutOfMemory {
            details: format!(
                "Entry uncompressed size ({} bytes) exceeds safety budget ({budget_bytes} bytes)",
                entry.uncompressed_size
            ),
        });
    }
    zip_archive
        .extract_entry_bytes(found_idx, password)
        .map_err(|s| map_single_entry_status(s, entry_path))
}

fn read_stored_7z_preview(
    mapped: &[u8],
    archive: &crate::sevenz::decoder::SevenZArchive<'_>,
    folder: &SevenZFolder,
    index: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    let signature = crate::sevenz::format::SevenZSignatureHeader::parse(mapped)?;
    let end_of_payload = usize::try_from(signature.next_header_offset)
        .ok()
        .and_then(|offset| offset.checked_add(32))
        .and_then(|end| mapped.get(..end))
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    let packed_end = folder
        .packed_offset
        .checked_add(folder.packed_len)
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    if folder.packed_offset < 32
        || packed_end > end_of_payload.len()
        || folder.unpack_sizes.as_slice() != [folder.packed_len as u64]
    {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    crate::archive::stream_adapter::extract_7z_single_entry_from_reader_fast(
        &mut std::io::Cursor::new(end_of_payload),
        archive.info(),
        archive.seek_index(),
        index,
        None,
    )
}

fn late_stored_folder<'a>(
    info: &'a SevenZHeaderInfo,
    location: &SevenZEntryLocation,
    budget_bytes: u64,
) -> Option<&'a SevenZFolder> {
    if location.offset_in_folder <= budget_bytes {
        return None;
    }
    let folder = info.folders.get(location.folder_index?)?;
    (crate::archive::stream_adapter::is_unencrypted_copy_folder(info, folder)
        && matches!(folder.coders.as_slice(), [coder]
            if coder.num_in_streams == 1
                && coder.num_out_streams == 1
                && coder.properties.is_empty()))
    .then_some(folder)
}

fn extract_7z_entry_by_path_bounded(
    mapped: &[u8],
    entry_path: &str,
    password: Option<&str>,
    budget_bytes: u64,
    cancellation: Option<&dyn Fn() -> bool>,
) -> Result<Vec<u8>, ArchiveEntryPreviewError> {
    let archive = open_sevenz_slice_for_preview(mapped, password)?;
    check_cancelled(cancellation)?;
    let index = resolve_7z_entry_index(archive.files(), Some(entry_path), -1)?;
    let info = archive.info();
    let seek_index = archive.seek_index();
    if let Some(loc) = seek_index.get_by_index(index) {
        if let Some(folder) = late_stored_folder(info, loc, budget_bytes) {
            enforce_7z_entry_budget(loc, 0)?;
            return read_stored_7z_preview(mapped, &archive, folder, index)
                .map_err(|status| map_single_entry_status(status, entry_path).into());
        }
        if let Some(cancelled) = cancellation
            && is_late_plain_lzma2(mapped, info, loc, budget_bytes)?
        {
            return solid_preview::extract_selected_solid_preview(
                mapped,
                &archive.files()[index].rel_path,
                index,
                cancelled,
            )
            .map_err(ArchiveEntryPreviewError::Terminal);
        }
        enforce_7z_entry_budget(loc, budget_bytes)?;
    }
    crate::sevenz::decoder::stream::extract_entry_bytes_stream_bounded(
        mapped,
        info,
        seek_index,
        index,
        password,
        budget_bytes,
    )
    .map_err(|status| map_single_entry_status(status, entry_path).into())
}
