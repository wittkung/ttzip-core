// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use std::ffi::{CStr, CString};
use std::fs;
use std::path::Path;
use libc::c_void;

use super::fallback_policy::{has_7z_signature, has_zip_signature, should_abort_without_fallback};
use super::selection::EntrySelection;
use crate::archive::tar::reader::TarArchive;
use crate::ffi::archive_ffi::guards::{archive_entry_pathname_safe, ArchiveReadGuard};
use crate::ffi::archive_ffi::sys::*;
use crate::fs::safe_extract::{sanitize_and_validate_path, DirJail};
use crate::sevenz::decoder::archive::SevenZArchive;
use crate::types::TTZipStatus;
use crate::zip::reader::ZipArchive;

fn extract_memory_special_or_compressed_stream(
    archive_path: &Path,
    volume_chain: &[std::path::PathBuf],
    entry_path: Option<&str>,
    entry_index: i64,
    password: Option<&str>,
) -> Result<Option<Vec<u8>>, TTZipStatus> {
    if let Some(transcoded) =
        crate::archive::stream_adapter::try_transcode_special_container(archive_path, volume_chain)?
    {
        if transcoded.entry_count == 0 {
            return Err(TTZipStatus::ErrFileNotFound);
        }
        let stream_reader =
            crate::archive::stream_adapter::read::ArchiveStreamReader::open_sequential(
                transcoded.cursor,
                65536,
            )?;
        let bytes = unsafe {
            extract_single_entry_from_libarchive_handle(
                stream_reader.as_raw_archive(),
                entry_path,
                entry_index,
            )
        }?;
        return Ok(Some(bytes));
    }

    match crate::archive::stream_adapter::try_open_resolved_stream(archive_path, volume_chain)? {
        Some(crate::archive::stream_adapter::ResolvedStreamKind::TarContainer(pre)) => {
            let pwd_c = password.and_then(|p| CString::new(p).ok());
            let stream_reader =
                crate::archive::stream_adapter::read::ArchiveStreamReader::open_sequential(
                    pre, 65536,
                )?;
            let a = stream_reader.as_raw_archive();
            if let Some(ref p) = pwd_c {
                unsafe {
                    archive_read_add_passphrase(a, p.as_ptr());
                }
            }
            let bytes =
                unsafe { extract_single_entry_from_libarchive_handle(a, entry_path, entry_index) }?;
            Ok(Some(bytes))
        }
        Some(crate::archive::stream_adapter::ResolvedStreamKind::SingleFileStream {
            stream,
            entry_name,
            compressed_size,
            ..
        }) => {
            let bytes = crate::archive::stream_adapter::extract_memory_single_file_stream(
                stream,
                &entry_name,
                compressed_size,
                entry_path,
                entry_index,
            )?;
            Ok(Some(bytes))
        }
        None => Ok(None),
    }
}

fn extract_multivolume_single_entry_memory(
    archive_path: &Path,
    volume_chain: Vec<std::path::PathBuf>,
    entry_path: Option<&str>,
    entry_index: i64,
    password: Option<&str>,
) -> Result<Vec<u8>, TTZipStatus> {
    use std::io::{Read, Seek, SeekFrom};
    let mut virtual_reader =
        crate::archive::split::VirtualMultiVolumeReader::from_volumes(volume_chain.clone())
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let mut magic = [0u8; 6];
    let has_magic = virtual_reader.read_exact(&mut magic).is_ok();
    let _ = virtual_reader.seek(SeekFrom::Start(0));
    if has_magic && has_7z_signature(&magic) {
        return crate::archive::stream_adapter::extract_7z_single_entry_from_seekable(
            &mut virtual_reader,
            entry_path,
            entry_index,
            password,
        );
    }

    if has_magic && has_zip_signature(&magic) {
        let total_len = virtual_reader.total_size();
        match crate::zip::reader_stream::extract_zip_single_entry_from_reader(
            &mut virtual_reader,
            total_len,
            entry_path,
            entry_index,
            password,
        ) {
            Ok(bytes) => return Ok(bytes),
            Err(err)
                if should_abort_without_fallback(err) || err == TTZipStatus::ErrFileNotFound =>
            {
                return Err(err);
            }
            Err(_) => {
                let _ = virtual_reader.seek(SeekFrom::Start(0));
            }
        }
    }

    if let Some(bytes) = extract_memory_special_or_compressed_stream(
        archive_path,
        &volume_chain,
        entry_path,
        entry_index,
        password,
    )? {
        return Ok(bytes);
    }

    let pwd_c = password.and_then(|p| CString::new(p).ok());
    let stream_reader =
        crate::archive::stream_adapter::read::ArchiveStreamReader::open_seekable(
            virtual_reader,
            65536,
        )?;
    let a = stream_reader.as_raw_archive();
    if let Some(ref p) = pwd_c {
        unsafe {
            archive_read_add_passphrase(a, p.as_ptr());
        }
    }
    unsafe { extract_single_entry_from_libarchive_handle(a, entry_path, entry_index) }
}

unsafe fn read_matched_libarchive_entry_bytes(
    a: *mut c_void,
    entry: *mut c_void,
) -> Result<Vec<u8>, TTZipStatus> {
    const MAX_IN_MEMORY_DECOMPRESS_BYTES: usize = 64 * 1024 * 1024;
    let size = archive_entry_size(entry).max(0) as usize;
    if size > MAX_IN_MEMORY_DECOMPRESS_BYTES {
        return Err(TTZipStatus::ErrOutOfMemory);
    }
    let mut payload = Vec::with_capacity(size.min(10 * 1024 * 1024));
    let mut chunk = [0u8; 65536];
    loop {
        let r = archive_read_data(a, chunk.as_mut_ptr() as *mut c_void, chunk.len());
        if r < 0 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        if r == 0 {
            break;
        }
        if payload.len() + (r as usize) > MAX_IN_MEMORY_DECOMPRESS_BYTES {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        payload.extend_from_slice(&chunk[..r as usize]);
    }
    Ok(payload)
}

unsafe fn extract_single_entry_from_libarchive_handle(
    a: *mut c_void,
    entry_path: Option<&str>,
    entry_index: i64,
) -> Result<Vec<u8>, TTZipStatus> {
    let mut entry: *mut c_void = std::ptr::null_mut();
    let mut curr_idx: i64 = 0;

    loop {
        let hdr_rc = archive_read_next_header(a, &mut entry);
        if hdr_rc == 1 {
            break;
        }
        if hdr_rc < -20 || (hdr_rc < 0 && entry.is_null()) {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        if entry.is_null() {
            break;
        }
        let Some(path_c_str) = archive_entry_pathname_safe(entry) else {
            archive_read_data_skip(a);
            curr_idx += 1;
            continue;
        };
        let Ok(entry_rel_str) = path_c_str.to_str() else {
            return Err(TTZipStatus::ErrSecurityViolation);
        };
        let is_match = if entry_index >= 0 {
            curr_idx == entry_index
        } else if let Some(target_p) = entry_path {
            entry_rel_str == target_p || entry_rel_str.trim_start_matches("./") == target_p
        } else {
            false
        };
        if is_match {
            return read_matched_libarchive_entry_bytes(a, entry);
        }
        archive_read_data_skip(a);
        curr_idx += 1;
    }
    Err(TTZipStatus::ErrFileNotFound)
}

/// Extracts a single entry directly into an in-memory buffer without full archive extraction.
pub fn extract_single_entry_memory(
    archive_path: &Path,
    entry_path: Option<&str>,
    entry_index: i64,
    password: Option<&str>,
) -> Result<Vec<u8>, TTZipStatus> {
    let volume_chain = crate::archive::split::detect_volume_chain(archive_path)
        .map_err(|_| TTZipStatus::ErrFileNotFound)?;
    if volume_chain.len() > 1 {
        return extract_multivolume_single_entry_memory(
            archive_path,
            volume_chain,
            entry_path,
            entry_index,
            password,
        );
    }

    let single_path = volume_chain.first().map(|p| p.as_path()).unwrap_or(archive_path);
    if !single_path.exists() {
        return Err(TTZipStatus::ErrFileNotFound);
    }

    let source = crate::archive::source::open_archive_source(single_path)?;

    if let Some(mapped) = source.as_slice() {
        if is_zip_magic(mapped) {
            let zip = ZipArchive::open_slice(mapped)?;
            let target_idx = if entry_index >= 0 {
                entry_index as usize
            } else if let Some(target_p) = entry_path {
                zip.entries()
                    .iter()
                    .position(|e| {
                        e.rel_path == target_p || e.rel_path.trim_start_matches("./") == target_p
                    })
                    .ok_or(TTZipStatus::ErrFileNotFound)?
            } else {
                return Err(TTZipStatus::ErrInvalidParam);
            };
            return zip.extract_entry_bytes(target_idx, password);
        }

        if is_sevenz_magic(mapped) {
            let sevenz = SevenZArchive::open_slice_with_password(mapped, password)?;
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

        if is_tar_magic(mapped) {
            let tar = TarArchive::open_slice(mapped)?;
            let target_idx = if entry_index >= 0 {
                entry_index as usize
            } else if let Some(target_p) = entry_path {
                tar.entries()
                    .iter()
                    .position(|e| {
                        e.path.as_ref() == target_p
                            || e.path.as_ref().trim_start_matches("./") == target_p
                    })
                    .ok_or(TTZipStatus::ErrFileNotFound)?
            } else {
                return Err(TTZipStatus::ErrInvalidParam);
            };
            return tar.extract_entry_bytes(target_idx).map(|slice| slice.to_vec());
        }
    }

    if let Some(bytes) = extract_memory_special_or_compressed_stream(
        single_path,
        &volume_chain,
        entry_path,
        entry_index,
        password,
    )? {
        return Ok(bytes);
    }

    let guard = open_libarchive_filename_guard(single_path, password)?;
    unsafe { extract_single_entry_from_libarchive_handle(guard.0, entry_path, entry_index) }
}

fn open_libarchive_filename_guard(
    single_path: &Path,
    password: Option<&str>,
) -> Result<ArchiveReadGuard, TTZipStatus> {
    let arch_c = CString::new(single_path.to_str().ok_or(TTZipStatus::ErrInvalidParam)?)
        .map_err(|_| TTZipStatus::ErrInvalidParam)?;
    unsafe {
        libc::setlocale(libc::LC_ALL, c"en_US.UTF-8".as_ptr());
        let a = archive_read_new();
        if a.is_null() {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let guard = ArchiveReadGuard(a);
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
        Ok(guard)
    }
}

use crate::fs::abort_guard::{DeferredHardlinks, InFlightJailFile};

fn try_extract_selected_stream(
    archive_path: &Path,
    volume_chain: &[std::path::PathBuf],
    selection: &EntrySelection,
    staging: &crate::fs::collision::staging::StagingRoot,
    options: &crate::types::TTZipExtractOptions,
    pwd_opt: Option<&str>,
) -> Result<Option<usize>, TTZipStatus> {
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
        let count = unsafe {
            extract_selected_from_libarchive_handle(
                stream_reader.as_raw_archive(),
                selection,
                staging,
                options,
            )
        }?;
        return Ok(Some(count));
    }

    match crate::archive::stream_adapter::try_open_resolved_stream(archive_path, volume_chain)? {
        Some(crate::archive::stream_adapter::ResolvedStreamKind::TarContainer(pre)) => {
            let mut stream_reader =
                crate::archive::stream_adapter::read::ArchiveStreamReader::open_sequential(
                    pre, 65536,
                )?;
            let a = stream_reader.as_raw_archive();
            if let Some(p_c) = pwd_opt.and_then(|p| CString::new(p).ok()) {
                unsafe {
                    archive_read_add_passphrase(a, p_c.as_ptr());
                }
            }
            let count =
                unsafe { extract_selected_from_libarchive_handle(a, selection, staging, options) }?;
            if !options.dry_run {
                stream_reader.verify_underlying_eof()?;
            }
            Ok(Some(count))
        }
        Some(crate::archive::stream_adapter::ResolvedStreamKind::SingleFileStream {
            stream,
            entry_name,
            compressed_size,
            mtime_epoch_secs,
            ..
        }) => crate::archive::stream_adapter::extract_selected_single_file_stream(
            stream,
            &entry_name,
            compressed_size,
            mtime_epoch_secs,
            selection,
            staging,
            options,
        )
        .map(Some),
        None => Ok(None),
    }
}

fn try_extract_selected_native_or_stream<R: std::io::Read + std::io::Seek>(
    archive_path: &Path,
    volume_chain: &[std::path::PathBuf],
    reader: &mut R,
    total_len: u64,
    selection: &EntrySelection,
    staging: &crate::fs::collision::staging::StagingRoot,
    options: &crate::types::TTZipExtractOptions,
    pwd_opt: Option<&str>,
) -> Result<Option<usize>, TTZipStatus> {
    use std::io::SeekFrom;
    let mut magic = [0u8; 6];
    let has_magic = reader.read_exact(&mut magic).is_ok();
    let _ = reader.seek(SeekFrom::Start(0));

    if has_magic && has_7z_signature(&magic) {
        return crate::archive::stream_adapter::extract_sevenz_selected_from_reader(
            reader, selection, staging, options, pwd_opt,
        )
        .map(Some);
    }

    if has_magic && has_zip_signature(&magic) {
        match crate::zip::reader_stream::extract_zip_selected_from_reader(
            reader,
            total_len,
            selection,
            staging.path(),
            options,
        ) {
            Ok(count) => return Ok(Some(count)),
            Err(err) if should_abort_without_fallback(err) => return Err(err),
            Err(_) => {
                let _ = reader.seek(SeekFrom::Start(0));
            }
        }
    }

    try_extract_selected_stream(archive_path, volume_chain, selection, staging, options, pwd_opt)
}

unsafe fn extract_selected_regular_file_from_handle(
    a: *mut c_void,
    entry: *mut c_void,
    jail: &DirJail,
    out_rel: &str,
    overwrite: bool,
    raw_path: *const libc::c_char,
    total_processed: &mut u64,
    options: &crate::types::TTZipExtractOptions,
) -> Result<(), TTZipStatus> {
    use std::io::Write;
    use std::os::unix::io::AsRawFd;
    use crate::fs::apfs::apfs_preallocate;

    if options.dry_run {
        archive_read_data_skip(a);
        return Ok(());
    }

    let mut inflight = InFlightJailFile::open(jail, out_rel, overwrite)?;

    let size = archive_entry_size(entry).max(0) as u64;
    if size > 0 {
        let _ = apfs_preallocate(inflight.file_mut().as_raw_fd(), size as i64);
    }

    let mut chunk = [0u8; 65536];
    let mut entry_written = 0u64;
    loop {
        let r = archive_read_data(a, chunk.as_mut_ptr() as *mut c_void, chunk.len());
        if r < 0 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        if r == 0 {
            if size > 0 && entry_written < size {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            break;
        }
        if inflight.file_mut().write_all(&chunk[..r as usize]).is_err() {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        entry_written = entry_written.saturating_add(r as u64);
        *total_processed = total_processed.saturating_add(r as u64);

        if let Some(cb) = options.progress_callback {
            if !cb(*total_processed, *total_processed, raw_path, options.user_data) {
                return Err(TTZipStatus::Cancelled);
            }
        }
    }
    inflight.commit(jail)
}

unsafe fn resolve_selected_hardlink_target(
    entry: *mut c_void,
    selection: &EntrySelection,
    staging_path: &Path,
) -> Result<Option<String>, TTZipStatus> {
    let hl_raw = archive_entry_hardlink(entry);
    if hl_raw.is_null() {
        return Ok(None);
    }
    let s = CStr::from_ptr(hl_raw)
        .to_str()
        .map_err(|_| TTZipStatus::ErrSecurityViolation)?;
    let trimmed = s.trim_start_matches('/');
    if trimmed.is_empty() || trimmed == "." {
        return Ok(None);
    }
    let mapped = selection.output_path(s).unwrap_or(s);
    let _ = sanitize_and_validate_path(staging_path, mapped)?;
    Ok(Some(mapped.to_string()))
}

struct SelectedLibarchiveCtx<'a> {
    selection: &'a EntrySelection,
    jail: &'a DirJail,
    staging_path: &'a Path,
    engine: &'a mut crate::fs::safe_extract::SafeExtractEngine,
    deferred_hardlinks: &'a mut DeferredHardlinks,
    total_processed: &'a mut u64,
    options: &'a crate::types::TTZipExtractOptions,
}

unsafe fn classify_libarchive_entry_kind(entry: *mut c_void, entry_rel_str: &str) -> (bool, bool, u64) {
    use libc::mode_t;
    let file_type = archive_entry_filetype(entry);
    let mode = archive_entry_mode(entry) as u32;
    let size = archive_entry_size(entry).max(0) as u64;
    let is_symlink = (file_type & (libc::S_IFMT as mode_t)) == (libc::S_IFLNK as mode_t)
        || (mode & (libc::S_IFMT as u32)) == (libc::S_IFLNK as u32);
    let is_dir = (file_type & (libc::S_IFMT as mode_t)) == (libc::S_IFDIR as mode_t)
        || (mode & (libc::S_IFMT as u32)) == (libc::S_IFDIR as u32)
        || entry_rel_str.ends_with('/');
    (is_symlink, is_dir, size)
}

unsafe fn extract_selected_libarchive_symlink(
    a: *mut c_void,
    entry: *mut c_void,
    out_comps: &[String],
    jail: &DirJail,
    dry_run: bool,
) -> Result<(), TTZipStatus> {
    let symlink_raw = archive_entry_symlink(entry);
    if symlink_raw.is_null() {
        return Err(TTZipStatus::ErrSecurityViolation);
    }
    let symlink_target = CStr::from_ptr(symlink_raw)
        .to_str()
        .map_err(|_| TTZipStatus::ErrSecurityViolation)?;
    if !dry_run {
        jail.create_symlink_components(out_comps, symlink_target)?;
    }
    archive_read_data_skip(a);
    Ok(())
}

unsafe fn dispatch_selected_libarchive_entry(
    a: *mut c_void,
    entry: *mut c_void,
    entry_rel_str: &str,
    out_rel: &str,
    raw_path: *const libc::c_char,
    ctx: &mut SelectedLibarchiveCtx<'_>,
) -> Result<(), TTZipStatus> {
    let (is_symlink, is_dir, size) = classify_libarchive_entry_kind(entry, entry_rel_str);
    let sanitized = sanitize_and_validate_path(ctx.staging_path, out_rel)?;
    let mode = (archive_entry_mode(entry) as u32) & 0o7777;
    let mtime = archive_entry_mtime(entry);
    ctx.engine
        .register_entry_typed(sanitized, mode, mtime, 0, is_dir, is_symlink);
    let out_comps = DirJail::extract_clean_components(out_rel)?;
    ctx.deferred_hardlinks.invalidate_destination(&out_comps);
    let hardlink_rel = resolve_selected_hardlink_target(entry, ctx.selection, ctx.staging_path)?;

    if is_symlink {
        return extract_selected_libarchive_symlink(a, entry, &out_comps, ctx.jail, ctx.options.dry_run);
    }
    if let (Some(src_link_rel), 0) = (&hardlink_rel, size) {
        if !ctx.options.dry_run {
            ctx.deferred_hardlinks.record_or_apply(ctx.jail, out_rel, src_link_rel)?;
        }
        archive_read_data_skip(a);
        return Ok(());
    }
    if is_dir {
        if !ctx.options.dry_run {
            ctx.jail.create_dir_all_components(&out_comps)?;
        }
        archive_read_data_skip(a);
        return Ok(());
    }
    extract_selected_regular_file_from_handle(
        a, entry, ctx.jail, out_rel, true, raw_path, ctx.total_processed, ctx.options,
    )?;
    if !ctx.options.dry_run {
        ctx.deferred_hardlinks
            .on_regular_committed(ctx.jail, &out_comps, hardlink_rel.as_deref())?;
    }
    Ok(())
}

unsafe fn read_next_libarchive_entry(
    a: *mut c_void,
    entry: &mut *mut c_void,
) -> Result<bool, TTZipStatus> {
    let hdr_rc = archive_read_next_header(a, entry);
    if hdr_rc == 1 {
        return Ok(false);
    }
    if hdr_rc < -20 || (hdr_rc < 0 && entry.is_null()) {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    Ok(!entry.is_null())
}

unsafe fn extract_selected_from_libarchive_handle(
    a: *mut c_void,
    selection: &EntrySelection,
    staging: &crate::fs::collision::staging::StagingRoot,
    options: &crate::types::TTZipExtractOptions,
) -> Result<usize, TTZipStatus> {
    fs::create_dir_all(staging.path()).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let jail = DirJail::open(staging.path())?;
    let mut engine = crate::fs::safe_extract::SafeExtractEngine::for_root(staging.path());
    let mut extracted_count = 0;
    let mut total_processed: u64 = 0;
    let mut entry: *mut c_void = std::ptr::null_mut();
    let mut deferred_hardlinks = DeferredHardlinks::default();

    while read_next_libarchive_entry(a, &mut entry)? {
        let Some(path_c_str) = archive_entry_pathname_safe(entry) else {
            archive_read_data_skip(a);
            continue;
        };
        let raw_path = path_c_str.as_ptr();
        if let Some(cb) = options.progress_callback {
            if !cb(total_processed, total_processed, raw_path, options.user_data) {
                return Err(TTZipStatus::Cancelled);
            }
        }
        let entry_rel_str = path_c_str
            .to_str()
            .map_err(|_| TTZipStatus::ErrSecurityViolation)?;
        if let Some(out_rel) = selection.output_path(entry_rel_str) {
            let mut ctx = SelectedLibarchiveCtx {
                selection,
                jail: &jail,
                staging_path: staging.path(),
                engine: &mut engine,
                deferred_hardlinks: &mut deferred_hardlinks,
                total_processed: &mut total_processed,
                options,
            };
            dispatch_selected_libarchive_entry(
                a, entry, entry_rel_str, out_rel, raw_path, &mut ctx,
            )?;
            extracted_count += 1;
        } else {
            archive_read_data_skip(a);
        }
    }
    if !options.dry_run {
        deferred_hardlinks.finalize(&jail)?;
        jail.verify_all_jail_symlinks()?;
        engine.apply_deferred_metadata(options.preserve_permissions)?;
    }
    Ok(extracted_count)
}

/// Selective extraction into a staging root.
pub fn extract_selected_entries(
    archive_path: &Path,
    selection: &EntrySelection,
    staging: &crate::fs::collision::staging::StagingRoot,
    options: &crate::types::TTZipExtractOptions,
) -> Result<usize, TTZipStatus> {
    if selection.is_empty() {
        return Ok(0);
    }
    let volume_chain = crate::archive::split::detect_volume_chain(archive_path)
        .map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let pwd_opt = if !options.password.is_null() {
        unsafe { CStr::from_ptr(options.password).to_str().ok() }.filter(|s| !s.is_empty())
    } else {
        None
    };

    if volume_chain.len() > 1 {
        let mut virtual_reader =
            crate::archive::split::VirtualMultiVolumeReader::from_volumes(volume_chain.clone())
                .map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let total_len = virtual_reader.total_size();
        if let Some(count) = try_extract_selected_native_or_stream(
            archive_path,
            &volume_chain,
            &mut virtual_reader,
            total_len,
            selection,
            staging,
            options,
            pwd_opt,
        )? {
            return Ok(count);
        }
        let pwd_c = pwd_opt.and_then(|p| CString::new(p).ok());
        let stream_reader =
            crate::archive::stream_adapter::read::ArchiveStreamReader::open_seekable(
                virtual_reader,
                65536,
            )?;
        let a = stream_reader.as_raw_archive();
        if let Some(ref p) = pwd_c {
            unsafe {
                archive_read_add_passphrase(a, p.as_ptr());
            }
        }
        return unsafe { extract_selected_from_libarchive_handle(a, selection, staging, options) };
    }

    let single_path = volume_chain.first().map(|p| p.as_path()).unwrap_or(archive_path);
    if !single_path.exists() {
        return Err(TTZipStatus::ErrFileNotFound);
    }

    let mut file = fs::File::open(single_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let total_len = file.metadata().map(|m| m.len()).unwrap_or(0);
    if let Some(count) = try_extract_selected_native_or_stream(
        single_path,
        &volume_chain,
        &mut file,
        total_len,
        selection,
        staging,
        options,
        pwd_opt,
    )? {
        return Ok(count);
    }

    let guard = open_libarchive_filename_guard(single_path, pwd_opt)?;
    unsafe { extract_selected_from_libarchive_handle(guard.0, selection, staging, options) }
}

#[inline]
fn is_zip_magic(mapped: &[u8]) -> bool {
    if mapped.len() < 22 {
        return false;
    }
    if mapped.starts_with(b"PK\x03\x04") || mapped.starts_with(b"PK\x05\x06") {
        return true;
    }
    let search_window = mapped.len().min(65535 + 22);
    let tail = &mapped[mapped.len() - search_window..];
    tail.windows(4).rev().any(|w| w == b"PK\x05\x06")
}

#[inline]
fn is_sevenz_magic(mapped: &[u8]) -> bool {
    mapped.len() >= 6 && &mapped[0..6] == b"7z\xBC\xAF\x27\x1C"
}

#[inline]
fn is_tar_magic(mapped: &[u8]) -> bool {
    mapped.len() >= 262 && &mapped[257..262] == b"ustar"
}



