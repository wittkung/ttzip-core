// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Full solid payload decoding with hardware AES decryption.

use crate::codecs::deflate::deflate_decompress;
use crate::crypto::aes256::aes256_cbc_decrypt;
use crate::crypto::sha256::sha256_7z_kdf;
use crate::sevenz::format::*;
use crate::sevenz::header::SevenZHeaderInfo;
use crate::types::TTZipStatus;

/// Decompresses raw chunked payload bytes for a specific coder method into a streaming sink.
fn decode_raw_payload_chunked<F>(
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
    let mut total_decompressed: u64 = 0;
    const MAX_RSS_BUFFER: u64 = 64 * 1024 * 1024; // 64MB bounded RSS budget

    if expected_unpack_size <= MAX_RSS_BUFFER {
        let mut unpack_buf = vec![0u8; expected_unpack_size as usize];
        let decomp_len = match method_id {
            METHOD_COPY => {
                let copy_len = raw_payload.len().min(unpack_buf.len());
                unpack_buf[..copy_len].copy_from_slice(&raw_payload[..copy_len]);
                copy_len
            }
            METHOD_DEFLATE => deflate_decompress(raw_payload, &mut unpack_buf)?,
            METHOD_LZMA => crate::codecs::lzma::lzma1_decompress(
                raw_payload,
                coder_props,
                expected_unpack_size,
                &mut unpack_buf,
            )?,
            METHOD_LZMA2 => {
                crate::codecs::lzma2::fl2_decompress(raw_payload, &mut unpack_buf, threads)?
            }
            _ => return Err(TTZipStatus::ErrUnsupportedFeature),
        };
        unpack_buf.truncate(decomp_len);

        const CHUNK_SIZE: usize = 4 * 1024 * 1024;
        for chunk in unpack_buf.chunks(CHUNK_SIZE) {
            sink(chunk)?;
            total_decompressed += chunk.len() as u64;
        }
    } else {
        match method_id {
            METHOD_LZMA2 => {
                // Large archive (>64MB): stream through Fl2DStream in 4MB sliding chunks
                let mut dstream = crate::codecs::lzma2::Fl2DStream::new(threads.max(1))?;
                dstream.init(coder_props.first().copied())?;

                let mut in_buf = crate::codecs::lzma2::Fl2InBuffer {
                    src: raw_payload.as_ptr() as *const libc::c_void,
                    size: raw_payload.len(),
                    pos: 0,
                };

                const CHUNK_SIZE: usize = 4 * 1024 * 1024;
                let mut out_chunk = vec![0u8; CHUNK_SIZE];
                while in_buf.pos < in_buf.size {
                    let mut out_buf = crate::codecs::lzma2::Fl2OutBuffer {
                        dst: out_chunk.as_mut_ptr() as *mut libc::c_void,
                        size: out_chunk.len(),
                        pos: 0,
                    };

                    let remaining = dstream.decompress_stream(&mut in_buf, &mut out_buf)?;
                    let produced = out_buf.pos;
                    if produced > 0 {
                        sink(&out_chunk[..produced])?;
                        total_decompressed += produced as u64;
                    }

                    if remaining == 0 && in_buf.pos >= in_buf.size {
                        break;
                    }
                    if produced == 0 && in_buf.pos == 0 {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                }
            }
            METHOD_LZMA => {
                if coder_props.len() < 5 {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
                let mut alone_input = Vec::with_capacity(13 + raw_payload.len());
                alone_input.extend_from_slice(&coder_props[..5]);
                alone_input.extend_from_slice(&expected_unpack_size.to_le_bytes());
                alone_input.extend_from_slice(raw_payload);

                let mut decoder = crate::codecs::lzma::LzmaAloneDecoder::new()?;
                let mut in_offset = 0usize;
                const CHUNK_SIZE: usize = 4 * 1024 * 1024;
                let mut out_chunk = vec![0u8; CHUNK_SIZE];

                while in_offset < alone_input.len() {
                    let (consumed, produced, is_end) = decoder.decompress_chunk(
                        &alone_input[in_offset..],
                        &mut out_chunk,
                        true,
                    )?;
                    in_offset += consumed;
                    if produced > 0 {
                        sink(&out_chunk[..produced])?;
                        total_decompressed += produced as u64;
                    }
                    if is_end || (consumed == 0 && produced == 0) {
                        break;
                    }
                }
            }
            METHOD_COPY => {
                const CHUNK_SIZE: usize = 4 * 1024 * 1024;
                for chunk in raw_payload.chunks(CHUNK_SIZE) {
                    sink(chunk)?;
                    total_decompressed += chunk.len() as u64;
                }
            }
            METHOD_DEFLATE => {
                let mut unpack_buf = vec![0u8; expected_unpack_size as usize];
                let decomp_len = deflate_decompress(raw_payload, &mut unpack_buf)?;
                unpack_buf.truncate(decomp_len);
                const CHUNK_SIZE: usize = 4 * 1024 * 1024;
                for chunk in unpack_buf.chunks(CHUNK_SIZE) {
                    sink(chunk)?;
                    total_decompressed += chunk.len() as u64;
                }
            }
            _ => return Err(TTZipStatus::ErrUnsupportedFeature),
        }
    }

    Ok(total_decompressed)
}

/// Decompresses and decrypts a specific 7z folder payload in bounded streaming chunks.
pub fn decode_7z_folder_streaming<F>(
    mapped: &[u8],
    info: &SevenZHeaderInfo,
    folder_idx: usize,
    password: Option<&str>,
    threads: u32,
    sink: F,
) -> Result<u64, TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    let (packed_offset, packed_len, method_id, coder_props, expected_unpack_size) = match info.folders.get(folder_idx) {
        Some(folder) => {
            let mid = folder
                .coders
                .first()
                .map(|c| c.method_id)
                .unwrap_or(info.primary_method_id);
            let props = folder
                .coders
                .first()
                .map(|c| c.properties.as_slice())
                .unwrap_or(&info.coder_props);
            let exp_sz = folder
                .unpack_sizes
                .last()
                .copied()
                .unwrap_or(0);
            (folder.packed_offset, folder.packed_len, mid, props, exp_sz)
        }
        None => {
            let exp_sz = if !info.stream_sizes.is_empty() {
                info.stream_sizes.iter().sum()
            } else {
                info.payload_len as u64
            };
            (info.payload_offset, info.payload_len, info.primary_method_id, info.coder_props.as_slice(), exp_sz)
        }
    };

    if packed_len == 0 {
        return Ok(0);
    }

    let payload_end = packed_offset + packed_len;
    if payload_end > mapped.len() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let mut raw_payload = &mapped[packed_offset..payload_end];
    let mut decrypted_storage = zeroize::Zeroizing::new(Vec::new());

    if info.is_encrypted || method_id == METHOD_AES {
        let pass = password.ok_or(TTZipStatus::ErrInvalidPassword)?;
        if pass.is_empty() {
            return Err(TTZipStatus::ErrInvalidPassword);
        }

        let key = zeroize::Zeroizing::new(sha256_7z_kdf(pass, &info.aes_salt[..info.aes_salt_len], info.aes_num_cycles_power));

        if !raw_payload.len().is_multiple_of(16) {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        decrypted_storage.resize(raw_payload.len(), 0);
        aes256_cbc_decrypt(&key, &info.aes_iv, raw_payload, &mut decrypted_storage)
            .map_err(|_| TTZipStatus::ErrInvalidPassword)?;

        raw_payload = &decrypted_storage;
    }

    let effective_unpack_size = if expected_unpack_size > 0 {
        expected_unpack_size
    } else {
        raw_payload.len() as u64
    };

    decode_raw_payload_chunked(raw_payload, method_id, coder_props, effective_unpack_size, threads, sink)
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
        if collected.len() + chunk.len() > 64 * 1024 * 1024 {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        collected.extend_from_slice(chunk);
        Ok(())
    })?;
    Ok(collected)
}
