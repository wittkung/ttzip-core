// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Full solid payload decoding with hardware AES decryption, 4MB micro-buffer streaming,
//! and Mach kernel physical memory RSS monitoring.

use crate::codecs::deflate::deflate_decompress;
use crate::crypto::aes256::aes256_cbc_decrypt;
use crate::crypto::crc32::crc32_fast;
use crate::crypto::sha256::sha256_7z_kdf;
use crate::sevenz::format::*;
use crate::sevenz::header::{SevenZHeaderInfo, SevenZSeekIndex};
use crate::types::TTZipStatus;

/// Maximum allowable uncompressed payload size allocated in contiguous memory (64MB ceiling).
pub const MAX_IN_MEMORY_DECOMPRESS_BYTES: usize = 64 * 1024 * 1024;

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
            // 1. Stack-allocated 13-byte Alone header (0 heap allocations)
            let mut header = [0u8; 13];
            header[..5].copy_from_slice(&coder_props[..5]);
            header[5..13].copy_from_slice(&expected_unpack_size.to_le_bytes());

            let mut decoder = crate::codecs::lzma::LzmaAloneDecoder::new()?;
            let mut out_chunk = vec![0u8; CHUNK_SIZE];

            // 2. Feed 13-byte header to initialize LZMA model parameters
            let mut header_slice = &header[..];
            while !header_slice.is_empty() {
                let (consumed, produced, is_end) = decoder.decompress_chunk(
                    header_slice,
                    &mut out_chunk,
                    false,
                )?;
                header_slice = &header_slice[consumed..];
                if produced > 0 {
                    sink(&out_chunk[..produced])?;
                    total_decompressed += produced as u64;
                }
                if is_end {
                    break;
                }
            }

            // 3. Zero-copy stream raw payload directly without intermediate Vec allocation
            let mut payload_slice = raw_payload;
            while !payload_slice.is_empty() {
                let (consumed, produced, is_end) = decoder.decompress_chunk(
                    payload_slice,
                    &mut out_chunk,
                    true,
                )?;
                payload_slice = &payload_slice[consumed..];
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
            // Direct pass-through streaming decompression for raw RFC 7932 Brotli streams.
            let mut dec = brotli::Decompressor::new(raw_payload, 65536);
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
        let aes_coder_idx = folder.coders.iter().position(|c| c.method_id == METHOD_AES);
        let aes_unpack_sz = aes_coder_idx
            .and_then(|idx| folder.unpack_sizes.get(idx).copied())
            .or_else(|| folder.unpack_sizes.first().copied())
            .unwrap_or(decrypted_storage.len() as u64) as usize;
        raw_payload = if aes_unpack_sz <= decrypted_storage.len() {
            &decrypted_storage[..aes_unpack_sz]
        } else {
            &decrypted_storage
        };
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
                let num_pack = if !f.packed_streams.is_empty() {
                    f.packed_streams.len()
                } else {
                    let total_in: usize = f.coders.iter().map(|c| (c.num_in_streams as usize).max(1)).sum();
                    (total_in.saturating_sub(f.bind_pairs.len())).max(1)
                };
                first_pack_idx += num_pack;
            }
        }

        let total_in: usize = folder.coders.iter().map(|c| (c.num_in_streams as usize).max(1)).sum();
        let total_out: usize = folder.coders.iter().map(|c| (c.num_out_streams as usize).max(1)).sum();
        let num_current_pack = if !folder.packed_streams.is_empty() {
            folder.packed_streams.len()
        } else {
            (total_in.saturating_sub(folder.bind_pairs.len())).max(1)
        };

        let mut stream_slices = Vec::with_capacity(num_current_pack);
        let mut curr_off = folder.packed_offset;
        for i in 0..num_current_pack {
            let p_sz = info.pack_sizes.get(first_pack_idx + i).copied().unwrap_or(0) as usize;
            let p_data = if curr_off + p_sz <= mapped.len() {
                &mapped[curr_off..curr_off + p_sz]
            } else {
                return Err(TTZipStatus::ErrCorruptHeader);
            };
            stream_slices.push(p_data);
            curr_off += p_sz;
        }

        let mut in_to_coder = Vec::with_capacity(total_in);
        for (c_idx, c) in folder.coders.iter().enumerate() {
            let count = (c.num_in_streams as usize).max(1);
            for _ in 0..count {
                in_to_coder.push(c_idx);
            }
        }

        let mut out_to_coder = Vec::with_capacity(total_out);
        for (c_idx, c) in folder.coders.iter().enumerate() {
            let count = (c.num_out_streams as usize).max(1);
            for _ in 0..count {
                out_to_coder.push(c_idx);
            }
        }

        let graph = crate::sevenz::dag::CoderGraph::build_with_maps(
            folder.coders.len(),
            &folder.bind_pairs,
            &in_to_coder,
            &out_to_coder,
        ).map_err(|_| TTZipStatus::ErrCorruptHeader)?;

        // Map global in_stream index -> packed slice
        let mut in_to_pack_data: std::collections::HashMap<usize, &[u8]> = std::collections::HashMap::new();
        if !folder.packed_streams.is_empty() {
            for (p_idx, &in_idx) in folder.packed_streams.iter().enumerate() {
                if p_idx < stream_slices.len() {
                    in_to_pack_data.insert(in_idx as usize, stream_slices[p_idx]);
                }
            }
        } else {
            for (p_idx, &in_idx) in graph.packed_streams.iter().enumerate() {
                if p_idx < stream_slices.len() {
                    in_to_pack_data.insert(in_idx, stream_slices[p_idx]);
                }
            }
        }

        // Map global in_stream -> global out_stream from bind_pairs
        let mut in_to_out: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &(in_idx, out_idx) in &folder.bind_pairs {
            in_to_out.insert(in_idx as usize, out_idx as usize);
        }

        let mut out_buffers: std::collections::HashMap<usize, Vec<u8>> = std::collections::HashMap::new();

        // Process coders in Kahn topological order
        for &c_idx in &graph.topological_order {
            let node = &graph.nodes[c_idx];
            let coder = &folder.coders[c_idx];

            if coder.method_id == METHOD_AES {
                continue;
            }

            if coder.method_id == METHOD_BCJ2 {
                if node.in_streams.len() < 4 || node.out_streams.is_empty() {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }

                let resolve_stream = |in_idx: usize| -> Result<&[u8], TTZipStatus> {
                    if let Some(&out_idx) = in_to_out.get(&in_idx) {
                        out_buffers.get(&out_idx).map(|v| v.as_slice()).ok_or(TTZipStatus::ErrCorruptHeader)
                    } else if let Some(&p_data) = in_to_pack_data.get(&in_idx) {
                        Ok(p_data)
                    } else {
                        Err(TTZipStatus::ErrCorruptHeader)
                    }
                };

                let main_data = resolve_stream(node.in_streams[0])?;
                let call_data = resolve_stream(node.in_streams[1])?;
                let jump_data = resolve_stream(node.in_streams[2])?;
                let rc_data = resolve_stream(node.in_streams[3])?;

                let bcj2_decoded = crate::codecs::branch::bcj2::decode_bcj2(
                    main_data,
                    call_data,
                    jump_data,
                    rc_data,
                    0,
                )?;

                out_buffers.insert(node.out_streams[0], bcj2_decoded);
            } else if coder.method_id == METHOD_DELTA
                || coder.method_id == METHOD_BCJ_X86
                || coder.method_id == METHOD_ARM64
                || coder.method_id == METHOD_ARM64_ALT
            {
                if node.in_streams.is_empty() || node.out_streams.is_empty() {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
                let in_idx = node.in_streams[0];
                let in_data = if let Some(&out_idx) = in_to_out.get(&in_idx) {
                    out_buffers.get(&out_idx).map(|v| v.as_slice()).ok_or(TTZipStatus::ErrCorruptHeader)?
                } else if let Some(&p_data) = in_to_pack_data.get(&in_idx) {
                    p_data
                } else {
                    return Err(TTZipStatus::ErrCorruptHeader);
                };

                let mut filtered = in_data.to_vec();
                apply_filter_inplace(coder.method_id, &coder.properties, &mut filtered);
                out_buffers.insert(node.out_streams[0], filtered);
            } else {
                if node.in_streams.is_empty() || node.out_streams.is_empty() {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
                let in_idx = node.in_streams[0];
                let in_data = if let Some(&out_idx) = in_to_out.get(&in_idx) {
                    out_buffers.get(&out_idx).map(|v| v.as_slice()).ok_or(TTZipStatus::ErrCorruptHeader)?
                } else if let Some(&p_data) = in_to_pack_data.get(&in_idx) {
                    p_data
                } else {
                    return Err(TTZipStatus::ErrCorruptHeader);
                };

                let out_idx = node.out_streams[0];
                let exp_sz = folder.unpack_sizes.get(out_idx).copied().unwrap_or(0);
                let mut decomp_buf = Vec::with_capacity(exp_sz as usize);
                decode_raw_payload_chunked(
                    in_data,
                    coder.method_id,
                    &coder.properties,
                    exp_sz,
                    threads,
                    |chunk| {
                        decomp_buf.extend_from_slice(chunk);
                        Ok(())
                    },
                )?;
                out_buffers.insert(out_idx, decomp_buf);
            }
        }

        let main_out_idx = graph.nodes[graph.main_coder].out_streams.first().copied().unwrap_or(0);
        let final_data = out_buffers.get(&main_out_idx).ok_or(TTZipStatus::ErrCorruptHeader)?;

        for chunk in final_data.chunks(CHUNK_SIZE) {
            sink(chunk)?;
        }
        return Ok(final_data.len() as u64);
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
        if exp_sz as usize > MAX_IN_MEMORY_DECOMPRESS_BYTES {
            return Err(TTZipStatus::ErrOutOfMemory);
        }

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
    let primary_coder_idx = folder.coders.iter().position(|c| c.method_id != METHOD_AES);
    let expected_unpack_size = primary_coder_idx
        .and_then(|idx| folder.unpack_sizes.get(idx).copied())
        .or_else(|| folder.unpack_sizes.last().copied())
        .unwrap_or(0);

    decode_raw_payload_chunked(
        raw_payload,
        method_id,
        coder_props,
        expected_unpack_size,
        threads,
        sink,
    ).map_err(|err| if has_aes { TTZipStatus::ErrInvalidPassword } else { err })
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

    // In-memory allocation ceiling: Enforce strict 64MB limit for single-entry in-memory extraction
    if (target_len as usize) > MAX_IN_MEMORY_DECOMPRESS_BYTES {
        return Err(TTZipStatus::ErrOutOfMemory);
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
}
