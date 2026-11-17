// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! 7-Zip Header and EncodedHeader Zero-Copy Binary Metadata Stream Parser.

mod files_info;

use super::models::{SevenZCoder, SevenZFolder, SevenZHeaderInfo};
use super::preview::HeaderAdmission;
use files_info::parse_files_info_section;
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

fn read_pack_location(
    hp: &[u8],
    hpos: &mut usize,
    out_info: &mut SevenZHeaderInfo,
    admission: HeaderAdmission,
) -> Result<u64, TTZipStatus> {
    let pack_pos = next_varint(hp, hpos)?;
    let num_pack_streams = next_varint(hp, hpos)?;
    admission.count(num_pack_streams, 1)?;
    if (num_pack_streams as usize) > hp.len() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let pack_pos_usize = usize::try_from(pack_pos).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    out_info.payload_offset = 32usize.checked_add(pack_pos_usize).ok_or(TTZipStatus::ErrCorruptHeader)?;
    Ok(num_pack_streams)
}

fn parse_pack_info_section(
    hp: &[u8],
    hpos: &mut usize,
    out_info: &mut SevenZHeaderInfo,
    admission: HeaderAdmission,
) -> Result<(), TTZipStatus> {
    let hlen = hp.len();
    let num_pack_streams = read_pack_location(hp, hpos, out_info, admission)?;
    let mut total_pack_size = 0u64;

    let mut tags = 0;
    while *hpos < hlen {
        let ptag = admission.next_tag(hp, hpos, &mut tags)?;
        match ptag {
            K_END => break,
            K_SIZE => read_pack_sizes(hp, hpos, num_pack_streams, &mut out_info.pack_sizes, &mut total_pack_size)?,
            K_CRC => skip_pack_crcs(hp, hpos, num_pack_streams)?,
            _ => skip_unknown_property(hp, hpos)?,
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

fn read_coder_properties(hp: &[u8], hpos: &mut usize) -> Result<Vec<u8>, TTZipStatus> {
    let size = usize::try_from(next_varint(hp, hpos)?)
        .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    let end = hpos.checked_add(size).ok_or(TTZipStatus::ErrCorruptHeader)?;
    let properties = hp.get(*hpos..end).ok_or(TTZipStatus::ErrCorruptHeader)?;
    *hpos = end;
    Ok(properties.to_vec())
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

    let props = if (flags & 0x20) != 0 {
        let properties = read_coder_properties(hp, hpos)?;
        if mid == METHOD_AES {
            apply_aes_coder_props(&properties, out_info);
        } else if !properties.is_empty() {
            out_info.coder_props = properties.clone();
        }
        properties
    } else {
        Vec::new()
    };

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
    admission: HeaderAdmission,
) -> Result<(), TTZipStatus> {
    let hlen = hp.len();
    let num_folders = next_varint(hp, hpos)?;
    admission.count(num_folders, 1)?;
    if (num_folders as usize) > hlen || *hpos >= hlen {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let external = hp[*hpos];
    *hpos += 1;
    if external != 0 {
        admission.unsupported()?;
        return Ok(());
    }
    for _ in 0..num_folders {
        let folder = parse_unpack_folder(hp, hpos, out_info, admission)?;
        out_info.folders.push(folder);
    }
    Ok(())
}

fn parse_unpack_folder(
    hp: &[u8],
    hpos: &mut usize,
    out_info: &mut SevenZHeaderInfo,
    admission: HeaderAdmission,
) -> Result<SevenZFolder, TTZipStatus> {
    let mut folder = SevenZFolder::default();
    let num_coders = next_varint(hp, hpos)?;
    admission.count(num_coders, 1)?;
    if num_coders == 0 || num_coders > 256 || (num_coders as usize) > hp.len() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    for _ in 0..num_coders {
        admission.coder(hp.get(*hpos..hpos.saturating_add(4)))?;
        folder.coders.push(parse_single_coder(hp, hpos, out_info)?);
    }
    parse_folder_bind_pairs_and_packs(hp, hpos, &mut folder)?;
    Ok(folder)
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
    admission: HeaderAdmission,
) -> Result<(), TTZipStatus> {
    let hlen = hp.len();
    let mut tags = 0;
    while *hpos < hlen {
        let utag = admission.next_tag(hp, hpos, &mut tags)?;
        match utag {
            K_END => break,
            K_FOLDER => parse_unpack_folders(hp, hpos, out_info, admission)?,
            K_CODERS_UNPACK_SIZE => read_folder_unpack_sizes(hp, hpos, &mut out_info.folders)?,
            K_CRC => {
                let crcs = read_optional_crc_list(hp, hpos, out_info.folders.len())?;
                for (folder, crc) in out_info.folders.iter_mut().zip(crcs) {
                    if crc.is_some() {
                        folder.crc = crc;
                    }
                }
            }
            _ => skip_unknown_property(hp, hpos)?,
        }
    }
    Ok(())
}

fn read_folder_unpack_sizes(
    hp: &[u8],
    hpos: &mut usize,
    folders: &mut [SevenZFolder],
) -> Result<(), TTZipStatus> {
    for folder in folders {
        let num_out = folder.coders.iter().map(|coder| coder.num_out_streams as usize).sum::<usize>().max(1);
        for _ in 0..num_out {
            if *hpos >= hp.len() {
                break;
            }
            folder.unpack_sizes.push(next_varint(hp, hpos)?);
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
    admission: HeaderAdmission,
) -> Result<(), TTZipStatus> {
    num_unpack_streams_per_folder.resize(folders_count, 1);
    let mut total_unpack = 0usize;
    for slot in num_unpack_streams_per_folder.iter_mut().take(folders_count) {
        let n = usize::try_from(next_varint(hp, hpos)?)
            .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
        total_unpack = total_unpack
            .checked_add(n)
            .ok_or(TTZipStatus::ErrCorruptHeader)?;
        admission.count(total_unpack as u64, 1024)?;
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
    admission: HeaderAdmission,
) -> Result<(), TTZipStatus> {
    let hlen = hp.len();
    let folders_count = out_info.folders.len().max(1);
    let mut num_unpack_streams_per_folder = vec![1usize; folders_count];
    let mut stream_sizes_per_folder: Vec<Vec<u64>> = vec![Vec::new(); folders_count];
    let mut size_tag_seen = false;

    let mut tags = 0;
    while *hpos < hlen {
        let stag = admission.next_tag(hp, hpos, &mut tags)?;
        match stag {
            K_END => break,
            K_NUM_UNPACK_STREAM => {
                parse_num_unpack_streams_per_folder(
                    hp,
                    hpos,
                    folders_count,
                    &mut num_unpack_streams_per_folder,
                    admission,
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
                let crcs = read_optional_crc_list(hp, hpos, total_streams)?;
                admission.require_complete_substream_crcs(&crcs)?;
                out_info.stream_crcs.extend(crcs.into_iter().flatten());
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
    parse_7z_header_stream_with_admission(hp, out_info, HeaderAdmission::Standard)
}

pub(super) fn parse_7z_header_stream_with_admission(
    hp: &[u8],
    out_info: &mut SevenZHeaderInfo,
    admission: HeaderAdmission,
) -> Result<(), TTZipStatus> {
    let mut hpos = 0;
    let hlen = hp.len();

    let mut tags = 0;
    while hpos < hlen {
        let tag = admission.next_tag(hp, &mut hpos, &mut tags)?;

        match tag {
            K_END | K_HEADER | K_MAIN_STREAMS_INFO => continue,
            K_PACK_INFO => parse_pack_info_section(hp, &mut hpos, out_info, admission)?,
            K_UNPACK_INFO => parse_unpack_info_section(hp, &mut hpos, out_info, admission)?,
            K_SUB_STREAMS_INFO => parse_substreams_info_section(hp, &mut hpos, out_info, admission)?,
            K_FILES_INFO => parse_files_info_section(hp, &mut hpos, out_info, admission)?,
            _ => {
                admission.unsupported()?;
                skip_unknown_property(hp, &mut hpos)?;
            }
        }
    }

    finalize_header_streams_and_pack_offsets(out_info);
    Ok(())
}

#[cfg(test)]
mod tests;
