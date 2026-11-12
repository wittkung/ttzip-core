// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Generic container in-place editing engine (SquashFS / ISO / CAB / WIM / DEB / RPM / CPIO / AR) with WAL transaction commit.

use super::wal::WalTransactionJournal;
use super::{InPlaceAction, PartitionedActions};
use crate::ffi::archive_ffi::guards::{ArchiveEntryGuard, ArchiveReadGuard, ArchiveWriteGuard};
use crate::ffi::archive_ffi::sys::*;
use crate::standards::signatures::DetectedFormat;
use crate::types::TTZipStatus;
use libc::{mode_t, time_t};
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
    let plan = PartitionedActions::from_slice(actions);

    let arch_c = path_to_cstring(archive_path)?;
    let shadow_c = path_to_cstring(shadow_path)?;

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
        configure_container_writer_format(writer, format);

        if archive_write_open_filename(writer, shadow_c.as_ptr()) != 0 {
            return Err(TTZipStatus::ErrOpenFailed);
        }

        rewrite_libarchive_entries(reader, writer, &plan)?;
    }

    wal.mark_shadow_written()?;
    wal.mark_committed()?;
    Ok(())
}

pub(crate) fn path_to_cstring(path: &Path) -> Result<CString, TTZipStatus> {
    let s = path.to_str().ok_or(TTZipStatus::ErrInvalidParam)?;
    CString::new(s).map_err(|_| TTZipStatus::ErrInvalidParam)
}

unsafe fn configure_container_writer_format(writer: *mut libc::c_void, format: DetectedFormat) {
    unsafe {
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
    }
}

pub(crate) unsafe fn rewrite_libarchive_entries(
    reader: *mut libc::c_void,
    writer: *mut libc::c_void,
    plan: &PartitionedActions,
) -> Result<(), TTZipStatus> {
    unsafe {
        let mut entry_ptr: *mut libc::c_void = std::ptr::null_mut();
        let mut buf = vec![0u8; 65536];

        while archive_read_next_header(reader, &mut entry_ptr) == 0 {
            if entry_ptr.is_null() {
                break;
            }
            let raw_path = archive_entry_pathname(entry_ptr);
            if raw_path.is_null() {
                continue;
            }
            let path_str = CStr::from_ptr(raw_path).to_string_lossy().into_owned();
            let clean_key = path_str.trim_start_matches('/');

            if plan.deleted.contains(clean_key) {
                archive_read_data_skip(reader);
                continue;
            }

            if let Some(src_path) = plan.replaced.get(clean_key) {
                archive_read_data_skip(reader);
                write_libarchive_file(writer, &path_str, src_path)?;
            } else {
                copy_libarchive_entry_payload(reader, writer, entry_ptr, &mut buf);
            }
        }

        for (rel_path, src_path) in &plan.appended {
            write_libarchive_file(writer, rel_path, src_path)?;
        }
        Ok(())
    }
}

unsafe fn copy_libarchive_entry_payload(
    reader: *mut libc::c_void,
    writer: *mut libc::c_void,
    entry_ptr: *mut libc::c_void,
    buf: &mut [u8],
) {
    unsafe {
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

pub(crate) unsafe fn write_libarchive_file(
    writer: *mut libc::c_void,
    rel_path: &str,
    src_path: &Path,
) -> Result<(), TTZipStatus> {
    unsafe {
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
            use std::io::Read;
            let mut file = fs::File::open(src_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
            let file_len = file
                .metadata()
                .map_err(|_| TTZipStatus::ErrFileNotFound)?
                .len();
            archive_entry_set_filetype(entry, libc::S_IFREG as u32);
            archive_entry_set_perm(entry, (if mode != 0 { mode } else { 0o644 } & 0o777) as mode_t);
            archive_entry_set_size(entry, file_len as i64);
            archive_write_header(writer, entry);
            if file_len > 0 {
                let mut buf = [0u8; 65536];
                let mut remaining = file_len;
                while remaining > 0 {
                    let to_read = (remaining as usize).min(buf.len());
                    let n = file
                        .read(&mut buf[..to_read])
                        .map_err(|_| TTZipStatus::ErrFileNotFound)?;
                    if n == 0 {
                        break;
                    }
                    archive_write_data(writer, buf.as_ptr() as *const libc::c_void, n);
                    remaining -= n as u64;
                }
            }
            archive_write_finish_entry(writer);
        }
        Ok(())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn test_modify_container_via_streaming_rebuild_with_symlink_source() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cpio_path = dir.path().join("base.cpio");
        let shadow_path = dir.path().join("shadow.cpio");
        let wal_path = dir.path().join("tx.wal");

        let initial_file = dir.path().join("a.bin");
        fs::write(&initial_file, b"short").expect("write initial");

        // Build initial cpio container with a.bin via libarchive
        unsafe {
            let w = archive_write_new();
            let _wg = ArchiveWriteGuard(w);
            configure_container_writer_format(w, DetectedFormat::Cpio);
            let c_path = path_to_cstring(&cpio_path).unwrap();
            assert_eq!(archive_write_open_filename(w, c_path.as_ptr()), 0);
            write_libarchive_file(w, "a.bin", &initial_file).expect("write initial entry");
        }

        let real_target = dir.path().join("t.bin");
        let payload = vec![0x5Cu8; 8192];
        fs::write(&real_target, &payload).expect("write real target");
        let symlink_src = dir.path().join("link_payload.bin");
        std::os::unix::fs::symlink(&real_target, &symlink_src).expect("symlink");

        let actions = vec![InPlaceAction::Replace {
            entry_path: "a.bin".to_string(),
            source_path: symlink_src,
        }];
        in_place_edit_generic_container_wal(
            &cpio_path,
            &shadow_path,
            &wal_path,
            DetectedFormat::Cpio,
            &actions,
        )
        .expect("in_place_edit_generic_container_wal with symlink source");

        // Read back entry from shadow.cpio via libarchive and verify full 8192-byte payload
        unsafe {
            let r = archive_read_new();
            let _rg = ArchiveReadGuard(r);
            archive_read_support_format_all(r);
            let s_path = path_to_cstring(&shadow_path).unwrap();
            assert_eq!(archive_read_open_filename(r, s_path.as_ptr(), 65536), 0);
            let mut entry: *mut libc::c_void = std::ptr::null_mut();
            assert_eq!(archive_read_next_header(r, &mut entry), 0);
            let mut extracted = vec![0u8; payload.len()];
            let n = archive_read_data(
                r,
                extracted.as_mut_ptr() as *mut libc::c_void,
                extracted.len(),
            );
            assert_eq!(n, payload.len() as isize);
            assert_eq!(extracted, payload);
        }
    }
}

