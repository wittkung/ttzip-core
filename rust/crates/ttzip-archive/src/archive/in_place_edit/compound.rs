// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Compound TAR stream in-place editing engine (TAR.GZ / TAR.BZ2 / TAR.XZ / TAR.ZSTD) via single-pass in-memory micro-buffering.

use super::container::{path_to_cstring, rewrite_libarchive_entries};
use super::{InPlaceAction, PartitionedActions};
use crate::ffi::archive_ffi::guards::{ArchiveReadGuard, ArchiveWriteGuard};
use crate::ffi::archive_ffi::sys::*;
use crate::standards::signatures::CompoundFormat;
use crate::types::{record_execution_provenance, TTZipEngineTag, TTZipExecutionProvenance, TTZipStatus};
use libc::c_void;
use std::fs;
use std::path::Path;

/// Modifies a compound TAR stream (TAR.GZ / TAR.BZ2 / TAR.XZ / TAR.ZSTD) in-memory without disk extraction.
pub fn in_place_edit_compound_stream(
    archive_path: &Path,
    shadow_path: &Path,
    compound_format: CompoundFormat,
    actions: &[InPlaceAction],
) -> Result<(), TTZipStatus> {
    validate_compound_format(compound_format)?;

    let source = crate::archive::source::open_archive_source(archive_path)?;
    let mapped = source.as_slice().ok_or(TTZipStatus::ErrOpenFailed)?;
    let plan = PartitionedActions::from_slice(actions);
    let shadow_c = path_to_cstring(shadow_path)?;

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
        apply_compound_writer_filter(writer, compound_format)?;

        if archive_write_open_filename(writer, shadow_c.as_ptr()) != 0 {
            return Err(TTZipStatus::ErrOpenFailed);
        }

        rewrite_libarchive_entries(reader, writer, &plan)?;
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

fn validate_compound_format(compound_format: CompoundFormat) -> Result<(), TTZipStatus> {
    match compound_format {
        CompoundFormat::TarGz
        | CompoundFormat::TarBz2
        | CompoundFormat::TarXz
        | CompoundFormat::TarZstd
        | CompoundFormat::TarLz4 => Ok(()),
        _ => Err(TTZipStatus::ErrUnsupportedFeature),
    }
}

unsafe fn apply_compound_writer_filter(
    writer: *mut c_void,
    compound_format: CompoundFormat,
) -> Result<(), TTZipStatus> {
    unsafe {
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
            _ => return Err(TTZipStatus::ErrUnsupportedFeature),
        }
        Ok(())
    }
}
