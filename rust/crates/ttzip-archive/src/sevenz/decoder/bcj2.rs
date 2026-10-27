// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! 7-Zip BCJ2 4-Stream Composite Folder Decoding Pipeline.
//!
//! Executes Kahn topological sort over coder DAGs to stream-decode
//! multi-input / multi-output composite filter topologies.

use std::collections::HashMap;

use super::payload::{apply_filter_inplace, decode_raw_payload_chunked, CHUNK_SIZE, MAX_IN_MEMORY_DECOMPRESS_BYTES};
use crate::sevenz::format::*;
use crate::sevenz::header::{SevenZFolder, SevenZHeaderInfo};
use crate::types::TTZipStatus;

/// Decompresses and reconstructs a 4-stream BCJ2 composite coder folder into a streaming sink.
pub(crate) fn decode_bcj2_folder_streaming<F>(
    mapped: &[u8],
    info: &SevenZHeaderInfo,
    folder: &SevenZFolder,
    folder_idx: usize,
    threads: u32,
    mut sink: F,
) -> Result<u64, TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    // Calculate pack streams for this folder
    let mut first_pack_idx = 0usize;
    for i in 0..folder_idx {
        if let Some(f) = info.folders.get(i) {
            first_pack_idx += f.pack_stream_count().max(1);
        }
    }
    first_pack_idx = first_pack_idx.min(info.pack_sizes.len());

    let total_in: usize = folder.coders.iter().map(|c| (c.num_in_streams as usize).max(1)).sum();
    let total_out: usize = folder.coders.iter().map(|c| (c.num_out_streams as usize).max(1)).sum();
    let num_current_pack = folder
        .pack_stream_count()
        .min(info.pack_sizes.len().saturating_sub(first_pack_idx))
        .max(1);

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
    let mut in_to_pack_data: HashMap<usize, &[u8]> = HashMap::new();
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
    let mut in_to_out: HashMap<usize, usize> = HashMap::new();
    for &(in_idx, out_idx) in &folder.bind_pairs {
        in_to_out.insert(in_idx as usize, out_idx as usize);
    }

    let mut out_buffers: HashMap<usize, Vec<u8>> = HashMap::new();

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
            if exp_sz > MAX_IN_MEMORY_DECOMPRESS_BYTES as u64 {
                return Err(TTZipStatus::ErrOutOfMemory);
            }
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
    Ok(final_data.len() as u64)
}
