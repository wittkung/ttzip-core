// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Full solid payload decoding with hardware AES decryption and 4MB micro-buffer streaming.

use crate::codecs::deflate::deflate_decompress;
use crate::crypto::aes256::aes256_cbc_decrypt;
use crate::crypto::sha256::sha256_7z_kdf;
use crate::sevenz::format::*;
use crate::sevenz::header::SevenZHeaderInfo;
use crate::types::TTZipStatus;

pub use super::mach_rss::get_current_rss_bytes;
pub use super::stream::extract_single_entry_bounded;

/// Maximum allowable uncompressed payload size allocated in contiguous memory (64MB ceiling).
pub const MAX_IN_MEMORY_DECOMPRESS_BYTES: usize = 64 * 1024 * 1024;

/// 4MB sliding-window micro-buffer chunk size for payload streaming.
pub(crate) const CHUNK_SIZE: usize = 4 * 1024 * 1024;

/// Resolves the final uncompressed output size of a 7z folder from its coder graph and unpack sizes.
pub(crate) fn folder_final_unpack_size(folder: &crate::sevenz::header::SevenZFolder) -> u64 {
    if folder.unpack_sizes.len() > 1 && !folder.bind_pairs.is_empty() {
        for (out_idx, &sz) in folder.unpack_sizes.iter().enumerate() {
            let is_bound = folder
                .bind_pairs
                .iter()
                .any(|&(_, b_out)| b_out as usize == out_idx);
            if !is_bound {
                return sz;
            }
        }
    }
    let primary_coder_idx = folder.coders.iter().position(|c| c.method_id != METHOD_AES);
    primary_coder_idx
        .and_then(|idx| folder.unpack_sizes.get(idx).copied())
        .or_else(|| folder.unpack_sizes.last().copied())
        .unwrap_or(0)
}

/// Returns `true` when the folder's coder graph needs the full coder-chain pipeline instead of
/// the single-coder streaming fast path.
///
/// The fast path (`decoder::stream`) decodes exactly one single-input non-AES coder and cannot
/// apply branch/delta filters or multi-input coders such as BCJ2. A folder qualifies for the
/// fast path only when it has a single non-AES coder; an optional AES coder decrypts the packed
/// stream in front of it and does not change the coder topology.
pub(crate) fn folder_needs_coder_chain(folder: &crate::sevenz::header::SevenZFolder) -> bool {
    let mut non_aes_coders = 0usize;
    for coder in &folder.coders {
        if coder.method_id == METHOD_AES {
            continue;
        }
        non_aes_coders += 1;
        if coder.num_in_streams > 1 {
            return true;
        }
    }
    non_aes_coders > 1
}

/// Resolves the true uncompressed size of a stream inside a folder, correcting for multi-coder
/// folders (such as external 7z `[LZMA2, AES]`) where the final output coder is not last.
pub(crate) fn resolve_folder_stream_size(
    info: &SevenZHeaderInfo,
    folder_idx: usize,
    global_stream_idx: usize,
    stream_idx_in_folder: usize,
    current_folder_offset: u64,
) -> u64 {
    if let Some(folder) = info.folders.get(folder_idx) {
        let final_unpack = folder_final_unpack_size(folder);
        let raw_last = folder.unpack_sizes.last().copied().unwrap_or(0);
        let num_streams = folder.num_unpack_streams.max(1);
        let is_last_in_folder = stream_idx_in_folder + 1 >= num_streams;
        if final_unpack != raw_last && is_last_in_folder {
            return final_unpack.saturating_sub(current_folder_offset);
        }
        if global_stream_idx < info.stream_sizes.len() {
            return info.stream_sizes[global_stream_idx];
        }
        return final_unpack.saturating_sub(current_folder_offset);
    }
    info.stream_sizes.get(global_stream_idx).copied().unwrap_or(0)
}

/// Normalizes codec bitstream decompression errors to `ErrCorruptHeader` while preserving
/// resource/feature statuses, keeping them distinct from downstream `sink` output errors.
#[inline]
pub(crate) fn map_codec_bitstream_err(err: TTZipStatus) -> TTZipStatus {
    match err {
        TTZipStatus::ErrExtractionFailed | TTZipStatus::ErrCorruptHeader => {
            TTZipStatus::ErrCorruptHeader
        }
        other => other,
    }
}

/// Maps folder decompression/integrity errors to `ErrInvalidPassword` on AES-encrypted folders
/// while preserving all sink write errors, security violations, cancellation, and resource limits.
#[inline]
pub(crate) fn map_folder_decompress_err(err: TTZipStatus, has_aes: bool) -> TTZipStatus {
    if has_aes && matches!(err, TTZipStatus::ErrCorruptHeader | TTZipStatus::ErrInvalidPassword) {
        TTZipStatus::ErrInvalidPassword
    } else {
        err
    }
}

pub(crate) use super::codec_pump::{
    emit_checked_chunk, init_lzma1_alone_decoder, pump_lzma1_stream, pump_lzma2_stream,
};

fn decode_buffered_codec_chunked<F>(
    raw_payload: &[u8],
    method_id: u64,
    expected_unpack_size: u64,
    total_decompressed: &mut u64,
    sink: &mut F,
) -> Result<(), TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    if expected_unpack_size > MAX_IN_MEMORY_DECOMPRESS_BYTES as u64 {
        return Err(TTZipStatus::ErrOutOfMemory);
    }
    let mut unpack_buf = vec![0u8; expected_unpack_size as usize];
    let decomp_len = if method_id == METHOD_DEFLATE {
        deflate_decompress(raw_payload, &mut unpack_buf).map_err(map_codec_bitstream_err)?
    } else {
        crate::codecs::bzip2::bzip2_decompress(raw_payload, &mut unpack_buf)
            .map_err(map_codec_bitstream_err)?
    };
    unpack_buf.truncate(decomp_len);
    for chunk in unpack_buf.chunks(CHUNK_SIZE) {
        emit_checked_chunk(chunk, total_decompressed, expected_unpack_size, sink)?;
    }
    Ok(())
}

fn decode_reader_codec_chunked<R: std::io::Read, F>(
    reader: &mut R,
    bound_to_expected: bool,
    expected_unpack_size: u64,
    total_decompressed: &mut u64,
    sink: &mut F,
) -> Result<(), TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    let mut out_chunk = vec![0u8; CHUNK_SIZE];
    loop {
        let max_read = if bound_to_expected && expected_unpack_size > 0 {
            let rem = expected_unpack_size.saturating_sub(*total_decompressed) as usize;
            if rem == 0 {
                break;
            }
            rem.min(out_chunk.len())
        } else {
            out_chunk.len()
        };
        let n = reader
            .read(&mut out_chunk[..max_read])
            .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
        if n == 0 {
            break;
        }
        emit_checked_chunk(&out_chunk[..n], total_decompressed, expected_unpack_size, sink)?;
    }
    Ok(())
}

fn decode_lzma2_raw_chunked<F>(
    raw_payload: &[u8],
    coder_props: &[u8],
    expected_unpack_size: u64,
    threads: u32,
    total_decompressed: &mut u64,
    sink: &mut F,
) -> Result<(), TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    if raw_payload.is_empty() {
        return if expected_unpack_size == 0 {
            Ok(())
        } else {
            Err(TTZipStatus::ErrCorruptHeader)
        };
    }
    let mut dstream = crate::codecs::lzma2::Fl2DStream::new(threads.max(1))?;
    dstream.init(coder_props.first().copied())?;
    let mut out_chunk = vec![0u8; CHUNK_SIZE];
    let finished = pump_lzma2_stream(
        &mut dstream,
        raw_payload,
        &mut out_chunk,
        true,
        total_decompressed,
        expected_unpack_size,
        sink,
    )?;
    if !finished {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    Ok(())
}

fn decode_lzma1_raw_chunked<F>(
    raw_payload: &[u8],
    coder_props: &[u8],
    expected_unpack_size: u64,
    total_decompressed: &mut u64,
    sink: &mut F,
) -> Result<(), TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    let mut out_chunk = vec![0u8; CHUNK_SIZE];
    let (mut decoder, is_end) = init_lzma1_alone_decoder(
        coder_props,
        expected_unpack_size,
        &mut out_chunk,
        total_decompressed,
        sink,
    )?;
    if !is_end {
        let finished = pump_lzma1_stream(
            &mut decoder,
            raw_payload,
            &mut out_chunk,
            true,
            total_decompressed,
            expected_unpack_size,
            sink,
        )?;
        if !finished {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
    }
    Ok(())
}

fn decode_ppmd_raw_chunked<F>(
    raw_payload: &[u8],
    coder_props: &[u8],
    expected_unpack_size: u64,
    total_decompressed: &mut u64,
    sink: &mut F,
) -> Result<(), TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    if coder_props.len() < 5 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let order = coder_props[0] as u32;
    let mem_size = u32::from_le_bytes(coder_props[1..5].try_into().unwrap_or([0; 4]));
    let mut dec = ppmd_rust::Ppmd7Decoder::new(raw_payload, order, mem_size)
        .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    decode_reader_codec_chunked(&mut dec, true, expected_unpack_size, total_decompressed, sink)
}

fn decode_lz4_raw_chunked<F>(
    raw_payload: &[u8],
    expected_unpack_size: u64,
    total_decompressed: &mut u64,
    sink: &mut F,
) -> Result<(), TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    let mut raw_lz4 = raw_payload;
    if raw_lz4.len() >= 12 {
        let magic = u32::from_le_bytes(raw_lz4[..4].try_into().unwrap_or([0; 4]));
        if magic == 0x184D2A50 {
            let comp_sz = u32::from_le_bytes(raw_lz4[8..12].try_into().unwrap_or([0; 4])) as usize;
            if 12 + comp_sz <= raw_lz4.len() {
                raw_lz4 = &raw_lz4[12..12 + comp_sz];
            }
        }
    }
    let mut dec = crate::codecs::lz4::frame::decoder::Lz4FrameDecoder::new(raw_lz4);
    decode_reader_codec_chunked(&mut dec, false, expected_unpack_size, total_decompressed, sink)
}

/// 7-Zip ZS container magic shared by the BROTLI and LZ4 coder families.
const ZS_CONTAINER_MAGIC: u32 = 0x184D2A50;

/// ZS container header of the BROTLI coder: `magic(4) | version(4) | size(4) | "BR" 0x01 0x00`.
const ZS_BROTLI_HEADER_SIZE: usize = 16;

/// Codec tag that identifies a BROTLI stream inside a 7-Zip ZS container.
const ZS_BROTLI_TAG: [u8; 4] = [0x42, 0x52, 0x01, 0x00];

/// Strips the 7-Zip ZS (`BROTLIMT`) container that wraps the BROTLI coder payload.
///
/// Layout: `magic(4) | version(4) | size(4) | "BR" 0x01 0x00 | brotli stream`, where `size`
/// counts the brotli stream only. Payloads without the container magic and codec tag are
/// returned unchanged, so a bare brotli stream keeps decoding.
fn strip_zs_brotli_container(raw_payload: &[u8]) -> &[u8] {
    if raw_payload.len() < ZS_BROTLI_HEADER_SIZE {
        return raw_payload;
    }
    let magic = u32::from_le_bytes(raw_payload[..4].try_into().unwrap_or([0; 4]));
    if magic != ZS_CONTAINER_MAGIC || raw_payload[12..16] != ZS_BROTLI_TAG {
        return raw_payload;
    }
    let stream_size = u32::from_le_bytes(raw_payload[8..12].try_into().unwrap_or([0; 4])) as usize;
    let stream_end = ZS_BROTLI_HEADER_SIZE.saturating_add(stream_size);
    if stream_end > raw_payload.len() {
        return raw_payload;
    }
    &raw_payload[ZS_BROTLI_HEADER_SIZE..stream_end]
}

fn dispatch_secondary_codec_chunked<F>(
    raw_payload: &[u8],
    method_id: u64,
    coder_props: &[u8],
    expected_unpack_size: u64,
    total: &mut u64,
    sink: &mut F,
) -> Result<(), TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    match method_id {
        METHOD_DEFLATE | METHOD_BZIP2 => {
            decode_buffered_codec_chunked(raw_payload, method_id, expected_unpack_size, total, sink)
        }
        METHOD_PPMD => decode_ppmd_raw_chunked(
            raw_payload,
            coder_props,
            expected_unpack_size,
            total,
            sink,
        ),
        METHOD_LZ4 => decode_lz4_raw_chunked(raw_payload, expected_unpack_size, total, sink),
        METHOD_BROTLI => {
            let raw_brotli = strip_zs_brotli_container(raw_payload);
            let mut dec = brotli::Decompressor::new(raw_brotli, 65536);
            decode_reader_codec_chunked(&mut dec, false, expected_unpack_size, total, sink)
        }
        _ => Err(TTZipStatus::ErrUnsupportedFeature),
    }
}

/// Decompresses raw chunked payload bytes for a specific coder method into a streaming sink.
pub(crate) fn decode_raw_payload_chunked<F>(
    raw_payload: &[u8],
    method_id: u64,
    coder_props: &[u8],
    expected_unpack_size: u64,
    threads: u32,
    mut sink: F,
) -> Result<u64, TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    let mut total: u64 = 0;

    match method_id {
        METHOD_LZMA2 => decode_lzma2_raw_chunked(
            raw_payload,
            coder_props,
            expected_unpack_size,
            threads,
            &mut total,
            &mut sink,
        )?,
        METHOD_LZMA => decode_lzma1_raw_chunked(
            raw_payload,
            coder_props,
            expected_unpack_size,
            &mut total,
            &mut sink,
        )?,
        METHOD_COPY => {
            for chunk in raw_payload.chunks(CHUNK_SIZE) {
                emit_checked_chunk(chunk, &mut total, expected_unpack_size, &mut sink)?;
            }
        }
        _ => dispatch_secondary_codec_chunked(
            raw_payload,
            method_id,
            coder_props,
            expected_unpack_size,
            &mut total,
            &mut sink,
        )?,
    }

    if expected_unpack_size > 0 && total != expected_unpack_size {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    Ok(total)
}

/// Helper function to apply in-place branch/delta filter to decompressed buffer.
pub(crate) fn apply_filter_inplace(method_id: u64, props: &[u8], buf: &mut [u8]) {
    match method_id {
        METHOD_BCJ_X86 => {
            crate::codecs::branch::bcj_x86::x86_decode(buf, 0);
        }
        METHOD_ARM64 | METHOD_ARM64_ALT => {
            crate::codecs::branch::bcj_arm64::arm64_decode(buf, 0);
        }
        METHOD_DELTA => {
            let dist = props.first().map(|&d| (d as usize) + 1).unwrap_or(1);
            for i in dist..buf.len() {
                buf[i] = buf[i].wrapping_add(buf[i - dist]);
            }
        }
        _ => {}
    }
}

fn is_branch_or_delta_filter(mid: u64) -> bool {
    mid == METHOD_BCJ_X86
        || mid == METHOD_ARM64
        || mid == METHOD_ARM64_ALT
        || mid == METHOD_DELTA
}

#[allow(clippy::too_many_arguments, reason = "Internal helper for filtered folder pipeline")]
fn decode_filtered_folder_pipeline<F>(
    raw_payload: &[u8],
    filter_coder: &crate::sevenz::header::SevenZCoder,
    comp_coder: Option<&crate::sevenz::header::SevenZCoder>,
    expected_unpack_size: u64,
    threads: u32,
    has_aes: bool,
    sink: &mut F,
) -> Result<u64, TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    let mid = comp_coder.map(|c| c.method_id).unwrap_or(METHOD_COPY);
    let props = comp_coder.map(|c| c.properties.as_slice()).unwrap_or(&[]);
    let mut decomp_buf = Vec::with_capacity(expected_unpack_size as usize);
    decode_raw_payload_chunked(
        raw_payload,
        mid,
        props,
        expected_unpack_size,
        threads,
        |chunk| {
            decomp_buf.extend_from_slice(chunk);
            Ok(())
        },
    )
    .map_err(|err| map_folder_decompress_err(err, has_aes))?;

    apply_filter_inplace(filter_coder.method_id, &filter_coder.properties, &mut decomp_buf);
    for chunk in decomp_buf.chunks(CHUNK_SIZE) {
        sink(chunk)?;
    }
    Ok(decomp_buf.len() as u64)
}

fn decrypt_folder_payload_in_memory(
    raw_payload: &[u8],
    info: &SevenZHeaderInfo,
    folder: &crate::sevenz::header::SevenZFolder,
    password: Option<&str>,
    decrypted_storage: &mut zeroize::Zeroizing<Vec<u8>>,
) -> Result<usize, TTZipStatus> {
    let pass = password.filter(|p| !p.is_empty()).ok_or(TTZipStatus::ErrInvalidPassword)?;
    let mut salt = [0u8; 16];
    let mut salt_len = info.aes_salt_len.min(16);
    salt[..salt_len].copy_from_slice(&info.aes_salt[..salt_len]);
    let mut iv = info.aes_iv;
    let mut num_cycles_power = info.aes_num_cycles_power;

    if let Some(aes_coder) = folder.coders.iter().find(|c| c.method_id == METHOD_AES) {
        let props = &aes_coder.properties;
        if !props.is_empty() {
            let b0 = props[0];
            num_cycles_power = (b0 & 0x3F) as u32;
            let b1 = if props.len() >= 2 { props[1] } else { 0 };
            let salt_size = (((b0 >> 7) & 1) + (b1 >> 4)) as usize;
            let iv_size = (((b0 >> 6) & 1) + (b1 & 0x0F)) as usize;
            let mut p_off = if props.len() >= 2 { 2 } else { 1 };
            salt_len = if salt_size > 0 && p_off + salt_size <= props.len() {
                let len = salt_size.min(16);
                salt[..len].copy_from_slice(&props[p_off..p_off + len]);
                p_off += salt_size;
                len
            } else {
                0
            };
            iv = [0u8; 16];
            if iv_size > 0 && p_off + iv_size <= props.len() {
                let copy_len = iv_size.min(16);
                iv[..copy_len].copy_from_slice(&props[p_off..p_off + copy_len]);
            }
        }
    }

    if !raw_payload.len().is_multiple_of(16) {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let key = zeroize::Zeroizing::new(sha256_7z_kdf(pass, &salt[..salt_len], num_cycles_power));
    decrypted_storage.resize(raw_payload.len(), 0);
    aes256_cbc_decrypt(&key, &iv, raw_payload, decrypted_storage)
        .map_err(|_| TTZipStatus::ErrInvalidPassword)?;

    let aes_coder_idx = folder.coders.iter().position(|c| c.method_id == METHOD_AES);
    let aes_unpack_sz = aes_coder_idx
        .and_then(|idx| folder.unpack_sizes.get(idx).copied())
        .or_else(|| folder.unpack_sizes.first().copied())
        .unwrap_or(decrypted_storage.len() as u64) as usize;
    Ok(aes_unpack_sz.min(decrypted_storage.len()))
}

fn decode_folderless_payload<F>(
    mapped: &[u8],
    info: &SevenZHeaderInfo,
    threads: u32,
    sink: F,
) -> Result<u64, TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    let exp_sz = if !info.stream_sizes.is_empty() {
        info.stream_sizes.iter().sum()
    } else {
        info.payload_len as u64
    };
    if exp_sz > MAX_IN_MEMORY_DECOMPRESS_BYTES as u64 {
        return Err(TTZipStatus::ErrOutOfMemory);
    }
    decode_raw_payload_chunked(
        &mapped[info.payload_offset..info.payload_offset + info.payload_len],
        info.primary_method_id,
        info.coder_props.as_slice(),
        exp_sz,
        threads,
        sink,
    )
}

/// Decompresses and decrypts a specific 7z folder payload in bounded streaming chunks.
pub fn decode_7z_folder_streaming<F>(
    mapped: &[u8],
    info: &SevenZHeaderInfo,
    folder_idx: usize,
    password: Option<&str>,
    threads: u32,
    mut sink: F,
) -> Result<u64, TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    let Some(folder) = info.folders.get(folder_idx) else {
        return decode_folderless_payload(mapped, info, threads, sink);
    };
    if folder.packed_len == 0 {
        return Ok(0);
    }
    let payload_end = folder.packed_offset + folder.packed_len;
    if payload_end > mapped.len() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let has_aes = folder.coders.iter().any(|c| c.method_id == METHOD_AES) || info.is_encrypted;
    let mut decrypted_storage = zeroize::Zeroizing::new(Vec::new());
    let mut raw_payload = &mapped[folder.packed_offset..payload_end];
    if has_aes {
        let valid_len =
            decrypt_folder_payload_in_memory(raw_payload, info, folder, password, &mut decrypted_storage)?;
        raw_payload = &decrypted_storage[..valid_len];
    }

    let non_aes: Vec<_> = folder.coders.iter().filter(|c| c.method_id != METHOD_AES).collect();
    if non_aes.iter().any(|c| c.method_id == METHOD_BCJ2) {
        return super::bcj2::decode_bcj2_folder_streaming(mapped, info, folder, folder_idx, threads, sink);
    }

    let expected_unpack_size = folder_final_unpack_size(folder);
    if expected_unpack_size > MAX_IN_MEMORY_DECOMPRESS_BYTES as u64 {
        return Err(TTZipStatus::ErrOutOfMemory);
    }

    if let Some(filter_coder) = non_aes.iter().find(|c| is_branch_or_delta_filter(c.method_id)) {
        let comp_coder = non_aes.iter().find(|c| !is_branch_or_delta_filter(c.method_id)).copied();
        return decode_filtered_folder_pipeline(
            raw_payload,
            filter_coder,
            comp_coder,
            expected_unpack_size,
            threads,
            has_aes,
            &mut sink,
        );
    }

    let primary_coder = non_aes.first().copied();
    let method_id = primary_coder.map(|c| c.method_id).unwrap_or(info.primary_method_id);
    let coder_props = primary_coder.map(|c| c.properties.as_slice()).unwrap_or(&info.coder_props);

    decode_raw_payload_chunked(raw_payload, method_id, coder_props, expected_unpack_size, threads, sink)
        .map_err(|err| map_folder_decompress_err(err, has_aes))
}

/// Decompresses and decrypts the 7z solid payload block in bounded streaming chunks, feeding into a sink callback.
pub fn decode_7z_solid_streaming<F>(
    mapped: &[u8],
    info: &SevenZHeaderInfo,
    password: Option<&str>,
    threads: u32,
    mut sink: F,
) -> Result<u64, TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    if !info.folders.is_empty() {
        let mut total = 0u64;
        for f in 0..info.folders.len() {
            total += decode_7z_folder_streaming(mapped, info, f, password, threads, &mut sink)?;
        }
        return Ok(total);
    }

    decode_7z_folder_streaming(mapped, info, 0, password, threads, sink)
}

/// Decompresses and decrypts the entire 7z solid payload block (bounded to 64MB for in-memory callers).
pub fn decode_7z_solid_payload(
    mapped: &[u8],
    info: &SevenZHeaderInfo,
    password: Option<&str>,
    threads: u32,
) -> Result<Vec<u8>, TTZipStatus> {
    let mut collected = Vec::new();
    decode_7z_solid_streaming(mapped, info, password, threads, |chunk| {
        if (collected.len() + chunk.len()) > MAX_IN_MEMORY_DECOMPRESS_BYTES {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        collected.extend_from_slice(chunk);
        Ok(())
    })?;
    Ok(collected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_raw_payload_chunked_brotli_passthrough() {
        let test_data = b"Brotli direct pass-through test payload without 0x184D2A50 magic framing.";
        let mut compressed = Vec::new();
        {
            let mut writer = brotli::CompressorWriter::new(&mut compressed, 4096, 6, 22);
            std::io::Write::write_all(&mut writer, test_data).unwrap();
        }

        let mut output = Vec::new();
        let total = decode_raw_payload_chunked(
            &compressed,
            METHOD_BROTLI,
            &[],
            test_data.len() as u64,
            1,
            |chunk| {
                output.extend_from_slice(chunk);
                Ok(())
            },
        )
        .expect("brotli decompression should succeed");

        assert_eq!(total, test_data.len() as u64);
        assert_eq!(output, test_data);
    }

    #[test]
    fn test_decode_raw_payload_chunked_deflate_exceeds_memory_limit() {
        let dummy_payload = [0u8; 16];
        let bomb_size = (MAX_IN_MEMORY_DECOMPRESS_BYTES as u64) + 1;
        let res = decode_raw_payload_chunked(
            &dummy_payload,
            METHOD_DEFLATE,
            &[],
            bomb_size,
            1,
            |_chunk| Ok(()),
        );
        assert_eq!(res, Err(TTZipStatus::ErrOutOfMemory));
    }

    #[test]
    fn test_decode_raw_payload_chunked_bzip2_exceeds_memory_limit() {
        let dummy_payload = [0u8; 16];
        let bomb_size = (MAX_IN_MEMORY_DECOMPRESS_BYTES as u64) + 1;
        let res = decode_raw_payload_chunked(
            &dummy_payload,
            METHOD_BZIP2,
            &[],
            bomb_size,
            1,
            |_chunk| Ok(()),
        );
        assert_eq!(res, Err(TTZipStatus::ErrOutOfMemory));
    }

    #[test]
    fn test_decode_7z_folder_streaming_single_coder_bomb_exceeds_memory_limit() {
        let dummy_payload = [0u8; 32];
        let bomb_size = (MAX_IN_MEMORY_DECOMPRESS_BYTES as u64) + 1;
        let folder = crate::sevenz::header::SevenZFolder {
            coders: vec![crate::sevenz::header::SevenZCoder {
                method_id: METHOD_COPY,
                properties: Vec::new(),
                num_in_streams: 1,
                num_out_streams: 1,
            }],
            unpack_sizes: vec![bomb_size],
            packed_offset: 0,
            packed_len: dummy_payload.len(),
            ..Default::default()
        };
        let info = SevenZHeaderInfo {
            folders: vec![folder],
            ..Default::default()
        };

        let res = decode_7z_folder_streaming(
            &dummy_payload,
            &info,
            0,
            None,
            1,
            |_chunk| Ok(()),
        );
        assert_eq!(res, Err(TTZipStatus::ErrOutOfMemory));
    }
}
