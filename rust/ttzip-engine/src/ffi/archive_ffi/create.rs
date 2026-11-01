// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Archive creation C-ABI FFI entry implementation.

use super::guards::{ArchiveEntryGuard, ArchiveWriteGuard};
use super::sys::*;
use crate::archive::unified::create::{
    CreateCommitMode, CreateStageCommitError, with_staged_archive_destination_detailed_outcome,
};
use crate::archive::unified::create_transaction::{
    CreateFileAccessMode, FileFinishSyncPolicy, ResidualPathRole,
};
use crate::ffi::helpers::safe_cstr;
use crate::fs::apfs::AlignedBuffer;
use crate::types::{
    TTZipArchiveFormat, TTZipCreateOptions, TTZipEncryptionMethod, TTZipStatus,
};
use libc::{c_char, c_void, mode_t, time_t};
use std::ffi::CString;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::panic::catch_unwind;
use std::path::{Path, PathBuf};

unsafe extern "C" fn cabi_file_write_cb(
    _a: *mut c_void,
    client_data: *mut c_void,
    buffer: *const c_void,
    length: libc::size_t,
) -> libc::ssize_t {
    unsafe {
        if client_data.is_null() || buffer.is_null() {
            return -1;
        }
        let file = &mut *(client_data as *mut File);
        let slice = std::slice::from_raw_parts(buffer as *const u8, length);
        match file.write_all(slice) {
            Ok(()) => length as libc::ssize_t,
            Err(_) => -1,
        }
    }
}

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

unsafe fn configure_cabi_filtered_stream_format(
    a: *mut c_void,
    format: TTZipArchiveFormat,
    is_pure_stream: bool,
) -> bool {
    unsafe {
        match format {
            TTZipArchiveFormat::TarGz | TTZipArchiveFormat::Gzip => {
                set_raw_or_pax(a, is_pure_stream);
                archive_write_add_filter_gzip(a);
                true
            }
            TTZipArchiveFormat::TarBz2 | TTZipArchiveFormat::Bzip2 => {
                set_raw_or_pax(a, is_pure_stream);
                archive_write_add_filter_bzip2(a);
                true
            }
            TTZipArchiveFormat::TarXz | TTZipArchiveFormat::Xz => {
                set_raw_or_pax(a, is_pure_stream);
                archive_write_add_filter_xz(a);
                true
            }
            TTZipArchiveFormat::TarZstd | TTZipArchiveFormat::Zstd => {
                set_raw_or_pax(a, is_pure_stream);
                archive_write_add_filter_zstd(a);
                true
            }
            TTZipArchiveFormat::TarLz4 | TTZipArchiveFormat::Lz4 => {
                set_raw_or_pax(a, is_pure_stream);
                archive_write_add_filter_lz4(a);
                true
            }
            TTZipArchiveFormat::TarLzip | TTZipArchiveFormat::Lzip => {
                set_raw_or_pax(a, is_pure_stream);
                archive_write_add_filter_lzip(a);
                true
            }
            TTZipArchiveFormat::TarLrzip | TTZipArchiveFormat::Lrzip => {
                set_raw_or_pax(a, is_pure_stream);
                archive_write_add_filter_lrzip(a);
                true
            }
            _ => false,
        }
    }
}

unsafe fn configure_cabi_archive_format(
    a: *mut c_void,
    opt: &TTZipCreateOptions,
    is_pure_stream: bool,
) -> Result<(), TTZipStatus> {
    unsafe {
        if !configure_cabi_filtered_stream_format(a, opt.format, is_pure_stream) {
            match opt.format {
                TTZipArchiveFormat::Zip | TTZipArchiveFormat::Auto => {
                    archive_write_set_format_zip(a);
                }
                TTZipArchiveFormat::Tar => {
                    archive_write_set_format_pax_restricted(a);
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
                _ => return Err(TTZipStatus::ErrUnsupportedFeature),
            }
        }
        if !opt.password.is_null() {
            archive_write_set_passphrase(a, opt.password);
            if opt.encryption == TTZipEncryptionMethod::Aes256 {
                let enc_opt = CString::new("zip:encryption=aes256").unwrap();
                archive_write_set_options(a, enc_opt.as_ptr());
            }
        }
        // Match `archive_write_open_filename` / `archive_write_open_fd` regular-file behavior
        // so `archive_write_open2`'s outer `client_filter` does not zero-pad to 10,240 bytes
        // after the inner compressor/container trailer finishes.
        if archive_write_set_bytes_in_last_block(a, 1) != 0 {
            return Err(TTZipStatus::ErrCompressionFailed);
        }
        Ok(())
    }
}

unsafe fn set_raw_or_pax(a: *mut c_void, is_pure_stream: bool) {
    unsafe {
        if is_pure_stream {
            archive_write_set_format_raw(a);
        } else {
            archive_write_set_format_pax_restricted(a);
        }
    }
}

unsafe fn write_cabi_single_entry(
    a: *mut c_void,
    abs_path: &Path,
    rel_name: &str,
    opt: &TTZipCreateOptions,
    buf: &mut AlignedBuffer,
    processed_bytes: &mut u64,
) -> Result<Option<CString>, TTZipStatus> {
    let Ok(meta) = fs::symlink_metadata(abs_path) else {
        return Ok(None);
    };
    unsafe {
        let entry = archive_entry_new();
        if entry.is_null() {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let _entry_guard = ArchiveEntryGuard(entry);
        let entry_path_str = if opt.format == TTZipArchiveFormat::Ar {
            Path::new(rel_name)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(rel_name)
        } else {
            rel_name
        };
        let Ok(rel_c_str) = CString::new(entry_path_str) else {
            return Ok(None);
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
            if let Ok(link_target) = fs::read_link(abs_path) {
                if let Ok(link_c) = CString::new(link_target.to_string_lossy().as_bytes()) {
                    archive_entry_set_symlink(entry, link_c.as_ptr());
                }
            }
            if archive_write_header(a, entry) != 0 {
                return Err(TTZipStatus::ErrCompressionFailed);
            }
            archive_write_finish_entry(a);
        } else if meta.is_dir() {
            archive_entry_set_filetype(entry, libc::S_IFDIR as u32);
            archive_entry_set_size(entry, 0);
            if archive_write_header(a, entry) != 0 {
                return Err(TTZipStatus::ErrCompressionFailed);
            }
            archive_write_finish_entry(a);
        } else {
            archive_entry_set_filetype(entry, libc::S_IFREG as u32);
            archive_entry_set_size(entry, meta.len() as i64);
            if archive_write_header(a, entry) != 0 {
                return Err(TTZipStatus::ErrCompressionFailed);
            }
            stream_cabi_file_payload(a, abs_path, buf, processed_bytes)?;
            archive_write_finish_entry(a);
        }
        Ok(Some(rel_c_str))
    }
}

unsafe fn stream_cabi_file_payload(
    a: *mut c_void,
    abs_path: &Path,
    buf: &mut AlignedBuffer,
    processed_bytes: &mut u64,
) -> Result<(), TTZipStatus> {
    let mut src_file = File::open(abs_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    loop {
        let n = match src_file.read(buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(TTZipStatus::ErrCompressionFailed),
        };
        if unsafe { archive_write_data(a, buf.as_ptr() as *const c_void, n) } < 0 {
            return Err(TTZipStatus::ErrCompressionFailed);
        }
        *processed_bytes = processed_bytes.saturating_add(n as u64);
    }
    Ok(())
}

unsafe fn write_cabi_entries_loop(
    a: *mut c_void,
    txn_ctx: &crate::archive::unified::create_transaction::CreateTransactionContext,
    entries_to_write: &[(PathBuf, String)],
    opt: &TTZipCreateOptions,
    buf: &mut AlignedBuffer,
) -> Result<(), TTZipStatus> {
    let mut processed_bytes: u64 = 0;
    for (abs_path, rel_name) in entries_to_write {
        if let Some(rel_c_str) =
            unsafe { write_cabi_single_entry(a, abs_path, rel_name, opt, buf, &mut processed_bytes)? }
        {
            if let Some(cb) = opt.progress_callback {
                if !unsafe { cb(processed_bytes, processed_bytes, rel_c_str.as_ptr(), opt.user_data) }
                {
                    txn_ctx.mark_cancellation_requested();
                    return Err(TTZipStatus::Cancelled);
                }
            }
        }
    }
    Ok(())
}

unsafe fn run_cabi_staged_write(
    staged_dest: &Path,
    txn_ctx: &crate::archive::unified::create_transaction::CreateTransactionContext,
    entries_to_write: &[(PathBuf, String)],
    opt: &TTZipCreateOptions,
    is_pure_stream: bool,
) -> Result<(), TTZipStatus> {
    unsafe {
        let a = archive_write_new();
        if a.is_null() {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let guard = ArchiveWriteGuard(a);
        configure_cabi_archive_format(a, opt, is_pure_stream)?;

        let (mut file, file_guard) = crate::fs::FileAbortGuard::create_staged_file_in_txn(
            staged_dest,
            CreateFileAccessMode::WriteOnly,
            Some(txn_ctx),
            ResidualPathRole::StagedArtifact,
        )
        .map_err(|_| TTZipStatus::ErrOpenFailed)?;

        if archive_write_open2(
            a,
            &mut file as *mut File as *mut c_void,
            None,
            Some(cabi_file_write_cb),
            None,
            None,
        ) != 0
        {
            let _ = guard.close_and_free();
            let _ = file_guard.finish_file_and_settle_in_txn(
                file,
                Err((TTZipStatus::ErrOpenFailed, "archive_write_open2 failed".to_string())),
                None,
                FileFinishSyncPolicy::None,
            );
            return Err(TTZipStatus::ErrOpenFailed);
        }

        let loop_res = match AlignedBuffer::new(64 * 1024) {
            Ok(mut buf) => write_cabi_entries_loop(a, txn_ctx, entries_to_write, opt, &mut buf),
            Err(st) => Err(st),
        };

        let close_arch_res = guard.close_and_free();
        let prior_res = match (loop_res, close_arch_res) {
            (Err(st), _) => Err((st, format!("cabi libarchive write failed: {st:?}"))),
            (Ok(()), Err(rc)) => Err((
                TTZipStatus::ErrCompressionFailed,
                format!("archive_write_close failed with rc={rc}"),
            )),
            (Ok(()), Ok(())) => Ok(()),
        };
        file_guard
            .finish_file_and_settle_in_txn(file, prior_res, None, FileFinishSyncPolicy::None)
            .map(|_| ())
            .map_err(|e| e.primary_status)
    }
}
std::thread_local! {
    static STAGED_FFI_ENCRYPT_FILE_NAMES: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// C-ABI v2 unified archive creation with explicit `encrypt_file_names_mode`
/// (`1` => `Some(true)`, `0` => `Some(false)`, `< 0` => `None`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ttzip_rust_create_archive_v2(
    source_paths: *const *const c_char,
    source_count: usize,
    destination_path: *const c_char,
    options: *const TTZipCreateOptions,
    encrypt_file_names_mode: i32,
) -> TTZipStatus {
    unsafe {
        let encrypt_file_names = match encrypt_file_names_mode {
            1 => Some(true),
            0 => Some(false),
            _ => None,
        };
        STAGED_FFI_ENCRYPT_FILE_NAMES.with(|slot| slot.set(encrypt_file_names));
        ttzip_rust_create_archive(source_paths, source_count, destination_path, options)
    }
}

unsafe fn validate_cabi_encryption_options(
    opt: &TTZipCreateOptions,
    encrypt_file_names: Option<bool>,
) -> Result<(), TTZipStatus> {
    let has_non_empty_password = if opt.password.is_null() {
        false
    } else {
        match unsafe { safe_cstr(opt.password) } {
            Ok(s) if !s.is_empty() => true,
            _ => return Err(TTZipStatus::ErrInvalidPassword),
        }
    };
    if (opt.encryption != TTZipEncryptionMethod::None || encrypt_file_names == Some(true))
        && !has_non_empty_password
    {
        return Err(TTZipStatus::ErrInvalidPassword);
    }
    if encrypt_file_names == Some(true) && opt.format != TTZipArchiveFormat::SevenZip {
        return Err(TTZipStatus::ErrUnsupportedFeature);
    }
    Ok(())
}

type CollectedCabiCreateSources = (Vec<PathBuf>, Vec<(PathBuf, String)>);

unsafe fn validate_and_collect_cabi_sources(
    source_paths: *const *const c_char,
    source_count: usize,
    dest_p: &Path,
    opt: &TTZipCreateOptions,
    encrypt_file_names: Option<bool>,
) -> Result<CollectedCabiCreateSources, TTZipStatus> {
    unsafe {
        validate_cabi_encryption_options(opt, encrypt_file_names)?;

        if let Some(parent) = dest_p.parent() {
            if !parent.exists() {
                let _ = fs::create_dir_all(parent);
            }
        }

        let mut raw_sources: Vec<PathBuf> = Vec::with_capacity(source_count);
        let mut entries_to_write: Vec<(PathBuf, String)> = Vec::new();
        for i in 0..source_count {
            let src_c = *source_paths.add(i);
            let Ok(src_str) = safe_cstr(src_c) else {
                continue;
            };
            let src_path = Path::new(src_str);
            if !src_path.exists() && fs::symlink_metadata(src_path).is_err() {
                return Err(TTZipStatus::ErrFileNotFound);
            }
            raw_sources.push(src_path.to_path_buf());
            let base_parent = src_path.parent().unwrap_or(src_path);
            let _ = collect_entries_recursive(base_parent, src_path, &mut entries_to_write);
        }

        if entries_to_write.is_empty() {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        Ok((raw_sources, entries_to_write))
    }
}

fn is_tar_compound_filename(dest_lower: &str) -> bool {
    const SUFFIXES: &[&str] = &[
        ".tgz", ".tbz2", ".tbz", ".txz", ".tzst", ".tlz4", ".tlz", ".tlrz",
    ];
    dest_lower.contains(".tar.") || SUFFIXES.iter().any(|ext| dest_lower.ends_with(ext))
}

fn compute_cabi_is_pure_stream(
    dest_p: &Path,
    opt: &TTZipCreateOptions,
    entries_to_write: &[(PathBuf, String)],
) -> Result<bool, TTZipStatus> {
    let is_single_regular_file = entries_to_write.len() == 1
        && fs::symlink_metadata(&entries_to_write[0].0)
            .map(|m| m.is_file() && !m.file_type().is_symlink())
            .unwrap_or(false);
    let dest_lower = dest_p
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    let is_pure_stream = matches!(
        opt.format,
        TTZipArchiveFormat::Gzip
            | TTZipArchiveFormat::Bzip2
            | TTZipArchiveFormat::Xz
            | TTZipArchiveFormat::Zstd
            | TTZipArchiveFormat::Lz4
            | TTZipArchiveFormat::Lzip
            | TTZipArchiveFormat::Lrzip
    ) && !is_tar_compound_filename(&dest_lower);

    if is_pure_stream && !is_single_regular_file {
        return Err(TTZipStatus::ErrInvalidParam);
    }
    if opt.format == TTZipArchiveFormat::Rar {
        return Err(TTZipStatus::ErrInvalidParam);
    }
    if opt.format == TTZipArchiveFormat::Ar {
        for (abs_path, _) in entries_to_write {
            if let Ok(meta) = fs::symlink_metadata(abs_path) {
                if meta.is_dir() || meta.file_type().is_symlink() {
                    return Err(TTZipStatus::ErrInvalidParam);
                }
            }
        }
    }
    Ok(is_pure_stream)
}

/// C-ABI exported unified archive creation.
///
/// Compresses `source_paths` into `destination_path` according to `options`.
#[unsafe(no_mangle)]
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
        let (raw_sources, entries_to_write) = match unsafe {
            validate_and_collect_cabi_sources(
                source_paths,
                source_count,
                dest_p,
                opt,
                encrypt_file_names,
            )
        } {
            Ok(pair) => pair,
            Err(st) => return st,
        };

        if opt.format == TTZipArchiveFormat::SevenZip && encrypt_file_names.is_some() {
            return match crate::archive::unified::create::create_archive_explicit_with_files_detailed_opts(
                &entries_to_write,
                dest_p,
                opt,
                0,
                encrypt_file_names,
            ) {
                Ok(_) => TTZipStatus::Ok,
                Err(e) => e.log_and_into_status(),
            };
        }

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

        let is_pure_stream = match compute_cabi_is_pure_stream(dest_p, opt, &entries_to_write) {
            Ok(v) => v,
            Err(st) => return st,
        };

        match with_staged_archive_destination_detailed_outcome(
            dest_p,
            CreateCommitMode::SingleFile,
            |staged_dest, txn_ctx| unsafe {
                run_cabi_staged_write(staged_dest, txn_ctx, &entries_to_write, opt, is_pure_stream)
            },
            || false,
        ) {
            Ok(_) => TTZipStatus::Ok,
            Err(err) => CreateStageCommitError::log_and_into_status(err),
        }
    });

    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}
