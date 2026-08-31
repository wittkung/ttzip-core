// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe Pure-Rust Google Brotli streaming decompressor (RFC 7932 & RFC 9841).
//!
//! Implements a high-throughput, zero-unsafe `BrotliStreamDecoder<R>` with:
//! - 64-bit register bitstream prefetching over underlying `std::io::Read` streams.
//! - Power-of-two sliding ring buffer with 542-byte write-ahead slack.
//! - Dynamic Canonical 2-level Huffman DTable decoding for literals, commands, and distances.
//! - 2nd-order context modeling (LSB6, MSB6, UTF8, Signed) with inverse MTF context maps.
//! - LZ77 backward reference match copy and RFC 7932 static dictionary 121 transforms.
//! - Stream-first micro-buffering and graceful error recovery (0 panics).

use std::io::{self, Read};

use super::command::CMD_LUT;
use super::context::{get_context_id, BrotliContextMode};
use super::dictionary::{get_dictionary_word, SIZE_BITS_BY_LENGTH};
use super::error::BrotliError;
use super::huffman::{HuffmanTable, HUFFMAN_TABLE_BITS, HUFFMAN_TABLE_MASK};
use super::meta_header::BLOCK_LEN_RANGES;
use super::ring_buffer::BrotliDecoderRingBuffer;
use super::state::BrotliDecoderFsmState;
use super::transform::transform_dictionary_word;
use super::window::BrotliWindow;

/// Default input micro-buffer size in bytes (64 KiB).
pub const DEFAULT_DECODER_BUFFER_SIZE: usize = 65536;

/// Safe Pure-Rust streaming Brotli decompressor conforming to RFC 7932.
pub struct BrotliStreamDecoder<R: Read> {
    /// Underlying compressed byte stream.
    pub reader: R,
    /// Input micro-buffer holding unconsumed raw bytes.
    pub buffer: Vec<u8>,
    pub buf_pos: usize,
    pub buf_len: usize,
    /// Sliding window ring buffer with slack capacity.
    pub ring_buffer: BrotliDecoderRingBuffer,
    /// Active sliding window geometry.
    pub window: BrotliWindow,
    /// Finite state machine state.
    pub state: BrotliDecoderFsmState,
    pub allow_large_window: bool,
    pub eof_reached: bool,
    pub bit_val: u64,
    pub bit_pos: u32,
    pub meta_block_remaining_len: usize,
    pub is_last_metablock: bool,
    pub num_block_types: [usize; 3],
    pub block_type_rb: [usize; 6],
    pub block_type: [usize; 3],
    pub block_length: [usize; 3],
    pub block_type_trees: [Option<HuffmanTable>; 3],
    pub block_len_trees: [Option<HuffmanTable>; 3],
    pub context_modes: Vec<BrotliContextMode>,
    pub literal_context_map: Vec<u8>,
    pub dist_context_map: Vec<u8>,
    pub literal_htrees: Vec<HuffmanTable>,
    pub command_htrees: Vec<HuffmanTable>,
    pub dist_htrees: Vec<HuffmanTable>,
    pub dist_extra_bits: Vec<u8>,
    pub dist_offsets: Vec<usize>,
    pub dist_rb: [usize; 4],
    pub dist_rb_idx: usize,
    pub distance_postfix_bits: u32,
    pub num_direct_distance_codes: usize,
}

impl<R: Read> BrotliStreamDecoder<R> {
    /// Creates a new `BrotliStreamDecoder` wrapping the given input reader with standard 64 KiB buffer.
    pub fn new(reader: R) -> Self {
        Self::with_buffer_size(reader, DEFAULT_DECODER_BUFFER_SIZE)
    }

    /// Creates a new `BrotliStreamDecoder` with customized input buffer capacity.
    pub fn with_buffer_size(reader: R, buffer_size: usize) -> Self {
        let buf_size = if buffer_size == 0 {
            DEFAULT_DECODER_BUFFER_SIZE
        } else {
            buffer_size
        };
        let initial_window = BrotliWindow::new(16, false).unwrap_or(BrotliWindow {
            window_bits: 16,
            max_distance: 65520,
        });
        let initial_ring = BrotliDecoderRingBuffer::new(16).unwrap();

        Self {
            reader,
            buffer: vec![0u8; buf_size],
            buf_pos: 0,
            buf_len: 0,
            ring_buffer: initial_ring,
            window: initial_window,
            state: BrotliDecoderFsmState::Init,
            allow_large_window: false,
            eof_reached: false,
            bit_val: 0,
            bit_pos: 0,
            meta_block_remaining_len: 0,
            is_last_metablock: false,
            num_block_types: [1, 1, 1],
            block_type_rb: [1, 0, 1, 0, 1, 0],
            block_type: [0; 3],
            block_length: [1 << 28; 3],
            block_type_trees: [None, None, None],
            block_len_trees: [None, None, None],
            context_modes: vec![BrotliContextMode::Lsb6],
            literal_context_map: vec![0],
            dist_context_map: vec![0],
            literal_htrees: Vec::new(),
            command_htrees: Vec::new(),
            dist_htrees: Vec::new(),
            dist_extra_bits: Vec::new(),
            dist_offsets: Vec::new(),
            dist_rb: [16, 15, 11, 4],
            dist_rb_idx: 0,
            distance_postfix_bits: 0,
            num_direct_distance_codes: 0,
        }
    }

    /// Configures whether RFC 9841 Large Window extension (up to 30 bits / 1 GiB) is permitted.
    pub fn with_large_window(reader: R, allow_large_window: bool) -> Self {
        let mut decoder = Self::new(reader);
        decoder.allow_large_window = allow_large_window;
        decoder
    }

    /// Refills the 64-bit accumulator from the input stream.
    pub(crate) fn refill_bit_window(&mut self) -> Result<(), BrotliError> {
        while self.bit_pos <= 56 {
            if self.buf_pos >= self.buf_len {
                if self.eof_reached {
                    break;
                }
                match self.reader.read(&mut self.buffer) {
                    Ok(0) => {
                        self.eof_reached = true;
                        break;
                    }
                    Ok(n) => {
                        self.buf_pos = 0;
                        self.buf_len = n;
                    }
                    Err(e) => return Err(BrotliError::DecompressionFailed(e.to_string())),
                }
            }

            while self.buf_pos < self.buf_len && self.bit_pos <= 56 {
                let byte = self.buffer[self.buf_pos] as u64;
                self.bit_val |= byte << self.bit_pos;
                self.bit_pos += 8;
                self.buf_pos += 1;
            }
        }
        Ok(())
    }

    /// Peeks `n` (0..=32) least-significant bits without consuming them.
    #[inline]
    pub(crate) fn peek_bits(&mut self, n: u32) -> Result<u32, BrotliError> {
        if self.bit_pos < n {
            self.refill_bit_window()?;
            if self.bit_pos < n {
                return Err(BrotliError::UnexpectedEof);
            }
        }
        if n == 0 {
            Ok(0)
        } else if n >= 32 {
            Ok(self.bit_val as u32)
        } else {
            Ok((self.bit_val as u32) & ((1u32 << n) - 1))
        }
    }

    /// Drops `n` bits from the accumulator.
    #[inline]
    pub(crate) fn drop_bits(&mut self, n: u32) {
        if n >= 64 {
            self.bit_val = 0;
            self.bit_pos = 0;
        } else {
            self.bit_val >>= n;
            self.bit_pos = self.bit_pos.saturating_sub(n);
        }
    }

    /// Reads and consumes `n` (0..=32) bits from the stream.
    #[inline]
    pub(crate) fn read_bits(&mut self, n: u32) -> Result<u32, BrotliError> {
        let bits = self.peek_bits(n)?;
        self.drop_bits(n);
        Ok(bits)
    }

    /// Aligns bitstream to the next byte boundary, verifying that all skipped padding bits are zero.
    pub(crate) fn jump_to_byte_boundary(&mut self) -> Result<(), BrotliError> {
        let pad_bits = self.bit_pos & 7;
        if pad_bits > 0 {
            let pad = (self.bit_val as u32) & ((1u32 << pad_bits) - 1);
            if pad != 0 {
                return Err(BrotliError::InvalidPadding);
            }
            self.drop_bits(pad_bits);
        }
        Ok(())
    }

    /// Decodes a variable-length unsigned integer per RFC 7932 Section 3.2.
    pub(crate) fn decode_var_len_uint8(&mut self) -> Result<usize, BrotliError> {
        if self.read_bits(1)? == 0 {
            Ok(0)
        } else {
            let nbits = self.read_bits(3)?;
            if nbits == 0 {
                Ok(1)
            } else {
                let v = self.read_bits(nbits)?;
                Ok((1usize << nbits) + (v as usize))
            }
        }
    }

    /// Decodes a single Huffman symbol using $O(1)$ zero-branch 2-level lookup from an explicit table reference.
    #[inline(always)]
    pub(crate) fn decode_huffman_from_table(
        &mut self,
        table: &HuffmanTable,
    ) -> Result<u16, BrotliError> {
        let _ = self.refill_bit_window();
        let peeked = self.bit_val as u32;
        let root_idx = (peeked & (HUFFMAN_TABLE_MASK as u32)) as usize;
        let root_entry = table.entries[root_idx];

        if root_entry.bits <= (HUFFMAN_TABLE_BITS as u8) {
            let bits = root_entry.bits as u32;
            if self.bit_pos < bits {
                return Err(BrotliError::UnexpectedEof);
            }
            self.drop_bits(bits);
            Ok(root_entry.value)
        } else {
            let nbits = (root_entry.bits as usize) - HUFFMAN_TABLE_BITS;
            let sub_mask = (1usize << nbits) - 1;
            let sub_offset = ((peeked as usize) >> HUFFMAN_TABLE_BITS) & sub_mask;
            let sub_table_start = root_idx + root_entry.value as usize;
            let sub_idx = sub_table_start + sub_offset;
            let sub_entry = table.entries[sub_idx];
            let total_drop = (HUFFMAN_TABLE_BITS as u32) + (sub_entry.bits as u32);
            if self.bit_pos < total_drop {
                return Err(BrotliError::UnexpectedEof);
            }
            self.drop_bits(total_drop);
            Ok(sub_entry.value)
        }
    }

    /// Decodes a block symbol and length for a block type switch.
    fn decode_block_type_symbol_and_len(
        &mut self,
        tree_type: usize,
    ) -> Result<(usize, usize), BrotliError> {
        let sym = {
            let _ = self.refill_bit_window();
            let peeked = self.bit_val as u32;
            let root_idx = (peeked & (HUFFMAN_TABLE_MASK as u32)) as usize;
            let tree = self.block_type_trees[tree_type]
                .as_ref()
                .ok_or_else(|| BrotliError::CorruptHeader("Missing block type tree".into()))?;
            let root_entry = tree.entries[root_idx];
            if root_entry.bits <= (HUFFMAN_TABLE_BITS as u8) {
                let bits = root_entry.bits as u32;
                if self.bit_pos < bits {
                    return Err(BrotliError::UnexpectedEof);
                }
                self.drop_bits(bits);
                root_entry.value as usize
            } else {
                let nbits = (root_entry.bits as usize) - HUFFMAN_TABLE_BITS;
                let sub_mask = (1usize << nbits) - 1;
                let sub_offset = ((peeked as usize) >> HUFFMAN_TABLE_BITS) & sub_mask;
                let sub_table_start = root_idx + root_entry.value as usize;
                let sub_idx = sub_table_start + sub_offset;
                let sub_entry = tree.entries[sub_idx];
                let total_drop = (HUFFMAN_TABLE_BITS as u32) + (sub_entry.bits as u32);
                if self.bit_pos < total_drop {
                    return Err(BrotliError::UnexpectedEof);
                }
                self.drop_bits(total_drop);
                sub_entry.value as usize
            }
        };

        let len_sym = {
            let _ = self.refill_bit_window();
            let peeked = self.bit_val as u32;
            let root_idx = (peeked & (HUFFMAN_TABLE_MASK as u32)) as usize;
            let tree = self.block_len_trees[tree_type]
                .as_ref()
                .ok_or_else(|| BrotliError::CorruptHeader("Missing block len tree".into()))?;
            let root_entry = tree.entries[root_idx];
            if root_entry.bits <= (HUFFMAN_TABLE_BITS as u8) {
                let bits = root_entry.bits as u32;
                if self.bit_pos < bits {
                    return Err(BrotliError::UnexpectedEof);
                }
                self.drop_bits(bits);
                root_entry.value as usize
            } else {
                let nbits = (root_entry.bits as usize) - HUFFMAN_TABLE_BITS;
                let sub_mask = (1usize << nbits) - 1;
                let sub_offset = ((peeked as usize) >> HUFFMAN_TABLE_BITS) & sub_mask;
                let sub_table_start = root_idx + root_entry.value as usize;
                let sub_idx = sub_table_start + sub_offset;
                let sub_entry = tree.entries[sub_idx];
                let total_drop = (HUFFMAN_TABLE_BITS as u32) + (sub_entry.bits as u32);
                if self.bit_pos < total_drop {
                    return Err(BrotliError::UnexpectedEof);
                }
                self.drop_bits(total_drop);
                sub_entry.value as usize
            }
        };

        if len_sym >= BLOCK_LEN_RANGES.len() {
            return Err(BrotliError::CorruptHeader(
                "Invalid block length symbol".into(),
            ));
        }
        let (offset, nbits) = BLOCK_LEN_RANGES[len_sym];
        let extra = self.read_bits(nbits)? as usize;
        let len = offset + extra;

        Ok((sym, len))
    }

    /// Executes a block type switch transition for literals (0), commands (1), or distances (2).
    fn decode_block_type_switch(&mut self, tree_type: usize) -> Result<(), BrotliError> {
        let max_block_type = self.num_block_types[tree_type];
        let (sym, new_len) = self.decode_block_type_symbol_and_len(tree_type)?;
        self.block_length[tree_type] = new_len;

        let rb_idx = tree_type * 2;
        let mut block_type = if sym == 1 {
            self.block_type_rb[rb_idx + 1] + 1
        } else if sym == 0 {
            self.block_type_rb[rb_idx]
        } else {
            sym - 2
        };
        if block_type >= max_block_type {
            block_type -= max_block_type;
        }
        self.block_type_rb[rb_idx] = self.block_type_rb[rb_idx + 1];
        self.block_type_rb[rb_idx + 1] = block_type;
        self.block_type[tree_type] = block_type;
        Ok(())
    }

    /// Resolves distance short codes (0..=15) using the 4-element past distance ring buffer.
    fn take_distance_from_ring_buffer(&mut self, dist_code: usize) -> (usize, usize) {
        if dist_code <= 3 {
            let offset = (dist_code as isize) - 3;
            let distance_context = 1usize >> dist_code;
            let idx = (self.dist_rb_idx as isize - offset) as usize & 3;
            let distance = self.dist_rb[idx];
            self.dist_rb_idx = (self.dist_rb_idx + 4 - distance_context) & 3;
            (distance, distance_context)
        } else {
            let (index_delta, base) = if dist_code < 10 {
                (3usize, dist_code - 4)
            } else {
                (2usize, dist_code - 10)
            };
            let delta = (((0x605142u32 >> (4 * base)) & 0xF) as isize) - 3;
            let idx = (self.dist_rb_idx + index_delta) & 3;
            let d = (self.dist_rb[idx] as isize + delta) as usize;
            let distance = if d == 0 { 0x7FFF_FFFF } else { d };
            (distance, 0)
        }
    }

    /// Decodes a chunk of compressed LZ77 insert & copy commands.
    fn decode_commands_chunk(&mut self) -> Result<(), BrotliError> {
        let mut transform_buf = [0u8; 542];

        while self.meta_block_remaining_len > 0 {
            if self.block_length[1] == 0 {
                self.decode_block_type_switch(1)?;
            }
            self.block_length[1] -= 1;

            let cmd_code = {
                let _ = self.refill_bit_window();
                let peeked = self.bit_val as u32;
                let root_idx = (peeked & (HUFFMAN_TABLE_MASK as u32)) as usize;
                let tree = &self.command_htrees[self.block_type[1]];
                let root_entry = tree.entries[root_idx];
                if root_entry.bits <= (HUFFMAN_TABLE_BITS as u8) {
                    let bits = root_entry.bits as u32;
                    if self.bit_pos < bits {
                        return Err(BrotliError::UnexpectedEof);
                    }
                    self.drop_bits(bits);
                    root_entry.value as usize
                } else {
                    let nbits = (root_entry.bits as usize) - HUFFMAN_TABLE_BITS;
                    let sub_mask = (1usize << nbits) - 1;
                    let sub_offset = ((peeked as usize) >> HUFFMAN_TABLE_BITS) & sub_mask;
                    let sub_table_start = root_idx + root_entry.value as usize;
                    let sub_idx = sub_table_start + sub_offset;
                    let sub_entry = tree.entries[sub_idx];
                    let total_drop = (HUFFMAN_TABLE_BITS as u32) + (sub_entry.bits as u32);
                    if self.bit_pos < total_drop {
                        return Err(BrotliError::UnexpectedEof);
                    }
                    self.drop_bits(total_drop);
                    sub_entry.value as usize
                }
            };

            if cmd_code >= 704 {
                return Err(BrotliError::CorruptHeader("Invalid command symbol".into()));
            }
            let cmd_elem = CMD_LUT[cmd_code];

            let insert_len_extra = if cmd_elem.insert_len_extra_bits > 0 {
                self.read_bits(cmd_elem.insert_len_extra_bits as u32)? as usize
            } else {
                0
            };
            let insert_len = (cmd_elem.insert_len_offset as usize) + insert_len_extra;

            let copy_len_extra = if cmd_elem.copy_len_extra_bits > 0 {
                self.read_bits(cmd_elem.copy_len_extra_bits as u32)? as usize
            } else {
                0
            };
            let copy_len = (cmd_elem.copy_len_offset as usize) + copy_len_extra;

            // 1. Literal insertion
            for _ in 0..insert_len {
                if self.block_length[0] == 0 {
                    self.decode_block_type_switch(0)?;
                }
                self.block_length[0] -= 1;

                let p1 = self.ring_buffer.get_recent_byte(1).unwrap_or(0);
                let p2 = self.ring_buffer.get_recent_byte(2).unwrap_or(0);
                let mode = self.context_modes[self.block_type[0]];
                let ctx = get_context_id(p1, p2, mode);
                let ctx_map_idx = (self.block_type[0] << 6) + ctx;
                let htree_idx = self.literal_context_map[ctx_map_idx] as usize;
                if htree_idx >= self.literal_htrees.len() {
                    return Err(BrotliError::CorruptHeader(
                        "Literal tree index out of range".into(),
                    ));
                }

                let lit = {
                    let _ = self.refill_bit_window();
                    let peeked = self.bit_val as u32;
                    let root_idx = (peeked & (HUFFMAN_TABLE_MASK as u32)) as usize;
                    let tree = &self.literal_htrees[htree_idx];
                    let root_entry = tree.entries[root_idx];
                    if root_entry.bits <= (HUFFMAN_TABLE_BITS as u8) {
                        let bits = root_entry.bits as u32;
                        if self.bit_pos < bits {
                            return Err(BrotliError::UnexpectedEof);
                        }
                        self.drop_bits(bits);
                        root_entry.value as u8
                    } else {
                        let nbits = (root_entry.bits as usize) - HUFFMAN_TABLE_BITS;
                        let sub_mask = (1usize << nbits) - 1;
                        let sub_offset = ((peeked as usize) >> HUFFMAN_TABLE_BITS) & sub_mask;
                        let sub_table_start = root_idx + root_entry.value as usize;
                        let sub_idx = sub_table_start + sub_offset;
                        let sub_entry = tree.entries[sub_idx];
                        let total_drop = (HUFFMAN_TABLE_BITS as u32) + (sub_entry.bits as u32);
                        if self.bit_pos < total_drop {
                            return Err(BrotliError::UnexpectedEof);
                        }
                        self.drop_bits(total_drop);
                        sub_entry.value as u8
                    }
                };

                self.ring_buffer.write_byte(lit);
                self.meta_block_remaining_len = self.meta_block_remaining_len.saturating_sub(1);
            }

            if self.meta_block_remaining_len == 0 {
                break;
            }

            // 2. Distance derivation
            let (distance, distance_context) = if cmd_elem.distance_code >= 0 {
                let ctx = if cmd_elem.distance_code != 0 { 0 } else { 1 };
                self.dist_rb_idx = (self.dist_rb_idx + 3) & 3;
                let d = self.dist_rb[self.dist_rb_idx];
                (d, ctx)
            } else {
                if self.block_length[2] == 0 {
                    self.decode_block_type_switch(2)?;
                }
                self.block_length[2] -= 1;

                let dist_ctx = cmd_elem.distance_context as usize;
                let ctx_map_idx = (self.block_type[2] << 2) + dist_ctx;
                let htree_idx = self.dist_context_map[ctx_map_idx] as usize;
                if htree_idx >= self.dist_htrees.len() {
                    return Err(BrotliError::CorruptHeader(
                        "Distance tree index out of range".into(),
                    ));
                }

                let dist_sym = {
                    let _ = self.refill_bit_window();
                    let peeked = self.bit_val as u32;
                    let root_idx = (peeked & (HUFFMAN_TABLE_MASK as u32)) as usize;
                    let tree = &self.dist_htrees[htree_idx];
                    let root_entry = tree.entries[root_idx];
                    if root_entry.bits <= (HUFFMAN_TABLE_BITS as u8) {
                        let bits = root_entry.bits as u32;
                        if self.bit_pos < bits {
                            return Err(BrotliError::UnexpectedEof);
                        }
                        self.drop_bits(bits);
                        root_entry.value as usize
                    } else {
                        let nbits = (root_entry.bits as usize) - HUFFMAN_TABLE_BITS;
                        let sub_mask = (1usize << nbits) - 1;
                        let sub_offset = ((peeked as usize) >> HUFFMAN_TABLE_BITS) & sub_mask;
                        let sub_table_start = root_idx + root_entry.value as usize;
                        let sub_idx = sub_table_start + sub_offset;
                        let sub_entry = tree.entries[sub_idx];
                        let total_drop = (HUFFMAN_TABLE_BITS as u32) + (sub_entry.bits as u32);
                        if self.bit_pos < total_drop {
                            return Err(BrotliError::UnexpectedEof);
                        }
                        self.drop_bits(total_drop);
                        sub_entry.value as usize
                    }
                };

                if dist_sym < 16 {
                    self.take_distance_from_ring_buffer(dist_sym)
                } else if dist_sym < 16 + self.num_direct_distance_codes {
                    (dist_sym - 15, 0)
                } else {
                    if dist_sym >= self.dist_extra_bits.len() {
                        return Err(BrotliError::CorruptHeader(
                            "Distance code out of table bounds".into(),
                        ));
                    }
                    let extra_bits = self.dist_extra_bits[dist_sym] as u32;
                    let extra = self.read_bits(extra_bits)? as usize;
                    let d = self.dist_offsets[dist_sym] + (extra << self.distance_postfix_bits);
                    (d, 0)
                }
            };

            // 3. Match copy or static dictionary word insertion
            let max_distance = self.ring_buffer.pos().min(self.window.max_distance);
            if distance <= max_distance {
                self.ring_buffer.copy_match(distance, copy_len)?;
                self.dist_rb[self.dist_rb_idx & 3] = distance;
                self.dist_rb_idx = (self.dist_rb_idx + 1) & 3;
                self.meta_block_remaining_len =
                    self.meta_block_remaining_len.saturating_sub(copy_len);
            } else {
                self.dist_rb_idx = (self.dist_rb_idx + distance_context) & 3;
                if !(4..=24).contains(&copy_len) {
                    return Err(BrotliError::CorruptHeader(format!(
                        "Invalid static dictionary word length: {copy_len}"
                    )));
                }
                let address = distance.saturating_sub(max_distance + 1);
                let shift = SIZE_BITS_BY_LENGTH[copy_len] as usize;
                if shift == 0 {
                    return Err(BrotliError::CorruptHeader(format!(
                        "No dictionary words for length {copy_len}"
                    )));
                }
                let mask = (1usize << shift) - 1;
                let word_idx = address & mask;
                let transform_idx = address >> shift;

                let word = get_dictionary_word(copy_len, word_idx).ok_or_else(|| {
                    BrotliError::CorruptHeader("Dictionary word index out of bounds".into())
                })?;

                let transformed_len =
                    transform_dictionary_word(&mut transform_buf, word, transform_idx)?;
                self.ring_buffer
                    .copy_slice(&transform_buf[..transformed_len]);
                self.meta_block_remaining_len = self
                    .meta_block_remaining_len
                    .saturating_sub(transformed_len);
            }

            if self.ring_buffer.available_data() >= 16384 {
                break;
            }
        }

        Ok(())
    }

    /// Advances the Finite State Machine (FSM) by one step.
    fn step_fsm(&mut self) -> Result<(), BrotliError> {
        match self.state {
            BrotliDecoderFsmState::Init | BrotliDecoderFsmState::ReadWindowBits => {
                let win = self.parse_window_bits_stream()?;
                self.window = win;
                self.ring_buffer = BrotliDecoderRingBuffer::new(win.window_bits)?;
                self.state = BrotliDecoderFsmState::ReadMetaBlockHeader;
                Ok(())
            }
            BrotliDecoderFsmState::ReadMetaBlockHeader => {
                let is_last = self.read_bits(1)? != 0;
                if is_last {
                    let is_last_empty = self.read_bits(1)? != 0;
                    if is_last_empty {
                        self.state = BrotliDecoderFsmState::Done;
                        return Ok(());
                    }
                }
                self.is_last_metablock = is_last;

                let mnibbles = self.read_bits(2)?;
                if mnibbles == 3 {
                    let reserved = self.read_bits(1)?;
                    if reserved != 0 {
                        return Err(BrotliError::CorruptHeader(
                            "Reserved bit non-zero in metadata block".into(),
                        ));
                    }
                    let mskipbytes = self.read_bits(2)?;
                    let mut mlen = 0usize;
                    for i in 0..mskipbytes {
                        let byte = self.read_bits(8)? as usize;
                        if i + 1 == mskipbytes && mskipbytes > 1 && byte == 0 {
                            return Err(BrotliError::CorruptHeader(
                                "Exuberant byte in metadata length".into(),
                            ));
                        }
                        mlen |= byte << (i * 8);
                    }
                    let uncompressed_len = if mskipbytes == 0 { 0 } else { mlen + 1 };
                    self.jump_to_byte_boundary()?;
                    self.meta_block_remaining_len = uncompressed_len;
                    self.state = BrotliDecoderFsmState::MetadataSkip;
                    return Ok(());
                }

                let size_nibbles = (mnibbles + 4) as usize;
                let mut mlen = 0usize;
                for i in 0..size_nibbles {
                    let nibble = self.read_bits(4)? as usize;
                    if i + 1 == size_nibbles && size_nibbles > 4 && nibble == 0 {
                        return Err(BrotliError::CorruptHeader(
                            "Exuberant nibble in metablock length".into(),
                        ));
                    }
                    mlen |= nibble << (i * 4);
                }
                let uncompressed_len = mlen + 1;
                self.meta_block_remaining_len = uncompressed_len;

                let is_uncompressed = if !is_last {
                    self.read_bits(1)? != 0
                } else {
                    false
                };

                if is_uncompressed {
                    self.jump_to_byte_boundary()?;
                    self.state = BrotliDecoderFsmState::UncompressedData;
                } else {
                    self.parse_compressed_metablock_header()?;
                    self.state = BrotliDecoderFsmState::CompressedCommands;
                }
                Ok(())
            }
            BrotliDecoderFsmState::UncompressedData => {
                self.read_uncompressed_chunk()?;
                if self.meta_block_remaining_len == 0 {
                    if self.is_last_metablock {
                        self.jump_to_byte_boundary()?;
                        self.state = BrotliDecoderFsmState::Done;
                    } else {
                        self.state = BrotliDecoderFsmState::ReadMetaBlockHeader;
                    }
                }
                Ok(())
            }
            BrotliDecoderFsmState::MetadataSkip => {
                self.skip_metadata_chunk()?;
                if self.meta_block_remaining_len == 0 {
                    if self.is_last_metablock {
                        self.jump_to_byte_boundary()?;
                        self.state = BrotliDecoderFsmState::Done;
                    } else {
                        self.state = BrotliDecoderFsmState::ReadMetaBlockHeader;
                    }
                }
                Ok(())
            }
            BrotliDecoderFsmState::CompressedCommands => {
                self.decode_commands_chunk()?;
                if self.meta_block_remaining_len == 0 {
                    if self.is_last_metablock {
                        self.jump_to_byte_boundary()?;
                        self.state = BrotliDecoderFsmState::Done;
                    } else {
                        self.state = BrotliDecoderFsmState::ReadMetaBlockHeader;
                    }
                }
                Ok(())
            }
            BrotliDecoderFsmState::Done => Ok(()),
        }
    }
}

impl<R: Read> Read for BrotliStreamDecoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        loop {
            if self.ring_buffer.available_data() > 0 {
                let drained = self.ring_buffer.drain_to(buf);
                if drained > 0 {
                    return Ok(drained);
                }
            }

            if self.state == BrotliDecoderFsmState::Done {
                return Ok(0);
            }

            match self.step_fsm() {
                Ok(()) => {}
                Err(BrotliError::UnexpectedEof) if self.state == BrotliDecoderFsmState::Done => {
                    return Ok(0);
                }
                Err(e) => return Err(io::Error::new(io::ErrorKind::InvalidData, e)),
            }
        }
    }
}
