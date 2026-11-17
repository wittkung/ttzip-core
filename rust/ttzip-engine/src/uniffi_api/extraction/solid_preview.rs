// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use std::path::Path;
use std::sync::Arc;

use crate::archive::source::open_local_regular_archive_mmap;
use crate::sevenz::decoder::stream::SolidLzma2Preview;

use super::{CancellationToken, TTZipError, TTZipStatus, map_single_entry_status};

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum SolidLzma2PreviewError {
    #[error("Solid preview cancelled")]
    Cancelled,
    #[error("Solid preview work budget exceeded")]
    BudgetExceeded,
    #[error("Solid preview memory limit reached")]
    ResourceLimit,
    #[error("Unsupported solid preview format")]
    Unsupported,
    #[error("Corrupt solid preview archive")]
    Corrupt,
    #[error("Solid preview failed: {message}")]
    Failed { message: String },
}

impl From<TTZipError> for SolidLzma2PreviewError {
    fn from(error: TTZipError) -> Self {
        match error {
            TTZipError::Cancelled => Self::Cancelled,
            TTZipError::EngineError { code } if code == TTZipStatus::Cancelled as i32 => {
                Self::Cancelled
            }
            TTZipError::EngineError { code }
                if code == TTZipStatus::ErrUnsupportedFeature as i32 =>
            {
                Self::Unsupported
            }
            TTZipError::DecompressionBombDetected { .. } => Self::BudgetExceeded,
            TTZipError::OutOfMemory { .. } => Self::ResourceLimit,
            TTZipError::CorruptHeader { .. } | TTZipError::ChecksumMismatch { .. } => Self::Corrupt,
            failure => Self::Failed {
                message: failure.to_string(),
            },
        }
    }
}

/// Opts into a single bounded read of a CRC-protected plaintext LZMA2 entry.
#[uniffi::export]
pub fn extract_solid_lzma2_preview(
    archive_path: String,
    entry_path: String,
    token: Arc<CancellationToken>,
) -> Result<Vec<u8>, SolidLzma2PreviewError> {
    extract_solid_preview_from_path(
        Path::new(&archive_path),
        &entry_path,
        preview_work(&|| token.is_cancelled()),
    )
    .map_err(Into::into)
}

pub(super) fn extract_solid_preview_from_path(
    path: &Path,
    entry_path: &str,
    policy: SolidLzma2Preview<'_>,
) -> Result<Vec<u8>, TTZipError> {
    let cancelled = || map_single_entry_status(TTZipStatus::Cancelled, entry_path);
    if (policy.cancellation)() {
        return Err(cancelled());
    }
    if !path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("7z"))
    {
        return Err(map_single_entry_status(
            TTZipStatus::ErrUnsupportedFeature,
            entry_path,
        ));
    }
    let source = open_local_regular_archive_mmap(path);
    if (policy.cancellation)() {
        return Err(cancelled());
    }
    let source =
        source.map_err(|status| map_single_entry_status(status, &path.to_string_lossy()))?;
    let mapped = source.as_slice().ok_or_else(|| TTZipError::IoError {
        message: "Failed to map archive bytes".to_string(),
    })?;
    if !mapped.starts_with(b"7z\xBC\xAF\x27\x1C") {
        return Err(map_single_entry_status(
            TTZipStatus::ErrUnsupportedFeature,
            entry_path,
        ));
    }
    extract_solid_preview_by_path(mapped, entry_path, policy)
}

fn extract_solid_preview_by_path(
    mapped: &[u8],
    entry_path: &str,
    policy: SolidLzma2Preview<'_>,
) -> Result<Vec<u8>, TTZipError> {
    use crate::sevenz::header::{SevenZSeekIndex, parse_solid_preview_metadata};
    let info = parse_solid_preview_metadata(mapped, policy.cancellation)
        .map_err(|status| map_single_entry_status(status, entry_path))?;
    if (policy.cancellation)() {
        return Err(map_single_entry_status(TTZipStatus::Cancelled, entry_path));
    }
    let relative_path = entry_path.trim_start_matches('/');
    let index = info
        .files
        .iter()
        .position(|file| file.rel_path == relative_path)
        .ok_or_else(|| TTZipError::FileNotFound {
            path: entry_path.to_owned(),
        })?;
    let seek_index = SevenZSeekIndex::build(&info);
    crate::sevenz::decoder::stream::extract_solid_lzma2_preview(
        mapped,
        &info,
        &seek_index,
        index,
        policy,
    )
    .map_err(|status| map_single_entry_status(status, entry_path))
}

fn preview_work(cancelled: &dyn Fn() -> bool) -> SolidLzma2Preview<'_> {
    const MIB: usize = 1024 * 1024;
    SolidLzma2Preview {
        max_packed_bytes: 40 * MIB,
        max_decoded_bytes: 66 * MIB as u64,
        cancellation: cancelled,
    }
}

pub(super) fn extract_selected_solid_preview(
    mapped: &[u8],
    selected_path: &str,
    index: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, TTZipError> {
    use crate::sevenz::header::{SevenZSeekIndex, parse_solid_preview_metadata};

    let terminal_status = |status| match status {
        TTZipStatus::Cancelled => TTZipError::Cancelled,
        TTZipStatus::ErrUnsupportedFeature => TTZipError::SecurityViolation {
            reason: "Unsupported metadata for bounded 7z preview".to_owned(),
        },
        status => map_single_entry_status(status, selected_path),
    };
    let info = parse_solid_preview_metadata(mapped, cancelled).map_err(terminal_status)?;
    if info.files.get(index).is_none_or(|file| file.rel_path != selected_path) {
        return Err(terminal_status(TTZipStatus::ErrCorruptHeader));
    }
    let seek_index = SevenZSeekIndex::build(&info);
    crate::sevenz::decoder::stream::extract_solid_lzma2_preview(
        mapped, &info, &seek_index, index, preview_work(cancelled),
    )
    .map_err(terminal_status)
}
