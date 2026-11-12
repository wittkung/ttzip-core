// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Self-healing in-memory and streaming in-place repair engine for damaged archives.

use super::cauchy::ReedSolomonEngine;
use super::inspect::{inspect_recovery_record, inspect_recovery_record_reader};
use crate::crc32::crc32_fast;
use crate::sha256::FastSha256;
use std::borrow::Cow;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use ttzip_types::TTZipStatus;

/// Workspace for zero-clone in-place Reed-Solomon shard reconstruction.
#[derive(Debug, Clone)]
pub struct ReconstructWorkspace {
    pub engine: ReedSolomonEngine,
}

impl ReconstructWorkspace {
    /// Creates a new reconstruction workspace for $K$ data shards and $M$ parity shards.
    pub fn new(k: usize, m: usize) -> Result<Self, TTZipStatus> {
        Ok(Self {
            engine: ReedSolomonEngine::new(k, m)?,
        })
    }

    /// Reconstructs erased data shards (`shards[0..k]`) in-place without cloning intact shards.
    ///
    /// Only allocates reconstruction buffers for the `missing_count` erased data shards,
    /// computes them from borrowed intact shard slices, and writes them back into `shards`.
    pub fn decode_in_place(&self, shards: &mut [Option<Vec<u8>>]) -> Result<(), TTZipStatus> {
        let k = self.engine.k;
        let m = self.engine.m;
        if shards.len() != k + m {
            return Err(TTZipStatus::ErrInvalidParam);
        }

        let missing_indices: Vec<usize> = (0..k).filter(|&i| shards[i].is_none()).collect();
        let missing_count = missing_indices.len();
        if missing_count == 0 {
            return Ok(());
        }
        if missing_count > m {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let reconstructed = {
            let mut available_indices = Vec::with_capacity(k);
            let mut available_refs = Vec::with_capacity(k);
            let mut shard_len = 0usize;

            for (idx, slot) in shards.iter().enumerate() {
                if let Some(buf) = slot.as_deref() {
                    if shard_len == 0 {
                        shard_len = buf.len();
                    } else if buf.len() != shard_len {
                        return Err(TTZipStatus::ErrInvalidParam);
                    }
                    available_indices.push(idx);
                    available_refs.push(buf);
                    if available_refs.len() == k {
                        break;
                    }
                }
            }

            if available_refs.len() < k || shard_len == 0 {
                return Err(TTZipStatus::ErrCorruptHeader);
            }

            let mut recon_buffers = vec![vec![0u8; shard_len]; missing_count];
            let mut recon_mut_refs: Vec<&mut [u8]> =
                recon_buffers.iter_mut().map(Vec::as_mut_slice).collect();
            self.engine.decode(
                &available_refs,
                &available_indices,
                &missing_indices,
                &mut recon_mut_refs,
            )?;
            recon_buffers
        };

        for (missing_i, buf) in missing_indices.into_iter().zip(reconstructed) {
            shards[missing_i] = Some(buf);
        }
        Ok(())
    }
}

#[inline]
fn is_valid_fec_geometry(k: usize, m: usize, slice_size: usize) -> bool {
    k > 0 && m > 0 && k + m <= 256 && slice_size > 0 && slice_size <= 64 * 1024 * 1024
}

fn reconstruct_missing_slices_from_refs(
    k: usize,
    m: usize,
    slice_size: usize,
    available_indices: &[usize],
    available_refs: &[&[u8]],
    missing_indices: &[usize],
) -> Result<Vec<Vec<u8>>, TTZipStatus> {
    let rs = ReedSolomonEngine::new(k, m)?;
    let mut reconstructed = vec![vec![0u8; slice_size]; missing_indices.len()];
    let mut recon_mut_refs: Vec<&mut [u8]> =
        reconstructed.iter_mut().map(Vec::as_mut_slice).collect();

    rs.decode(
        available_refs,
        available_indices,
        missing_indices,
        &mut recon_mut_refs,
    )?;
    Ok(reconstructed)
}

fn reconstruct_missing_slices(
    k: usize,
    m: usize,
    slice_size: usize,
    available_indices: &[usize],
    available_buffers: &[Vec<u8>],
    missing_indices: &[usize],
) -> Result<Vec<Vec<u8>>, TTZipStatus> {
    let available_refs: Vec<&[u8]> = available_buffers.iter().map(Vec::as_slice).collect();
    reconstruct_missing_slices_from_refs(
        k,
        m,
        slice_size,
        available_indices,
        &available_refs,
        missing_indices,
    )
}

type ClassifiedMemoryDataSlices<'a> = (Vec<(usize, Cow<'a, [u8]>)>, Vec<usize>);

fn classify_memory_data_slices<'a>(
    archive_data: &'a [u8],
    payload_len: usize,
    k: usize,
    slice_size: usize,
) -> ClassifiedMemoryDataSlices<'a> {
    let mut intact_shards = Vec::with_capacity(k);
    let mut missing_indices = Vec::new();

    for i in 0..k {
        let crc_off = payload_len + 54 + (i * 4);
        let expected_crc =
            u32::from_le_bytes(archive_data[crc_off..crc_off + 4].try_into().unwrap_or([0u8; 4]));

        let start = i * slice_size;
        let end = (start + slice_size).min(payload_len);
        if start < payload_len && end - start == slice_size {
            let borrowed = &archive_data[start..end];
            if crc32_fast(0, borrowed) == expected_crc {
                intact_shards.push((i, Cow::Borrowed(borrowed)));
            } else {
                missing_indices.push(i);
            }
        } else {
            let mut padded = vec![0u8; slice_size];
            if start < payload_len {
                padded[..(end - start)].copy_from_slice(&archive_data[start..end]);
            }
            if crc32_fast(0, &padded) == expected_crc {
                intact_shards.push((i, Cow::Owned(padded)));
            } else {
                missing_indices.push(i);
            }
        }
    }
    (intact_shards, missing_indices)
}

fn collect_memory_parity_slices<'a>(
    archive_data: &'a [u8],
    payload_len: usize,
    k: usize,
    m: usize,
    slice_size: usize,
    intact_shards: &mut Vec<(usize, Cow<'a, [u8]>)>,
) {
    let mut p_offset = payload_len + 54 + (k * 4);
    for p_idx in 0..m {
        if p_offset + 6 + slice_size <= archive_data.len() {
            let p_expected_crc = u32::from_le_bytes(
                archive_data[p_offset + 2..p_offset + 6].try_into().unwrap_or([0u8; 4]),
            );
            let p_slice = &archive_data[p_offset + 6..p_offset + 6 + slice_size];
            if crc32_fast(0, p_slice) == p_expected_crc {
                intact_shards.push((k + p_idx, Cow::Borrowed(p_slice)));
            }
            p_offset += 6 + slice_size;
        }
    }
}

/// Verifies and performs self-healing restoration on damaged archive data in memory.
pub fn repair_archive_data(archive_data: &mut [u8]) -> Result<bool, TTZipStatus> {
    let Some(info) = inspect_recovery_record(archive_data)? else {
        return Ok(false);
    };

    let payload_len = info.protected_payload_length as usize;
    if payload_len > archive_data.len() {
        return Ok(false);
    }

    if FastSha256::digest(&archive_data[..payload_len]) == info.root_hash {
        return Ok(true);
    }

    let (k, m, slice_size) = (info.data_slices_count, info.parity_slices_count, info.slice_size);
    if !is_valid_fec_geometry(k, m, slice_size) || archive_data.len() - payload_len < 54 + (k * 4) {
        return Ok(false);
    }

    let (missing_indices, reconstructed) = {
        let (mut intact_shards, missing_indices) =
            classify_memory_data_slices(archive_data, payload_len, k, slice_size);
        if missing_indices.is_empty() {
            return Ok(true);
        }
        if missing_indices.len() > m {
            return Ok(false);
        }

        collect_memory_parity_slices(
            archive_data,
            payload_len,
            k,
            m,
            slice_size,
            &mut intact_shards,
        );
        if intact_shards.len() < k {
            return Ok(false);
        }

        intact_shards.truncate(k);
        let available_indices: Vec<usize> = intact_shards.iter().map(|s| s.0).collect();
        let available_refs: Vec<&[u8]> = intact_shards.iter().map(|s| s.1.as_ref()).collect();

        let reconstructed = reconstruct_missing_slices_from_refs(
            k,
            m,
            slice_size,
            &available_indices,
            &available_refs,
            &missing_indices,
        )?;
        (missing_indices, reconstructed)
    };

    for (m_idx, &missing_i) in missing_indices.iter().enumerate() {
        let start = missing_i * slice_size;
        let end = (start + slice_size).min(payload_len);
        archive_data[start..end].copy_from_slice(&reconstructed[m_idx][..(end - start)]);
    }

    Ok(FastSha256::digest(&archive_data[..payload_len]) == info.root_hash)
}

struct StreamingDataScan {
    initial_hash: [u8; 32],
    intact_data_indices: Vec<usize>,
    missing_indices: Vec<usize>,
}

struct StreamingRepairShards {
    available_indices: Vec<usize>,
    available_buffers: Vec<Vec<u8>>,
}

fn scan_streaming_data_slices(
    file: &mut std::fs::File,
    payload_len: u64,
    k: usize,
    slice_size: usize,
) -> Result<StreamingDataScan, TTZipStatus> {
    file.seek(SeekFrom::Start(payload_len + 54))
        .map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let mut expected_crcs = Vec::with_capacity(k);
    for _ in 0..k {
        let mut buf = [0u8; 4];
        file.read_exact(&mut buf)
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
        expected_crcs.push(u32::from_le_bytes(buf));
    }

    file.seek(SeekFrom::Start(0))
        .map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let mut hasher = FastSha256::new();
    let mut missing_indices = Vec::new();
    let mut intact_data_indices = Vec::new();
    let mut slice_buf = vec![0u8; slice_size];

    for (d, &expected_crc) in expected_crcs.iter().enumerate() {
        let start = d as u64 * slice_size as u64;
        let len = (payload_len.saturating_sub(start)).min(slice_size as u64) as usize;
        file.read_exact(&mut slice_buf[..len])
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
        slice_buf[len..slice_size].fill(0);
        hasher.update(&slice_buf[..len]);
        if crc32_fast(0, &slice_buf) == expected_crc {
            intact_data_indices.push(d);
        } else {
            missing_indices.push(d);
        }
    }

    Ok(StreamingDataScan {
        initial_hash: hasher.finalize(),
        intact_data_indices,
        missing_indices,
    })
}

fn gather_streaming_repair_shards(
    file: &mut std::fs::File,
    payload_len: u64,
    k: usize,
    m: usize,
    slice_size: usize,
    intact_data_indices: &[usize],
) -> Result<Option<StreamingRepairShards>, TTZipStatus> {
    let parity_start_offset = payload_len + 54 + (k as u64 * 4);
    file.seek(SeekFrom::Start(parity_start_offset))
        .map_err(|_| TTZipStatus::ErrOpenFailed)?;

    let mut intact_parity_shards = Vec::new();
    for p in 0..m {
        let mut header_buf = [0u8; 6];
        let mut p_slice = vec![0u8; slice_size];
        file.read_exact(&mut header_buf)
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
        file.read_exact(&mut p_slice)
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;

        let p_expected_crc = u32::from_le_bytes([header_buf[2], header_buf[3], header_buf[4], header_buf[5]]);
        if crc32_fast(0, &p_slice) == p_expected_crc {
            intact_parity_shards.push((k + p, p_slice));
        }
    }

    if intact_data_indices.len() + intact_parity_shards.len() < k {
        return Ok(None);
    }

    let mut available_indices = Vec::with_capacity(k);
    let mut available_buffers = Vec::with_capacity(k);
    let needed_data_count = k.saturating_sub(intact_parity_shards.len());
    let data_shards_to_use = needed_data_count.max(intact_data_indices.len().min(k));

    for &d in &intact_data_indices[..data_shards_to_use] {
        let start = d as u64 * slice_size as u64;
        let len = (payload_len.saturating_sub(start)).min(slice_size as u64) as usize;
        file.seek(SeekFrom::Start(start))
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let mut d_slice = vec![0u8; slice_size];
        file.read_exact(&mut d_slice[..len])
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
        available_indices.push(d);
        available_buffers.push(d_slice);
    }

    for (p_idx, p_data) in intact_parity_shards {
        if available_buffers.len() == k {
            break;
        }
        available_indices.push(p_idx);
        available_buffers.push(p_data);
    }

    Ok(Some(StreamingRepairShards {
        available_indices,
        available_buffers,
    }))
}

fn write_and_verify_streaming_repair(
    file: &mut std::fs::File,
    payload_len: u64,
    k: usize,
    slice_size: usize,
    missing_indices: &[usize],
    reconstructed: &[Vec<u8>],
    expected_root_hash: &[u8; 32],
) -> Result<bool, TTZipStatus> {
    for (m_idx, &missing_col) in missing_indices.iter().enumerate() {
        let start = missing_col as u64 * slice_size as u64;
        let len = (payload_len.saturating_sub(start)).min(slice_size as u64) as usize;
        file.seek(SeekFrom::Start(start))
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
        file.write_all(&reconstructed[m_idx][..len])
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    }
    file.flush()
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;

    file.seek(SeekFrom::Start(0))
        .map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let mut verify_hasher = FastSha256::new();
    let mut slice_buf = vec![0u8; slice_size];
    for d in 0..k {
        let start = d as u64 * slice_size as u64;
        let len = (payload_len.saturating_sub(start)).min(slice_size as u64) as usize;
        file.read_exact(&mut slice_buf[..len])
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
        verify_hasher.update(&slice_buf[..len]);
    }

    Ok(verify_hasher.finalize() == *expected_root_hash)
}

/// Repairs an archive file in-place using streaming verification and Cauchy RS repair.
pub fn repair_archive_file_streaming(file_path: &Path) -> Result<bool, TTZipStatus> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(file_path)
        .map_err(|_| TTZipStatus::ErrFileNotFound)?;

    let Some(info) = inspect_recovery_record_reader(&mut file)? else {
        return Ok(false);
    };

    let payload_len = info.protected_payload_length;
    let file_len = file
        .metadata()
        .map_err(|_| TTZipStatus::ErrOpenFailed)?
        .len();
    let (k, m, slice_size) = (info.data_slices_count, info.parity_slices_count, info.slice_size);
    if payload_len > file_len || !is_valid_fec_geometry(k, m, slice_size) {
        return Ok(false);
    }

    let scan = scan_streaming_data_slices(&mut file, payload_len, k, slice_size)?;
    if scan.missing_indices.is_empty() && scan.initial_hash == info.root_hash {
        return Ok(true);
    }
    if scan.missing_indices.len() > m {
        return Ok(false);
    }

    let Some(shards) = gather_streaming_repair_shards(
        &mut file,
        payload_len,
        k,
        m,
        slice_size,
        &scan.intact_data_indices,
    )?
    else {
        return Ok(false);
    };

    let reconstructed = reconstruct_missing_slices(
        k,
        m,
        slice_size,
        &shards.available_indices,
        &shards.available_buffers,
        &scan.missing_indices,
    )?;

    write_and_verify_streaming_repair(
        &mut file,
        payload_len,
        k,
        slice_size,
        &scan.missing_indices,
        &reconstructed,
        &info.root_hash,
    )
}

/// Repairs an archive file by streaming copy from `damaged_path` to `repaired_path` and in-place reconstructing the copy.
/// Strictly enforces the read-only contract on `damaged_path`.
pub fn repair_archive_file_to(damaged_path: &Path, repaired_path: &Path) -> Result<bool, TTZipStatus> {
    if !damaged_path.exists() {
        return Err(TTZipStatus::ErrFileNotFound);
    }
    let mut src = std::fs::File::open(damaged_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let mut dst = std::fs::File::create(repaired_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    std::io::copy(&mut src, &mut dst).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    dst.flush().map_err(|_| TTZipStatus::ErrOpenFailed)?;
    drop(src);
    drop(dst);

    match repair_archive_file_streaming(repaired_path) {
        Ok(true) => Ok(true),
        other => {
            let _ = std::fs::remove_file(repaired_path);
            other
        }
    }
}

/// In-place repair wrapper.
pub fn repair_archive_file(file_path: &Path) -> Result<bool, TTZipStatus> {
    repair_archive_file_streaming(file_path)
}
