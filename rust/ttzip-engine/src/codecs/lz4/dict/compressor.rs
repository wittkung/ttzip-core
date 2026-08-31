// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zero-copy attached dictionary block compressor supporting dual-tiered small/large block engines.

use crate::codecs::lz4::block::lz4_compress_bound;
use crate::codecs::lz4::dict::preloaded::Lz4PreloadedDict;
use crate::codecs::lz4::hash::lz4_hash4;
use crate::codecs::lz4::matchfinder::{
    LASTLITERALS, LZ4_DISTANCE_MAX, LZ4_HASH_LOG, MFLIMIT, MINMATCH,
};
use crate::types::TTZipStatus;

// MARK: - Memory Read Helpers

#[inline(always)]
fn read_u32(src: &[u8], pos: usize) -> u32 {
    let b: [u8; 4] = src[pos..pos + 4].try_into().unwrap();
    u32::from_le_bytes(b)
}

#[inline(always)]
fn read_u64(src: &[u8], pos: usize) -> u64 {
    let b: [u8; 8] = src[pos..pos + 8].try_into().unwrap();
    u64::from_le_bytes(b)
}

// MARK: - Low-Level Sequence Serialization

#[inline(always)]
fn emit_sequence(
    dst: &mut [u8],
    dst_pos: &mut usize,
    literals: &[u8],
    offset: u16,
    match_len: usize,
) -> Result<(), TTZipStatus> {
    let lit_len = literals.len();
    let match_code = match_len.saturating_sub(MINMATCH);

    let lit_token = if lit_len >= 15 { 15u8 } else { lit_len as u8 };
    let match_token = if match_code >= 15 { 15u8 } else { match_code as u8 };
    let token = (lit_token << 4) | match_token;

    let extra_lit_bytes = if lit_len >= 15 { (lit_len - 15) / 255 + 1 } else { 0 };
    let extra_match_bytes = if match_code >= 15 { (match_code - 15) / 255 + 1 } else { 0 };
    let required_len = 1 + extra_lit_bytes + lit_len + 2 + extra_match_bytes;

    if *dst_pos + required_len > dst.len() {
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    dst[*dst_pos] = token;
    *dst_pos += 1;

    if lit_len >= 15 {
        let mut rem = lit_len - 15;
        while rem >= 255 {
            dst[*dst_pos] = 255;
            *dst_pos += 1;
            rem -= 255;
        }
        dst[*dst_pos] = rem as u8;
        *dst_pos += 1;
    }

    if lit_len > 0 {
        dst[*dst_pos..*dst_pos + lit_len].copy_from_slice(literals);
        *dst_pos += lit_len;
    }

    dst[*dst_pos..*dst_pos + 2].copy_from_slice(&offset.to_le_bytes());
    *dst_pos += 2;

    if match_code >= 15 {
        let mut rem = match_code - 15;
        while rem >= 255 {
            dst[*dst_pos] = 255;
            *dst_pos += 1;
            rem -= 255;
        }
        dst[*dst_pos] = rem as u8;
        *dst_pos += 1;
    }

    Ok(())
}

#[inline(always)]
fn emit_last_literals(
    dst: &mut [u8],
    dst_pos: &mut usize,
    literals: &[u8],
) -> Result<(), TTZipStatus> {
    let lit_len = literals.len();
    let lit_token = if lit_len >= 15 { 15u8 } else { lit_len as u8 };
    let token = lit_token << 4;

    let extra_lit_bytes = if lit_len >= 15 { (lit_len - 15) / 255 + 1 } else { 0 };
    let required_len = 1 + extra_lit_bytes + lit_len;

    if *dst_pos + required_len > dst.len() {
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    dst[*dst_pos] = token;
    *dst_pos += 1;

    if lit_len >= 15 {
        let mut rem = lit_len - 15;
        while rem >= 255 {
            dst[*dst_pos] = 255;
            *dst_pos += 1;
            rem -= 255;
        }
        dst[*dst_pos] = rem as u8;
        *dst_pos += 1;
    }

    if lit_len > 0 {
        dst[*dst_pos..*dst_pos + lit_len].copy_from_slice(literals);
        *dst_pos += lit_len;
    }

    Ok(())
}

// MARK: - Attached Dictionary Compressor

/// Zero-copy dictionary attached block compressor supporting dual-tiered small/large block strategies.
#[derive(Debug, Clone, Default)]
pub struct Lz4DictCompressor<'a> {
    dict: Option<&'a Lz4PreloadedDict>,
    acceleration: i32,
}

impl<'a> Lz4DictCompressor<'a> {
    /// Creates a new attached dictionary compressor with default acceleration (1).
    pub const fn new() -> Self {
        Self {
            dict: None,
            acceleration: 1,
        }
    }

    /// Creates a compressor attached to a specific preloaded dictionary.
    pub const fn with_dictionary(dict: &'a Lz4PreloadedDict) -> Self {
        Self {
            dict: Some(dict),
            acceleration: 1,
        }
    }

    /// Attaches a preloaded dictionary without copying or allocating.
    pub fn attach_dictionary(&mut self, dict: &'a Lz4PreloadedDict) -> &mut Self {
        self.dict = Some(dict);
        self
    }

    /// Configures the acceleration factor (1..=100).
    pub fn with_acceleration(&mut self, acceleration: i32) -> &mut Self {
        self.acceleration = acceleration.clamp(1, 100);
        self
    }

    /// Compresses `src` into `dst` with attached dictionary acceleration.
    pub fn compress(&self, src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
        let src_len = src.len();
        if src_len == 0 {
            return Ok(0);
        }
        if src_len > i32::MAX as usize || dst.len() > i32::MAX as usize {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        if src_len < MFLIMIT {
            let mut dst_pos = 0;
            emit_last_literals(dst, &mut dst_pos, src)?;
            return Ok(dst_pos);
        }

        let accel = self.acceleration.clamp(1, 100);

        match self.dict {
            Some(dict) => {
                if src_len <= 4096 {
                    compress_small_block_two_level(src, dst, dict, accel)
                } else {
                    compress_large_block_single_addressing(src, dst, dict, accel)
                }
            }
            None => {
                crate::codecs::lz4::matchfinder::Lz4FastCompressor::new()
                    .with_acceleration(accel)
                    .compress(src, dst)
            }
        }
    }

    /// Compresses `src` into a newly allocated `Vec<u8>`.
    pub fn compress_to_vec(&self, src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
        if src.is_empty() {
            return Ok(Vec::new());
        }
        let bound = lz4_compress_bound(src.len());
        let mut out = vec![0u8; bound];
        let written = self.compress(src, &mut out)?;
        out.truncate(written);
        Ok(out)
    }
}

// MARK: - Small Block Two-Level Match Engine (<= 4KB)

fn compress_small_block_two_level(
    src: &[u8],
    dst: &mut [u8],
    dict: &Lz4PreloadedDict,
    acceleration: i32,
) -> Result<usize, TTZipStatus> {
    let src_len = src.len();
    let dict_slice = dict.effective_slice();
    let dict_len = dict_slice.len();

    let mut local_table = [0u16; 4096];
    let local_mask = local_table.len() - 1;

    let mut dst_pos = 0;
    let mut anchor = 0usize;
    let search_limit = src_len - MFLIMIT;
    let match_limit = src_len - LASTLITERALS;
    let mut step_counter = 1usize;

    let first_h = lz4_hash4(read_u32(src, 0), LZ4_HASH_LOG) as usize;
    local_table[first_h & local_mask] = 1;
    let mut ip = 1usize;

    'search: while ip < search_limit {
        let mut forward_ip = ip;
        let mut match_offset = 0u16;
        let mut match_len = 0usize;
        let mut match_start = 0usize;

        loop {
            let seq = read_u32(src, forward_ip);
            let h = lz4_hash4(seq, LZ4_HASH_LOG) as usize;

            // 1. Level 1: Check Local Table
            let local_val = local_table[h & local_mask] as usize;
            local_table[h & local_mask] = (forward_ip + 1) as u16;

            let mut found_match = false;

            if local_val > 0 {
                let local_pos = local_val - 1;
                if forward_ip > local_pos
                    && (forward_ip - local_pos) <= LZ4_DISTANCE_MAX
                    && read_u32(src, local_pos) == seq
                {
                    let mut ms = forward_ip;
                    let mut mr = local_pos;
                    while ms > anchor && mr > 0 && src[ms - 1] == src[mr - 1] {
                        ms -= 1;
                        mr -= 1;
                    }

                    let mut fc = forward_ip + MINMATCH;
                    let mut rc = local_pos + MINMATCH;
                    while fc + 8 <= match_limit {
                        let diff = read_u64(src, fc) ^ read_u64(src, rc);
                        if diff == 0 {
                            fc += 8;
                            rc += 8;
                        } else {
                            let zeros = if cfg!(target_endian = "little") {
                                diff.trailing_zeros()
                            } else {
                                diff.leading_zeros()
                            };
                            fc += (zeros >> 3) as usize;
                            break;
                        }
                    }
                    while fc < match_limit && src[fc] == src[rc] {
                        fc += 1;
                        rc += 1;
                    }

                    match_start = ms;
                    match_len = fc - ms;
                    match_offset = (ms - mr) as u16;
                    ip = fc;
                    found_match = true;
                }
            }

            // 2. Level 2: Fallback to Preloaded Dict Table
            if !found_match && dict_len >= MINMATCH {
                let dict_val = dict.dict_table()[h] as usize;
                if dict_val > 0 {
                    let d_pos = dict_val - 1;
                    if d_pos + MINMATCH <= dict_len {
                        let dist = forward_ip + (dict_len - d_pos);
                        if dist <= LZ4_DISTANCE_MAX && read_u32(dict_slice, d_pos) == seq {
                            let mut ms = forward_ip;
                            let mut dr = d_pos;
                            while ms > anchor && dr > 0 && src[ms - 1] == dict_slice[dr - 1] {
                                ms -= 1;
                                dr -= 1;
                            }

                            let mut fc = forward_ip + MINMATCH;
                            let mut dc = d_pos + MINMATCH;
                            while fc < match_limit && dc < dict_len && src[fc] == dict_slice[dc] {
                                fc += 1;
                                dc += 1;
                            }

                            if dc == dict_len {
                                let mut sc = 0usize;
                                while fc < match_limit && sc < ms && src[fc] == src[sc] {
                                    fc += 1;
                                    sc += 1;
                                }
                            }

                            match_start = ms;
                            match_len = fc - ms;
                            match_offset = (ms + (dict_len - dr)) as u16;
                            ip = fc;
                            found_match = true;
                        }
                    }
                }
            }

            if found_match {
                break;
            }

            let step = (step_counter >> 6) + (acceleration as usize);
            forward_ip += step;
            step_counter += 1;

            if forward_ip >= search_limit {
                break 'search;
            }
        }

        emit_sequence(
            dst,
            &mut dst_pos,
            &src[anchor..match_start],
            match_offset,
            match_len,
        )?;
        anchor = ip;

        if ip >= search_limit {
            break 'search;
        }

        let h_prev = lz4_hash4(read_u32(src, ip - 2), LZ4_HASH_LOG) as usize;
        local_table[h_prev & local_mask] = (ip - 1) as u16;
        step_counter = 1;
    }

    if anchor < src_len {
        emit_last_literals(dst, &mut dst_pos, &src[anchor..src_len])?;
    }

    Ok(dst_pos)
}

// MARK: - Large Block Single Addressing Match Engine (> 4KB)

fn compress_large_block_single_addressing(
    src: &[u8],
    dst: &mut [u8],
    dict: &Lz4PreloadedDict,
    acceleration: i32,
) -> Result<usize, TTZipStatus> {
    let src_len = src.len();
    let dict_slice = dict.effective_slice();
    let dict_len = dict_slice.len();

    let mut table = *dict.dict_table();

    let mut dst_pos = 0;
    let mut anchor = 0usize;
    let search_limit = src_len - MFLIMIT;
    let match_limit = src_len - LASTLITERALS;
    let mut step_counter = 1usize;

    let mut ip = 0usize;

    #[inline(always)]
    fn get_vbyte(dict: &[u8], dict_len: usize, src: &[u8], vpos: usize) -> u8 {
        if vpos < dict_len {
            dict[vpos]
        } else {
            src[vpos - dict_len]
        }
    }

    #[inline(always)]
    fn get_vu32(dict: &[u8], dict_len: usize, src: &[u8], vpos: usize) -> u32 {
        if vpos + 4 <= dict_len {
            read_u32(dict, vpos)
        } else if vpos >= dict_len {
            read_u32(src, vpos - dict_len)
        } else {
            let mut buf = [0u8; 4];
            for i in 0..4 {
                buf[i] = get_vbyte(dict, dict_len, src, vpos + i);
            }
            u32::from_le_bytes(buf)
        }
    }

    'search: while ip < search_limit {
        let mut forward_ip = ip;
        let v_match_ref;

        loop {
            let v_fip = dict_len + forward_ip;
            let seq = read_u32(src, forward_ip);
            let h = lz4_hash4(seq, LZ4_HASH_LOG) as usize;
            let stored_v = table[h] as usize;
            table[h] = (v_fip + 1) as u32;

            if stored_v > 0 {
                let candidate_v = stored_v - 1;
                if (1..=LZ4_DISTANCE_MAX).contains(&v_fip.saturating_sub(candidate_v))
                    && get_vu32(dict_slice, dict_len, src, candidate_v) == seq
                {
                    v_match_ref = candidate_v;
                    ip = forward_ip;
                    break;
                }
            }

            let step = (step_counter >> 6) + (acceleration as usize);
            forward_ip += step;
            step_counter += 1;

            if forward_ip >= search_limit {
                break 'search;
            }
        }

        let mut v_ms = dict_len + ip;
        let mut v_mr = v_match_ref;
        let v_anchor = dict_len + anchor;

        while v_ms > v_anchor
            && v_mr > 0
            && get_vbyte(dict_slice, dict_len, src, v_ms - 1)
                == get_vbyte(dict_slice, dict_len, src, v_mr - 1)
        {
            v_ms -= 1;
            v_mr -= 1;
        }

        let mut v_fc = dict_len + ip + MINMATCH;
        let mut v_rc = v_match_ref + MINMATCH;
        let v_match_limit = dict_len + match_limit;

        while v_fc < v_match_limit
            && get_vbyte(dict_slice, dict_len, src, v_fc)
                == get_vbyte(dict_slice, dict_len, src, v_rc)
        {
            v_fc += 1;
            v_rc += 1;
        }

        let match_len = v_fc - v_ms;
        let offset = (v_ms - v_mr) as u16;
        let ms = v_ms - dict_len;

        emit_sequence(dst, &mut dst_pos, &src[anchor..ms], offset, match_len)?;
        anchor = v_fc - dict_len;
        ip = anchor;

        if ip >= search_limit {
            break 'search;
        }

        let h_prev = lz4_hash4(read_u32(src, ip - 2), LZ4_HASH_LOG) as usize;
        table[h_prev] = (dict_len + ip - 1) as u32;
        step_counter = 1;
    }

    if anchor < src_len {
        emit_last_literals(dst, &mut dst_pos, &src[anchor..src_len])?;
    }

    Ok(dst_pos)
}
