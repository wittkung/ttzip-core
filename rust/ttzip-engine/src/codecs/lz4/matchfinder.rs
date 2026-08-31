// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, high-performance LZ4 `byU16`/`byU32` dual hash table matchfinder with
//! adaptive step acceleration and Catch-Up backward match extension.
//!
//! # Architecture & Algorithm
//!
//! 1. **Dual Hash Table Topologies (`TableType`)**:
//!    - `ByU16`: 16-bit compact hash table ($2^{15} = 32768$ slots, $64\,\text{KB}$ RAM).
//!      Automatically selected for input payloads $\le 64\,\text{KB}$, maximizing L1/L2 cache locality.
//!    - `ByU32`: 32-bit hash table ($2^{15} = 32768$ slots, $128\,\text{KB}$ RAM) for larger payloads.
//!
//! 2. **Adaptive Stepping Acceleration**:
//!    - When non-matching data is encountered, search step dynamically scales:
//!      $\text{step}(k) = \text{acceleration} + \lfloor k / 64 \rfloor$.
//!    - Bypasses low-entropy / uncompressible regions at ultra-high throughput.
//!
//! 3. **Catch-Up Backward Match Extension**:
//!    - Upon detecting a forward 4-byte match, scans backwards (`ip[-1] == match[-1]`),
//!      absorbing preceding unmatched literals into match length and boosting compression ratio.
//!
//! 4. **Standard LZ4 Block Compliance**:
//!    - Strict adherence to LZ4 Block format: Token, Literals, 16-bit LE Offset, Match Length,
//!      enforcing `MINMATCH = 4`, `MFLIMIT = 12`, and `LASTLITERALS = 5`.

use crate::codecs::lz4::block::lz4_compress_bound;
use crate::codecs::lz4::hash::lz4_hash4;
use crate::types::TTZipStatus;

// MARK: - Constants

/// Minimum match length required by the LZ4 compression format.
pub const MINMATCH: usize = 4;

/// Minimum forward limit: matches cannot start within the last 12 bytes of a block.
pub const MFLIMIT: usize = 12;

/// Last literals limit: every LZ4 block must end with at least 5 literal bytes.
pub const LASTLITERALS: usize = 5;

/// Input size threshold ($64\,\text{KB}$) for automatic selection of `ByU16` compact table.
pub const LZ4_64K_LIMIT: usize = 64 * 1024;

/// Maximum backward reference distance supported by standard LZ4 ($64\,\text{KB} - 1$).
pub const LZ4_DISTANCE_MAX: usize = 65535;

/// Hash log width ($2^{15} = 32768$ entries) for optimal cache hit ratio and minimal collisions.
pub const LZ4_HASH_LOG: u32 = 15;

/// Number of hash table slots ($32768$).
pub const LZ4_HASH_SIZE: usize = 1 << LZ4_HASH_LOG;

// MARK: - Table Type

/// Hash table index width mode for LZ4 fast compression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TableType {
    /// 16-bit compact hash table (32768 slots, $64\,\text{KB}$). Ideal for input $\le 64\,\text{KB}$.
    #[default]
    ByU16,
    /// 32-bit universal hash table (32768 slots, $128\,\text{KB}$).
    ByU32,
}

impl TableType {
    /// Automatically selects the most cache-friendly table type based on uncompressed input size.
    #[inline(always)]
    pub const fn auto_select(input_len: usize) -> Self {
        if input_len < 65536 {
            Self::ByU16
        } else {
            Self::ByU32
        }
    }

    /// Returns the hash log width ($2^{\text{log}}$ slots).
    #[inline(always)]
    pub const fn hash_log(self) -> u32 {
        LZ4_HASH_LOG
    }

    /// Returns total number of hash table entries ($32768$).
    #[inline(always)]
    pub const fn table_size(self) -> usize {
        LZ4_HASH_SIZE
    }
}

// MARK: - Fast Compressor Engine

/// High-performance LZ4 Fast Block Compressor with dual hash tables,
/// adaptive step acceleration, and Catch-Up backward match extension.
#[derive(Debug, Clone)]
pub struct Lz4FastCompressor {
    /// Optional table type override. If `None`, automatically selects based on input size.
    pub table_type: Option<TableType>,
    /// Acceleration factor ($1..=100$, default: 1). Higher values trade slight compression ratio for extreme speed.
    pub acceleration: i32,
}

impl Default for Lz4FastCompressor {
    fn default() -> Self {
        Self::new()
    }
}

impl Lz4FastCompressor {
    /// Creates a new compressor with default settings (acceleration = 1, auto table type).
    pub const fn new() -> Self {
        Self {
            table_type: None,
            acceleration: 1,
        }
    }

    /// Configures the acceleration factor ($1..=100$).
    pub const fn with_acceleration(mut self, acceleration: i32) -> Self {
        self.acceleration = acceleration;
        self
    }

    /// Forces a specific table type (`ByU16` or `ByU32`).
    pub const fn with_table_type(mut self, table_type: TableType) -> Self {
        self.table_type = Some(table_type);
        self
    }

    /// Compresses `src` into `dst` using LZ4 block encoding.
    ///
    /// Returns the number of compressed bytes written to `dst`.
    pub fn compress(&self, src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
        let accel = self.acceleration.clamp(1, 100);
        let table = self.table_type.unwrap_or_else(|| TableType::auto_select(src.len()));
        match table {
            TableType::ByU16 => compress_impl_u16(src, dst, accel),
            TableType::ByU32 => compress_impl_u32(src, dst, accel),
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

    /// Compresses `src` using the 16-bit compact hash table.
    pub fn compress_by_u16(src: &[u8], dst: &mut [u8], acceleration: i32) -> Result<usize, TTZipStatus> {
        compress_impl_u16(src, dst, acceleration.clamp(1, 100))
    }

    /// Compresses `src` using the 32-bit universal hash table.
    pub fn compress_by_u32(src: &[u8], dst: &mut [u8], acceleration: i32) -> Result<usize, TTZipStatus> {
        compress_impl_u32(src, dst, acceleration.clamp(1, 100))
    }

    /// Compresses `src` using automatic table selection and custom acceleration.
    pub fn compress_fast(src: &[u8], dst: &mut [u8], acceleration: i32) -> Result<usize, TTZipStatus> {
        let compressor = Self::new().with_acceleration(acceleration);
        compressor.compress(src, dst)
    }
}

// MARK: - Free Helper Functions

/// Compresses a memory slice using pure Rust LZ4 Fast algorithm.
#[inline]
pub fn lz4_compress_fast_rust(src: &[u8], dst: &mut [u8], acceleration: i32) -> Result<usize, TTZipStatus> {
    Lz4FastCompressor::compress_fast(src, dst, acceleration)
}

/// Compresses a memory slice into a `Vec<u8>` using pure Rust LZ4 Fast algorithm.
#[inline]
pub fn lz4_compress_fast_rust_to_vec(src: &[u8], acceleration: i32) -> Result<Vec<u8>, TTZipStatus> {
    Lz4FastCompressor::new()
        .with_acceleration(acceleration)
        .compress_to_vec(src)
}

// MARK: - Low-Level Sequence Serialization

/// Emits an LZ4 sequence (Token, Literals, Offset, Extra Match Length) into `dst`.
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

    // Compute token nibbles
    let lit_token = if lit_len >= 15 { 15u8 } else { lit_len as u8 };
    let match_token = if match_code >= 15 { 15u8 } else { match_code as u8 };
    let token = (lit_token << 4) | match_token;

    // Fast capacity bound check
    let extra_lit_bytes = if lit_len >= 15 { (lit_len - 15) / 255 + 1 } else { 0 };
    let extra_match_bytes = if match_code >= 15 { (match_code - 15) / 255 + 1 } else { 0 };
    let required_len = 1 + extra_lit_bytes + lit_len + 2 + extra_match_bytes;

    if *dst_pos + required_len > dst.len() {
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    // 1. Write Token
    dst[*dst_pos] = token;
    *dst_pos += 1;

    // 2. Write Extra Literal Length
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

    // 3. Write Literals
    if lit_len > 0 {
        dst[*dst_pos..*dst_pos + lit_len].copy_from_slice(literals);
        *dst_pos += lit_len;
    }

    // 4. Write Match Offset (16-bit Little-Endian)
    dst[*dst_pos..*dst_pos + 2].copy_from_slice(&offset.to_le_bytes());
    *dst_pos += 2;

    // 5. Write Extra Match Length
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

/// Emits the final trailing literal sequence (Token + Literals without Offset).
#[inline(always)]
fn emit_last_literals(dst: &mut [u8], dst_pos: &mut usize, literals: &[u8]) -> Result<(), TTZipStatus> {
    let lit_len = literals.len();
    let lit_token = if lit_len >= 15 { 15u8 } else { lit_len as u8 };
    let token = lit_token << 4;

    let extra_lit_bytes = if lit_len >= 15 { (lit_len - 15) / 255 + 1 } else { 0 };
    let required_len = 1 + extra_lit_bytes + lit_len;

    if *dst_pos + required_len > dst.len() {
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    // 1. Write Token
    dst[*dst_pos] = token;
    *dst_pos += 1;

    // 2. Write Extra Literal Length
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

    // 3. Write Literals
    if lit_len > 0 {
        dst[*dst_pos..*dst_pos + lit_len].copy_from_slice(literals);
        *dst_pos += lit_len;
    }

    Ok(())
}

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

// MARK: - ByU16 Compression Engine

fn compress_impl_u16(src: &[u8], dst: &mut [u8], acceleration: i32) -> Result<usize, TTZipStatus> {
    let src_len = src.len();
    if src_len == 0 {
        return Ok(0);
    }
    if src_len >= 65536 {
        return compress_impl_u32(src, dst, acceleration);
    }
    if src_len < MFLIMIT {
        let mut dst_pos = 0;
        emit_last_literals(dst, &mut dst_pos, src)?;
        return Ok(dst_pos);
    }

    // 16-bit hash table storing 1-based index. Initialized with 0.
    let mut table = [0u16; LZ4_HASH_SIZE];
    let mut dst_pos = 0;
    let mut anchor = 0usize;
    let search_limit = src_len - MFLIMIT;
    let match_limit = src_len - LASTLITERALS;
    let mut step_counter = 1usize;

    // Seed first position
    let first_hash = lz4_hash4(read_u32(src, 0), LZ4_HASH_LOG) as usize;
    table[first_hash] = 1; // 1-based indexing so 0 represents empty slot
    let mut ip = 1usize;

    'search: while ip < search_limit {
        let mut forward_ip = ip;
        let mut match_pos;

        // Step Search Loop
        loop {
            let seq = read_u32(src, forward_ip);
            let h = lz4_hash4(seq, LZ4_HASH_LOG) as usize;
            let stored_val = table[h] as usize;
            table[h] = (forward_ip + 1) as u16;

            if stored_val > 0 {
                match_pos = stored_val - 1;
                if (1..=LZ4_DISTANCE_MAX).contains(&forward_ip.saturating_sub(match_pos))
                    && read_u32(src, match_pos) == seq
                {
                    // Found 4-byte match!
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

        // Catch-Up Backward Match Extension
        let mut match_start = ip;
        let mut match_ref = match_pos;
        while match_start > anchor && match_ref > 0 && src[match_start - 1] == src[match_ref - 1] {
            match_start -= 1;
            match_ref -= 1;
        }

        // Fast Forward Match Extension
        let mut forward_cursor = ip + MINMATCH;
        let mut ref_cursor = match_pos + MINMATCH;

        // 8-byte unrolled word comparison
        while forward_cursor + 8 <= match_limit {
            let diff = read_u64(src, forward_cursor) ^ read_u64(src, ref_cursor);
            if diff == 0 {
                forward_cursor += 8;
                ref_cursor += 8;
            } else {
                let zeros = if cfg!(target_endian = "little") {
                    diff.trailing_zeros()
                } else {
                    diff.leading_zeros()
                };
                forward_cursor += (zeros >> 3) as usize;
                break;
            }
        }

        // Byte-by-byte comparison for remainder
        while forward_cursor < match_limit && src[forward_cursor] == src[ref_cursor] {
            forward_cursor += 1;
            ref_cursor += 1;
        }

        let match_len = forward_cursor - match_start;
        let offset = (match_start - match_ref) as u16;

        // Emit Sequence
        emit_sequence(dst, &mut dst_pos, &src[anchor..match_start], offset, match_len)?;
        anchor = forward_cursor;
        ip = forward_cursor;

        if ip >= search_limit {
            break 'search;
        }

        // Populate hash table with preceding byte
        if ip >= 2 {
            let h_prev = lz4_hash4(read_u32(src, ip - 2), LZ4_HASH_LOG) as usize;
            table[h_prev] = (ip - 1) as u16;
        }

        // Check if current position matches immediately
        let seq_curr = read_u32(src, ip);
        let h_curr = lz4_hash4(seq_curr, LZ4_HASH_LOG) as usize;
        let stored_curr = table[h_curr] as usize;
        table[h_curr] = (ip + 1) as u16;

        if stored_curr > 0 {
            let next_match = stored_curr - 1;
            if ip > next_match && (ip - next_match) <= LZ4_DISTANCE_MAX && read_u32(src, next_match) == seq_curr {
                match_pos = next_match;

                // Catch-Up
                let mut ms = ip;
                let mut mr = match_pos;
                while ms > anchor && mr > 0 && src[ms - 1] == src[mr - 1] {
                    ms -= 1;
                    mr -= 1;
                }

                let mut fc = ip + MINMATCH;
                let mut rc = match_pos + MINMATCH;
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

                let mlen = fc - ms;
                let off = (ms - mr) as u16;
                emit_sequence(dst, &mut dst_pos, &src[anchor..ms], off, mlen)?;
                anchor = fc;
                ip = fc;
            }
        }

        step_counter = 1;
    }

    // Emit Trailing Literals
    if anchor < src_len {
        emit_last_literals(dst, &mut dst_pos, &src[anchor..src_len])?;
    }

    Ok(dst_pos)
}

// MARK: - ByU32 Compression Engine

fn compress_impl_u32(src: &[u8], dst: &mut [u8], acceleration: i32) -> Result<usize, TTZipStatus> {
    let src_len = src.len();
    if src_len == 0 {
        return Ok(0);
    }
    if src_len < MFLIMIT {
        let mut dst_pos = 0;
        emit_last_literals(dst, &mut dst_pos, src)?;
        return Ok(dst_pos);
    }

    let mut table = [0u32; LZ4_HASH_SIZE];
    let mut dst_pos = 0;
    let mut anchor = 0usize;
    let search_limit = src_len - MFLIMIT;
    let match_limit = src_len - LASTLITERALS;
    let mut step_counter = 1usize;

    // Seed first position
    let first_hash = lz4_hash4(read_u32(src, 0), LZ4_HASH_LOG) as usize;
    table[first_hash] = 1;
    let mut ip = 1usize;

    'search: while ip < search_limit {
        let mut forward_ip = ip;
        let mut match_pos;

        // Step Search Loop
        loop {
            let seq = read_u32(src, forward_ip);
            let h = lz4_hash4(seq, LZ4_HASH_LOG) as usize;
            let stored_val = table[h] as usize;
            table[h] = (forward_ip + 1) as u32;

            if stored_val > 0 {
                match_pos = stored_val - 1;
                if (1..=LZ4_DISTANCE_MAX).contains(&forward_ip.saturating_sub(match_pos))
                    && read_u32(src, match_pos) == seq
                {
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

        // Catch-Up Backward Match Extension
        let mut match_start = ip;
        let mut match_ref = match_pos;
        while match_start > anchor && match_ref > 0 && src[match_start - 1] == src[match_ref - 1] {
            match_start -= 1;
            match_ref -= 1;
        }

        // Fast Forward Match Extension
        let mut forward_cursor = ip + MINMATCH;
        let mut ref_cursor = match_pos + MINMATCH;

        while forward_cursor + 8 <= match_limit {
            let diff = read_u64(src, forward_cursor) ^ read_u64(src, ref_cursor);
            if diff == 0 {
                forward_cursor += 8;
                ref_cursor += 8;
            } else {
                let zeros = if cfg!(target_endian = "little") {
                    diff.trailing_zeros()
                } else {
                    diff.leading_zeros()
                };
                forward_cursor += (zeros >> 3) as usize;
                break;
            }
        }

        while forward_cursor < match_limit && src[forward_cursor] == src[ref_cursor] {
            forward_cursor += 1;
            ref_cursor += 1;
        }

        let match_len = forward_cursor - match_start;
        let offset = (match_start - match_ref) as u16;

        // Emit Sequence
        emit_sequence(dst, &mut dst_pos, &src[anchor..match_start], offset, match_len)?;
        anchor = forward_cursor;
        ip = forward_cursor;

        if ip >= search_limit {
            break 'search;
        }

        // Populate hash table with preceding byte
        if ip >= 2 {
            let h_prev = lz4_hash4(read_u32(src, ip - 2), LZ4_HASH_LOG) as usize;
            table[h_prev] = (ip - 1) as u32;
        }

        // Check if current position matches immediately
        let seq_curr = read_u32(src, ip);
        let h_curr = lz4_hash4(seq_curr, LZ4_HASH_LOG) as usize;
        let stored_curr = table[h_curr] as usize;
        table[h_curr] = (ip + 1) as u32;

        if stored_curr > 0 {
            let next_match = stored_curr - 1;
            if ip > next_match && (ip - next_match) <= LZ4_DISTANCE_MAX && read_u32(src, next_match) == seq_curr {
                match_pos = next_match;

                // Catch-Up
                let mut ms = ip;
                let mut mr = match_pos;
                while ms > anchor && mr > 0 && src[ms - 1] == src[mr - 1] {
                    ms -= 1;
                    mr -= 1;
                }

                let mut fc = ip + MINMATCH;
                let mut rc = match_pos + MINMATCH;
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

                let mlen = fc - ms;
                let off = (ms - mr) as u16;
                emit_sequence(dst, &mut dst_pos, &src[anchor..ms], off, mlen)?;
                anchor = fc;
                ip = fc;
            }
        }

        step_counter = 1;
    }

    // Emit Trailing Literals
    if anchor < src_len {
        emit_last_literals(dst, &mut dst_pos, &src[anchor..src_len])?;
    }

    Ok(dst_pos)
}
