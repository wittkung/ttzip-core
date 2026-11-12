// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Streaming extraction pump for `libarchive` reader handles with ZipSlip defense,
//! APFS extent preallocation, deferred hardlinks, and expansion ratio guards.

use std::ffi::CStr;
use std::fs;
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use libc::{c_int, c_void, mode_t};

use super::is_sequential_compressed_stream_path;
use crate::ffi::archive_ffi::sys::*;
use crate::fs::abort_guard::{DeferredHardlinks, InFlightJailFile};
use crate::fs::apfs::apfs_preallocate_entry;
use crate::fs::safe_extract::{sanitize_and_validate_path, DirJail, SafeExtractEngine};
use crate::security::path_sanitizer::ExpansionRatioGuard;
use crate::types::{ProgressAtomicSheet, TTZipExtractOptions, TTZipStatus};

unsafe extern "C" {
    fn archive_filter_bytes(a: *mut c_void, filter_num: c_int) -> i64;
}

fn scan_mapped_uncompressed_total(
    archive_path: &Path,
    mapped: &[u8],
    password: *const libc::c_char,
) -> Option<u64> {
    if crate::archive::stream_adapter::classify_stream_codec(
        archive_path,
        &mapped[..mapped.len().min(16)],
    )
    .ok()
    .flatten()
    .is_some()
    {
        return None;
    }
    let pwd_str = (!password.is_null())
        .then(|| unsafe { CStr::from_ptr(password).to_str().ok() })
        .flatten();
    if let Ok(sevenz) =
        crate::sevenz::decoder::archive::SevenZArchive::open_slice_with_password(mapped, pwd_str)
    {
        let sum: u64 = sevenz.info().stream_sizes.iter().copied().sum();
        let total = if sum > 0 {
            sum
        } else {
            sevenz
                .info()
                .folders
                .iter()
                .map(|f| f.unpack_sizes.last().copied().unwrap_or(0))
                .sum()
        };
        if total > 0 {
            return Some(total);
        }
    }
    if let Ok(zip_archive) = crate::zip::reader::ZipArchive::open_slice(mapped) {
        let sum: u64 = zip_archive.entries().iter().map(|e| e.uncompressed_size).sum();
        if sum > 0 {
            return Some(sum);
        }
    }
    if mapped.len() >= 512
        && crate::archive::stream_adapter::is_valid_tar_header_block(&mapped[..512])
    {
        if let Ok(tar_archive) = crate::archive::tar::TarArchive::open_slice(mapped) {
            let sum: u64 = tar_archive.entries().iter().map(|e| e.size).sum();
            if sum > 0 {
                return Some(sum);
            }
        }
    }
    None
}

fn estimate_or_scan_total_uncompressed(archive_path: &Path, password: *const libc::c_char) -> u64 {
    let file_len = archive_path.metadata().map(|m| m.len()).unwrap_or(0);
    if file_len == 0 || is_sequential_compressed_stream_path(archive_path) {
        return file_len;
    }

    if let Ok(source) = crate::archive::source::open_archive_source(archive_path) {
        if let Some(mapped) = source.as_slice() {
            if let Some(total) = scan_mapped_uncompressed_total(archive_path, mapped, password) {
                return total;
            }
        }
    }
    file_len
}

unsafe fn classify_archive_error(a: *mut c_void, is_enc: bool) -> TTZipStatus {
    if is_enc {
        return TTZipStatus::ErrInvalidPassword;
    }
    if let Some(err_str) = unsafe { get_archive_error_string(a) } {
        let lower = err_str.to_lowercase();
        if lower.contains("passphrase") || lower.contains("password") || lower.contains("encrypt") {
            return TTZipStatus::ErrInvalidPassword;
        }
    }
    TTZipStatus::ErrCorruptHeader
}

unsafe fn next_libarchive_header(
    a: *mut c_void,
    entry: &mut *mut c_void,
    active_sheet: &ProgressAtomicSheet,
) -> Result<Option<bool>, TTZipStatus> {
    if active_sheet.is_cancelled() {
        return Err(TTZipStatus::Cancelled);
    }
    let hdr_rc = unsafe { archive_read_next_header(a, entry) };
    if hdr_rc == 1 {
        return Ok(Some(false));
    }
    if hdr_rc == -10 {
        return Ok(None);
    }
    if hdr_rc != 0 {
        return Err(unsafe { classify_archive_error(a, false) });
    }
    Ok(Some(!entry.is_null()))
}

unsafe fn handle_dry_run_entry(
    a: *mut c_void,
    is_dir: bool,
    is_symlink: bool,
    size: u64,
    raw_path: *const libc::c_char,
    options: &TTZipExtractOptions,
    active_sheet: &ProgressAtomicSheet,
    total_processed: &mut u64,
) -> Result<(), TTZipStatus> {
    unsafe {
        if !is_dir && !is_symlink && size > 0 {
            let mut block_ptr: *const c_void = std::ptr::null();
            let mut block_size: libc::size_t = 0;
            let mut block_offset: i64 = 0;
            let r = archive_read_data_block(a, &mut block_ptr, &mut block_size, &mut block_offset);
            if r < 0 {
                return Err(TTZipStatus::ErrInvalidPassword);
            }
            archive_read_data_skip(a);
        } else {
            archive_read_data_skip(a);
        }
        let total_done = active_sheet.add_processed(size);
        *total_processed = total_done;
        if let Some(cb) = options.progress_callback {
            let total_target = active_sheet.total_bytes.load(Ordering::Relaxed).max(total_done);
            if !cb(total_done, total_target, raw_path, options.user_data) {
                active_sheet.cancel();
                return Err(TTZipStatus::Cancelled);
            }
        }
        Ok(())
    }
}

unsafe fn resolve_entry_hardlink(
    entry: *mut c_void,
    destination_path: &Path,
) -> Result<Option<String>, TTZipStatus> {
    let hl_raw = unsafe { archive_entry_hardlink(entry) };
    if hl_raw.is_null() {
        return Ok(None);
    }
    let s = unsafe { CStr::from_ptr(hl_raw) }
        .to_str()
        .map_err(|_| TTZipStatus::ErrSecurityViolation)?;
    let trimmed = s.trim_start_matches('/');
    if trimmed.is_empty() || trimmed == "." {
        return Ok(None);
    }
    let _ = sanitize_and_validate_path(destination_path, s)?;
    Ok(Some(s.to_string()))
}

fn ensure_inflight_file<'a>(
    inflight: &'a mut Option<InFlightJailFile>,
    jail: &DirJail,
    clean_rel_str: &str,
    size: u64,
) -> Result<&'a mut fs::File, TTZipStatus> {
    if inflight.is_none() {
        let mut opened = InFlightJailFile::open(jail, clean_rel_str, true)?;
        if size > 0 {
            let _ = apfs_preallocate_entry(opened.file_mut().as_raw_fd(), size);
        }
        *inflight = Some(opened);
    }
    match inflight.as_mut() {
        Some(existing) => Ok(existing.file_mut()),
        None => Err(TTZipStatus::ErrExtractionFailed),
    }
}

struct RegularBlockPumpCtx<'a> {
    jail: &'a DirJail,
    clean_rel_str: &'a str,
    raw_path: *const libc::c_char,
    size: u64,
    is_enc: bool,
    options: &'a TTZipExtractOptions,
    active_sheet: &'a ProgressAtomicSheet,
    bomb_guard: &'a ExpansionRatioGuard,
    total_processed: &'a mut u64,
}

unsafe fn write_archive_data_block(
    a: *mut c_void,
    inflight: &mut Option<InFlightJailFile>,
    block_ptr: *const c_void,
    block_size: usize,
    block_offset: i64,
    entry_end_offset: &mut u64,
    ctx: &mut RegularBlockPumpCtx<'_>,
) -> Result<(), TTZipStatus> {
    if block_size == 0 || block_ptr.is_null() {
        return Ok(());
    }
    let f = ensure_inflight_file(inflight, ctx.jail, ctx.clean_rel_str, ctx.size)?;
    if block_offset >= 0 && f.seek(SeekFrom::Start(block_offset as u64)).is_err() {
        return Err(TTZipStatus::ErrExtractionFailed);
    }
    let chunk = unsafe { std::slice::from_raw_parts(block_ptr as *const u8, block_size) };
    if f.write_all(chunk).is_err() {
        return Err(TTZipStatus::ErrExtractionFailed);
    }
    *entry_end_offset = (*entry_end_offset).max((block_offset.max(0) as u64).saturating_add(block_size as u64));
    let total_done = ctx.active_sheet.add_processed(block_size as u64);
    *ctx.total_processed = total_done;
    let comp_bytes = unsafe { archive_filter_bytes(a, -1) }.max(0) as u64;
    ctx.bomb_guard.check(*ctx.total_processed, comp_bytes)?;
    if let Some(cb) = ctx.options.progress_callback {
        let total_target = ctx
            .active_sheet
            .total_bytes
            .load(Ordering::Relaxed)
            .max(total_done);
        if !unsafe { cb(total_done, total_target, ctx.raw_path, ctx.options.user_data) } {
            ctx.active_sheet.cancel();
            return Err(TTZipStatus::Cancelled);
        }
    }
    Ok(())
}

unsafe fn pump_regular_entry_blocks(
    a: *mut c_void,
    ctx: &mut RegularBlockPumpCtx<'_>,
) -> Result<(), TTZipStatus> {
    let mut inflight: Option<InFlightJailFile> = if !ctx.is_enc && ctx.size == 0 {
        Some(InFlightJailFile::open(ctx.jail, ctx.clean_rel_str, true)?)
    } else {
        None
    };

    let mut block_ptr: *const c_void = std::ptr::null();
    let mut block_size: libc::size_t = 0;
    let mut block_offset: i64 = 0;
    let mut entry_end_offset: u64 = 0;

    loop {
        if ctx.active_sheet.is_cancelled() {
            return Err(TTZipStatus::Cancelled);
        }
        let rc = unsafe {
            archive_read_data_block(a, &mut block_ptr, &mut block_size, &mut block_offset)
        };
        if rc == 1 {
            break;
        }
        if rc != 0 {
            return Err(unsafe { classify_archive_error(a, ctx.is_enc) });
        }
        let _ = ensure_inflight_file(&mut inflight, ctx.jail, ctx.clean_rel_str, ctx.size)?;
        unsafe {
            write_archive_data_block(
                a,
                &mut inflight,
                block_ptr,
                block_size,
                block_offset,
                &mut entry_end_offset,
                ctx,
            )?;
        }
    }

    if ctx.size > 0 && entry_end_offset < ctx.size {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let inflight_file = match inflight {
        Some(f) => f,
        None => InFlightJailFile::open(ctx.jail, ctx.clean_rel_str, true)?,
    };
    inflight_file.commit(ctx.jail)
}

struct HandleLoopState<'a> {
    destination_path: &'a Path,
    jail: &'a DirJail,
    engine: &'a mut SafeExtractEngine,
    deferred_hardlinks: &'a mut DeferredHardlinks,
    bomb_guard: &'a ExpansionRatioGuard,
    options: &'a TTZipExtractOptions,
    active_sheet: &'a ProgressAtomicSheet,
    total_processed: &'a mut u64,
    entries_seen: &'a mut usize,
}

struct ParsedHandleEntry<'a> {
    raw_path: *const libc::c_char,
    clean_rel_str: &'a str,
    target_path: PathBuf,
    size: u64,
    mode: u32,
    mtime: i64,
    is_symlink: bool,
    is_dir: bool,
    is_enc: bool,
}

unsafe fn parse_and_validate_handle_entry<'a>(
    a: *mut c_void,
    entry: *mut c_void,
    state: &mut HandleLoopState<'_>,
) -> Result<Option<ParsedHandleEntry<'a>>, TTZipStatus> {
    let raw_path = unsafe { archive_entry_pathname(entry) };
    if raw_path.is_null() {
        unsafe { archive_read_data_skip(a) };
        return Ok(None);
    }
    let entry_rel_str = unsafe { CStr::from_ptr(raw_path) }
        .to_str()
        .map_err(|_| TTZipStatus::ErrSecurityViolation)?;
    let clean_rel_str = entry_rel_str.strip_suffix(";1").unwrap_or(entry_rel_str);
    let root_check = clean_rel_str.trim_start_matches('/');
    if root_check.is_empty() || root_check == "." || root_check == "./" {
        if clean_rel_str.contains('\\') {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        *state.entries_seen += 1;
        unsafe { archive_read_data_skip(a) };
        return Ok(None);
    }

    let target_path = sanitize_and_validate_path(state.destination_path, clean_rel_str)?;
    *state.entries_seen += 1;

    let size = unsafe { archive_entry_size(entry) }.max(0) as u64;
    let mode = unsafe { archive_entry_mode(entry) } as u32;
    let mtime = unsafe { archive_entry_mtime(entry) } as i64;
    let filetype = unsafe { archive_entry_filetype(entry) };
    let is_symlink = (filetype & (libc::S_IFMT as mode_t)) == (libc::S_IFLNK as mode_t);
    let is_dir = (filetype & (libc::S_IFMT as mode_t)) == (libc::S_IFDIR as mode_t)
        || (mode & (libc::S_IFMT as u32)) == (libc::S_IFDIR as u32)
        || entry_rel_str.ends_with('/');
    let is_enc = unsafe {
        archive_entry_is_data_encrypted(entry) != 0
            || archive_entry_is_metadata_encrypted(entry) != 0
    };
    if is_enc && state.options.password.is_null() {
        return Err(TTZipStatus::ErrInvalidPassword);
    }

    Ok(Some(ParsedHandleEntry {
        raw_path,
        clean_rel_str,
        target_path,
        size,
        mode,
        mtime,
        is_symlink,
        is_dir,
        is_enc,
    }))
}

unsafe fn try_apply_non_regular_entry(
    a: *mut c_void,
    entry: *mut c_void,
    parsed: &ParsedHandleEntry<'_>,
    entry_comps: &[String],
    hardlink_rel: &Option<String>,
    state: &mut HandleLoopState<'_>,
) -> Result<bool, TTZipStatus> {
    if parsed.is_symlink {
        let symlink_raw = unsafe { archive_entry_symlink(entry) };
        if symlink_raw.is_null() {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        let symlink_target = unsafe { CStr::from_ptr(symlink_raw) }
            .to_str()
            .map_err(|_| TTZipStatus::ErrSecurityViolation)?;
        state.jail.create_symlink_components(entry_comps, symlink_target)?;
        state.engine.register_entry_typed(
            parsed.target_path.clone(),
            parsed.mode,
            parsed.mtime,
            0,
            false,
            true,
        );
        unsafe { archive_read_data_skip(a) };
        return Ok(true);
    }
    if let (Some(src_link_rel), 0) = (hardlink_rel, parsed.size) {
        state
            .deferred_hardlinks
            .record_or_apply(state.jail, parsed.clean_rel_str, src_link_rel)?;
        state.engine.register_entry_typed(
            parsed.target_path.clone(),
            parsed.mode,
            parsed.mtime,
            0,
            false,
            false,
        );
        unsafe { archive_read_data_skip(a) };
        return Ok(true);
    }
    if parsed.is_dir {
        state.jail.create_dir_all_components(entry_comps)?;
        state.engine.register_entry_typed(
            parsed.target_path.clone(),
            parsed.mode,
            parsed.mtime,
            0,
            true,
            false,
        );
        unsafe { archive_read_data_skip(a) };
        return Ok(true);
    }
    Ok(false)
}

unsafe fn process_single_handle_entry(
    a: *mut c_void,
    entry: *mut c_void,
    state: &mut HandleLoopState<'_>,
) -> Result<(), TTZipStatus> {
    let Some(parsed) = (unsafe { parse_and_validate_handle_entry(a, entry, state)? }) else {
        return Ok(());
    };

    if state.options.dry_run {
        return unsafe {
            handle_dry_run_entry(
                a,
                parsed.is_dir,
                parsed.is_symlink,
                parsed.size,
                parsed.raw_path,
                state.options,
                state.active_sheet,
                state.total_processed,
            )
        };
    }

    let hardlink_rel = unsafe { resolve_entry_hardlink(entry, state.destination_path)? };
    let entry_comps = DirJail::extract_clean_components(parsed.clean_rel_str)?;
    state.deferred_hardlinks.invalidate_destination(&entry_comps);

    if unsafe {
        try_apply_non_regular_entry(a, entry, &parsed, &entry_comps, &hardlink_rel, state)?
    } {
        return Ok(());
    }

    let mut block_ctx = RegularBlockPumpCtx {
        jail: state.jail,
        clean_rel_str: parsed.clean_rel_str,
        raw_path: parsed.raw_path,
        size: parsed.size,
        is_enc: parsed.is_enc,
        options: state.options,
        active_sheet: state.active_sheet,
        bomb_guard: state.bomb_guard,
        total_processed: state.total_processed,
    };
    unsafe { pump_regular_entry_blocks(a, &mut block_ctx)? };
    state
        .engine
        .register_entry_typed(parsed.target_path, parsed.mode, parsed.mtime, 0, false, false);
    state
        .deferred_hardlinks
        .on_regular_committed(state.jail, &entry_comps, hardlink_rel.as_deref())
}

fn finalize_handle_extraction(
    archive_path: &Path,
    options: &TTZipExtractOptions,
    jail: &DirJail,
    engine: &mut SafeExtractEngine,
    deferred_hardlinks: DeferredHardlinks,
    active_sheet: &ProgressAtomicSheet,
    entries_seen: usize,
    total_processed: u64,
) -> Result<u64, TTZipStatus> {
    deferred_hardlinks.finalize(jail)?;

    if !options.dry_run {
        super::super::fallback_policy::verify_gzip_stream_if_applicable(archive_path)?;
        jail.verify_all_jail_symlinks()?;
    }

    if entries_seen == 0
        && total_processed == 0
        && !options.dry_run
        && archive_path.metadata().map(|m| m.len() > 0).unwrap_or(false)
    {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    active_sheet.mark_finished();

    if !options.dry_run {
        engine.apply_deferred_metadata(options.preserve_permissions)?;
    }

    Ok(total_processed)
}

pub(super) unsafe fn extract_from_archive_handle(
    a: *mut c_void,
    archive_path: &Path,
    destination_path: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<u64, TTZipStatus> {
    unsafe {
        libc::setlocale(libc::LC_ALL, c"en_US.UTF-8".as_ptr());

        let local_sheet = ProgressAtomicSheet::new();
        let active_sheet = sheet.unwrap_or(&local_sheet);

        if options.progress_callback.is_some()
            && active_sheet.total_bytes.load(Ordering::Relaxed) == 0
        {
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
        let mut deferred_hardlinks = DeferredHardlinks::default();
        let bomb_guard = ExpansionRatioGuard::default();

        loop {
            match next_libarchive_header(a, &mut entry, active_sheet)? {
                Some(false) => break,
                None => continue,
                Some(true) => {}
            }
            let mut state = HandleLoopState {
                destination_path,
                jail: &jail,
                engine: &mut engine,
                deferred_hardlinks: &mut deferred_hardlinks,
                bomb_guard: &bomb_guard,
                options,
                active_sheet,
                total_processed: &mut total_processed,
                entries_seen: &mut entries_seen,
            };
            process_single_handle_entry(a, entry, &mut state)?;
        }

        finalize_handle_extraction(
            archive_path,
            options,
            &jail,
            &mut engine,
            deferred_hardlinks,
            active_sheet,
            entries_seen,
            total_processed,
        )
    }
}
