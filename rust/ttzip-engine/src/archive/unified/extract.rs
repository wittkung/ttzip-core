// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

//! Archive extraction lifecycle submodule for Unified Orchestrator.

use std::ffi::{CStr, CString};
use std::fs;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::Path;

use crate::archive::split::{detect_volume_chain, VirtualMultiVolumeReader};
use crate::ffi::archive_ffi::guards::ArchiveReadGuard;
use crate::ffi::archive_ffi::sys::*;
use crate::fs::apfs::apfs_preallocate;
use crate::fs::safe_extract::{sanitize_and_validate_path, SafeExtractEngine};
use crate::types::{TTZipExtractOptions, TTZipStatus};
use libc::{c_void, mode_t};

/// Extracts an archive to destination directory with security verification and APFS preallocation.
pub fn extract_archive(
    archive_path: &Path,
    destination_path: &Path,
    options: &TTZipExtractOptions,
) -> Result<(), TTZipStatus> {
    extract_archive_with_metrics(archive_path, destination_path, options).map(|_| ())
}

/// Extracts an archive and returns total extracted uncompressed bytes.
pub fn extract_archive_with_metrics(
    archive_path: &Path,
    destination_path: &Path,
    options: &TTZipExtractOptions,
) -> Result<u64, TTZipStatus> {
    if !archive_path.exists() {
        return Err(TTZipStatus::ErrFileNotFound);
    }

    if !options.dry_run {
        fs::create_dir_all(destination_path).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
    }

    // Multi-Volume detection
    let volume_chain = detect_volume_chain(archive_path).unwrap_or_else(|_| vec![archive_path.to_path_buf()]);
    if volume_chain.len() > 1 {
        let virtual_reader = VirtualMultiVolumeReader::from_volumes(volume_chain)
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let reader = crate::archive::stream_adapter::read::ArchiveStreamReader::open_seekable(virtual_reader, 65536)?;
        let raw_a = reader.as_raw_archive();
        if !options.password.is_null() {
            if let Ok(p_str) = unsafe { CStr::from_ptr(options.password).to_str() } {
                if !p_str.is_empty() {
                    unsafe { archive_read_add_passphrase(raw_a, options.password); }
                }
            }
        }
        return unsafe { extract_from_archive_handle(raw_a, archive_path, destination_path, options) };
    }

    // 1. Fast-Path: Pure Safe Rust Streaming 7z Decoder with Bounded RSS
    if let Ok(source) = crate::archive::source::open_archive_source(archive_path) {
        if let Some(mapped) = source.as_slice() {
            let pwd_str = if !options.password.is_null() {
                unsafe { CStr::from_ptr(options.password).to_str().ok() }
            } else {
                None
            };
            if let Ok(sevenz) = crate::sevenz::decoder::archive::SevenZArchive::open_slice_with_password(mapped, pwd_str) {
                if let Ok(report) = sevenz.extract_all(destination_path, options) {
                    return Ok(report.total_uncompressed_bytes);
                }
            }
            if let Ok(zip_archive) = crate::zip::reader::ZipArchive::open_slice(mapped) {
                if let Ok(report) = zip_archive.extract_all(destination_path, options) {
                    return Ok(report.total_uncompressed_bytes);
                }
            }
        }
    }

    let arch_c = CString::new(archive_path.to_str().ok_or(TTZipStatus::ErrInvalidParam)?)
        .map_err(|_| TTZipStatus::ErrInvalidParam)?;

    unsafe {
        let a = archive_read_new();
        if a.is_null() {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let _guard = ArchiveReadGuard(a);

        archive_read_support_format_all(a);
        archive_read_support_filter_all(a);

        if !options.password.is_null() {
            if let Ok(p_str) = CStr::from_ptr(options.password).to_str() {
                if !p_str.is_empty() {
                    archive_read_add_passphrase(a, options.password);
                }
            }
        }

        let open_rc = archive_read_open_filename(a, arch_c.as_ptr(), 65536);
        if open_rc != 0 {
            return Err(TTZipStatus::ErrOpenFailed);
        }

        extract_from_archive_handle(a, archive_path, destination_path, options)
    }
}

unsafe fn extract_from_archive_handle(
    a: *mut c_void,
    archive_path: &Path,
    destination_path: &Path,
    options: &TTZipExtractOptions,
) -> Result<u64, TTZipStatus> {
    let mut engine = SafeExtractEngine::new();
    let mut entry: *mut c_void = std::ptr::null_mut();
    let mut total_processed: u64 = 0;
    let mut buf = vec![0u8; 64 * 1024];
    let bomb_guard = crate::security::path_sanitizer::ExpansionRatioGuard::default();

    while archive_read_next_header(a, &mut entry) == 0 {
        if entry.is_null() {
            break;
        }
        let raw_path = archive_entry_pathname(entry);
        if raw_path.is_null() {
            archive_read_data_skip(a);
            continue;
        }

        let entry_rel_str = match CStr::from_ptr(raw_path).to_str() {
            Ok(s) => s,
            Err(_) => {
                archive_read_data_skip(a);
                continue;
            }
        };

        // Invariant II: ZipSlip & Security Path Sanitization
        let target_path = sanitize_and_validate_path(destination_path, entry_rel_str)?;

        let size = archive_entry_size(entry).max(0) as u64;
        let mode = archive_entry_mode(entry) as u32;
        let mtime = archive_entry_mtime(entry) as i64;
        let filetype = archive_entry_filetype(entry);
        let is_symlink = (filetype & (libc::S_IFMT as mode_t)) == (libc::S_IFLNK as mode_t);
        let is_dir = (filetype & (libc::S_IFMT as mode_t)) == (libc::S_IFDIR as mode_t)
            || (mode & (libc::S_IFMT as u32)) == (libc::S_IFDIR as u32)
            || entry_rel_str.ends_with('/');

        if options.dry_run {
            if !is_dir && !is_symlink && size > 0 {
                let r = archive_read_data(
                    a,
                    buf.as_mut_ptr() as *mut c_void,
                    buf.len().min(size as usize),
                );
                if r < 0 {
                    return Err(TTZipStatus::ErrInvalidPassword);
                }
            } else {
                archive_read_data_skip(a);
            }
            total_processed = total_processed.saturating_add(size);
            if let Some(cb) = options.progress_callback {
                if !cb(total_processed, total_processed, raw_path, options.user_data) {
                    return Err(TTZipStatus::Cancelled);
                }
            }
            continue;
        }

        if is_symlink {
            let symlink_raw = archive_entry_symlink(entry);
            if !symlink_raw.is_null() {
                if let Ok(symlink_target) = CStr::from_ptr(symlink_raw).to_str() {
                    // Reject absolute symlink targets and targets resolving outside destination
                    if symlink_target.starts_with('/') || symlink_target.starts_with('\\') {
                        return Err(TTZipStatus::ErrSecurityViolation);
                    }
                    let parent_dir = target_path.parent().unwrap_or(destination_path);
                    let resolved_target = parent_dir.join(symlink_target);
                    if sanitize_and_validate_path(destination_path, &resolved_target.to_string_lossy()).is_err() {
                        return Err(TTZipStatus::ErrSecurityViolation);
                    }
                    crate::fs::safe_extract::validate_no_intermediate_symlinks(destination_path, &target_path)?;
                    if target_path.exists() || fs::symlink_metadata(&target_path).is_ok() {
                        let _ = fs::remove_file(&target_path);
                    }
                    if let Some(parent) = target_path.parent() {
                        if !parent.exists() {
                            let _ = fs::create_dir_all(parent);
                        }
                    }
                    let _ = std::os::unix::fs::symlink(symlink_target, &target_path);
                }
            }
            archive_read_data_skip(a);
        } else if is_dir {
            if engine.create_dir_all_secure(&target_path, mode, mtime).is_err() {
                return Err(TTZipStatus::ErrExtractionFailed);
            }
            archive_read_data_skip(a);
        } else {
            if let Some(parent) = target_path.parent() {
                if !parent.exists() {
                    let _ = engine.create_dir_all_secure(parent, 0o755, mtime);
                }
            }

            let mut file = match engine.create_file_secure(&target_path, mode, mtime, options.overwrite_existing) {
                Ok(f) => f,
                Err(_) => return Err(TTZipStatus::ErrExtractionFailed),
            };

            if size > 0 {
                let _ = apfs_preallocate(file.as_raw_fd(), size as i64);
            }

            let mut remaining = size;
            while remaining > 0 {
                let to_read = buf.len().min(remaining as usize);
                let bytes_read = archive_read_data(a, buf.as_mut_ptr() as *mut c_void, to_read);
                if bytes_read < 0 {
                    return Err(TTZipStatus::ErrExtractionFailed);
                }
                if bytes_read == 0 {
                    break;
                }
                if file.write_all(&buf[..bytes_read as usize]).is_err() {
                    return Err(TTZipStatus::ErrExtractionFailed);
                }
                remaining = remaining.saturating_sub(bytes_read as u64);
                total_processed = total_processed.saturating_add(bytes_read as u64);
                bomb_guard.check(total_processed, total_processed / 1000 + 1)?;
                if let Some(cb) = options.progress_callback {
                    if !cb(total_processed, total_processed, raw_path, options.user_data) {
                        return Err(TTZipStatus::Cancelled);
                    }
                }
            }
        }
    }

    if total_processed == 0 && !options.dry_run {
        if let Ok(source) = crate::archive::source::open_archive_source(archive_path) {
            if let Some(mapped) = source.as_slice() {
                if let Ok(sevenz) = crate::sevenz::decoder::archive::SevenZArchive::open_slice(mapped) {
                    if let Ok(report) = sevenz.extract_all(destination_path, options) {
                        return Ok(report.total_uncompressed_bytes);
                    }
                }
            }
        }
        if archive_path.metadata().map(|m| m.len() > 0).unwrap_or(false) {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
    }

    if !options.dry_run {
        engine.apply_deferred_metadata(options.preserve_permissions)?;
    }

    Ok(total_processed)
}
