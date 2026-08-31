// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 7-Zip Header and EncodedHeader Zero-Copy Binary Metadata Stream Parser.

use super::models::{SevenZCoder, SevenZFileMeta, SevenZFolder, SevenZHeaderInfo};
use crate::sevenz::format::*;
use crate::types::TTZipStatus;

/// Helper function to parse 7z Header structures from a byte buffer.
pub fn parse_7z_header_stream(hp: &[u8], out_info: &mut SevenZHeaderInfo) -> Result<(), TTZipStatus> {
    let mut hpos = 0;
    let hlen = hp.len();

    while hpos < hlen {
        let tag = hp[hpos];
        hpos += 1;

        if tag == K_END || tag == K_HEADER || tag == K_MAIN_STREAMS_INFO {
            continue;
        }

        if tag == K_PACK_INFO {
            let (pack_pos, rd1) = read_varint(&hp[hpos..]).ok_or(TTZipStatus::ErrCorruptHeader)?;
            hpos += rd1;
            let (num_pack_streams, rd2) = read_varint(&hp[hpos..]).ok_or(TTZipStatus::ErrCorruptHeader)?;
            hpos += rd2;
            if (num_pack_streams as usize) > hlen {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            out_info.payload_offset = 32usize.saturating_add(pack_pos as usize);
            let mut total_pack_size = 0u64;

            while hpos < hlen {
                let ptag = hp[hpos];
                hpos += 1;
                if ptag == K_END {
                    break;
                }
                if ptag == K_SIZE {
                    for _ in 0..num_pack_streams {
                        let (sz, rd) = read_varint(&hp[hpos..]).ok_or(TTZipStatus::ErrCorruptHeader)?;
                        hpos += rd;
                        out_info.pack_sizes.push(sz);
                        total_pack_size = total_pack_size.saturating_add(sz);
                    }
                } else if ptag == K_CRC {
                    let all_defined = hp[hpos];
                    hpos += 1;
                    if all_defined != 0 {
                        let crc_bytes = (num_pack_streams as usize).saturating_mul(4);
                        if hpos.saturating_add(crc_bytes) > hlen {
                            return Err(TTZipStatus::ErrCorruptHeader);
                        }
                        hpos += crc_bytes;
                    }
                } else {
                    let (sz, rd) = read_varint(&hp[hpos..]).ok_or(TTZipStatus::ErrCorruptHeader)?;
                    hpos = hpos.saturating_add(rd).saturating_add(sz as usize);
                    if hpos > hlen {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                }
            }
            out_info.payload_len = total_pack_size as usize;
        } else if tag == K_UNPACK_INFO {
            while hpos < hlen {
                let utag = hp[hpos];
                hpos += 1;
                if utag == K_END {
                    break;
                }
                if utag == K_FOLDER {
                    let (num_folders, rd) = read_varint(&hp[hpos..]).ok_or(TTZipStatus::ErrCorruptHeader)?;
                    hpos += rd;
                    if (num_folders as usize) > hlen {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    let external = hp[hpos];
                    hpos += 1;

                    if external == 0 {
                        for _ in 0..num_folders {
                            let mut folder = SevenZFolder::default();
                            let (num_coders, rd) = read_varint(&hp[hpos..]).ok_or(TTZipStatus::ErrCorruptHeader)?;
                            hpos += rd;

                            if num_coders == 0 || num_coders > 256 || (num_coders as usize) > hlen {
                                return Err(TTZipStatus::ErrCorruptHeader);
                            }

                            for _ in 0..num_coders {
                                let flags = hp[hpos];
                                hpos += 1;
                                let method_size = (flags & 0x0F) as usize;
                                let mut mid = 0u64;
                                for m in 0..method_size.min(8) {
                                    mid = (mid << 8) | (hp[hpos + m] as u64);
                                }
                                hpos += method_size;

                                if mid == METHOD_AES {
                                    out_info.is_encrypted = true;
                                } else {
                                    out_info.primary_method_id = mid;
                                }

                                let mut in_streams = 1u64;
                                let mut out_streams = 1u64;
                                if (flags & 0x10) != 0 {
                                    let (ins, rd) = read_varint(&hp[hpos..]).ok_or(TTZipStatus::ErrCorruptHeader)?;
                                    hpos += rd;
                                    let (outs, rd) = read_varint(&hp[hpos..]).ok_or(TTZipStatus::ErrCorruptHeader)?;
                                    hpos += rd;
                                    in_streams = ins;
                                    out_streams = outs;
                                }

                                let mut props = Vec::new();
                                if (flags & 0x20) != 0 {
                                    let (props_sz, rd) = read_varint(&hp[hpos..]).ok_or(TTZipStatus::ErrCorruptHeader)?;
                                    hpos += rd;
                                    let psz = props_sz as usize;
                                    if psz > 0 && hpos + psz <= hlen {
                                        props.extend_from_slice(&hp[hpos..hpos + psz]);

                                        if mid == METHOD_AES && psz >= 1 {
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
                                        } else if mid != METHOD_AES {
                                            out_info.coder_props = props.clone();
                                        }

                                        hpos += psz;
                                    }
                                }

                                folder.coders.push(SevenZCoder {
                                    method_id: mid,
                                    num_in_streams: in_streams,
                                    num_out_streams: out_streams,
                                    properties: props,
                                });
                            }

                            if num_coders > 1 {
                                let num_bind_pairs = (num_coders - 1) as usize;
                                let total_in: usize = folder.coders.iter().map(|c| c.num_in_streams as usize).sum();
                                let total_out: usize = folder.coders.iter().map(|c| c.num_out_streams as usize).sum();

                                let mut in_to_coder = Vec::with_capacity(total_in);
                                for (c_idx, c) in folder.coders.iter().enumerate() {
                                    for _ in 0..c.num_in_streams {
                                        in_to_coder.push(c_idx);
                                    }
                                }

                                let mut out_to_coder = Vec::with_capacity(total_out);
                                for (c_idx, c) in folder.coders.iter().enumerate() {
                                    for _ in 0..c.num_out_streams {
                                        out_to_coder.push(c_idx);
                                    }
                                }

                                let mut in_used = vec![false; total_in];
                                let mut out_used = vec![false; total_out];
                                let mut adj = vec![Vec::new(); num_coders as usize];
                                let mut in_degree = vec![0usize; num_coders as usize];

                                for _ in 0..num_bind_pairs {
                                    let (in_idx_val, rd1) = read_varint(&hp[hpos..]).ok_or(TTZipStatus::ErrCorruptHeader)?;
                                    hpos += rd1;
                                    let (out_idx_val, rd2) = read_varint(&hp[hpos..]).ok_or(TTZipStatus::ErrCorruptHeader)?;
                                    hpos += rd2;

                                    let in_idx = in_idx_val as usize;
                                    let out_idx = out_idx_val as usize;

                                    if in_idx >= total_in || out_idx >= total_out {
                                        return Err(TTZipStatus::ErrCorruptHeader);
                                    }
                                    if in_used[in_idx] || out_used[out_idx] {
                                        return Err(TTZipStatus::ErrCorruptHeader);
                                    }
                                    in_used[in_idx] = true;
                                    out_used[out_idx] = true;

                                    let in_coder = in_to_coder[in_idx];
                                    let out_coder = out_to_coder[out_idx];

                                    // Self-loop check (In == Out on same coder)
                                    if in_coder == out_coder {
                                        return Err(TTZipStatus::ErrCorruptHeader);
                                    }

                                    adj[out_coder].push(in_coder);
                                    in_degree[in_coder] += 1;
                                }

                                // Kahn's topological sort for cycle detection
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

                                if visited_count != num_coders as usize {
                                    return Err(TTZipStatus::ErrCorruptHeader);
                                }

                                let num_packed_streams = total_in.saturating_sub(num_bind_pairs);
                                if num_packed_streams > 1 {
                                    for _ in 0..num_packed_streams {
                                        let (_, rd) = read_varint(&hp[hpos..]).ok_or(TTZipStatus::ErrCorruptHeader)?;
                                        hpos += rd;
                                    }
                                }
                            }

                            out_info.folders.push(folder);
                        }
                    }
                } else if utag == K_CODERS_UNPACK_SIZE {
                    for folder in &mut out_info.folders {
                        let num_out: usize = folder.coders.iter().map(|c| c.num_out_streams as usize).sum::<usize>().max(1);
                        for _ in 0..num_out {
                            if hpos >= hlen {
                                break;
                            }
                            let (folder_unpack_sz, rd) = read_varint(&hp[hpos..]).ok_or(TTZipStatus::ErrCorruptHeader)?;
                            hpos += rd;
                            folder.unpack_sizes.push(folder_unpack_sz);
                        }
                    }
                } else if utag == K_CRC {
                    let all_defined = hp[hpos];
                    hpos += 1;
                    if all_defined != 0 {
                        for folder in &mut out_info.folders {
                            if hpos + 4 <= hlen {
                                folder.crc = Some(u32::from_le_bytes(hp[hpos..hpos + 4].try_into().unwrap()));
                                hpos += 4;
                            }
                        }
                    } else {
                        let num_folders = out_info.folders.len();
                        let bitmask_bytes = num_folders.div_ceil(8);
                        if hpos + bitmask_bytes <= hlen {
                            let bitmask = &hp[hpos..hpos + bitmask_bytes];
                            hpos += bitmask_bytes;
                            for (i, folder) in out_info.folders.iter_mut().enumerate() {
                                let byte_idx = i / 8;
                                let bit_idx = 7 - (i % 8);
                                if (bitmask[byte_idx] & (1 << bit_idx)) != 0 && hpos + 4 <= hlen {
                                    folder.crc = Some(u32::from_le_bytes(hp[hpos..hpos + 4].try_into().unwrap()));
                                    hpos += 4;
                                }
                            }
                        }
                    }
                } else {
                    let (sz, rd) = read_varint(&hp[hpos..]).ok_or(TTZipStatus::ErrCorruptHeader)?;
                    hpos += rd + (sz as usize);
                }
            }
        } else if tag == K_SUB_STREAMS_INFO {
            let num_folders = out_info.folders.len();
            let mut num_unpack_streams_per_folder = vec![1usize; num_folders.max(1)];
            let mut stream_sizes_per_folder: Vec<Vec<u64>> = vec![Vec::new(); num_folders.max(1)];
            let mut size_tag_seen = false;

            while hpos < hlen {
                let stag = hp[hpos];
                hpos += 1;
                if stag == K_END {
                    break;
                }
                if stag == K_NUM_UNPACK_STREAM {
                    let folders_count = if num_folders > 0 { num_folders } else { 1 };
                    num_unpack_streams_per_folder.resize(folders_count, 1);
                    for f in 0..folders_count {
                        let (ns, rd) = read_varint(&hp[hpos..]).ok_or(TTZipStatus::ErrCorruptHeader)?;
                        hpos += rd;
                        num_unpack_streams_per_folder[f] = ns as usize;
                    }
                } else if stag == K_SIZE {
                    size_tag_seen = true;
                    let folders_count = if num_folders > 0 { num_folders } else { 1 };
                    if stream_sizes_per_folder.len() < folders_count {
                        stream_sizes_per_folder.resize(folders_count, Vec::new());
                    }
                    for f in 0..folders_count {
                        let num_streams = num_unpack_streams_per_folder.get(f).copied().unwrap_or(1);
                        let num_explicit = num_streams.saturating_sub(1);
                        let mut explicit_sum = 0u64;
                        for _ in 0..num_explicit {
                            let (sval, rd) = read_varint(&hp[hpos..]).ok_or(TTZipStatus::ErrCorruptHeader)?;
                            hpos += rd;
                            explicit_sum += sval;
                            stream_sizes_per_folder[f].push(sval);
                        }
                        if num_streams > 0 {
                            let folder_total = out_info
                                .folders
                                .get(f)
                                .and_then(|folder| folder.unpack_sizes.last().copied().or_else(|| folder.unpack_sizes.first().copied()))
                                .unwrap_or(0);
                            let last_sz = folder_total.saturating_sub(explicit_sum);
                            stream_sizes_per_folder[f].push(last_sz);
                        }
                    }
                } else if stag == K_CRC {
                    let all_defined = hp[hpos];
                    hpos += 1;
                    let total_streams: usize = num_unpack_streams_per_folder.iter().sum();
                    if all_defined != 0 {
                        for _ in 0..total_streams {
                            if hpos + 4 <= hlen {
                                let c = u32::from_le_bytes(hp[hpos..hpos + 4].try_into().unwrap());
                                hpos += 4;
                                out_info.stream_crcs.push(c);
                            }
                        }
                    } else {
                        let bitmask_bytes = total_streams.div_ceil(8);
                        if hpos + bitmask_bytes <= hlen {
                            let bitmask = &hp[hpos..hpos + bitmask_bytes];
                            hpos += bitmask_bytes;
                            for i in 0..total_streams {
                                let byte_idx = i / 8;
                                let bit_idx = 7 - (i % 8);
                                if (bitmask[byte_idx] & (1 << bit_idx)) != 0 && hpos + 4 <= hlen {
                                    let c = u32::from_le_bytes(hp[hpos..hpos + 4].try_into().unwrap());
                                    hpos += 4;
                                    out_info.stream_crcs.push(c);
                                }
                            }
                        }
                    }
                } else {
                    let (sz, rd) = read_varint(&hp[hpos..]).ok_or(TTZipStatus::ErrCorruptHeader)?;
                    hpos += rd + (sz as usize);
                }
            }

            // Sync num_unpack_streams to each folder model
            for (f, folder) in out_info.folders.iter_mut().enumerate() {
                folder.num_unpack_streams = num_unpack_streams_per_folder.get(f).copied().unwrap_or(1);
            }

            if size_tag_seen {
                for folder_streams in stream_sizes_per_folder {
                    out_info.stream_sizes.extend(folder_streams);
                }
            } else {
                for (f, folder) in out_info.folders.iter().enumerate() {
                    let count = num_unpack_streams_per_folder.get(f).copied().unwrap_or(1);
                    if count == 1 {
                        let sz = folder.unpack_sizes.last().copied().unwrap_or(0);
                        out_info.stream_sizes.push(sz);
                    }
                }
            }
        } else if tag == K_FILES_INFO {
            let (num_files_val, rd) = read_varint(&hp[hpos..]).ok_or(TTZipStatus::ErrCorruptHeader)?;
            hpos += rd;
            if num_files_val > 1_000_000 || (num_files_val as usize) > hlen.saturating_mul(1024) {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            let num_files = num_files_val as usize;
            out_info.files.resize(num_files, SevenZFileMeta::default());

            while hpos < hlen {
                let ftag = hp[hpos];
                hpos += 1;
                if ftag == K_END {
                    break;
                }

                let (prop_size_val, rd) = read_varint(&hp[hpos..]).ok_or(TTZipStatus::ErrCorruptHeader)?;
                hpos += rd;
                let prop_size = prop_size_val as usize;
                let prop_end = hpos + prop_size;

                if ftag == K_EMPTY_STREAM {
                    for i in 0..num_files {
                        let byte_idx = i / 8;
                        let bit_idx = 7 - (i % 8);
                        if byte_idx < prop_size {
                            let b = hp[hpos + byte_idx];
                            if ((b >> bit_idx) & 1) != 0 {
                                out_info.files[i].is_empty_stream = true;
                            }
                        }
                    }
                    hpos = prop_end;
                } else if ftag == K_NAME {
                    let _ext = hp[hpos];
                    hpos += 1;
                    let mut name_pos = hpos;
                    let mut name_bytes_left = prop_size.saturating_sub(1);

                    for f in 0..num_files {
                        let mut u16_chars = Vec::new();
                        while name_bytes_left >= 2 {
                            let ch = u16::from_le_bytes(hp[name_pos..name_pos + 2].try_into().unwrap());
                            name_pos += 2;
                            name_bytes_left -= 2;
                            if ch == 0 {
                                break;
                            }
                            u16_chars.push(ch);
                        }
                        let utf8_name = String::from_utf16_lossy(&u16_chars).replace('\\', "/");
                        out_info.files[f].rel_path = utf8_name;
                    }
                    hpos = prop_end;
                } else if ftag == K_WIN_ATTRIBUTES {
                    if hpos < prop_end {
                        let all_defined = hp[hpos];
                        hpos += 1;
                        if all_defined == 1 && hpos < prop_end {
                            let external = hp[hpos];
                            hpos += 1;
                            if external == 0 {
                                for f in 0..num_files {
                                    if hpos + 4 <= prop_end {
                                        let attr = u32::from_le_bytes(hp[hpos..hpos + 4].try_into().unwrap());
                                        hpos += 4;
                                        if (attr & 0x10) != 0 {
                                            out_info.files[f].is_directory = true;
                                        }
                                        out_info.files[f].mode = if out_info.files[f].is_directory {
                                            0o755
                                        } else {
                                            0o644
                                        };
                                    }
                                }
                            }
                        }
                    }
                    hpos = prop_end;
                } else {
                    hpos = prop_end;
                }
            }
        } else {
            let (sz, rd) = read_varint(&hp[hpos..]).ok_or(TTZipStatus::ErrCorruptHeader)?;
            hpos += rd + (sz as usize);
        }
    }

    // If stream_sizes is empty and folders are present, initialize 1 stream per folder from unpack_sizes
    if out_info.stream_sizes.is_empty() && !out_info.folders.is_empty() {
        for folder in &mut out_info.folders {
            if folder.num_unpack_streams == 0 {
                folder.num_unpack_streams = 1;
            }
            let sz = folder.unpack_sizes.last().copied().unwrap_or(0);
            out_info.stream_sizes.push(sz);
        }
    }

    // Map packed offsets and lengths to folders
    let mut cur_pack_offset = out_info.payload_offset;
    let mut pack_idx = 0usize;
    let num_folders = out_info.folders.len();
    for folder in &mut out_info.folders {
        let mut folder_pack_len = 0usize;
        if pack_idx < out_info.pack_sizes.len() {
            folder_pack_len = out_info.pack_sizes[pack_idx] as usize;
            pack_idx += 1;
        } else if num_folders == 1 {
            folder_pack_len = out_info.payload_len;
        }
        folder.packed_offset = cur_pack_offset;
        folder.packed_len = folder_pack_len;
        cur_pack_offset += folder_pack_len;
    }

    Ok(())
}
