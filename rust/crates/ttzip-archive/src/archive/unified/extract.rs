// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Archive extraction lifecycle submodule for Unified Orchestrator.

use std::ffi::{CStr, CString};
use std::fs;
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use super::fallback_policy::{has_7z_signature, has_zip_signature, should_abort_without_fallback};
use crate::archive::split::{detect_volume_chain, VirtualMultiVolumeReader};
use crate::ffi::archive_ffi::guards::ArchiveReadGuard;
use crate::ffi::archive_ffi::sys::*;
use crate::fs::apfs::apfs_preallocate;
use crate::fs::safe_extract::{sanitize_and_validate_path, DirJail, SafeExtractEngine};
use crate::types::{ProgressAtomicSheet, TTZipExtractOptions, TTZipStatus};
use libc::{c_int, c_void, mode_t};
use std::sync::atomic::Ordering;

extern "C" {
    fn archive_filter_bytes(a: *mut c_void, filter_num: c_int) -> i64;
}

/// Extracts an archive to destination directory with security verification and APFS preallocation.
pub fn extract_archive(
    archive_path: &Path,
    staging: &crate::fs::collision::staging::StagingRoot,
    options: &TTZipExtractOptions,
) -> Result<(), TTZipStatus> {
    extract_archive_with_sheet(archive_path, staging, options, None).map(|_| ())
}

/// Extracts an archive and returns total extracted uncompressed bytes.
pub fn extract_archive_with_metrics(
    archive_path: &Path,
    staging: &crate::fs::collision::staging::StagingRoot,
    options: &TTZipExtractOptions,
) -> Result<u64, TTZipStatus> {
    extract_archive_with_sheet(archive_path, staging, options, None)
}

/// Extracts an archive with an optional atomic progress sheet for non-blocking cancellation and progress tracking.
pub fn extract_archive_with_sheet(
    archive_path: &Path,
    staging: &crate::fs::collision::staging::StagingRoot,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<u64, TTZipStatus> {
    let volume_chain = detect_volume_chain(archive_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;

    if options.dry_run {
        return extract_archive_internal(archive_path, &volume_chain, staging.path(), options, sheet);
    }

    let result = extract_archive_internal(archive_path, &volume_chain, staging.path(), options, sheet);
    match result {
        Ok(bytes) => {
            if let Some(s) = sheet {
                s.mark_finished();
            }
            Ok(bytes)
        }
        Err(err) => Err(err),
    }
}

fn strip_split_suffix(name_lower: &str) -> &str {
    if let Some(dot_pos) = name_lower.rfind('.') {
        let ext = &name_lower[dot_pos + 1..];
        let is_num = ext.len() >= 3 && ext.chars().all(|c| c.is_ascii_digit());
        let is_z = ext.starts_with('z') && ext.len() >= 3 && ext[1..].chars().all(|c| c.is_ascii_digit());
        if is_num || is_z {
            return &name_lower[..dot_pos];
        }
    }
    name_lower
}

fn extract_multivolume_7z(
    virtual_reader: &mut VirtualMultiVolumeReader,
    destination_path: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<u64, TTZipStatus> {
    let pwd_str = if !options.password.is_null() {
        unsafe { CStr::from_ptr(options.password).to_str().ok() }
    } else {
        None
    };
    let info = crate::sevenz::encrypted_header::EncodedHeaderDecoder::default()
        .decode_from_reader(virtual_reader, pwd_str)
        .map_err(|e| match e {
            crate::sevenz::dag::SevenZError::BadPassword => TTZipStatus::ErrInvalidPassword,
            _ => TTZipStatus::ErrCorruptHeader,
        })?;
    crate::sevenz::decoder::archive::extract_7z_from_reader(
        virtual_reader,
        &info,
        destination_path,
        options,
        sheet,
    )
}

fn extract_multivolume_seekable_libarchive(
    virtual_reader: VirtualMultiVolumeReader,
    archive_path: &Path,
    destination_path: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<u64, TTZipStatus> {
    let reader = crate::archive::stream_adapter::read::ArchiveStreamReader::open_seekable(virtual_reader, 65536)?;
    let raw_a = reader.as_raw_archive();
    if !options.password.is_null() {
        if let Ok(p_str) = unsafe { CStr::from_ptr(options.password).to_str() } {
            if !p_str.is_empty() {
                unsafe { archive_read_add_passphrase(raw_a, options.password); }
            }
        }
    }
    unsafe { extract_from_archive_handle(raw_a, archive_path, destination_path, options, sheet) }
}

fn verify_multivolume_tar_eof_trailer(
    virtual_reader: &mut VirtualMultiVolumeReader,
) -> Result<(), TTZipStatus> {
    use std::io::Read;
    if virtual_reader.total_size() < 1024 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let mut tail = [0u8; 512];
    if virtual_reader.seek(SeekFrom::End(-512)).is_err()
        || virtual_reader.read_exact(&mut tail).is_err()
        || tail.iter().any(|&b| b != 0)
    {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let _ = virtual_reader.seek(SeekFrom::Start(0));
    Ok(())
}

fn extract_compressed_tar_stream<R: std::io::Read + 'static>(
    pre: R,
    archive_path: &Path,
    destination_path: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<u64, TTZipStatus> {
    let mut stream_reader =
        crate::archive::stream_adapter::read::ArchiveStreamReader::open_sequential(pre, 65536)?;
    let raw_a = stream_reader.as_raw_archive();
    if !options.password.is_null() {
        if let Ok(p_str) = unsafe { CStr::from_ptr(options.password).to_str() } {
            if !p_str.is_empty() {
                unsafe {
                    archive_read_add_passphrase(raw_a, options.password);
                }
            }
        }
    }
    let bytes = unsafe {
        extract_from_archive_handle(raw_a, archive_path, destination_path, options, sheet)
    }?;
    if !options.dry_run {
        stream_reader.verify_underlying_eof()?;
    }
    Ok(bytes)
}

fn extract_special_or_compressed_stream(
    archive_path: &Path,
    volume_chain: &[PathBuf],
    destination_path: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<Option<u64>, TTZipStatus> {
    if let Some(transcoded) =
        crate::archive::stream_adapter::try_transcode_special_container(archive_path, volume_chain)?
    {
        if transcoded.entry_count == 0 {
            return Ok(Some(0));
        }
        let stream_reader =
            crate::archive::stream_adapter::read::ArchiveStreamReader::open_sequential(
                transcoded.cursor,
                65536,
            )?;
        let bytes = unsafe {
            extract_from_archive_handle(
                stream_reader.as_raw_archive(),
                archive_path,
                destination_path,
                options,
                sheet,
            )
        }?;
        return Ok(Some(bytes));
    }

    match crate::archive::stream_adapter::try_open_resolved_stream(archive_path, volume_chain)? {
        Some(crate::archive::stream_adapter::ResolvedStreamKind::TarContainer(pre)) => {
            extract_compressed_tar_stream(pre, archive_path, destination_path, options, sheet)
                .map(Some)
        }
        Some(crate::archive::stream_adapter::ResolvedStreamKind::SingleFileStream {
            stream,
            entry_name,
            compressed_size,
            mtime_epoch_secs,
            ..
        }) => crate::archive::stream_adapter::extract_single_file_stream(
            stream,
            &entry_name,
            compressed_size,
            mtime_epoch_secs,
            destination_path,
            options,
            sheet,
        )
        .map(Some),
        None => Ok(None),
    }
}

fn extract_multivolume_tar_or_libarchive(
    mut virtual_reader: VirtualMultiVolumeReader,
    volume_chain: &[PathBuf],
    base_lower: &str,
    archive_path: &Path,
    destination_path: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<u64, TTZipStatus> {
    if base_lower.ends_with(".tar") {
        verify_multivolume_tar_eof_trailer(&mut virtual_reader)?;
    }
    if let Some(bytes) = extract_special_or_compressed_stream(
        archive_path,
        volume_chain,
        destination_path,
        options,
        sheet,
    )? {
        return Ok(bytes);
    }
    extract_multivolume_seekable_libarchive(
        virtual_reader,
        archive_path,
        destination_path,
        options,
        sheet,
    )
}

fn extract_multivolume_archive(
    volume_chain: Vec<PathBuf>,
    archive_path: &Path,
    destination_path: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<u64, TTZipStatus> {
    use std::io::Read;
    let first_name_lower = volume_chain
        .first()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    let base_lower = strip_split_suffix(&first_name_lower);

    let mut virtual_reader = VirtualMultiVolumeReader::from_volumes(volume_chain.clone())
        .map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let total_len = virtual_reader.total_size();
    let mut magic = [0u8; 6];
    let has_magic = virtual_reader.read_exact(&mut magic).is_ok();
    let _ = virtual_reader.seek(SeekFrom::Start(0));

    if has_magic && &magic == b"7z\xBC\xAF\x27\x1C" {
        return extract_multivolume_7z(&mut virtual_reader, destination_path, options, sheet);
    }

    let is_zip = has_magic
        && (&magic[..4] == b"PK\x03\x04"
            || &magic[..4] == b"PK\x05\x06"
            || &magic[..4] == b"PK\x07\x08");
    if is_zip || base_lower.ends_with(".zip") {
        match crate::zip::reader_stream::extract_zip_from_reader(
            &mut virtual_reader,
            total_len,
            destination_path,
            options,
            sheet,
        ) {
            Ok(report) => return Ok(report.total_uncompressed_bytes),
            Err(err) if should_abort_without_fallback(err) => return Err(err),
            Err(_) => {
                let _ = virtual_reader.seek(SeekFrom::Start(0));
            }
        }
    }

    extract_multivolume_tar_or_libarchive(
        virtual_reader,
        &volume_chain,
        base_lower,
        archive_path,
        destination_path,
        options,
        sheet,
    )
}

fn extract_archive_internal(
    archive_path: &Path,
    volume_chain: &[PathBuf],
    destination_path: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<u64, TTZipStatus> {
    fs::create_dir_all(destination_path).map_err(|_| TTZipStatus::ErrExtractionFailed)?;

    if volume_chain.len() > 1 {
        return extract_multivolume_archive(
            volume_chain.to_vec(),
            archive_path,
            destination_path,
            options,
            sheet,
        );
    }

    // 1. Fast-Path: Pure Safe Rust Streaming 7z & Resident ZIP Decoders with Bounded RSS
    if let Ok(source) = crate::archive::source::open_archive_source(archive_path) {
        if let Some(mapped) = source.as_slice() {
            let pwd_str = if !options.password.is_null() {
                unsafe { CStr::from_ptr(options.password).to_str().ok() }
            } else {
                None
            };
            if has_7z_signature(mapped) {
                match crate::sevenz::decoder::archive::SevenZArchive::open_slice_with_password(
                    mapped, pwd_str,
                ) {
                    Ok(sevenz) => {
                        match sevenz.extract_all_with_sheet(destination_path, options, sheet) {
                            Ok(report) => return Ok(report.total_uncompressed_bytes),
                            Err(err) if should_abort_without_fallback(err) => return Err(err),
                            Err(err) => {
                                log::debug!(
                                    "7z fast-path extraction failed with non-fatal algorithm error ({:?}); falling back to libarchive engine",
                                    err
                                );
                            }
                        }
                    }
                    Err(err) if should_abort_without_fallback(err) => return Err(err),
                    Err(_) => {}
                }
            }
            if let Ok(zip_archive) = crate::zip::reader::ZipArchive::open_slice(mapped) {
                match zip_archive.extract_all_with_sheet(destination_path, options, sheet) {
                    Ok(report) => return Ok(report.total_uncompressed_bytes),
                    Err(err) if should_abort_without_fallback(err) => return Err(err),
                    Err(err) => {
                        log::debug!(
                            "Zip fast-path extraction failed with non-fatal algorithm error ({:?}); falling back to libarchive engine",
                            err
                        );
                    }
                }
            } else if has_zip_signature(mapped) {
                if let Err(
                    err @ (TTZipStatus::ErrSecurityViolation | TTZipStatus::ErrOutOfMemory),
                ) = crate::zip::reader::ZipArchive::open_slice(mapped)
                {
                    return Err(err);
                }
            }
        }
    }

    // 2. Unified Special Container (AAR/XAR/WIM/DMG) & Confirmed Compressed Stream Pipeline
    if let Some(bytes) = extract_special_or_compressed_stream(
        archive_path,
        volume_chain,
        destination_path,
        options,
        sheet,
    )? {
        return Ok(bytes);
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

        extract_from_archive_handle(a, archive_path, destination_path, options, sheet)
    }
}

fn estimate_or_scan_total_uncompressed(archive_path: &Path, password: *const libc::c_char) -> u64 {
    if !archive_path.is_file() {
        return 0;
    }

    // 1. Fast-path for 7z / zip if memory-mappable
    if let Ok(source) = crate::archive::source::open_archive_source(archive_path) {
        if let Some(mapped) = source.as_slice() {
            let pwd_str = if !password.is_null() {
                unsafe { CStr::from_ptr(password).to_str().ok() }
            } else {
                None
            };
            if let Ok(sevenz) = crate::sevenz::decoder::archive::SevenZArchive::open_slice_with_password(mapped, pwd_str) {
                let mut sum = 0u64;
                for &sz in &sevenz.info().stream_sizes {
                    sum += sz;
                }
                if sum == 0 {
                    for f in &sevenz.info().folders {
                        sum += f.unpack_sizes.last().copied().unwrap_or(0);
                    }
                }
                if sum > 0 {
                    return sum;
                }
            }
            if let Ok(zip_archive) = crate::zip::reader::ZipArchive::open_slice(mapped) {
                let sum: u64 = zip_archive.entries().iter().map(|e| e.uncompressed_size).sum();
                if sum > 0 {
                    return sum;
                }
            }
        }
    }

    // 2. Libarchive fast header scan (skips entry payloads)
    let Ok(arch_c) = CString::new(archive_path.to_str().unwrap_or_default()) else {
        return 0;
    };
    unsafe {
        let a = archive_read_new();
        if a.is_null() {
            return 0;
        }
        let _guard = ArchiveReadGuard(a);
        archive_read_support_format_all(a);
        archive_read_support_filter_all(a);
        if !password.is_null() {
            if let Ok(p_str) = CStr::from_ptr(password).to_str() {
                if !p_str.is_empty() {
                    archive_read_add_passphrase(a, password);
                }
            }
        }
        if archive_read_open_filename(a, arch_c.as_ptr(), 65536) != 0 {
            return 0;
        }
        let mut entry: *mut c_void = std::ptr::null_mut();
        let mut total_sz: u64 = 0;
        while archive_read_next_header(a, &mut entry) == 0 {
            if !entry.is_null() {
                let sz = archive_entry_size(entry).max(0) as u64;
                total_sz = total_sz.saturating_add(sz);
                archive_read_data_skip(a);
            }
        }
        total_sz
    }
}

unsafe fn extract_from_archive_handle(
    a: *mut c_void,
    archive_path: &Path,
    destination_path: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<u64, TTZipStatus> {
    libc::setlocale(libc::LC_ALL, c"en_US.UTF-8".as_ptr());

    let local_sheet = ProgressAtomicSheet::new();
    let active_sheet = sheet.unwrap_or(&local_sheet);

    if active_sheet.total_bytes.load(Ordering::Relaxed) == 0 {
        let scanned = estimate_or_scan_total_uncompressed(archive_path, options.password);
        if scanned > 0 {
            active_sheet.set_total(scanned);
        }
    }

    if active_sheet.is_cancelled() {
        return Err(TTZipStatus::Cancelled);
    }

    let jail = DirJail::open(destination_path)?;
    let mut engine = SafeExtractEngine::for_root(destination_path);
    let mut entry: *mut c_void = std::ptr::null_mut();
    let mut total_processed: u64 = 0;
    let mut entries_seen: usize = 0;
    let mut deferred_hardlinks = crate::fs::abort_guard::DeferredHardlinks::default();
    let bomb_guard = crate::security::path_sanitizer::ExpansionRatioGuard::default();

    loop {
        if active_sheet.is_cancelled() {
            return Err(TTZipStatus::Cancelled);
        }
        let hdr_rc = archive_read_next_header(a, &mut entry);
        if hdr_rc == 1 {
            break;
        }
        if hdr_rc == -10 {
            continue;
        }
        if hdr_rc != 0 {
            if let Some(err_str) = get_archive_error_string(a) {
                let lower = err_str.to_lowercase();
                if lower.contains("passphrase") || lower.contains("password") || lower.contains("encrypt") {
                    return Err(TTZipStatus::ErrInvalidPassword);
                }
            }
            return Err(TTZipStatus::ErrCorruptHeader);
        }
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
            Err(_) => return Err(TTZipStatus::ErrSecurityViolation),
        };

        let mut clean_rel_str = entry_rel_str;
        if clean_rel_str.ends_with(";1") {
            clean_rel_str = &clean_rel_str[..clean_rel_str.len() - 2];
        }
        let root_check = clean_rel_str.trim_start_matches('/');
        if root_check.is_empty() || root_check == "." || root_check == "./" {
            if clean_rel_str.contains('\\') {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
            entries_seen += 1;
            archive_read_data_skip(a);
            continue;
        }

        // Invariant II: ZipSlip & Security Path Sanitization
        let target_path = sanitize_and_validate_path(destination_path, clean_rel_str)?;
        entries_seen += 1;

        let size = archive_entry_size(entry).max(0) as u64;
        let mode = archive_entry_mode(entry) as u32;
        let mtime = archive_entry_mtime(entry) as i64;
        let filetype = archive_entry_filetype(entry);
        let is_symlink = (filetype & (libc::S_IFMT as mode_t)) == (libc::S_IFLNK as mode_t);
        let is_dir = (filetype & (libc::S_IFMT as mode_t)) == (libc::S_IFDIR as mode_t)
            || (mode & (libc::S_IFMT as u32)) == (libc::S_IFDIR as u32)
            || entry_rel_str.ends_with('/');

        let is_enc = archive_entry_is_data_encrypted(entry) != 0
            || archive_entry_is_metadata_encrypted(entry) != 0;
        if is_enc && options.password.is_null() {
            return Err(TTZipStatus::ErrInvalidPassword);
        }

        if options.dry_run {
            if !is_dir && !is_symlink && size > 0 {
                let mut block_ptr: *const c_void = std::ptr::null();
                let mut block_size: libc::size_t = 0;
                let mut block_offset: i64 = 0;
                let r = archive_read_data_block(
                    a,
                    &mut block_ptr,
                    &mut block_size,
                    &mut block_offset,
                );
                if r < 0 {
                    return Err(TTZipStatus::ErrInvalidPassword);
                }
                archive_read_data_skip(a);
            } else {
                archive_read_data_skip(a);
            }
            let total_done = active_sheet.add_processed(size);
            total_processed = total_done;
            if let Some(cb) = options.progress_callback {
                let total_target = active_sheet.total_bytes.load(Ordering::Relaxed).max(total_done);
                if !cb(total_done, total_target, raw_path, options.user_data) {
                    active_sheet.cancel();
                    return Err(TTZipStatus::Cancelled);
                }
            }
            continue;
        }

        let hardlink_rel: Option<String> = {
            let hl_raw = archive_entry_hardlink(entry);
            if !hl_raw.is_null() {
                let s = CStr::from_ptr(hl_raw)
                    .to_str()
                    .map_err(|_| TTZipStatus::ErrSecurityViolation)?;
                if !s.trim_start_matches('/').is_empty() && s.trim_start_matches('/') != "." {
                    let _ = sanitize_and_validate_path(destination_path, s)?;
                    Some(s.to_string())
                } else {
                    None
                }
            } else {
                None
            }
        };

        let entry_comps = DirJail::extract_clean_components(clean_rel_str)?;
        deferred_hardlinks.invalidate_destination(&entry_comps);

        if is_symlink {
            let symlink_raw = archive_entry_symlink(entry);
            if symlink_raw.is_null() {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
            let symlink_target = CStr::from_ptr(symlink_raw)
                .to_str()
                .map_err(|_| TTZipStatus::ErrSecurityViolation)?;
            jail.create_symlink_components(&entry_comps, symlink_target)?;
            engine.register_entry_typed(target_path, mode, mtime, 0, false, true);
            archive_read_data_skip(a);
        } else if let (Some(src_link_rel), 0) = (&hardlink_rel, size) {
            deferred_hardlinks.record_or_apply(&jail, clean_rel_str, src_link_rel)?;
            engine.register_entry_typed(target_path, mode, mtime, 0, false, false);
            archive_read_data_skip(a);
        } else if is_dir {
            jail.create_dir_all_components(&entry_comps)?;
            engine.register_entry_typed(target_path, mode, mtime, 0, true, false);
            archive_read_data_skip(a);
        } else {
            let mut inflight: Option<crate::fs::abort_guard::InFlightJailFile> =
                if !is_enc && size == 0 {
                    Some(crate::fs::abort_guard::InFlightJailFile::open(
                        &jail,
                        clean_rel_str,
                        true,
                    )?)
                } else {
                    None
                };

            let mut block_ptr: *const c_void = std::ptr::null();
            let mut block_size: libc::size_t = 0;
            let mut block_offset: i64 = 0;
            let mut entry_end_offset: u64 = 0;

            loop {
                if active_sheet.is_cancelled() {
                    return Err(TTZipStatus::Cancelled);
                }
                let rc = archive_read_data_block(
                    a,
                    &mut block_ptr,
                    &mut block_size,
                    &mut block_offset,
                );
                if rc == 1 {
                    if size > 0 && entry_end_offset < size {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    break;
                }
                if rc != 0 {
                    if is_enc {
                        return Err(TTZipStatus::ErrInvalidPassword);
                    }
                    if let Some(err_str) = get_archive_error_string(a) {
                        let lower = err_str.to_lowercase();
                        if lower.contains("passphrase") || lower.contains("password") || lower.contains("encrypt") {
                            return Err(TTZipStatus::ErrInvalidPassword);
                        }
                    }
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
                if inflight.is_none() {
                    let mut f =
                        crate::fs::abort_guard::InFlightJailFile::open(&jail, clean_rel_str, true)?;
                    if size > 0 {
                        let _ = apfs_preallocate(f.file_mut().as_raw_fd(), size as i64);
                    }
                    inflight = Some(f);
                }
                if block_size > 0 && !block_ptr.is_null() {
                    let f = inflight.as_mut().unwrap().file_mut();
                    if block_offset >= 0 && f.seek(SeekFrom::Start(block_offset as u64)).is_err() {
                        return Err(TTZipStatus::ErrExtractionFailed);
                    }
                    let chunk = std::slice::from_raw_parts(block_ptr as *const u8, block_size);
                    if f.write_all(chunk).is_err() {
                        return Err(TTZipStatus::ErrExtractionFailed);
                    }
                    let cur_end = (block_offset.max(0) as u64).saturating_add(block_size as u64);
                    if cur_end > entry_end_offset {
                        entry_end_offset = cur_end;
                    }
                    let total_done = active_sheet.add_processed(block_size as u64);
                    total_processed = total_done;
                    let comp_bytes = archive_filter_bytes(a, -1).max(0) as u64;
                    bomb_guard.check(total_processed, comp_bytes)?;
                    if let Some(cb) = options.progress_callback {
                        let total_target = active_sheet.total_bytes.load(Ordering::Relaxed).max(total_done);
                        if !cb(total_done, total_target, raw_path, options.user_data) {
                            active_sheet.cancel();
                            return Err(TTZipStatus::Cancelled);
                        }
                    }
                }
            }

            let inflight_file = match inflight {
                Some(f) => f,
                None => crate::fs::abort_guard::InFlightJailFile::open(&jail, clean_rel_str, true)?,
            };
            inflight_file.commit(&jail)?;
            engine.register_entry_typed(target_path, mode, mtime, 0, false, false);
            deferred_hardlinks.on_regular_committed(&jail, &entry_comps, hardlink_rel.as_deref())?;
        }
    }

    deferred_hardlinks.finalize(&jail)?;

    if !options.dry_run {
        super::fallback_policy::verify_gzip_stream_if_applicable(archive_path)?;
        jail.verify_all_jail_symlinks()?;
    }

    if entries_seen == 0 && total_processed == 0 && !options.dry_run {
        if archive_path.metadata().map(|m| m.len() > 0).unwrap_or(false) {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
    }

    active_sheet.mark_finished();

    if !options.dry_run {
        engine.apply_deferred_metadata(options.preserve_permissions)?;
    }

    Ok(total_processed)
}


