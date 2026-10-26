// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! 7-Zip Solid Stream selective entry decoding with Bounded Memory and Early Termination.

use super::codec_pump::{
    emit_checked_chunk, init_lzma1_alone_decoder, pump_lzma1_stream, pump_lzma2_stream,
};
use super::payload::{
    decode_7z_folder_streaming, folder_final_unpack_size, folder_needs_coder_chain,
    map_folder_decompress_err, MAX_IN_MEMORY_DECOMPRESS_BYTES,
};
use crate::crypto::crc32::crc32_fast;
use crate::sevenz::header::{SevenZEntryLocation, SevenZHeaderInfo, SevenZSeekIndex};
use crate::types::TTZipStatus;

fn resolve_seek_entry_bounds(info: &SevenZHeaderInfo, loc: &SevenZEntryLocation) -> (u64, u64) {
    let folder_idx = loc.folder_index.unwrap_or(0);
    if let Some(folder) = info.folders.get(folder_idx) {
        let final_unpack = folder_final_unpack_size(folder);
        let raw_last = folder.unpack_sizes.last().copied().unwrap_or(0);
        if final_unpack != raw_last && final_unpack > loc.offset_in_folder {
            if let Some(range) =
                super::solid_stream::SolidFolderIndex::build(info).lookup(loc.file_index)
            {
                return (range.offset_start, range.uncompressed_size);
            }
        }
    }
    (loc.offset_in_folder, loc.uncompressed_size)
}

fn verify_extracted_entry_bytes(
    result_vec: &[u8],
    target_len: u64,
    expected_crc: Option<u32>,
    is_encrypted: bool,
) -> Result<(), TTZipStatus> {
    let corrupt_err = if is_encrypted {
        TTZipStatus::ErrInvalidPassword
    } else {
        TTZipStatus::ErrCorruptHeader
    };
    if result_vec.len() as u64 != target_len {
        return Err(corrupt_err);
    }
    if let Some(crc) = expected_crc {
        if crc32_fast(0, result_vec) != crc {
            return Err(corrupt_err);
        }
    }
    Ok(())
}

/// Decompresses and extracts a single entry from 7z solid stream with 4MB micro-buffer discarding,
/// precise target slice extraction, and early termination upon completion.
pub fn extract_single_entry_bounded(
    mapped: &[u8],
    info: &SevenZHeaderInfo,
    seek_index: &SevenZSeekIndex,
    entry_idx: usize,
    password: Option<&str>,
    max_preceding_budget_bytes: u64,
) -> Result<Vec<u8>, TTZipStatus> {
    let loc = seek_index
        .get_by_index(entry_idx)
        .ok_or(TTZipStatus::ErrInvalidOffset)?;

    let (target_start, target_len) = resolve_seek_entry_bounds(info, loc);
    if loc.is_directory || loc.is_empty_stream || target_len == 0 {
        return Ok(Vec::new());
    }

    let target_end = target_start
        .checked_add(target_len)
        .ok_or(TTZipStatus::ErrCorruptHeader)?;

    if max_preceding_budget_bytes > 0 && target_start > max_preceding_budget_bytes {
        return Err(TTZipStatus::ErrSolidBudgetExceeded);
    }
    if target_len > MAX_IN_MEMORY_DECOMPRESS_BYTES as u64 {
        return Err(TTZipStatus::ErrOutOfMemory);
    }

    let mut current_offset: u64 = 0;
    let mut result_vec = Vec::with_capacity(target_len as usize);
    let folder_idx = loc.folder_index.unwrap_or(0);
    let folder = info
        .folders
        .get(folder_idx)
        .ok_or(TTZipStatus::ErrCorruptHeader)?;

    let mut cursor = std::io::Cursor::new(mapped);
    let mut sink = |chunk: &[u8]| -> Result<(), TTZipStatus> {
        append_entry_window_chunk(
            chunk,
            &mut current_offset,
            target_start,
            target_end,
            &mut result_vec,
        )
    };

    let decode_res = match stream_7z_folder_from_reader(
        &mut cursor,
        info,
        folder,
        password,
        1,
        &mut sink,
    ) {
        Ok(_) => Ok(()),
        Err(TTZipStatus::ErrUnsupportedFeature) => decode_7z_folder_streaming(
            mapped,
            info,
            folder_idx,
            password,
            1,
            sink,
        )
        .map(|_| ()),
        Err(e) => Err(e),
    };

    match decode_res {
        Ok(_) | Err(TTZipStatus::Eof) => {}
        Err(e) => return Err(e),
    }

    verify_extracted_entry_bytes(&result_vec, target_len, loc.crc, info.is_encrypted)?;
    Ok(result_vec)
}

/// Backwards-compatible alias for bounded stream extraction.
pub use extract_single_entry_bounded as extract_entry_bytes_stream_bounded;

/// Backwards-compatible stream extraction with default unbounded budget (0 = disabled budget limit).
pub fn extract_entry_bytes_stream(
    mapped: &[u8],
    info: &SevenZHeaderInfo,
    seek_index: &SevenZSeekIndex,
    entry_idx: usize,
    password: Option<&str>,
) -> Result<Vec<u8>, TTZipStatus> {
    extract_single_entry_bounded(mapped, info, seek_index, entry_idx, password, 0)
}

fn derive_folder_aes_params(
    info: &SevenZHeaderInfo,
    folder: &crate::sevenz::header::SevenZFolder,
    password: &str,
) -> ([u8; 32], [u8; 16], u64) {
    let mut salt = [0u8; 16];
    let mut salt_len = info.aes_salt_len.min(16);
    salt[..salt_len].copy_from_slice(&info.aes_salt[..salt_len]);
    let mut iv = info.aes_iv;
    let mut num_cycles_power = info.aes_num_cycles_power;

    if let Some(aes_coder) = folder
        .coders
        .iter()
        .find(|c| c.method_id == crate::sevenz::format::METHOD_AES)
    {
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

    let key = crate::crypto::sha256::sha256_7z_kdf(password, &salt[..salt_len], num_cycles_power);
    let aes_coder_idx = folder
        .coders
        .iter()
        .position(|c| c.method_id == crate::sevenz::format::METHOD_AES);
    let aes_unpack_sz = aes_coder_idx
        .and_then(|idx| folder.unpack_sizes.get(idx).copied())
        .or_else(|| folder.unpack_sizes.first().copied())
        .unwrap_or(folder.packed_len as u64);
    (key, iv, aes_unpack_sz)
}

enum FolderStreamDecoder {
    Copy,
    Lzma2(crate::codecs::lzma2::Fl2DStream),
    Lzma1 {
        decoder: crate::codecs::lzma::LzmaAloneDecoder,
        finished: bool,
    },
}

fn init_folder_stream_decoder<F>(
    info: &SevenZHeaderInfo,
    folder: &crate::sevenz::header::SevenZFolder,
    expected_unpack_size: u64,
    threads: u32,
    out_chunk: &mut [u8],
    total_out: &mut u64,
    sink: &mut F,
) -> Result<FolderStreamDecoder, TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    use crate::sevenz::format::{METHOD_AES, METHOD_COPY, METHOD_LZMA, METHOD_LZMA2};
    if folder_needs_coder_chain(folder) {
        // Multi-coder folders (branch/delta filters, multi-input coders such as BCJ2) need the
        // coder-chain pipeline. The fast path can only decode one coder and would emit the
        // pre-filter bytes, so hand the folder to the DAG pipeline instead.
        return Err(TTZipStatus::ErrUnsupportedFeature);
    }
    let primary_coder = folder.coders.iter().find(|c| c.method_id != METHOD_AES);
    let method_id = primary_coder.map(|c| c.method_id).unwrap_or(info.primary_method_id);
    let coder_props = primary_coder
        .map(|c| c.properties.as_slice())
        .unwrap_or(&info.coder_props);

    if method_id == METHOD_LZMA2 {
        let mut ds = crate::codecs::lzma2::Fl2DStream::new(threads.max(1))?;
        ds.init(coder_props.first().copied())?;
        Ok(FolderStreamDecoder::Lzma2(ds))
    } else if method_id == METHOD_LZMA {
        let (decoder, finished) =
            init_lzma1_alone_decoder(coder_props, expected_unpack_size, out_chunk, total_out, sink)?;
        Ok(FolderStreamDecoder::Lzma1 { decoder, finished })
    } else if method_id == METHOD_COPY {
        Ok(FolderStreamDecoder::Copy)
    } else {
        Err(TTZipStatus::ErrUnsupportedFeature)
    }
}

fn decrypt_folder_chunk<'a>(
    key: &[u8; 32],
    current_iv: &mut [u8; 16],
    remaining_aes_unpack: &mut u64,
    cipher_chunk: &[u8],
    dec_buf: &'a mut [u8],
) -> Result<&'a [u8], TTZipStatus> {
    let to_read = cipher_chunk.len();
    let next_iv: [u8; 16] = cipher_chunk[to_read - 16..to_read].try_into().unwrap_or([0u8; 16]);
    crate::crypto::aes256::aes256_cbc_decrypt(key, current_iv, cipher_chunk, &mut dec_buf[..to_read])
        .map_err(|_| TTZipStatus::ErrInvalidPassword)?;
    *current_iv = next_iv;
    let valid_len = (to_read as u64).min(*remaining_aes_unpack) as usize;
    *remaining_aes_unpack = remaining_aes_unpack.saturating_sub(valid_len as u64);
    Ok(&dec_buf[..valid_len])
}

struct FolderAesState {
    key: zeroize::Zeroizing<[u8; 32]>,
    iv: [u8; 16],
    unpack_rem: u64,
}

fn prepare_folder_aes_state(
    info: &SevenZHeaderInfo,
    folder: &crate::sevenz::header::SevenZFolder,
    password: Option<&str>,
) -> Result<Option<FolderAesState>, TTZipStatus> {
    use crate::sevenz::format::METHOD_AES;
    let has_aes = folder.coders.iter().any(|c| c.method_id == METHOD_AES) || info.is_encrypted;
    if !has_aes {
        return Ok(None);
    }
    let pass = password.filter(|p| !p.is_empty()).ok_or(TTZipStatus::ErrInvalidPassword)?;
    if !folder.packed_len.is_multiple_of(16) {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let (key, iv, unpack_rem) = derive_folder_aes_params(info, folder, pass);
    Ok(Some(FolderAesState {
        key: zeroize::Zeroizing::new(key),
        iv,
        unpack_rem,
    }))
}

fn feed_folder_decoder_slice<F>(
    decoder: &mut FolderStreamDecoder,
    plain_slice: &[u8],
    out_chunk: &mut [u8],
    is_final_input: bool,
    total_out: &mut u64,
    expected_unpack_size: u64,
    sink: &mut F,
) -> Result<bool, TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    match decoder {
        FolderStreamDecoder::Lzma2(ref mut dstream) => pump_lzma2_stream(
            dstream,
            plain_slice,
            out_chunk,
            is_final_input,
            total_out,
            expected_unpack_size,
            sink,
        ),
        FolderStreamDecoder::Lzma1 {
            ref mut decoder,
            ref mut finished,
        } => {
            if *finished {
                return Ok(true);
            }
            let done = pump_lzma1_stream(
                decoder,
                plain_slice,
                out_chunk,
                is_final_input,
                total_out,
                expected_unpack_size,
                sink,
            )?;
            *finished = done;
            Ok(done)
        }
        FolderStreamDecoder::Copy => {
            emit_checked_chunk(plain_slice, total_out, expected_unpack_size, sink)?;
            Ok(is_final_input)
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Internal streaming reader loop binds IO buffers, AES state, and decoder state without heap wrapper overhead"
)]
fn run_folder_stream_loop<R: std::io::Read, F>(
    reader: &mut R,
    decoder: &mut FolderStreamDecoder,
    aes_state: Option<&FolderAesState>,
    packed_len: usize,
    expected_unpack_size: u64,
    out_chunk: &mut [u8],
    total_out: &mut u64,
    sink: &mut F,
) -> Result<bool, TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    let has_aes = aes_state.is_some();
    // A small packed folder needs only its actual input length. The validated AES packed
    // length and CHUNK_SIZE are both block-aligned, so the CBC chunk boundaries are unchanged.
    let input_capacity = packed_len.min(super::payload::CHUNK_SIZE);
    let mut read_buf = vec![0u8; input_capacity];
    let mut dec_buf = if has_aes {
        vec![0u8; input_capacity]
    } else {
        Vec::new()
    };
    let mut remaining_packed = packed_len;
    let mut current_iv = aes_state.map(|s| s.iv).unwrap_or([0u8; 16]);
    let mut remaining_aes_unpack = aes_state.map(|s| s.unpack_rem).unwrap_or(0);
    let mut stream_finished = matches!(decoder, FolderStreamDecoder::Lzma1 { finished: true, .. });

    while remaining_packed > 0 && !stream_finished {
        let to_read = remaining_packed.min(read_buf.len());
        reader
            .read_exact(&mut read_buf[..to_read])
            .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
        remaining_packed -= to_read;

        let plain_slice = if let Some(state) = aes_state {
            decrypt_folder_chunk(
                &state.key,
                &mut current_iv,
                &mut remaining_aes_unpack,
                &read_buf[..to_read],
                &mut dec_buf,
            )?
        } else {
            &read_buf[..to_read]
        };

        let is_final_input = remaining_packed == 0 || (has_aes && remaining_aes_unpack == 0);
        stream_finished = feed_folder_decoder_slice(
            decoder,
            plain_slice,
            out_chunk,
            is_final_input,
            total_out,
            expected_unpack_size,
            sink,
        )
        .map_err(|e| map_folder_decompress_err(e, has_aes))?;
        if is_final_input {
            break;
        }
    }

    Ok(stream_finished)
}

pub(crate) fn stream_7z_folder_from_reader<R: std::io::Read + std::io::Seek, F>(
    reader: &mut R,
    info: &SevenZHeaderInfo,
    folder: &crate::sevenz::header::SevenZFolder,
    password: Option<&str>,
    threads: u32,
    sink: &mut F,
) -> Result<u64, TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    let expected_unpack_size = folder_final_unpack_size(folder);
    if folder.packed_len == 0 {
        return if expected_unpack_size == 0 {
            Ok(0)
        } else {
            Err(TTZipStatus::ErrCorruptHeader)
        };
    }
    reader
        .seek(std::io::SeekFrom::Start(folder.packed_offset as u64))
        .map_err(|_| TTZipStatus::ErrCorruptHeader)?;

    let aes_state = prepare_folder_aes_state(info, folder, password)?;
    let has_aes = aes_state.is_some();
    let mut out_chunk = vec![0u8; super::payload::CHUNK_SIZE];
    let mut total_out = 0u64;
    let mut decoder = init_folder_stream_decoder(
        info,
        folder,
        expected_unpack_size,
        threads,
        &mut out_chunk,
        &mut total_out,
        sink,
    )
    .map_err(|e| map_folder_decompress_err(e, has_aes))?;

    let stream_finished = run_folder_stream_loop(
        reader,
        &mut decoder,
        aes_state.as_ref(),
        folder.packed_len,
        expected_unpack_size,
        &mut out_chunk,
        &mut total_out,
        sink,
    )?;

    if !stream_finished || (expected_unpack_size > 0 && total_out != expected_unpack_size) {
        return Err(map_folder_decompress_err(TTZipStatus::ErrCorruptHeader, has_aes));
    }
    Ok(total_out)
}

fn append_entry_window_chunk(
    chunk: &[u8],
    current_offset: &mut u64,
    target_start: u64,
    target_end: u64,
    result_vec: &mut Vec<u8>,
) -> Result<(), TTZipStatus> {
    let chunk_start = *current_offset;
    let chunk_end = chunk_start
        .checked_add(chunk.len() as u64)
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    *current_offset = chunk_end;
    if chunk_end <= target_start {
        return Ok(());
    }
    if chunk_start < target_end && chunk_end > target_start {
        let s = (target_start.saturating_sub(chunk_start)) as usize;
        let e = (target_end.min(chunk_end) - chunk_start) as usize;
        result_vec.extend_from_slice(&chunk[s..e]);
    }
    if *current_offset >= target_end {
        return Err(TTZipStatus::Eof);
    }
    Ok(())
}

/// Extracts a single entry into memory from a seekable 7z reader (such as `VirtualMultiVolumeReader`).
pub fn extract_7z_single_entry_from_reader<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    info: &SevenZHeaderInfo,
    seek_index: &SevenZSeekIndex,
    entry_idx: usize,
    password: Option<&str>,
) -> Result<Vec<u8>, TTZipStatus> {
    let loc = seek_index.get_by_index(entry_idx).ok_or(TTZipStatus::ErrInvalidOffset)?;
    let (target_start, target_len) = resolve_seek_entry_bounds(info, loc);
    if loc.is_directory || loc.is_empty_stream || target_len == 0 {
        return Ok(Vec::new());
    }
    if target_len > MAX_IN_MEMORY_DECOMPRESS_BYTES as u64 {
        return Err(TTZipStatus::ErrOutOfMemory);
    }
    let folder = info
        .folders
        .get(loc.folder_index.unwrap_or(0))
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    let target_end = target_start
        .checked_add(target_len)
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    let mut current_offset = 0u64;
    let mut result_vec = Vec::with_capacity(target_len as usize);
    let mut sink = |chunk: &[u8]| -> Result<(), TTZipStatus> {
        append_entry_window_chunk(chunk, &mut current_offset, target_start, target_end, &mut result_vec)
    };
    match stream_7z_folder_from_reader(reader, info, folder, password, 1, &mut sink) {
        Ok(_) | Err(TTZipStatus::Eof) => {}
        Err(e) => return Err(e),
    }
    verify_extracted_entry_bytes(&result_vec, target_len, loc.crc, info.is_encrypted)?;
    Ok(result_vec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sevenz::format::METHOD_COPY;
    use crate::sevenz::header::{
        SevenZCoder, SevenZEntryLocation, SevenZFileMeta, SevenZFolder,
    };
    use std::io::Cursor;

    fn make_truncated_no_crc_header_and_index(
        actual_payload: &[u8],
        declared_uncompressed_size: u64,
        offset_in_folder: u64,
    ) -> (SevenZHeaderInfo, SevenZSeekIndex) {
        let folder = SevenZFolder {
            coders: vec![SevenZCoder {
                method_id: METHOD_COPY,
                num_in_streams: 1,
                num_out_streams: 1,
                properties: Vec::new(),
            }],
            bind_pairs: Vec::new(),
            packed_streams: vec![0],
            unpack_sizes: vec![actual_payload.len() as u64],
            crc: None,
            num_unpack_streams: 1,
            packed_offset: 0,
            packed_len: actual_payload.len(),
        };
        let info = SevenZHeaderInfo {
            payload_offset: 0,
            payload_len: actual_payload.len(),
            pack_sizes: vec![actual_payload.len() as u64],
            folders: vec![folder],
            stream_sizes: vec![declared_uncompressed_size],
            stream_crcs: Vec::new(),
            files: vec![SevenZFileMeta {
                rel_path: "truncated_no_crc.bin".to_string(),
                is_directory: false,
                is_empty_stream: false,
                mtime_epoch_secs: None,
                mode: 0o644,
            }],
            primary_method_id: METHOD_COPY,
            coder_props: Vec::new(),
            is_encrypted: false,
            aes_salt: [0u8; 16],
            aes_salt_len: 0,
            aes_iv: [0u8; 16],
            aes_iv_len: 0,
            aes_num_cycles_power: 0,
        };
        let seek_index = SevenZSeekIndex {
            entries: vec![SevenZEntryLocation {
                file_index: 0,
                rel_path: "truncated_no_crc.bin".to_string(),
                is_directory: false,
                is_empty_stream: false,
                folder_index: Some(0),
                stream_index: Some(0),
                offset_in_folder,
                uncompressed_size: declared_uncompressed_size,
                crc: None,
            }],
        };
        (info, seek_index)
    }

    #[test]
    fn test_extract_single_entry_bounded_rejects_short_output_without_crc() {
        let short_payload = b"SHORT_STREAM_ONLY_24_BYT";
        let (info, seek_index) = make_truncated_no_crc_header_and_index(short_payload, 64, 0);
        let res = extract_single_entry_bounded(short_payload, &info, &seek_index, 0, None, 0);
        assert_eq!(res, Err(TTZipStatus::ErrCorruptHeader));
    }

    #[test]
    fn test_extract_7z_single_entry_from_reader_rejects_short_output_without_crc() {
        let short_payload = b"SHORT_STREAM_ONLY_24_BYT";
        let (info, seek_index) = make_truncated_no_crc_header_and_index(short_payload, 64, 0);
        let mut reader = Cursor::new(short_payload.as_slice());
        let res = extract_7z_single_entry_from_reader(&mut reader, &info, &seek_index, 0, None);
        assert_eq!(res, Err(TTZipStatus::ErrCorruptHeader));
    }

    #[test]
    fn test_extract_single_entry_rejects_offset_addition_overflow() {
        let payload = b"0123456789ABCDEF";
        let (info, seek_index) = make_truncated_no_crc_header_and_index(payload, 16, u64::MAX - 4);
        assert_eq!(
            extract_single_entry_bounded(payload, &info, &seek_index, 0, None, 0),
            Err(TTZipStatus::ErrCorruptHeader)
        );
        let mut reader = Cursor::new(payload.as_slice());
        assert_eq!(
            extract_7z_single_entry_from_reader(&mut reader, &info, &seek_index, 0, None),
            Err(TTZipStatus::ErrCorruptHeader)
        );
    }
}
