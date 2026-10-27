// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! ZIP Central Directory and Local Header Zero-Copy Parser.
//!
//! Handles standard ZIP, Zip64 large archives (>4GB / >65535 entries), and WinZip AES headers.

use crate::types::TTZipStatus;
use crate::zip::extra::ZipExtraFields;

pub const MAGIC_LFH: u32 = 0x04034B50;
pub const MAGIC_CDFH: u32 = 0x02014B50;
pub const MAGIC_EOCD: u32 = 0x06054B50;
pub const MAGIC_ZIP64_EOCD: u32 = 0x06064B50;
pub const MAGIC_ZIP64_LOCATOR: u32 = 0x07064B50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EocdInfo {
    pub total_entries: u64,
    pub cd_size: u64,
    pub cd_offset: u64,
    pub is_zip64: bool,
    pub eocd_offset: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipEntry {
    pub rel_path: String,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub lfh_offset: u64,
    pub mtime_epoch_secs: i64,
    pub crc32: u32,
    pub mode: u32,
    pub compression_method: u16,
    pub actual_method: u16,
    pub flag: u16,
    pub aes_version: u16,
    pub aes_strength: u8,
    pub is_directory: bool,
    pub is_encrypted: bool,
}

fn read_u16_le(slice: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([slice[offset], slice[offset + 1]])
}

#[inline(always)]
fn read_u32_le(slice: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        slice[offset],
        slice[offset + 1],
        slice[offset + 2],
        slice[offset + 3],
    ])
}

#[inline(always)]
fn read_u64_le(slice: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        slice[offset],
        slice[offset + 1],
        slice[offset + 2],
        slice[offset + 3],
        slice[offset + 4],
        slice[offset + 5],
        slice[offset + 6],
        slice[offset + 7],
    ])
}

/// Finds End of Central Directory (EOCD) and optional Zip64 EOCD structures.
pub fn find_eocd(mapped: &[u8]) -> Result<EocdInfo, TTZipStatus> {
    let file_size = mapped.len();
    if file_size < 22 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let search_back = file_size.min(65557);
    let search_start = file_size - search_back;

    let mut eocd_pos = None;

    // First pass: search backwards for exact EOCD where pos + 22 + comment_len == file_size.
    // This disambiguates real EOCD from fake EOCD signatures embedded in comments.
    let mut pos = file_size - 22;
    loop {
        if pos < search_start {
            break;
        }
        let sig = read_u32_le(mapped, pos);
        if sig == MAGIC_EOCD {
            let comment_len = read_u16_le(mapped, pos + 20) as usize;
            if pos + 22 + comment_len == file_size {
                eocd_pos = Some(pos);
                break;
            }
        }
        if pos == 0 {
            break;
        }
        pos -= 1;
    }

    // Fallback pass: if trailing bytes exist, accept pos + 22 + comment_len <= file_size
    if eocd_pos.is_none() {
        let mut pos = file_size - 22;
        loop {
            if pos < search_start {
                break;
            }
            let sig = read_u32_le(mapped, pos);
            if sig == MAGIC_EOCD {
                let comment_len = read_u16_le(mapped, pos + 20) as usize;
                if pos + 22 + comment_len <= file_size {
                    eocd_pos = Some(pos);
                    break;
                }
            }
            if pos == 0 {
                break;
            }
            pos -= 1;
        }
    }

    let p = eocd_pos.ok_or(TTZipStatus::ErrCorruptHeader)?;

    let entries16 = read_u16_le(mapped, p + 10) as u64;
    let cd_size32 = read_u32_le(mapped, p + 12) as u64;
    let cd_off32 = read_u32_le(mapped, p + 16) as u64;

    let mut info = EocdInfo {
        total_entries: entries16,
        cd_size: cd_size32,
        cd_offset: cd_off32,
        is_zip64: false,
        eocd_offset: p as u64,
    };

    // Check for Zip64 EOCD Locator (20 bytes before standard EOCD)
    if p >= 20 {
        let locator_pos = p - 20;
        let locator_sig = read_u32_le(mapped, locator_pos);
        if locator_sig == MAGIC_ZIP64_LOCATOR {
            let z64_eocd_off = read_u64_le(mapped, locator_pos + 8) as usize;
            if z64_eocd_off.saturating_add(56) <= file_size {
                let z64_sig = read_u32_le(mapped, z64_eocd_off);
                if z64_sig == MAGIC_ZIP64_EOCD {
                    info.is_zip64 = true;
                    info.total_entries = read_u64_le(mapped, z64_eocd_off + 32);
                    info.cd_size = read_u64_le(mapped, z64_eocd_off + 40);
                    info.cd_offset = read_u64_le(mapped, z64_eocd_off + 48);
                }
            }
        }
    }

    Ok(info)
}

/// Converts DOS timestamp (time: u16, date: u16) to Unix epoch timestamp in seconds.
pub fn dos_to_unix_time(dos_time: u16, dos_date: u16) -> i64 {
    crate::zip::datetime::DosDateTime::from_dos(dos_date, dos_time)
        .map(|dt| dt.to_unix_epoch_secs())
        .unwrap_or(0)
}

/// Parses a single Central Directory File Header (CDFH) entry.
pub fn parse_cdfh_entry(mapped: &[u8], curr_pos: usize) -> Result<(ZipEntry, usize), TTZipStatus> {
    let curr_pos_end = curr_pos.checked_add(46).ok_or(TTZipStatus::ErrCorruptHeader)?;
    let hdr = mapped.get(curr_pos..curr_pos_end).ok_or(TTZipStatus::ErrCorruptHeader)?;
    if read_u32_le(hdr, 0) != MAGIC_CDFH {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let flag = read_u16_le(hdr, 8);
    let method = read_u16_le(hdr, 10);
    let dos_time = read_u16_le(hdr, 12);
    let dos_date = read_u16_le(hdr, 14);
    let crc32 = read_u32_le(hdr, 16);
    let comp_size32 = read_u32_le(hdr, 20);
    let uncomp_size32 = read_u32_le(hdr, 24);
    let fn_len = read_u16_le(hdr, 28) as usize;
    let extra_len = read_u16_le(hdr, 30) as usize;
    let comment_len = read_u16_le(hdr, 32) as usize;
    let ext_attr = read_u32_le(hdr, 38);
    let lfh_offset32 = read_u32_le(hdr, 42);

    let rec_len = 46usize
        .checked_add(fn_len)
        .and_then(|v| v.checked_add(extra_len))
        .and_then(|v| v.checked_add(comment_len))
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    let next_pos = curr_pos.checked_add(rec_len).ok_or(TTZipStatus::ErrCorruptHeader)?;
    if next_pos > mapped.len() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let fn_start = curr_pos + 46;
    let fn_bytes = &mapped[fn_start..fn_start + fn_len];
    let extra_start = fn_start + fn_len;
    let extra_bytes = &mapped[extra_start..extra_start + extra_len];

    let extra = ZipExtraFields::parse(
        extra_bytes,
        true,
        uncomp_size32 == 0xFFFFFFFF,
        comp_size32 == 0xFFFFFFFF,
        lfh_offset32 == 0xFFFFFFFF,
    );
    let filename = resolve_cdfh_filename(fn_bytes, flag, &extra);

    let uncompressed_size = extra.uncompressed_size.unwrap_or(uncomp_size32 as u64);
    let compressed_size = extra.compressed_size.unwrap_or(comp_size32 as u64);
    let lfh_offset = extra.local_header_offset.unwrap_or(lfh_offset32 as u64);
    let (actual_method, aes_version, aes_strength) = if is_valid_winzip_aes_extra(&extra) {
        (extra.aes_actual_method, extra.aes_version, extra.aes_strength)
    } else {
        (method, 0, 0)
    };

    let is_encrypted = (flag & 0x0001) != 0;
    let mtime_epoch_secs = crate::zip::datetime::resolve_zip_mtime_secs(&extra, dos_time, dos_date);
    let (is_directory, mode) = resolve_entry_mode_and_dir(&filename, ext_attr);
    Ok((
        ZipEntry {
            rel_path: filename,
            uncompressed_size,
            compressed_size,
            lfh_offset,
            mtime_epoch_secs,
            crc32,
            mode,
            compression_method: method,
            actual_method,
            flag,
            aes_version,
            aes_strength,
            is_directory,
            is_encrypted,
        },
        next_pos,
    ))
}

/// Returns `true` iff `extra` contains a structurally valid `0x9901` WinZip AES extra field.
#[inline]
pub fn is_valid_winzip_aes_extra(extra: &ZipExtraFields) -> bool {
    extra.has_winzip_aes
        && extra.aes_vendor_id == crate::zip::extra::WINZIP_AES_VENDOR_ID
        && matches!(
            extra.aes_version,
            crate::zip::extra::WINZIP_AES_VERSION_AE1
                | crate::zip::extra::WINZIP_AES_VERSION_AE2
        )
        && (1..=3).contains(&extra.aes_strength)
}

fn resolve_cdfh_filename(fn_bytes: &[u8], flag: u16, extra: &ZipExtraFields) -> String {
    let is_utf8 = (flag & 0x0800) != 0;
    let mut filename = crate::zip::cp437::decode_zip_filename(fn_bytes, is_utf8);
    if filename.contains('\\') {
        filename = filename.replace('\\', "/");
    }
    if let Some(upath) = &extra.unicode_path {
        if upath.is_valid_for(fn_bytes) {
            if upath.text.contains('\\') {
                filename = upath.text.replace('\\', "/");
            } else {
                filename = upath.text.clone();
            }
        }
    }
    filename
}

fn resolve_entry_mode_and_dir(filename: &str, ext_attr: u32) -> (bool, u32) {
    let mut is_directory = filename.ends_with('/');
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

/// Parses all Central Directory entries.
pub fn parse_all_entries(mapped: &[u8]) -> Result<Vec<ZipEntry>, TTZipStatus> {
    let eocd = find_eocd(mapped)?;
    let mut entries = Vec::with_capacity(eocd.total_entries.min(65536) as usize);

    let cd_end_u64 = eocd.cd_offset.checked_add(eocd.cd_size).ok_or(TTZipStatus::ErrCorruptHeader)?;
    let mut pos = usize::try_from(eocd.cd_offset).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    let cd_end = usize::try_from(cd_end_u64).map_err(|_| TTZipStatus::ErrCorruptHeader)?;

    if cd_end > mapped.len() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    while pos < cd_end {
        let (entry, next_pos) = parse_cdfh_entry(mapped, pos)?;
        entries.push(entry);
        pos = next_pos;
    }

    Ok(entries)
}

/// Parses Local File Header at `lfh_offset` and returns payload byte range `(payload_offset, header_size)`.
pub fn parse_local_file_header(mapped: &[u8], lfh_offset: usize) -> Result<(usize, usize), TTZipStatus> {
    let lfh_min_end = lfh_offset.checked_add(30).ok_or(TTZipStatus::ErrCorruptHeader)?;
    if lfh_min_end > mapped.len() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let sig = read_u32_le(mapped, lfh_offset);
    if sig != MAGIC_LFH {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let fn_len = read_u16_le(mapped, lfh_offset + 26) as usize;
    let extra_len = read_u16_le(mapped, lfh_offset + 28) as usize;

    let header_size = 30usize
        .checked_add(fn_len)
        .and_then(|v| v.checked_add(extra_len))
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    let payload_offset = lfh_offset.checked_add(header_size).ok_or(TTZipStatus::ErrCorruptHeader)?;

    if payload_offset > mapped.len() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    Ok((payload_offset, header_size))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dos_to_unix_time() {
        // 2026-01-01 00:00:00 UTC
        let dos_date = ((2026 - 1980) << 9) | (1 << 5) | 1;
        let dos_time = 0;
        let unix = dos_to_unix_time(dos_time, dos_date as u16);
        assert_eq!(unix, 1767225600);
    }
}
