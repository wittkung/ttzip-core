// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Archive creation C-ABI FFI entry implementation.

use super::guards::{ArchiveEntryGuard, ArchiveWriteGuard};
use super::sys::*;
use crate::ffi::helpers::safe_cstr;
use crate::fs::apfs::AlignedBuffer;
use crate::types::{
    TTZipArchiveFormat, TTZipCreateOptions, TTZipEncryptionMethod, TTZipStatus,
};
use libc::{c_char, c_void, mode_t, time_t};
use std::ffi::CString;
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::panic::catch_unwind;
use std::path::{Path, PathBuf};

fn collect_entries_recursive(
    root: &Path,
    current: &Path,
    out: &mut Vec<(PathBuf, String)>,
) -> std::io::Result<()> {
    let rel_str = if current == root {
        current.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
    } else {
        current.strip_prefix(root).unwrap_or(current).to_string_lossy().to_string()
    };

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

std::thread_local! {
    static STAGED_FFI_ENCRYPT_FILE_NAMES: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// C-ABI v2 unified archive creation with explicit `encrypt_file_names_mode`
/// (`1` => `Some(true)`, `0` => `Some(false)`, `< 0` => `None`).
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_create_archive_v2(
    source_paths: *const *const c_char,
    source_count: usize,
    destination_path: *const c_char,
    options: *const TTZipCreateOptions,
    encrypt_file_names_mode: i32,
) -> TTZipStatus {
    let encrypt_file_names = match encrypt_file_names_mode {
        1 => Some(true),
        0 => Some(false),
        _ => None,
    };
    STAGED_FFI_ENCRYPT_FILE_NAMES.with(|slot| slot.set(encrypt_file_names));
    ttzip_rust_create_archive(source_paths, source_count, destination_path, options)
}

/// C-ABI exported unified archive creation.
///
/// Compresses `source_paths` into `destination_path` according to `options`.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_create_archive(
    source_paths: *const *const c_char,
    source_count: usize,
    destination_path: *const c_char,
    options: *const TTZipCreateOptions,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        let encrypt_file_names = STAGED_FFI_ENCRYPT_FILE_NAMES.with(|slot| slot.take());
        if source_paths.is_null() || source_count == 0 || options.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }

        let dest_str = match unsafe { safe_cstr(destination_path) } {
            Ok(s) => s,
            Err(st) => return st,
        };
        let dest_p = Path::new(dest_str);
        let opt = unsafe { &*options };

        let has_non_empty_password = if opt.password.is_null() {
            false
        } else {
            match unsafe { safe_cstr(opt.password) } {
                Ok(s) if !s.is_empty() => true,
                _ => return TTZipStatus::ErrInvalidPassword,
            }
        };
        if (opt.encryption != TTZipEncryptionMethod::None || encrypt_file_names == Some(true))
            && !has_non_empty_password
        {
            return TTZipStatus::ErrInvalidPassword;
        }
        if encrypt_file_names == Some(true) && opt.format != TTZipArchiveFormat::SevenZip {
            return TTZipStatus::ErrUnsupportedFeature;
        }

        if let Some(parent) = dest_p.parent() {
            if !parent.exists() {
                let _ = fs::create_dir_all(parent);
            }
        }

        // Collect all entries to compress first so we can validate cardinality and existence
        let mut raw_sources: Vec<PathBuf> = Vec::with_capacity(source_count);
        let mut entries_to_write: Vec<(PathBuf, String)> = Vec::new();
        for i in 0..source_count {
            let src_c = unsafe { *source_paths.add(i) };
            let src_str = match unsafe { safe_cstr(src_c) } {
                Ok(s) => s,
                Err(_) => continue,
            };
            let src_path = Path::new(src_str);
            if !src_path.exists() && fs::symlink_metadata(src_path).is_err() {
                return TTZipStatus::ErrFileNotFound;
            }
            raw_sources.push(src_path.to_path_buf());
            let base_parent = src_path.parent().unwrap_or(src_path);
            let _ = collect_entries_recursive(base_parent, src_path, &mut entries_to_write);
        }

        if entries_to_write.is_empty() {
            return TTZipStatus::ErrInvalidParam;
        }

        if opt.format == TTZipArchiveFormat::SevenZip && encrypt_file_names.is_some() {
            return match crate::archive::unified::create::create_archive_explicit_with_files_detailed_opts(
                &entries_to_write,
                dest_p,
                opt,
                0,
                encrypt_file_names,
            ) {
                Ok(_) => TTZipStatus::Ok,
                Err(e) => e.status(),
            };
        }

        // Delegate formats handled by native Rust / AppleArchive writers to unified::create
        if matches!(
            opt.format,
            TTZipArchiveFormat::SevenZip
                | TTZipArchiveFormat::Aar
                | TTZipArchiveFormat::Xar
                | TTZipArchiveFormat::Brotli
                | TTZipArchiveFormat::TarBrotli
                | TTZipArchiveFormat::Snappy
        ) {
            return match crate::archive::unified::create::create_archive(
                &raw_sources,
                dest_p,
                opt,
                0,
            ) {
                Ok(()) => TTZipStatus::Ok,
                Err(st) => st,
            };
        }

        let is_single_regular_file = entries_to_write.len() == 1
            && fs::symlink_metadata(&entries_to_write[0].0)
                .map(|m| m.is_file() && !m.file_type().is_symlink())
                .unwrap_or(false);
        let dest_lower = dest_p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();
        let is_tar_compound_dest = dest_lower.contains(".tar.")
            || dest_lower.ends_with(".tgz")
            || dest_lower.ends_with(".tbz2")
            || dest_lower.ends_with(".tbz")
            || dest_lower.ends_with(".txz")
            || dest_lower.ends_with(".tzst")
            || dest_lower.ends_with(".tlz4")
            || dest_lower.ends_with(".tlz")
            || dest_lower.ends_with(".tlrz");

        let is_pure_stream = matches!(
            opt.format,
            TTZipArchiveFormat::Gzip
                | TTZipArchiveFormat::Bzip2
                | TTZipArchiveFormat::Xz
                | TTZipArchiveFormat::Zstd
                | TTZipArchiveFormat::Lz4
                | TTZipArchiveFormat::Lzip
                | TTZipArchiveFormat::Lrzip
        ) && !is_tar_compound_dest;

        if is_pure_stream && !is_single_regular_file {
            return TTZipStatus::ErrInvalidParam;
        }

        let a = archive_write_new();
        if a.is_null() {
            return TTZipStatus::ErrOutOfMemory;
        }
        let guard = ArchiveWriteGuard(a);

        match opt.format {
            TTZipArchiveFormat::Zip | TTZipArchiveFormat::Auto => {
                archive_write_set_format_zip(a);
            }
            TTZipArchiveFormat::Tar => {
                archive_write_set_format_pax_restricted(a);
            }
            TTZipArchiveFormat::TarGz | TTZipArchiveFormat::Gzip => {
                if is_pure_stream {
                    archive_write_set_format_raw(a);
                } else {
                    archive_write_set_format_pax_restricted(a);
                }
                archive_write_add_filter_gzip(a);
            }
            TTZipArchiveFormat::TarBz2 | TTZipArchiveFormat::Bzip2 => {
                if is_pure_stream {
                    archive_write_set_format_raw(a);
                } else {
                    archive_write_set_format_pax_restricted(a);
                }
                archive_write_add_filter_bzip2(a);
            }
            TTZipArchiveFormat::TarXz | TTZipArchiveFormat::Xz => {
                if is_pure_stream {
                    archive_write_set_format_raw(a);
                } else {
                    archive_write_set_format_pax_restricted(a);
                }
                archive_write_add_filter_xz(a);
            }
            TTZipArchiveFormat::TarZstd | TTZipArchiveFormat::Zstd => {
                if is_pure_stream {
                    archive_write_set_format_raw(a);
                } else {
                    archive_write_set_format_pax_restricted(a);
                }
                archive_write_add_filter_zstd(a);
            }
            TTZipArchiveFormat::TarLz4 | TTZipArchiveFormat::Lz4 => {
                if is_pure_stream {
                    archive_write_set_format_raw(a);
                } else {
                    archive_write_set_format_pax_restricted(a);
                }
                archive_write_add_filter_lz4(a);
            }
            TTZipArchiveFormat::TarLzip | TTZipArchiveFormat::Lzip => {
                if is_pure_stream {
                    archive_write_set_format_raw(a);
                } else {
                    archive_write_set_format_pax_restricted(a);
                }
                archive_write_add_filter_lzip(a);
            }
            TTZipArchiveFormat::TarLrzip | TTZipArchiveFormat::Lrzip => {
                if is_pure_stream {
                    archive_write_set_format_raw(a);
                } else {
                    archive_write_set_format_pax_restricted(a);
                }
                archive_write_add_filter_lrzip(a);
            }
            TTZipArchiveFormat::Iso => {
                archive_write_set_format_iso9660(a);
            }
            TTZipArchiveFormat::Cpio => {
                archive_write_set_format_cpio_newc(a);
            }
            TTZipArchiveFormat::Ar => {
                archive_write_set_format_ar_bsd(a);
            }
            TTZipArchiveFormat::Xar => {
                archive_write_set_format_xar(a);
            }
            TTZipArchiveFormat::Rar => {
                return TTZipStatus::ErrInvalidParam;
            }
            _ => {
                return TTZipStatus::ErrUnsupportedFeature;
            }
        }

        if !opt.password.is_null() {
            archive_write_set_passphrase(a, opt.password);
            if opt.encryption == TTZipEncryptionMethod::Aes256 {
                let enc_opt = CString::new("zip:encryption=aes256").unwrap();
                archive_write_set_options(a, enc_opt.as_ptr());
            }
        }

        let open_rc = archive_write_open_filename(a, destination_path);
        if open_rc != 0 {
            return TTZipStatus::ErrOpenFailed;
        }

        let mut processed_bytes: u64 = 0;
        let mut buf = match AlignedBuffer::new(64 * 1024) {
            Ok(b) => b,
            Err(st) => return st,
        };

        for (abs_path, rel_name) in entries_to_write {
            let meta = match fs::symlink_metadata(&abs_path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if opt.format == TTZipArchiveFormat::Ar && (meta.is_dir() || meta.file_type().is_symlink()) {
                return TTZipStatus::ErrInvalidParam;
            }

            let entry = archive_entry_new();
            if entry.is_null() {
                return TTZipStatus::ErrOutOfMemory;
            }
            let entry_guard = ArchiveEntryGuard(entry);

            let entry_path_str = if opt.format == TTZipArchiveFormat::Ar {
                Path::new(rel_name.as_str())
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(rel_name.as_str())
            } else {
                rel_name.as_str()
            };
            let rel_c_str = match CString::new(entry_path_str) {
                Ok(c) => c,
                Err(_) => continue,
            };

            archive_entry_set_pathname(entry, rel_c_str.as_ptr());
            archive_entry_set_perm(entry, (meta.permissions().mode() & 0o7777) as mode_t);

            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as time_t)
                .unwrap_or(0);
            archive_entry_set_mtime(entry, mtime, 0);

            if meta.file_type().is_symlink() {
                archive_entry_set_filetype(entry, libc::S_IFLNK as u32);
                archive_entry_set_size(entry, 0);
                if let Ok(link_target) = fs::read_link(&abs_path) {
                    if let Ok(link_c) = CString::new(link_target.to_string_lossy().as_bytes()) {
                        archive_entry_set_symlink(entry, link_c.as_ptr());
                    }
                }
                let r_hdr = archive_write_header(a, entry);
                if r_hdr != 0 {
                    return TTZipStatus::ErrCompressionFailed;
                }
                archive_write_finish_entry(a);
            } else if meta.is_dir() {
                archive_entry_set_filetype(entry, libc::S_IFDIR as u32);
                archive_entry_set_size(entry, 0);
                let r_hdr = archive_write_header(a, entry);
                if r_hdr != 0 {
                    return TTZipStatus::ErrCompressionFailed;
                }
                archive_write_finish_entry(a);
            } else {
                archive_entry_set_filetype(entry, libc::S_IFREG as u32);
                archive_entry_set_size(entry, meta.len() as i64);
                let r_hdr = archive_write_header(a, entry);
                if r_hdr != 0 {
                    return TTZipStatus::ErrCompressionFailed;
                }

                let mut file = match File::open(&abs_path) {
                    Ok(f) => f,
                    Err(_) => return TTZipStatus::ErrFileNotFound,
                };

                loop {
                    let n = match file.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(_) => return TTZipStatus::ErrCompressionFailed,
                    };

                    let written = archive_write_data(a, buf.as_ptr() as *const c_void, n);
                    if written < 0 {
                        return TTZipStatus::ErrCompressionFailed;
                    }
                    processed_bytes = processed_bytes.saturating_add(n as u64);
                }
                archive_write_finish_entry(a);
            }

            drop(entry_guard);

            if let Some(cb) = opt.progress_callback {
                if !cb(processed_bytes, processed_bytes, rel_c_str.as_ptr(), opt.user_data) {
                    return TTZipStatus::Cancelled;
                }
            }
        }

        drop(guard);
        TTZipStatus::Ok
    });

    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}
