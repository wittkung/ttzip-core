// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Streaming and multi-volume deep payload verification to an in-memory quota sink.

use super::tree_tracker::HardlinkTracker;
use super::{QuotaVerifySink, check_quota, deep_verify_tar};
use crate::cli::format::{ContainerFormat, detect_volume_chain};
use std::ffi::{CStr, CString};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use ttzip_archive::crypto::crc32::crc32_fast;
use ttzip_archive::ffi::archive_ffi::guards::archive_entry_pathname_safe;
use ttzip_archive::ffi::archive_ffi::sys::{
    archive_entry_filetype, archive_entry_hardlink, archive_entry_is_data_encrypted,
    archive_entry_is_metadata_encrypted, archive_entry_mode, archive_entry_size,
    archive_entry_symlink, archive_read_add_passphrase, archive_read_data_block,
    archive_read_data_skip, archive_read_next_header, get_archive_error_string,
};
use ttzip_archive::sevenz::decoder::archive::stream_7z_folder_from_reader;
use ttzip_archive::sevenz::{EncodedHeaderDecoder, SolidFolderIndex};
use ttzip_archive::zip::reader_stream::{
    extract_zip_entry_to_sink_from_reader, parse_entries_from_reader,
};
use ttzip_engine::archive::split::VirtualMultiVolumeReader;
use ttzip_engine::archive::stream_adapter::read::ArchiveStreamReader;
use ttzip_engine::archive::stream_adapter::{
    PreReadStream, ResolvedStreamKind, try_open_resolved_stream, try_transcode_special_container,
};
use ttzip_engine::security::{ExpansionRatioGuard, detect_overlapping_entries};
use ttzip_engine::types::TTZipStatus;

const STREAM_VERIFY_BUF_SIZE: usize = 65536;

unsafe fn drain_libarchive_entry_blocks(
    a: *mut libc::c_void,
    expected_size: u64,
    compressed_size: u64,
    bomb_guard: &ExpansionRatioGuard,
    sink: &mut QuotaVerifySink,
) -> Result<(), String> {
    let mut ptr: *const libc::c_void = std::ptr::null();
    let mut size: usize = 0;
    let mut offset: i64 = 0;
    let mut end_off: u64 = 0;
    loop {
        let rc = unsafe { archive_read_data_block(a, &mut ptr, &mut size, &mut offset) };
        if rc == 1 {
            break;
        }
        if rc == -10 {
            continue;
        }
        if rc != 0 {
            let msg = unsafe { get_archive_error_string(a) }.unwrap_or_else(|| "corrupt".into());
            return Err(format!("Deep payload verification failed: {}", msg));
        }
        if offset < 0 {
            return Err("Deep payload verification failed: negative block offset".to_string());
        }
        if size > 0 && !ptr.is_null() {
            let block_end = (offset as u64).saturating_add(size as u64);
            check_quota(block_end, sink.max_bytes)?;
            end_off = end_off.max(block_end);
            sink.record(size as u64)?;
            if bomb_guard.check(sink.total_bytes, compressed_size).is_err() {
                return Err(
                    "Deep verification aborted: expansion ratio limit exceeded".to_string()
                );
            }
        }
    }
    if expected_size > 0 && end_off < expected_size {
        return Err(format!(
            "Deep verification failed: truncated ({} < {})",
            end_off, expected_size
        ));
    }
    Ok(())
}

unsafe fn resolve_libarchive_hardlink(entry: *mut libc::c_void) -> Result<Option<String>, String> {
    let hl_raw = unsafe { archive_entry_hardlink(entry) };
    if hl_raw.is_null() {
        return Ok(None);
    }
    let target = unsafe { CStr::from_ptr(hl_raw) }
        .to_str()
        .map_err(|_| "Deep payload verification failed: non-UTF-8 hardlink target".to_string())?;
    let trimmed = target.trim_start_matches('/');
    if trimmed.is_empty() || trimmed == "." {
        return Err("Deep payload verification failed: empty hardlink target".to_string());
    }
    Ok(Some(target.to_string()))
}

unsafe fn apply_non_regular_libarchive_entry(
    entry: *mut libc::c_void,
    raw_path: &str,
    entry_comps: &[String],
    hardlinks: &mut HardlinkTracker,
) -> Result<bool, String> {
    let ftype = unsafe { archive_entry_filetype(entry) } as u32 & (libc::S_IFMT as u32);
    let mode = unsafe { archive_entry_mode(entry) } as u32 & (libc::S_IFMT as u32);
    let sym_ptr = unsafe { archive_entry_symlink(entry) };
    if ftype == (libc::S_IFLNK as u32) || mode == (libc::S_IFLNK as u32) || !sym_ptr.is_null() {
        if sym_ptr.is_null() {
            return Err("Deep payload verification failed: symlink missing target".to_string());
        }
        let target = unsafe { CStr::from_ptr(sym_ptr) }
            .to_str()
            .map_err(|_| "Deep payload verification failed: non-UTF-8 symlink target".to_string())?;
        hardlinks.on_symlink(entry_comps.to_vec(), target)?;
        return Ok(true);
    }
    if ftype == (libc::S_IFDIR as u32)
        || mode == (libc::S_IFDIR as u32)
        || raw_path.ends_with('/')
    {
        hardlinks.on_directory(entry_comps.to_vec())?;
        return Ok(true);
    }
    Ok(false)
}

unsafe fn read_libarchive_entry_path(
    entry: *mut libc::c_void,
    password: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(path_cstr) = (unsafe { archive_entry_pathname_safe(entry) }) else {
        return Ok(None);
    };
    let raw_path = path_cstr
        .to_str()
        .map_err(|_| "Deep payload verification failed: non-UTF-8 entry path".to_string())?;
    if raw_path.is_empty() {
        return Ok(None);
    }
    let is_enc = unsafe {
        archive_entry_is_data_encrypted(entry) != 0
            || archive_entry_is_metadata_encrypted(entry) != 0
    };
    if is_enc && password.is_none_or(str::is_empty) {
        return Err(
            "Deep payload verification failed: encrypted entry requires password".to_string()
        );
    }
    Ok(Some(raw_path.to_string()))
}

unsafe fn verify_libarchive_entry(
    a: *mut libc::c_void,
    entry: *mut libc::c_void,
    compressed_size: u64,
    password: Option<&str>,
    bomb_guard: &ExpansionRatioGuard,
    hardlinks: &mut HardlinkTracker,
    sink: &mut QuotaVerifySink,
) -> Result<bool, String> {
    let Some(raw_path) = unsafe { read_libarchive_entry_path(entry, password) }? else {
        unsafe { archive_read_data_skip(a) };
        return Ok(false);
    };
    let Some(entry_comps) = HardlinkTracker::parse_entry_components(&raw_path)? else {
        unsafe { archive_read_data_skip(a) };
        return Ok(true);
    };
    if unsafe { apply_non_regular_libarchive_entry(entry, &raw_path, &entry_comps, hardlinks) }? {
        unsafe { archive_read_data_skip(a) };
        return Ok(true);
    }
    let hardlink_target = unsafe { resolve_libarchive_hardlink(entry) }?;
    let raw_size = unsafe { archive_entry_size(entry) };
    if raw_size < 0 {
        return Err("Deep payload verification failed: negative entry size".to_string());
    }
    let expected = raw_size as u64;
    if let (Some(target), 0) = (&hardlink_target, expected) {
        hardlinks.record_hardlink(entry_comps, target)?;
        unsafe { archive_read_data_skip(a) };
        return Ok(true);
    }
    check_quota(sink.total_bytes.saturating_add(expected), sink.max_bytes)?;
    unsafe { drain_libarchive_entry_blocks(a, expected, compressed_size, bomb_guard, sink) }?;
    hardlinks.on_regular_committed(entry_comps, hardlink_target.as_deref())?;
    Ok(true)
}

unsafe fn verify_libarchive_handle(
    a: *mut libc::c_void,
    compressed_size: u64,
    password: Option<&str>,
    sink: &mut QuotaVerifySink,
) -> Result<usize, String> {
    let pwd_c = password
        .filter(|p| !p.is_empty())
        .and_then(|p| CString::new(p).ok());
    if let Some(ref p) = pwd_c {
        unsafe { archive_read_add_passphrase(a, p.as_ptr()) };
    }
    let bomb_guard = ExpansionRatioGuard::default();
    let mut hardlinks = HardlinkTracker::default();
    let mut entry: *mut libc::c_void = std::ptr::null_mut();
    let mut seen: usize = 0;
    loop {
        let rc = unsafe { archive_read_next_header(a, &mut entry) };
        if rc == 1 {
            break;
        }
        if rc == -10 {
            continue;
        }
        if rc != 0 {
            let msg =
                unsafe { get_archive_error_string(a) }.unwrap_or_else(|| "bad header".into());
            return Err(format!("Deep payload verification failed: {}", msg));
        }
        if entry.is_null() {
            break;
        }
        if unsafe {
            verify_libarchive_entry(
                a,
                entry,
                compressed_size,
                password,
                &bomb_guard,
                &mut hardlinks,
                sink,
            )
        }? {
            seen += 1;
        }
    }
    hardlinks.finalize()?;
    Ok(seen)
}

fn verify_tar_container_stream(
    pre: PreReadStream,
    compressed_size: u64,
    password: Option<&str>,
    max_output_bytes: u64,
) -> Result<usize, String> {
    let mut reader = ArchiveStreamReader::open_sequential(pre, STREAM_VERIFY_BUF_SIZE)
        .map_err(|e| format!("Deep payload verification failed: {:?}", e))?;
    let mut sink = QuotaVerifySink::new(max_output_bytes);
    unsafe {
        verify_libarchive_handle(
            reader.as_raw_archive(),
            compressed_size,
            password,
            &mut sink,
        )
    }?;
    reader
        .verify_underlying_eof()
        .map_err(|e| format!("Deep payload verification failed: {:?}", e))?;
    if !reader.snapshot().is_eof {
        return Err(
            "Deep payload verification failed: excessive trailing stream data".to_string()
        );
    }
    Ok(0)
}

fn verify_single_file_stream(
    mut stream: PreReadStream,
    compressed_size: u64,
    max_output_bytes: u64,
) -> Result<usize, String> {
    let bomb_guard = ExpansionRatioGuard::default();
    let mut sink = QuotaVerifySink::new(max_output_bytes);
    let mut buf = [0u8; STREAM_VERIFY_BUF_SIZE];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => return Ok(0),
            Ok(n) => {
                sink.record(n as u64)?;
                if bomb_guard.check(sink.total_bytes, compressed_size).is_err() {
                    return Err(
                        "Deep verification aborted: expansion ratio limit exceeded".to_string()
                    );
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(format!("Deep payload verification failed: {}", e)),
        }
    }
}

fn verify_seekable_zip_symlink(
    vr: &mut VirtualMultiVolumeReader,
    compressed_size: u64,
    entry: &ttzip_archive::zip::ZipEntry,
    entry_comps: Option<Vec<String>>,
    password: Option<&str>,
    tracker: &mut HardlinkTracker,
    sink: &mut QuotaVerifySink,
) -> Result<(), String> {
    check_quota(
        sink.total_bytes.saturating_add(entry.uncompressed_size),
        sink.max_bytes,
    )?;
    let mut sym_bytes = Vec::new();
    extract_zip_entry_to_sink_from_reader(
        vr,
        compressed_size,
        entry,
        password,
        &mut sym_bytes,
        None,
    )
    .map_err(|e| format!("ZIP symlink verification failed for '{}': {:?}", entry.rel_path, e))?;
    sink.record(sym_bytes.len() as u64)?;
    let target = std::str::from_utf8(&sym_bytes).map_err(|_| {
        "Deep payload verification failed: non-UTF-8 ZIP symlink target".to_string()
    })?;
    let comps = entry_comps.ok_or_else(|| "Invalid ZIP symlink path".to_string())?;
    tracker.on_symlink(comps, target)
}

fn verify_seekable_zip_entry(
    vr: &mut VirtualMultiVolumeReader,
    compressed_size: u64,
    entry: &ttzip_archive::zip::ZipEntry,
    password: Option<&str>,
    tracker: &mut HardlinkTracker,
    sink: &mut QuotaVerifySink,
) -> Result<(), String> {
    if entry.is_encrypted && password.is_none_or(str::is_empty) {
        return Err(
            "Deep payload verification failed: encrypted ZIP entry requires password".to_string(),
        );
    }
    let entry_comps = HardlinkTracker::parse_entry_components(&entry.rel_path)?;
    if entry.is_directory {
        if let Some(comps) = entry_comps {
            tracker.on_directory(comps)?;
        }
        return Ok(());
    }
    if (entry.mode & 0o170000) == 0o120000 {
        return verify_seekable_zip_symlink(
            vr,
            compressed_size,
            entry,
            entry_comps,
            password,
            tracker,
            sink,
        );
    }
    if entry.uncompressed_size > 0 || entry.is_encrypted {
        extract_zip_entry_to_sink_from_reader(vr, compressed_size, entry, password, sink, None)
            .map_err(|e| format!("Verification failed for '{}': {:?}", entry.rel_path, e))?;
    }
    if let Some(comps) = entry_comps {
        tracker.on_regular_committed(comps, None)?;
    }
    Ok(())
}

fn verify_seekable_zip(
    vr: &mut VirtualMultiVolumeReader,
    compressed_size: u64,
    password: Option<&str>,
    max_output_bytes: u64,
) -> Result<usize, String> {
    let entries = parse_entries_from_reader(vr, compressed_size)
        .map_err(|e| format!("Failed to parse split ZIP central directory: {:?}", e))?;
    let intervals: Vec<(u64, u64)> = entries
        .iter()
        .map(|e| (e.lfh_offset, e.compressed_size))
        .collect();
    if detect_overlapping_entries(&intervals) {
        return Err("Deep payload verification failed: overlapping ZIP entries".to_string());
    }
    let declared = entries
        .iter()
        .fold(0u64, |a, e| a.saturating_add(e.uncompressed_size));
    check_quota(declared, max_output_bytes)?;
    let mut sink = QuotaVerifySink::new(max_output_bytes);
    let mut tracker = HardlinkTracker::default();
    for entry in &entries {
        verify_seekable_zip_entry(vr, compressed_size, entry, password, &mut tracker, &mut sink)?;
    }
    tracker.finalize()?;
    Ok(0)
}

enum SevenZVerifyKind {
    Directory,
    Regular,
    Symlink { target_bytes: Vec<u8> },
}

struct PendingSevenZVerifyItem {
    rel_path: String,
    entry_comps: Option<Vec<String>>,
    size: u64,
    expected_crc: Option<u32>,
    kind: SevenZVerifyKind,
    written: u64,
    running_crc: u32,
}

fn build_sevenz_verify_item(
    idx: usize,
    file: &ttzip_archive::sevenz::SevenZFileMeta,
    folder_index: &SolidFolderIndex,
) -> Result<PendingSevenZVerifyItem, String> {
    let entry_comps = HardlinkTracker::parse_entry_components(&file.rel_path)?;
    if file.is_directory {
        return Ok(PendingSevenZVerifyItem {
            rel_path: file.rel_path.clone(),
            entry_comps,
            size: 0,
            expected_crc: None,
            kind: SevenZVerifyKind::Directory,
            written: 0,
            running_crc: 0,
        });
    }
    let (size, expected_crc) = if file.is_empty_stream {
        (0, None)
    } else {
        let range = folder_index
            .lookup(idx)
            .ok_or_else(|| format!("Missing 7z stream range for '{}'", file.rel_path))?;
        (range.uncompressed_size, range.crc)
    };
    let is_symlink = (file.mode & 0o170000) == 0o120000;
    let kind = if is_symlink {
        if size == 0 || size > 4096 {
            return Err(
                "Deep payload verification failed: invalid 7z symlink target size".to_string(),
            );
        }
        SevenZVerifyKind::Symlink {
            target_bytes: Vec::with_capacity(size as usize),
        }
    } else {
        SevenZVerifyKind::Regular
    };
    Ok(PendingSevenZVerifyItem {
        rel_path: file.rel_path.clone(),
        entry_comps,
        size,
        expected_crc,
        kind,
        written: 0,
        running_crc: 0,
    })
}

fn finalize_sevenz_verify_item(
    item: &PendingSevenZVerifyItem,
    tracker: &mut HardlinkTracker,
) -> Result<(), String> {
    if let Some(exp_crc) = item.expected_crc {
        if item.running_crc != exp_crc {
            return Err(format!(
                "Verification failed for '{}': 7z CRC mismatch",
                item.rel_path
            ));
        }
    }
    match &item.kind {
        SevenZVerifyKind::Directory => {
            if let Some(ref comps) = item.entry_comps {
                tracker.on_directory(comps.clone())?;
            }
            Ok(())
        }
        SevenZVerifyKind::Regular => {
            if let Some(ref comps) = item.entry_comps {
                tracker.on_regular_committed(comps.clone(), None)?;
            }
            Ok(())
        }
        SevenZVerifyKind::Symlink { target_bytes } => {
            let target = std::str::from_utf8(target_bytes).map_err(|_| {
                "Deep payload verification failed: non-UTF-8 7z symlink target".to_string()
            })?;
            let comps = item
                .entry_comps
                .clone()
                .ok_or_else(|| "Invalid 7z symlink path".to_string())?;
            tracker.on_symlink(comps, target)
        }
    }
}

fn flush_ready_sevenz_items(
    items: &[PendingSevenZVerifyItem],
    cur_idx: &mut usize,
    tracker: &mut HardlinkTracker,
) -> Result<(), String> {
    while *cur_idx < items.len() && items[*cur_idx].written == items[*cur_idx].size {
        finalize_sevenz_verify_item(&items[*cur_idx], tracker)?;
        *cur_idx += 1;
    }
    Ok(())
}

fn consume_sevenz_verify_chunk(
    mut chunk: &[u8],
    items: &mut [PendingSevenZVerifyItem],
    cur_idx: &mut usize,
    tracker: &mut HardlinkTracker,
    sink: &mut QuotaVerifySink,
) -> Result<(), String> {
    sink.record(chunk.len() as u64)?;
    flush_ready_sevenz_items(items, cur_idx, tracker)?;
    while !chunk.is_empty() && *cur_idx < items.len() {
        let item = &mut items[*cur_idx];
        let take = (chunk.len() as u64).min(item.size.saturating_sub(item.written)) as usize;
        let (slice, rest) = chunk.split_at(take);
        chunk = rest;
        if let SevenZVerifyKind::Symlink { target_bytes } = &mut item.kind {
            target_bytes.extend_from_slice(slice);
        }
        item.running_crc = crc32_fast(item.running_crc, slice);
        item.written += take as u64;
        if item.written == item.size {
            flush_ready_sevenz_items(items, cur_idx, tracker)?;
        }
    }
    if !chunk.is_empty() {
        return Err("Deep payload verification failed: trailing 7z stream bytes".to_string());
    }
    Ok(())
}

fn stream_all_sevenz_folders(
    vr: &mut VirtualMultiVolumeReader,
    info: &ttzip_archive::sevenz::SevenZHeaderInfo,
    password: Option<&str>,
    items: &mut [PendingSevenZVerifyItem],
    cur_idx: &mut usize,
    tracker: &mut HardlinkTracker,
    sink: &mut QuotaVerifySink,
) -> Result<(), String> {
    let mut verify_err: Option<String> = None;
    let mut sink_fn = |chunk: &[u8]| -> Result<(), TTZipStatus> {
        consume_sevenz_verify_chunk(chunk, items, cur_idx, tracker, sink).map_err(|e| {
            verify_err = Some(e);
            TTZipStatus::ErrExtractionFailed
        })
    };
    for folder in &info.folders {
        if let Err(status) =
            stream_7z_folder_from_reader(vr, info, folder, password, 1, &mut sink_fn)
        {
            if let Some(err) = verify_err.take() {
                return Err(err);
            }
            return Err(format!("Deep payload verification failed: {:?}", status));
        }
    }
    Ok(())
}

fn verify_seekable_sevenz(
    vr: &mut VirtualMultiVolumeReader,
    password: Option<&str>,
    max_output_bytes: u64,
) -> Result<usize, String> {
    let info = EncodedHeaderDecoder::default()
        .decode_from_reader(vr, password)
        .map_err(|e| format!("Failed to parse split 7z header: {:?}", e))?;
    if info.is_encrypted && password.is_none_or(str::is_empty) {
        return Err(
            "Deep payload verification failed: encrypted 7z archive requires password".to_string(),
        );
    }
    let s_sum = info
        .stream_sizes
        .iter()
        .copied()
        .fold(0u64, u64::saturating_add);
    let folder_index = SolidFolderIndex::build(&info);
    let f_sum = folder_index
        .folders
        .iter()
        .fold(0u64, |a, f| a.saturating_add(f.total_uncompressed_size));
    check_quota(s_sum.max(f_sum), max_output_bytes)?;
    let mut items = Vec::with_capacity(info.files.len());
    for (idx, file) in info.files.iter().enumerate() {
        items.push(build_sevenz_verify_item(idx, file, &folder_index)?);
    }
    let mut sink = QuotaVerifySink::new(max_output_bytes);
    let mut tracker = HardlinkTracker::default();
    let mut cur_idx = 0usize;
    flush_ready_sevenz_items(&items, &mut cur_idx, &mut tracker)?;
    stream_all_sevenz_folders(
        vr,
        &info,
        password,
        &mut items,
        &mut cur_idx,
        &mut tracker,
        &mut sink,
    )?;
    flush_ready_sevenz_items(&items, &mut cur_idx, &mut tracker)?;
    if cur_idx < items.len() {
        return Err("Deep payload verification failed: incomplete 7z solid stream".to_string());
    }
    tracker.finalize()?;
    Ok(0)
}

fn verify_seekable_volume_stream(
    volume_chain: Vec<PathBuf>,
    fmt: ContainerFormat,
    password: Option<&str>,
    max_output_bytes: u64,
) -> Result<usize, String> {
    let mut vr = VirtualMultiVolumeReader::from_volumes(volume_chain)
        .map_err(|e| format!("Failed to open archive stream: {}", e))?;
    let compressed_size = vr.total_size();
    match fmt {
        ContainerFormat::Zip => {
            return verify_seekable_zip(&mut vr, compressed_size, password, max_output_bytes);
        }
        ContainerFormat::SevenZip => {
            return verify_seekable_sevenz(&mut vr, password, max_output_bytes);
        }
        ContainerFormat::Tar => {
            let mut tail = [0u8; 512];
            if compressed_size < 1024
                || vr.seek(SeekFrom::End(-512)).is_err()
                || vr.read_exact(&mut tail).is_err()
                || tail.iter().any(|&b| b != 0)
                || vr.seek(SeekFrom::Start(0)).is_err()
            {
                return Err(
                    "Deep payload verification failed: missing TAR end-of-archive trailer"
                        .to_string(),
                );
            }
        }
        _ => {}
    }
    let sr = ArchiveStreamReader::open_seekable(vr, STREAM_VERIFY_BUF_SIZE)
        .map_err(|e| format!("Deep payload verification failed: {:?}", e))?;
    let mut sink = QuotaVerifySink::new(max_output_bytes);
    unsafe { verify_libarchive_handle(sr.as_raw_archive(), compressed_size, password, &mut sink) }?;
    Ok(0)
}

fn verify_stream_inner(
    archive_path: &Path,
    fmt: ContainerFormat,
    password: Option<&str>,
    max_output_bytes: u64,
    errors: &mut Vec<String>,
) -> Result<usize, String> {
    let chain =
        detect_volume_chain(archive_path).unwrap_or_else(|_| vec![archive_path.to_path_buf()]);
    match try_open_resolved_stream(archive_path, &chain)
        .map_err(|e| format!("Deep payload verification failed: {:?}", e))?
    {
        Some(ResolvedStreamKind::TarContainer(pre)) => {
            let comp_sz = chain
                .iter()
                .filter_map(|p| std::fs::metadata(p).ok())
                .fold(0u64, |a, m| a.saturating_add(m.len()));
            return verify_tar_container_stream(pre, comp_sz, password, max_output_bytes);
        }
        Some(ResolvedStreamKind::SingleFileStream {
            stream,
            compressed_size,
            ..
        }) => {
            return verify_single_file_stream(stream, compressed_size, max_output_bytes);
        }
        None => {}
    }
    if let Some(transcoded) = try_transcode_special_container(archive_path, &chain)
        .map_err(|e| format!("Deep payload verification failed: {:?}", e))?
    {
        if transcoded.entry_count == 0 {
            return Ok(0);
        }
        let buf = transcoded.cursor.get_ref();
        check_quota(buf.len() as u64, max_output_bytes)?;
        return Ok(deep_verify_tar(buf, errors));
    }
    verify_seekable_volume_stream(chain, fmt, password, max_output_bytes)
}

pub(super) fn deep_verify_stream_extract_with_quota(
    archive_path: &Path,
    fmt: ContainerFormat,
    password: Option<&str>,
    max_output_bytes: u64,
    errors: &mut Vec<String>,
) -> usize {
    match verify_stream_inner(archive_path, fmt, password, max_output_bytes, errors) {
        Ok(corrupted) => corrupted,
        Err(err) => {
            errors.push(err);
            1
        }
    }
}
