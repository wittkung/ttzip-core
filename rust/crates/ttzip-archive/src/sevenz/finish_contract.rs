// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! 7-Zip header encryption, signature header finalization, and transaction-bound
//! streaming archive creation entrypoints.

use crate::archive::unified::create_transaction::CreateTransactionContext;
use crate::crypto::crc32::crc32_fast;
use crate::sevenz::format::{build_7z_encoded_header_boot, build_sig_header_bytes, METHOD_LZMA2};
use crate::types::{TTZipCreateOptions, TTZipStatus};
use crate::zip::writer::ZipCreateReport;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::writer::{
    feed_fl2_slice, finish_fl2_stream, init_fl2_stream,
    write_7z_archive_streaming_from_disk_in_txn, SevenZDiskEntry, SevenZPayloadSink,
};

pub(crate) fn stat_disk_entry(
    disk_path: &Path,
    rel_path: String,
) -> Result<SevenZDiskEntry, TTZipStatus> {
    let meta = fs::symlink_metadata(disk_path)
        .or_else(|_| fs::metadata(disk_path))
        .map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let is_symlink = meta.file_type().is_symlink();
    let is_dir = !is_symlink && meta.is_dir();
    let mtime = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
    #[cfg(unix)]
    let posix_mode = {
        use std::os::unix::fs::MetadataExt;
        meta.mode()
    };
    #[cfg(not(unix))]
    let posix_mode = if is_symlink {
        0o120777
    } else if is_dir {
        0o040755
    } else if meta.permissions().readonly() {
        0o100444
    } else {
        0o100644
    };

    if is_symlink {
        let target_path = fs::read_link(disk_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
        let target_bytes = target_path.to_string_lossy().as_bytes().to_vec();
        let size = target_bytes.len() as u64;
        return Ok(SevenZDiskEntry {
            disk_path: None,
            symlink_target: Some(target_bytes),
            rel_path,
            is_directory: false,
            is_symlink: true,
            posix_mode: (posix_mode & 0o7777) | 0o120000,
            mtime,
            size,
            is_empty_stream: size == 0,
        });
    }

    let size = if is_dir { 0 } else { meta.len() };
    Ok(SevenZDiskEntry {
        disk_path: if is_dir {
            None
        } else {
            Some(disk_path.to_path_buf())
        },
        symlink_target: None,
        rel_path,
        is_directory: is_dir,
        is_symlink: false,
        posix_mode,
        mtime,
        size,
        is_empty_stream: is_dir || size == 0,
    })
}

pub(crate) fn collect_7z_disk_entries(
    base_src: &Path,
    rel_prefix: &str,
    out_entries: &mut Vec<SevenZDiskEntry>,
) -> Result<(), TTZipStatus> {
    let mut stack = vec![(base_src.to_path_buf(), rel_prefix.to_string())];
    while let Some((curr_path, curr_rel)) = stack.pop() {
        let mut entry = stat_disk_entry(&curr_path, curr_rel.clone())?;
        if entry.is_directory && !entry.rel_path.is_empty() && !entry.rel_path.ends_with('/') {
            entry.rel_path.push('/');
        }
        let is_dir = entry.is_directory;
        if !entry.rel_path.is_empty() {
            out_entries.push(entry);
        }
        if is_dir {
            let dir_iter = fs::read_dir(&curr_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
            let mut children = Vec::new();
            for item in dir_iter.flatten() {
                let path = item.path();
                let fname = path.file_name().unwrap_or_default().to_string_lossy();
                let child_rel = if curr_rel.is_empty() {
                    fname.to_string()
                } else {
                    format!("{}/{}", curr_rel.trim_end_matches('/'), fname)
                };
                children.push((path, child_rel));
            }
            children.reverse();
            stack.extend(children);
        }
    }
    Ok(())
}

pub(crate) fn encode_7z_header_with_aes(
    plain_header: &[u8],
    payload_compressed_len: u64,
    password: &str,
) -> Result<(Vec<u8>, Vec<u8>), TTZipStatus> {
    if password.is_empty() {
        return Err(TTZipStatus::ErrInvalidPassword);
    }
    let mut encrypted_header_payload = Vec::with_capacity(plain_header.len() + 64);
    let mut hdr_sink = SevenZPayloadSink::new(&mut encrypted_header_payload, Some(password));
    let mut cstream = init_fl2_stream(6, 1)?;
    let mut out_chunk = vec![0u8; 64 * 1024];
    feed_fl2_slice(&mut cstream, plain_header, &mut out_chunk, |comp| {
        hdr_sink.write_chunk(comp)
    })?;
    let dict_prop =
        finish_fl2_stream(&mut cstream, &mut out_chunk, |comp| hdr_sink.write_chunk(comp))?;
    let (enc_hdr_len, hdr_aes_cfg) = hdr_sink.finish()?;

    let boot = build_7z_encoded_header_boot(
        payload_compressed_len,
        enc_hdr_len,
        plain_header.len() as u64,
        crc32_fast(0, plain_header),
        METHOD_LZMA2,
        &[dict_prop],
        hdr_aes_cfg.as_ref(),
    );
    Ok((encrypted_header_payload, boot))
}

pub(crate) fn finalize_7z_header_and_signature(
    mut writer: crate::archive::split::SevenZOutputTarget,
    plain_header_bytes: &[u8],
    compressed_len: u64,
    use_encoded_header: bool,
    password: Option<&str>,
) -> Result<u64, TTZipStatus> {
    if use_encoded_header {
        let pass = password.unwrap_or_default();
        let (enc_hdr_payload, boot_bytes) =
            match encode_7z_header_with_aes(plain_header_bytes, compressed_len, pass) {
                Ok(v) => v,
                Err(status) => {
                    writer.abort_in_txn(status);
                    return Err(status);
                }
            };
        if writer.write_all(&enc_hdr_payload).is_err() || writer.write_all(&boot_bytes).is_err() {
            writer.abort_in_txn(TTZipStatus::ErrCompressionFailed);
            return Err(TTZipStatus::ErrCompressionFailed);
        }
        let next_hdr_off = compressed_len + (enc_hdr_payload.len() as u64);
        let sig_bytes = build_sig_header_bytes(next_hdr_off, &boot_bytes);
        writer.finalize_signature_in_txn(&sig_bytes)?;
        Ok(32 + next_hdr_off + (boot_bytes.len() as u64))
    } else {
        if writer.write_all(plain_header_bytes).is_err() {
            writer.abort_in_txn(TTZipStatus::ErrCompressionFailed);
            return Err(TTZipStatus::ErrCompressionFailed);
        }
        let sig_bytes = build_sig_header_bytes(compressed_len, plain_header_bytes);
        writer.finalize_signature_in_txn(&sig_bytes)?;
        Ok(32 + compressed_len + (plain_header_bytes.len() as u64))
    }
}

/// Creates a 7z archive file (single or split-volume) bound to an optional `CreateTransactionContext`
/// directly from input source paths with explicit header-encryption control.
pub fn create_7z_archive_split_with_header_encryption_in_txn(
    dest_path: &Path,
    source_paths: &[PathBuf],
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    encrypt_file_names: Option<bool>,
    txn_ctx: Option<&CreateTransactionContext>,
) -> Result<ZipCreateReport, TTZipStatus> {
    let start_time = Instant::now();
    let mut disk_entries = Vec::new();
    for src in source_paths {
        if !src.exists() && fs::symlink_metadata(src).is_err() {
            return Err(TTZipStatus::ErrFileNotFound);
        }
        let file_name = src.file_name().unwrap_or_default().to_string_lossy();
        collect_7z_disk_entries(src, &file_name, &mut disk_entries)?;
    }
    write_7z_archive_streaming_from_disk_in_txn(
        dest_path,
        disk_entries,
        options,
        split_volume_size_bytes,
        encrypt_file_names,
        None,
        start_time,
        txn_ctx,
    )
}

/// Creates a 7z archive file (single or split-volume) bound to an optional `CreateTransactionContext`.
pub fn create_7z_archive_split_in_txn(
    dest_path: &Path,
    source_paths: &[PathBuf],
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    txn_ctx: Option<&CreateTransactionContext>,
) -> Result<ZipCreateReport, TTZipStatus> {
    create_7z_archive_split_with_header_encryption_in_txn(
        dest_path,
        source_paths,
        options,
        split_volume_size_bytes,
        None,
        txn_ctx,
    )
}

/// Creates a 7z archive file (single or split-volume) from explicit pre-collected entries bound
/// to an optional `CreateTransactionContext` with explicit header-encryption control.
pub fn create_7z_archive_explicit_split_with_header_encryption_in_txn(
    dest_path: &Path,
    entries: &[(PathBuf, String)],
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    encrypt_file_names: Option<bool>,
    txn_ctx: Option<&CreateTransactionContext>,
) -> Result<ZipCreateReport, TTZipStatus> {
    let start_time = Instant::now();
    let mut disk_entries = Vec::with_capacity(entries.len());
    for (disk_path, rel_path) in entries {
        disk_entries.push(stat_disk_entry(disk_path, rel_path.clone())?);
    }
    write_7z_archive_streaming_from_disk_in_txn(
        dest_path,
        disk_entries,
        options,
        split_volume_size_bytes,
        encrypt_file_names,
        None,
        start_time,
        txn_ctx,
    )
}
