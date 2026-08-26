// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Generic container in-place editing engine (SquashFS / ISO / CAB / WIM / DEB / RPM / CPIO / AR) with WAL transaction commit.

use super::wal::WalTransactionJournal;
use super::InPlaceAction;
use crate::ffi::archive_ffi::guards::{ArchiveEntryGuard, ArchiveReadGuard, ArchiveWriteGuard};
use crate::ffi::archive_ffi::sys::*;
use crate::standards::signatures::DetectedFormat;
use crate::types::TTZipStatus;
use libc::{mode_t, time_t};
use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, CString};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Modifies generic containers (SquashFS, ISO, CAB, WIM, DEB, RPM, CPIO, AR) with WAL transaction commit.
pub fn in_place_edit_generic_container_wal(
    archive_path: &Path,
    shadow_path: &Path,
    wal_path: &Path,
    format: DetectedFormat,
    actions: &[InPlaceAction],
) -> Result<(), TTZipStatus> {
    let wal = WalTransactionJournal::begin(wal_path, archive_path, shadow_path, actions.len())?;

    let mut deleted = HashSet::new();
    let mut replaced = HashMap::new();
    let mut appended = Vec::new();

    for action in actions {
        match action {
            InPlaceAction::Delete { entry_path } => {
                deleted.insert(entry_path.trim_start_matches('/').to_string());
            }
            InPlaceAction::Replace { entry_path, source_path } => {
                replaced.insert(entry_path.trim_start_matches('/').to_string(), source_path.clone());
            }
            InPlaceAction::Append { entry_path, source_path } => {
                appended.push((entry_path.trim_start_matches('/').to_string(), source_path.clone()));
            }
        }
    }

    let arch_c = CString::new(archive_path.to_str().ok_or(TTZipStatus::ErrInvalidParam)?)
        .map_err(|_| TTZipStatus::ErrInvalidParam)?;
    let shadow_c = CString::new(shadow_path.to_str().ok_or(TTZipStatus::ErrInvalidParam)?)
        .map_err(|_| TTZipStatus::ErrInvalidParam)?;

    unsafe {
        let reader = archive_read_new();
        if reader.is_null() {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let _r_guard = ArchiveReadGuard(reader);
        archive_read_support_format_all(reader);
        archive_read_support_filter_all(reader);

        if archive_read_open_filename(reader, arch_c.as_ptr(), 65536) != 0 {
            return Err(TTZipStatus::ErrOpenFailed);
        }

        let writer = archive_write_new();
        if writer.is_null() {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let _w_guard = ArchiveWriteGuard(writer);

        match format {
            DetectedFormat::Ar => {
                archive_write_set_format_ar_bsd(writer);
            }
            DetectedFormat::Iso => {
                archive_write_set_format_iso9660(writer);
            }
            DetectedFormat::Xar => {
                archive_write_set_format_xar(writer);
            }
            _ => {
                archive_write_set_format_pax_restricted(writer);
            }
        }

        if archive_write_open_filename(writer, shadow_c.as_ptr()) != 0 {
            return Err(TTZipStatus::ErrOpenFailed);
        }

        let mut entry_ptr: *mut libc::c_void = std::ptr::null_mut();
        let mut buf = vec![0u8; 65536];

        while archive_read_next_header(reader, &mut entry_ptr) == 0 {
            if entry_ptr.is_null() {
                break;
            }
            let raw_path = archive_entry_pathname(entry_ptr);
            let path_str = if !raw_path.is_null() {
                CStr::from_ptr(raw_path).to_string_lossy().into_owned()
            } else {
                continue;
            };

            let clean_key = path_str.trim_start_matches('/').to_string();
            if deleted.contains(&clean_key) {
                archive_read_data_skip(reader);
                continue;
            }

            if let Some(src_path) = replaced.get(&clean_key) {
                archive_read_data_skip(reader);
                write_libarchive_file(writer, &path_str, src_path)?;
            } else {
                archive_write_header(writer, entry_ptr);
                loop {
                    let n = archive_read_data(reader, buf.as_mut_ptr() as *mut libc::c_void, buf.len());
                    if n <= 0 {
                        break;
                    }
                    archive_write_data(writer, buf.as_ptr() as *const libc::c_void, n as usize);
                }
                archive_write_finish_entry(writer);
            }
        }

        for (rel_path, src_path) in &appended {
            write_libarchive_file(writer, rel_path, src_path)?;
        }
    }

    wal.mark_shadow_written()?;
    wal.mark_committed()?;
    Ok(())
}

pub(crate) unsafe fn write_libarchive_file(
    writer: *mut libc::c_void,
    rel_path: &str,
    src_path: &Path,
) -> Result<(), TTZipStatus> {
    let meta = fs::symlink_metadata(src_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let is_dir = meta.is_dir();
    let mode = meta.permissions().mode();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as time_t)
        .unwrap_or(0);

    let entry = archive_entry_new();
    if entry.is_null() {
        return Err(TTZipStatus::ErrOutOfMemory);
    }
    let _e_guard = ArchiveEntryGuard(entry);

    let rel_c = CString::new(rel_path).map_err(|_| TTZipStatus::ErrInvalidParam)?;
    archive_entry_set_pathname(entry, rel_c.as_ptr());
    archive_entry_set_mtime(entry, mtime, 0);

    if is_dir {
        archive_entry_set_filetype(entry, libc::S_IFDIR as u32);
        archive_entry_set_perm(entry, (if mode != 0 { mode } else { 0o755 } & 0o777) as mode_t);
        archive_entry_set_size(entry, 0);
        archive_write_header(writer, entry);
        archive_write_finish_entry(writer);
    } else {
        let data = fs::read(src_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
        archive_entry_set_filetype(entry, libc::S_IFREG as u32);
        archive_entry_set_perm(entry, (if mode != 0 { mode } else { 0o644 } & 0o777) as mode_t);
        archive_entry_set_size(entry, data.len() as i64);
        archive_write_header(writer, entry);
        if !data.is_empty() {
            archive_write_data(writer, data.as_ptr() as *const libc::c_void, data.len());
        }
        archive_write_finish_entry(writer);
    }
    Ok(())
}
