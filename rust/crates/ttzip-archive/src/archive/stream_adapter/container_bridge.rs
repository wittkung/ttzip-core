// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Native Safe Rust container classification and USTAR stream bridge for external formats:
//! - Apple Archive (`pbze` Parallel LZFSE + `AA01` / `YAA1` records; `AEA1` -> `ErrUnsupportedFeature`)
//! - eXtensible ARchive (`xar!` zlib TOC + heap extents, independent of `libxml2`)
//! - Windows Imaging Format (`MSWIM\0\0\0` uncompressed WIM; compressed WIM -> `ErrUnsupportedFeature`)
//! - Apple UDIF Disk Image (`koly` trailer -> `ErrUnsupportedFeature`; corrupt `.dmg` -> `ErrCorruptHeader`)

use std::fs::File;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::single_stream::strip_split_suffix_preserve_case;
use super::xar_wim_bridge::{transcode_wim_bytes, transcode_xar_bytes};
use crate::archive::split::VirtualMultiVolumeReader;
use crate::tar::header::TarHeader;
use crate::tar::types::TarEntryType;
use crate::types::TTZipStatus;

pub(super) const MAX_TRANSCODE_CONTAINER_BYTES: u64 = 64 * 1024 * 1024;

/// In-memory USTAR stream produced by transcoding a validated external container.
pub struct TranscodedContainerStream {
    pub cursor: Cursor<Vec<u8>>,
    pub entry_count: usize,
}

#[derive(Debug)]
pub(super) struct UstarRecord<'a> {
    pub path: &'a str,
    pub mode: u32,
    pub mtime: u64,
    pub is_dir: bool,
    pub symlink_target: Option<&'a str>,
    pub payload: &'a [u8],
}

pub(super) fn append_ustar_record(
    out: &mut Vec<u8>,
    rec: &UstarRecord<'_>,
) -> Result<(), TTZipStatus> {
    let clean = rec.path.trim_start_matches("./").trim_start_matches('/');
    if clean.is_empty() || clean == "." {
        return Ok(());
    }
    let mut name_str = clean.to_string();
    if rec.is_dir && !name_str.ends_with('/') {
        name_str.push('/');
    }

    let mut hdr = TarHeader::new();
    hdr.set_ustar_magic();
    if name_str.len() <= 100 {
        hdr.set_name(&name_str);
    } else if let Some((prefix, base)) = split_ustar_path(&name_str) {
        hdr.set_prefix(prefix);
        hdr.set_name(base);
    } else {
        return Err(TTZipStatus::ErrPathTooLong);
    }

    hdr.set_mode(rec.mode & 0o7777);
    hdr.set_uid(0);
    hdr.set_gid(0);
    hdr.set_mtime(rec.mtime);

    let padded_payload_bytes = if rec.is_dir || rec.symlink_target.is_some() {
        0usize
    } else {
        rec.payload.len().saturating_add(511) & !511
    };
    let next_total = out
        .len()
        .saturating_add(512)
        .saturating_add(padded_payload_bytes)
        .saturating_add(1024);
    if next_total > MAX_TRANSCODE_CONTAINER_BYTES as usize {
        return Err(TTZipStatus::ErrOutOfMemory);
    }

    if let Some(target) = rec.symlink_target {
        hdr.set_entry_type(TarEntryType::Symlink);
        hdr.set_size(0);
        hdr.set_linkname(target);
        hdr.update_checksum();
        out.extend_from_slice(hdr.as_bytes());
    } else if rec.is_dir {
        hdr.set_entry_type(TarEntryType::Directory);
        hdr.set_size(0);
        hdr.update_checksum();
        out.extend_from_slice(hdr.as_bytes());
    } else {
        hdr.set_entry_type(TarEntryType::Regular);
        hdr.set_size(rec.payload.len() as u64);
        hdr.update_checksum();
        out.extend_from_slice(hdr.as_bytes());
        out.extend_from_slice(rec.payload);
        let rem = rec.payload.len() % 512;
        if rem != 0 {
            out.resize(out.len() + (512 - rem), 0);
        }
    }
    Ok(())
}

fn split_ustar_path(path: &str) -> Option<(&str, &str)> {
    for (idx, ch) in path.char_indices().rev() {
        if ch == '/' && idx <= 155 && (path.len() - idx - 1) <= 100 {
            return Some((&path[..idx], &path[idx + 1..]));
        }
    }
    None
}

pub(super) fn finalize_ustar_stream(
    mut out: Vec<u8>,
    entry_count: usize,
) -> TranscodedContainerStream {
    out.resize(out.len() + 1024, 0);
    TranscodedContainerStream {
        cursor: Cursor::new(out),
        entry_count,
    }
}

fn has_valid_udif_koly_trailer<R: Read + Seek>(reader: &mut R, total_len: u64) -> bool {
    if total_len < 512 || reader.seek(SeekFrom::End(-512)).is_err() {
        return false;
    }
    let mut trailer = [0u8; 16];
    if reader.read_exact(&mut trailer).is_err() || &trailer[..4] != b"koly" {
        return false;
    }
    let version = u32::from_be_bytes([trailer[4], trailer[5], trailer[6], trailer[7]]);
    let header_size = u32::from_be_bytes([trailer[8], trailer[9], trailer[10], trailer[11]]);
    version == 4 && header_size == 512
}

fn read_bounded_bytes<R: Read>(reader: &mut R, total_len: u64) -> Result<Vec<u8>, TTZipStatus> {
    if total_len > MAX_TRANSCODE_CONTAINER_BYTES {
        return Err(TTZipStatus::ErrOutOfMemory);
    }
    let max_bytes = MAX_TRANSCODE_CONTAINER_BYTES as usize;
    let mut limited = reader.take(MAX_TRANSCODE_CONTAINER_BYTES.saturating_add(1));
    let mut buf = Vec::with_capacity((total_len as usize).min(65536));
    let mut chunk = [0u8; 65536];
    loop {
        match limited.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                if buf.len().saturating_add(n) > max_bytes {
                    return Err(TTZipStatus::ErrOutOfMemory);
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(TTZipStatus::ErrOpenFailed),
        }
    }
    Ok(buf)
}

/// Checks if `archive_path` / `volume_chain` is an external container requiring native bridging
/// or explicit `ErrUnsupportedFeature` / `ErrCorruptHeader` classification.
pub fn try_transcode_special_container(
    archive_path: &Path,
    volume_chain: &[PathBuf],
) -> Result<Option<TranscodedContainerStream>, TTZipStatus> {
    let raw_name = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let base_lower = strip_split_suffix_preserve_case(&raw_name);

    if volume_chain.len() > 1 {
        let mut vr = VirtualMultiVolumeReader::from_volumes(volume_chain.to_vec())
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let total_len = vr.total_size();
        return inspect_and_transcode_reader(&mut vr, total_len, base_lower);
    }

    let first_path = volume_chain.first().map(|p| p.as_path()).unwrap_or(archive_path);
    let mut f = File::open(first_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let total_len = f.metadata().map(|m| m.len()).unwrap_or(0);
    inspect_and_transcode_reader(&mut f, total_len, base_lower)
}

fn check_unsupported_container_format<R: Read + Seek>(
    reader: &mut R,
    total_len: u64,
    head_slice: &[u8],
    base_lower: &str,
) -> Result<(), TTZipStatus> {
    if has_valid_udif_koly_trailer(reader, total_len) {
        return Err(TTZipStatus::ErrUnsupportedFeature);
    }
    let _ = reader.seek(SeekFrom::Start(0));
    if base_lower.ends_with(".dmg") {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    if head_slice.starts_with(b"AEA1") || head_slice.starts_with(b"hsqs") {
        return Err(TTZipStatus::ErrUnsupportedFeature);
    }
    if base_lower.ends_with(".aea")
        || base_lower.ends_with(".squashfs")
        || base_lower.ends_with(".sqsh")
    {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    Ok(())
}

fn is_apple_archive_magic(head_slice: &[u8]) -> bool {
    head_slice.starts_with(b"pbze")
        || head_slice.starts_with(b"AA01")
        || head_slice.starts_with(b"YAA1")
}

fn is_wim_extension(base_lower: &str) -> bool {
    base_lower.ends_with(".wim") || base_lower.ends_with(".swm") || base_lower.ends_with(".esd")
}

fn inspect_and_transcode_reader<R: Read + Seek>(
    reader: &mut R,
    total_len: u64,
    base_lower: &str,
) -> Result<Option<TranscodedContainerStream>, TTZipStatus> {
    let mut head = [0u8; 16];
    let head_len = reader.read(&mut head).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let _ = reader.seek(SeekFrom::Start(0));
    let head_slice = &head[..head_len];

    check_unsupported_container_format(reader, total_len, head_slice, base_lower)?;

    if is_apple_archive_magic(head_slice) {
        let raw = read_bounded_bytes(reader, total_len)?;
        return transcode_aar_bytes(&raw).map(Some);
    }
    if head_slice.starts_with(b"xar!") {
        let raw = read_bounded_bytes(reader, total_len)?;
        return transcode_xar_bytes(&raw).map(Some);
    }
    if head_slice.starts_with(b"MSWIM\0\0\0") {
        let raw = read_bounded_bytes(reader, total_len)?;
        return transcode_wim_bytes(&raw).map(Some);
    }
    if base_lower.ends_with(".aar")
        || base_lower.ends_with(".xar")
        || is_wim_extension(base_lower)
    {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// 1. Apple Archive (`pbze` Parallel LZFSE + `AA01` / `YAA1`) Transcoder
// ---------------------------------------------------------------------------

fn decompress_pbze_stream(data: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    if data.len() < 28 || &data[..4] != b"pbze" {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let mut pos = 12usize;
    let mut out = Vec::new();
    while pos < data.len() {
        if pos + 16 > data.len() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let uncomp_len = u64::from_be_bytes(data[pos..pos + 8].try_into().unwrap()) as usize;
        let comp_len = u64::from_be_bytes(data[pos + 8..pos + 16].try_into().unwrap()) as usize;
        pos += 16;
        if comp_len == 0 && uncomp_len == 0 {
            break;
        }
        if pos + comp_len > data.len()
            || out.len().saturating_add(uncomp_len) > MAX_TRANSCODE_CONTAINER_BYTES as usize
        {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let block = &data[pos..pos + comp_len];
        pos += comp_len;
        if comp_len == uncomp_len && !block.starts_with(b"bvx") {
            out.extend_from_slice(block);
        } else {
            let mut decoded = vec![0u8; uncomp_len + 64];
            let n = crate::codecs::lzfse::lzfse_decompress(block, &mut decoded)
                .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
            if n != uncomp_len {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            out.extend_from_slice(&decoded[..n]);
        }
    }
    if out.is_empty() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    Ok(out)
}

#[derive(Default)]
struct YaaParsedHeader {
    typ: u8,
    path: String,
    link_target: Option<String>,
    mode: u32,
    mtime: u64,
    data_blob_len: usize,
    extra_blob_len: usize,
}

fn parse_yaa_header_fields(hdr_bytes: &[u8]) -> Result<YaaParsedHeader, TTZipStatus> {
    let mut parsed = YaaParsedHeader {
        mode: 0o644,
        ..Default::default()
    };
    let mut p = 6usize;
    while p < hdr_bytes.len() {
        if p + 4 > hdr_bytes.len() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let tag = &hdr_bytes[p..p + 4];
        p += 4;
        p = apply_yaa_field(tag, hdr_bytes, p, &mut parsed)?;
    }
    Ok(parsed)
}

fn apply_yaa_scalar_field(
    tag: &[u8],
    hdr: &[u8],
    p: usize,
    out: &mut YaaParsedHeader,
) -> Result<usize, TTZipStatus> {
    let width = match tag[3] {
        b'1' => 1,
        b'2' => 2,
        b'4' => 4,
        b'8' | b'S' => 8,
        b'T' => 12,
        _ => return Err(TTZipStatus::ErrCorruptHeader),
    };
    if p + width > hdr.len() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    if tag == b"TYP1" {
        out.typ = hdr[p];
    } else if tag == b"MOD2" {
        out.mode = u16::from_le_bytes([hdr[p], hdr[p + 1]]) as u32;
    } else if tag == b"MTMS" || tag == b"MTMT" {
        out.mtime = u64::from_le_bytes(hdr[p..p + 8].try_into().unwrap());
    }
    Ok(p + width)
}

fn apply_yaa_path_field(
    tag: &[u8],
    hdr: &[u8],
    p: usize,
    out: &mut YaaParsedHeader,
) -> Result<usize, TTZipStatus> {
    if p + 2 > hdr.len() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let slen = u16::from_le_bytes([hdr[p], hdr[p + 1]]) as usize;
    if p + 2 + slen > hdr.len() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let s = std::str::from_utf8(&hdr[p + 2..p + 2 + slen])
        .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    if tag == b"PATP" {
        out.path = s.to_string();
    } else if tag == b"LNKP" {
        out.link_target = Some(s.to_string());
    }
    Ok(p + 2 + slen)
}

fn apply_yaa_blob_field(
    tag: &[u8],
    hdr: &[u8],
    p: usize,
    out: &mut YaaParsedHeader,
) -> Result<usize, TTZipStatus> {
    let (width, len) = match tag[3] {
        b'A' if p + 2 <= hdr.len() => (2, u16::from_le_bytes([hdr[p], hdr[p + 1]]) as usize),
        b'B' if p + 4 <= hdr.len() => (4, u32::from_le_bytes(hdr[p..p + 4].try_into().unwrap()) as usize),
        b'C' if p + 8 <= hdr.len() => (8, u64::from_le_bytes(hdr[p..p + 8].try_into().unwrap()) as usize),
        _ => return Err(TTZipStatus::ErrCorruptHeader),
    };
    if &tag[..3] == b"DAT" {
        out.data_blob_len = len;
    } else {
        out.extra_blob_len += len;
    }
    Ok(p + width)
}

fn apply_yaa_field(
    tag: &[u8],
    hdr: &[u8],
    p: usize,
    out: &mut YaaParsedHeader,
) -> Result<usize, TTZipStatus> {
    match tag[3] {
        b'1' | b'2' | b'4' | b'8' | b'S' | b'T' => apply_yaa_scalar_field(tag, hdr, p, out),
        b'P' => apply_yaa_path_field(tag, hdr, p, out),
        b'A' | b'B' | b'C' => apply_yaa_blob_field(tag, hdr, p, out),
        _ => Err(TTZipStatus::ErrCorruptHeader),
    }
}

fn transcode_aar_bytes(raw: &[u8]) -> Result<TranscodedContainerStream, TTZipStatus> {
    let yaa_storage: Vec<u8>;
    let yaa = if raw.starts_with(b"pbze") {
        yaa_storage = decompress_pbze_stream(raw)?;
        yaa_storage.as_slice()
    } else {
        raw
    };

    let mut pos = 0usize;
    let mut ustar_out = Vec::new();
    let mut entry_count = 0usize;

    while pos < yaa.len() {
        if pos + 6 > yaa.len() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let magic = &yaa[pos..pos + 4];
        if magic != b"AA01" && magic != b"YAA1" {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let hdr_len = u16::from_le_bytes([yaa[pos + 4], yaa[pos + 5]]) as usize;
        if hdr_len < 6 || pos + hdr_len > yaa.len() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let parsed = parse_yaa_header_fields(&yaa[pos..pos + hdr_len])?;
        let data_start = pos + hdr_len;
        let total_blobs = parsed.data_blob_len.saturating_add(parsed.extra_blob_len);
        if data_start + total_blobs > yaa.len() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let payload = &yaa[data_start..data_start + parsed.data_blob_len];
        pos = data_start + total_blobs;

        let clean_path = parsed.path.trim_start_matches("./").trim_start_matches('/');
        if !clean_path.is_empty() && clean_path != "." {
            let is_dir = parsed.typ == b'D';
            let symlink = if parsed.typ == b'L' {
                parsed.link_target.as_deref()
            } else {
                None
            };
            append_ustar_record(
                &mut ustar_out,
                &UstarRecord {
                    path: clean_path,
                    mode: parsed.mode,
                    mtime: parsed.mtime,
                    is_dir,
                    symlink_target: symlink,
                    payload,
                },
            )?;
            entry_count += 1;
        }
    }

    Ok(finalize_ustar_stream(ustar_out, entry_count))
}

// ---------------------------------------------------------------------------
// 2. Unified 7z Seekable Single-Entry & Selected-Entry Extraction
// ---------------------------------------------------------------------------

use crate::archive::unified::selection::EntrySelection;
use crate::types::TTZipExtractOptions;

/// Extracts a single 7z entry into memory from any `Read + Seek` reader (single-volume or multi-volume).
///
/// This is the in-memory track: the whole entry must fit in one `Vec<u8>`, and entries larger than
/// the 64 MiB `MAX_IN_MEMORY_DECOMPRESS_BYTES` budget are rejected with `ErrOutOfMemory` by the
/// decoder. Callers that need to materialize arbitrarily large entries must use the streaming
/// disk track ([`extract_sevenz_selected_from_reader`]) instead of relaxing this limit.
pub fn extract_7z_single_entry_from_seekable<R: Read + Seek>(
    reader: &mut R,
    entry_path: Option<&str>,
    entry_index: i64,
    password: Option<&str>,
) -> Result<Vec<u8>, TTZipStatus> {
    let info = decode_sevenz_header_info(reader, password)?;
    let seek_index = crate::sevenz::header::SevenZSeekIndex::build(&info);
    let target_idx = if entry_index >= 0 {
        entry_index as usize
    } else if let Some(target_p) = entry_path {
        seek_index
            .get_by_path(target_p)
            .map(|loc| loc.file_index)
            .ok_or(TTZipStatus::ErrFileNotFound)?
    } else {
        return Err(TTZipStatus::ErrInvalidParam);
    };
    crate::sevenz::decoder::archive::extract_7z_single_entry_from_reader(
        reader,
        &info,
        &seek_index,
        target_idx,
        password,
    )
}

fn decode_sevenz_header_info<R: Read + Seek>(
    reader: &mut R,
    password: Option<&str>,
) -> Result<crate::sevenz::header::SevenZHeaderInfo, TTZipStatus> {
    crate::sevenz::encrypted_header::EncodedHeaderDecoder::default()
        .decode_from_reader(reader, password)
        .map_err(|e| match e {
            crate::sevenz::dag::SevenZError::BadPassword => TTZipStatus::ErrInvalidPassword,
            _ => TTZipStatus::ErrCorruptHeader,
        })
}

/// Extracts selected entries from a 7z archive (`Read + Seek`, single-volume or multi-volume)
/// into `staging` using `DirJail`, `InFlightJailFile`, and streaming chunk dispatch.
///
/// This is the streaming disk track and the production default entry for selective extraction:
/// it has no per-entry size limit, and it always builds `ExtractionQuotaGuard::default_limits()`
/// so decoded bytes (discarded and written) are charged against the decompression budget.
pub fn extract_sevenz_selected_from_reader<R: Read + Seek>(
    reader: &mut R,
    selection: &EntrySelection,
    staging: &crate::fs::collision::staging::StagingRoot,
    options: &TTZipExtractOptions,
    password: Option<&str>,
) -> Result<usize, TTZipStatus> {
    super::sevenz_selected::extract_sevenz_selected_from_reader(
        reader, selection, staging, options, password,
    )
}

/// Extracts selected entries from a 7z archive with an explicit extraction quota guard.
///
/// `None` selects `ExtractionQuotaGuard::default_limits()`. The guard's compressed denominator is
/// metered from each decoded folder's declared `packed_len`, registered once before that folder
/// starts decoding; the numerator counts every decoded byte, including discarded solid-block
/// prefixes.
pub fn extract_sevenz_selected_from_reader_with_quota<R: Read + Seek>(
    reader: &mut R,
    selection: &EntrySelection,
    staging: &crate::fs::collision::staging::StagingRoot,
    options: &TTZipExtractOptions,
    password: Option<&str>,
    quota_guard: Option<crate::security::zip_defense::ExtractionQuotaGuard>,
) -> Result<usize, TTZipStatus> {
    super::sevenz_selected::extract_sevenz_selected_from_reader_with_quota(
        reader, selection, staging, options, password, quota_guard,
    )
}
