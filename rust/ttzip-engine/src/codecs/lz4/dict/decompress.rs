// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe LZ4 external read-only dictionary (`ExtDict`) decompressor with cross-boundary match reconstruction.

use crate::codecs::lz4::matchfinder::{LZ4_DISTANCE_MAX, MINMATCH};
use crate::types::TTZipStatus;

// MARK: - Safe ExtDict Decompressor

/// Decompresses an LZ4 block with an external disjoint read-only dictionary (`ExtDict`).
///
/// Features cross-segment addressing and seamless dual-segment boundary reconstruction
/// when match sequences span across dictionary tail and current decompressed block.
pub fn lz4_decompress_safe_ext_dict(
    src: &[u8],
    dst: &mut [u8],
    dict: &[u8],
) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    if src.len() > i32::MAX as usize || dst.len() > i32::MAX as usize {
        return Err(TTZipStatus::ErrInvalidParam);
    }
    if dst.is_empty() {
        return Err(TTZipStatus::ErrExtractionFailed);
    }

    let dict_len = dict.len();
    let mut ip = 0usize;
    let mut op = 0usize;

    while ip < src.len() {
        // 1. Read Token
        let token = src[ip];
        ip += 1;
        let lit_len_token = (token >> 4) as usize;
        let match_len_token = (token & 0x0F) as usize;

        // 2. Decode Literal Length
        let mut lit_len = lit_len_token;
        if lit_len == 15 {
            loop {
                if ip >= src.len() {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
                let s = src[ip] as usize;
                ip += 1;
                lit_len = lit_len
                    .checked_add(s)
                    .ok_or(TTZipStatus::ErrCorruptHeader)?;
                if s != 255 {
                    break;
                }
            }
        }

        // 3. Bounds Check & Copy Literals
        if lit_len > 0 {
            if ip + lit_len > src.len() || op + lit_len > dst.len() {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            dst[op..op + lit_len].copy_from_slice(&src[ip..ip + lit_len]);
            ip += lit_len;
            op += lit_len;
        }

        // 4. End of Block Check
        if ip == src.len() {
            return Ok(op);
        }

        // 5. Read 2-byte Match Offset
        if ip + 2 > src.len() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let offset = u16::from_le_bytes([src[ip], src[ip + 1]]) as usize;
        ip += 2;

        if offset == 0 {
            return Err(TTZipStatus::ErrInvalidOffset);
        }

        // 6. Decode Match Length
        let mut match_len = match_len_token + MINMATCH;
        if match_len_token == 15 {
            loop {
                if ip >= src.len() {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
                let s = src[ip] as usize;
                ip += 1;
                match_len = match_len
                    .checked_add(s)
                    .ok_or(TTZipStatus::ErrCorruptHeader)?;
                if s != 255 {
                    break;
                }
            }
        }

        if op + match_len > dst.len() {
            return Err(TTZipStatus::ErrExtractionFailed);
        }

        // 7. ExtDict Cross-Segment Match Reconstruction
        if offset <= op {
            // Case 1: Intra-block match (completely within dst buffer)
            let match_start = op - offset;
            if offset >= match_len {
                dst.copy_within(match_start..match_start + match_len, op);
            } else {
                for i in 0..match_len {
                    dst[op + i] = dst[match_start + i];
                }
            }
        } else {
            // Case 2 & 3: Match starts inside external dictionary
            let dict_offset = offset - op;
            if dict_offset > dict_len || offset > LZ4_DISTANCE_MAX {
                return Err(TTZipStatus::ErrInvalidOffset);
            }
            let dict_start = dict_len - dict_offset;
            let bytes_in_dict = dict_len - dict_start;

            if match_len <= bytes_in_dict {
                // Case 2: Intra-dictionary match (entirely within dictionary)
                dst[op..op + match_len].copy_from_slice(&dict[dict_start..dict_start + match_len]);
            } else {
                // Case 3: Cross-boundary match (dual-segment stitching: dict tail + dst prefix)
                dst[op..op + bytes_in_dict].copy_from_slice(&dict[dict_start..dict_len]);
                let rem_match = match_len - bytes_in_dict;
                let dst_target = op + bytes_in_dict;

                if bytes_in_dict >= rem_match {
                    dst.copy_within(0..rem_match, dst_target);
                } else {
                    for i in 0..rem_match {
                        dst[dst_target + i] = dst[i];
                    }
                }
            }
        }

        op += match_len;
    }

    Ok(op)
}

/// Decompresses an LZ4 block with external dictionary into a newly allocated `Vec<u8>`.
pub fn lz4_decompress_safe_ext_dict_to_vec(
    src: &[u8],
    uncompressed_len: usize,
    dict: &[u8],
) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() || uncompressed_len == 0 {
        return Ok(Vec::new());
    }
    let mut out = vec![0u8; uncompressed_len];
    let written = lz4_decompress_safe_ext_dict(src, &mut out, dict)?;
    if written != uncompressed_len {
        return Err(TTZipStatus::ErrExtractionFailed);
    }
    Ok(out)
}

/// Partially decompresses an LZ4 block with external dictionary until at least `target_output_size` bytes.
pub fn lz4_decompress_safe_ext_dict_partial(
    src: &[u8],
    dst: &mut [u8],
    dict: &[u8],
    target_output_size: usize,
) -> Result<usize, TTZipStatus> {
    if target_output_size == 0 {
        return Ok(0);
    }
    if src.is_empty() {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let written = lz4_decompress_safe_ext_dict(src, dst, dict)?;
    if written < target_output_size {
        return Err(TTZipStatus::ErrExtractionFailed);
    }
    Ok(written)
}
