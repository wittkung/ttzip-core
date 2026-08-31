// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Brotli meta-block header, context map, and Huffman table bitstream parsing.
//!
//! Conforms strictly to RFC 7932 Sections 3, 5, 7 and RFC 9841 Large Window extension.

use std::io::Read;

use super::command::calculate_distance_lut;
use super::context::{BrotliContextMap, BrotliContextMode};
use super::decoder::BrotliStreamDecoder;
use super::error::BrotliError;
use super::huffman::HuffmanTable;
use super::window::BrotliWindow;

/// Block length code table representing base offsets and extra bit lengths per RFC 7932 Section 6.
pub const BLOCK_LEN_RANGES: [(usize, u32); 26] = [
    (1, 2),
    (5, 2),
    (9, 2),
    (13, 2),
    (17, 3),
    (25, 3),
    (33, 3),
    (41, 3),
    (49, 4),
    (65, 4),
    (81, 4),
    (97, 4),
    (113, 5),
    (145, 5),
    (177, 5),
    (209, 5),
    (241, 6),
    (305, 6),
    (369, 7),
    (497, 8),
    (753, 9),
    (1265, 10),
    (2289, 11),
    (4337, 12),
    (8433, 13),
    (16625, 24),
];

impl<R: Read> BrotliStreamDecoder<R> {
    /// Parses the variable-length WBITS prefix from the bitstream.
    pub(crate) fn parse_window_bits_stream(&mut self) -> Result<BrotliWindow, BrotliError> {
        let bit0 = self.read_bits(1)?;
        if bit0 == 0 {
            return BrotliWindow::new(16, self.allow_large_window);
        }
        let n = self.read_bits(3)?;
        if n > 0 {
            return BrotliWindow::new(17 + n as u8, self.allow_large_window);
        }
        let m = self.read_bits(3)?;
        if m == 0 {
            return BrotliWindow::new(17, self.allow_large_window);
        }
        if m == 1 {
            if !self.allow_large_window {
                return Err(BrotliError::InvalidWindowBits(1));
            }
            let extra_bit = self.read_bits(1)?;
            if extra_bit != 0 {
                return Err(BrotliError::InvalidWindowBits(0));
            }
            let wbits = self.read_bits(6)? as u8;
            return BrotliWindow::new(wbits, true);
        }
        let window_bits = 8 + m as u8;
        BrotliWindow::new(window_bits, self.allow_large_window)
    }

    /// Reads a Canonical Huffman table from the bitstream per RFC 7932 Section 3.5.
    pub(crate) fn read_huffman_code(
        &mut self,
        alphabet_size_max: usize,
        alphabet_size_limit: usize,
    ) -> Result<HuffmanTable, BrotliError> {
        let simple_or_complex = self.read_bits(2)?;
        if simple_or_complex == 1 {
            let num_symbols = (self.read_bits(2)? + 1) as usize;
            let max_bits = if alphabet_size_max <= 1 {
                0
            } else {
                (alphabet_size_max - 1).ilog2() + 1
            };
            let mut symbols = Vec::with_capacity(num_symbols);
            for _ in 0..num_symbols {
                let sym = self.read_bits(max_bits)? as u16;
                if sym as usize >= alphabet_size_limit {
                    return Err(BrotliError::CorruptHeader(format!(
                        "Symbol {sym} exceeds alphabet limit {alphabet_size_limit}"
                    )));
                }
                symbols.push(sym);
            }
            for i in 0..symbols.len() {
                for j in (i + 1)..symbols.len() {
                    if symbols[i] == symbols[j] {
                        return Err(BrotliError::DuplicateSymbol);
                    }
                }
            }
            let lengths = if num_symbols == 4 {
                let tree_select = self.read_bits(1)?;
                if tree_select == 0 {
                    vec![2, 2, 2, 2]
                } else {
                    vec![1, 2, 3, 3]
                }
            } else {
                vec![0; num_symbols]
            };
            HuffmanTable::build_simple(&symbols, &lengths)
        } else {
            const CODE_LENGTH_ORDER: [usize; 18] = [
                1, 2, 3, 4, 0, 5, 17, 6, 16, 7, 8, 9, 10, 11, 12, 13, 14, 15,
            ];
            const CODE_LENGTH_PREFIX_LEN: [u32; 16] = [
                2, 2, 2, 3, 2, 2, 2, 4, 2, 2, 2, 3, 2, 2, 2, 4,
            ];
            const CODE_LENGTH_PREFIX_VAL: [u8; 16] = [
                0, 4, 3, 2, 0, 4, 3, 1, 0, 4, 3, 2, 0, 4, 3, 5,
            ];

            let skip = simple_or_complex as usize;
            let mut code_length_code_lengths = [0u8; 18];
            let mut space = 32i32;
            let mut num_codes = 0usize;

            for &idx in &CODE_LENGTH_ORDER[skip..] {
                let peek = self.peek_bits(4)? as usize;
                let val = CODE_LENGTH_PREFIX_VAL[peek];
                let len = CODE_LENGTH_PREFIX_LEN[peek];
                self.drop_bits(len);
                code_length_code_lengths[idx] = val;
                if val != 0 {
                    space -= 32 >> val;
                    num_codes += 1;
                    if space <= 0 {
                        break;
                    }
                }
            }
            if !(num_codes == 1 || space == 0) {
                return Err(BrotliError::HuffmanSpaceViolation);
            }

            let cl_table = HuffmanTable::build(&code_length_code_lengths, 18)?;
            let mut code_lengths = vec![0u8; alphabet_size_limit];
            let mut symbol = 0usize;
            let mut prev_code_len = 8u8;
            let mut repeat = 0usize;
            let mut repeat_code_len = 0u8;
            let mut space = 32768i32;

            while symbol < alphabet_size_limit && space > 0 {
                let code_len = self.decode_huffman_from_table(&cl_table)? as usize;
                if code_len < 16 {
                    repeat = 0;
                    let cl = code_len as u8;
                    code_lengths[symbol] = cl;
                    if cl != 0 {
                        prev_code_len = cl;
                        space -= 32768 >> cl;
                    }
                    symbol += 1;
                } else {
                    let extra_bits = if code_len == 16 { 2 } else { 3 };
                    let repeat_delta = self.read_bits(extra_bits)? as usize;
                    let new_len = if code_len == 16 { prev_code_len } else { 0 };
                    if repeat_code_len != new_len {
                        repeat = 0;
                        repeat_code_len = new_len;
                    }
                    let old_repeat = repeat;
                    if repeat > 0 {
                        repeat -= 2;
                        repeat <<= extra_bits;
                    }
                    repeat += repeat_delta + 3;
                    let repeat_step = repeat - old_repeat;
                    if symbol + repeat_step > alphabet_size_limit {
                        return Err(BrotliError::CorruptHeader(
                            "Repeat code length overflow".into(),
                        ));
                    }
                    if repeat_code_len != 0 {
                        for s in symbol..(symbol + repeat_step) {
                            code_lengths[s] = repeat_code_len;
                        }
                        space -= (repeat_step as i32) * (32768 >> repeat_code_len);
                    }
                    symbol += repeat_step;
                }
            }

            if space != 0 && num_codes != 1 {
                return Err(BrotliError::HuffmanSpaceViolation);
            }

            HuffmanTable::build(&code_lengths, alphabet_size_limit)
        }
    }

    /// Decodes a context map per RFC 7932 Section 7.3.
    pub(crate) fn decode_context_map(
        &mut self,
        context_map_size: usize,
    ) -> Result<(usize, Vec<u8>), BrotliError> {
        let num_htrees = self.decode_var_len_uint8()? + 1;
        if num_htrees <= 1 {
            return Ok((1, vec![0u8; context_map_size]));
        }

        let use_rle = self.read_bits(1)? != 0;
        let max_run_length_prefix = if use_rle {
            (self.read_bits(4)? + 1) as usize
        } else {
            0
        };

        let alphabet_size = num_htrees + max_run_length_prefix;
        let htree = self.read_huffman_code(alphabet_size, alphabet_size)?;

        let mut context_map = Vec::with_capacity(context_map_size);
        while context_map.len() < context_map_size {
            let code = self.decode_huffman_from_table(&htree)? as usize;
            if code == 0 {
                context_map.push(0);
            } else if code <= max_run_length_prefix {
                let extra = self.read_bits(code as u32)? as usize;
                let reps = (1usize << code) + extra;
                if context_map.len() + reps > context_map_size {
                    return Err(BrotliError::CorruptHeader(
                        "Context map RLE exceeds size".into(),
                    ));
                }
                context_map.resize(context_map.len() + reps, 0);
            } else {
                let val = code - max_run_length_prefix;
                context_map.push(val as u8);
            }
        }

        let use_mtf = self.read_bits(1)? != 0;
        if use_mtf {
            BrotliContextMap::inverse_move_to_front(&mut context_map);
        }

        Ok((num_htrees, context_map))
    }

    /// Parses all metadata headers, Huffman trees, and context maps for a compressed meta-block.
    pub(crate) fn parse_compressed_metablock_header(&mut self) -> Result<(), BrotliError> {
        for cat in 0..3 {
            let ntypes = self.decode_var_len_uint8()? + 1;
            self.num_block_types[cat] = ntypes;
            if ntypes >= 2 {
                let alpha = ntypes + 2;
                let type_tree = self.read_huffman_code(alpha, alpha)?;
                let len_tree = self.read_huffman_code(26, 26)?;
                let init_len = {
                    let len_sym = self.decode_huffman_from_table(&len_tree)? as usize;
                    if len_sym >= BLOCK_LEN_RANGES.len() {
                        return Err(BrotliError::CorruptHeader(
                            "Invalid initial block length symbol".into(),
                        ));
                    }
                    let (offset, nbits) = BLOCK_LEN_RANGES[len_sym];
                    let extra = self.read_bits(nbits)? as usize;
                    offset + extra
                };
                self.block_length[cat] = init_len;
                self.block_type_trees[cat] = Some(type_tree);
                self.block_len_trees[cat] = Some(len_tree);
            } else {
                self.block_type_trees[cat] = None;
                self.block_len_trees[cat] = None;
                self.block_length[cat] = 1 << 28;
            }
            self.block_type_rb[cat * 2] = 1;
            self.block_type_rb[cat * 2 + 1] = 0;
            self.block_type[cat] = 0;
        }

        self.distance_postfix_bits = self.read_bits(2)?;
        let direct_codes = self.read_bits(4)?;
        self.num_direct_distance_codes = (direct_codes as usize) << self.distance_postfix_bits;

        self.context_modes.clear();
        for _ in 0..self.num_block_types[0] {
            let mode_val = self.read_bits(2)? as u8;
            self.context_modes
                .push(BrotliContextMode::from_u8_clamped(mode_val));
        }

        let lit_map_size = self.num_block_types[0] << 6;
        let (num_lit_htrees, lit_map) = self.decode_context_map(lit_map_size)?;
        self.literal_context_map = lit_map;

        let dist_map_size = self.num_block_types[2] << 2;
        let (num_dist_htrees, dist_map) = self.decode_context_map(dist_map_size)?;
        self.dist_context_map = dist_map;

        self.literal_htrees.clear();
        for _ in 0..num_lit_htrees {
            let tree = self.read_huffman_code(256, 256)?;
            self.literal_htrees.push(tree);
        }

        self.command_htrees.clear();
        for _ in 0..self.num_block_types[1] {
            let tree = self.read_huffman_code(704, 704)?;
            self.command_htrees.push(tree);
        }

        let max_dist_bits = if self.allow_large_window { 30 } else { 24 };
        let dist_alpha = 16
            + self.num_direct_distance_codes
            + (max_dist_bits << (self.distance_postfix_bits + 1));

        self.dist_htrees.clear();
        for _ in 0..num_dist_htrees {
            let tree = self.read_huffman_code(dist_alpha, dist_alpha)?;
            self.dist_htrees.push(tree);
        }

        let (extra_bits, offsets) = calculate_distance_lut(
            self.distance_postfix_bits,
            self.num_direct_distance_codes,
            dist_alpha,
        );
        self.dist_extra_bits = extra_bits;
        self.dist_offsets = offsets;

        Ok(())
    }
}
