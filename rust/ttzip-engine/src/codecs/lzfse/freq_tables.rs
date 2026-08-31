// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Apple `LZFSE` normalized frequency tables, symbol counts, and Huffman codec.
//!
//! Handles serialization and deserialization of L, M, D, and literal frequency tables.

use crate::types::TTZipStatus;

// MARK: - Constants

/// Number of literal length (`L`) symbols in LZFSE FSE coder.
pub const LZFSE_ENCODE_L_SYMBOLS: usize = 20;

/// Number of match length (`M`) symbols in LZFSE FSE coder.
pub const LZFSE_ENCODE_M_SYMBOLS: usize = 20;

/// Number of match distance (`D`) symbols in LZFSE FSE coder.
pub const LZFSE_ENCODE_D_SYMBOLS: usize = 64;

/// Number of literal byte symbols in LZFSE FSE coder.
pub const LZFSE_ENCODE_LITERAL_SYMBOLS: usize = 256;

/// Total number of frequency table entries across L, M, D, and Literal tables.
pub const LZFSE_FREQ_TOTAL_SYMBOLS: usize = LZFSE_ENCODE_L_SYMBOLS
    + LZFSE_ENCODE_M_SYMBOLS
    + LZFSE_ENCODE_D_SYMBOLS
    + LZFSE_ENCODE_LITERAL_SYMBOLS; // 360

/// Number of FSE states for `L` stream (6 bits).
pub const LZFSE_ENCODE_L_STATES: usize = 64;

/// Number of FSE states for `M` stream (6 bits).
pub const LZFSE_ENCODE_M_STATES: usize = 64;

/// Number of FSE states for `D` stream (8 bits).
pub const LZFSE_ENCODE_D_STATES: usize = 256;

/// Number of FSE states for `Literal` stream (10 bits).
pub const LZFSE_ENCODE_LITERAL_STATES: usize = 1024;

/// Maximum number of matches allowed per LZFSE compressed block (20-bit wire field).
pub const LZFSE_MATCHES_PER_BLOCK: usize = 1 << 20;

/// Maximum number of literals allowed per LZFSE compressed block (20-bit wire field).
pub const LZFSE_LITERALS_PER_BLOCK: usize = 1 << 20;

/// Minimum fixed size in bytes for a `bvx2` compressed block header.
pub const LZFSE_V2_HEADER_FIXED_SIZE: usize = 32;

// MARK: - Frequency Table Definitions & Huffman Codec

/// Normalized frequency tables for L, M, D, and literal streams.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LzfseFreqTables {
    /// FSE frequency table for literal lengths (`L`), 20 symbols, sum <= 64.
    pub l_freq: [u16; LZFSE_ENCODE_L_SYMBOLS],
    /// FSE frequency table for match lengths (`M`), 20 symbols, sum <= 64.
    pub m_freq: [u16; LZFSE_ENCODE_M_SYMBOLS],
    /// FSE frequency table for match distances (`D`), 64 symbols, sum <= 256.
    pub d_freq: [u16; LZFSE_ENCODE_D_SYMBOLS],
    /// FSE frequency table for raw literal bytes, 256 symbols, sum <= 1024.
    pub literal_freq: [u16; LZFSE_ENCODE_LITERAL_SYMBOLS],
}

impl Default for LzfseFreqTables {
    fn default() -> Self {
        Self {
            l_freq: [0u16; LZFSE_ENCODE_L_SYMBOLS],
            m_freq: [0u16; LZFSE_ENCODE_M_SYMBOLS],
            d_freq: [0u16; LZFSE_ENCODE_D_SYMBOLS],
            literal_freq: [0u16; LZFSE_ENCODE_LITERAL_SYMBOLS],
        }
    }
}

impl LzfseFreqTables {
    /// Validates that frequency table sums do not exceed state capacities.
    pub fn validate(&self) -> Result<(), TTZipStatus> {
        let sum_l: usize = self.l_freq.iter().map(|&v| v as usize).sum();
        let sum_m: usize = self.m_freq.iter().map(|&v| v as usize).sum();
        let sum_d: usize = self.d_freq.iter().map(|&v| v as usize).sum();
        let sum_lit: usize = self.literal_freq.iter().map(|&v| v as usize).sum();

        if sum_l > LZFSE_ENCODE_L_STATES
            || sum_m > LZFSE_ENCODE_M_STATES
            || sum_d > LZFSE_ENCODE_D_STATES
            || sum_lit > LZFSE_ENCODE_LITERAL_STATES
        {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        Ok(())
    }

    /// Flattens all 360 frequency symbols in order: `l_freq`, `m_freq`, `d_freq`, `literal_freq`.
    #[must_use]
    pub fn to_symbols(&self) -> [u16; LZFSE_FREQ_TOTAL_SYMBOLS] {
        let mut symbols = [0u16; LZFSE_FREQ_TOTAL_SYMBOLS];
        symbols[0..20].copy_from_slice(&self.l_freq);
        symbols[20..40].copy_from_slice(&self.m_freq);
        symbols[40..104].copy_from_slice(&self.d_freq);
        symbols[104..360].copy_from_slice(&self.literal_freq);
        symbols
    }

    /// Populates frequency tables from a flattened array of 360 symbols and validates sums.
    pub fn from_symbols(symbols: &[u16; LZFSE_FREQ_TOTAL_SYMBOLS]) -> Result<Self, TTZipStatus> {
        let mut tables = Self::default();
        tables.l_freq.copy_from_slice(&symbols[0..20]);
        tables.m_freq.copy_from_slice(&symbols[20..40]);
        tables.d_freq.copy_from_slice(&symbols[40..104]);
        tables.literal_freq.copy_from_slice(&symbols[104..360]);
        tables.validate()?;
        Ok(tables)
    }
}

/// Lookup tables for decoding variable-length prefix Huffman frequency codes.
static FREQ_NBITS_TABLE: [u8; 32] = [
    2, 3, 2, 5, 2, 3, 2, 8, 2, 3, 2, 5, 2, 3, 2, 14, 2, 3, 2, 5, 2, 3, 2, 8, 2, 3, 2, 5, 2, 3,
    2, 14,
];

static FREQ_VALUE_TABLE: [i16; 32] = [
    0, 2, 1, 4, 0, 3, 1, -1, 0, 2, 1, 5, 0, 3, 1, -1, 0, 2, 1, 6, 0, 3, 1, -1, 0, 2, 1, 7, 0, 3,
    1, -1,
];

/// Encodes a single normalized frequency value into a variable-length Huffman prefix code.
///
/// Returns `(bits, nbits)` where bits are read from the LSB.
#[inline]
pub fn encode_v1_freq_value(value: u16) -> (u32, usize) {
    match value {
        0 => (0, 2),
        1 => (2, 2),
        2 => (1, 3),
        3 => (5, 3),
        4 => (3, 5),
        5 => (11, 5),
        6 => (19, 5),
        7 => (27, 5),
        8..=23 => (7 | (((value - 8) as u32) << 4), 8),
        24..=1047 => (15 | (((value - 24) as u32) << 4), 14),
        _ => (0, 0),
    }
}

/// Decodes a single normalized frequency value from the lower bits of `bits`.
///
/// Returns `(value, nbits)` on success.
#[inline]
pub fn decode_v1_freq_value(bits: u32) -> Result<(u16, usize), TTZipStatus> {
    let b = (bits & 31) as usize;
    let nbits = FREQ_NBITS_TABLE[b] as usize;
    if nbits == 8 {
        let val = 8 + (((bits >> 4) & 0x0F) as u16);
        Ok((val, 8))
    } else if nbits == 14 {
        let val = 24 + (((bits >> 4) & 0x3FF) as u16);
        Ok((val, 14))
    } else {
        let val = FREQ_VALUE_TABLE[b];
        if val < 0 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        Ok((val as u16, nbits))
    }
}

/// Serializes all 360 frequency table entries using variable-length Huffman codes into `dst`.
pub fn encode_v2_freq_tables(tables: &LzfseFreqTables, dst: &mut Vec<u8>) {
    let mut accum: u32 = 0;
    let mut accum_nbits: usize = 0;
    let symbols = tables.to_symbols();

    for &val in &symbols {
        let (bits, nbits) = encode_v1_freq_value(val);
        accum |= bits << accum_nbits;
        accum_nbits += nbits;

        while accum_nbits >= 8 {
            dst.push((accum & 0xFF) as u8);
            accum >>= 8;
            accum_nbits -= 8;
        }
    }

    if accum_nbits > 0 {
        dst.push((accum & 0xFF) as u8);
    }
}

/// Deserializes 360 frequency table entries from `src` using variable-length Huffman decoding.
///
/// Returns `(tables, bytes_consumed)` on success.
pub fn decode_v2_freq_tables(src: &[u8]) -> Result<(LzfseFreqTables, usize), TTZipStatus> {
    if src.is_empty() {
        return Ok((LzfseFreqTables::default(), 0));
    }

    let mut symbols = [0u16; LZFSE_FREQ_TOTAL_SYMBOLS];
    let mut accum: u32 = 0;
    let mut accum_nbits: usize = 0;
    let mut src_idx = 0;

    for i in 0..LZFSE_FREQ_TOTAL_SYMBOLS {
        while accum_nbits < 14 && src_idx < src.len() {
            accum |= (src[src_idx] as u32) << accum_nbits;
            accum_nbits += 8;
            src_idx += 1;
        }

        let (val, nbits) = decode_v1_freq_value(accum)?;
        if nbits > accum_nbits {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        symbols[i] = val;
        accum >>= nbits;
        accum_nbits -= nbits;
    }

    let unconsumed_bytes = accum_nbits / 8;
    let bytes_consumed = src_idx.saturating_sub(unconsumed_bytes);

    let tables = LzfseFreqTables::from_symbols(&symbols)?;
    Ok((tables, bytes_consumed))
}
