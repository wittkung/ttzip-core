// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! LZFSE 4-State Interleaved FSE Reverse Bitstream Decoder.
//!
//! Implements Apple's Finite State Entropy (FSE) reverse bitstream decoder,
//! 4-way interleaved literal decoding pipeline, and fused Literal-Match-Distance (LMD)
//! execution engine with strict bound checking and zero panic guarantee.

use crate::codecs::lzfse::fse::FseValueDecoderEntry;
use crate::types::TTZipStatus;

// MARK: - Reverse Bitstream Buffer

/// 64-bit reverse input bitstream buffer for LZFSE decoding.
///
/// Bitstreams in LZFSE are stored backwards in memory and decoded from right to left.
/// The accumulator holds between 56 and 63 valid bits to eliminate unpredictable branches
/// during symbol extraction.
#[derive(Debug)]
pub struct FseInStream<'a> {
    /// 64-bit accumulator containing input bits.
    pub accum: u64,
    /// Number of valid bits in accumulator (maintained in `56..63`).
    pub accum_nbits: i32,
    /// Source buffer reference.
    pub buf: &'a [u8],
    /// Current reverse read cursor (points to the next byte boundary to load).
    pub cursor: usize,
    /// Stream validity indicator.
    pub ok: bool,
}

impl<'a> FseInStream<'a> {
    /// Initializes a reverse bitstream stream from payload and header bit count.
    ///
    /// `nbits` is the initial bit offset (in `[-7, 0]`).
    pub fn init(nbits: i32, payload: &'a [u8]) -> Result<Self, TTZipStatus> {
        let mut cursor = payload.len();
        let (accum, accum_nbits) = if nbits != 0 {
            if cursor < 8 {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            cursor -= 8;
            let bytes: [u8; 8] = payload[cursor..cursor + 8]
                .try_into()
                .map_err(|_| TTZipStatus::ErrCorruptHeader)?;
            let val = u64::from_le_bytes(bytes);
            let bits = nbits + 64;
            (val, bits)
        } else {
            if cursor < 7 {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            cursor -= 7;
            let mut bytes = [0u8; 8];
            bytes[..7].copy_from_slice(&payload[cursor..cursor + 7]);
            let val = u64::from_le_bytes(bytes) & 0x00ff_ffff_ffff_ffff;
            let bits = nbits + 56;
            (val, bits)
        };

        if !(56..64).contains(&accum_nbits) {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        if (accum >> accum_nbits) != 0 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        Ok(Self {
            accum,
            accum_nbits,
            buf: payload,
            cursor,
            ok: true,
        })
    }

    /// Extracts `n` bits from the accumulator without branch hazards.
    #[inline(always)]
    pub fn pull(&mut self, n: u8) -> u64 {
        if n == 0 {
            return 0;
        }
        let n_i32 = n as i32;
        if n_i32 > self.accum_nbits || !self.ok {
            self.ok = false;
            return 0;
        }

        self.accum_nbits -= n_i32;
        let result = self.accum >> self.accum_nbits;
        let mask = if self.accum_nbits >= 64 {
            !0u64
        } else if self.accum_nbits <= 0 {
            0
        } else {
            (1u64 << self.accum_nbits) - 1
        };
        self.accum &= mask;
        result
    }

    /// Refills the accumulator by pulling up to 8 bytes backwards from the input buffer.
    ///
    /// Ensures `accum_nbits` is returned to the range `56..63`.
    #[inline(always)]
    pub fn flush(&mut self) {
        let nbits = (63 - self.accum_nbits) & !7;
        if nbits <= 0 {
            return;
        }
        let nbytes = (nbits >> 3) as usize;
        if self.cursor < nbytes {
            self.ok = false;
            return;
        }

        self.cursor -= nbytes;
        let incoming = if self.cursor + 8 <= self.buf.len() {
            let bytes: [u8; 8] = self.buf[self.cursor..self.cursor + 8]
                .try_into()
                .unwrap_or([0u8; 8]);
            u64::from_le_bytes(bytes)
        } else {
            let mut tmp = [0u8; 8];
            let avail = self.buf.len().saturating_sub(self.cursor);
            let copy_len = avail.min(8);
            tmp[..copy_len].copy_from_slice(&self.buf[self.cursor..self.cursor + copy_len]);
            u64::from_le_bytes(tmp)
        };

        let mask = if nbits >= 64 {
            !0u64
        } else {
            (1u64 << nbits) - 1
        };
        self.accum = (self.accum << nbits) | (incoming & mask);
        self.accum_nbits += nbits;
    }

    /// Validates stream consistency and absence of overrun.
    #[inline(always)]
    pub fn check(&self) -> bool {
        self.ok
            && self.accum_nbits >= 0
            && (self.accum_nbits >= 64 || (self.accum >> self.accum_nbits) == 0)
    }
}

// MARK: - FSE Decoding Primitives

/// Decodes a single literal symbol from the stream and advances the FSE state.
#[inline(always)]
pub fn fse_decode(state: &mut u16, table: &[i32; 1024], stream: &mut FseInStream) -> u8 {
    let s = *state as usize;
    if s >= table.len() {
        stream.ok = false;
        return 0;
    }
    let e = table[s];
    let k = (e & 0xff) as u8;
    let pull_bits = stream.pull(k) as u16;
    let delta = (e >> 16) as u16;
    *state = delta.wrapping_add(pull_bits);
    ((e >> 8) & 0xff) as u8
}

/// Decodes a fused L/M/D value from the stream and advances the FSE state.
#[inline(always)]
pub fn fse_value_decode(
    state: &mut u16,
    table: &[FseValueDecoderEntry],
    stream: &mut FseInStream,
) -> i32 {
    let s = *state as usize;
    if s >= table.len() {
        stream.ok = false;
        return 0;
    }
    let entry = table[s];
    let state_and_value_bits = stream.pull(entry.total_bits) as u32;
    let next_state_bits = (state_and_value_bits >> entry.value_bits) as u16;
    *state = (entry.delta as u16).wrapping_add(next_state_bits);

    let mask = if entry.value_bits >= 32 {
        !0u32
    } else if entry.value_bits == 0 {
        0
    } else {
        (1u32 << entry.value_bits) - 1
    };
    let value_bits = (state_and_value_bits & mask) as i32;
    entry.vbase.wrapping_add(value_bits)
}

// MARK: - 4-Way Interleaved Literal Decoder

/// Decodes literals using 4 independent interleaved FSE state machines in a single loop.
///
/// `dst.len()` must be a multiple of 4 bytes as per the LZFSE block specification.
pub fn decode_literals_4way(
    stream: &mut FseInStream,
    table: &[i32; 1024],
    states: &mut [u16; 4],
    dst: &mut [u8],
) -> Result<(), TTZipStatus> {
    if !dst.len().is_multiple_of(4) {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    for chunk in dst.chunks_exact_mut(4) {
        stream.flush();
        if !stream.check() {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        chunk[0] = fse_decode(&mut states[0], table, stream);
        chunk[1] = fse_decode(&mut states[1], table, stream);
        chunk[2] = fse_decode(&mut states[2], table, stream);
        chunk[3] = fse_decode(&mut states[3], table, stream);
    }

    if !stream.check() {
        return Err(TTZipStatus::ErrExtractionFailed);
    }
    Ok(())
}

// MARK: - Fused LMD Stream Execution Engine

/// Triad of FSE value decoding tables for LZFSE LMD (Literal, Match, Distance) operations.
#[derive(Debug, Clone, Copy)]
pub struct FseLmdTables<'a> {
    pub l_table: &'a [FseValueDecoderEntry],
    pub m_table: &'a [FseValueDecoderEntry],
    pub d_table: &'a [FseValueDecoderEntry],
}

/// Mutable state registers for LMD execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FseLmdState {
    pub l_state: u16,
    pub m_state: u16,
    pub d_state: u16,
}

/// Decodes and executes an LZFSE Literal-Match-Distance (LMD) command stream into `out`.
///
/// Fully supports multi-block history references up to the full uncompressed stream bounds.
/// Returns the total number of uncompressed bytes produced by this block.
pub fn decode_lmd_stream(
    stream: &mut FseInStream,
    tables: &FseLmdTables<'_>,
    state: &mut FseLmdState,
    n_matches: usize,
    literals: &[u8],
    out: &mut Vec<u8>,
    expected_raw_len: usize,
) -> Result<usize, TTZipStatus> {
    let start_len = out.len();
    let target_len = start_len
        .checked_add(expected_raw_len)
        .ok_or(TTZipStatus::ErrExtractionFailed)?;
    out.reserve(expected_raw_len);

    let mut lit_cursor = 0usize;
    let mut d: i32 = -1;

    for _ in 0..n_matches {
        stream.flush();
        if !stream.check() {
            return Err(TTZipStatus::ErrExtractionFailed);
        }

        let l = fse_value_decode(&mut state.l_state, tables.l_table, stream);
        if l < 0 {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        let l = l as usize;

        let m = fse_value_decode(&mut state.m_state, tables.m_table, stream);
        if m < 0 {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        let m = m as usize;

        let new_d = fse_value_decode(&mut state.d_state, tables.d_table, stream);
        if new_d != 0 {
            d = new_d;
        }

        if !stream.check() {
            return Err(TTZipStatus::ErrExtractionFailed);
        }

        // Copy literal segment
        if l > 0 {
            let lit_end = lit_cursor
                .checked_add(l)
                .ok_or(TTZipStatus::ErrExtractionFailed)?;
            if lit_end > literals.len() {
                return Err(TTZipStatus::ErrExtractionFailed);
            }
            if out
                .len()
                .checked_add(l)
                .ok_or(TTZipStatus::ErrExtractionFailed)?
                > target_len
            {
                return Err(TTZipStatus::ErrExtractionFailed);
            }
            out.extend_from_slice(&literals[lit_cursor..lit_end]);
            lit_cursor = lit_end;
        }

        // Copy match segment (supports overlapping run-length copies and cross-block history)
        if m > 0 {
            if d <= 0 || (d as usize) > out.len() {
                return Err(TTZipStatus::ErrExtractionFailed);
            }
            if out
                .len()
                .checked_add(m)
                .ok_or(TTZipStatus::ErrExtractionFailed)?
                > target_len
            {
                return Err(TTZipStatus::ErrExtractionFailed);
            }
            let match_start = out.len() - (d as usize);
            for i in 0..m {
                let byte = out[match_start + i];
                out.push(byte);
            }
        }
    }

    let written = out.len() - start_len;
    if written != expected_raw_len {
        return Err(TTZipStatus::ErrExtractionFailed);
    }

    Ok(written)
}

