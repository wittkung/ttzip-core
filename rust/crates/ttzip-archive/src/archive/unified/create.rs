// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Archive creation lifecycle submodule for Unified Orchestrator.

use std::fs;
use std::path::{Path, PathBuf};

pub use super::create_staging::{
    commit_staged_archive_files_in_txn_with, commit_staged_archive_files_with,
    recover_preserved_create_backup, with_staged_archive_destination,
    with_staged_archive_destination_detailed, with_staged_archive_destination_detailed_outcome,
    with_staged_archive_destination_detailed_using,
    with_staged_archive_destination_detailed_using_outcome, with_staged_archive_destination_using,
    CreateCommitMode, CreateStageCommitError, CreateStagedCommitOutput,
};
pub use super::create_to_writer::{
    create_archive_to_writer, run_libarchive_staged_write, run_libarchive_staged_write_in_txn,
};
use super::create_to_writer::{
    collect_source_entries, write_archive_entries_to_handle_fmt, write_entries_slice_to_handle_fmt,
};
use super::create_transaction::CreateTransactionContext;
use super::format_config::{
    write_brotli_target_in_txn, write_snappy_target_in_txn, write_zstd_target_in_txn,
};
use crate::types::{TTZipArchiveFormat, TTZipCreateOptions, TTZipStatus};

#[inline]
pub(crate) fn is_pure_single_stream_format(fmt: TTZipArchiveFormat) -> bool {
    matches!(
        fmt,
        TTZipArchiveFormat::Gzip
            | TTZipArchiveFormat::Bzip2
            | TTZipArchiveFormat::Xz
            | TTZipArchiveFormat::Zstd
            | TTZipArchiveFormat::Lz4
            | TTZipArchiveFormat::Brotli
            | TTZipArchiveFormat::Snappy
            | TTZipArchiveFormat::Lzip
            | TTZipArchiveFormat::Lrzip
    )
}

#[inline]
fn is_single_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.is_file() && !m.file_type().is_symlink())
        .unwrap_or(false)
}

#[inline]
pub(crate) fn should_use_raw_stream(
    source_paths: &[PathBuf],
    resolved_format: TTZipArchiveFormat,
) -> bool {
    source_paths.len() == 1
        && is_single_regular_file(&source_paths[0])
        && is_pure_single_stream_format(resolved_format)
}

#[inline]
fn should_use_raw_stream_explicit(
    entries: &[(PathBuf, String)],
    resolved_format: TTZipArchiveFormat,
) -> bool {
    entries.len() == 1
        && is_single_regular_file(&entries[0].0)
        && is_pure_single_stream_format(resolved_format)
}

fn prepare_staged_create(
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    is_explicit: bool,
    encrypt_file_names: Option<bool>,
    raw_stream_ok: impl FnOnce(TTZipArchiveFormat) -> bool,
) -> Result<(TTZipArchiveFormat, CreateCommitMode), CreateStageCommitError> {
    let resolved_format =
        super::format_config::effective_create_format(options.format, destination_path);
    super::format_config::validate_creatable_format(resolved_format, is_explicit)
        .map_err(CreateStageCommitError::Status)?;
    super::create_staging::validate_create_encryption_options(
        resolved_format,
        options,
        encrypt_file_names,
    )
    .map_err(CreateStageCommitError::Status)?;
    if is_pure_single_stream_format(resolved_format) && !raw_stream_ok(resolved_format) {
        return Err(CreateStageCommitError::Status(TTZipStatus::ErrInvalidParam));
    }
    Ok((
        resolved_format,
        CreateCommitMode::for_creation(destination_path, resolved_format, split_volume_size_bytes),
    ))
}

/// Recursively compresses source paths into the destination archive path with a cancellation probe,
/// returning `CreateStagedCommitOutput` or `CreateStageCommitError`.
pub fn create_archive_detailed_outcome_with_cancel<C>(
    source_paths: &[PathBuf],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    cancel_probe: C,
) -> Result<CreateStagedCommitOutput, CreateStageCommitError>
where
    C: FnOnce() -> bool,
{
    if source_paths.is_empty() {
        return Err(CreateStageCommitError::Status(TTZipStatus::ErrInvalidParam));
    }
    for src_path in source_paths {
        if !src_path.exists() && fs::symlink_metadata(src_path).is_err() {
            return Err(CreateStageCommitError::Status(TTZipStatus::ErrFileNotFound));
        }
    }
    let (resolved_format, commit_mode) = prepare_staged_create(
        destination_path,
        options,
        split_volume_size_bytes,
        false,
        None,
        |fmt| should_use_raw_stream(source_paths, fmt),
    )?;
    with_staged_archive_destination_detailed_outcome(
        destination_path,
        commit_mode,
        |staged_dest, txn_ctx| {
            write_staged_source_archive_in_txn(
                source_paths,
                staged_dest,
                resolved_format,
                options,
                split_volume_size_bytes,
                Some(txn_ctx),
            )
        },
        cancel_probe,
    )
}

/// Recursively compresses source paths into the destination archive path, returning `CreateStagedCommitOutput`.
pub fn create_archive_detailed_outcome(
    source_paths: &[PathBuf],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> Result<CreateStagedCommitOutput, CreateStageCommitError> {
    create_archive_detailed_outcome_with_cancel(
        source_paths,
        destination_path,
        options,
        split_volume_size_bytes,
        || false,
    )
}

/// Recursively compresses source paths into the destination archive path with detailed rollback diagnostics.
pub fn create_archive_detailed(
    source_paths: &[PathBuf],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> Result<Vec<PathBuf>, CreateStageCommitError> {
    create_archive_detailed_outcome(
        source_paths,
        destination_path,
        options,
        split_volume_size_bytes,
    )
    .map(|out| out.committed_files)
}

/// Recursively compresses source paths into the destination archive path.
pub fn create_archive(
    source_paths: &[PathBuf],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> Result<(), TTZipStatus> {
    create_archive_detailed(source_paths, destination_path, options, split_volume_size_bytes)
        .map(|_| ())
        .map_err(CreateStageCommitError::log_and_into_status)
}

fn write_staged_special_container_in_txn(
    source_paths: &[PathBuf],
    staged_dest: &Path,
    resolved_format: TTZipArchiveFormat,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    txn_ctx: Option<&CreateTransactionContext>,
) -> Option<Result<(), TTZipStatus>> {
    match resolved_format {
        TTZipArchiveFormat::Zip | TTZipArchiveFormat::Auto => Some(
            crate::zip::writer::create_zip_streaming_parallel_split_in_txn(
                staged_dest,
                source_paths,
                options,
                split_volume_size_bytes,
                txn_ctx,
            )
            .map(|_| ()),
        ),
        TTZipArchiveFormat::SevenZip => Some(
            crate::sevenz::writer::create_7z_archive_split_in_txn(
                staged_dest,
                source_paths,
                options,
                split_volume_size_bytes,
                txn_ctx,
            )
            .map(|_| ()),
        ),
        TTZipArchiveFormat::Aar => {
            Some(collect_source_entries(source_paths).and_then(|entries| {
                super::format_config::write_apple_archive_entries_in_txn(
                    &entries,
                    staged_dest,
                    split_volume_size_bytes,
                    txn_ctx,
                )
            }))
        }
        TTZipArchiveFormat::Xar => {
            Some(collect_source_entries(source_paths).and_then(|entries| {
                super::format_config::write_xar_entries_in_txn(
                    &entries,
                    staged_dest,
                    options,
                    split_volume_size_bytes,
                    txn_ctx,
                )
            }))
        }
        _ => None,
    }
}

fn write_staged_source_archive_in_txn(
    source_paths: &[PathBuf],
    staged_dest: &Path,
    resolved_format: TTZipArchiveFormat,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    txn_ctx: Option<&CreateTransactionContext>,
) -> Result<(), TTZipStatus> {
    if let Some(res) = write_staged_special_container_in_txn(
        source_paths,
        staged_dest,
        resolved_format,
        options,
        split_volume_size_bytes,
        txn_ctx,
    ) {
        return res;
    }

    let is_raw_stream = should_use_raw_stream(source_paths, resolved_format);
    let raw_src = is_raw_stream.then(|| source_paths[0].as_path());

    if matches!(
        resolved_format,
        TTZipArchiveFormat::TarBrotli | TTZipArchiveFormat::Brotli
    ) {
        return write_brotli_target_in_txn(
            staged_dest,
            split_volume_size_bytes,
            options,
            raw_src,
            txn_ctx,
            |w| crate::archive::tar::write_tar_to_writer(source_paths, w, options),
        );
    }
    if resolved_format == TTZipArchiveFormat::Snappy {
        return write_snappy_target_in_txn(
            staged_dest,
            split_volume_size_bytes,
            raw_src,
            txn_ctx,
            |w| crate::archive::tar::write_tar_to_writer(source_paths, w, options),
        );
    }
    if matches!(
        resolved_format,
        TTZipArchiveFormat::TarZstd | TTZipArchiveFormat::Zstd
    ) {
        return write_zstd_target_in_txn(
            staged_dest,
            split_volume_size_bytes,
            options,
            raw_src,
            txn_ctx,
            |w| crate::archive::tar::write_tar_to_writer(source_paths, w, options),
        );
    }

    run_libarchive_staged_write_in_txn(
        staged_dest,
        resolved_format,
        options,
        is_raw_stream,
        split_volume_size_bytes,
        txn_ctx,
        |a| unsafe { write_archive_entries_to_handle_fmt(a, source_paths, options, resolved_format) },
    )
}

/// Recursively compresses pre-filtered explicit entries into the destination archive path with a
/// cancellation probe and optional 7z header encryption override, returning `CreateStagedCommitOutput`.
pub fn create_archive_explicit_with_files_detailed_opts_outcome_with_cancel<C>(
    entries: &[(PathBuf, String)],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    encrypt_file_names: Option<bool>,
    cancel_probe: C,
) -> Result<CreateStagedCommitOutput, CreateStageCommitError>
where
    C: FnOnce() -> bool,
{
    if entries.is_empty() {
        return Err(CreateStageCommitError::Status(TTZipStatus::ErrInvalidParam));
    }
    for (src_path, _) in entries {
        if !src_path.exists() && fs::symlink_metadata(src_path).is_err() {
            return Err(CreateStageCommitError::Status(TTZipStatus::ErrFileNotFound));
        }
    }

    let (resolved_format, commit_mode) = prepare_staged_create(
        destination_path,
        options,
        split_volume_size_bytes,
        true,
        encrypt_file_names,
        |fmt| should_use_raw_stream_explicit(entries, fmt),
    )?;
    with_staged_archive_destination_detailed_outcome(
        destination_path,
        commit_mode,
        |staged_dest, txn_ctx| {
            write_staged_explicit_archive_in_txn(
                entries,
                staged_dest,
                resolved_format,
                options,
                split_volume_size_bytes,
                encrypt_file_names,
                Some(txn_ctx),
            )
        },
        cancel_probe,
    )
}

/// Recursively compresses pre-filtered explicit entries into the destination archive path with optional
/// 7z header encryption override, returning `CreateStagedCommitOutput`.
pub fn create_archive_explicit_with_files_detailed_opts_outcome(
    entries: &[(PathBuf, String)],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    encrypt_file_names: Option<bool>,
) -> Result<CreateStagedCommitOutput, CreateStageCommitError> {
    create_archive_explicit_with_files_detailed_opts_outcome_with_cancel(
        entries,
        destination_path,
        options,
        split_volume_size_bytes,
        encrypt_file_names,
        || false,
    )
}

/// Recursively compresses pre-filtered explicit entries into the destination archive path with optional 7z header encryption override.
pub fn create_archive_explicit_with_files_detailed_opts(
    entries: &[(PathBuf, String)],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    encrypt_file_names: Option<bool>,
) -> Result<Vec<PathBuf>, CreateStageCommitError> {
    create_archive_explicit_with_files_detailed_opts_outcome(
        entries,
        destination_path,
        options,
        split_volume_size_bytes,
        encrypt_file_names,
    )
    .map(|out| out.committed_files)
}

/// Recursively compresses pre-filtered explicit entries into the destination archive path,
/// returning committed volume paths or detailed rollback/backup preservation diagnostics.
pub fn create_archive_explicit_with_files_detailed(
    entries: &[(PathBuf, String)],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> Result<Vec<PathBuf>, CreateStageCommitError> {
    create_archive_explicit_with_files_detailed_opts(
        entries,
        destination_path,
        options,
        split_volume_size_bytes,
        None,
    )
}

/// Recursively compresses pre-filtered explicit entries into the destination archive path and returns committed volume paths.
pub fn create_archive_explicit_with_files(
    entries: &[(PathBuf, String)],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> Result<Vec<PathBuf>, TTZipStatus> {
    create_archive_explicit_with_files_detailed(
        entries,
        destination_path,
        options,
        split_volume_size_bytes,
    )
    .map_err(CreateStageCommitError::log_and_into_status)
}

fn write_staged_explicit_special_container_in_txn(
    entries: &[(PathBuf, String)],
    staged_dest: &Path,
    resolved_format: TTZipArchiveFormat,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    encrypt_file_names: Option<bool>,
    txn_ctx: Option<&CreateTransactionContext>,
) -> Option<Result<(), TTZipStatus>> {
    match resolved_format {
        TTZipArchiveFormat::Zip | TTZipArchiveFormat::Auto => Some(
            crate::zip::writer::create_zip_streaming_parallel_explicit_split_in_txn(
                staged_dest,
                entries,
                options,
                split_volume_size_bytes,
                txn_ctx,
            )
            .map(|_| ()),
        ),
        TTZipArchiveFormat::SevenZip => Some(
            crate::sevenz::writer::create_7z_archive_explicit_split_with_header_encryption_in_txn(
                staged_dest,
                entries,
                options,
                split_volume_size_bytes,
                encrypt_file_names,
                txn_ctx,
            )
            .map(|_| ()),
        ),
        TTZipArchiveFormat::Aar => Some(super::format_config::write_apple_archive_entries_in_txn(
            entries,
            staged_dest,
            split_volume_size_bytes,
            txn_ctx,
        )),
        TTZipArchiveFormat::Xar => Some(super::format_config::write_xar_entries_in_txn(
            entries,
            staged_dest,
            options,
            split_volume_size_bytes,
            txn_ctx,
        )),
        _ => None,
    }
}

fn write_staged_explicit_archive_in_txn(
    entries: &[(PathBuf, String)],
    staged_dest: &Path,
    resolved_format: TTZipArchiveFormat,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    encrypt_file_names: Option<bool>,
    txn_ctx: Option<&CreateTransactionContext>,
) -> Result<(), TTZipStatus> {
    if let Some(res) = write_staged_explicit_special_container_in_txn(
        entries,
        staged_dest,
        resolved_format,
        options,
        split_volume_size_bytes,
        encrypt_file_names,
        txn_ctx,
    ) {
        return res;
    }

    let is_raw_stream = should_use_raw_stream_explicit(entries, resolved_format);
    let raw_src = is_raw_stream.then(|| entries[0].0.as_path());
    if matches!(
        resolved_format,
        TTZipArchiveFormat::TarBrotli | TTZipArchiveFormat::Brotli
    ) {
        return write_brotli_target_in_txn(
            staged_dest,
            split_volume_size_bytes,
            options,
            raw_src,
            txn_ctx,
            |w| crate::archive::tar::write_tar_entries_to_writer(entries, w, options),
        );
    }
    if resolved_format == TTZipArchiveFormat::Snappy {
        return write_snappy_target_in_txn(
            staged_dest,
            split_volume_size_bytes,
            raw_src,
            txn_ctx,
            |w| crate::archive::tar::write_tar_entries_to_writer(entries, w, options),
        );
    }
    if matches!(
        resolved_format,
        TTZipArchiveFormat::TarZstd | TTZipArchiveFormat::Zstd
    ) {
        return write_zstd_target_in_txn(
            staged_dest,
            split_volume_size_bytes,
            options,
            raw_src,
            txn_ctx,
            |w| crate::archive::tar::write_tar_entries_to_writer(entries, w, options),
        );
    }
    run_libarchive_staged_write_in_txn(
        staged_dest,
        resolved_format,
        options,
        is_raw_stream,
        split_volume_size_bytes,
        txn_ctx,
        |a| unsafe { write_entries_slice_to_handle_fmt(a, entries, options, resolved_format) },
    )
}

/// Recursively compresses pre-filtered explicit entries into the destination archive path.
pub fn create_archive_explicit(
    entries: &[(PathBuf, String)],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> Result<(), TTZipStatus> {
    create_archive_explicit_with_files(entries, destination_path, options, split_volume_size_bytes)
        .map(|_| ())
}
