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

fn slice_folder_pack_streams<'a>(
    mapped: &'a [u8],
    info: &SevenZHeaderInfo,
    folder: &SevenZFolder,
    folder_idx: usize,
) -> Result<Vec<&'a [u8]>, TTZipStatus> {
    let mut first_pack_idx = 0usize;
    for i in 0..folder_idx {
        if let Some(f) = info.folders.get(i) {
            first_pack_idx += f.pack_stream_count().max(1);
        }
    }
    first_pack_idx = first_pack_idx.min(info.pack_sizes.len());

    let num_current_pack = folder
        .pack_stream_count()
        .min(info.pack_sizes.len().saturating_sub(first_pack_idx))
        .max(1);

    let mut stream_slices = Vec::with_capacity(num_current_pack);
    let mut curr_off = folder.packed_offset;
    for i in 0..num_current_pack {
        let p_sz = info.pack_sizes.get(first_pack_idx + i).copied().unwrap_or(0) as usize;
        let end = curr_off.checked_add(p_sz).ok_or(TTZipStatus::ErrCorruptHeader)?;
        let p_data = mapped.get(curr_off..end).ok_or(TTZipStatus::ErrCorruptHeader)?;
        stream_slices.push(p_data);
        curr_off = end;
    }
    Ok(stream_slices)
}

fn build_folder_coder_graph(
    folder: &SevenZFolder,
) -> Result<crate::sevenz::dag::CoderGraph, TTZipStatus> {
    let total_in: usize = folder.coders.iter().map(|c| (c.num_in_streams as usize).max(1)).sum();
    let total_out: usize = folder.coders.iter().map(|c| (c.num_out_streams as usize).max(1)).sum();

    let mut in_to_coder = Vec::with_capacity(total_in);
    let mut out_to_coder = Vec::with_capacity(total_out);
    for (c_idx, c) in folder.coders.iter().enumerate() {
        in_to_coder.extend(std::iter::repeat_n(c_idx, (c.num_in_streams as usize).max(1)));
        out_to_coder.extend(std::iter::repeat_n(c_idx, (c.num_out_streams as usize).max(1)));
    }

    crate::sevenz::dag::CoderGraph::build_with_maps(
        folder.coders.len(),
        &folder.bind_pairs,
        &in_to_coder,
        &out_to_coder,
    )
    .map_err(|_| TTZipStatus::ErrCorruptHeader)
}

fn map_pack_streams_to_inputs<'a>(
    folder: &SevenZFolder,
    graph: &crate::sevenz::dag::CoderGraph,
    stream_slices: &[&'a [u8]],
) -> HashMap<usize, &'a [u8]> {
    let mut in_to_pack_data: HashMap<usize, &[u8]> = HashMap::new();
    if !folder.packed_streams.is_empty() {
        for (p_idx, &in_idx) in folder.packed_streams.iter().enumerate() {
            if let Some(&slice) = stream_slices.get(p_idx) {
                in_to_pack_data.insert(in_idx as usize, slice);
            }
        }
    } else {
        for (p_idx, &in_idx) in graph.packed_streams.iter().enumerate() {
            if let Some(&slice) = stream_slices.get(p_idx) {
                in_to_pack_data.insert(in_idx, slice);
            }
        }
    }
    in_to_pack_data
}

fn resolve_coder_input_slice<'a>(
    in_idx: usize,
    in_to_out: &HashMap<usize, usize>,
    out_buffers: &'a HashMap<usize, Vec<u8>>,
    in_to_pack_data: &HashMap<usize, &'a [u8]>,
) -> Result<&'a [u8], TTZipStatus> {
    if let Some(&out_idx) = in_to_out.get(&in_idx) {
        out_buffers
            .get(&out_idx)
            .map(Vec::as_slice)
            .ok_or(TTZipStatus::ErrCorruptHeader)
    } else {
        in_to_pack_data
            .get(&in_idx)
            .copied()
            .ok_or(TTZipStatus::ErrCorruptHeader)
    }
}

fn is_single_stream_branch_or_delta(method_id: u64) -> bool {
    matches!(
        method_id,
        METHOD_DELTA | METHOD_BCJ_X86 | METHOD_ARM64 | METHOD_ARM64_ALT
    )
}

fn execute_bcj2_coder_node(
    node: &crate::sevenz::dag::CoderNode,
    coder: &crate::sevenz::header::SevenZCoder,
    folder: &SevenZFolder,
    threads: u32,
    in_to_out: &HashMap<usize, usize>,
    in_to_pack_data: &HashMap<usize, &[u8]>,
    out_buffers: &mut HashMap<usize, Vec<u8>>,
) -> Result<(), TTZipStatus> {
    if coder.method_id == METHOD_AES {
        return Ok(());
    }
    let out_idx = *node.out_streams.first().ok_or(TTZipStatus::ErrCorruptHeader)?;

    if coder.method_id == METHOD_BCJ2 {
        if node.in_streams.len() < 4 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let main_data = resolve_coder_input_slice(node.in_streams[0], in_to_out, out_buffers, in_to_pack_data)?;
        let call_data = resolve_coder_input_slice(node.in_streams[1], in_to_out, out_buffers, in_to_pack_data)?;
        let jump_data = resolve_coder_input_slice(node.in_streams[2], in_to_out, out_buffers, in_to_pack_data)?;
        let rc_data = resolve_coder_input_slice(node.in_streams[3], in_to_out, out_buffers, in_to_pack_data)?;
        let bcj2_decoded = crate::codecs::branch::bcj2::decode_bcj2(main_data, call_data, jump_data, rc_data, 0)?;
        out_buffers.insert(out_idx, bcj2_decoded);
        return Ok(());
    }

    let in_idx = *node.in_streams.first().ok_or(TTZipStatus::ErrCorruptHeader)?;
    let in_data = resolve_coder_input_slice(in_idx, in_to_out, out_buffers, in_to_pack_data)?;

    if is_single_stream_branch_or_delta(coder.method_id) {
        let mut filtered = in_data.to_vec();
        apply_filter_inplace(coder.method_id, &coder.properties, &mut filtered);
        out_buffers.insert(out_idx, filtered);
        return Ok(());
    }

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
    Ok(())
}

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
    let stream_slices = slice_folder_pack_streams(mapped, info, folder, folder_idx)?;
    let graph = build_folder_coder_graph(folder)?;
    let in_to_pack_data = map_pack_streams_to_inputs(folder, &graph, &stream_slices);

    let in_to_out: HashMap<usize, usize> = folder
        .bind_pairs
        .iter()
        .map(|&(in_idx, out_idx)| (in_idx as usize, out_idx as usize))
        .collect();
    let mut out_buffers: HashMap<usize, Vec<u8>> = HashMap::new();

    for &c_idx in &graph.topological_order {
        execute_bcj2_coder_node(
            &graph.nodes[c_idx],
            &folder.coders[c_idx],
            folder,
            threads,
            &in_to_out,
            &in_to_pack_data,
            &mut out_buffers,
        )?;
    }

    let main_out_idx = graph.nodes[graph.main_coder].out_streams.first().copied().unwrap_or(0);
    let final_data = out_buffers.get(&main_out_idx).ok_or(TTZipStatus::ErrCorruptHeader)?;

    for chunk in final_data.chunks(CHUNK_SIZE) {
        sink(chunk)?;
    }
    Ok(final_data.len() as u64)
}
