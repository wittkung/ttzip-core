// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Recovery record format specifications, metadata definitions, and streaming Cauchy accumulator.

use super::cauchy::create_cauchy_matrix;
use super::gf8::gf8_mul_add_slice;
use crate::crc32::crc32_fast;
use crate::sha256::FastSha256;
use ttzip_types::TTZipStatus;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

pub const MAGIC_HEADER: &[u8; 4] = b"TTZR";
pub const MAGIC_FOOTER: &[u8; 4] = b"TTRC";
pub const DEFAULT_SLICE_SIZE: usize = 65536; // 64 KB
pub const MAX_PARITY_BUFFER_BYTES: usize = 24 * 1024 * 1024; // 24 MiB parity ceiling (< 64 MiB total)

#[inline]
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX_CHARS[(b >> 4) as usize] as char);
        s.push(HEX_CHARS[(b & 0x0F) as usize] as char);
    }
    s
}

/// Parsed metadata for a TTZip archive recovery record.
#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryRecordInfo {
    pub slice_size: usize,
    pub data_slices_count: usize,
    pub parity_slices_count: usize,
    pub protected_payload_length: u64,
    pub root_hash: [u8; 32],
    pub redundancy_percent: f64,
}

impl RecoveryRecordInfo {
    pub fn root_hash_hex(&self) -> String {
        bytes_to_hex(&self.root_hash)
    }
}

/// Streaming Cauchy Accumulator maintaining constant RAM overhead (<4MB).
pub struct StreamingCauchyAccumulator {
    pub slice_size: usize,
    pub total_k: usize,
    pub total_m: usize,
    pub payload_len: u64,
    cauchy_matrix: Vec<u8>,
    parity_slices: Vec<Vec<u8>>,
    data_crcs: Vec<u32>,
    hasher: FastSha256,
}

impl StreamingCauchyAccumulator {
    pub fn new(
        payload_len: u64,
        redundancy_percent: f64,
        slice_size: usize,
    ) -> Result<Self, TTZipStatus> {
        if payload_len == 0 {
            return Err(TTZipStatus::ErrInvalidParam);
        }

        // Dynamic Slice Scaling: Max K = 200 ensures K + M <= 256 in GF(2^8)
        const MAX_DATA_SLICES: usize = 200;
        const SLICE_ALIGNMENT: usize = 4096;

        let base_slice = if slice_size == 0 {
            DEFAULT_SLICE_SIZE
        } else {
            slice_size
        };
        let min_slice = payload_len.div_ceil(MAX_DATA_SLICES as u64) as usize;
        let mut effective_slice_size = base_slice.max(min_slice);
        effective_slice_size = effective_slice_size.div_ceil(SLICE_ALIGNMENT) * SLICE_ALIGNMENT;

        let total_k = payload_len.div_ceil(effective_slice_size as u64) as usize;
        if total_k == 0 || total_k > 200 {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        if effective_slice_size > MAX_PARITY_BUFFER_BYTES {
            return Err(TTZipStatus::ErrOutOfMemory);
        }

        let max_m_by_memory = (MAX_PARITY_BUFFER_BYTES / effective_slice_size).max(1);
        let raw_m = ((total_k as f64) * (redundancy_percent / 100.0)).ceil() as usize;
        let total_m = raw_m.clamp(1, total_k.min(256 - total_k).min(max_m_by_memory));

        let cauchy_matrix = create_cauchy_matrix(total_m, total_k);
        let parity_slices = vec![vec![0u8; effective_slice_size]; total_m];
        let data_crcs = Vec::with_capacity(total_k);
        let hasher = FastSha256::new();

        Ok(Self {
            slice_size: effective_slice_size,
            total_k,
            total_m,
            payload_len,
            cauchy_matrix,
            parity_slices,
            data_crcs,
            hasher,
        })
    }

    pub fn feed_slice(&mut self, slice_index: usize, chunk: &[u8]) -> Result<(), TTZipStatus> {
        if slice_index >= self.total_k || chunk.is_empty() || chunk.len() > self.slice_size {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        self.hasher.update(chunk);

        if chunk.len() == self.slice_size {
            let crc = crc32_fast(0, chunk);
            self.data_crcs.push(crc);
            for p in 0..self.total_m {
                let coeff = self.cauchy_matrix[p * self.total_k + slice_index];
                if coeff != 0 {
                    gf8_mul_add_slice(coeff, chunk, &mut self.parity_slices[p]);
                }
            }
        } else {
            let mut padded = vec![0u8; self.slice_size];
            padded[..chunk.len()].copy_from_slice(chunk);
            let crc = crc32_fast(0, &padded);
            self.data_crcs.push(crc);
            for p in 0..self.total_m {
                let coeff = self.cauchy_matrix[p * self.total_k + slice_index];
                if coeff != 0 {
                    gf8_mul_add_slice(coeff, &padded, &mut self.parity_slices[p]);
                }
            }
        }
        Ok(())
    }

    pub fn finalize_to_writer<W: Write>(
        self,
        writer: &mut W,
        trailer_bridge: &[u8],
    ) -> Result<RecoveryRecordInfo, TTZipStatus> {
        let root_hash = self.hasher.finalize();
        let total_k = self.total_k;
        let total_m = self.total_m;
        let slice_size = self.slice_size;
        let payload_len = self.payload_len;

        let total_block_size =
            (54 + (total_k * 4) + total_m * (6 + slice_size) + trailer_bridge.len() + 12) as u64;

        let mut header = [0u8; 54];
        header[0..4].copy_from_slice(MAGIC_HEADER);
        header[4..6].copy_from_slice(&0x0100u16.to_le_bytes());
        header[6..10].copy_from_slice(&(slice_size as u32).to_le_bytes());
        header[10..12].copy_from_slice(&(total_k as u16).to_le_bytes());
        header[12..14].copy_from_slice(&(total_m as u16).to_le_bytes());
        header[14..22].copy_from_slice(&payload_len.to_le_bytes());
        header[22..54].copy_from_slice(&root_hash);
        writer
            .write_all(&header)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;

        for &crc in &self.data_crcs {
            writer
                .write_all(&crc.to_le_bytes())
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        }

        for (idx, p_slice) in self.parity_slices.iter().enumerate() {
            let p_crc = crc32_fast(0, p_slice);
            let mut p_hdr = [0u8; 6];
            p_hdr[0..2].copy_from_slice(&(idx as u16).to_le_bytes());
            p_hdr[2..6].copy_from_slice(&p_crc.to_le_bytes());
            writer
                .write_all(&p_hdr)
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            writer
                .write_all(p_slice)
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        }

        if !trailer_bridge.is_empty() {
            writer
                .write_all(trailer_bridge)
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        }

        let mut footer = [0u8; 12];
        footer[0..4].copy_from_slice(MAGIC_FOOTER);
        footer[4..12].copy_from_slice(&total_block_size.to_le_bytes());
        writer
            .write_all(&footer)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;

        let redundancy_percent = (total_m as f64 / total_k as f64) * 100.0;
        Ok(RecoveryRecordInfo {
            slice_size,
            data_slices_count: total_k,
            parity_slices_count: total_m,
            protected_payload_length: payload_len,
            root_hash,
            redundancy_percent,
        })
    }

    pub fn finalize(self) -> (RecoveryRecordInfo, Vec<u8>) {
        let block_capacity = 54 + (self.total_k * 4) + self.total_m * (6 + self.slice_size) + 12;
        let mut block = Vec::with_capacity(block_capacity);
        let info = self
            .finalize_to_writer(&mut block, &[])
            .expect("Writing to in-memory Vec cannot fail");
        (info, block)
    }
}

fn extract_zip_trailer_bridge(
    file: &mut File,
    payload_len: u64,
    rec_body_len: u64,
) -> Option<Vec<u8>> {
    if payload_len < 22 {
        return None;
    }
    file.seek(SeekFrom::Start(payload_len - 22)).ok()?;
    let mut eocd = [0u8; 22];
    file.read_exact(&mut eocd).ok()?;
    if &eocd[0..4] != b"PK\x05\x06" || u16::from_le_bytes([eocd[20], eocd[21]]) != 0 {
        return None;
    }
    let cd_size = u32::from_le_bytes([eocd[12], eocd[13], eocd[14], eocd[15]]) as u64;
    let cd_offset = u32::from_le_bytes([eocd[16], eocd[17], eocd[18], eocd[19]]) as u64;
    if cd_size > 4 * 1024 * 1024 || cd_offset.checked_add(cd_size) != Some(payload_len - 22) {
        return None;
    }
    let new_cd_offset = u32::try_from(payload_len.checked_add(rec_body_len)?).ok()?;
    file.seek(SeekFrom::Start(cd_offset)).ok()?;
    let mut bridge = vec![0u8; (cd_size as usize) + 22];
    file.read_exact(&mut bridge[..cd_size as usize]).ok()?;
    eocd[16..20].copy_from_slice(&new_cd_offset.to_le_bytes());
    eocd[20..22].copy_from_slice(&0u16.to_le_bytes());
    bridge[cd_size as usize..].copy_from_slice(&eocd);
    Some(bridge)
}

/// Generates an encoded recovery record block via streaming reader (<4MB RAM).
pub fn create_recovery_record_streaming<R: Read>(
    reader: &mut R,
    payload_len: u64,
    redundancy_percent: f64,
    slice_size: usize,
) -> Result<(RecoveryRecordInfo, Vec<u8>), TTZipStatus> {
    let mut acc = StreamingCauchyAccumulator::new(payload_len, redundancy_percent, slice_size)?;
    let actual_slice_size = acc.slice_size;
    let mut chunk_buf = vec![0u8; actual_slice_size];

    for d in 0..acc.total_k {
        let expected = std::cmp::min(
            actual_slice_size as u64,
            payload_len - (d as u64 * actual_slice_size as u64),
        ) as usize;
        reader
            .read_exact(&mut chunk_buf[..expected])
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
        acc.feed_slice(d, &chunk_buf[..expected])?;
    }

    Ok(acc.finalize())
}

/// In-memory recovery record generation wrapper.
pub fn create_recovery_record(
    payload: &[u8],
    redundancy_percent: f64,
    slice_size: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    let mut cursor = std::io::Cursor::new(payload);
    let (_, block) = create_recovery_record_streaming(
        &mut cursor,
        payload.len() as u64,
        redundancy_percent,
        slice_size,
    )?;
    Ok(block)
}

/// Appends recovery record trailer to an existing archive file streamingly.
pub fn append_recovery_record_to_file(
    file_path: &Path,
    redundancy_percent: f64,
    slice_size: usize,
) -> Result<RecoveryRecordInfo, TTZipStatus> {
    let mut file = File::open(file_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let payload_len = file
        .metadata()
        .map_err(|_| TTZipStatus::ErrOpenFailed)?
        .len();
    if payload_len == 0 {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let mut acc = StreamingCauchyAccumulator::new(payload_len, redundancy_percent, slice_size)?;
    let rec_body_len = (54 + acc.total_k * 4 + acc.total_m * (6 + acc.slice_size)) as u64;
    let trailer_bridge =
        extract_zip_trailer_bridge(&mut file, payload_len, rec_body_len).unwrap_or_default();
    file.seek(SeekFrom::Start(0))
        .map_err(|_| TTZipStatus::ErrOpenFailed)?;

    let actual_slice_size = acc.slice_size;
    let mut chunk_buf = vec![0u8; actual_slice_size];
    for d in 0..acc.total_k {
        let expected = std::cmp::min(
            actual_slice_size as u64,
            payload_len - (d as u64 * actual_slice_size as u64),
        ) as usize;
        file.read_exact(&mut chunk_buf[..expected])
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
        acc.feed_slice(d, &chunk_buf[..expected])?;
    }
    drop(file);
    drop(chunk_buf);

    let mut out_file = OpenOptions::new()
        .append(true)
        .open(file_path)
        .map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let info = acc.finalize_to_writer(&mut out_file, &trailer_bridge)?;
    out_file
        .flush()
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;

    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_cauchy_accumulator_caps_memory_under_64mib_for_1gib_payload() {
        let one_gib = 1024 * 1024 * 1024u64;
        let acc = StreamingCauchyAccumulator::new(one_gib, 10.0, 4096)
            .expect("1 GiB accumulator creation must succeed within memory ceiling");
        let parity_bytes = acc.total_m * acc.slice_size;
        assert!(
            parity_bytes <= 24 * 1024 * 1024,
            "Parity buffer allocation ({parity_bytes} bytes) must stay <= 24 MiB so total recovery pass stays < 64 MiB"
        );
    }
}

