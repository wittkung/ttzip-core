// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Canonical Huffman decode table construction and precomputed lookup tables for DEFLATE decompression.
//!
//! Provides 2-level canonical Huffman decoding tables with compact 32-bit entry representations,
//! supporting simultaneous symbol lookup, codeword length consumption, and extra bit extraction.

use crate::types::TTZipStatus;
use super::huffman::{
    DEFLATE_NUM_LITLEN_SYMS, DEFLATE_NUM_OFFSET_SYMS, DEFLATE_NUM_PRECODE_SYMS,
};

// MARK: - Constants

/// Maximum table bits for literal/length decode table.
pub const LITLEN_TABLEBITS: usize = 11;

/// Maximum table bits for offset/distance decode table.
pub const OFFSET_TABLEBITS: usize = 8;

/// Table bits for precode decode table (covers max precode codeword length 7).
pub const PRECODE_TABLEBITS: usize = 7;

/// Worst-case maximum decode table size for literal/length symbols.
pub const LITLEN_ENOUGH: usize = 2342;

/// Worst-case maximum decode table size for offset symbols.
pub const OFFSET_ENOUGH: usize = 402;

/// Worst-case maximum decode table size for precode symbols.
pub const PRECODE_ENOUGH: usize = 128;

/// Entry flag: indicates a literal entry (bit 31).
pub const HUFFDEC_LITERAL: u32 = 0x8000_0000;

/// Entry flag: indicates an exceptional entry (subtable pointer or end of block, bit 15).
pub const HUFFDEC_EXCEPTIONAL: u32 = 0x0000_8000;

/// Entry flag: indicates a subtable pointer (bit 14).
pub const HUFFDEC_SUBTABLE_POINTER: u32 = 0x0000_4000;

/// Entry flag: indicates end of block marker (bit 13).
pub const HUFFDEC_END_OF_BLOCK: u32 = 0x0000_2000;

/// Maximum number of bytes written during a single iteration of the fast loop (2 + 258 + 40 - 1).
pub const FASTLOOP_MAX_BYTES_WRITTEN: usize = 299;

/// Maximum number of bytes read during a single iteration of the fast loop.
pub const FASTLOOP_MAX_BYTES_READ: usize = 32;

/// Static length base values for symbols 257..=287.
pub const LENGTH_BASES: [u16; 31] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
    131, 163, 195, 227, 258, 258, 258,
];

/// Extra bits for length symbols 257..=287.
pub const LENGTH_EXTRA_BITS: [u8; 31] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0, 0, 0,
];

/// Static offset base values for symbols 0..=31.
pub const OFFSET_BASES: [u16; 32] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577, 24577, 24577,
];

/// Extra bits for offset symbols 0..=31.
pub const OFFSET_EXTRA_BITS: [u8; 32] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
    13, 13, 13, 13,
];

/// Precomputed static decode result template for precode symbols 0..18.
pub const PRECODE_DECODE_RESULTS: [u32; DEFLATE_NUM_PRECODE_SYMS] = [
    0 << 16, 1 << 16, 2 << 16, 3 << 16, 4 << 16, 5 << 16, 6 << 16, 7 << 16,
    8 << 16, 9 << 16, 10 << 16, 11 << 16, 12 << 16, 13 << 16, 14 << 16, 15 << 16,
    16 << 16, 17 << 16, 18 << 16,
];

// MARK: - Table Entry Construction

#[inline(always)]
const fn generate_litlen_decode_results() -> [u32; DEFLATE_NUM_LITLEN_SYMS] {
    let mut res = [0u32; DEFLATE_NUM_LITLEN_SYMS];
    let mut i = 0;
    while i < 256 {
        res[i] = HUFFDEC_LITERAL | ((i as u32) << 16);
        i += 1;
    }
    res[256] = HUFFDEC_EXCEPTIONAL | HUFFDEC_END_OF_BLOCK;
    i = 257;
    while i < 288 {
        let idx = i - 257;
        let base = LENGTH_BASES[idx] as u32;
        let extra = LENGTH_EXTRA_BITS[idx] as u32;
        res[i] = (base << 16) | extra;
        i += 1;
    }
    res
}

#[inline(always)]
const fn generate_offset_decode_results() -> [u32; DEFLATE_NUM_OFFSET_SYMS] {
    let mut res = [0u32; DEFLATE_NUM_OFFSET_SYMS];
    let mut i = 0;
    while i < 32 {
        let base = OFFSET_BASES[i] as u32;
        let extra = OFFSET_EXTRA_BITS[i] as u32;
        res[i] = (base << 16) | extra;
        i += 1;
    }
    res
}

/// Precomputed static decode results for litlen symbols.
pub static LITLEN_DECODE_RESULTS: [u32; DEFLATE_NUM_LITLEN_SYMS] = generate_litlen_decode_results();

/// Precomputed static decode results for offset symbols.
pub static OFFSET_DECODE_RESULTS: [u32; DEFLATE_NUM_OFFSET_SYMS] = generate_offset_decode_results();

#[inline(always)]
pub fn make_decode_table_entry(decode_results: &[u32], sym: usize, len: u32) -> u32 {
    decode_results[sym] + (len << 8) + len
}

// MARK: - Decode Table Builder

/// Internal generic Huffman decode table builder with incremental doubling and subtable linking.
pub fn build_decode_table_impl(
    lens: &[u8],
    num_syms: usize,
    decode_results: &[u32],
    tablebits: usize,
    max_codeword_len: usize,
    table: &mut [u32],
) -> Result<(), TTZipStatus> {
    let mut len_counts = [0u32; 16];
    for sym in 0..num_syms {
        let l = lens[sym] as usize;
        if l > max_codeword_len {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        len_counts[l] += 1;
    }

    let mut max_len = max_codeword_len;
    while max_len > 1 && len_counts[max_len] == 0 {
        max_len -= 1;
    }

    let mut offsets = [0usize; 16];
    offsets[0] = 0;
    offsets[1] = len_counts[0] as usize;
    let mut codespace_used: u32 = 0;
    for len in 1..max_len {
        offsets[len + 1] = offsets[len] + len_counts[len] as usize;
        codespace_used = (codespace_used << 1) + len_counts[len];
    }
    codespace_used = (codespace_used << 1) + len_counts[max_len];

    if codespace_used > (1u32 << max_len) {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let mut sorted_syms = [0u16; 320];
    for sym in 0..num_syms {
        let l = lens[sym] as usize;
        sorted_syms[offsets[l]] = sym as u16;
        offsets[l] += 1;
    }
    let mut sym_idx = offsets[0];

    // Incomplete code handling
    if codespace_used < (1u32 << max_len) {
        let sym = if codespace_used == 0 {
            0
        } else {
            if codespace_used != (1u32 << (max_len - 1)) || len_counts[1] != 1 {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            sorted_syms[sym_idx] as usize
        };
        let entry = make_decode_table_entry(decode_results, sym, 1);
        let num_entries = 1usize << tablebits;
        for item in table[..num_entries].iter_mut() {
            *item = entry;
        }
        return Ok(());
    }

    // Complete code population
    let mut codeword = 0u32;
    let mut len = 1usize;
    while len_counts[len] == 0 {
        len += 1;
    }
    let mut cur_table_end = 1usize << len;
    let mut count = len_counts[len] as usize;

    while len <= tablebits {
        loop {
            table[codeword as usize] = make_decode_table_entry(
                decode_results,
                sorted_syms[sym_idx] as usize,
                len as u32,
            );
            sym_idx += 1;

            if codeword == (cur_table_end as u32 - 1) {
                while len < tablebits {
                    table.copy_within(0..cur_table_end, cur_table_end);
                    cur_table_end <<= 1;
                    len += 1;
                }
                return Ok(());
            }

            let diff = codeword ^ (cur_table_end as u32 - 1);
            let bit = 1u32 << (31 - diff.leading_zeros());
            codeword &= bit - 1;
            codeword |= bit;
            count -= 1;
            if count == 0 {
                break;
            }
        }

        loop {
            len += 1;
            if len <= tablebits {
                table.copy_within(0..cur_table_end, cur_table_end);
                cur_table_end <<= 1;
            }
            count = len_counts[len] as usize;
            if count != 0 {
                break;
            }
        }
    }

    // Subtables for codewords with len > tablebits
    cur_table_end = 1usize << tablebits;
    let mut subtable_prefix = u32::MAX;
    let mut subtable_start = 0usize;

    loop {
        let prefix = codeword & ((1u32 << tablebits) - 1);
        if prefix != subtable_prefix {
            subtable_prefix = prefix;
            subtable_start = cur_table_end;
            let mut subtable_bits = len - tablebits;
            let mut sub_codespace = count as u32;
            while sub_codespace < (1u32 << subtable_bits) {
                subtable_bits += 1;
                sub_codespace = (sub_codespace << 1) + len_counts[tablebits + subtable_bits];
            }
            cur_table_end = subtable_start + (1usize << subtable_bits);
            if cur_table_end > table.len() {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            table[subtable_prefix as usize] = ((subtable_start as u32) << 16)
                | HUFFDEC_EXCEPTIONAL
                | HUFFDEC_SUBTABLE_POINTER
                | ((subtable_bits as u32) << 8)
                | (tablebits as u32);
        }

        let entry = make_decode_table_entry(
            decode_results,
            sorted_syms[sym_idx] as usize,
            (len - tablebits) as u32,
        );
        sym_idx += 1;
        let mut i = subtable_start + ((codeword >> tablebits) as usize);
        let stride = 1usize << (len - tablebits);
        while i < cur_table_end {
            table[i] = entry;
            i += stride;
        }

        if codeword == (1u32 << len) - 1 {
            return Ok(());
        }

        let diff = codeword ^ ((1u32 << len) - 1);
        let bit = 1u32 << (31 - diff.leading_zeros());
        codeword &= bit - 1;
        codeword |= bit;
        count -= 1;
        while count == 0 {
            len += 1;
            count = len_counts[len] as usize;
        }
    }
}

/// Builds a canonical Huffman decode table with specified symbol count and maximum codeword length.
pub fn build_decode_table(
    lens: &[u8],
    num_syms: usize,
    tablebits: usize,
    max_codeword_len: usize,
    table: &mut [u32],
) -> Result<(), TTZipStatus> {
    if num_syms <= DEFLATE_NUM_PRECODE_SYMS && tablebits == PRECODE_TABLEBITS {
        build_decode_table_impl(lens, num_syms, &PRECODE_DECODE_RESULTS, tablebits, max_codeword_len, table)
    } else if num_syms <= DEFLATE_NUM_OFFSET_SYMS && tablebits == OFFSET_TABLEBITS {
        build_decode_table_impl(lens, num_syms, &OFFSET_DECODE_RESULTS, tablebits, max_codeword_len, table)
    } else {
        build_decode_table_impl(lens, num_syms, &LITLEN_DECODE_RESULTS, tablebits, max_codeword_len, table)
    }
}
