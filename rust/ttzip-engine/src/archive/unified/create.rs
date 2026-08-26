// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Archive creation lifecycle submodule for Unified Orchestrator.

use std::ffi::CString;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::archive::split::{SplitVolumeWriter, VolumeNamingScheme};
use crate::archive::unified::detect::resolve_create_format;
use crate::ffi::archive_ffi::guards::{ArchiveEntryGuard, ArchiveWriteGuard};
use crate::ffi::archive_ffi::sys::*;
use crate::types::{
    TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod,
    TTZipStatus,
};
use libc::{c_void, mode_t, time_t};

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
        && options.encryption == crate::types::TTZipEncryptionMethod::None
    {
        return crate::zip::writer::create_zip_streaming_parallel(destination_path, source_paths, options)
            .map(|_| ());
    }

    // 2. Pure Safe Rust High-Throughput Brotli Pipeline
    if resolved_format == TTZipArchiveFormat::TarBrotli || resolved_format == TTZipArchiveFormat::Brotli {
        let tmp_tar = destination_path.with_extension(format!("ttzip_tmp_{}.tar", std::process::id()));
        let mut tar_opts = *options;
        tar_opts.format = TTZipArchiveFormat::Tar;
        create_archive(source_paths, &tmp_tar, &tar_opts, 0)?;
        let res = crate::codecs::brotli::brotli_compress_file(&tmp_tar, destination_path, 6, 22, None);
        let _ = fs::remove_file(&tmp_tar);
        return res.map(|_| ());
    }

    // 3. Pure Safe Rust High-Throughput Snappy Pipeline
    if resolved_format == TTZipArchiveFormat::Snappy {
        let tmp_tar = destination_path.with_extension(format!("ttzip_tmp_{}.tar", std::process::id()));
        let mut tar_opts = *options;
        tar_opts.format = TTZipArchiveFormat::Tar;
        create_archive(source_paths, &tmp_tar, &tar_opts, 0)?;
        let res = crate::codecs::snappy::snappy_compress_file(&tmp_tar, destination_path, None);
        let _ = fs::remove_file(&tmp_tar);
        return res.map(|_| ());
    }

    // 4. Native Multi-Core High-Throughput Zstandard Pipeline
    if (resolved_format == TTZipArchiveFormat::TarZstd || resolved_format == TTZipArchiveFormat::Zstd)
        && split_volume_size_bytes == 0
    {
        let tmp_tar = destination_path.with_extension(format!("ttzip_tmp_{}.tar", std::process::id()));
        let mut tar_opts = *options;
        tar_opts.format = TTZipArchiveFormat::Tar;
        create_archive(source_paths, &tmp_tar, &tar_opts, 0)?;

        let mut src_f = File::open(&tmp_tar).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let mut dst_f = File::create(destination_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let z_lvl = match options.level {
            TTZipCompressionLevel::Store => 1,
            TTZipCompressionLevel::Fastest => 1,
            TTZipCompressionLevel::Fast => 3,
            TTZipCompressionLevel::Normal => 6,
            TTZipCompressionLevel::Maximum => 19,
            TTZipCompressionLevel::Ultra => 22,
        };
        let cfg = crate::codecs::zstd::ZstdConfig {
            level: z_lvl,
            ..Default::default()
        };
        let res = crate::codecs::zstd::zstd_compress_stream_pipe(&mut src_f, &mut dst_f, &cfg, None);
        let _ = fs::remove_file(&tmp_tar);
        return res.map(|_| ());
    }

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

    unsafe {
        let a = archive_write_new();
        if a.is_null() {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let _guard = ArchiveWriteGuard(a);

        let fmt_rc = match resolved_format {
            TTZipArchiveFormat::Zip | TTZipArchiveFormat::Auto => {
                let rc = archive_write_set_format_zip(a);
                if rc == 0 {
                    let lvl = match options.level {
                        TTZipCompressionLevel::Store => 0,
                        TTZipCompressionLevel::Fastest => 1,
                        TTZipCompressionLevel::Fast => 3,
                        TTZipCompressionLevel::Normal => 6,
                        TTZipCompressionLevel::Maximum => 9,
                        TTZipCompressionLevel::Ultra => 9,
                    };
                    let lvl_opt = CString::new(format!("zip:compression-level={}", lvl)).unwrap();
                    archive_write_set_options(a, lvl_opt.as_ptr());
                }
                rc
            }
            TTZipArchiveFormat::SevenZip => {
                if !options.password.is_null() {
                    return Err(TTZipStatus::ErrCompressionFailed);
                }
                archive_write_set_format_7zip(a)
            }
            TTZipArchiveFormat::Tar => {
                archive_write_set_format_pax_restricted(a)
            }
            TTZipArchiveFormat::TarGz | TTZipArchiveFormat::Gzip => {
                let r1 = archive_write_set_format_pax_restricted(a);
                let r2 = archive_write_add_filter_gzip(a);
                if r1 != 0 { r1 } else { r2 }
            }
            TTZipArchiveFormat::TarBz2 | TTZipArchiveFormat::Bzip2 => {
                let r1 = archive_write_set_format_pax_restricted(a);
                let r2 = archive_write_add_filter_bzip2(a);
                if r1 != 0 { r1 } else { r2 }
            }
            TTZipArchiveFormat::TarXz | TTZipArchiveFormat::Xz => {
                let r1 = archive_write_set_format_pax_restricted(a);
                let r2 = archive_write_add_filter_xz(a);
                if r1 != 0 { r1 } else { r2 }
            }
            TTZipArchiveFormat::TarZstd | TTZipArchiveFormat::Zstd => {
                let r1 = archive_write_set_format_pax_restricted(a);
                let r2 = archive_write_add_filter_zstd(a);
                if r1 != 0 { r1 } else { r2 }
            }
            TTZipArchiveFormat::TarLz4 | TTZipArchiveFormat::Lz4 => {
                let r1 = archive_write_set_format_pax_restricted(a);
                let r2 = archive_write_add_filter_lz4(a);
                if r1 != 0 { r1 } else { r2 }
            }
            TTZipArchiveFormat::TarBrotli | TTZipArchiveFormat::Brotli => {
                archive_write_set_format_pax_restricted(a)
            }
            TTZipArchiveFormat::TarLzip | TTZipArchiveFormat::Lzip => {
                let r1 = archive_write_set_format_pax_restricted(a);
                let r2 = archive_write_add_filter_lzip(a);
                if r1 != 0 { r1 } else { r2 }
            }
            TTZipArchiveFormat::TarLrzip | TTZipArchiveFormat::Lrzip => {
                let r1 = archive_write_set_format_pax_restricted(a);
                let r2 = archive_write_add_filter_lrzip(a);
                if r1 != 0 { r1 } else { r2 }
            }
            TTZipArchiveFormat::Iso => {
                archive_write_set_format_iso9660(a)
            }
            TTZipArchiveFormat::Cpio => {
                archive_write_set_format_cpio_newc(a)
            }
            TTZipArchiveFormat::Ar | TTZipArchiveFormat::Deb => {
                archive_write_set_format_ar_svr4(a)
            }
            TTZipArchiveFormat::Xar => {
                archive_write_set_format_xar(a)
            }
            TTZipArchiveFormat::Wim => {
                // WIM archive creation is not supported by libarchive format writer
                return Err(TTZipStatus::ErrCompressionFailed);
            }
            TTZipArchiveFormat::Rar => {
                // RAR creation is proprietary and unlicensable for arbitrary runtime packaging
                return Err(TTZipStatus::ErrInvalidParam);
            }
            _ => {
                archive_write_set_format_zip(a)
            }
        };

        if fmt_rc != 0 && fmt_rc != -20 {
            return Err(TTZipStatus::ErrCompressionFailed);
        }

        if !options.password.is_null() {
            archive_write_set_passphrase(a, options.password);
            if options.encryption == TTZipEncryptionMethod::Aes256 {
                let enc_opt = CString::new("zip:encryption=aes256").unwrap();
                archive_write_set_options(a, enc_opt.as_ptr());
            }
        }

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

        // Collect all source entries
        let mut entries_to_write = Vec::new();
        for src_path in source_paths {
            if !src_path.exists() && fs::symlink_metadata(src_path).is_err() {
                return Err(TTZipStatus::ErrFileNotFound);
            }
            let base_parent = src_path.parent().unwrap_or(src_path);
            collect_entries_recursive(base_parent, src_path, &mut entries_to_write)
                .map_err(|_| TTZipStatus::ErrFileNotFound)?;
        }

        let mut processed_bytes: u64 = 0;
        let mut buf = vec![0u8; 64 * 1024];

        for (abs_path, rel_name) in entries_to_write {
            let meta = match fs::symlink_metadata(&abs_path) {
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
                if let Ok(target) = fs::read_link(&abs_path) {
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

                let mut file = File::open(&abs_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
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
                }
                archive_write_finish_entry(a);
            }

            if let Some(cb) = options.progress_callback {
                if !cb(processed_bytes, processed_bytes, rel_c.as_ptr(), options.user_data) {
                    return Err(TTZipStatus::Cancelled);
                }
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
