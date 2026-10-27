// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Archive creation lifecycle submodule for Unified Orchestrator.

use std::ffi::CString;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::archive::split::SplitVolumeWriter;
use super::format_config::{
    configure_archive_write_format, write_brotli_target, write_snappy_target, write_zstd_target,
};
use crate::ffi::archive_ffi::guards::{ArchiveEntryGuard, ArchiveWriteGuard};
use crate::ffi::archive_ffi::sys::*;
use crate::types::{
    TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipStatus,
};
use libc::{c_void, mode_t, time_t};

unsafe extern "C" fn split_write_cb(
    _a: *mut c_void,
    client_data: *mut c_void,
    buffer: *const c_void,
    length: libc::size_t,
) -> libc::ssize_t {
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

unsafe extern "C" fn split_close_cb(
    _a: *mut c_void,
    client_data: *mut c_void,
) -> libc::c_int {
    if client_data.is_null() {
        return 0;
    }
    let writer = &mut *(client_data as *mut SplitVolumeWriter);
    match writer.close() {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

unsafe extern "C" fn split_free_cb(
    _a: *mut c_void,
    client_data: *mut c_void,
) -> libc::c_int {
    if !client_data.is_null() {
        drop(Box::from_raw(client_data as *mut SplitVolumeWriter));
    }
    0
}

#[inline]
fn is_pure_single_stream_format(fmt: TTZipArchiveFormat) -> bool {
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
fn should_use_raw_stream(source_paths: &[PathBuf], resolved_format: TTZipArchiveFormat) -> bool {
    source_paths.len() == 1
        && fs::symlink_metadata(&source_paths[0])
            .map(|m| m.is_file() && !m.file_type().is_symlink())
            .unwrap_or(false)
        && is_pure_single_stream_format(resolved_format)
}

#[inline]
fn should_use_raw_stream_explicit(entries: &[(PathBuf, String)], resolved_format: TTZipArchiveFormat) -> bool {
    entries.len() == 1
        && fs::symlink_metadata(&entries[0].0)
            .map(|m| m.is_file() && !m.file_type().is_symlink())
            .unwrap_or(false)
        && is_pure_single_stream_format(resolved_format)
}

unsafe fn open_libarchive_destination(
    a: *mut c_void,
    destination_path: &Path,
    resolved_format: TTZipArchiveFormat,
    split_volume_size_bytes: u64,
) -> Result<(), TTZipStatus> {
    let open_rc = if split_volume_size_bytes > 0 {
        let (base_path, scheme) =
            CreateCommitMode::resolve_split_target(destination_path, resolved_format);
        let split_writer = SplitVolumeWriter::new(&base_path, split_volume_size_bytes, scheme)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        let split_box = Box::into_raw(Box::new(split_writer));
        archive_write_open2(
            a,
            split_box as *mut c_void,
            None,
            Some(split_write_cb),
            Some(split_close_cb),
            Some(split_free_cb),
        )
    } else {
        let dest_c = CString::new(destination_path.to_str().ok_or(TTZipStatus::ErrInvalidParam)?)
            .map_err(|_| TTZipStatus::ErrInvalidParam)?;
        archive_write_open_filename(a, dest_c.as_ptr())
    };
    if open_rc != 0 {
        return Err(TTZipStatus::ErrOpenFailed);
    }
    Ok(())
}

pub use super::create_staging::{
    commit_staged_archive_files_with, recover_preserved_create_backup,
    with_staged_archive_destination, with_staged_archive_destination_detailed,
    with_staged_archive_destination_detailed_using, with_staged_archive_destination_using,
    CreateCommitMode, CreateStageCommitError,
};

fn validate_create_encryption_options(
    resolved_format: TTZipArchiveFormat,
    options: &TTZipCreateOptions,
    encrypt_file_names: Option<bool>,
) -> Result<(), TTZipStatus> {
    let has_non_empty_password = if options.password.is_null() {
        false
    } else {
        let s = unsafe { std::ffi::CStr::from_ptr(options.password) }
            .to_str()
            .map_err(|_| TTZipStatus::ErrInvalidPassword)?;
        if s.is_empty() {
            return Err(TTZipStatus::ErrInvalidPassword);
        }
        true
    };
    let wants_encryption = options.encryption != crate::types::TTZipEncryptionMethod::None
        || encrypt_file_names == Some(true);
    if wants_encryption && !has_non_empty_password {
        return Err(TTZipStatus::ErrInvalidPassword);
    }
    if encrypt_file_names == Some(true) && resolved_format != TTZipArchiveFormat::SevenZip {
        return Err(TTZipStatus::ErrUnsupportedFeature);
    }
    if (wants_encryption || has_non_empty_password)
        && !matches!(
            resolved_format,
            TTZipArchiveFormat::Zip | TTZipArchiveFormat::Auto | TTZipArchiveFormat::SevenZip
        )
    {
        return Err(TTZipStatus::ErrUnsupportedFeature);
    }
    Ok(())
}

/// Recursively compresses source paths into the destination archive path with detailed rollback diagnostics.
pub fn create_archive_detailed(
    source_paths: &[PathBuf],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> Result<Vec<PathBuf>, CreateStageCommitError> {
    if source_paths.is_empty() {
        return Err(CreateStageCommitError::Status(TTZipStatus::ErrInvalidParam));
    }
    for src_path in source_paths {
        if !src_path.exists() && fs::symlink_metadata(src_path).is_err() {
            return Err(CreateStageCommitError::Status(TTZipStatus::ErrFileNotFound));
        }
    }

    let resolved_format = super::format_config::effective_create_format(options.format, destination_path);
    super::format_config::validate_creatable_format(resolved_format, false)
        .map_err(CreateStageCommitError::Status)?;
    validate_create_encryption_options(resolved_format, options, None)
        .map_err(CreateStageCommitError::Status)?;
    if is_pure_single_stream_format(resolved_format) && !should_use_raw_stream(source_paths, resolved_format) {
        return Err(CreateStageCommitError::Status(TTZipStatus::ErrInvalidParam));
    }
    let commit_mode = CreateCommitMode::for_creation(
        destination_path,
        resolved_format,
        split_volume_size_bytes,
    );

    with_staged_archive_destination_detailed(destination_path, commit_mode, |staged_dest| {
        write_staged_source_archive(
            source_paths,
            staged_dest,
            resolved_format,
            options,
            split_volume_size_bytes,
        )
    })
}

/// Recursively compresses source paths into the destination archive path.
pub fn create_archive(
    source_paths: &[PathBuf],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> Result<(), TTZipStatus> {
    create_archive_detailed(
        source_paths,
        destination_path,
        options,
        split_volume_size_bytes,
    )
    .map(|_| ())
    .map_err(|e| e.status())
}

fn collect_source_entries(source_paths: &[PathBuf]) -> Result<Vec<(PathBuf, String)>, TTZipStatus> {
    let mut entries_to_write = Vec::new();
    for src_path in source_paths {
        let base_parent = src_path.parent().unwrap_or(src_path);
        collect_entries_recursive(base_parent, src_path, &mut entries_to_write)
            .map_err(|_| TTZipStatus::ErrFileNotFound)?;
    }
    Ok(entries_to_write)
}

fn write_staged_special_container(
    source_paths: &[PathBuf],
    staged_dest: &Path,
    resolved_format: TTZipArchiveFormat,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> Option<Result<(), TTZipStatus>> {
    match resolved_format {
        TTZipArchiveFormat::Zip | TTZipArchiveFormat::Auto => Some(
            crate::zip::writer::create_zip_streaming_parallel_split(
                staged_dest, source_paths, options, split_volume_size_bytes,
            )
            .map(|_| ()),
        ),
        TTZipArchiveFormat::SevenZip => Some(
            crate::sevenz::writer::create_7z_archive_split(
                staged_dest, source_paths, options, split_volume_size_bytes,
            )
            .map(|_| ()),
        ),
        TTZipArchiveFormat::Aar => Some(collect_source_entries(source_paths).and_then(|entries| {
            super::format_config::write_apple_archive_entries(&entries, staged_dest, split_volume_size_bytes)
        })),
        TTZipArchiveFormat::Xar => Some(collect_source_entries(source_paths).and_then(|entries| {
            super::format_config::write_xar_entries(&entries, staged_dest, options, split_volume_size_bytes)
        })),
        _ => None,
    }
}

fn write_staged_source_archive(
    source_paths: &[PathBuf],
    staged_dest: &Path,
    resolved_format: TTZipArchiveFormat,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> Result<(), TTZipStatus> {
    if let Some(res) = write_staged_special_container(
        source_paths, staged_dest, resolved_format, options, split_volume_size_bytes,
    ) {
        return res;
    }

    let is_raw_stream = should_use_raw_stream(source_paths, resolved_format);
    let raw_src = is_raw_stream.then(|| source_paths[0].as_path());

    if matches!(resolved_format, TTZipArchiveFormat::TarBrotli | TTZipArchiveFormat::Brotli) {
        return write_brotli_target(staged_dest, split_volume_size_bytes, options, raw_src, |w| {
            crate::archive::tar::write_tar_to_writer(source_paths, w, options)
        });
    }
    if resolved_format == TTZipArchiveFormat::Snappy {
        return write_snappy_target(staged_dest, split_volume_size_bytes, raw_src, |w| {
            crate::archive::tar::write_tar_to_writer(source_paths, w, options)
        });
    }
    if matches!(resolved_format, TTZipArchiveFormat::TarZstd | TTZipArchiveFormat::Zstd) {
        return write_zstd_target(staged_dest, split_volume_size_bytes, options, raw_src, |w| {
            crate::archive::tar::write_tar_to_writer(source_paths, w, options)
        });
    }

    unsafe {
        let a = archive_write_new();
        if a.is_null() {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let _guard = ArchiveWriteGuard(a);
        configure_archive_write_format(a, resolved_format, options, is_raw_stream)?;
        open_libarchive_destination(a, staged_dest, resolved_format, split_volume_size_bytes)?;
        write_archive_entries_to_handle_fmt(a, source_paths, options, resolved_format)?;
    }

    Ok(())
}

/// Recursively compresses pre-filtered explicit entries into the destination archive path with optional 7z header encryption override.
pub fn create_archive_explicit_with_files_detailed_opts(
    entries: &[(PathBuf, String)],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    encrypt_file_names: Option<bool>,
) -> Result<Vec<PathBuf>, CreateStageCommitError> {
    if entries.is_empty() {
        return Err(CreateStageCommitError::Status(TTZipStatus::ErrInvalidParam));
    }
    for (src_path, _) in entries {
        if !src_path.exists() && fs::symlink_metadata(src_path).is_err() {
            return Err(CreateStageCommitError::Status(TTZipStatus::ErrFileNotFound));
        }
    }

    let resolved_format = super::format_config::effective_create_format(options.format, destination_path);
    super::format_config::validate_creatable_format(resolved_format, true)
        .map_err(CreateStageCommitError::Status)?;
    validate_create_encryption_options(resolved_format, options, encrypt_file_names)
        .map_err(CreateStageCommitError::Status)?;
    if is_pure_single_stream_format(resolved_format) && !should_use_raw_stream_explicit(entries, resolved_format) {
        return Err(CreateStageCommitError::Status(TTZipStatus::ErrInvalidParam));
    }
    let commit_mode = CreateCommitMode::for_creation(
        destination_path,
        resolved_format,
        split_volume_size_bytes,
    );

    with_staged_archive_destination_detailed(destination_path, commit_mode, |staged_dest| {
        write_staged_explicit_archive(
            entries,
            staged_dest,
            resolved_format,
            options,
            split_volume_size_bytes,
            encrypt_file_names,
        )
    })
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
        entries, destination_path, options, split_volume_size_bytes, None,
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
        entries, destination_path, options, split_volume_size_bytes,
    )
    .map_err(|e| e.status())
}

fn write_staged_explicit_archive(
    entries: &[(PathBuf, String)],
    staged_dest: &Path,
    resolved_format: TTZipArchiveFormat,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    encrypt_file_names: Option<bool>,
) -> Result<(), TTZipStatus> {
    if matches!(resolved_format, TTZipArchiveFormat::Zip | TTZipArchiveFormat::Auto) {
        return crate::zip::writer::create_zip_streaming_parallel_explicit_split(
            staged_dest, entries, options, split_volume_size_bytes,
        )
        .map(|_| ());
    }

    if resolved_format == TTZipArchiveFormat::SevenZip {
        return crate::sevenz::writer::create_7z_archive_explicit_split_with_header_encryption(
            staged_dest, entries, options, split_volume_size_bytes, encrypt_file_names,
        )
        .map(|_| ());
    }

    if resolved_format == TTZipArchiveFormat::Aar {
        return super::format_config::write_apple_archive_entries(
            entries, staged_dest, split_volume_size_bytes,
        );
    }
    if resolved_format == TTZipArchiveFormat::Xar {
        return super::format_config::write_xar_entries(
            entries, staged_dest, options, split_volume_size_bytes,
        );
    }

    let is_raw_stream = should_use_raw_stream_explicit(entries, resolved_format);
    let raw_src = is_raw_stream.then(|| entries[0].0.as_path());

    if matches!(resolved_format, TTZipArchiveFormat::TarBrotli | TTZipArchiveFormat::Brotli) {
        return write_brotli_target(staged_dest, split_volume_size_bytes, options, raw_src, |w| {
            crate::archive::tar::write_tar_entries_to_writer(entries, w, options)
        });
    }
    if resolved_format == TTZipArchiveFormat::Snappy {
        return write_snappy_target(staged_dest, split_volume_size_bytes, raw_src, |w| {
            crate::archive::tar::write_tar_entries_to_writer(entries, w, options)
        });
    }
    if matches!(resolved_format, TTZipArchiveFormat::TarZstd | TTZipArchiveFormat::Zstd) {
        return write_zstd_target(staged_dest, split_volume_size_bytes, options, raw_src, |w| {
            crate::archive::tar::write_tar_entries_to_writer(entries, w, options)
        });
    }

    unsafe {
        let a = archive_write_new();
        if a.is_null() {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let _guard = ArchiveWriteGuard(a);
        configure_archive_write_format(a, resolved_format, options, is_raw_stream)?;
        open_libarchive_destination(a, staged_dest, resolved_format, split_volume_size_bytes)?;
        write_entries_slice_to_handle_fmt(a, entries, options, resolved_format)?;
    }

    Ok(())
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

struct CountingStreamWriter<'a> {
    writer: &'a mut dyn Write,
    bytes_written: u64,
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

unsafe extern "C" fn writer_stream_write_cb(
    _a: *mut c_void,
    client_data: *mut c_void,
    buffer: *const c_void,
    length: libc::size_t,
) -> libc::ssize_t {
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
    super::format_config::validate_creatable_format(resolved_format, false)?;
    let is_raw_stream = should_use_raw_stream(source_paths, resolved_format);
    if is_pure_single_stream_format(resolved_format) && !is_raw_stream {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    if matches!(resolved_format, TTZipArchiveFormat::TarBrotli | TTZipArchiveFormat::Brotli) {
        let q = match options.level {
            TTZipCompressionLevel::Store => 0,
            TTZipCompressionLevel::Fastest => 1,
            TTZipCompressionLevel::Fast => 3,
            TTZipCompressionLevel::Normal => 6,
            TTZipCompressionLevel::Maximum => 9,
            TTZipCompressionLevel::Ultra => 11,
        };
        let mut counting = CountingStreamWriter { writer: &mut writer, bytes_written: 0 };
        {
            let mut bw = brotli::CompressorWriter::new(&mut counting, 64 * 1024, q, 22);
            if is_raw_stream {
                let mut src = File::open(&source_paths[0]).map_err(|_| TTZipStatus::ErrFileNotFound)?;
                std::io::copy(&mut src, &mut bw).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            } else {
                crate::archive::tar::write_tar_to_writer(source_paths, bw, options)?;
            }
        }
        counting.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        return Ok(counting.bytes_written);
    }

    if resolved_format == TTZipArchiveFormat::Snappy {
        let mut counting = CountingStreamWriter { writer: &mut writer, bytes_written: 0 };
        {
            let mut se = snap::write::FrameEncoder::new(&mut counting);
            if is_raw_stream {
                let mut src = File::open(&source_paths[0]).map_err(|_| TTZipStatus::ErrFileNotFound)?;
                std::io::copy(&mut src, &mut se).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            } else {
                se = crate::archive::tar::write_tar_to_writer(source_paths, se, options)?;
            }
            se.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        }
        counting.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        return Ok(counting.bytes_written);
    }

    if matches!(resolved_format, TTZipArchiveFormat::TarZstd | TTZipArchiveFormat::Zstd) {
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
            std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(4)
        };
        let config = crate::codecs::zstd::types::ZstdConfig {
            level: z_lvl,
            nb_workers,
            enable_checksum: true,
            ..Default::default()
        };
        let mut counting = CountingStreamWriter { writer: &mut writer, bytes_written: 0 };
        {
            let mut zw = crate::codecs::zstd::ZstdStreamWriter::new(&mut counting, &config)?;
            if is_raw_stream {
                let mut src = File::open(&source_paths[0]).map_err(|_| TTZipStatus::ErrFileNotFound)?;
                std::io::copy(&mut src, &mut zw).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            } else {
                zw = crate::archive::tar::write_tar_to_writer(source_paths, zw, options)?;
            }
            zw.finish()?;
        }
        counting.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        return Ok(counting.bytes_written);
    }

    if resolved_format == TTZipArchiveFormat::Tar {
        let mut counting = CountingStreamWriter { writer: &mut writer, bytes_written: 0 };
        crate::archive::tar::write_tar_to_writer(source_paths, &mut counting, options)?;
        counting.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        return Ok(counting.bytes_written);
    }

    let mut counting = CountingStreamWriter { writer: &mut writer, bytes_written: 0 };
    unsafe {
        let a = archive_write_new();
        if a.is_null() {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let _guard = ArchiveWriteGuard(a);
        configure_archive_write_format(a, resolved_format, options, is_raw_stream)?;
        let open_rc = archive_write_open2(
            a,
            &mut counting as *mut _ as *mut c_void,
            None,
            Some(writer_stream_write_cb),
            None,
            None,
        );
        if open_rc != 0 {
            return Err(TTZipStatus::ErrOpenFailed);
        }
        write_archive_entries_to_handle_fmt(a, source_paths, options, resolved_format)?;
    }

    counting.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    Ok(counting.bytes_written)
}

unsafe fn write_archive_entries_to_handle_fmt(
    a: *mut c_void,
    source_paths: &[PathBuf],
    options: &TTZipCreateOptions,
    format: TTZipArchiveFormat,
) -> Result<(), TTZipStatus> {
    let mut entries_to_write = Vec::new();
    for src_path in source_paths {
        if !src_path.exists() && fs::symlink_metadata(src_path).is_err() {
            return Err(TTZipStatus::ErrFileNotFound);
        }
        let base_parent = src_path.parent().unwrap_or(src_path);
        collect_entries_recursive(base_parent, src_path, &mut entries_to_write)
            .map_err(|_| TTZipStatus::ErrFileNotFound)?;
    }
    write_entries_slice_to_handle_fmt(a, &entries_to_write, options, format)
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
        let n = file.read(buf).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        if n == 0 {
            break;
        }
        if archive_write_data(a, buf.as_ptr() as *const c_void, n) < 0 {
            return Err(TTZipStatus::ErrCompressionFailed);
        }
        *processed_bytes = processed_bytes.saturating_add(n as u64);
        if let Some(cb) = options.progress_callback {
            if !cb(*processed_bytes, *processed_bytes, rel_c.as_ptr(), options.user_data) {
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

pub(crate) unsafe fn write_entries_slice_to_handle_fmt(
    a: *mut c_void,
    entries_to_write: &[(PathBuf, String)],
    options: &TTZipCreateOptions,
    format: TTZipArchiveFormat,
) -> Result<(), TTZipStatus> {
    libc::setlocale(libc::LC_ALL, c"en_US.UTF-8".as_ptr());
    let mut processed_bytes: u64 = 0;
    let mut buf = vec![0u8; 64 * 1024];

    for (abs_path, rel_name) in entries_to_write {
        let Ok(meta) = fs::symlink_metadata(abs_path) else { continue };
        if format == TTZipArchiveFormat::Ar && (meta.is_dir() || meta.file_type().is_symlink()) {
            return Err(TTZipStatus::ErrInvalidParam);
        }

        let entry = archive_entry_new();
        if entry.is_null() {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let _entry_guard = ArchiveEntryGuard(entry);

        let entry_path_str = if format == TTZipArchiveFormat::Ar {
            Path::new(rel_name.as_str()).file_name().and_then(|s| s.to_str()).unwrap_or(rel_name.as_str())
        } else {
            rel_name.as_str()
        };
        let Ok(rel_c) = CString::new(entry_path_str) else { continue };

        archive_entry_set_pathname(entry, rel_c.as_ptr());
        let mtime = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as time_t);
        archive_entry_set_mtime(entry, mtime, 0);
        write_libarchive_typed_entry(a, entry, abs_path, &meta, &rel_c, options, &mut buf, &mut processed_bytes)?;

        if let Some(cb) = options.progress_callback {
            if !cb(processed_bytes, processed_bytes, rel_c.as_ptr(), options.user_data) {
                return Err(TTZipStatus::Cancelled);
            }
        }
    }

    Ok(())
}

/// Helper to collect files and directories using an explicit heap worklist to ensure O(1) call stack depth.
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
