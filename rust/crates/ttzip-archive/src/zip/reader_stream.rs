// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Seekable stream-based ZIP Archive Reader for multi-volume `.zip.001` and `.z01..zip` containers.

use crate::archive::unified::selection::{normalize_entry_path, EntrySelection};
use crate::archive::unified::zip_central_directory::locate_zip_central_directory_reader;
use crate::crypto::crc32::crc32_fast;
use crate::crypto::sha1::winzip_aes256_decrypt_and_verify;
use crate::fs::apfs::apfs_preallocate;
use crate::fs::safe_extract::{sanitize_and_validate_path, DirJail, SafeExtractEngine};
use crate::security::zip_defense::{detect_overlapping_entries, ExtractionQuotaGuard};
use crate::types::{TTZipExtractOptions, TTZipStatus};
use crate::zip::parser::{parse_cdfh_entry, ZipEntry, MAGIC_LFH};
use crate::zip::reader::{ZipExtractReport, MAX_IN_MEMORY_DECOMPRESS_BYTES};
use std::ffi::CString;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::io::AsRawFd;
use std::path::Path;
use ttzip_types::options::ProgressAtomicSheet;

const STREAM_DECODE_CHUNK: usize = 64 * 1024;

/// Parses all Central Directory entries from a seekable ZIP stream (`VirtualMultiVolumeReader`).
pub fn parse_entries_from_reader<R: Read + Seek>(
    reader: &mut R,
    total_len: u64,
) -> Result<Vec<ZipEntry>, TTZipStatus> {
    let (cd_offset, cd_size, total_entries) = locate_zip_central_directory_reader(reader, total_len)?;
    let cd_end = cd_offset.checked_add(cd_size).ok_or(TTZipStatus::ErrCorruptHeader)?;
    if cd_end > total_len {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let cd_len = usize::try_from(cd_size).map_err(|_| TTZipStatus::ErrOutOfMemory)?;
    let mut cd_buf = vec![0u8; cd_len];
    reader
        .seek(SeekFrom::Start(cd_offset))
        .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    reader
        .read_exact(&mut cd_buf)
        .map_err(|_| TTZipStatus::ErrCorruptHeader)?;

    let mut entries = Vec::with_capacity(total_entries.min(65536) as usize);
    let mut pos = 0usize;
    while pos < cd_len {
        let (entry, next_pos) = parse_cdfh_entry(&cd_buf, pos)?;
        entries.push(entry);
        pos = next_pos;
    }
    Ok(entries)
}

struct RawEntryPayload {
    bytes: Vec<u8>,
    lfh_extra: Vec<u8>,
    lfh_flag: u16,
    lfh_dos_time: u16,
}

fn read_entry_raw_payload<R: Read + Seek>(
    reader: &mut R,
    total_len: u64,
    entry: &ZipEntry,
) -> Result<RawEntryPayload, TTZipStatus> {
    if entry.lfh_offset.saturating_add(30) > total_len {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    reader
        .seek(SeekFrom::Start(entry.lfh_offset))
        .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    let mut lfh = [0u8; 30];
    reader
        .read_exact(&mut lfh)
        .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    let sig = u32::from_le_bytes([lfh[0], lfh[1], lfh[2], lfh[3]]);
    if sig != MAGIC_LFH {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let lfh_flag = u16::from_le_bytes([lfh[6], lfh[7]]);
    let lfh_dos_time = u16::from_le_bytes([lfh[10], lfh[11]]);
    let fn_len = u16::from_le_bytes([lfh[26], lfh[27]]) as u64;
    let extra_len = u16::from_le_bytes([lfh[28], lfh[29]]) as u64;
    let extra_offset = entry
        .lfh_offset
        .checked_add(30)
        .and_then(|v| v.checked_add(fn_len))
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    let payload_offset = extra_offset
        .checked_add(extra_len)
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    let payload_end = payload_offset
        .checked_add(entry.compressed_size)
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    if payload_end > total_len {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let mut lfh_extra = vec![0u8; extra_len as usize];
    if extra_len > 0 {
        reader
            .seek(SeekFrom::Start(extra_offset))
            .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
        reader
            .read_exact(&mut lfh_extra)
            .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    }
    let comp_len = usize::try_from(entry.compressed_size).map_err(|_| TTZipStatus::ErrOutOfMemory)?;
    let mut bytes = vec![0u8; comp_len];
    reader
        .seek(SeekFrom::Start(payload_offset))
        .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    reader
        .read_exact(&mut bytes)
        .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    Ok(RawEntryPayload {
        bytes,
        lfh_extra,
        lfh_flag,
        lfh_dos_time,
    })
}

pub(super) fn decrypt_zipcrypto_slice(
    raw_payload: &[u8],
    password: &str,
    crc32: u32,
    lfh_dos_time: u16,
    bit3_data_descriptor: bool,
) -> Result<Vec<u8>, TTZipStatus> {
    if raw_payload.len() < 12 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let header: &[u8; 12] = raw_payload[..12]
        .try_into()
        .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    let mut engine = crate::crypto::zipcrypto::ZipCryptoEngine::verify_and_init(
        password.as_bytes(),
        header,
        crc32,
        lfh_dos_time,
        bit3_data_descriptor,
    )?;
    let mut decrypted = raw_payload[12..].to_vec();
    engine.decrypt_slice(&mut decrypted);
    Ok(decrypted)
}

#[inline]
pub(super) fn is_valid_winzip_aes_entry(entry: &ZipEntry) -> bool {
    entry.is_encrypted
        && entry.compression_method == 99
        && matches!(
            entry.aes_version,
            crate::zip::extra::WINZIP_AES_VERSION_AE1
                | crate::zip::extra::WINZIP_AES_VERSION_AE2
        )
        && (1..=3).contains(&entry.aes_strength)
}

pub(crate) fn validate_lfh_aes_extra(entry: &ZipEntry, lfh_extra_bytes: &[u8]) -> Result<(), TTZipStatus> {
    if lfh_extra_bytes.is_empty() {
        return Ok(());
    }
    let lfh_extra =
        crate::zip::extra::ZipExtraFields::parse(lfh_extra_bytes, false, false, false, false);
    if lfh_extra.has_winzip_aes
        && (!crate::zip::parser::is_valid_winzip_aes_extra(&lfh_extra)
            || lfh_extra.aes_version != entry.aes_version
            || lfh_extra.aes_strength != entry.aes_strength
            || lfh_extra.aes_actual_method != entry.actual_method)
    {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    Ok(())
}

pub(super) fn decrypt_slice_entry_payload<'a>(
    data: &'a [u8],
    entry: &ZipEntry,
    lfh_offset: usize,
    raw_payload: &'a [u8],
    password: Option<&str>,
) -> Result<std::borrow::Cow<'a, [u8]>, TTZipStatus> {
    if !entry.is_encrypted {
        return Ok(std::borrow::Cow::Borrowed(raw_payload));
    }
    let pass = password
        .filter(|s| !s.is_empty())
        .ok_or(TTZipStatus::ErrInvalidPassword)?;
    if entry.compression_method == 99 {
        if !is_valid_winzip_aes_entry(entry) || raw_payload.len() < 28 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let fn_len = u16::from_le_bytes([data[lfh_offset + 26], data[lfh_offset + 27]]) as usize;
        let extra_len = u16::from_le_bytes([data[lfh_offset + 28], data[lfh_offset + 29]]) as usize;
        let extra_start = lfh_offset + 30 + fn_len;
        validate_lfh_aes_extra(entry, &data[extra_start..extra_start + extra_len])?;
        let mut decrypted = vec![0u8; raw_payload.len() - 28];
        let dec_len = winzip_aes256_decrypt_and_verify(pass, raw_payload, &mut decrypted)?;
        decrypted.truncate(dec_len);
        Ok(std::borrow::Cow::Owned(decrypted))
    } else {
        let lfh_flag = u16::from_le_bytes([data[lfh_offset + 6], data[lfh_offset + 7]]);
        let lfh_dos_time = u16::from_le_bytes([data[lfh_offset + 10], data[lfh_offset + 11]]);
        let bit3 = ((entry.flag | lfh_flag) & 0x0008) != 0;
        let decrypted =
            decrypt_zipcrypto_slice(raw_payload, pass, entry.crc32, lfh_dos_time, bit3)?;
        Ok(std::borrow::Cow::Owned(decrypted))
    }
}

pub(super) fn map_zipcrypto_decode_err(entry: &ZipEntry, err: TTZipStatus) -> TTZipStatus {
    if entry.is_encrypted
        && entry.compression_method != 99
        && matches!(err, TTZipStatus::ErrCorruptHeader)
    {
        TTZipStatus::ErrInvalidPassword
    } else {
        err
    }
}

#[inline]
pub(super) fn should_verify_entry_crc32(entry: &ZipEntry) -> bool {
    !(is_valid_winzip_aes_entry(entry)
        && entry.aes_version == crate::zip::extra::WINZIP_AES_VERSION_AE2)
}

pub(super) fn decompress_deflate_in_memory(
    entry: &ZipEntry,
    effective_payload: &[u8],
    out_buffer: &mut [u8],
) -> Result<usize, TTZipStatus> {
    use crate::codecs::deflate::{with_thread_local_decompressor, DeflateDecompressError};
    let precise_res = with_thread_local_decompressor(|dec| {
        Ok(dec.decompress_precise(effective_payload, out_buffer))
    })?;
    match precise_res {
        Ok(sz) if sz == out_buffer.len() => Ok(sz),
        Ok(_) | Err(DeflateDecompressError::BadData | DeflateDecompressError::ShortOutput) => {
            Err(map_zipcrypto_decode_err(entry, TTZipStatus::ErrCorruptHeader))
        }
        Err(DeflateDecompressError::InsufficientSpace) => {
            let mut decoder = flate2::read::DeflateDecoder::new(effective_payload);
            let mut sink = std::io::sink();
            let _ = stream_decoded_reader_to_sink(
                &mut decoder,
                entry.uncompressed_size,
                &mut sink,
                None,
            )
            .map_err(|e| map_zipcrypto_decode_err(entry, e))?;
            Err(map_zipcrypto_decode_err(entry, TTZipStatus::ErrCorruptHeader))
        }
    }
}

fn decrypt_entry_payload(
    entry: &ZipEntry,
    raw: RawEntryPayload,
    password: Option<&str>,
) -> Result<Vec<u8>, TTZipStatus> {
    if !entry.is_encrypted {
        return Ok(raw.bytes);
    }
    let pass = password.filter(|s| !s.is_empty()).ok_or(TTZipStatus::ErrInvalidPassword)?;
    if entry.compression_method == 99 {
        if !is_valid_winzip_aes_entry(entry) || raw.bytes.len() < 28 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        validate_lfh_aes_extra(entry, &raw.lfh_extra)?;
        let cipher_len = raw.bytes.len() - 28;
        let mut decrypted = vec![0u8; cipher_len];
        let dec_len = winzip_aes256_decrypt_and_verify(pass, &raw.bytes, &mut decrypted)?;
        decrypted.truncate(dec_len);
        Ok(decrypted)
    } else {
        let bit3 = ((entry.flag | raw.lfh_flag) & 0x0008) != 0;
        decrypt_zipcrypto_slice(&raw.bytes, pass, entry.crc32, raw.lfh_dos_time, bit3)
    }
}

pub(super) fn stream_stored_slice_to_sink<W: Write>(
    payload: &[u8],
    expected_uncomp_size: u64,
    sink: &mut W,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<(u64, u32), TTZipStatus> {
    if (payload.len() as u64) != expected_uncomp_size {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let mut computed_crc = 0u32;
    let mut total_written = 0u64;
    for piece in payload.chunks(STREAM_DECODE_CHUNK) {
        if sheet.is_some_and(|s| s.is_cancelled()) {
            return Err(TTZipStatus::Cancelled);
        }
        sink.write_all(piece).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
        computed_crc = crc32_fast(computed_crc, piece);
        total_written += piece.len() as u64;
        if let Some(s) = sheet {
            s.add_processed(piece.len() as u64);
        }
    }
    Ok((total_written, computed_crc))
}

pub(super) fn stream_decoded_reader_to_sink<D: Read, W: Write>(
    decoder: &mut D,
    expected_uncomp_size: u64,
    sink: &mut W,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<(u64, u32), TTZipStatus> {
    let mut computed_crc = 0u32;
    let mut total_written = 0u64;
    let mut chunk = [0u8; STREAM_DECODE_CHUNK];
    loop {
        if sheet.is_some_and(|s| s.is_cancelled()) {
            return Err(TTZipStatus::Cancelled);
        }
        let n = decoder.read(&mut chunk).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
        if n == 0 {
            break;
        }
        if total_written.saturating_add(n as u64) > expected_uncomp_size.saturating_add(4096) {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        sink.write_all(&chunk[..n]).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
        computed_crc = crc32_fast(computed_crc, &chunk[..n]);
        total_written += n as u64;
        if let Some(s) = sheet {
            s.add_processed(n as u64);
        }
    }
    if total_written != expected_uncomp_size {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    Ok((total_written, computed_crc))
}

fn verify_extracted_entry_integrity(
    entry: &ZipEntry,
    total_written: u64,
    computed_crc: u32,
) -> Result<u64, TTZipStatus> {
    if total_written != entry.uncompressed_size {
        return Err(map_zipcrypto_decode_err(entry, TTZipStatus::ErrCorruptHeader));
    }
    if should_verify_entry_crc32(entry) && computed_crc != entry.crc32 {
        return Err(map_zipcrypto_decode_err(entry, TTZipStatus::ErrCorruptHeader));
    }
    Ok(total_written)
}

pub(super) fn decode_decrypted_payload_to_sink<W: Write>(
    entry: &ZipEntry,
    effective_payload: &[u8],
    sink: &mut W,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<(u64, u32), TTZipStatus> {
    let raw_res = match entry.actual_method {
        0 => stream_stored_slice_to_sink(effective_payload, entry.uncompressed_size, sink, sheet),
        8 => {
            let mut decoder = flate2::read::DeflateDecoder::new(effective_payload);
            stream_decoded_reader_to_sink(&mut decoder, entry.uncompressed_size, sink, sheet)
        }
        93 => {
            let mut decoder = crate::codecs::zstd::stream::ZstdStreamReader::new(effective_payload)
                .map_err(|e| map_zipcrypto_decode_err(entry, e))?;
            stream_decoded_reader_to_sink(&mut decoder, entry.uncompressed_size, sink, sheet)
        }
        _ => return Err(TTZipStatus::ErrArchiveInitFailed),
    };
    raw_res.map_err(|e| map_zipcrypto_decode_err(entry, e))
}

/// Streams a single `ZipEntry` from a seekable reader into `sink`.
pub fn extract_zip_entry_to_sink_from_reader<R: Read + Seek, W: Write>(
    reader: &mut R,
    total_len: u64,
    entry: &ZipEntry,
    password: Option<&str>,
    sink: &mut W,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<u64, TTZipStatus> {
    if entry.is_directory || (entry.uncompressed_size == 0 && !entry.is_encrypted) {
        return Ok(0);
    }
    if entry.uncompressed_size == 0 && entry.compressed_size == 0 {
        let _ = password.filter(|s| !s.is_empty()).ok_or(TTZipStatus::ErrInvalidPassword)?;
        return Ok(0);
    }
    let raw_payload = read_entry_raw_payload(reader, total_len, entry)?;
    let effective_payload = decrypt_entry_payload(entry, raw_payload, password)?;
    let (total_written, computed_crc) =
        decode_decrypted_payload_to_sink(entry, &effective_payload, sink, sheet)?;

    verify_extracted_entry_integrity(entry, total_written, computed_crc)
}

/// Extracts a single entry into memory from a seekable ZIP stream (`VirtualMultiVolumeReader`).
pub fn extract_zip_single_entry_from_reader<R: Read + Seek>(
    reader: &mut R,
    total_len: u64,
    entry_path: Option<&str>,
    entry_index: i64,
    password: Option<&str>,
) -> Result<Vec<u8>, TTZipStatus> {
    let entries = parse_entries_from_reader(reader, total_len)?;
    let entry = if entry_index >= 0 {
        entries
            .get(entry_index as usize)
            .ok_or(TTZipStatus::ErrFileNotFound)?
    } else if let Some(target_path) = entry_path {
        let norm_target = normalize_entry_path(target_path);
        entries
            .iter()
            .find(|e| normalize_entry_path(&e.rel_path) == norm_target || e.rel_path == target_path)
            .ok_or(TTZipStatus::ErrFileNotFound)?
    } else {
        return Err(TTZipStatus::ErrInvalidParam);
    };

    if entry.uncompressed_size as usize > MAX_IN_MEMORY_DECOMPRESS_BYTES {
        return Err(TTZipStatus::ErrOutOfMemory);
    }
    let mut out = Vec::with_capacity(entry.uncompressed_size as usize);
    extract_zip_entry_to_sink_from_reader(reader, total_len, entry, password, &mut out, None)?;
    Ok(out)
}

#[allow(clippy::too_many_arguments, reason = "Internal reader-to-jail extraction step carries stream, jail, and progress state")]
fn extract_reader_entry_to_disk<R: Read + Seek>(
    reader: &mut R,
    total_len: u64,
    jail: &DirJail,
    out_rel: &str,
    overwrite: bool,
    entry: &ZipEntry,
    password: Option<&str>,
    sheet: &ProgressAtomicSheet,
    options: &TTZipExtractOptions,
    total_uncomp_bytes: u64,
    cancel_sheet_on_abort: bool,
) -> Result<(), TTZipStatus> {
    if entry.is_encrypted && password.filter(|p| !p.is_empty()).is_none() {
        return Err(TTZipStatus::ErrInvalidPassword);
    }

    if entry.is_directory {
        jail.create_dir_all_in_jail(out_rel)?;
        return notify_zip_stream_progress(
            options,
            sheet,
            total_uncomp_bytes,
            &entry.rel_path,
            cancel_sheet_on_abort,
        );
    }

    if (entry.mode & 0o170000) == 0o120000 {
        let mut sym_bytes = Vec::new();
        extract_zip_entry_to_sink_from_reader(
            reader,
            total_len,
            entry,
            password,
            &mut sym_bytes,
            Some(sheet),
        )?;
        let target_str =
            std::str::from_utf8(&sym_bytes).map_err(|_| TTZipStatus::ErrSecurityViolation)?;
        jail.create_symlink_in_jail(out_rel, target_str)?;
        return notify_zip_stream_progress(
            options,
            sheet,
            total_uncomp_bytes,
            &entry.rel_path,
            cancel_sheet_on_abort,
        );
    }

    let mut inflight = crate::fs::abort_guard::InFlightJailFile::open(jail, out_rel, overwrite)?;
    if entry.uncompressed_size > 0 {
        let _ = apfs_preallocate(inflight.file_mut().as_raw_fd(), entry.uncompressed_size as i64);
        extract_zip_entry_to_sink_from_reader(
            reader,
            total_len,
            entry,
            password,
            inflight.file_mut(),
            Some(sheet),
        )?;
    }
    notify_zip_stream_progress(
        options,
        sheet,
        total_uncomp_bytes,
        &entry.rel_path,
        cancel_sheet_on_abort,
    )?;
    inflight.commit(jail)
}

fn register_and_tally_zip_entries(
    dest_dir: &Path,
    entries: &[ZipEntry],
    engine: &mut SafeExtractEngine,
) -> Result<(u64, u64), TTZipStatus> {
    let entry_intervals: Vec<(u64, u64)> = entries
        .iter()
        .map(|e| (e.lfh_offset, e.compressed_size))
        .collect();
    if detect_overlapping_entries(&entry_intervals) {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    let mut total_uncomp_bytes = 0u64;
    let mut total_comp_bytes = 0u64;
    for entry in entries {
        total_uncomp_bytes += entry.uncompressed_size;
        total_comp_bytes += entry.compressed_size;
        let safe_path = sanitize_and_validate_path(dest_dir, &entry.rel_path)?;
        let is_symlink = (entry.mode & 0o170000) == 0o120000;
        engine.register_entry_typed(
            safe_path,
            entry.mode & 0o7777,
            entry.mtime_epoch_secs,
            0,
            entry.is_directory,
            is_symlink,
        );
    }
    Ok((total_uncomp_bytes, total_comp_bytes))
}

fn extract_password_option(options: &TTZipExtractOptions) -> Option<&str> {
    if !options.password.is_null() {
        unsafe { std::ffi::CStr::from_ptr(options.password) }.to_str().ok()
    } else {
        None
    }
}

fn preflight_verify_encrypted_zip_entries<R: Read + Seek>(
    reader: &mut R,
    total_len: u64,
    entries: &[ZipEntry],
    password_str: Option<&str>,
) -> Result<(), TTZipStatus> {
    if !entries.iter().any(|e| e.is_encrypted) {
        return Ok(());
    }
    if password_str.filter(|p| !p.is_empty()).is_none() {
        return Err(TTZipStatus::ErrInvalidPassword);
    }
    if let Some(enc_entry) = entries.iter().find(|e| {
        e.is_encrypted
            && !e.is_directory
            && ((e.compression_method == 99 && e.compressed_size >= 28)
                || (e.compression_method != 99 && e.compressed_size >= 12))
    }) {
        let raw_payload = read_entry_raw_payload(reader, total_len, enc_entry)?;
        let _ = decrypt_entry_payload(enc_entry, raw_payload, password_str)?;
    }
    Ok(())
}

fn notify_zip_stream_progress(
    options: &TTZipExtractOptions,
    sheet: &ProgressAtomicSheet,
    total_uncomp_bytes: u64,
    rel_path: &str,
    cancel_sheet_on_abort: bool,
) -> Result<(), TTZipStatus> {
    let Some(cb) = options.progress_callback else {
        return Ok(());
    };
    let current_done = sheet.read_snapshot().processed_bytes;
    let c_path = CString::new(rel_path).unwrap_or_default();
    let keep_going = unsafe {
        cb(
            current_done,
            total_uncomp_bytes.max(current_done),
            c_path.as_ptr(),
            options.user_data,
        )
    };
    if !keep_going {
        if cancel_sheet_on_abort {
            sheet.cancel();
        }
        return Err(TTZipStatus::Cancelled);
    }
    Ok(())
}

/// Extracts all entries from a seekable ZIP stream (`VirtualMultiVolumeReader`) to `dest_dir`.
pub fn extract_zip_from_reader<R: Read + Seek>(
    reader: &mut R,
    total_len: u64,
    dest_dir: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<ZipExtractReport, TTZipStatus> {
    let start_time = std::time::Instant::now();
    fs::create_dir_all(dest_dir).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let jail = DirJail::open(dest_dir)?;

    let entries = parse_entries_from_reader(reader, total_len)?;
    let password_str = extract_password_option(options);
    preflight_verify_encrypted_zip_entries(reader, total_len, &entries, password_str)?;

    let mut engine = SafeExtractEngine::for_root(dest_dir);
    let (total_uncomp_bytes, total_comp_bytes) =
        register_and_tally_zip_entries(dest_dir, &entries, &mut engine)?;

    if options.dry_run {
        return Ok(ZipExtractReport {
            processed_entries_count: entries.len(),
            total_uncompressed_bytes: total_uncomp_bytes,
            total_compressed_bytes: total_comp_bytes,
            duration_ms: start_time.elapsed().as_millis() as u64,
        });
    }

    let local_sheet = ProgressAtomicSheet::with_total(total_uncomp_bytes);
    let active_sheet = sheet.unwrap_or(&local_sheet);
    active_sheet.set_total(total_uncomp_bytes);
    let mut quota_guard = ExtractionQuotaGuard::default_limits();

    for entry in &entries {
        if active_sheet.is_cancelled() {
            return Err(TTZipStatus::Cancelled);
        }
        quota_guard.observe_and_validate(entry.compressed_size, entry.uncompressed_size)?;
        extract_reader_entry_to_disk(
            reader,
            total_len,
            &jail,
            &entry.rel_path,
            true,
            entry,
            password_str,
            active_sheet,
            options,
            total_uncomp_bytes,
            true,
        )?;
    }

    jail.verify_all_jail_symlinks()?;
    engine.apply_deferred_metadata(options.preserve_permissions)?;
    active_sheet.mark_finished();

    Ok(ZipExtractReport {
        processed_entries_count: entries.len(),
        total_uncompressed_bytes: total_uncomp_bytes,
        total_compressed_bytes: total_comp_bytes,
        duration_ms: start_time.elapsed().as_millis() as u64,
    })
}

/// Selectively extracts matching entries from a seekable ZIP stream (`VirtualMultiVolumeReader`) to `dest_dir`.
pub fn extract_zip_selected_from_reader<R: Read + Seek>(
    reader: &mut R,
    total_len: u64,
    selection: &EntrySelection,
    dest_dir: &Path,
    options: &TTZipExtractOptions,
) -> Result<usize, TTZipStatus> {
    fs::create_dir_all(dest_dir).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let jail = DirJail::open(dest_dir)?;
    let entries = parse_entries_from_reader(reader, total_len)?;
    let password_str = extract_password_option(options);

    let matched: Vec<(&ZipEntry, &str)> = entries
        .iter()
        .filter_map(|e| selection.output_path(&e.rel_path).map(|out_rel| (e, out_rel)))
        .collect();
    let total_uncomp: u64 = matched.iter().map(|(e, _)| e.uncompressed_size).sum();
    let sheet = ProgressAtomicSheet::with_total(total_uncomp);
    let mut engine = SafeExtractEngine::for_root(dest_dir);
    let mut extracted_count = 0usize;

    for (entry, out_rel) in matched {
        let safe_path = sanitize_and_validate_path(dest_dir, out_rel)?;
        let is_symlink = (entry.mode & 0o170000) == 0o120000;
        engine.register_entry_typed(
            safe_path,
            entry.mode & 0o7777,
            entry.mtime_epoch_secs,
            0,
            entry.is_directory,
            is_symlink,
        );
        if !options.dry_run {
            extract_reader_entry_to_disk(
                reader,
                total_len,
                &jail,
                out_rel,
                true,
                entry,
                password_str,
                &sheet,
                options,
                total_uncomp,
                false,
            )?;
        } else {
            notify_zip_stream_progress(options, &sheet, total_uncomp, &entry.rel_path, false)?;
        }
        extracted_count += 1;
    }

    if !options.dry_run {
        jail.verify_all_jail_symlinks()?;
        engine.apply_deferred_metadata(options.preserve_permissions)?;
    }
    Ok(extracted_count)
}


