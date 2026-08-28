// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Full solid payload decoding with hardware AES decryption, 4MB micro-buffer streaming,
//! and Mach kernel physical memory RSS monitoring.

use crate::codecs::deflate::deflate_decompress;
use crate::crypto::aes256::aes256_cbc_decrypt;
use crate::crypto::crc32::crc32_fast;
use crate::crypto::sha256::sha256_7z_kdf;
use crate::sevenz::format::*;
use crate::sevenz::header::{SevenZHeaderInfo, SevenZSeekIndex};
use crate::types::TTZipStatus;

/// Darwin Mach kernel task_info FFI binding for physical RSS tracking.
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
struct MachTaskBasicInfo {
    pub virtual_size: u64,
    pub resident_size: u64,
    pub resident_size_max: u64,
    pub user_time_sec: i32,
    pub user_time_usec: i32,
    pub system_time_sec: i32,
    pub system_time_usec: i32,
    pub policy: i32,
    pub suspend_count: i32,
}

const MACH_TASK_BASIC_INFO: u32 = 20;
const MACH_TASK_BASIC_INFO_COUNT: u32 =
    (std::mem::size_of::<MachTaskBasicInfo>() / std::mem::size_of::<i32>()) as u32;

#[cfg(target_os = "macos")]
extern "C" {
    fn mach_task_self() -> u32;
    fn task_info(
        target_task: u32,
        flavor: u32,
        task_info_out: *mut MachTaskBasicInfo,
        task_info_outCnt: *mut u32,
    ) -> i32;
}

/// Retrieves current physical Resident Set Size (RSS) in bytes.
pub fn get_current_rss_bytes() -> u64 {
    #[cfg(target_os = "macos")]
    unsafe {
        let mut info: MachTaskBasicInfo = std::mem::zeroed();
        let mut count = MACH_TASK_BASIC_INFO_COUNT;
        let kret = task_info(mach_task_self(), MACH_TASK_BASIC_INFO, &mut info, &mut count);
        if kret == 0 && info.resident_size > 0 {
            return info.resident_size;
        }
    }

    #[cfg(unix)]
    unsafe {
        let mut usage: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut usage);
        #[cfg(target_os = "macos")]
        {
            usage.ru_maxrss as u64
        }
        #[cfg(not(target_os = "macos"))]
        {
            (usage.ru_maxrss as u64) * 1024
        }
    }
    #[cfg(not(unix))]
    {
        0
    }
}

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
    const CHUNK_SIZE: usize = 4 * 1024 * 1024;

    match method_id {
        METHOD_LZMA2 => {
            let mut dstream = crate::codecs::lzma2::Fl2DStream::new(threads.max(1))?;
            dstream.init(coder_props.first().copied())?;

            let mut in_buf = crate::codecs::lzma2::Fl2InBuffer {
                src: raw_payload.as_ptr() as *const libc::c_void,
                size: raw_payload.len(),
                pos: 0,
            };

            let mut out_chunk = vec![0u8; CHUNK_SIZE];
            while in_buf.pos < in_buf.size {
                let prev_in_pos = in_buf.pos;
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
                if produced == 0 && in_buf.pos == prev_in_pos {
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
            for chunk in raw_payload.chunks(CHUNK_SIZE) {
                sink(chunk)?;
                total_decompressed += chunk.len() as u64;
            }
        }
        METHOD_DEFLATE => {
            let mut unpack_buf = vec![0u8; expected_unpack_size as usize];
            let decomp_len = deflate_decompress(raw_payload, &mut unpack_buf)?;
            unpack_buf.truncate(decomp_len);
            for chunk in unpack_buf.chunks(CHUNK_SIZE) {
                sink(chunk)?;
                total_decompressed += chunk.len() as u64;
            }
        }
        _ => return Err(TTZipStatus::ErrUnsupportedFeature),
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

    if loc.is_directory || loc.is_empty_stream || loc.uncompressed_size == 0 {
        return Ok(Vec::new());
    }

    let target_start = loc.offset_in_folder;
    let target_len = loc.uncompressed_size;
    let target_end = target_start + target_len;

    // Budget guard: If preceding data exceeds budget and budget is set
    if max_preceding_budget_bytes > 0 && target_start > max_preceding_budget_bytes {
        return Err(TTZipStatus::ErrSolidBudgetExceeded);
    }

    let mut current_offset: u64 = 0;
    let mut result_vec = Vec::with_capacity(target_len as usize);
    let folder_idx = loc.folder_index.unwrap_or(0);

    let decode_res = decode_7z_folder_streaming(
        mapped,
        info,
        folder_idx,
        password,
        1,
        |chunk| -> Result<(), TTZipStatus> {
            let chunk_start = current_offset;
            let chunk_len = chunk.len() as u64;
            let chunk_end = chunk_start + chunk_len;
            current_offset += chunk_len;

            // Chunk is entirely before target file: discard with zero allocations
            if chunk_end <= target_start {
                return Ok(());
            }

            // Chunk overlaps with target [target_start, target_end)
            if chunk_start < target_end && chunk_end > target_start {
                let slice_start = (target_start.saturating_sub(chunk_start)) as usize;
                let slice_end = (target_end.min(chunk_end) - chunk_start) as usize;
                result_vec.extend_from_slice(&chunk[slice_start..slice_end]);
            }

            // Early termination once target is fully read
            if current_offset >= target_end {
                return Err(TTZipStatus::Eof);
            }

            Ok(())
        },
    );

    match decode_res {
        Ok(_) | Err(TTZipStatus::Eof) => {}
        Err(e) => return Err(e),
    }

    // Verify CRC32
    if let Some(expected_crc) = loc.crc {
        if expected_crc != 0 && !result_vec.is_empty() {
            let computed = crc32_fast(0, &result_vec);
            if computed != expected_crc && info.is_encrypted {
                return Err(TTZipStatus::ErrInvalidPassword);
            }
        }
    }

    Ok(result_vec)
}
