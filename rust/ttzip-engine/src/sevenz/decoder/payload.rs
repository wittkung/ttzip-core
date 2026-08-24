// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

//! Full solid payload decoding with hardware AES decryption.

use crate::codecs::deflate::deflate_decompress;
use crate::crypto::aes256::aes256_cbc_decrypt;
use crate::crypto::sha256::sha256_7z_kdf;
use crate::sevenz::format::*;
use crate::sevenz::header::SevenZHeaderInfo;
use crate::types::TTZipStatus;

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
    if info.payload_len == 0 {
        return Ok(0);
    }

    let payload_end = info.payload_offset + info.payload_len;
    if payload_end > mapped.len() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let mut raw_payload = &mapped[info.payload_offset..payload_end];
    let mut decrypted_storage = Vec::new();

    // 1. Hardware AES-256-CBC decryption via ARM64 NEON if encrypted
    if info.is_encrypted {
        let pass = password.ok_or(TTZipStatus::ErrInvalidPassword)?;
        if pass.is_empty() {
            return Err(TTZipStatus::ErrInvalidPassword);
        }

        let key = sha256_7z_kdf(pass, &info.aes_salt[..info.aes_salt_len], info.aes_num_cycles_power);

        if !raw_payload.len().is_multiple_of(16) {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        decrypted_storage.resize(raw_payload.len(), 0);
        aes256_cbc_decrypt(&key, &info.aes_iv, raw_payload, &mut decrypted_storage)
            .map_err(|_| TTZipStatus::ErrInvalidPassword)?;

        raw_payload = &decrypted_storage;
    }

    let expected_unpack_size: u64 = if !info.stream_sizes.is_empty() {
        info.stream_sizes.iter().sum()
    } else if !info.folders.is_empty() {
        info.folders.iter().map(|f| f.unpack_sizes.iter().sum::<u64>()).sum()
    } else {
        raw_payload.len() as u64
    };

    let mut total_decompressed: u64 = 0;
    const MAX_RSS_BUFFER: u64 = 64 * 1024 * 1024; // 64MB bounded RSS budget

    if expected_unpack_size <= MAX_RSS_BUFFER {
        let mut unpack_buf = vec![0u8; expected_unpack_size as usize];
        let decomp_len = match info.primary_method_id {
            METHOD_COPY => {
                let copy_len = raw_payload.len().min(unpack_buf.len());
                unpack_buf[..copy_len].copy_from_slice(&raw_payload[..copy_len]);
                copy_len
            }
            METHOD_DEFLATE => deflate_decompress(raw_payload, &mut unpack_buf)?,
            METHOD_LZMA | METHOD_LZMA2 | _ => crate::codecs::lzma2::fl2_decompress(raw_payload, &mut unpack_buf, threads)?,
        };
        unpack_buf.truncate(decomp_len);

        const CHUNK_SIZE: usize = 4 * 1024 * 1024;
        for chunk in unpack_buf.chunks(CHUNK_SIZE) {
            sink(chunk)?;
            total_decompressed += chunk.len() as u64;
        }
    } else {
        // Large archive (>64MB): stream through Fl2DStream in 4MB sliding chunks
        let mut dstream = crate::codecs::lzma2::Fl2DStream::new(threads.max(1))?;
        dstream.init(info.coder_props.first().copied())?;

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

    Ok(total_decompressed)
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
