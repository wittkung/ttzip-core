// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! 7-Zip Header and EncodedHeader Zero-Copy Binary Metadata Stream Parser.

use super::models::{SevenZCoder, SevenZFileMeta, SevenZFolder, SevenZHeaderInfo};
use crate::sevenz::format::*;
use crate::types::TTZipStatus;

/// Upper bound for the in/out stream count of a single coder.
const MAX_CODER_STREAMS: u64 = 64;

fn next_varint(hp: &[u8], hpos: &mut usize) -> Result<u64, TTZipStatus> {
    let (val, rd) = read_varint(&hp[*hpos..]).ok_or(TTZipStatus::ErrCorruptHeader)?;
    *hpos += rd;
    Ok(val)
}

fn skip_unknown_property(hp: &[u8], hpos: &mut usize) -> Result<(), TTZipStatus> {
    let sz = next_varint(hp, hpos)?;
    *hpos = hpos.checked_add(sz as usize).ok_or(TTZipStatus::ErrCorruptHeader)?;
    if *hpos > hp.len() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    Ok(())
}

fn skip_pack_crcs(hp: &[u8], hpos: &mut usize, num_pack_streams: u64) -> Result<(), TTZipStatus> {
    let hlen = hp.len();
    if *hpos >= hlen {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let all_defined = hp[*hpos];
    *hpos += 1;
    if all_defined != 0 {
        let crc_bytes = (num_pack_streams as usize).saturating_mul(4);
        if hpos.saturating_add(crc_bytes) > hlen {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        *hpos += crc_bytes;
    }
    Ok(())
}

fn read_pack_sizes(
    hp: &[u8],
    hpos: &mut usize,
    num_pack_streams: u64,
    pack_sizes: &mut Vec<u64>,
    total_pack_size: &mut u64,
) -> Result<(), TTZipStatus> {
    for _ in 0..num_pack_streams {
        let sz = next_varint(hp, hpos)?;
        pack_sizes.push(sz);
        *total_pack_size = total_pack_size.checked_add(sz).ok_or(TTZipStatus::ErrCorruptHeader)?;
    }
    Ok(())
}

fn parse_pack_info_section(
    hp: &[u8],
    hpos: &mut usize,
    out_info: &mut SevenZHeaderInfo,
) -> Result<(), TTZipStatus> {
    let hlen = hp.len();
    let pack_pos = next_varint(hp, hpos)?;
    let num_pack_streams = next_varint(hp, hpos)?;
    if (num_pack_streams as usize) > hlen {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let pack_pos_usize = usize::try_from(pack_pos).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    out_info.payload_offset = 32usize.checked_add(pack_pos_usize).ok_or(TTZipStatus::ErrCorruptHeader)?;
    let mut total_pack_size = 0u64;

    while *hpos < hlen {
        let ptag = hp[*hpos];
        *hpos += 1;
        if ptag == K_END {
            break;
        } else if ptag == K_SIZE {
            read_pack_sizes(hp, hpos, num_pack_streams, &mut out_info.pack_sizes, &mut total_pack_size)?;
        } else if ptag == K_CRC {
            skip_pack_crcs(hp, hpos, num_pack_streams)?;
        } else {
            skip_unknown_property(hp, hpos)?;
        }
    }
    out_info.payload_len = usize::try_from(total_pack_size).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    Ok(())
}

fn apply_aes_coder_props(props: &[u8], out_info: &mut SevenZHeaderInfo) {
    let psz = props.len();
    if psz == 0 {
        return;
    }
    let b0 = props[0];
    out_info.aes_num_cycles_power = (b0 & 0x3F) as u32;
    let b1 = if psz >= 2 { props[1] } else { 0 };
    let salt_size = (((b0 >> 7) & 1) + (b1 >> 4)) as usize;
    let iv_size = (((b0 >> 6) & 1) + (b1 & 0x0F)) as usize;
    let mut p_off = if psz >= 2 { 2 } else { 1 };
    if salt_size > 0 && p_off + salt_size <= psz {
        let copy_len = salt_size.min(16);
        out_info.aes_salt[..copy_len].copy_from_slice(&props[p_off..p_off + copy_len]);
        out_info.aes_salt_len = copy_len;
        p_off += salt_size;
    }
    if iv_size > 0 && p_off + iv_size <= psz {
        let copy_len = iv_size.min(16);
        out_info.aes_iv[..copy_len].copy_from_slice(&props[p_off..p_off + copy_len]);
        out_info.aes_iv_len = copy_len;
    }
}

fn parse_single_coder(
    hp: &[u8],
    hpos: &mut usize,
    out_info: &mut SevenZHeaderInfo,
) -> Result<SevenZCoder, TTZipStatus> {
    let hlen = hp.len();
    if *hpos >= hlen {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let flags = hp[*hpos];
    *hpos += 1;
    let method_size = (flags & 0x0F) as usize;
    let method_end = hpos.checked_add(method_size).filter(|&end| end <= hlen).ok_or(TTZipStatus::ErrCorruptHeader)?;

    let mut mid = 0u64;
    for &b in &hp[*hpos..(*hpos + method_size.min(8))] {
        mid = (mid << 8) | (b as u64);
    }
    *hpos = method_end;

    if mid == METHOD_AES {
        out_info.is_encrypted = true;
    } else {
        out_info.primary_method_id = mid;
    }

    let mut in_streams = 1u64;
    let mut out_streams = 1u64;
    if (flags & 0x10) != 0 {
        let ins = next_varint(hp, hpos)?;
        let outs = next_varint(hp, hpos)?;
        if ins > MAX_CODER_STREAMS || outs > MAX_CODER_STREAMS {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        in_streams = ins;
        out_streams = outs;
    }

    let mut props = Vec::new();
    if (flags & 0x20) != 0 {
        let psz = next_varint(hp, hpos)? as usize;
        if psz > 0 && *hpos + psz <= hlen {
            props.extend_from_slice(&hp[*hpos..*hpos + psz]);
            if mid == METHOD_AES {
                apply_aes_coder_props(&props, out_info);
            } else {
                out_info.coder_props = props.clone();
            }
            *hpos += psz;
        }
    }

    Ok(SevenZCoder {
        method_id: mid,
        num_in_streams: in_streams,
        num_out_streams: out_streams,
        properties: props,
    })
}

fn verify_coder_dag_acyclic(
    num_coders: usize,
    adj: &[Vec<usize>],
    in_degree: &mut [usize],
) -> Result<(), TTZipStatus> {
    let mut queue = std::collections::VecDeque::new();
    for (c_idx, &deg) in in_degree.iter().enumerate() {
        if deg == 0 {
            queue.push_back(c_idx);
        }
    }
    let mut visited_count = 0usize;
    while let Some(u) = queue.pop_front() {
        visited_count += 1;
        for &v in &adj[u] {
            in_degree[v] -= 1;
            if in_degree[v] == 0 {
                queue.push_back(v);
            }
        }
    }
    if visited_count != num_coders {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    Ok(())
}

fn parse_folder_bind_pairs_and_packs(
    hp: &[u8],
    hpos: &mut usize,
    folder: &mut SevenZFolder,
) -> Result<(), TTZipStatus> {
    let num_coders = folder.coders.len();
    if num_coders <= 1 {
        return Ok(());
    }
    let num_bind_pairs = num_coders - 1;
    let mut in_to_coder = Vec::new();
    let mut out_to_coder = Vec::new();
    for (c_idx, c) in folder.coders.iter().enumerate() {
        in_to_coder.extend(std::iter::repeat_n(c_idx, c.num_in_streams as usize));
        out_to_coder.extend(std::iter::repeat_n(c_idx, c.num_out_streams as usize));
    }
    let total_in = in_to_coder.len();
    let total_out = out_to_coder.len();
    let mut in_used = vec![false; total_in];
    let mut out_used = vec![false; total_out];
    let mut adj = vec![Vec::new(); num_coders];
    let mut in_degree = vec![0usize; num_coders];

    for _ in 0..num_bind_pairs {
        let in_idx_val = next_varint(hp, hpos)?;
        let out_idx_val = next_varint(hp, hpos)?;
        folder.bind_pairs.push((in_idx_val, out_idx_val));

        let in_idx = in_idx_val as usize;
        let out_idx = out_idx_val as usize;
        if in_idx >= total_in || out_idx >= total_out || in_used[in_idx] || out_used[out_idx] {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        in_used[in_idx] = true;
        out_used[out_idx] = true;

        let in_coder = in_to_coder[in_idx];
        let out_coder = out_to_coder[out_idx];
        if in_coder == out_coder {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        adj[out_coder].push(in_coder);
        in_degree[in_coder] += 1;
    }

    verify_coder_dag_acyclic(num_coders, &adj, &mut in_degree)?;

    let num_packed_streams = total_in.saturating_sub(num_bind_pairs);
    if num_packed_streams > 1 {
        for _ in 0..num_packed_streams {
            folder.packed_streams.push(next_varint(hp, hpos)?);
        }
    }
    Ok(())
}

fn parse_unpack_folders(
    hp: &[u8],
    hpos: &mut usize,
    out_info: &mut SevenZHeaderInfo,
) -> Result<(), TTZipStatus> {
    let hlen = hp.len();
    let num_folders = next_varint(hp, hpos)?;
    if (num_folders as usize) > hlen || *hpos >= hlen {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let external = hp[*hpos];
    *hpos += 1;
    if external != 0 {
        return Ok(());
    }
    for _ in 0..num_folders {
        let mut folder = SevenZFolder::default();
        let num_coders = next_varint(hp, hpos)?;
        if num_coders == 0 || num_coders > 256 || (num_coders as usize) > hlen {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        for _ in 0..num_coders {
            folder.coders.push(parse_single_coder(hp, hpos, out_info)?);
        }
        parse_folder_bind_pairs_and_packs(hp, hpos, &mut folder)?;
        out_info.folders.push(folder);
    }
    Ok(())
}

const MAX_SUBSTREAMS_COUNT: usize = 1_048_576;

fn read_optional_crc_list(
    hp: &[u8],
    hpos: &mut usize,
    count: usize,
) -> Result<Vec<Option<u32>>, TTZipStatus> {
    let hlen = hp.len();
    if count > MAX_SUBSTREAMS_COUNT || *hpos >= hlen {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let all_defined = hp[*hpos];
    *hpos += 1;
    let remaining = hlen - *hpos;
    if all_defined != 0 {
        let crc_bytes = count.checked_mul(4).ok_or(TTZipStatus::ErrCorruptHeader)?;
        if crc_bytes > remaining {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let mut out = vec![None; count];
        for slot in &mut out {
            let s = &hp[*hpos..*hpos + 4];
            *slot = Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]));
            *hpos += 4;
        }
        return Ok(out);
    }
    let bitmask_bytes = count.div_ceil(8);
    if bitmask_bytes > remaining {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let bitmask = &hp[*hpos..*hpos + bitmask_bytes];
    *hpos += bitmask_bytes;
    let mut out = vec![None; count];
    for (i, slot) in out.iter_mut().enumerate() {
        if (bitmask[i / 8] & (1 << (7 - (i % 8)))) != 0 {
            let s = hp.get(*hpos..*hpos + 4).ok_or(TTZipStatus::ErrCorruptHeader)?;
            *slot = Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]));
            *hpos += 4;
        }
    }
    Ok(out)
}

fn parse_unpack_info_section(
    hp: &[u8],
    hpos: &mut usize,
    out_info: &mut SevenZHeaderInfo,
) -> Result<(), TTZipStatus> {
    let hlen = hp.len();
    while *hpos < hlen {
        let utag = hp[*hpos];
        *hpos += 1;
        if utag == K_END {
            break;
        }
        if utag == K_FOLDER {
            parse_unpack_folders(hp, hpos, out_info)?;
        } else if utag == K_CODERS_UNPACK_SIZE {
            for folder in &mut out_info.folders {
                let num_out: usize = folder.coders.iter().map(|c| c.num_out_streams as usize).sum::<usize>().max(1);
                for _ in 0..num_out {
                    if *hpos >= hlen {
                        break;
                    }
                    folder.unpack_sizes.push(next_varint(hp, hpos)?);
                }
            }
        } else if utag == K_CRC {
            let crcs = read_optional_crc_list(hp, hpos, out_info.folders.len())?;
            for (folder, crc) in out_info.folders.iter_mut().zip(crcs) {
                if crc.is_some() {
                    folder.crc = crc;
                }
            }
        } else {
            skip_unknown_property(hp, hpos)?;
        }
    }
    Ok(())
}

fn parse_substream_sizes(
    hp: &[u8],
    hpos: &mut usize,
    folders_count: usize,
    num_unpack_streams_per_folder: &[usize],
    out_info: &SevenZHeaderInfo,
    stream_sizes_per_folder: &mut Vec<Vec<u64>>,
) -> Result<(), TTZipStatus> {
    if stream_sizes_per_folder.len() < folders_count {
        stream_sizes_per_folder.resize(folders_count, Vec::new());
    }
    for (f, streams) in stream_sizes_per_folder.iter_mut().enumerate().take(folders_count) {
        let num_streams = num_unpack_streams_per_folder.get(f).copied().unwrap_or(1);
        let num_explicit = num_streams.saturating_sub(1);
        let mut explicit_sum = 0u64;
        for _ in 0..num_explicit {
            let sval = next_varint(hp, hpos)?;
            explicit_sum = explicit_sum.checked_add(sval).ok_or(TTZipStatus::ErrCorruptHeader)?;
            streams.push(sval);
        }
        if num_streams > 0 {
            let folder_total = out_info
                .folders
                .get(f)
                .map(crate::sevenz::decoder::payload::folder_final_unpack_size)
                .unwrap_or(0);
            streams.push(folder_total.saturating_sub(explicit_sum));
        }
    }
    Ok(())
}

fn finalize_substreams_info(
    out_info: &mut SevenZHeaderInfo,
    num_unpack_streams_per_folder: &[usize],
    stream_sizes_per_folder: Vec<Vec<u64>>,
    size_tag_seen: bool,
) {
    for (f, folder) in out_info.folders.iter_mut().enumerate() {
        folder.num_unpack_streams = num_unpack_streams_per_folder.get(f).copied().unwrap_or(1);
    }
    if size_tag_seen {
        for folder_streams in stream_sizes_per_folder {
            out_info.stream_sizes.extend(folder_streams);
        }
    } else {
        for (f, folder) in out_info.folders.iter().enumerate() {
            if num_unpack_streams_per_folder.get(f).copied().unwrap_or(1) == 1 {
                let sz = crate::sevenz::decoder::payload::folder_final_unpack_size(folder);
                out_info.stream_sizes.push(sz);
            }
        }
    }
}

fn parse_num_unpack_streams_per_folder(
    hp: &[u8],
    hpos: &mut usize,
    folders_count: usize,
    num_unpack_streams_per_folder: &mut Vec<usize>,
) -> Result<(), TTZipStatus> {
    num_unpack_streams_per_folder.resize(folders_count, 1);
    let mut total_unpack = 0usize;
    for slot in num_unpack_streams_per_folder.iter_mut().take(folders_count) {
        let n = usize::try_from(next_varint(hp, hpos)?)
            .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
        total_unpack = total_unpack
            .checked_add(n)
            .ok_or(TTZipStatus::ErrCorruptHeader)?;
        if total_unpack > MAX_SUBSTREAMS_COUNT {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        *slot = n;
    }
    Ok(())
}

fn parse_substreams_info_section(
    hp: &[u8],
    hpos: &mut usize,
    out_info: &mut SevenZHeaderInfo,
) -> Result<(), TTZipStatus> {
    let hlen = hp.len();
    let folders_count = out_info.folders.len().max(1);
    let mut num_unpack_streams_per_folder = vec![1usize; folders_count];
    let mut stream_sizes_per_folder: Vec<Vec<u64>> = vec![Vec::new(); folders_count];
    let mut size_tag_seen = false;

    while *hpos < hlen {
        let stag = hp[*hpos];
        *hpos += 1;
        match stag {
            K_END => break,
            K_NUM_UNPACK_STREAM => {
                parse_num_unpack_streams_per_folder(
                    hp,
                    hpos,
                    folders_count,
                    &mut num_unpack_streams_per_folder,
                )?;
            }
            K_SIZE => {
                size_tag_seen = true;
                parse_substream_sizes(
                    hp,
                    hpos,
                    folders_count,
                    &num_unpack_streams_per_folder,
                    out_info,
                    &mut stream_sizes_per_folder,
                )?;
            }
            K_CRC => {
                let total_streams = num_unpack_streams_per_folder
                    .iter()
                    .try_fold(0usize, |acc, &n| acc.checked_add(n))
                    .filter(|&sum| sum <= MAX_SUBSTREAMS_COUNT)
                    .ok_or(TTZipStatus::ErrCorruptHeader)?;
                out_info.stream_crcs.extend(
                    read_optional_crc_list(hp, hpos, total_streams)?
                        .into_iter()
                        .flatten(),
                );
            }
            _ => skip_unknown_property(hp, hpos)?,
        }
    }

    finalize_substreams_info(
        out_info,
        &num_unpack_streams_per_folder,
        stream_sizes_per_folder,
        size_tag_seen,
    );
    Ok(())
}

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
        if *hpos + mask_bytes <= prop_end {
            for (i, slot) in defined.iter_mut().enumerate().take(num_files) {
                let b = hp[*hpos + (i / 8)];
                *slot = ((b >> (7 - (i % 8))) & 1) != 0;
            }
            *hpos += mask_bytes;
        }
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
        file.mode = if posix_mode != 0 { posix_mode } else { 0o120777 };
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

fn parse_files_info_section(
    hp: &[u8],
    hpos: &mut usize,
    out_info: &mut SevenZHeaderInfo,
) -> Result<(), TTZipStatus> {
    let hlen = hp.len();
    let num_files_val = next_varint(hp, hpos)?;
    if num_files_val > 1_000_000 || (num_files_val as usize) > hlen.saturating_mul(1024) {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    out_info.files.resize(num_files_val as usize, SevenZFileMeta::default());

    while *hpos < hlen {
        let ftag = hp[*hpos];
        *hpos += 1;
        if ftag == K_END {
            break;
        }
        let prop_size = next_varint(hp, hpos)? as usize;
        let prop_end = hpos.checked_add(prop_size).filter(|&end| end <= hlen).ok_or(TTZipStatus::ErrCorruptHeader)?;

        match ftag {
            K_EMPTY_STREAM | K_EMPTY_FILE => {
                apply_empty_streams_and_files(ftag, hp, *hpos, prop_size, &mut out_info.files);
            }
            K_NAME => parse_files_names(hp, *hpos, prop_end, &mut out_info.files)?,
            K_MTIME => parse_files_mtime(hp, *hpos, prop_end, &mut out_info.files),
            K_WIN_ATTRIBUTES => parse_files_win_attributes(hp, *hpos, prop_end, &mut out_info.files),
            _ => {}
        }
        *hpos = prop_end;
    }
    Ok(())
}

fn populate_fallback_stream_sizes_and_crcs(out_info: &mut SevenZHeaderInfo) {
    if out_info.stream_sizes.is_empty() && !out_info.folders.is_empty() {
        for folder in &mut out_info.folders {
            if folder.num_unpack_streams == 0 {
                folder.num_unpack_streams = 1;
            }
            let sz = crate::sevenz::decoder::payload::folder_final_unpack_size(folder);
            out_info.stream_sizes.push(sz);
        }
    }
    let can_inherit_crcs = out_info.stream_crcs.is_empty()
        && !out_info.folders.is_empty()
        && out_info.folders.iter().all(|f| f.num_unpack_streams <= 1 && f.crc.is_some());
    if can_inherit_crcs {
        for c in out_info.folders.iter().filter_map(|f| f.crc) {
            out_info.stream_crcs.push(c);
        }
    }
}

fn finalize_header_streams_and_pack_offsets(out_info: &mut SevenZHeaderInfo) {
    populate_fallback_stream_sizes_and_crcs(out_info);

    let mut cur_pack_offset = out_info.payload_offset;
    let mut pack_idx = 0usize;
    let num_folders = out_info.folders.len();
    for folder in &mut out_info.folders {
        let remaining_packs = out_info.pack_sizes.len().saturating_sub(pack_idx);
        let num_pack_streams = folder.pack_stream_count().min(remaining_packs).max(1);
        let mut folder_pack_len = 0usize;
        for _ in 0..num_pack_streams {
            if let Some(&pack_size) = out_info.pack_sizes.get(pack_idx) {
                folder_pack_len = folder_pack_len.saturating_add(pack_size as usize);
                pack_idx += 1;
            }
        }
        if folder_pack_len == 0 && num_folders == 1 {
            folder_pack_len = out_info.payload_len;
        }
        folder.packed_offset = cur_pack_offset;
        folder.packed_len = folder_pack_len;
        cur_pack_offset = cur_pack_offset.saturating_add(folder_pack_len);
    }
}

/// Helper function to parse 7z Header structures from a byte buffer.
pub fn parse_7z_header_stream(
    hp: &[u8],
    out_info: &mut SevenZHeaderInfo,
) -> Result<(), TTZipStatus> {
    let mut hpos = 0;
    let hlen = hp.len();

    while hpos < hlen {
        let tag = hp[hpos];
        hpos += 1;

        match tag {
            K_END | K_HEADER | K_MAIN_STREAMS_INFO => continue,
            K_PACK_INFO => parse_pack_info_section(hp, &mut hpos, out_info)?,
            K_UNPACK_INFO => parse_unpack_info_section(hp, &mut hpos, out_info)?,
            K_SUB_STREAMS_INFO => parse_substreams_info_section(hp, &mut hpos, out_info)?,
            K_FILES_INFO => parse_files_info_section(hp, &mut hpos, out_info)?,
            _ => skip_unknown_property(hp, &mut hpos)?,
        }
    }

    finalize_header_streams_and_pack_offsets(out_info);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_optional_crc_list_and_substreams_reject_oversized_counts_on_short_header() {
        let mut hpos = 0usize;
        assert_eq!(
            read_optional_crc_list(&[0x01u8, 0x00], &mut hpos, 10_000_000),
            Err(TTZipStatus::ErrCorruptHeader)
        );
        let mut hpos_mask = 0usize;
        assert_eq!(
            read_optional_crc_list(&[0x00u8, 0xFF], &mut hpos_mask, 500_000),
            Err(TTZipStatus::ErrCorruptHeader)
        );
        let header_bytes = [K_NUM_UNPACK_STREAM, 0xE0, 0x40, 0x4B, 0x4C, K_CRC, 0x01, K_END];
        let mut info = SevenZHeaderInfo {
            folders: vec![SevenZFolder::default()],
            ..Default::default()
        };
        let mut pos = 0usize;
        assert_eq!(
            parse_substreams_info_section(&header_bytes, &mut pos, &mut info),
            Err(TTZipStatus::ErrCorruptHeader)
        );
    }
}

