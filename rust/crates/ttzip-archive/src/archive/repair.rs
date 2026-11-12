// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Hardware NEON-accelerated ZIP/TAR damaged stream TOC reconstruction and corrupt archive self-healing engine.

use crate::types::TTZipStatus;
use crate::zip::writer::{assemble_zip_archive, ZipCompressedItem};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

/// Finds next ZIP Local File Header signature (`PK\x03\x04`).
pub fn find_next_pk_signature(data: &[u8], start: usize) -> Option<usize> {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        find_next_pk_neon(data, start)
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        find_next_pk_scalar(data, start)
    }
}

#[cfg(target_arch = "aarch64")]
unsafe fn find_next_pk_neon(data: &[u8], start: usize) -> Option<usize> {
    unsafe {
        use core::arch::aarch64::*;
        let target = vdupq_n_u8(b'P');
        let mut i = start;
        let len = data.len();

        while i + 16 <= len {
            let chunk = vld1q_u8(data.as_ptr().add(i));
            let cmp = vceqq_u8(chunk, target);
            let max_val = vmaxvq_u8(cmp);
            if max_val != 0 {
                for j in 0..16 {
                    let idx = i + j;
                    if idx + 4 <= len
                        && data[idx] == b'P'
                        && data[idx + 1] == b'K'
                        && data[idx + 2] == 0x03
                        && data[idx + 3] == 0x04
                    {
                        return Some(idx);
                    }
                }
            }
            i += 16;
        }

        find_next_pk_scalar(data, i)
    }
}

#[inline]
fn find_next_pk_scalar(data: &[u8], mut i: usize) -> Option<usize> {
    let len = data.len();
    while i + 4 <= len {
        if data[i] == b'P' && data[i + 1] == b'K' && data[i + 2] == 0x03 && data[i + 3] == 0x04 {
            return Some(i);
        }
        i += 1;
    }
    None
}

struct RawLfhHeader {
    flags: u16,
    method: u16,
    mdate_dos: u16,
    crc32: u32,
    comp_size: usize,
    uncomp_size: u64,
    clean_name: String,
    payload_start: usize,
}

struct ResolvedPayloadSpan {
    comp_size: usize,
    uncomp_size: u64,
    crc32: u32,
    next_offset: usize,
}

/// Scans damaged ZIP stream and reconstructs a healthy ZIP archive at `repaired_path`.
pub fn repair_damaged_zip(damaged_path: &Path, repaired_path: &Path) -> Result<usize, TTZipStatus> {
    let file_data = fs::read(damaged_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let total_len = file_data.len();
    let mut offset = 0;
    let mut salvaged_items = Vec::new();

    while let Some(pk_pos) = find_next_pk_signature(&file_data, offset) {
        if pk_pos + 30 > total_len {
            break;
        }

        let Some(lfh) = parse_raw_lfh(&file_data, pk_pos) else {
            offset = pk_pos + 4;
            continue;
        };

        let is_dir = lfh.clean_name.ends_with('/');
        let max_payload_len = total_len.saturating_sub(lfh.payload_start);
        let has_bit3 = (lfh.flags & 0x0008) != 0;

        let span = if !is_dir && (has_bit3 || lfh.comp_size == 0) {
            resolve_streaming_payload_span(&file_data, &lfh, max_payload_len)
        } else {
            ResolvedPayloadSpan {
                comp_size: lfh.comp_size,
                uncomp_size: lfh.uncomp_size,
                crc32: lfh.crc32,
                next_offset: lfh.payload_start + lfh.comp_size,
            }
        };

        let actual_comp_len = span.comp_size.min(max_payload_len);
        let payload = file_data[lfh.payload_start..lfh.payload_start + actual_comp_len].to_vec();
        let epoch_secs = dos_date_to_approx_epoch(lfh.mdate_dos);

        salvaged_items.push(ZipCompressedItem {
            rel_path: lfh.clean_name,
            uncompressed_size: if span.uncomp_size > 0 {
                span.uncomp_size
            } else {
                payload.len() as u64
            },
            compressed_size: payload.len() as u64,
            crc32: span.crc32,
            compression_method: lfh.method,
            actual_method: lfh.method,
            aes_strength: 0,
            payload,
            mtime_epoch_secs: epoch_secs,
            mode: if is_dir { 0o755 } else { 0o644 },
            is_directory: is_dir,
            is_encrypted: false,
        });

        offset = span.next_offset.max(pk_pos + 4);
    }

    if salvaged_items.is_empty() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let count = salvaged_items.len();
    let rebuilt_bytes = assemble_zip_archive(&salvaged_items)?;
    fs::write(repaired_path, &rebuilt_bytes).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    Ok(count)
}

fn parse_raw_lfh(file_data: &[u8], pk_pos: usize) -> Option<RawLfhHeader> {
    let total_len = file_data.len();
    let flags = u16::from_le_bytes([file_data[pk_pos + 6], file_data[pk_pos + 7]]);
    let method = u16::from_le_bytes([file_data[pk_pos + 8], file_data[pk_pos + 9]]);
    let mdate_dos = u16::from_le_bytes([file_data[pk_pos + 12], file_data[pk_pos + 13]]);
    let crc32 = u32::from_le_bytes([
        file_data[pk_pos + 14],
        file_data[pk_pos + 15],
        file_data[pk_pos + 16],
        file_data[pk_pos + 17],
    ]);
    let comp_size = u32::from_le_bytes([
        file_data[pk_pos + 18],
        file_data[pk_pos + 19],
        file_data[pk_pos + 20],
        file_data[pk_pos + 21],
    ]) as usize;
    let uncomp_size = u32::from_le_bytes([
        file_data[pk_pos + 22],
        file_data[pk_pos + 23],
        file_data[pk_pos + 24],
        file_data[pk_pos + 25],
    ]) as u64;
    let fn_len = u16::from_le_bytes([file_data[pk_pos + 26], file_data[pk_pos + 27]]) as usize;
    let extra_len = u16::from_le_bytes([file_data[pk_pos + 28], file_data[pk_pos + 29]]) as usize;

    let header_len = 30 + fn_len + extra_len;
    if fn_len == 0 || pk_pos + header_len > total_len {
        return None;
    }

    let fn_bytes = &file_data[pk_pos + 30..pk_pos + 30 + fn_len];
    let clean_name = String::from_utf8_lossy(fn_bytes).trim_matches('\0').to_string();
    if clean_name.is_empty() {
        return None;
    }

    Some(RawLfhHeader {
        flags,
        method,
        mdate_dos,
        crc32,
        comp_size,
        uncomp_size,
        clean_name,
        payload_start: pk_pos + header_len,
    })
}

fn resolve_streaming_payload_span(
    file_data: &[u8],
    lfh: &RawLfhHeader,
    max_payload_len: usize,
) -> ResolvedPayloadSpan {
    if let Some(span) = probe_data_descriptor(file_data, lfh.payload_start) {
        return span;
    }
    if lfh.method == 8 {
        if let Some(span) = probe_deflate_stream(file_data, lfh.payload_start) {
            return span;
        }
    }
    if let Some(next_pk) = find_next_pk_signature(file_data, lfh.payload_start + 4) {
        ResolvedPayloadSpan {
            comp_size: next_pk - lfh.payload_start,
            uncomp_size: lfh.uncomp_size,
            crc32: lfh.crc32,
            next_offset: next_pk,
        }
    } else {
        ResolvedPayloadSpan {
            comp_size: max_payload_len,
            uncomp_size: lfh.uncomp_size,
            crc32: lfh.crc32,
            next_offset: file_data.len(),
        }
    }
}

fn probe_data_descriptor(file_data: &[u8], payload_start: usize) -> Option<ResolvedPayloadSpan> {
    let total_len = file_data.len();
    let dd_limit = total_len.saturating_sub(16);
    let mut p = payload_start;

    while p <= dd_limit {
        if file_data[p..p + 4] == [0x50, 0x4B, 0x07, 0x08] {
            let cand_len = p - payload_start;
            let dd_crc = u32::from_le_bytes([file_data[p + 4], file_data[p + 5], file_data[p + 6], file_data[p + 7]]);
            let dd_comp = u32::from_le_bytes([file_data[p + 8], file_data[p + 9], file_data[p + 10], file_data[p + 11]]) as usize;
            let dd_uncomp = u32::from_le_bytes([file_data[p + 12], file_data[p + 13], file_data[p + 14], file_data[p + 15]]) as u64;

            if dd_comp == cand_len {
                return Some(ResolvedPayloadSpan {
                    comp_size: cand_len,
                    uncomp_size: dd_uncomp,
                    crc32: dd_crc,
                    next_offset: p + 16,
                });
            }

            if p + 24 <= total_len {
                let dd_comp64 = u64::from_le_bytes([
                    file_data[p + 8], file_data[p + 9], file_data[p + 10], file_data[p + 11],
                    file_data[p + 12], file_data[p + 13], file_data[p + 14], file_data[p + 15],
                ]) as usize;
                let dd_uncomp64 = u64::from_le_bytes([
                    file_data[p + 16], file_data[p + 17], file_data[p + 18], file_data[p + 19],
                    file_data[p + 20], file_data[p + 21], file_data[p + 22], file_data[p + 23],
                ]);
                if dd_comp64 == cand_len {
                    return Some(ResolvedPayloadSpan {
                        comp_size: cand_len,
                        uncomp_size: dd_uncomp64,
                        crc32: dd_crc,
                        next_offset: p + 24,
                    });
                }
            }
        }
        p += 1;
    }
    None
}

fn probe_deflate_stream(file_data: &[u8], payload_start: usize) -> Option<ResolvedPayloadSpan> {
    let total_len = file_data.len();
    let mut decompressor = flate2::Decompress::new(false);
    let mut sink = [0u8; 16384];
    let mut in_pos = payload_start;
    let mut total_out = 0u64;
    let mut calc_crc = crc32fast::Hasher::new();
    let mut stream_ended = false;

    while in_pos < total_len {
        let before_in = decompressor.total_in();
        let before_out = decompressor.total_out();
        let chunk_end = total_len.min(in_pos + 8192);
        let chunk = &file_data[in_pos..chunk_end];

        let Ok(status) = decompressor.decompress(chunk, &mut sink, flate2::FlushDecompress::None) else {
            break;
        };

        let consumed_in = (decompressor.total_in() - before_in) as usize;
        let produced_out = (decompressor.total_out() - before_out) as usize;
        in_pos += consumed_in;
        total_out += produced_out as u64;
        calc_crc.update(&sink[..produced_out]);

        if status == flate2::Status::StreamEnd {
            stream_ended = true;
            break;
        }
        if consumed_in == 0 && produced_out == 0 {
            break;
        }
    }

    if !stream_ended {
        return None;
    }

    let comp_size = decompressor.total_in() as usize;
    let payload_end = payload_start + comp_size;
    let next_offset = skip_trailing_data_descriptor(file_data, payload_end, comp_size);

    Some(ResolvedPayloadSpan {
        comp_size,
        uncomp_size: total_out,
        crc32: calc_crc.finalize(),
        next_offset,
    })
}

fn skip_trailing_data_descriptor(file_data: &[u8], payload_end: usize, comp_size: usize) -> usize {
    let total_len = file_data.len();
    if payload_end + 16 <= total_len && file_data[payload_end..payload_end + 4] == [0x50, 0x4B, 0x07, 0x08] {
        return payload_end + 16;
    }
    if payload_end + 12 <= total_len {
        let cand_comp = u32::from_le_bytes([
            file_data[payload_end + 4],
            file_data[payload_end + 5],
            file_data[payload_end + 6],
            file_data[payload_end + 7],
        ]) as usize;
        if cand_comp == comp_size {
            return payload_end + 12;
        }
    }
    payload_end
}

fn dos_date_to_approx_epoch(mdate_dos: u16) -> u32 {
    let yr = ((mdate_dos >> 9) & 0x7F) as u32 + 1980;
    let mo = ((mdate_dos >> 5) & 0x0F) as u32;
    let dy = (mdate_dos & 0x1F) as u32;
    if yr >= 1980 && (1..=12).contains(&mo) && (1..=31).contains(&dy) {
        (yr - 1970) * 31536000 + mo * 2592000 + dy * 86400
    } else {
        1700000000u32
    }
}

/// Scans damaged TAR stream and reconstructs a healthy TAR archive at `repaired_path`.
pub fn repair_damaged_tar(damaged_path: &Path, repaired_path: &Path) -> Result<usize, TTZipStatus> {
    let mut file = File::open(damaged_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).map_err(|_| TTZipStatus::ErrOpenFailed)?;

    let mut out_file = File::create(repaired_path).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    let mut offset = 0;
    let total_len = buffer.len();
    let mut salvaged_count = 0;

    while offset + 512 <= total_len {
        let block = &buffer[offset..offset + 512];
        let Ok(block_arr) = <&[u8; 512]>::try_from(block) else {
            offset += 512;
            continue;
        };

        if !block_arr.iter().all(|&b| b == 0) && crate::archive::tar::header::verify_tar_checksum(block_arr) {
            let advance = write_salvaged_tar_entry(&mut out_file, &buffer, offset, block_arr)?;
            salvaged_count += 1;
            offset += advance;
        } else {
            offset += 512;
        }
    }

    out_file.write_all(&[0u8; 1024]).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    out_file.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;

    if salvaged_count == 0 {
        Err(TTZipStatus::ErrCorruptHeader)
    } else {
        Ok(salvaged_count)
    }
}

fn write_salvaged_tar_entry<W: Write>(
    out_file: &mut W,
    buffer: &[u8],
    offset: usize,
    block: &[u8; 512],
) -> Result<usize, TTZipStatus> {
    let file_size = crate::archive::tar::header::parse_octal(&block[124..136]).unwrap_or(0) as usize;
    let payload_blocks = file_size.div_ceil(512);
    let available_payload = buffer.len().saturating_sub(offset + 512);
    let write_payload_len = file_size.min(available_payload);

    out_file.write_all(block).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    if write_payload_len > 0 {
        let payload = &buffer[offset + 512..offset + 512 + write_payload_len];
        out_file.write_all(payload).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        let pad = (512 - (write_payload_len % 512)) % 512;
        if pad > 0 {
            out_file.write_all(&vec![0u8; pad]).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        }
    }
    Ok(512 + (payload_blocks * 512))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_repair_damaged_zip_corrupt_tail() {
        let temp_dir_obj = tempfile::Builder::new()
            .prefix("ttzip_test_repair_zip_")
            .tempdir()
            .expect("tempdir");
        let temp_dir = temp_dir_obj.path();

        let good_item = ZipCompressedItem {
            rel_path: "document.txt".to_string(),
            uncompressed_size: 13,
            compressed_size: 13,
            crc32: 0x12345678,
            compression_method: 0,
            actual_method: 0,
            aes_strength: 0,
            payload: b"Hello World!!".to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        };

        let zip_bytes = assemble_zip_archive(&[good_item]).expect("assemble");
        // Truncate archive right after the payload to destroy the Central Directory & EOCD
        let truncated = zip_bytes[..30 + 12 + 13].to_vec();

        let damaged_file = temp_dir.join("damaged.zip");
        let repaired_file = temp_dir.join("repaired.zip");
        fs::write(&damaged_file, &truncated).expect("write damaged");

        let count = repair_damaged_zip(&damaged_file, &repaired_file).expect("repair");
        assert_eq!(count, 1);
        assert!(repaired_file.exists());
    }
}
