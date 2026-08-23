// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

use std::ffi::{CStr, CString};
use std::fs;
use std::path::Path;
use libc::c_void;

use crate::archive::tar::reader::TarArchive;
use crate::ffi::archive_ffi::guards::ArchiveReadGuard;
use crate::ffi::archive_ffi::sys::*;
use crate::sevenz::decoder::archive::SevenZArchive;
use crate::types::TTZipStatus;
use crate::zip::reader::ZipArchive;

/// Extracts a single entry directly into an in-memory buffer without full archive extraction.
pub fn extract_single_entry_memory(
    archive_path: &Path,
    entry_path: Option<&str>,
    entry_index: i64,
    password: Option<&str>,
) -> Result<Vec<u8>, TTZipStatus> {
    if !archive_path.exists() {
        return Err(TTZipStatus::ErrFileNotFound);
    }

    let ext = archive_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // 1. Fast Path: ZIP Random Seek Table
    if ext == "zip" || ext == "cbz" || ext == "jar" || ext == "apk" {
        let mapped = fs::read(archive_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let zip = ZipArchive::open_slice(&mapped)?;

        let target_idx = if entry_index >= 0 {
            entry_index as usize
        } else if let Some(target_p) = entry_path {
            zip.entries()
                .iter()
                .position(|e| e.rel_path == target_p || e.rel_path.trim_start_matches("./") == target_p)
                .ok_or(TTZipStatus::ErrFileNotFound)?
        } else {
            return Err(TTZipStatus::ErrInvalidParam);
        };

        return zip.extract_entry_bytes(target_idx, password);
    }

    // 2. Fast Path: 7z Solid Stream Random Seek Table
    if ext == "7z" || ext == "cb7" {
        let mapped = fs::read(archive_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let sevenz = SevenZArchive::open_slice(&mapped)?;

        let target_idx = if entry_index >= 0 {
            entry_index as usize
        } else if let Some(target_p) = entry_path {
            sevenz
                .seek_index()
                .get_by_path(target_p)
                .map(|loc| loc.file_index)
                .ok_or(TTZipStatus::ErrFileNotFound)?
        } else {
            return Err(TTZipStatus::ErrInvalidParam);
        };

        return sevenz.extract_entry_bytes_stream(target_idx, password);
    }

    // 3. Fast Path: Pure Rust Uncompressed TAR Subslice
    if ext == "tar" || ext == "cbt" {
        let mapped = fs::read(archive_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let tar = TarArchive::open_slice(&mapped)?;

        let target_idx = if entry_index >= 0 {
            entry_index as usize
        } else if let Some(target_p) = entry_path {
            tar.entries()
                .iter()
                .position(|e| e.path.as_ref() == target_p || e.path.as_ref().trim_start_matches("./") == target_p)
                .ok_or(TTZipStatus::ErrFileNotFound)?
        } else {
            return Err(TTZipStatus::ErrInvalidParam);
        };

        return tar.extract_entry_bytes(target_idx).map(|slice| slice.to_vec());
    }

    // 4. General Streaming Path: TAR.GZ / TAR.XZ / TAR.ZST / RAR / ISO (Libarchive Stream-Discarding)
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

        if let Some(pwd) = password {
            if let Ok(c_pwd) = CString::new(pwd) {
                archive_read_add_passphrase(a, c_pwd.as_ptr());
            }
        }

        if archive_read_open_filename(a, arch_c.as_ptr(), 65536) != 0 {
            return Err(TTZipStatus::ErrOpenFailed);
        }

        let mut entry: *mut c_void = std::ptr::null_mut();
        let mut curr_idx: i64 = 0;

        while archive_read_next_header(a, &mut entry) == 0 {
            if entry.is_null() {
                break;
            }

            let raw_path = archive_entry_pathname(entry);
            if raw_path.is_null() {
                archive_read_data_skip(a);
                curr_idx += 1;
                continue;
            }

            let entry_rel_str = match CStr::from_ptr(raw_path).to_str() {
                Ok(s) => s,
                Err(_) => {
                    archive_read_data_skip(a);
                    curr_idx += 1;
                    continue;
                }
            };

            let is_match = if entry_index >= 0 {
                curr_idx == entry_index
            } else if let Some(target_p) = entry_path {
                entry_rel_str == target_p || entry_rel_str.trim_start_matches("./") == target_p
            } else {
                false
            };

            if is_match {
                let size = archive_entry_size(entry).max(0) as usize;
                let mut payload = Vec::with_capacity(size.min(10 * 1024 * 1024));
                let mut chunk = [0u8; 65536];

                loop {
                    let r = archive_read_data(a, chunk.as_mut_ptr() as *mut c_void, chunk.len());
                    if r < 0 {
                        return Err(TTZipStatus::ErrExtractionFailed);
                    }
                    if r == 0 {
                        break;
                    }
                    payload.extend_from_slice(&chunk[..r as usize]);
                }
                return Ok(payload);
            }

            archive_read_data_skip(a);
            curr_idx += 1;
        }

        Err(TTZipStatus::ErrFileNotFound)
    }
}

/// Batch selective extraction to destination directory.
/// Single-pass O(N) stream scan with HashSet lookup and direct-to-disk write.
pub fn extract_selected_entries(
    archive_path: &Path,
    target_paths: &[String],
    destination_dir: &Path,
    options: &crate::types::TTZipExtractOptions,
) -> Result<usize, TTZipStatus> {
    use std::collections::HashSet;
    use std::io::Write;
    use std::ffi::c_void;
    use std::os::unix::io::AsRawFd;
    use crate::ffi::archive_ffi::sys::*;
    use crate::fs::apfs::apfs_preallocate;
    use crate::fs::safe_extract::sanitize_and_validate_path;
    use libc::mode_t;

    if !archive_path.exists() {
        return Err(TTZipStatus::ErrFileNotFound);
    }
    if target_paths.is_empty() {
        return Ok(0);
    }

    if !options.dry_run && !destination_dir.exists() {
        let _ = fs::create_dir_all(destination_dir);
    }

    let target_set: HashSet<&str> = target_paths.iter().map(|s| s.as_str()).collect();
    let mut extracted_count = 0;

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

        if archive_read_open_filename(a, arch_c.as_ptr(), 65536) != 0 {
            return Err(TTZipStatus::ErrOpenFailed);
        }

        let mut entry: *mut c_void = std::ptr::null_mut();

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

            let clean_rel = entry_rel_str.trim_start_matches("./");
            let is_match = target_set.contains(entry_rel_str) || target_set.contains(clean_rel);

            if is_match {
                let file_type = archive_entry_filetype(entry);
                let mode = archive_entry_mode(entry) as u32;
                let is_dir = (file_type & (libc::S_IFMT as mode_t)) == (libc::S_IFDIR as mode_t)
                    || (mode & (libc::S_IFMT as u32)) == (libc::S_IFDIR as u32)
                    || entry_rel_str.ends_with('/');

                let sanitized = sanitize_and_validate_path(destination_dir, entry_rel_str)?;

                if is_dir {
                    if !options.dry_run {
                        fs::create_dir_all(&sanitized).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                    }
                    extracted_count += 1;
                } else {
                    if let Some(parent) = sanitized.parent() {
                        if !parent.exists() && !options.dry_run {
                            let _ = fs::create_dir_all(parent);
                        }
                    }

                    if !options.dry_run {
                        let mut out_file = fs::OpenOptions::new()
                            .create(true)
                            .write(true)
                            .truncate(true)
                            .open(&sanitized)
                            .map_err(|_| TTZipStatus::ErrExtractionFailed)?;

                        let size = archive_entry_size(entry).max(0) as u64;
                        if size > 0 {
                            let _ = apfs_preallocate(out_file.as_raw_fd(), size as i64);
                        }

                        let mut chunk = [0u8; 65536];
                        loop {
                            let r = archive_read_data(a, chunk.as_mut_ptr() as *mut c_void, chunk.len());
                            if r < 0 {
                                return Err(TTZipStatus::ErrExtractionFailed);
                            }
                            if r == 0 {
                                break;
                            }
                            out_file
                                .write_all(&chunk[..r as usize])
                                .map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                        }
                        out_file.flush().map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                    } else {
                        archive_read_data_skip(a);
                    }
                    extracted_count += 1;
                }
            } else {
                archive_read_data_skip(a);
            }
        }

        Ok(extracted_count)
    }
}
