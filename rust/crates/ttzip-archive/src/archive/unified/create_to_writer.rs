// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Streaming writer archive creation entrypoint (`create_archive_to_writer`) and libarchive entry helpers.

use std::ffi::CString;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use libc::{c_void, mode_t, time_t};

use super::create::{is_pure_single_stream_format, should_use_raw_stream};
use super::create_staging::CreateCommitMode;
use super::create_transaction::{
    ArchiveCleanupDisposition, CreateFileAccessMode, CreateTransactionContext,
    FileFinishSyncPolicy, ResidualPathRole, StageCleanupFailure,
};
use super::format_config::{configure_archive_write_format, validate_creatable_format};
use crate::archive::split::SplitVolumeWriter;
use crate::ffi::archive_ffi::guards::{ArchiveEntryGuard, ArchiveWriteGuard};
use crate::ffi::archive_ffi::sys::*;
use crate::types::{TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipStatus};

pub(crate) struct CountingStreamWriter<'a> {
    pub(crate) writer: &'a mut dyn Write,
    pub(crate) bytes_written: u64,
}

impl<'a> Write for CountingStreamWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.writer.write(buf)?;
        self.bytes_written = self.bytes_written.saturating_add(n as u64);
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

pub(crate) unsafe extern "C" fn writer_stream_write_cb(
    _a: *mut c_void,
    client_data: *mut c_void,
    buffer: *const c_void,
    length: libc::size_t,
) -> libc::ssize_t {
    unsafe {
        if client_data.is_null() || buffer.is_null() {
            return -1;
        }
        let ctx = &mut *(client_data as *mut CountingStreamWriter);
        let slice = std::slice::from_raw_parts(buffer as *const u8, length);
        match ctx.writer.write_all(slice) {
            Ok(()) => {
                ctx.bytes_written = ctx.bytes_written.saturating_add(length as u64);
                length as libc::ssize_t
            }
            Err(_) => -1,
        }
    }
}

fn write_brotli_to_counting(
    source_paths: &[PathBuf],
    counting: &mut CountingStreamWriter<'_>,
    options: &TTZipCreateOptions,
    is_raw_stream: bool,
) -> Result<u64, TTZipStatus> {
    let q = match options.level {
        TTZipCompressionLevel::Store => 0,
        TTZipCompressionLevel::Fastest => 1,
        TTZipCompressionLevel::Fast => 3,
        TTZipCompressionLevel::Normal => 6,
        TTZipCompressionLevel::Maximum => 9,
        TTZipCompressionLevel::Ultra => 11,
    };
    {
        let mut bw = brotli::CompressorWriter::new(&mut *counting, 64 * 1024, q, 22);
        if is_raw_stream {
            let mut src = File::open(&source_paths[0]).map_err(|_| TTZipStatus::ErrFileNotFound)?;
            std::io::copy(&mut src, &mut bw).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        } else {
            crate::archive::tar::write_tar_to_writer(source_paths, bw, options)?;
        }
    }
    counting
        .flush()
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    Ok(counting.bytes_written)
}

fn write_zstd_to_counting(
    source_paths: &[PathBuf],
    counting: &mut CountingStreamWriter<'_>,
    options: &TTZipCreateOptions,
    is_raw_stream: bool,
) -> Result<u64, TTZipStatus> {
    let z_lvl = match options.level {
        TTZipCompressionLevel::Store | TTZipCompressionLevel::Fastest => 1,
        TTZipCompressionLevel::Fast => 3,
        TTZipCompressionLevel::Normal => 6,
        TTZipCompressionLevel::Maximum => 19,
        TTZipCompressionLevel::Ultra => 22,
    };
    let nb_workers = if options.thread_budget > 0 {
        options.thread_budget
    } else {
        std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(4)
    };
    let config = crate::codecs::zstd::types::ZstdConfig {
        level: z_lvl,
        nb_workers,
        enable_checksum: true,
        ..Default::default()
    };
    {
        let mut zw = crate::codecs::zstd::ZstdStreamWriter::new(&mut *counting, &config)?;
        if is_raw_stream {
            let mut src = File::open(&source_paths[0]).map_err(|_| TTZipStatus::ErrFileNotFound)?;
            std::io::copy(&mut src, &mut zw).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        } else {
            zw = crate::archive::tar::write_tar_to_writer(source_paths, zw, options)?;
        }
        zw.finish()?;
    }
    counting
        .flush()
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    Ok(counting.bytes_written)
}

fn write_snappy_to_counting(
    source_paths: &[PathBuf],
    counting: &mut CountingStreamWriter<'_>,
    options: &TTZipCreateOptions,
    is_raw_stream: bool,
) -> Result<u64, TTZipStatus> {
    {
        let mut se = snap::write::FrameEncoder::new(&mut *counting);
        if is_raw_stream {
            let mut src = File::open(&source_paths[0]).map_err(|_| TTZipStatus::ErrFileNotFound)?;
            std::io::copy(&mut src, &mut se).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        } else {
            se = crate::archive::tar::write_tar_to_writer(source_paths, se, options)?;
        }
        se.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    }
    counting
        .flush()
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    Ok(counting.bytes_written)
}

fn write_libarchive_to_counting(
    source_paths: &[PathBuf],
    counting: &mut CountingStreamWriter<'_>,
    resolved_format: TTZipArchiveFormat,
    options: &TTZipCreateOptions,
    is_raw_stream: bool,
) -> Result<u64, TTZipStatus> {
    unsafe {
        let a = archive_write_new();
        if a.is_null() {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let guard = ArchiveWriteGuard(a);
        configure_archive_write_format(a, resolved_format, options, is_raw_stream)?;
        let open_rc = archive_write_open2(
            a,
            counting as *mut _ as *mut c_void,
            None,
            Some(writer_stream_write_cb),
            None,
            None,
        );
        if open_rc != 0 {
            return Err(TTZipStatus::ErrOpenFailed);
        }
        write_archive_entries_to_handle_fmt(a, source_paths, options, resolved_format)?;
        guard
            .close_and_free()
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    }
    counting
        .flush()
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    Ok(counting.bytes_written)
}

/// Recursively compresses source paths into a streaming writer (0% disk write amplification).
pub fn create_archive_to_writer<W: Write>(
    source_paths: &[PathBuf],
    mut writer: W,
    options: &TTZipCreateOptions,
) -> Result<u64, TTZipStatus> {
    if source_paths.is_empty() {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let resolved_format = if options.format == TTZipArchiveFormat::Auto {
        TTZipArchiveFormat::Zip
    } else {
        options.format
    };
    validate_creatable_format(resolved_format, false)?;
    let is_raw_stream = should_use_raw_stream(source_paths, resolved_format);
    if is_pure_single_stream_format(resolved_format) && !is_raw_stream {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let mut counting = CountingStreamWriter {
        writer: &mut writer,
        bytes_written: 0,
    };
    if matches!(
        resolved_format,
        TTZipArchiveFormat::TarBrotli | TTZipArchiveFormat::Brotli
    ) {
        return write_brotli_to_counting(source_paths, &mut counting, options, is_raw_stream);
    }
    if resolved_format == TTZipArchiveFormat::Snappy {
        return write_snappy_to_counting(source_paths, &mut counting, options, is_raw_stream);
    }
    if matches!(
        resolved_format,
        TTZipArchiveFormat::TarZstd | TTZipArchiveFormat::Zstd
    ) {
        return write_zstd_to_counting(source_paths, &mut counting, options, is_raw_stream);
    }
    if resolved_format == TTZipArchiveFormat::Tar {
        crate::archive::tar::write_tar_to_writer(source_paths, &mut counting, options)?;
        counting
            .flush()
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        return Ok(counting.bytes_written);
    }
    write_libarchive_to_counting(
        source_paths,
        &mut counting,
        resolved_format,
        options,
        is_raw_stream,
    )
}

pub(crate) fn collect_source_entries(
    source_paths: &[PathBuf],
) -> Result<Vec<(PathBuf, String)>, TTZipStatus> {
    let mut entries_to_write = Vec::new();
    for src_path in source_paths {
        let base_parent = src_path.parent().unwrap_or(src_path);
        collect_entries_recursive(base_parent, src_path, &mut entries_to_write)
            .map_err(|_| TTZipStatus::ErrFileNotFound)?;
    }
    Ok(entries_to_write)
}

pub(crate) unsafe fn write_archive_entries_to_handle_fmt(
    a: *mut c_void,
    source_paths: &[PathBuf],
    options: &TTZipCreateOptions,
    format: TTZipArchiveFormat,
) -> Result<(), TTZipStatus> {
    for src_path in source_paths {
        if !src_path.exists() && fs::symlink_metadata(src_path).is_err() {
            return Err(TTZipStatus::ErrFileNotFound);
        }
    }
    let entries_to_write = collect_source_entries(source_paths)?;
    unsafe { write_entries_slice_to_handle_fmt(a, &entries_to_write, options, format) }
}

unsafe fn stream_regular_file_to_libarchive(
    a: *mut c_void,
    abs_path: &Path,
    rel_c: &CString,
    options: &TTZipCreateOptions,
    buf: &mut [u8],
    processed_bytes: &mut u64,
) -> Result<(), TTZipStatus> {
    let mut file = File::open(abs_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    loop {
        let n = file
            .read(buf)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        if n == 0 {
            break;
        }
        if unsafe { archive_write_data(a, buf.as_ptr() as *const c_void, n) } < 0 {
            return Err(TTZipStatus::ErrCompressionFailed);
        }
        *processed_bytes = processed_bytes.saturating_add(n as u64);
        if let Some(cb) = options.progress_callback {
            if !unsafe { cb(*processed_bytes, *processed_bytes, rel_c.as_ptr(), options.user_data) }
            {
                return Err(TTZipStatus::Cancelled);
            }
        }
    }
    Ok(())
}

unsafe fn write_libarchive_typed_entry(
    a: *mut c_void,
    entry: *mut c_void,
    abs_path: &Path,
    meta: &fs::Metadata,
    rel_c: &CString,
    options: &TTZipCreateOptions,
    buf: &mut [u8],
    processed_bytes: &mut u64,
) -> Result<(), TTZipStatus> {
    unsafe {
        let filetype = meta.file_type();
        if filetype.is_symlink() {
            archive_entry_set_filetype(entry, libc::S_IFLNK as u32);
            archive_entry_set_perm(entry, 0o777 as mode_t);
            archive_entry_set_size(entry, 0);
            if let Ok(target) = fs::read_link(abs_path) {
                if let Ok(target_c) = CString::new(target.to_string_lossy().as_bytes()) {
                    archive_entry_set_symlink(entry, target_c.as_ptr());
                }
            }
            if archive_write_header(a, entry) != 0 {
                return Err(TTZipStatus::ErrCompressionFailed);
            }
        } else if filetype.is_dir() {
            archive_entry_set_filetype(entry, libc::S_IFDIR as u32);
            archive_entry_set_perm(entry, (meta.permissions().mode() & 0o777) as mode_t);
            archive_entry_set_size(entry, 0);
            if archive_write_header(a, entry) != 0 {
                return Err(TTZipStatus::ErrCompressionFailed);
            }
        } else {
            archive_entry_set_filetype(entry, libc::S_IFREG as u32);
            archive_entry_set_perm(entry, (meta.permissions().mode() & 0o777) as mode_t);
            archive_entry_set_size(entry, meta.len() as i64);
            if archive_write_header(a, entry) != 0 {
                return Err(TTZipStatus::ErrCompressionFailed);
            }
            stream_regular_file_to_libarchive(a, abs_path, rel_c, options, buf, processed_bytes)?;
        }
        archive_write_finish_entry(a);
        Ok(())
    }
}

pub(crate) unsafe fn write_entries_slice_to_handle_fmt(
    a: *mut c_void,
    entries_to_write: &[(PathBuf, String)],
    options: &TTZipCreateOptions,
    format: TTZipArchiveFormat,
) -> Result<(), TTZipStatus> {
    unsafe {
        libc::setlocale(libc::LC_ALL, c"en_US.UTF-8".as_ptr());
        let mut processed_bytes: u64 = 0;
        let mut buf = vec![0u8; 64 * 1024];

        for (abs_path, rel_name) in entries_to_write {
            let Ok(meta) = fs::symlink_metadata(abs_path) else {
                continue;
            };
            if format == TTZipArchiveFormat::Ar && (meta.is_dir() || meta.file_type().is_symlink())
            {
                return Err(TTZipStatus::ErrInvalidParam);
            }

            let entry = archive_entry_new();
            if entry.is_null() {
                return Err(TTZipStatus::ErrOutOfMemory);
            }
            let _entry_guard = ArchiveEntryGuard(entry);

            let entry_path_str = if format == TTZipArchiveFormat::Ar {
                Path::new(rel_name.as_str())
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(rel_name.as_str())
            } else {
                rel_name.as_str()
            };
            let Ok(rel_c) = CString::new(entry_path_str) else {
                continue;
            };

            archive_entry_set_pathname(entry, rel_c.as_ptr());
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs() as time_t);
            archive_entry_set_mtime(entry, mtime, 0);
            write_libarchive_typed_entry(
                a,
                entry,
                abs_path,
                &meta,
                &rel_c,
                options,
                &mut buf,
                &mut processed_bytes,
            )?;

            if let Some(cb) = options.progress_callback {
                if !cb(
                    processed_bytes,
                    processed_bytes,
                    rel_c.as_ptr(),
                    options.user_data,
                ) {
                    return Err(TTZipStatus::Cancelled);
                }
            }
        }

        Ok(())
    }
}

fn collect_entries_recursive(
    root: &Path,
    start: &Path,
    out: &mut Vec<(PathBuf, String)>,
) -> std::io::Result<()> {
    let mut stack: Vec<PathBuf> = vec![start.to_path_buf()];

    while let Some(current) = stack.pop() {
        let rel_prefix = current.strip_prefix(root).unwrap_or(&current);
        let rel_str = rel_prefix.to_string_lossy().to_string();

        if !rel_str.is_empty() {
            out.push((current.clone(), rel_str));
        }

        if let Ok(meta) = fs::symlink_metadata(&current) {
            if meta.is_dir() && !meta.file_type().is_symlink() {
                let mut children = Vec::new();
                for entry in fs::read_dir(&current)? {
                    children.push(entry?.path());
                }
                children.sort_unstable_by(|a, b| b.cmp(a));
                stack.extend(children);
            }
        }
    }
    Ok(())
}

unsafe extern "C" fn split_write_cb(
    _a: *mut c_void,
    client_data: *mut c_void,
    buffer: *const c_void,
    length: libc::size_t,
) -> libc::ssize_t {
    unsafe {
        if client_data.is_null() || buffer.is_null() {
            return -1;
        }
        let writer = &mut *(client_data as *mut SplitVolumeWriter);
        let slice = std::slice::from_raw_parts(buffer as *const u8, length);
        match writer.write_all(slice) {
            Ok(()) => length as libc::ssize_t,
            Err(_) => -1,
        }
    }
}

fn record_preexisting_occupant_refusal(
    staged_dest: &Path,
    status: TTZipStatus,
    txn_ctx: Option<&CreateTransactionContext>,
) {
    let err = std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!(
            "Refused to delete '{}': untracked or pre-existing file occupies staged path",
            staged_dest.display()
        ),
    );
    super::create_staging::record_inner_stage_cleanup_error_with_status(staged_dest, status, &err);
    if let Some(ctx) = txn_ctx {
        ctx.record_inner_failure(StageCleanupFailure {
            role: ResidualPathRole::UnownedPathOccupant,
            disposition: ArchiveCleanupDisposition::RefusedOwnershipMismatch,
            original_path: staged_dest.to_path_buf(),
            final_known_path: Some(staged_dest.to_path_buf()),
            expected_identity: None,
            observed_identity: None,
            os_error_code: Some(libc::EEXIST),
            detail: err.to_string(),
        });
    }
}

fn run_libarchive_single_file_in_txn<F>(
    a: *mut c_void,
    guard: ArchiveWriteGuard,
    staged_dest: &Path,
    txn_ctx: Option<&CreateTransactionContext>,
    write_fn: F,
) -> Result<(), TTZipStatus>
where
    F: FnOnce(*mut c_void) -> Result<(), TTZipStatus>,
{
    let (file, file_guard) = match crate::fs::FileAbortGuard::create_staged_file_in_txn(
        staged_dest,
        CreateFileAccessMode::WriteOnly,
        txn_ctx,
        ResidualPathRole::StagedArtifact,
    ) {
        Ok(pair) => pair,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                record_preexisting_occupant_refusal(staged_dest, TTZipStatus::ErrOpenFailed, None);
            }
            return Err(TTZipStatus::ErrOpenFailed);
        }
    };

    let prior_res: Result<(), (TTZipStatus, String)> = {
        use std::os::fd::AsRawFd;
        let open_rc = unsafe { archive_write_open_fd(a, file.as_raw_fd()) };
        if open_rc != 0 {
            let _ = guard.close_and_free();
            Err((
                TTZipStatus::ErrOpenFailed,
                "archive_write_open_fd failed".to_string(),
            ))
        } else {
            let write_res = write_fn(a);
            let close_arch_res = guard.close_and_free();
            match (write_res, close_arch_res) {
                (Err(st), _) => Err((st, format!("libarchive write_fn failed: {st:?}"))),
                (Ok(()), Err(rc)) => Err((
                    TTZipStatus::ErrCompressionFailed,
                    format!("archive_write_close failed with rc={rc}"),
                )),
                (Ok(()), Ok(())) => Ok(()),
            }
        }
    };

    let has_ctx = file_guard.txn_ctx().is_some();
    match file_guard.finish_file_and_settle_in_txn(
        file,
        prior_res,
        None,
        FileFinishSyncPolicy::None,
    ) {
        Ok(_proof) => Ok(()),
        Err(finish_err) => {
            if !has_ctx {
                if let Some((_, p, msg)) = super::take_last_inner_stage_cleanup_record() {
                    let synth = std::io::Error::other(msg);
                    super::create_staging::record_inner_stage_cleanup_error_with_status(
                        p,
                        finish_err.primary_status,
                        &synth,
                    );
                    return Err(TTZipStatus::ErrCompressionFailed);
                }
            }
            Err(finish_err.primary_status)
        }
    }
}

fn run_libarchive_split_in_txn<F>(
    a: *mut c_void,
    guard: ArchiveWriteGuard,
    staged_dest: &Path,
    resolved_format: TTZipArchiveFormat,
    split_volume_size_bytes: u64,
    txn_ctx: Option<&CreateTransactionContext>,
    write_fn: F,
) -> Result<(), TTZipStatus>
where
    F: FnOnce(*mut c_void) -> Result<(), TTZipStatus>,
{
    let (base_path, scheme) = CreateCommitMode::resolve_split_target(staged_dest, resolved_format);
    let mut split_writer =
        SplitVolumeWriter::new_in_txn(&base_path, split_volume_size_bytes, scheme, txn_ctx)
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let open_rc = unsafe {
        archive_write_open2(
            a,
            &mut split_writer as *mut _ as *mut c_void,
            None,
            Some(split_write_cb),
            None,
            None,
        )
    };
    if open_rc != 0 {
        let _ = guard.close_and_free();
        let _ = split_writer.cancel_and_cleanup_in_txn(TTZipStatus::ErrOpenFailed);
        return Err(TTZipStatus::ErrOpenFailed);
    }
    let write_res = write_fn(a);
    let close_arch_res = guard.close_and_free();
    if let Err(st) = write_res {
        if split_writer.cancel_and_cleanup_in_txn(st).is_err() && txn_ctx.is_none() {
            return Err(TTZipStatus::ErrCompressionFailed);
        }
        return Err(st);
    }
    if close_arch_res.is_err() {
        let _ = split_writer.cancel_and_cleanup_in_txn(TTZipStatus::ErrCompressionFailed);
        return Err(TTZipStatus::ErrCompressionFailed);
    }
    split_writer
        .close_in_txn()
        .map(|_| ())
        .map_err(|_| TTZipStatus::ErrCompressionFailed)
}

/// Executes a staged libarchive write transaction bound to `CreateTransactionContext`
/// with explicit single-close verification on every exit path.
pub fn run_libarchive_staged_write_in_txn<F>(
    staged_dest: &Path,
    resolved_format: TTZipArchiveFormat,
    options: &TTZipCreateOptions,
    is_raw_stream: bool,
    split_volume_size_bytes: u64,
    txn_ctx: Option<&CreateTransactionContext>,
    write_fn: F,
) -> Result<(), TTZipStatus>
where
    F: FnOnce(*mut c_void) -> Result<(), TTZipStatus>,
{
    let preexisting = split_volume_size_bytes == 0 && fs::symlink_metadata(staged_dest).is_ok();
    let a = unsafe { archive_write_new() };
    if a.is_null() {
        if preexisting {
            record_preexisting_occupant_refusal(staged_dest, TTZipStatus::ErrOutOfMemory, txn_ctx);
        }
        return Err(TTZipStatus::ErrOutOfMemory);
    }
    let guard = ArchiveWriteGuard(a);
    if let Err(cfg_err) =
        unsafe { configure_archive_write_format(a, resolved_format, options, is_raw_stream) }
    {
        if preexisting {
            record_preexisting_occupant_refusal(staged_dest, cfg_err, txn_ctx);
        }
        return Err(cfg_err);
    }
    if split_volume_size_bytes == 0 {
        run_libarchive_single_file_in_txn(a, guard, staged_dest, txn_ctx, write_fn)
    } else {
        run_libarchive_split_in_txn(
            a,
            guard,
            staged_dest,
            resolved_format,
            split_volume_size_bytes,
            txn_ctx,
            write_fn,
        )
    }
}

/// Executes a staged libarchive write transaction with explicit finalization on every exit path.
pub fn run_libarchive_staged_write<F>(
    staged_dest: &Path,
    resolved_format: TTZipArchiveFormat,
    options: &TTZipCreateOptions,
    is_raw_stream: bool,
    split_volume_size_bytes: u64,
    write_fn: F,
) -> Result<(), TTZipStatus>
where
    F: FnOnce(*mut c_void) -> Result<(), TTZipStatus>,
{
    run_libarchive_staged_write_in_txn(
        staged_dest,
        resolved_format,
        options,
        is_raw_stream,
        split_volume_size_bytes,
        None,
        write_fn,
    )
}
