// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Finite State Entropy (tANS) tables, normalization, and bit-level encoding/decoding for LZFSE.

use super::tables::*;
use crate::types::TTZipStatus;

// MARK: - Types

/// Single symbol entry in FSE encoder table.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct FseEncoderEntry {
    pub s0: i16,
    pub k: i16,
    pub delta0: i16,
    pub delta1: i16,
}

/// Entry for one state in the FSE decoder table (32-bit representation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
pub struct FseDecoderEntry {
    pub k: i8,
    pub symbol: u8,
    pub delta: i16,
}

impl FseDecoderEntry {
    /// Packs entry into 32-bit integer matching C memory layout (k | symbol << 8 | delta << 16).
    #[inline]
    pub fn to_packed_i32(&self) -> i32 {
        (self.k as u8 as i32)
            | ((self.symbol as i32) << 8)
            | ((self.delta as u16 as i32) << 16)
    }

    /// Unpacks entry from 32-bit integer matching C memory layout.
    #[inline]
    pub fn from_packed_i32(v: i32) -> Self {
        Self {
            k: (v & 0xFF) as i8,
            symbol: ((v >> 8) & 0xFF) as u8,
            delta: (v >> 16) as i16,
        }
    }
}

/// Entry for one state in the fused value decoder table (64-bit representation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
pub struct FseValueDecoderEntry {
    pub total_bits: u8,
    pub value_bits: u8,
    pub delta: i16,
    pub vbase: i32,
}

// MARK: - Table Validation

/// Verifies that the sum of symbol frequencies does not exceed total state count.
#[inline]
pub fn fse_check_freq(freq_table: &[u16], number_of_states: usize) -> Result<(), TTZipStatus> {
    let sum: usize = freq_table.iter().map(|&f| f as usize).sum();
    if sum > number_of_states {
        Err(TTZipStatus::ErrCorruptHeader)
    } else {
        Ok(())
    }
}

// MARK: - Table Initialization

/// Initializes the FSE encoder table `table` for `nsymbols` using normalized frequency table `freq`.
pub fn fse_init_encoder_table(
    nstates: usize,
    nsymbols: usize,
    freq: &[u16],
    table: &mut [FseEncoderEntry],
) -> Result<(), TTZipStatus> {
    if nstates == 0 || !nstates.is_power_of_two() || table.len() < nsymbols || freq.len() < nsymbols {
        return Err(TTZipStatus::ErrInvalidParam);
    }
    let mut offset: i32 = 0;
    let n_clz = (nstates as u32).leading_zeros() as i32;
    let nstates_i = nstates as i32;

    for i in 0..nsymbols {
        let f = freq[i] as i32;
        if f == 0 {
            table[i] = FseEncoderEntry::default();
            continue;
        }

        let f_clz = (f as u32).leading_zeros() as i32;
        let k = f_clz - n_clz;
        let delta1 = if k > 0 {
            (offset - f + (nstates_i >> (k - 1))) as i16
        } else {
            (offset - f + (nstates_i << 1)) as i16
        };

        table[i] = FseEncoderEntry {
            s0: ((f << k) - nstates_i) as i16,
            k: k as i16,
            delta0: (offset - f + (nstates_i >> k)) as i16,
            delta1,
        };
        offset += f;
    }
    Ok(())
}

/// Initializes the FSE state decoder table `table` for `nstates` using normalized frequency table `freq`.
pub fn fse_init_decoder_table(
    nstates: usize,
    nsymbols: usize,
    freq: &[u16],
    table: &mut [FseDecoderEntry],
) {
    if nstates == 0 || !nstates.is_power_of_two() || table.len() < nstates {
        return;
    }
    let n_clz = (nstates as u32).leading_zeros() as i32;
    let nstates_i = nstates as i32;
    let mut t_idx = 0;

    for i in 0..nsymbols.min(freq.len()) {
        let f = freq[i] as i32;
        if f == 0 {
            continue;
        }

        let f_clz = (f as u32).leading_zeros() as i32;
        let k = f_clz - n_clz;
        let j0 = ((2 * nstates_i) >> k) - f;

        for j in 0..f {
            if t_idx >= table.len() {
                break;
            }
            let (k_val, delta) = if j < j0 {
                (k as i8, (((f + j) << k) - nstates_i) as i16)
            } else {
                ((k - 1) as i8, ((j - j0) << (k - 1)) as i16)
            };
            table[t_idx] = FseDecoderEntry {
                k: k_val,
                symbol: i as u8,
                delta,
            };
            t_idx += 1;
        }
    }
}

/// Initializes the 32-bit packed integer decoder table for literals (1024 states).
pub fn fse_init_decoder_table_packed(
    nstates: usize,
    nsymbols: usize,
    freq: &[u16],
    table: &mut [i32],
) -> Result<(), TTZipStatus> {
    if nsymbols > 256 || table.len() < nstates || nstates == 0 {
        return Err(TTZipStatus::ErrInvalidParam);
    }
    fse_check_freq(&freq[..nsymbols.min(freq.len())], nstates)?;
    let mut entries = vec![FseDecoderEntry::default(); nstates];
    fse_init_decoder_table(nstates, nsymbols, freq, &mut entries);
    for (i, entry) in entries.into_iter().enumerate() {
        table[i] = entry.to_packed_i32();
    }
    Ok(())
}

/// Initializes the 64-bit fused value decoder table `table` for `nstates` using symbol base values and extra bits.
pub fn fse_init_value_decoder_table(
    nstates: usize,
    nsymbols: usize,
    freq: &[u16],
    symbol_vbase: &[i32],
    symbol_extra_bits: &[u8],
    table: &mut [FseValueDecoderEntry],
) -> Result<(), TTZipStatus> {
    if nstates == 0 || !nstates.is_power_of_two() || table.len() < nstates {
        return Err(TTZipStatus::ErrInvalidParam);
    }
    fse_check_freq(&freq[..nsymbols.min(freq.len())], nstates)?;
    let n_clz = (nstates as u32).leading_zeros() as i32;
    let nstates_i = nstates as i32;
    let mut t_idx = 0;

    for i in 0..nsymbols.min(freq.len()) {
        let f = freq[i] as i32;
        if f == 0 {
            continue;
        }

        let f_clz = (f as u32).leading_zeros() as i32;
        let k = f_clz - n_clz;
        let j0 = ((2 * nstates_i) >> k) - f;
        let vbits = if i < symbol_extra_bits.len() {
            symbol_extra_bits[i]
        } else {
            0
        };
        let vbase = if i < symbol_vbase.len() {
            symbol_vbase[i]
        } else {
            0
        };

        for j in 0..f {
            if t_idx >= table.len() {
                break;
            }
            let (total_bits, delta) = if j < j0 {
                (
                    (k as u8).saturating_add(vbits),
                    (((f + j) << k) - nstates_i) as i16,
                )
            } else {
                (
                    ((k - 1) as u8).saturating_add(vbits),
                    ((j - j0) << (k - 1)) as i16,
                )
            };

            table[t_idx] = FseValueDecoderEntry {
                total_bits,
                value_bits: vbits,
                delta,
                vbase,
            };
            t_idx += 1;
        }
    }
    Ok(())
}

// MARK: - Frequency Normalization

/// Removes states from symbols until the correct number of states is used.
fn fse_adjust_freqs(freq: &mut [u16], mut overrun: i32, nsymbols: usize) {
    let mut shift = 3;
    while shift >= 0 && overrun != 0 {
        for sym in 0..nsymbols {
            if freq[sym] > 1 {
                let n = ((freq[sym] as i32 - 1) >> shift).min(overrun);
                freq[sym] -= n as u16;
                overrun -= n;
                if overrun == 0 {
                    break;
                }
            }
        }
        if shift == 0 {
            break;
        }
        shift -= 1;
    }
}

/// Normalizes occurrence counts to a sum of `nstates` frequencies.
pub fn fse_normalize_freq(
    nstates: usize,
    nsymbols: usize,
    occurrences: &[u32],
    freq: &mut [u16],
) {
    let s_count: u64 = occurrences[..nsymbols].iter().map(|&x| x as u64).sum();
    let mut remaining = nstates as i32;
    let mut max_freq = 0i32;
    let mut max_freq_sym = 0usize;
    let shift = (nstates as u32).leading_zeros() - 1;
    let highprec_step = (1u64 << 31).checked_div(s_count).unwrap_or(0);

    for i in 0..nsymbols {
        let count = occurrences[i] as u64;
        let mut f = ((((count * highprec_step) >> shift) + 1) >> 1) as i32;

        if f == 0 && count != 0 {
            f = 1;
        }

        freq[i] = f as u16;
        remaining -= f;

        if f > max_freq {
            max_freq = f;
            max_freq_sym = i;
        }
    }

    if -remaining < (max_freq >> 2) {
        freq[max_freq_sym] = (freq[max_freq_sym] as i32 + remaining) as u16;
    } else {
        fse_adjust_freqs(freq, -remaining, nsymbols);
    }
}

// MARK: - Frequency Table Serialization

/// Serializes normalized frequency tables into V2 block header freq byte stream.
pub fn lzfse_encode_v1_freq_table(
    l_freq: &[u16; LZFSE_ENCODE_L_SYMBOLS],
    m_freq: &[u16; LZFSE_ENCODE_M_SYMBOLS],
    d_freq: &[u16; LZFSE_ENCODE_D_SYMBOLS],
    literal_freq: &[u16; LZFSE_ENCODE_LITERAL_SYMBOLS],
    dst: &mut Vec<u8>,
) {
    let mut accum: u32 = 0;
    let mut accum_nbits: i32 = 0;

    let tables: [&[u16]; 4] = [l_freq, m_freq, d_freq, literal_freq];

    for table in tables {
        for &val in table {
            let (bits, nbits) = lzfse_encode_v1_freq_value(val as i32);
            accum |= bits << accum_nbits;
            accum_nbits += nbits;

            while accum_nbits >= 8 {
                dst.push((accum & 0xFF) as u8);
                accum >>= 8;
                accum_nbits -= 8;
            }
        }
    }

    if accum_nbits > 0 {
        dst.push((accum & 0xFF) as u8);
    }
}
