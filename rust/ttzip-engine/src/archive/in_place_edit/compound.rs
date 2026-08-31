// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Compound TAR stream in-place editing engine (TAR.GZ / TAR.BZ2 / TAR.XZ / TAR.ZSTD) via single-pass in-memory micro-buffering.

use super::container::write_libarchive_file;
use super::InPlaceAction;
use crate::ffi::archive_ffi::guards::{ArchiveReadGuard, ArchiveWriteGuard};
use crate::ffi::archive_ffi::sys::*;
use crate::standards::signatures::CompoundFormat;
use crate::types::{record_execution_provenance, TTZipEngineTag, TTZipExecutionProvenance, TTZipStatus};
use libc::c_void;
use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, CString};
use std::fs;
use std::path::Path;

/// Modifies a compound TAR stream (TAR.GZ / TAR.BZ2 / TAR.XZ / TAR.ZSTD) in-memory without disk extraction.
pub fn in_place_edit_compound_stream(
    archive_path: &Path,
    shadow_path: &Path,
    compound_format: CompoundFormat,
    actions: &[InPlaceAction],
) -> Result<(), TTZipStatus> {
    match compound_format {
        CompoundFormat::TarGz
        | CompoundFormat::TarBz2
        | CompoundFormat::TarXz
        | CompoundFormat::TarZstd
        | CompoundFormat::TarLz4 => {}
        _ => return Err(TTZipStatus::ErrUnsupportedFeature),
    }

    let source = crate::archive::source::open_archive_source(archive_path)?;
    let mapped = source.as_slice().ok_or(TTZipStatus::ErrOpenFailed)?;

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

        if archive_read_open_memory(reader, mapped.as_ptr() as *const c_void, mapped.len()) != 0 {
            return Err(TTZipStatus::ErrOpenFailed);
        }

        let writer = archive_write_new();
        if writer.is_null() {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let _w_guard = ArchiveWriteGuard(writer);
        archive_write_set_format_pax_restricted(writer);

        match compound_format {
            CompoundFormat::TarGz => {
                archive_write_add_filter_gzip(writer);
            }
            CompoundFormat::TarBz2 => {
                archive_write_add_filter_bzip2(writer);
            }
            CompoundFormat::TarXz => {
                archive_write_add_filter_xz(writer);
            }
            CompoundFormat::TarZstd => {
                archive_write_add_filter_zstd(writer);
            }
            CompoundFormat::TarLz4 => {
                archive_write_add_filter_lz4(writer);
            }
            _ => {
                return Err(TTZipStatus::ErrUnsupportedFeature);
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

    let prov = TTZipExecutionProvenance {
        engine_tag: TTZipEngineTag::RustTarStreamEngine,
        compressed_bytes: fs::metadata(shadow_path).map(|m| m.len()).unwrap_or(0),
        is_fallback: false,
        ..Default::default()
    };
    record_execution_provenance(prov);

    Ok(())
}
