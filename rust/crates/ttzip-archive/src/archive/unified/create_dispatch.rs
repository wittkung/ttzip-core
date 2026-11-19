// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Staged format and container write dispatchers for unified archive creation.

use std::path::{Path, PathBuf};

use super::create::{should_use_raw_stream, should_use_raw_stream_explicit};
use super::create_to_writer::{
    collect_source_entries, run_libarchive_staged_write_in_txn,
    write_archive_entries_to_handle_fmt, write_entries_slice_to_handle_fmt,
};
use super::create_transaction::CreateTransactionContext;
use super::format_config::{
    write_apple_archive_entries_in_txn, write_brotli_target_in_txn, write_gzip_target_in_txn,
    write_snappy_target_in_txn, write_xar_entries_in_txn, write_zstd_target_in_txn,
};
use crate::types::{TTZipArchiveFormat, TTZipCreateOptions, TTZipStatus};

pub(super) fn write_staged_special_container_in_txn(
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
                write_apple_archive_entries_in_txn(
                    &entries,
                    staged_dest,
                    split_volume_size_bytes,
                    txn_ctx,
                )
            }))
        }
        TTZipArchiveFormat::Xar => {
            Some(collect_source_entries(source_paths).and_then(|entries| {
                write_xar_entries_in_txn(
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

pub(super) fn try_write_staged_source_native_stream_in_txn(
    source_paths: &[PathBuf],
    staged_dest: &Path,
    resolved_format: TTZipArchiveFormat,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    raw_src: Option<&Path>,
    txn_ctx: Option<&CreateTransactionContext>,
) -> Option<Result<(), TTZipStatus>> {
    match resolved_format {
        TTZipArchiveFormat::TarBrotli | TTZipArchiveFormat::Brotli => {
            Some(write_brotli_target_in_txn(
                staged_dest,
                split_volume_size_bytes,
                options,
                raw_src,
                txn_ctx,
                |w| crate::archive::tar::write_tar_to_writer(source_paths, w, options),
            ))
        }
        TTZipArchiveFormat::Snappy => Some(write_snappy_target_in_txn(
            staged_dest,
            split_volume_size_bytes,
            raw_src,
            txn_ctx,
            |w| crate::archive::tar::write_tar_to_writer(source_paths, w, options),
        )),
        TTZipArchiveFormat::TarZstd | TTZipArchiveFormat::Zstd => Some(write_zstd_target_in_txn(
            staged_dest,
            split_volume_size_bytes,
            options,
            raw_src,
            txn_ctx,
            |w| crate::archive::tar::write_tar_to_writer(source_paths, w, options),
        )),
        TTZipArchiveFormat::TarGz | TTZipArchiveFormat::Gzip => Some(write_gzip_target_in_txn(
            staged_dest,
            split_volume_size_bytes,
            options,
            raw_src,
            txn_ctx,
            |w| crate::archive::tar::write_tar_to_writer(source_paths, w, options),
        )),
        _ => None,
    }
}

pub(super) fn write_staged_source_archive_in_txn(
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
    if let Some(res) = try_write_staged_source_native_stream_in_txn(
        source_paths,
        staged_dest,
        resolved_format,
        options,
        split_volume_size_bytes,
        raw_src,
        txn_ctx,
    ) {
        return res;
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

pub(super) fn write_staged_explicit_special_container_in_txn(
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
        TTZipArchiveFormat::Aar => Some(write_apple_archive_entries_in_txn(
            entries,
            staged_dest,
            split_volume_size_bytes,
            txn_ctx,
        )),
        TTZipArchiveFormat::Xar => Some(write_xar_entries_in_txn(
            entries,
            staged_dest,
            options,
            split_volume_size_bytes,
            txn_ctx,
        )),
        _ => None,
    }
}

pub(super) fn try_write_staged_explicit_native_stream_in_txn(
    entries: &[(PathBuf, String)],
    staged_dest: &Path,
    resolved_format: TTZipArchiveFormat,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    raw_src: Option<&Path>,
    txn_ctx: Option<&CreateTransactionContext>,
) -> Option<Result<(), TTZipStatus>> {
    match resolved_format {
        TTZipArchiveFormat::TarBrotli | TTZipArchiveFormat::Brotli => {
            Some(write_brotli_target_in_txn(
                staged_dest,
                split_volume_size_bytes,
                options,
                raw_src,
                txn_ctx,
                |w| crate::archive::tar::write_tar_entries_to_writer(entries, w, options),
            ))
        }
        TTZipArchiveFormat::Snappy => Some(write_snappy_target_in_txn(
            staged_dest,
            split_volume_size_bytes,
            raw_src,
            txn_ctx,
            |w| crate::archive::tar::write_tar_entries_to_writer(entries, w, options),
        )),
        TTZipArchiveFormat::TarZstd | TTZipArchiveFormat::Zstd => Some(write_zstd_target_in_txn(
            staged_dest,
            split_volume_size_bytes,
            options,
            raw_src,
            txn_ctx,
            |w| crate::archive::tar::write_tar_entries_to_writer(entries, w, options),
        )),
        TTZipArchiveFormat::TarGz | TTZipArchiveFormat::Gzip => Some(write_gzip_target_in_txn(
            staged_dest,
            split_volume_size_bytes,
            options,
            raw_src,
            txn_ctx,
            |w| crate::archive::tar::write_tar_entries_to_writer(entries, w, options),
        )),
        _ => None,
    }
}

pub(super) fn write_staged_explicit_archive_in_txn(
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
    if let Some(res) = try_write_staged_explicit_native_stream_in_txn(
        entries,
        staged_dest,
        resolved_format,
        options,
        split_volume_size_bytes,
        raw_src,
        txn_ctx,
    ) {
        return res;
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
