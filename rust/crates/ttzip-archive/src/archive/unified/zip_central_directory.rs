// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Pure Rust ZIP Central Directory inspection engine with bounded pread I/O.
//!
//! Provides zero-mmap EOCD/Zip64 location, bounded Central Directory pread,
//! and CDFH parsing with lossless path decoding and zero-allocation borrowing.

use std::fs::File;
#[cfg(unix)]
use std::os::unix::fs::FileExt;
use std::path::Path;

use crate::types::TTZipStatus;
use super::inspect::ArchiveEntryRef;

pub const MAX_CENTRAL_DIRECTORY_SIZE: u64 = 1024 * 1024 * 1024; // 1 GiB bounded limit

pub fn find_eocd_in_tail(tail_buf: &[u8]) -> Option<usize> {
    if tail_buf.len() < 22 {
        return None;
    }
    for pass in 0..2 {
        let mut pos = tail_buf.len() - 22;
        loop {
            let sig = u32::from_le_bytes([tail_buf[pos], tail_buf[pos + 1], tail_buf[pos + 2], tail_buf[pos + 3]]);
            if sig == 0x06054B50 {
                let comment_len = u16::from_le_bytes([tail_buf[pos + 20], tail_buf[pos + 21]]) as usize;
                if (pass == 0 && pos + 22 + comment_len == tail_buf.len())
                    || (pass == 1 && pos + 22 + comment_len <= tail_buf.len())
                {
                    return Some(pos);
                }
            }
            if pos == 0 {
                break;
            }
            pos -= 1;
        }
    }
    None
}

pub fn check_zip64_locator(
    file: &File,
    abs_eocd_pos: u64,
    file_len: u64,
) -> Result<Option<(u64, u64, u64)>, TTZipStatus> {
    if abs_eocd_pos < 20 {
        return Ok(None);
    }
    let mut locator_buf = [0u8; 20];
    if file.read_exact_at(&mut locator_buf, abs_eocd_pos - 20).is_err() {
        return Ok(None);
    }
    if u32::from_le_bytes([locator_buf[0], locator_buf[1], locator_buf[2], locator_buf[3]]) != 0x07064B50 {
        return Ok(None);
    }
    let total_disks = u32::from_le_bytes([locator_buf[16], locator_buf[17], locator_buf[18], locator_buf[19]]);
    if total_disks > 1 {
        return Err(TTZipStatus::ErrUnsupportedFeature);
    }
    let z64_eocd_off = u64::from_le_bytes([
        locator_buf[8], locator_buf[9], locator_buf[10], locator_buf[11],
        locator_buf[12], locator_buf[13], locator_buf[14], locator_buf[15],
    ]);
    if z64_eocd_off.saturating_add(56) > file_len {
        return Ok(None);
    }
    let mut z64 = [0u8; 56];
    if file.read_exact_at(&mut z64, z64_eocd_off).is_err()
        || u32::from_le_bytes([z64[0], z64[1], z64[2], z64[3]]) != 0x06064B50
    {
        return Ok(None);
    }
    let total_entries = u64::from_le_bytes([z64[32], z64[33], z64[34], z64[35], z64[36], z64[37], z64[38], z64[39]]);
    let cd_size = u64::from_le_bytes([z64[40], z64[41], z64[42], z64[43], z64[44], z64[45], z64[46], z64[47]]);
    let cd_offset = u64::from_le_bytes([z64[48], z64[49], z64[50], z64[51], z64[52], z64[53], z64[54], z64[55]]);
    Ok(Some((cd_offset, cd_size, total_entries)))
}

pub fn locate_zip_central_directory(
    file: &File,
    file_len: u64,
) -> Result<(u64, u64, u64), TTZipStatus> {
    let search_window = (file_len.min(65535 + 22)) as usize;
    let tail_offset = file_len - search_window as u64;

    let mut tail_buf = vec![0u8; search_window];
    file.read_exact_at(&mut tail_buf, tail_offset)
        .map_err(|_| TTZipStatus::ErrCorruptHeader)?;

    let p = find_eocd_in_tail(&tail_buf).ok_or(TTZipStatus::ErrCorruptHeader)?;
    let abs_eocd_pos = tail_offset + p as u64;

    let disk_num = u16::from_le_bytes([tail_buf[p + 4], tail_buf[p + 5]]);
    let cd_disk = u16::from_le_bytes([tail_buf[p + 6], tail_buf[p + 7]]);
    if disk_num != 0 || cd_disk != 0 {
        return Err(TTZipStatus::ErrUnsupportedFeature);
    }

    let entries16 = u16::from_le_bytes([tail_buf[p + 10], tail_buf[p + 11]]) as u64;
    let cd_size32 = u32::from_le_bytes([
        tail_buf[p + 12], tail_buf[p + 13], tail_buf[p + 14], tail_buf[p + 15],
    ]) as u64;
    let cd_off32 = u32::from_le_bytes([
        tail_buf[p + 16], tail_buf[p + 17], tail_buf[p + 18], tail_buf[p + 19],
    ]) as u64;

    if let Some((cd_off, cd_sz, total)) = check_zip64_locator(file, abs_eocd_pos, file_len)? {
        return Ok((cd_off, cd_sz, total));
    }

    Ok((cd_off32, cd_size32, entries16))
}

#[inline]
pub fn dos_to_local_unix_time(dos_time: u16, dos_date: u16) -> i64 {
    crate::zip::datetime::dos_to_local_unix_epoch_secs(dos_time, dos_date)
}

pub fn resolve_zip_path<'a>(
    fn_bytes: &'a [u8],
    flag: u16,
    extra: &crate::zip::extra::ZipExtraFields,
    detect_encoding: bool,
) -> (std::borrow::Cow<'a, str>, Option<String>) {
    if let Some(upath) = &extra.unicode_path {
        if upath.is_valid_for(fn_bytes) {
            let s = if upath.text.contains('\\') {
                upath.text.replace('\\', "/")
            } else {
                upath.text.clone()
            };
            return (std::borrow::Cow::Owned(s), None);
        }
    }

    let is_utf8 = (flag & 0x0800) != 0;
    if is_utf8 {
        if let Ok(utf8_str) = std::str::from_utf8(fn_bytes) {
            if !utf8_str.contains('\\') {
                return (std::borrow::Cow::Borrowed(utf8_str), None);
            }
            return (std::borrow::Cow::Owned(utf8_str.replace('\\', "/")), None);
        }
    }

    let detected_charset = if detect_encoding && !fn_bytes.is_ascii() {
        crate::codecs::chardet::detect_charset(fn_bytes)
    } else {
        None
    };

    if let Ok(utf8_str) = std::str::from_utf8(fn_bytes) {
        if !utf8_str.contains('\\') {
            return (std::borrow::Cow::Borrowed(utf8_str), detected_charset);
        }
        return (std::borrow::Cow::Owned(utf8_str.replace('\\', "/")), detected_charset);
    }

    let transcoded = if let Some(ref charset) = detected_charset {
        crate::charset::transcode_to_utf8(fn_bytes, charset)
            .unwrap_or_else(|_| crate::charset::sanitize_filename(fn_bytes))
    } else {
        crate::charset::sanitize_filename(fn_bytes)
    };

    let normalized = if transcoded.contains('\\') {
        transcoded.replace('\\', "/")
    } else {
        transcoded
    };
    (std::borrow::Cow::Owned(normalized), detected_charset)
}

#[inline]
pub fn resolve_dos_mtime(dos_time: u16, dos_date: u16, cache: &mut (u32, i64)) -> i64 {
    if dos_date == 0 {
        return 0;
    }
    let key = ((dos_date as u32) << 16) | (dos_time as u32);
    if key == cache.0 && key != 0 {
        return cache.1;
    }
    let epoch = dos_to_local_unix_time(dos_time, dos_date);
    *cache = (key, epoch);
    epoch
}

struct CdfhHeader {
    flag: u16,
    method: u16,
    dos_time: u16,
    dos_date: u16,
    crc32: u32,
    comp_size32: u32,
    uncomp_size32: u32,
    fn_len: usize,
    extra_len: usize,
    ext_attr: u32,
    lfh_off32: u32,
    next_pos: usize,
}

#[inline]
fn parse_cdfh_header(cd_buf: &[u8], pos: usize) -> Result<CdfhHeader, TTZipStatus> {
    let cd_end = cd_buf.len();
    if pos + 46 > cd_end {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let sig = u32::from_le_bytes([cd_buf[pos], cd_buf[pos + 1], cd_buf[pos + 2], cd_buf[pos + 3]]);
    if sig != crate::zip::parser::MAGIC_CDFH {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let flag = u16::from_le_bytes([cd_buf[pos + 8], cd_buf[pos + 9]]);
    let method = u16::from_le_bytes([cd_buf[pos + 10], cd_buf[pos + 11]]);
    let dos_time = u16::from_le_bytes([cd_buf[pos + 12], cd_buf[pos + 13]]);
    let dos_date = u16::from_le_bytes([cd_buf[pos + 14], cd_buf[pos + 15]]);
    let crc32 = u32::from_le_bytes([cd_buf[pos + 16], cd_buf[pos + 17], cd_buf[pos + 18], cd_buf[pos + 19]]);
    let comp_size32 = u32::from_le_bytes([cd_buf[pos + 20], cd_buf[pos + 21], cd_buf[pos + 22], cd_buf[pos + 23]]);
    let uncomp_size32 = u32::from_le_bytes([cd_buf[pos + 24], cd_buf[pos + 25], cd_buf[pos + 26], cd_buf[pos + 27]]);
    let fn_len = u16::from_le_bytes([cd_buf[pos + 28], cd_buf[pos + 29]]) as usize;
    let extra_len = u16::from_le_bytes([cd_buf[pos + 30], cd_buf[pos + 31]]) as usize;
    let comment_len = u16::from_le_bytes([cd_buf[pos + 32], cd_buf[pos + 33]]) as usize;
    let ext_attr = u32::from_le_bytes([cd_buf[pos + 38], cd_buf[pos + 39], cd_buf[pos + 40], cd_buf[pos + 41]]);
    let lfh_off32 = u32::from_le_bytes([cd_buf[pos + 42], cd_buf[pos + 43], cd_buf[pos + 44], cd_buf[pos + 45]]);

    let rec_len = 46usize
        .checked_add(fn_len)
        .and_then(|v| v.checked_add(extra_len))
        .and_then(|v| v.checked_add(comment_len))
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    let next_pos = pos.checked_add(rec_len).ok_or(TTZipStatus::ErrCorruptHeader)?;
    if next_pos > cd_end {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    Ok(CdfhHeader {
        flag, method, dos_time, dos_date, crc32,
        comp_size32, uncomp_size32, fn_len, extra_len,
        ext_attr, lfh_off32, next_pos,
    })
}

#[inline]
fn resolve_entry_mode(ext_attr: u32, is_directory_name: bool) -> (bool, u32) {
    let mut is_directory = is_directory_name;
    if !is_directory && ext_attr != 0 {
        let posix_mode = ext_attr >> 16;
        if (posix_mode & 0o170000) == 0o040000 || (ext_attr & 0x10) != 0 {
            is_directory = true;
        }
    }

    let full_posix_mode = ext_attr >> 16;
    let mode = if full_posix_mode != 0 {
        if (full_posix_mode & 0o170000) == 0 {
            if is_directory {
                0o040000 | full_posix_mode
            } else {
                0o100000 | full_posix_mode
            }
        } else {
            full_posix_mode
        }
    } else if is_directory {
        0o040755
    } else {
        0o100644
    };

    (is_directory, mode)
}

pub fn process_single_cdfh_record<F>(
    cd_buf: &[u8],
    pos: usize,
    detect_encoding: bool,
    dos_cache: &mut (u32, i64),
    callback: &mut F,
) -> Result<(bool, usize), TTZipStatus>
where
    F: FnMut(&ArchiveEntryRef<'_>) -> bool,
{
    let hdr = parse_cdfh_header(cd_buf, pos)?;
    let fn_start = pos + 46;
    let fn_bytes = &cd_buf[fn_start..fn_start + hdr.fn_len];
    if fn_bytes.is_empty() {
        return Ok((true, hdr.next_pos));
    }

    let extra_start = fn_start + hdr.fn_len;
    let extra_bytes = &cd_buf[extra_start..extra_start + hdr.extra_len];
    let extra = crate::zip::extra::ZipExtraFields::parse(
        extra_bytes,
        true,
        hdr.uncomp_size32 == 0xFFFFFFFF,
        hdr.comp_size32 == 0xFFFFFFFF,
        hdr.lfh_off32 == 0xFFFFFFFF,
    );

    let uncompressed_size = extra.uncompressed_size.unwrap_or(hdr.uncomp_size32 as u64);
    let compressed_size = extra.compressed_size.unwrap_or(hdr.comp_size32 as u64);
    let mut actual_method = hdr.method;
    if crate::zip::parser::is_valid_winzip_aes_extra(&extra) {
        actual_method = extra.aes_actual_method;
    }
    let is_encrypted = (hdr.flag & 0x0001) != 0;

    let (is_directory, mode) = resolve_entry_mode(hdr.ext_attr, fn_bytes.ends_with(b"/"));
    let mtime_epoch = if let Some(m) = extra.mod_time {
        m as i64
    } else if let Some(ntfs) = extra.ntfs.as_ref().filter(|n| n.mtime_ticks > 0) {
        ntfs.mtime_unix_secs()
    } else {
        resolve_dos_mtime(hdr.dos_time, hdr.dos_date, dos_cache)
    };

    let (path_cow, detected_encoding_opt) =
        resolve_zip_path(fn_bytes, hdr.flag, &extra, detect_encoding);

    let entry_ref = ArchiveEntryRef {
        path: path_cow.as_ref(),
        raw_path: fn_bytes,
        uncompressed_size,
        compressed_size,
        crc32: hdr.crc32,
        mtime_epoch_secs: mtime_epoch,
        mode,
        is_directory,
        is_encrypted,
        compression_method: actual_method,
        detected_encoding: detected_encoding_opt.as_deref(),
    };

    let should_continue = callback(&entry_ref);
    Ok((should_continue, hdr.next_pos))
}

pub fn parse_central_directory_buffer<F>(
    cd_buf: &[u8],
    detect_encoding: bool,
    callback: &mut F,
) -> Result<usize, TTZipStatus>
where
    F: FnMut(&ArchiveEntryRef<'_>) -> bool,
{
    let cd_end = cd_buf.len();
    let mut pos = 0;
    let mut count = 0;
    let mut dos_cache: (u32, i64) = (0, 0);

    while pos < cd_end {
        let (should_continue, next_pos) =
            process_single_cdfh_record(cd_buf, pos, detect_encoding, &mut dos_cache, callback)?;
        count += 1;
        if !should_continue {
            break;
        }
        pos = next_pos;
    }

    Ok(count)
}

pub fn inspect_zip_central_directory_file<F>(
    archive_path: &Path,
    detect_encoding: bool,
    callback: &mut F,
) -> Result<usize, TTZipStatus>
where
    F: FnMut(&ArchiveEntryRef<'_>) -> bool,
{
    let file = File::open(archive_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let file_len = file.metadata().map_err(|_| TTZipStatus::ErrOpenFailed)?.len();
    if file_len < 22 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let (cd_offset, cd_size, total_entries) = locate_zip_central_directory(&file, file_len)?;

    if cd_size > file_len || cd_size > MAX_CENTRAL_DIRECTORY_SIZE {
        return Err(TTZipStatus::ErrUnsupportedFeature);
    }
    let cd_end = cd_offset.checked_add(cd_size).ok_or(TTZipStatus::ErrCorruptHeader)?;
    if cd_end > file_len {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let mut cd_buf = vec![0u8; cd_size as usize];
    file.read_exact_at(&mut cd_buf, cd_offset)
        .map_err(|_| TTZipStatus::ErrCorruptHeader)?;

    if total_entries > 0 && (cd_buf.len() < 4 || &cd_buf[0..4] != b"PK\x01\x02") {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    parse_central_directory_buffer(&cd_buf, detect_encoding, callback)
}

fn check_zip64_locator_reader<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    abs_eocd_pos: u64,
    total_len: u64,
) -> Result<Option<(u64, u64, u64)>, TTZipStatus> {
    if abs_eocd_pos < 20 {
        return Ok(None);
    }
    let mut locator_buf = [0u8; 20];
    if reader.seek(std::io::SeekFrom::Start(abs_eocd_pos - 20)).is_err()
        || reader.read_exact(&mut locator_buf).is_err()
    {
        return Ok(None);
    }
    if u32::from_le_bytes([locator_buf[0], locator_buf[1], locator_buf[2], locator_buf[3]]) != 0x07064B50 {
        return Ok(None);
    }
    let z64_eocd_off = u64::from_le_bytes([
        locator_buf[8], locator_buf[9], locator_buf[10], locator_buf[11],
        locator_buf[12], locator_buf[13], locator_buf[14], locator_buf[15],
    ]);
    if z64_eocd_off.saturating_add(56) > total_len {
        return Ok(None);
    }
    let mut z64 = [0u8; 56];
    if reader.seek(std::io::SeekFrom::Start(z64_eocd_off)).is_err()
        || reader.read_exact(&mut z64).is_err()
        || u32::from_le_bytes([z64[0], z64[1], z64[2], z64[3]]) != 0x06064B50
    {
        return Ok(None);
    }
    let total_entries = u64::from_le_bytes([z64[32], z64[33], z64[34], z64[35], z64[36], z64[37], z64[38], z64[39]]);
    let cd_size = u64::from_le_bytes([z64[40], z64[41], z64[42], z64[43], z64[44], z64[45], z64[46], z64[47]]);
    let cd_offset = u64::from_le_bytes([z64[48], z64[49], z64[50], z64[51], z64[52], z64[53], z64[54], z64[55]]);
    Ok(Some((cd_offset, cd_size, total_entries)))
}

/// Locates the ZIP Central Directory `(cd_offset, cd_size, total_entries)` from any seekable reader.
pub fn locate_zip_central_directory_reader<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    total_len: u64,
) -> Result<(u64, u64, u64), TTZipStatus> {
    if total_len < 22 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let search_window = (total_len.min(65535 + 22)) as usize;
    let tail_offset = total_len - search_window as u64;

    let mut tail_buf = vec![0u8; search_window];
    reader
        .seek(std::io::SeekFrom::Start(tail_offset))
        .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    reader
        .read_exact(&mut tail_buf)
        .map_err(|_| TTZipStatus::ErrCorruptHeader)?;

    let p = find_eocd_in_tail(&tail_buf).ok_or(TTZipStatus::ErrCorruptHeader)?;
    let abs_eocd_pos = tail_offset + p as u64;

    let entries16 = u16::from_le_bytes([tail_buf[p + 10], tail_buf[p + 11]]) as u64;
    let cd_size32 = u32::from_le_bytes([
        tail_buf[p + 12], tail_buf[p + 13], tail_buf[p + 14], tail_buf[p + 15],
    ]) as u64;
    let cd_off32 = u32::from_le_bytes([
        tail_buf[p + 16], tail_buf[p + 17], tail_buf[p + 18], tail_buf[p + 19],
    ]) as u64;

    if let Some((cd_off, cd_sz, total)) = check_zip64_locator_reader(reader, abs_eocd_pos, total_len)? {
        return Ok((cd_off, cd_sz, total));
    }

    Ok((cd_off32, cd_size32, entries16))
}

/// Inspects a ZIP archive from any seekable stream (`VirtualMultiVolumeReader`) via its Central Directory.
pub fn inspect_zip_central_directory_reader<R: std::io::Read + std::io::Seek, F>(
    reader: &mut R,
    total_len: u64,
    detect_encoding: bool,
    callback: &mut F,
) -> Result<usize, TTZipStatus>
where
    F: FnMut(&ArchiveEntryRef<'_>) -> bool,
{
    let (cd_offset, cd_size, total_entries) = locate_zip_central_directory_reader(reader, total_len)?;
    if cd_size > total_len || cd_size > MAX_CENTRAL_DIRECTORY_SIZE {
        return Err(TTZipStatus::ErrUnsupportedFeature);
    }
    let cd_end = cd_offset.checked_add(cd_size).ok_or(TTZipStatus::ErrCorruptHeader)?;
    if cd_end > total_len {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let mut cd_buf = vec![0u8; cd_size as usize];
    reader
        .seek(std::io::SeekFrom::Start(cd_offset))
        .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    reader
        .read_exact(&mut cd_buf)
        .map_err(|_| TTZipStatus::ErrCorruptHeader)?;

    if total_entries > 0 && (cd_buf.len() < 4 || &cd_buf[0..4] != b"PK\x01\x02") {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    parse_central_directory_buffer(&cd_buf, detect_encoding, callback)
}

