// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! RFC 7932 2-Level Canonical Huffman decoding table (DTable) and zero-branch lookup engine.

use super::bit_reader::BrotliBitReader;
use super::error::BrotliError;

/// Maximum allowable Huffman code length per RFC 7932 Section 3.4.
pub const BROTLI_HUFFMAN_MAX_CODE_LENGTH: usize = 15;

/// Number of bits used for direct 1st-level root table indexing ($2^8 = 256$ entries).
pub const HUFFMAN_TABLE_BITS: usize = 8;

/// Bitmask for 1st-level root table lookup (0xFF).
pub const HUFFMAN_TABLE_MASK: usize = 0xFF;

/// Total number of entries in the 1st-level root table ($1 \ll 8 = 256$).
pub const HUFFMAN_ROOT_TABLE_SIZE: usize = 1 << HUFFMAN_TABLE_BITS;

/// Compact Huffman lookup entry representing either a direct decoded symbol or a 2nd-level sub-table branch pointer.
///
/// If `bits <= HUFFMAN_TABLE_BITS` (<= 8), `value` contains the decoded alphabet symbol.
/// If `bits > HUFFMAN_TABLE_BITS` (> 8), `bits` represents `HUFFMAN_TABLE_BITS + sub_table_bits`,
/// and `value` represents the relative offset in the table array to the base of the 2nd-level sub-table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
pub struct HuffmanCode {
    /// Number of bits consumed for this code (or `8 + sub_bits` for sub-table pointer).
    pub bits: u8,
    /// Decoded alphabet symbol or relative offset to 2nd-level sub-table.
    pub value: u16,
}

impl HuffmanCode {
    /// Constructs a new `HuffmanCode` entry.
    #[inline(always)]
    pub const fn new(bits: u8, value: u16) -> Self {
        Self { bits, value }
    }
}

/// Helper function to calculate the bit size of the next 2nd-level sub-table.
///
/// Mirrors RFC 7932 reference logic: computes minimum bit depth required to cover all remaining codes.
#[inline]
fn next_table_bit_size(count: &[u16; 16], mut len: usize, root_bits: usize) -> usize {
    let mut left = 1i32 << (len - root_bits);
    while len < BROTLI_HUFFMAN_MAX_CODE_LENGTH {
        left -= count[len] as i32;
        if left <= 0 {
            break;
        }
        len += 1;
        left <<= 1;
    }
    len - root_bits
}

/// Replicates `code` into `table` at regular intervals of `step` within a segment of size `table_size`.
#[inline(always)]
fn replicate_value(
    table: &mut [HuffmanCode],
    start: usize,
    step: usize,
    table_size: usize,
    code: HuffmanCode,
) {
    let mut end = table_size;
    while end > 0 {
        end -= step;
        table[start + end] = code;
    }
}

/// High-throughput 2-level Canonical Huffman decoding table.
///
/// Features direct $O(1)$ 8-bit root table lookup for frequent codes ($\le 8$ bits)
/// and single-hop secondary table branching for deep codes (9..15 bits).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HuffmanTable {
    /// Contiguous array storing the 256-entry root table followed by secondary sub-tables.
    pub entries: Vec<HuffmanCode>,
}

impl HuffmanTable {
    /// Builds a 2-level Canonical Huffman decoding table from standard code lengths.
    ///
    /// # Errors
    /// Returns `Err(BrotliError::HuffmanSpaceViolation)` if:
    /// - `code_lengths` violates Kraft inequality (over-subscribed or under-subscribed with `num_codes != 1`).
    /// - `code_lengths` contains any length exceeding 15.
    /// - `alphabet_size == 0` or no valid symbols are present.
    pub fn build(code_lengths: &[u8], alphabet_size: usize) -> Result<Self, BrotliError> {
        if alphabet_size == 0 || code_lengths.is_empty() {
            return Err(BrotliError::HuffmanSpaceViolation);
        }

        let limit = std::cmp::min(code_lengths.len(), alphabet_size);
        let mut count = [0u16; 16];
        let mut num_codes = 0usize;
        let mut max_length = 0usize;
        let mut single_symbol = 0u16;
        let mut space = 32768i32; // 2^15

        for (sym, &len) in code_lengths[..limit].iter().enumerate() {
            if len > 0 {
                if len as usize > BROTLI_HUFFMAN_MAX_CODE_LENGTH {
                    return Err(BrotliError::HuffmanSpaceViolation);
                }
                count[len as usize] += 1;
                num_codes += 1;
                single_symbol = sym as u16;
                if (len as usize) > max_length {
                    max_length = len as usize;
                }
                space -= 1i32 << (15 - len);
                if space < 0 {
                    return Err(BrotliError::HuffmanSpaceViolation);
                }
            }
        }

        if space != 0 && num_codes != 1 {
            return Err(BrotliError::HuffmanSpaceViolation);
        }
        if num_codes == 0 {
            return Err(BrotliError::HuffmanSpaceViolation);
        }

        // Special case: single-symbol alphabet.
        if num_codes == 1 {
            let entries = vec![HuffmanCode::new(0, single_symbol); HUFFMAN_ROOT_TABLE_SIZE];
            return Ok(Self { entries });
        }

        // Group symbols by code length in increasing canonical order.
        let mut sorted_symbols: Vec<Vec<u16>> = vec![Vec::new(); max_length + 1];
        for (sym, &len) in code_lengths[..limit].iter().enumerate() {
            if len > 0 && (len as usize) <= max_length {
                sorted_symbols[len as usize].push(sym as u16);
            }
        }

        let mut entries = vec![HuffmanCode::default(); HUFFMAN_ROOT_TABLE_SIZE];
        let root_bits = HUFFMAN_TABLE_BITS;
        let table_bits = std::cmp::min(max_length, root_bits);
        let mut cur_table_size = 1usize << table_bits;

        let mut key = 0u32;
        let mut key_step = 128u32; // 1 << (8 - 1)
        let mut step = 2usize;

        for bits in 1..=table_bits {
            for &sym in &sorted_symbols[bits] {
                let code = HuffmanCode::new(bits as u8, sym);
                let rev_key = (key as u8).reverse_bits() as usize;
                replicate_value(&mut entries, rev_key, step, cur_table_size, code);
                key += key_step;
            }
            step <<= 1;
            key_step >>= 1;
        }

        // If max_length < 8, replicate root entries to fill full 256-entry table.
        while cur_table_size < HUFFMAN_ROOT_TABLE_SIZE {
            let (src, dst) = entries.split_at_mut(cur_table_size);
            dst[..cur_table_size].copy_from_slice(&src[..cur_table_size]);
            cur_table_size <<= 1;
        }

        // Build 2nd-level sub-tables for code lengths > 8.
        if max_length > root_bits {
            let root_key_step = 1u32; // 128 >> 7
            let mut sub_key = 256u32; // Sentinel indicating a new secondary table is required
            let mut sub_key_step = 128u32;
            let mut sub_step = 2usize;
            let mut sub_table_size = 0usize;
            let mut sub_table_start = 0usize;
            let mut rem_count = count;

            for len in (root_bits + 1)..=max_length {
                for &sym in &sorted_symbols[len] {
                    if sub_key == 256 {
                        let sub_table_bits = next_table_bit_size(&rem_count, len, root_bits);
                        sub_table_size = 1usize << sub_table_bits;
                        sub_table_start = entries.len();
                        entries.resize(sub_table_start + sub_table_size, HuffmanCode::default());

                        let root_idx = (key as u8).reverse_bits() as usize;
                        key += root_key_step;

                        let offset = (sub_table_start - root_idx) as u16;
                        entries[root_idx] =
                            HuffmanCode::new((sub_table_bits + root_bits) as u8, offset);
                        sub_key = 0;
                    }

                    let code = HuffmanCode::new((len - root_bits) as u8, sym);
                    let rev_sub_key = (sub_key as u8).reverse_bits() as usize;
                    replicate_value(
                        &mut entries,
                        sub_table_start + rev_sub_key,
                        sub_step,
                        sub_table_size,
                        code,
                    );
                    sub_key += sub_key_step;
                    rem_count[len] -= 1;
                }
                sub_step <<= 1;
                sub_key_step >>= 1;
            }
        }

        Ok(Self { entries })
    }

    /// Fast construction of 1-4 symbol simple prefix codes matching RFC 7932 Section 3.5.
    ///
    /// # Errors
    /// Returns `Err(BrotliError::DuplicateSymbol)` if duplicate symbols are present in `symbols`.
    /// Returns `Err(BrotliError::CorruptHeader)` if `symbols.len()` is not between 1 and 4.
    pub fn build_simple(symbols: &[u16], code_lengths: &[u8]) -> Result<Self, BrotliError> {
        let num_symbols = symbols.len();
        if num_symbols == 0 || num_symbols > 4 {
            return Err(BrotliError::CorruptHeader(
                "Invalid number of symbols for simple Huffman table (must be 1..=4)".to_string(),
            ));
        }

        // Verify duplicate symbols.
        for i in 0..num_symbols {
            for j in (i + 1)..num_symbols {
                if symbols[i] == symbols[j] {
                    return Err(BrotliError::DuplicateSymbol);
                }
            }
        }

        let mut entries = vec![HuffmanCode::default(); HUFFMAN_ROOT_TABLE_SIZE];

        match num_symbols {
            1 => {
                entries.fill(HuffmanCode::new(0, symbols[0]));
            }
            2 => {
                let (s0, s1) = if symbols[0] < symbols[1] {
                    (symbols[0], symbols[1])
                } else {
                    (symbols[1], symbols[0])
                };
                entries[0] = HuffmanCode::new(1, s0);
                entries[1] = HuffmanCode::new(1, s1);
                for i in 2..HUFFMAN_ROOT_TABLE_SIZE {
                    entries[i] = entries[i % 2];
                }
            }
            3 => {
                let s0 = symbols[0];
                let s1 = std::cmp::min(symbols[1], symbols[2]);
                let s2 = std::cmp::max(symbols[1], symbols[2]);
                entries[0] = HuffmanCode::new(1, s0);
                entries[2] = HuffmanCode::new(1, s0);
                entries[1] = HuffmanCode::new(2, s1);
                entries[3] = HuffmanCode::new(2, s2);
                for i in 4..HUFFMAN_ROOT_TABLE_SIZE {
                    entries[i] = entries[i % 4];
                }
            }
            4 => {
                let is_1233 = code_lengths.len() >= 4
                    && code_lengths[0] == 1
                    && code_lengths[1] == 2
                    && code_lengths[2] == 3
                    && code_lengths[3] == 3;
                if is_1233 {
                    let s0 = symbols[0];
                    let s1 = symbols[1];
                    let s2 = std::cmp::min(symbols[2], symbols[3]);
                    let s3 = std::cmp::max(symbols[2], symbols[3]);
                    entries[0] = HuffmanCode::new(1, s0);
                    entries[1] = HuffmanCode::new(2, s1);
                    entries[2] = HuffmanCode::new(1, s0);
                    entries[3] = HuffmanCode::new(3, s2);
                    entries[4] = HuffmanCode::new(1, s0);
                    entries[5] = HuffmanCode::new(2, s1);
                    entries[6] = HuffmanCode::new(1, s0);
                    entries[7] = HuffmanCode::new(3, s3);
                    for i in 8..HUFFMAN_ROOT_TABLE_SIZE {
                        entries[i] = entries[i % 8];
                    }
                } else {
                    let mut s = [symbols[0], symbols[1], symbols[2], symbols[3]];
                    s.sort_unstable();
                    entries[0] = HuffmanCode::new(2, s[0]);
                    entries[2] = HuffmanCode::new(2, s[1]);
                    entries[1] = HuffmanCode::new(2, s[2]);
                    entries[3] = HuffmanCode::new(2, s[3]);
                    for i in 4..HUFFMAN_ROOT_TABLE_SIZE {
                        entries[i] = entries[i % 4];
                    }
                }
            }
            _ => unreachable!(),
        }

        Ok(Self { entries })
    }

    /// Decodes a single Huffman symbol from the bitstream using $O(1)$ zero-branch 2-level lookup.
    ///
    /// Consumes the exact number of bits associated with the decoded symbol from `br`.
    #[inline(always)]
    pub fn decode_symbol(&self, br: &mut BrotliBitReader) -> Result<u16, BrotliError> {
        if br.bit_pos < 15 {
            br.fill_window();
        }
        let peeked = br.peek_bits(15);
        let root_idx = (peeked & (HUFFMAN_TABLE_MASK as u32)) as usize;
        let root_entry = self.entries[root_idx];

        if root_entry.bits <= (HUFFMAN_TABLE_BITS as u8) {
            if root_entry.bits == 0 {
                return Ok(root_entry.value);
            }
            if br.bit_pos < root_entry.bits as u32 {
                return Err(BrotliError::UnexpectedEof);
            }
            br.drop_bits(root_entry.bits as u32);
            Ok(root_entry.value)
        } else {
            let nbits = (root_entry.bits as usize) - HUFFMAN_TABLE_BITS;
            let sub_mask = (1usize << nbits) - 1;
            let sub_offset = ((peeked as usize) >> HUFFMAN_TABLE_BITS) & sub_mask;
            let sub_table_start = root_idx + root_entry.value as usize;
            let sub_idx = sub_table_start + sub_offset;
            let sub_entry = self.entries[sub_idx];
            let total_drop = (HUFFMAN_TABLE_BITS as u32) + (sub_entry.bits as u32);
            if br.bit_pos < total_drop {
                return Err(BrotliError::UnexpectedEof);
            }
            br.drop_bits(total_drop);
            Ok(sub_entry.value)
        }
    }

    /// Total number of entries allocated in the table (root + secondary sub-tables).
    #[inline]
    pub fn total_entries(&self) -> usize {
        self.entries.len()
    }
}
