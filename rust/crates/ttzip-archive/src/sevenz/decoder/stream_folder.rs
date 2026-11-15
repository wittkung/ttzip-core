// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Streaming reader-based 7-Zip Folder decompression with AES-256-CBC and filter pipelines.

use super::codec_pump::{
    bounded_out_chunk_len, effective_lzma2_dict_prop, emit_checked_chunk,
    init_lzma1_alone_decoder, pump_lzma1_stream, pump_lzma2_stream, resolve_decoder_threads,
    with_decode_scratch_buf, StreamingFilterState,
};
use super::payload::{
    folder_final_unpack_size, is_branch_or_delta_filter, map_folder_decompress_err,
    MAX_IN_MEMORY_DECOMPRESS_BYTES,
};
use crate::sevenz::header::SevenZHeaderInfo;
use crate::types::TTZipStatus;

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

fn resolve_folder_stream_coders(
    folder: &crate::sevenz::header::SevenZFolder,
) -> Result<(Option<&crate::sevenz::header::SevenZCoder>, Option<StreamingFilterState>), TTZipStatus> {
    use crate::sevenz::format::METHOD_AES;
    let mut filter_coder = None;
    let mut comp_coder = None;
    for coder in &folder.coders {
        if coder.method_id == METHOD_AES {
            continue;
        }
        if coder.num_in_streams > 1 || coder.num_out_streams > 1 {
            return Err(TTZipStatus::ErrUnsupportedFeature);
        }
        let target = if is_branch_or_delta_filter(coder.method_id) {
            &mut filter_coder
        } else {
            &mut comp_coder
        };
        if target.replace(coder).is_some() {
            return Err(TTZipStatus::ErrUnsupportedFeature);
        }
    }
    let filter = filter_coder.map(|fc| StreamingFilterState::new(fc.method_id, &fc.properties));
    Ok((comp_coder, filter))
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
) -> Result<(FolderStreamDecoder, Option<StreamingFilterState>), TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    use crate::sevenz::format::{METHOD_COPY, METHOD_LZMA, METHOD_LZMA2};
    let (primary_coder, mut filter_state) = resolve_folder_stream_coders(folder)?;
    let method_id = primary_coder.map(|c| c.method_id).unwrap_or(info.primary_method_id);
    let coder_props = primary_coder
        .map(|c| c.properties.as_slice())
        .unwrap_or(&info.coder_props);

    let decoder = if method_id == METHOD_LZMA2 {
        let mut ds = crate::codecs::lzma2::Fl2DStream::new(resolve_decoder_threads(threads))?;
        ds.init(effective_lzma2_dict_prop(
            coder_props.first().copied(),
            expected_unpack_size,
        ))?;
        FolderStreamDecoder::Lzma2(ds)
    } else if method_id == METHOD_LZMA {
        let mut init_sink = |chunk: &[u8]| -> Result<(), TTZipStatus> {
            if let Some(ref mut fs) = filter_state {
                fs.push_chunk(chunk, sink)
            } else {
                sink(chunk)
            }
        };
        let (decoder, finished) = init_lzma1_alone_decoder(
            coder_props,
            expected_unpack_size,
            out_chunk,
            total_out,
            &mut init_sink,
        )?;
        FolderStreamDecoder::Lzma1 { decoder, finished }
    } else if method_id == METHOD_COPY {
        FolderStreamDecoder::Copy
    } else {
        return Err(TTZipStatus::ErrUnsupportedFeature);
    };
    Ok((decoder, filter_state))
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
        FolderStreamDecoder::Lzma2(dstream) => pump_lzma2_stream(
            dstream,
            plain_slice,
            out_chunk,
            is_final_input,
            total_out,
            super::codec_pump::Lzma2Work::Archive(expected_unpack_size),
            sink,
        ),
        FolderStreamDecoder::Lzma1 {
            decoder,
            finished,
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

struct FolderStreamPumpContext<'a> {
    decoder: &'a mut FolderStreamDecoder,
    filter_state: &'a mut Option<StreamingFilterState>,
    aes_state: Option<&'a FolderAesState>,
    packed_len: usize,
    expected_unpack_size: u64,
    out_chunk: &'a mut [u8],
    total_out: &'a mut u64,
}

fn run_folder_stream_loop<R: std::io::Read, F>(
    reader: &mut R,
    ctx: &mut FolderStreamPumpContext<'_>,
    sink: &mut F,
) -> Result<bool, TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    let has_aes = ctx.aes_state.is_some();
    let input_capacity = ctx.packed_len.min(super::payload::CHUNK_SIZE);
    let mut read_buf = vec![0u8; input_capacity];
    let mut dec_buf = if has_aes {
        vec![0u8; input_capacity]
    } else {
        Vec::new()
    };
    let mut remaining_packed = ctx.packed_len;
    let mut current_iv = ctx.aes_state.map(|s| s.iv).unwrap_or([0u8; 16]);
    let mut remaining_aes_unpack = ctx.aes_state.map(|s| s.unpack_rem).unwrap_or(0);
    let mut stream_finished =
        matches!(ctx.decoder, FolderStreamDecoder::Lzma1 { finished: true, .. });

    while remaining_packed > 0 && !stream_finished {
        let to_read = remaining_packed.min(read_buf.len());
        reader
            .read_exact(&mut read_buf[..to_read])
            .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
        remaining_packed -= to_read;

        let plain_slice = if let Some(state) = ctx.aes_state {
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
            ctx.decoder,
            plain_slice,
            ctx.out_chunk,
            is_final_input,
            ctx.total_out,
            ctx.expected_unpack_size,
            sink,
        )
        .map_err(|e| map_folder_decompress_err(e, has_aes))?;
        if is_final_input {
            break;
        }
    }

    Ok(stream_finished)
}

fn decode_packed_folder_from_reader<R: std::io::Read + std::io::Seek, F>(
    reader: &mut R,
    info: &SevenZHeaderInfo,
    folder: &crate::sevenz::header::SevenZFolder,
    password: Option<&str>,
    threads: u32,
    existing_allocated_bytes: u64,
    sink: &mut F,
) -> Result<u64, TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    let total_budget = MAX_IN_MEMORY_DECOMPRESS_BYTES as u64;
    let packed_bytes = folder.packed_len as u64;
    let used_after_pack = existing_allocated_bytes
        .checked_add(packed_bytes)
        .ok_or(TTZipStatus::ErrOutOfMemory)?;
    if used_after_pack > total_budget {
        return Err(TTZipStatus::ErrOutOfMemory);
    }
    let remaining_budget = total_budget - used_after_pack;
    reader
        .seek(std::io::SeekFrom::Start(folder.packed_offset as u64))
        .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    let mut packed_buf = vec![0u8; folder.packed_len];
    reader
        .read_exact(&mut packed_buf)
        .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    super::payload::decode_7z_packed_folder_slice_bounded(
        &packed_buf,
        info,
        folder,
        password,
        threads,
        Some(remaining_budget),
        sink,
    )
}

fn is_non_bcj2_folder(folder: &crate::sevenz::header::SevenZFolder) -> bool {
    !folder
        .coders
        .iter()
        .any(|c| c.method_id == crate::sevenz::format::METHOD_BCJ2)
}

fn pump_filtered_folder_stream<R: std::io::Read, F>(
    reader: &mut R,
    ctx: &mut FolderStreamPumpContext<'_>,
    sink: &mut F,
) -> Result<bool, TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    let filter_ptr = ctx.filter_state as *mut Option<StreamingFilterState>;
    let mut filtered_sink = |chunk: &[u8]| -> Result<(), TTZipStatus> {
        let filter_ref = unsafe { &mut *filter_ptr };
        match filter_ref.as_mut() {
            Some(fs) => fs.push_chunk(chunk, sink),
            None => sink(chunk),
        }
    };
    let finished = run_folder_stream_loop(reader, ctx, &mut filtered_sink)?;
    if let Some(fs) = ctx.filter_state {
        fs.finish(sink)?;
    }
    Ok(finished)
}

#[derive(Clone, Copy)]
pub(crate) struct FolderReaderStreamBounds {
    pub(crate) max_unpack_hint: Option<u64>,
    pub(crate) existing_allocated_bytes: u64,
    pub(crate) allow_reader_fallback: bool,
}

pub fn stream_7z_folder_from_reader<R: std::io::Read + std::io::Seek, F>(
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
    stream_7z_folder_from_reader_with_budget(
        reader,
        info,
        folder,
        password,
        threads,
        FolderReaderStreamBounds {
            max_unpack_hint: None,
            existing_allocated_bytes: 0,
            allow_reader_fallback: true,
        },
        sink,
    )
}

pub(crate) fn stream_7z_folder_from_reader_with_budget<R: std::io::Read + std::io::Seek, F>(
    reader: &mut R,
    info: &SevenZHeaderInfo,
    folder: &crate::sevenz::header::SevenZFolder,
    password: Option<&str>,
    threads: u32,
    bounds: FolderReaderStreamBounds,
    sink: &mut F,
) -> Result<u64, TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    let threads = resolve_decoder_threads(threads);
    let expected_unpack_size = folder_final_unpack_size(folder);
    if folder.packed_len == 0 {
        return (expected_unpack_size == 0)
            .then_some(0)
            .ok_or(TTZipStatus::ErrCorruptHeader);
    }
    reader
        .seek(std::io::SeekFrom::Start(folder.packed_offset as u64))
        .map_err(|_| TTZipStatus::ErrCorruptHeader)?;

    let aes_state = prepare_folder_aes_state(info, folder, password)?;
    let has_aes = aes_state.is_some();
    let unpack_bound = bounds.max_unpack_hint.unwrap_or(expected_unpack_size);
    let chunk_len = bounded_out_chunk_len(unpack_bound, super::payload::CHUNK_SIZE);
    let mut total_out = 0u64;
    let init_res = with_decode_scratch_buf(chunk_len, |out_chunk| {
        init_folder_stream_decoder(
            info,
            folder,
            expected_unpack_size,
            threads,
            out_chunk,
            &mut total_out,
            sink,
        )
    });
    let (mut decoder, mut filter_state) = match init_res {
        Ok(res) => res,
        Err(TTZipStatus::ErrUnsupportedFeature)
            if bounds.allow_reader_fallback && is_non_bcj2_folder(folder) =>
        {
            return decode_packed_folder_from_reader(
                reader,
                info,
                folder,
                password,
                threads,
                bounds.existing_allocated_bytes,
                sink,
            );
        }
        Err(e) => return Err(map_folder_decompress_err(e, has_aes)),
    };

    let stream_finished = with_decode_scratch_buf(chunk_len, |out_chunk| {
        let mut ctx = FolderStreamPumpContext {
            decoder: &mut decoder,
            filter_state: &mut filter_state,
            aes_state: aes_state.as_ref(),
            packed_len: folder.packed_len,
            expected_unpack_size,
            out_chunk,
            total_out: &mut total_out,
        };
        pump_filtered_folder_stream(reader, &mut ctx, sink)
    })?;

    if !stream_finished || (expected_unpack_size > 0 && total_out != expected_unpack_size) {
        return Err(map_folder_decompress_err(TTZipStatus::ErrCorruptHeader, has_aes));
    }
    Ok(total_out)
}
