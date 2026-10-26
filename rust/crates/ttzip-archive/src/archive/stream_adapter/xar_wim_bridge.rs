// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Pure Safe Rust USTAR transcoders for eXtensible ARchive (`xar!`) and uncompressed WIM (`MSWIM\0\0\0`).

use std::collections::HashMap;
use std::io::Read;

use super::container_bridge::{
    append_ustar_record, finalize_ustar_stream, TranscodedContainerStream, UstarRecord,
    MAX_TRANSCODE_CONTAINER_BYTES,
};
use crate::types::TTZipStatus;

// ---------------------------------------------------------------------------
// 1. XAR (`xar!`) Pure Safe Rust XML TOC + Heap Extent Transcoder
// ---------------------------------------------------------------------------

const STREAM_DECODE_CHUNK_BYTES: usize = 65536;

/// Reads decompressed bytes from `reader` while enforcing both `expected_len` and `remaining_budget`
/// during decompression (never trusting `expected_len` for pre-allocation or reading unboundedly).
fn read_decompressed_bounded_exact<R: Read>(
    reader: R,
    expected_len: usize,
    remaining_budget: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    if expected_len > MAX_TRANSCODE_CONTAINER_BYTES as usize || expected_len > remaining_budget {
        return Err(TTZipStatus::ErrOutOfMemory);
    }
    let mut limited = reader.take((expected_len as u64).saturating_add(1));
    let mut out = Vec::with_capacity(expected_len.min(STREAM_DECODE_CHUNK_BYTES));
    let mut chunk = [0u8; STREAM_DECODE_CHUNK_BYTES];
    loop {
        match limited.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                if out.len().saturating_add(n) > expected_len {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
                out.extend_from_slice(&chunk[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(TTZipStatus::ErrCorruptHeader),
        }
    }
    if out.len() != expected_len {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    Ok(out)
}

pub(super) fn transcode_xar_bytes(raw: &[u8]) -> Result<TranscodedContainerStream, TTZipStatus> {
    if raw.len() < 28 || &raw[..4] != b"xar!" {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let header_size = u16::from_be_bytes([raw[4], raw[5]]) as usize;
    let version = u16::from_be_bytes([raw[6], raw[7]]);
    let toc_len_comp = u64::from_be_bytes(raw[8..16].try_into().unwrap()) as usize;
    let toc_len_uncomp = u64::from_be_bytes(raw[16..24].try_into().unwrap()) as usize;

    if version != 1
        || header_size < 28
        || toc_len_comp == 0
        || toc_len_uncomp == 0
        || header_size.saturating_add(toc_len_comp) > raw.len()
    {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    if toc_len_uncomp > MAX_TRANSCODE_CONTAINER_BYTES as usize {
        return Err(TTZipStatus::ErrOutOfMemory);
    }

    let mut remaining_budget = MAX_TRANSCODE_CONTAINER_BYTES as usize;
    let heap_offset = header_size + toc_len_comp;
    let decoder = flate2::read::ZlibDecoder::new(&raw[header_size..heap_offset]);
    let toc_xml_bytes = read_decompressed_bounded_exact(decoder, toc_len_uncomp, remaining_budget)?;
    remaining_budget = remaining_budget.saturating_sub(toc_xml_bytes.len());

    let toc_xml = std::str::from_utf8(&toc_xml_bytes).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    let Some(toc_start) = toc_xml.find("<toc") else {
        return Err(TTZipStatus::ErrCorruptHeader);
    };
    let Some(toc_end) = toc_xml.rfind("</toc>") else {
        return Err(TTZipStatus::ErrCorruptHeader);
    };

    let heap = &raw[heap_offset..];
    let mut ustar_out = Vec::new();
    let mut entry_count = 0usize;
    parse_xar_file_nodes(
        &toc_xml[toc_start..toc_end],
        "",
        heap,
        &mut ustar_out,
        &mut entry_count,
        &mut remaining_budget,
    )?;

    Ok(finalize_ustar_stream(ustar_out, entry_count))
}

fn find_next_file_open_tag(xml: &str, from: usize) -> Option<(usize, usize)> {
    let mut search = from;
    while let Some(rel) = xml[search..].find("<file") {
        let start = search + rel;
        let after = xml.as_bytes().get(start + 5).copied().unwrap_or(0);
        if after == b'>' || after == b' ' || after == b'\t' || after == b'\n' || after == b'\r' {
            let gt = xml[start..].find('>')?;
            return Some((start, start + gt + 1));
        }
        search = start + 5;
    }
    None
}

fn find_matching_file_close_tag(xml: &str, body_start: usize) -> Option<usize> {
    let mut depth = 1usize;
    let mut pos = body_start;
    while pos < xml.len() {
        let next_open = find_next_file_open_tag(xml, pos).map(|(s, _)| s);
        let next_close = xml[pos..].find("</file>").map(|rel| pos + rel);
        match (next_open, next_close) {
            (Some(o), Some(c)) if o < c => {
                depth += 1;
                pos = o + 5;
            }
            (_, Some(c)) => {
                depth -= 1;
                if depth == 0 {
                    return Some(c);
                }
                pos = c + 7;
            }
            _ => return None,
        }
    }
    None
}

fn extract_xml_tag_value<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let s = xml.find(&open)? + open.len();
    let e = s + xml[s..].find(&close)?;
    Some(xml[s..e].trim())
}

fn unescape_xml_text(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn decode_base64_str(input: &str) -> Option<String> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u8;
    for b in input.bytes() {
        let val = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        };
        buf = (buf << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xFF) as u8);
        }
    }
    String::from_utf8(out).ok()
}

fn extract_xar_name(xml: &str) -> Option<String> {
    let s = xml.find("<name")?;
    let after = xml.as_bytes().get(s + 5).copied()?;
    if after != b'>' && after != b' ' {
        return None;
    }
    let gt = s + xml[s..].find('>')?;
    let tag_header = &xml[s..=gt];
    let body_start = gt + 1;
    let body_end = body_start + xml[body_start..].find("</name>")?;
    let inner = xml[body_start..body_end].trim();
    if tag_header.contains("base64") {
        decode_base64_str(inner)
    } else {
        Some(unescape_xml_text(inner))
    }
}

fn decode_xar_payload(
    style: &str,
    slice: &[u8],
    size: usize,
    remaining_budget: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    let decoded = if style.contains("gzip") || style.contains("zlib") {
        let z = flate2::read::ZlibDecoder::new(slice);
        read_decompressed_bounded_exact(z, size, remaining_budget)?
    } else if style.contains("bzip2") {
        let bz = super::single_stream::LibarchiveRawFilterReader::new(std::io::Cursor::new(
            slice.to_vec(),
        ))?;
        read_decompressed_bounded_exact(bz, size, remaining_budget)?
    } else if style.contains("octet-stream") || style == "none" {
        if slice.len() != size {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        slice.to_vec()
    } else {
        return Err(TTZipStatus::ErrUnsupportedFeature);
    };

    Ok(decoded)
}

fn decode_xar_data_extent(
    data_xml: &str,
    heap: &[u8],
    remaining_budget: &mut usize,
) -> Result<Vec<u8>, TTZipStatus> {
    let offset: usize = extract_xml_tag_value(data_xml, "offset")
        .and_then(|v| v.parse().ok())
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    let length: usize = extract_xml_tag_value(data_xml, "length")
        .and_then(|v| v.parse().ok())
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    let size: usize = extract_xml_tag_value(data_xml, "size")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    if offset.saturating_add(length) > heap.len() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    if size > MAX_TRANSCODE_CONTAINER_BYTES as usize || size > *remaining_budget {
        return Err(TTZipStatus::ErrOutOfMemory);
    }
    let slice = &heap[offset..offset + length];
    if size == 0 && length == 0 {
        return Ok(Vec::new());
    }

    let style = data_xml
        .find("style=\"")
        .and_then(|i| {
            let rest = &data_xml[i + 7..];
            rest.find('"').map(|e| &rest[..e])
        })
        .unwrap_or("application/octet-stream");

    let decoded = decode_xar_payload(style, slice, size, *remaining_budget)?;

    *remaining_budget = remaining_budget.saturating_sub(decoded.len());
    Ok(decoded)
}

fn partition_xar_file_body(file_body: &str) -> Result<(String, Vec<(usize, usize)>), TTZipStatus> {
    let mut own_props = String::with_capacity(file_body.len().min(1024));
    let mut child_ranges = Vec::new();
    let mut scan = 0usize;
    while let Some((c_open, c_body_start)) = find_next_file_open_tag(file_body, scan) {
        let c_end = find_matching_file_close_tag(file_body, c_body_start)
            .ok_or(TTZipStatus::ErrCorruptHeader)?;
        own_props.push_str(&file_body[scan..c_open]);
        child_ranges.push((c_open, c_end + 7));
        scan = c_end + 7;
    }
    own_props.push_str(&file_body[scan..]);
    Ok((own_props, child_ranges))
}

fn emit_xar_node_record(
    own_props: &str,
    full_path: &str,
    heap: &[u8],
    ustar_out: &mut Vec<u8>,
    remaining_budget: &mut usize,
) -> Result<(), TTZipStatus> {
    let ftype = extract_xml_tag_value(own_props, "type").unwrap_or("file");
    let is_dir = ftype == "directory";
    let mode = extract_xml_tag_value(own_props, "mode")
        .and_then(|m| u32::from_str_radix(m, 8).ok())
        .unwrap_or(if is_dir { 0o755 } else { 0o644 });
    let symlink_target = (ftype == "symlink")
        .then(|| extract_xml_tag_value(own_props, "target").map(unescape_xml_text))
        .flatten();
    let payload = match (
        !is_dir && symlink_target.is_none(),
        extract_xml_tag_value(own_props, "data"),
    ) {
        (true, Some(data_xml)) => decode_xar_data_extent(data_xml, heap, remaining_budget)?,
        _ => Vec::new(),
    };
    append_ustar_record(
        ustar_out,
        &UstarRecord {
            path: full_path,
            mode,
            mtime: 0,
            is_dir,
            symlink_target: symlink_target.as_deref(),
            payload: &payload,
        },
    )
}

fn parse_xar_file_nodes(
    xml_scope: &str,
    parent_path: &str,
    heap: &[u8],
    ustar_out: &mut Vec<u8>,
    entry_count: &mut usize,
    remaining_budget: &mut usize,
) -> Result<(), TTZipStatus> {
    let mut cursor = 0usize;
    while let Some((_, body_start)) = find_next_file_open_tag(xml_scope, cursor) {
        let body_end = find_matching_file_close_tag(xml_scope, body_start)
            .ok_or(TTZipStatus::ErrCorruptHeader)?;
        let file_body = &xml_scope[body_start..body_end];
        cursor = body_end + 7;

        let (own_props, child_ranges) = partition_xar_file_body(file_body)?;
        let name = extract_xar_name(&own_props).unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let full_path = if parent_path.is_empty() {
            name
        } else {
            format!("{}/{}", parent_path, name)
        };

        emit_xar_node_record(&own_props, &full_path, heap, ustar_out, remaining_budget)?;
        *entry_count += 1;

        for (cs, ce) in child_ranges {
            parse_xar_file_nodes(
                &file_body[cs..ce],
                &full_path,
                heap,
                ustar_out,
                entry_count,
                remaining_budget,
            )?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. Uncompressed WIM (`MSWIM\0\0\0`) Pure Safe Rust Transcoder
// ---------------------------------------------------------------------------

fn read_reshdr_short(buf: &[u8]) -> Option<(usize, u8, usize, usize)> {
    if buf.len() < 24 {
        return None;
    }
    let size_and_flags = u64::from_le_bytes(buf[0..8].try_into().ok()?);
    let comp_size = (size_and_flags & 0x00FF_FFFF_FFFF_FFFF) as usize;
    let flags = (size_and_flags >> 56) as u8;
    let offset = u64::from_le_bytes(buf[8..16].try_into().ok()?) as usize;
    let orig_size = u64::from_le_bytes(buf[16..24].try_into().ok()?) as usize;
    Some((comp_size, flags, offset, orig_size))
}

type WimResources<'a> = HashMap<[u8; 20], &'a [u8]>;

fn parse_wim_offset_table<'a>(
    raw: &'a [u8],
    ot_bytes: &'a [u8],
) -> Result<(WimResources<'a>, &'a [u8]), TTZipStatus> {
    let mut hash_map: WimResources<'a> = HashMap::new();
    let mut meta_slice: Option<&'a [u8]> = None;
    let mut p = 0usize;
    while p + 50 <= ot_bytes.len() {
        let (c_sz, r_flags, r_off, o_sz) =
            read_reshdr_short(&ot_bytes[p..p + 24]).ok_or(TTZipStatus::ErrCorruptHeader)?;
        if (r_flags & 0x04) != 0 || c_sz != o_sz {
            return Err(TTZipStatus::ErrUnsupportedFeature);
        }
        if r_off.saturating_add(c_sz) > raw.len() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let mut sha1 = [0u8; 20];
        sha1.copy_from_slice(&ot_bytes[p + 30..p + 50]);
        let res_data = &raw[r_off..r_off + c_sz];
        if (r_flags & 0x02) != 0 && meta_slice.is_none() {
            meta_slice = Some(res_data);
        } else {
            hash_map.insert(sha1, res_data);
        }
        p += 50;
    }
    let meta = meta_slice.ok_or(TTZipStatus::ErrCorruptHeader)?;
    Ok((hash_map, meta))
}

pub(super) fn transcode_wim_bytes(raw: &[u8]) -> Result<TranscodedContainerStream, TTZipStatus> {
    if raw.len() < 208 || &raw[..8] != b"MSWIM\0\0\0" {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let cb_size = u32::from_le_bytes(raw[8..12].try_into().unwrap());
    let dw_flags = u32::from_le_bytes(raw[16..20].try_into().unwrap());
    let total_parts = u16::from_le_bytes([raw[42], raw[43]]);
    if cb_size != 208 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    if (dw_flags & 0x000E_0000) != 0 || total_parts > 1 {
        return Err(TTZipStatus::ErrUnsupportedFeature);
    }

    let (ot_size, _, ot_offset, _) =
        read_reshdr_short(&raw[48..72]).ok_or(TTZipStatus::ErrCorruptHeader)?;
    if ot_offset.saturating_add(ot_size) > raw.len() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let (hash_map, meta) = parse_wim_offset_table(raw, &raw[ot_offset..ot_offset + ot_size])?;
    if meta.len() < 8 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let sec_len = u32::from_le_bytes(meta[0..4].try_into().unwrap()) as usize;
    let root_off = (sec_len.max(8) + 7) & !7;
    if root_off >= meta.len() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let mut ustar_out = Vec::new();
    let mut entry_count = 0usize;
    walk_wim_dir_tree(
        meta,
        root_off,
        "",
        &hash_map,
        &mut ustar_out,
        &mut entry_count,
    )?;

    Ok(finalize_ustar_stream(ustar_out, entry_count))
}

struct ParsedWimDirEntry {
    li_len: usize,
    is_dir: bool,
    subdir_off: usize,
    mtime: u64,
    sha1: [u8; 20],
    name: String,
}

fn parse_wim_direntry(meta: &[u8], pos: usize) -> Result<Option<ParsedWimDirEntry>, TTZipStatus> {
    let li_len = u64::from_le_bytes(meta[pos..pos + 8].try_into().unwrap()) as usize;
    if li_len == 0 {
        return Ok(None);
    }
    if li_len < 102 || pos.saturating_add(li_len) > meta.len() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let attrs = u32::from_le_bytes(meta[pos + 8..pos + 12].try_into().unwrap());
    let subdir_off = u64::from_le_bytes(meta[pos + 16..pos + 24].try_into().unwrap()) as usize;
    let ft = u64::from_le_bytes(meta[pos + 48..pos + 56].try_into().unwrap());
    let mtime = (ft / 10_000_000).saturating_sub(11_644_473_600);
    let mut sha1 = [0u8; 20];
    sha1.copy_from_slice(&meta[pos + 64..pos + 84]);
    let fn_len = u16::from_le_bytes([meta[pos + 100], meta[pos + 101]]) as usize;
    if 102 + fn_len > li_len || !fn_len.is_multiple_of(2) {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let u16_units: Vec<u16> = meta[pos + 102..pos + 102 + fn_len]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let name = String::from_utf16(&u16_units).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    Ok(Some(ParsedWimDirEntry {
        li_len,
        is_dir: (attrs & 0x10) != 0,
        subdir_off,
        mtime,
        sha1,
        name,
    }))
}

fn walk_wim_dir_tree(
    meta: &[u8],
    mut pos: usize,
    parent_path: &str,
    hash_map: &HashMap<[u8; 20], &[u8]>,
    ustar_out: &mut Vec<u8>,
    entry_count: &mut usize,
) -> Result<(), TTZipStatus> {
    while pos + 8 <= meta.len() {
        let Some(ent) = parse_wim_direntry(meta, pos)? else {
            break;
        };
        let next_parent = if ent.name.is_empty() {
            parent_path.to_string()
        } else {
            let full = if parent_path.is_empty() {
                ent.name
            } else {
                format!("{}/{}", parent_path, ent.name)
            };
            let payload = if ent.is_dir || ent.sha1 == [0u8; 20] {
                &[][..]
            } else {
                hash_map
                    .get(&ent.sha1)
                    .copied()
                    .ok_or(TTZipStatus::ErrCorruptHeader)?
            };
            append_ustar_record(
                ustar_out,
                &UstarRecord {
                    path: &full,
                    mode: if ent.is_dir { 0o755 } else { 0o644 },
                    mtime: ent.mtime,
                    is_dir: ent.is_dir,
                    symlink_target: None,
                    payload,
                },
            )?;
            *entry_count += 1;
            full
        };

        if ent.is_dir && ent.subdir_off != 0 && ent.subdir_off != pos {
            walk_wim_dir_tree(
                meta,
                ent.subdir_off,
                &next_parent,
                hash_map,
                ustar_out,
                entry_count,
            )?;
        }
        pos += ent.li_len;
    }
    Ok(())
}
