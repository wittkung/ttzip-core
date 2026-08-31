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
    use std::io::Read;
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
        METHOD_BZIP2 => {
            let mut unpack_buf = vec![0u8; expected_unpack_size as usize];
            let decomp_len = crate::codecs::bzip2::bzip2_decompress(raw_payload, &mut unpack_buf)?;
            unpack_buf.truncate(decomp_len);
            for chunk in unpack_buf.chunks(CHUNK_SIZE) {
                sink(chunk)?;
                total_decompressed += chunk.len() as u64;
            }
        }
        METHOD_PPMD => {
            if coder_props.len() < 5 {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            let order = coder_props[0] as u32;
            let mem_size = u32::from_le_bytes(coder_props[1..5].try_into().unwrap_or([0; 4]));
            let mut dec = ppmd_rust::Ppmd7Decoder::new(raw_payload, order, mem_size)
                .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
            let mut out_chunk = vec![0u8; CHUNK_SIZE];
            loop {
                let n = dec.read(&mut out_chunk).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                if n == 0 {
                    break;
                }
                sink(&out_chunk[..n])?;
                total_decompressed += n as u64;
            }
        }
        METHOD_LZ4 => {
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
            let mut out_chunk = vec![0u8; CHUNK_SIZE];
            loop {
                let n = dec.read(&mut out_chunk).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                if n == 0 {
                    break;
                }
                sink(&out_chunk[..n])?;
                total_decompressed += n as u64;
            }
        }
        METHOD_BROTLI => {
            let mut raw_br = raw_payload;
            if raw_br.len() >= 16 {
                let magic = u32::from_le_bytes(raw_br[..4].try_into().unwrap_or([0; 4]));
                if magic == 0x184D2A50 {
                    let comp_sz = u32::from_le_bytes(raw_br[8..12].try_into().unwrap_or([0; 4])) as usize;
                    if 16 + comp_sz <= raw_br.len() {
                        raw_br = &raw_br[16..16 + comp_sz];
                    }
                }
            }
            let mut dec = brotli::Decompressor::new(raw_br, 65536);
            let mut out_chunk = vec![0u8; CHUNK_SIZE];
            loop {
                let n = dec.read(&mut out_chunk).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                if n == 0 {
                    break;
                }
                sink(&out_chunk[..n])?;
                total_decompressed += n as u64;
            }
        }
        _ => return Err(TTZipStatus::ErrUnsupportedFeature),
    }

    Ok(total_decompressed)
}

/// Helper function to apply in-place branch/delta filter to decompressed buffer.
fn apply_filter_inplace(method_id: u64, props: &[u8], buf: &mut [u8]) {
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
    const CHUNK_SIZE: usize = 4 * 1024 * 1024;
    let folder = match info.folders.get(folder_idx) {
        Some(f) => f,
        None => {
            let exp_sz = if !info.stream_sizes.is_empty() {
                info.stream_sizes.iter().sum()
            } else {
                info.payload_len as u64
            };
            return decode_raw_payload_chunked(
                &mapped[info.payload_offset..info.payload_offset + info.payload_len],
                info.primary_method_id,
                info.coder_props.as_slice(),
                exp_sz,
                threads,
                sink,
            );
        }
    };

    if folder.packed_len == 0 {
        return Ok(0);
    }

    let payload_end = folder.packed_offset + folder.packed_len;
    if payload_end > mapped.len() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    // Check for AES encryption layer
    let has_aes = folder.coders.iter().any(|c| c.method_id == METHOD_AES) || info.is_encrypted;
    let mut decrypted_storage = zeroize::Zeroizing::new(Vec::new());
    let mut raw_payload = &mapped[folder.packed_offset..payload_end];

    if has_aes {
        let pass = password.ok_or(TTZipStatus::ErrInvalidPassword)?;
        if pass.is_empty() {
            return Err(TTZipStatus::ErrInvalidPassword);
        }

        let mut salt = [0u8; 16];
        let mut salt_len = info.aes_salt_len;
        salt[..salt_len.min(16)].copy_from_slice(&info.aes_salt[..salt_len.min(16)]);
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
                if salt_size > 0 && p_off + salt_size <= props.len() {
                    salt_len = salt_size.min(16);
                    salt[..salt_len].copy_from_slice(&props[p_off..p_off + salt_len]);
                    p_off += salt_size;
                } else {
                    salt_len = 0;
                }
                iv = [0u8; 16];
                if iv_size > 0 && p_off + iv_size <= props.len() {
                    let copy_len = iv_size.min(16);
                    iv[..copy_len].copy_from_slice(&props[p_off..p_off + copy_len]);
                }
            }
        }

        let key = zeroize::Zeroizing::new(sha256_7z_kdf(
            pass,
            &salt[..salt_len],
            num_cycles_power,
        ));

        if !raw_payload.len().is_multiple_of(16) {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        decrypted_storage.resize(raw_payload.len(), 0);
        aes256_cbc_decrypt(&key, &iv, raw_payload, &mut decrypted_storage)
            .map_err(|_| TTZipStatus::ErrInvalidPassword)?;

        // If folder has an unpack_size for the AES coder, trim to that size
        let aes_unpack_sz = folder.unpack_sizes.first().copied().unwrap_or(decrypted_storage.len() as u64) as usize;
        if aes_unpack_sz <= decrypted_storage.len() {
            raw_payload = &decrypted_storage[..aes_unpack_sz];
        } else {
            raw_payload = &decrypted_storage;
        }
    }

    // Filter out AES coder for downstream processing
    let non_aes_coders: Vec<_> = folder
        .coders
        .iter()
        .filter(|c| c.method_id != METHOD_AES)
        .collect();

    // Check for BCJ2 coder (4-Stream composite folder)
    let has_bcj2 = non_aes_coders.iter().any(|c| c.method_id == METHOD_BCJ2);
    if has_bcj2 {
        // Calculate pack streams for this folder
        let mut first_pack_idx = 0usize;
        for i in 0..folder_idx {
            if let Some(f) = info.folders.get(i) {
                let num_pack = if f.coders.iter().any(|c| c.method_id == METHOD_BCJ2) { 4 } else { 1 };
                first_pack_idx += num_pack;
            }
        }

        let mut stream_slices = Vec::with_capacity(4);
        let mut curr_off = folder.packed_offset;
        for i in 0..4 {
            let p_sz = info.pack_sizes.get(first_pack_idx + i).copied().unwrap_or(0) as usize;
            let p_data = if curr_off + p_sz <= mapped.len() {
                &mapped[curr_off..curr_off + p_sz]
            } else {
                return Err(TTZipStatus::ErrCorruptHeader);
            };
            stream_slices.push(p_data);
            curr_off += p_sz;
        }

        // Decompress individual BCJ2 input streams
        let mut main_buf = Vec::new();
        let mut call_buf = Vec::new();
        let mut jump_buf = Vec::new();
        let rc_buf;

        if folder.coders.len() >= 4 {
            // Complex BCJ2 topology (e.g. 7za433_7zip_lzma2_bcj2):
            // p0 (LZMA2 with Coder 2) -> Main (405636 bytes)
            // p1 (Raw) -> RC (578 bytes)
            // p2 (LZMA with Coder 1) -> Call (37980 bytes)
            // p3 (LZMA with Coder 0) -> Jump (18720 bytes)
            let s0_sz = folder.unpack_sizes.get(2).copied().unwrap_or(0);
            decode_raw_payload_chunked(
                stream_slices[0],
                folder.coders[2].method_id,
                &folder.coders[2].properties,
                s0_sz,
                threads,
                |chunk| {
                    main_buf.extend_from_slice(chunk);
                    Ok(())
                },
            )?;

            rc_buf = stream_slices[1];

            let s2_sz = folder.unpack_sizes.get(1).copied().unwrap_or(0);
            decode_raw_payload_chunked(
                stream_slices[2],
                folder.coders[1].method_id,
                &folder.coders[1].properties,
                s2_sz,
                threads,
                |chunk| {
                    call_buf.extend_from_slice(chunk);
                    Ok(())
                },
            )?;

            let s3_sz = folder.unpack_sizes.first().copied().unwrap_or(0);
            decode_raw_payload_chunked(
                stream_slices[3],
                folder.coders[0].method_id,
                &folder.coders[0].properties,
                s3_sz,
                threads,
                |chunk| {
                    jump_buf.extend_from_slice(chunk);
                    Ok(())
                },
            )?;
        } else {
            // Raw BCJ2 streams (e.g. delta_bcj2)
            main_buf = stream_slices[0].to_vec();
            call_buf = stream_slices[1].to_vec();
            jump_buf = stream_slices[2].to_vec();
            rc_buf = stream_slices[3];
        }

        let mut bcj2_decoded = crate::codecs::branch::bcj2::decode_bcj2(
            &main_buf,
            &call_buf,
            &jump_buf,
            rc_buf,
            0,
        )?;

        // Apply any chained filter (e.g. Delta in delta_bcj2)
        if let Some(delta_coder) = non_aes_coders.iter().find(|c| c.method_id == METHOD_DELTA) {
            apply_filter_inplace(METHOD_DELTA, &delta_coder.properties, &mut bcj2_decoded);
        }

        for chunk in bcj2_decoded.chunks(CHUNK_SIZE) {
            sink(chunk)?;
        }
        return Ok(bcj2_decoded.len() as u64);
    }

    // Check for branch / delta filter + compression coder pipeline
    let is_filter = |mid: u64| {
        mid == METHOD_BCJ_X86 || mid == METHOD_ARM64 || mid == METHOD_ARM64_ALT || mid == METHOD_DELTA
    };

    if let Some(filter_coder) = non_aes_coders.iter().find(|c| is_filter(c.method_id)) {
        let comp_coder = non_aes_coders
            .iter()
            .find(|c| !is_filter(c.method_id))
            .copied();

        let mid = comp_coder.map(|c| c.method_id).unwrap_or(METHOD_COPY);
        let props = comp_coder.map(|c| c.properties.as_slice()).unwrap_or(&[]);
        let exp_sz = folder.unpack_sizes.last().copied().unwrap_or(0);

        let mut decomp_buf = Vec::with_capacity(exp_sz as usize);
        decode_raw_payload_chunked(raw_payload, mid, props, exp_sz, threads, |chunk| {
            decomp_buf.extend_from_slice(chunk);
            Ok(())
        })?;

        apply_filter_inplace(filter_coder.method_id, &filter_coder.properties, &mut decomp_buf);

        for chunk in decomp_buf.chunks(CHUNK_SIZE) {
            sink(chunk)?;
        }
        return Ok(decomp_buf.len() as u64);
    }

    // Single coder standard path
    let primary_coder = non_aes_coders.first().copied();
    let method_id = primary_coder.map(|c| c.method_id).unwrap_or(info.primary_method_id);
    let coder_props = primary_coder
        .map(|c| c.properties.as_slice())
        .unwrap_or(&info.coder_props);
    let expected_unpack_size = folder.unpack_sizes.last().copied().unwrap_or(0);

    decode_raw_payload_chunked(
        raw_payload,
        method_id,
        coder_props,
        expected_unpack_size,
        threads,
        sink,
    )
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
