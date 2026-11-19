// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use std::ops::Range;

use super::{HeaderAdmission, next_varint, require_section_end};
use crate::sevenz::format::{
    K_ATIME, K_CTIME, K_DUMMY, K_EMPTY_FILE, K_EMPTY_STREAM, K_END, K_MTIME, K_NAME,
    K_WIN_ATTRIBUTES,
};
use crate::sevenz::header::{SevenZFileMeta, SevenZHeaderInfo};
use crate::types::TTZipStatus;

fn read_defined_bitmask(
    hp: &[u8],
    hpos: &mut usize,
    prop_end: usize,
    num_files: usize,
) -> Option<Vec<bool>> {
    if *hpos >= prop_end {
        return None;
    }
    let all_defined = hp[*hpos];
    *hpos += 1;
    let mut defined = vec![true; num_files];
    if all_defined == 0 {
        let mask_bytes = num_files.div_ceil(8);
        let mask_end = hpos.checked_add(mask_bytes).filter(|&end| end <= prop_end)?;
        for (i, slot) in defined.iter_mut().enumerate().take(num_files) {
            let b = hp[*hpos + (i / 8)];
            *slot = ((b >> (7 - (i % 8))) & 1) != 0;
        }
        *hpos = mask_end;
    }
    if *hpos >= prop_end {
        return None;
    }
    let external = hp[*hpos];
    *hpos += 1;
    (external == 0).then_some(defined)
}

fn parse_files_names(
    hp: &[u8],
    hpos: usize,
    prop_end: usize,
    files: &mut [SevenZFileMeta],
) -> Result<(), TTZipStatus> {
    if hpos >= prop_end {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let mut name_pos = hpos + 1;
    let mut name_bytes_left = (prop_end - hpos).saturating_sub(1);
    let mut u16_chars = Vec::new();

    for file in files.iter_mut() {
        u16_chars.clear();
        while name_bytes_left >= 2 && name_pos + 2 <= prop_end {
            let mut ch = u16::from_le_bytes([hp[name_pos], hp[name_pos + 1]]);
            name_pos += 2;
            name_bytes_left -= 2;
            if ch == 0 {
                break;
            }
            if ch == 0x005C {
                ch = 0x002F;
            }
            u16_chars.push(ch);
        }
        file.rel_path = String::from_utf16_lossy(&u16_chars);
    }
    Ok(())
}

fn parse_files_mtime(hp: &[u8], mut hpos: usize, prop_end: usize, files: &mut [SevenZFileMeta]) {
    let Some(defined) = read_defined_bitmask(hp, &mut hpos, prop_end, files.len()) else {
        return;
    };
    for (file, is_def) in files.iter_mut().zip(defined) {
        if !is_def || hpos + 8 > prop_end {
            continue;
        }
        let mut t_bytes = [0u8; 8];
        t_bytes.copy_from_slice(&hp[hpos..hpos + 8]);
        hpos += 8;
        let nt = crate::sevenz::time::NtTime(u64::from_le_bytes(t_bytes));
        let sys_time = nt.to_system_time();
        let secs = match sys_time.duration_since(std::time::UNIX_EPOCH) {
            Ok(dur) => dur.as_secs() as i64,
            Err(err) => -(err.duration().as_secs() as i64),
        };
        file.mtime_epoch_secs = Some(secs);
    }
}

fn apply_win_attribute_to_file(attr: u32, file: &mut SevenZFileMeta) {
    let posix_mode = (attr >> 16) & 0xFFFF;
    let file_type = posix_mode & 0o170000;
    if file_type == 0o120000 || (file_type == 0 && (attr & 0x400) != 0) {
        file.is_directory = false;
        file.mode = if posix_mode != 0 {
            posix_mode
        } else {
            0o120777
        };
    } else {
        if (attr & 0x10) != 0 || file_type == 0o040000 {
            file.is_directory = true;
        }
        file.mode = if posix_mode != 0 {
            posix_mode
        } else if file.is_directory {
            0o755
        } else {
            0o644
        };
    }
}

fn parse_files_win_attributes(
    hp: &[u8],
    mut hpos: usize,
    prop_end: usize,
    files: &mut [SevenZFileMeta],
) {
    let Some(defined) = read_defined_bitmask(hp, &mut hpos, prop_end, files.len()) else {
        return;
    };
    for (file, is_def) in files.iter_mut().zip(defined) {
        if !is_def || hpos + 4 > prop_end {
            continue;
        }
        let attr = u32::from_le_bytes([hp[hpos], hp[hpos + 1], hp[hpos + 2], hp[hpos + 3]]);
        hpos += 4;
        apply_win_attribute_to_file(attr, file);
    }
}

fn apply_empty_streams_and_files(
    ftag: u8,
    hp: &[u8],
    hpos: usize,
    prop_size: usize,
    files: &mut [SevenZFileMeta],
) {
    if ftag == K_EMPTY_STREAM {
        for (i, file) in files.iter_mut().enumerate() {
            let byte_idx = i / 8;
            if byte_idx < prop_size && ((hp[hpos + byte_idx] >> (7 - (i % 8))) & 1) != 0 {
                file.is_empty_stream = true;
            }
        }
    } else if ftag == K_EMPTY_FILE {
        let mut empty_idx = 0usize;
        for file in files.iter_mut() {
            if file.is_empty_stream {
                let byte_idx = empty_idx / 8;
                if byte_idx < prop_size {
                    let is_empty_file = ((hp[hpos + byte_idx] >> (7 - (empty_idx % 8))) & 1) != 0;
                    file.is_directory = !is_empty_file;
                }
                empty_idx += 1;
            }
        }
    }
}

pub(super) fn parse_files_info_section(
    hp: &[u8],
    hpos: &mut usize,
    out_info: &mut SevenZHeaderInfo,
    admission: HeaderAdmission,
) -> Result<(), TTZipStatus> {
    let hlen = hp.len();
    let num_files_val = next_varint(hp, hpos)?;
    admission.count(num_files_val, 1024)?;
    let num_files =
        usize::try_from(num_files_val).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    if num_files_val > 1_000_000 || num_files > hlen.saturating_mul(1024) {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    out_info
        .files
        .resize(num_files, SevenZFileMeta::default());

    let mut tags = 0;
    let mut saw_end = false;
    while *hpos < hlen {
        let ftag = admission.next_tag(hp, hpos, &mut tags)?;
        if ftag == K_END {
            saw_end = true;
            break;
        }
        let prop_size =
            usize::try_from(next_varint(hp, hpos)?).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
        let prop_end = hpos
            .checked_add(prop_size)
            .filter(|&end| end <= hlen)
            .ok_or(TTZipStatus::ErrCorruptHeader)?;

        apply_file_property(ftag, hp, *hpos..prop_end, &mut out_info.files, admission)?;
        *hpos = prop_end;
    }
    require_section_end(admission, saw_end)
}

fn apply_file_property(
    tag: u8,
    bytes: &[u8],
    property: Range<usize>,
    files: &mut [SevenZFileMeta],
    admission: HeaderAdmission,
) -> Result<(), TTZipStatus> {
    let start = property.start;
    let end = property.end;
    match tag {
        K_EMPTY_STREAM | K_EMPTY_FILE => {
            apply_empty_streams_and_files(tag, bytes, start, property.len(), files);
        }
        K_NAME => {
            match bytes[start..end].first().copied() {
                None => return Err(TTZipStatus::ErrCorruptHeader),
                Some(0) => {}
                Some(_) => admission.unsupported()?,
            }
            parse_files_names(bytes, start, end, files)?;
        }
        K_MTIME => parse_files_mtime(bytes, start, end, files),
        K_WIN_ATTRIBUTES => parse_files_win_attributes(bytes, start, end, files),
        K_DUMMY | K_CTIME | K_ATIME => {}
        _ => admission.unsupported()?,
    }
    Ok(())
}
