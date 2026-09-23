// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Archive creation lifecycle submodule for Unified Orchestrator.

use std::ffi::CString;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::archive::split::{SplitVolumeWriter, VolumeNamingScheme};
use crate::archive::unified::detect::resolve_create_format;
use super::format_config::configure_archive_write_format;
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
        && source_paths[0].is_file()
        && is_pure_single_stream_format(resolved_format)
}

#[inline]
fn should_use_raw_stream_explicit(entries: &[(PathBuf, String)], resolved_format: TTZipArchiveFormat) -> bool {
    entries.len() == 1
        && entries[0].0.is_file()
        && is_pure_single_stream_format(resolved_format)
}

/// Recursively compresses source paths into the destination archive path.
pub fn create_archive(
    source_paths: &[PathBuf],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> Result<(), TTZipStatus> {
    if source_paths.is_empty() {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    if let Some(parent) = destination_path.parent() {
        if !parent.exists() {
            let _ = fs::create_dir_all(parent);
        }
    }

    let resolved_format = resolve_create_format(options.format, destination_path);

    // 1. High-Performance Streaming Multi-Core Parallel ZIP Fast Path
    if (resolved_format == TTZipArchiveFormat::Zip || resolved_format == TTZipArchiveFormat::Auto)
        && split_volume_size_bytes == 0
    {
        return crate::zip::writer::create_zip_streaming_parallel(destination_path, source_paths, options)
            .map(|_| ());
    }

    let is_raw_stream = should_use_raw_stream(source_paths, resolved_format);

    // 2. Pure Safe Rust High-Throughput Brotli Streaming Pipeline (0% Disk Write Amplification)
    if resolved_format == TTZipArchiveFormat::TarBrotli || resolved_format == TTZipArchiveFormat::Brotli {
        if split_volume_size_bytes > 0 {
            return Err(TTZipStatus::ErrUnsupportedFeature);
        }
        let out_file = File::create(destination_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let q = match options.level {
            TTZipCompressionLevel::Store => 0,
            TTZipCompressionLevel::Fastest => 1,
            TTZipCompressionLevel::Fast => 3,
            TTZipCompressionLevel::Normal => 6,
            TTZipCompressionLevel::Maximum => 9,
            TTZipCompressionLevel::Ultra => 11,
        };
        let mut brotli_writer = brotli::CompressorWriter::new(out_file, 64 * 1024, q, 22);
        if is_raw_stream {
            let mut in_file = File::open(&source_paths[0]).map_err(|_| TTZipStatus::ErrFileNotFound)?;
            std::io::copy(&mut in_file, &mut brotli_writer).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            brotli_writer.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        } else {
            crate::archive::tar::write_tar_to_writer(source_paths, brotli_writer, options)?;
        }
        return Ok(());
    }

    // 3. Pure Safe Rust High-Throughput Snappy Streaming Pipeline (0% Disk Write Amplification)
    if resolved_format == TTZipArchiveFormat::Snappy {
        if split_volume_size_bytes > 0 {
            return Err(TTZipStatus::ErrUnsupportedFeature);
        }
        let out_file = File::create(destination_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let mut snappy_encoder = snap::write::FrameEncoder::new(out_file);
        if is_raw_stream {
            let mut in_file = File::open(&source_paths[0]).map_err(|_| TTZipStatus::ErrFileNotFound)?;
            std::io::copy(&mut in_file, &mut snappy_encoder).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            snappy_encoder.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        } else {
            let mut snappy_encoder = crate::archive::tar::write_tar_to_writer(source_paths, snappy_encoder, options)?;
            snappy_encoder.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        }
        return Ok(());
    }

    // 4. Native Streaming High-Throughput Zstandard Multithreaded Pipeline (0% Disk Write Amplification)
    if (resolved_format == TTZipArchiveFormat::TarZstd || resolved_format == TTZipArchiveFormat::Zstd)
        && split_volume_size_bytes == 0
    {
        let out_file = File::create(destination_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let z_lvl = match options.level {
            TTZipCompressionLevel::Store => 1,
            TTZipCompressionLevel::Fastest => 1,
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
        let mut zstd_writer = crate::codecs::zstd::ZstdStreamWriter::new(out_file, &config)?;
        if is_raw_stream {
            let mut in_file = File::open(&source_paths[0]).map_err(|_| TTZipStatus::ErrFileNotFound)?;
            std::io::copy(&mut in_file, &mut zstd_writer).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            zstd_writer.finish()?;
        } else {
            let zstd_writer = crate::archive::tar::write_tar_to_writer(source_paths, zstd_writer, options)?;
            zstd_writer.finish()?;
        }
        return Ok(());
    }

    // 5. Pure Safe Rust High-Throughput 7-Zip Solid Archive Creation Pipeline
    if resolved_format == TTZipArchiveFormat::SevenZip && split_volume_size_bytes == 0 {
        return crate::sevenz::writer::create_7z_archive(destination_path, source_paths, options)
            .map(|_| ());
    }

    unsafe {
        let a = archive_write_new();
        if a.is_null() {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let _guard = ArchiveWriteGuard(a);

        configure_archive_write_format(a, resolved_format, options, is_raw_stream)?;

        let open_rc = if split_volume_size_bytes > 0 {
            let is_zip = resolved_format == TTZipArchiveFormat::Zip;
            let scheme = if is_zip {
                VolumeNamingScheme::PkzipSpanned
            } else {
                VolumeNamingScheme::NumberedExtension
            };

            let split_writer = SplitVolumeWriter::new(
                destination_path,
                split_volume_size_bytes,
                scheme,
            )
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
            let dest_c = CString::new(
                destination_path
                    .to_str()
                    .ok_or(TTZipStatus::ErrInvalidParam)?,
            )
            .map_err(|_| TTZipStatus::ErrInvalidParam)?;
            archive_write_open_filename(a, dest_c.as_ptr())
        };

        if open_rc != 0 {
            return Err(TTZipStatus::ErrOpenFailed);
        }

        write_archive_entries_to_handle(a, source_paths, options)?;
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
    if entries.is_empty() {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    if let Some(parent) = destination_path.parent() {
        if !parent.exists() {
            let _ = fs::create_dir_all(parent);
        }
    }

    let resolved_format = resolve_create_format(options.format, destination_path);

    // 1. High-Performance Streaming Multi-Core Parallel ZIP Fast Path
    if (resolved_format == TTZipArchiveFormat::Zip || resolved_format == TTZipArchiveFormat::Auto)
        && split_volume_size_bytes == 0
    {
        return crate::zip::writer::create_zip_streaming_parallel_explicit(destination_path, entries, options)
            .map(|_| ());
    }

    let is_raw_stream = should_use_raw_stream_explicit(entries, resolved_format);

    // 2. Pure Safe Rust High-Throughput 7-Zip Solid Archive Creation Pipeline
    if resolved_format == TTZipArchiveFormat::SevenZip && split_volume_size_bytes == 0 {
        return crate::sevenz::writer::create_7z_archive_explicit(destination_path, entries, options)
            .map(|_| ());
    }

    unsafe {
        let a = archive_write_new();
        if a.is_null() {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let _guard = ArchiveWriteGuard(a);

        configure_archive_write_format(a, resolved_format, options, is_raw_stream)?;

        let open_rc = if split_volume_size_bytes > 0 {
            let is_zip = resolved_format == TTZipArchiveFormat::Zip;
            let scheme = if is_zip {
                VolumeNamingScheme::PkzipSpanned
            } else {
                VolumeNamingScheme::NumberedExtension
            };

            let split_writer = SplitVolumeWriter::new(
                destination_path,
                split_volume_size_bytes,
                scheme,
            )
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

        write_entries_slice_to_handle(a, entries, options)?;
    }

    Ok(())
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

    // 1. Pure Safe Rust High-Throughput Brotli Streaming Pipeline
    if resolved_format == TTZipArchiveFormat::TarBrotli || resolved_format == TTZipArchiveFormat::Brotli {
        let q = match options.level {
            TTZipCompressionLevel::Store => 0,
            TTZipCompressionLevel::Fastest => 1,
            TTZipCompressionLevel::Fast => 3,
            TTZipCompressionLevel::Normal => 6,
            TTZipCompressionLevel::Maximum => 9,
            TTZipCompressionLevel::Ultra => 11,
        };
        let mut counting = CountingStreamWriter {
            writer: &mut writer,
            bytes_written: 0,
        };
        {
            let brotli_writer = brotli::CompressorWriter::new(&mut counting, 64 * 1024, q, 22);
            crate::archive::tar::write_tar_to_writer(source_paths, brotli_writer, options)?;
        }
        counting.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        return Ok(counting.bytes_written);
    }

    // 2. Pure Safe Rust High-Throughput Snappy Streaming Pipeline
    if resolved_format == TTZipArchiveFormat::Snappy {
        let mut counting = CountingStreamWriter {
            writer: &mut writer,
            bytes_written: 0,
        };
        {
            let snappy_encoder = snap::write::FrameEncoder::new(&mut counting);
            let mut snappy_encoder =
                crate::archive::tar::write_tar_to_writer(source_paths, snappy_encoder, options)?;
            snappy_encoder
                .flush()
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        }
        counting.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        return Ok(counting.bytes_written);
    }

    // 3. Native Streaming High-Throughput Zstandard Multithreaded Pipeline
    if resolved_format == TTZipArchiveFormat::TarZstd || resolved_format == TTZipArchiveFormat::Zstd {
        let z_lvl = match options.level {
            TTZipCompressionLevel::Store => 1,
            TTZipCompressionLevel::Fastest => 1,
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
        let mut counting = CountingStreamWriter {
            writer: &mut writer,
            bytes_written: 0,
        };
        {
            let zstd_writer = crate::codecs::zstd::ZstdStreamWriter::new(&mut counting, &config)?;
            let zstd_writer =
                crate::archive::tar::write_tar_to_writer(source_paths, zstd_writer, options)?;
            zstd_writer.finish()?;
        }
        counting.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        return Ok(counting.bytes_written);
    }

    // 4. Pure Safe Rust TAR Streaming Pipeline
    if resolved_format == TTZipArchiveFormat::Tar {
        let mut counting = CountingStreamWriter {
            writer: &mut writer,
            bytes_written: 0,
        };
        crate::archive::tar::write_tar_to_writer(source_paths, &mut counting, options)?;
        counting.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        return Ok(counting.bytes_written);
    }

    // 5. Universal Libarchive Stream Writing Pipeline (Zip, 7z, TarGz, TarBz2, TarXz, etc.)
    let mut counting = CountingStreamWriter {
        writer: &mut writer,
        bytes_written: 0,
    };

    let is_raw_stream = should_use_raw_stream(source_paths, resolved_format);

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

        write_archive_entries_to_handle(a, source_paths, options)?;
        drop(_guard);
    }

    counting.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    Ok(counting.bytes_written)
}



unsafe fn write_archive_entries_to_handle(
    a: *mut c_void,
    source_paths: &[PathBuf],
    options: &TTZipCreateOptions,
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
    write_entries_slice_to_handle(a, &entries_to_write, options)
}

pub(crate) unsafe fn write_entries_slice_to_handle(
    a: *mut c_void,
    entries_to_write: &[(PathBuf, String)],
    options: &TTZipCreateOptions,
) -> Result<(), TTZipStatus> {
    libc::setlocale(libc::LC_ALL, b"en_US.UTF-8\0".as_ptr() as *const libc::c_char);
    let mut processed_bytes: u64 = 0;
    let mut buf = vec![0u8; 64 * 1024];

    for (abs_path, rel_name) in entries_to_write {
        let meta = match fs::symlink_metadata(abs_path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let entry = archive_entry_new();
        if entry.is_null() {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let _entry_guard = ArchiveEntryGuard(entry);

        let rel_c = match CString::new(rel_name.as_str()) {
            Ok(c) => c,
            Err(_) => continue,
        };

        archive_entry_set_pathname(entry, rel_c.as_ptr());
        archive_entry_set_mtime(
            entry,
            meta.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as time_t)
                .unwrap_or(0),
            0,
        );

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
            archive_write_header(a, entry);
            archive_write_finish_entry(a);
        } else if filetype.is_dir() {
            archive_entry_set_filetype(entry, libc::S_IFDIR as u32);
            archive_entry_set_perm(entry, (meta.permissions().mode() & 0o777) as mode_t);
            archive_entry_set_size(entry, 0);
            archive_write_header(a, entry);
            archive_write_finish_entry(a);
        } else {
            let size = meta.len();
            archive_entry_set_filetype(entry, libc::S_IFREG as u32);
            archive_entry_set_perm(entry, (meta.permissions().mode() & 0o777) as mode_t);
            archive_entry_set_size(entry, size as i64);

            let header_rc = archive_write_header(a, entry);
            if header_rc != 0 {
                return Err(TTZipStatus::ErrCompressionFailed);
            }

            let mut file = File::open(abs_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
            loop {
                let n = file
                    .read(&mut buf)
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                if n == 0 {
                    break;
                }

                let written = archive_write_data(a, buf.as_ptr() as *const c_void, n);
                if written < 0 {
                    return Err(TTZipStatus::ErrCompressionFailed);
                }
                processed_bytes = processed_bytes.saturating_add(n as u64);

                // Cancellation check within 64KB chunk streaming loop for large files
                if let Some(cb) = options.progress_callback {
                    if !cb(processed_bytes, processed_bytes, rel_c.as_ptr(), options.user_data) {
                        return Err(TTZipStatus::Cancelled);
                    }
                }
            }
            archive_write_finish_entry(a);
        }

        if let Some(cb) = options.progress_callback {
            if !cb(processed_bytes, processed_bytes, rel_c.as_ptr(), options.user_data) {
                return Err(TTZipStatus::Cancelled);
            }
        }
    }

    Ok(())
}

fn collect_entries_recursive(
    root: &Path,
    current: &Path,
    out: &mut Vec<(PathBuf, String)>,
) -> std::io::Result<()> {
    let rel_prefix = current.strip_prefix(root).unwrap_or(current);
    let rel_str = rel_prefix.to_string_lossy().to_string();

    if !rel_str.is_empty() {
        out.push((current.to_path_buf(), rel_str));
    }

    if let Ok(meta) = fs::symlink_metadata(current) {
        if meta.is_dir() && !meta.file_type().is_symlink() {
            for entry in fs::read_dir(current)? {
                let entry = entry?;
                collect_entries_recursive(root, &entry.path(), out)?;
            }
        }
    }
    Ok(())
}



