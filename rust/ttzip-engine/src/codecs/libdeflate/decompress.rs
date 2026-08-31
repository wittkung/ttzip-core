// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, ultra-high-throughput RFC 1951 DEFLATE decompressor with branchless bitbuffer
//! refill, 2-level canonical Huffman decoding tables, and 3-tier SIMD Wild Copy match replication.
//!
//! # Architecture & Performance Invariants
//!
//! 1. **64-Bit Branchless Bitbuffer**:
//!    - Loads 64-bit words via unaligned little-endian reads.
//!    - Advances consumable bit count branchlessly: `bitsleft |= 56`.
//!
//! 2. **Compact 2-Level Decode Tables**:
//!    - Literal/Length: 11-bit main table (`LITLEN_TABLEBITS = 11`), dynamic subtable allocation (`LITLEN_ENOUGH = 2342`).
//!    - Offset: 8-bit main table (`OFFSET_TABLEBITS = 8`), dynamic subtable allocation (`OFFSET_ENOUGH = 402`).
//!    - Precode: 7-bit flat table (`PRECODE_TABLEBITS = 7`, `PRECODE_ENOUGH = 128`).
//!    - Entry bit-packing encodes symbol, codeword length, and extra bits in a single `u32`,
//!      allowing simultaneous bitstream consumption and extra-bit extraction with a single shift.
//!
//! 3. **3-Tier SIMD Match Copy (Fast Loop)**:
//!    - $D \ge 8$: 5x 8-byte (40B) unrolled Wild Copy via direct unaligned 64-bit load/stores.
//!    - $D == 1$: `[byte; 8]` SIMD broadcast pattern expansion.
//!    - $1 < D < 8$: Step-slide self-expanding vector copy.
//!    - Boundary safety guaranteed when output distance to end is $\ge 299$ bytes.
//!
//! 4. **Generic Boundary Loop**:
//!    - Byte-level bounds checking for buffer endpoints and arbitrary non-aligned streams.

use crate::types::TTZipStatus;
use super::decompress_tables::{
    build_decode_table_impl, FASTLOOP_MAX_BYTES_READ, FASTLOOP_MAX_BYTES_WRITTEN,
    HUFFDEC_END_OF_BLOCK, HUFFDEC_EXCEPTIONAL, HUFFDEC_LITERAL, HUFFDEC_SUBTABLE_POINTER,
    LITLEN_DECODE_RESULTS, LITLEN_ENOUGH, LITLEN_TABLEBITS, OFFSET_DECODE_RESULTS, OFFSET_ENOUGH,
    OFFSET_TABLEBITS, PRECODE_DECODE_RESULTS, PRECODE_ENOUGH, PRECODE_TABLEBITS,
};
use super::huffman::{
    DEFLATE_NUM_LITLEN_SYMS, DEFLATE_NUM_OFFSET_SYMS, DEFLATE_NUM_PRECODE_SYMS,
    DEFLATE_PRECODE_LENS_PERMUTATION,
};

// MARK: - Decompressor Context

/// Reusable full-buffer DEFLATE decompressor state allocating decode tables once.
pub struct LibdeflateDecompressor {
    /// Literal/length decode table (main table + dynamic subtables).
    pub litlen_table: [u32; LITLEN_ENOUGH],
    /// Offset decode table (main table + dynamic subtables).
    pub offset_table: [u32; OFFSET_ENOUGH],
    /// Precode decode table (flat table).
    pub precode_table: [u32; PRECODE_ENOUGH],
    /// Precode codeword lengths buffer.
    pub precode_lens: [u8; DEFLATE_NUM_PRECODE_SYMS],
    /// Combined litlen and offset codeword lengths buffer (with overrun padding).
    pub lens: [u8; DEFLATE_NUM_LITLEN_SYMS + DEFLATE_NUM_OFFSET_SYMS + 138],
    /// Indicates whether static Huffman tables are currently initialized.
    pub static_codes_loaded: bool,
}

impl Default for LibdeflateDecompressor {
    fn default() -> Self {
        Self::new()
    }
}

// MARK: - Bitstream Reader & Output Buffer Contexts

struct BitStreamReader<'a> {
    src: &'a [u8],
    in_pos: usize,
    bitbuf: u64,
    bitsleft: u32,
    overread_count: usize,
}

struct DstBufferCursor<'a> {
    dst: &'a mut [u8],
    dst_pos: usize,
}

#[inline(always)]
fn refill_branchless_ctx(reader: &mut BitStreamReader<'_>) {
    let word = unsafe {
        let ptr = reader.src.as_ptr().add(reader.in_pos) as *const u64;
        u64::from_le(std::ptr::read_unaligned(ptr))
    };
    reader.bitbuf |= word << (reader.bitsleft as u8);
    reader.in_pos += 7 - ((reader.bitsleft >> 3) & 0x7) as usize;
    reader.bitsleft |= 56;
}

#[inline(always)]
fn refill_bits_ctx(reader: &mut BitStreamReader<'_>) -> Result<(), TTZipStatus> {
    if reader.src.len().saturating_sub(reader.in_pos) >= 8 {
        refill_branchless_ctx(reader);
    } else {
        while (reader.bitsleft as u8) < 56 {
            if reader.in_pos < reader.src.len() {
                reader.bitbuf |= (reader.src[reader.in_pos] as u64) << (reader.bitsleft as u8);
                reader.in_pos += 1;
            } else {
                reader.overread_count += 1;
                if reader.overread_count > 8 {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
            }
            reader.bitsleft += 8;
        }
    }
    Ok(())
}

impl LibdeflateDecompressor {
    /// Allocates a new decompressor state instance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            litlen_table: [0u32; LITLEN_ENOUGH],
            offset_table: [0u32; OFFSET_ENOUGH],
            precode_table: [0u32; PRECODE_ENOUGH],
            precode_lens: [0u8; DEFLATE_NUM_PRECODE_SYMS],
            lens: [0u8; DEFLATE_NUM_LITLEN_SYMS + DEFLATE_NUM_OFFSET_SYMS + 138],
            static_codes_loaded: false,
        }
    }

    /// Decompresses raw RFC 1951 Deflate byte stream `src` into `dst`.
    pub fn decompress(&mut self, src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
        let mut reader = BitStreamReader {
            src,
            in_pos: 0,
            bitbuf: 0,
            bitsleft: 0,
            overread_count: 0,
        };
        let mut writer = DstBufferCursor {
            dst,
            dst_pos: 0,
        };

        loop {
            refill_bits_ctx(&mut reader)?;

            let is_final = (reader.bitbuf & 1) != 0;
            let block_type = ((reader.bitbuf >> 1) & 0x3) as u8;

            match block_type {
                0 => {
                    // Uncompressed block
                    reader.bitsleft -= 3;
                    let bits_avail = (reader.bitsleft as u8) as usize;
                    let unused_bytes = (bits_avail >> 3).saturating_sub(reader.overread_count);
                    if reader.in_pos < unused_bytes {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    reader.in_pos -= unused_bytes;
                    reader.overread_count = 0;
                    reader.bitbuf = 0;
                    reader.bitsleft = 0;

                    if reader.src.len().saturating_sub(reader.in_pos) < 4 {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    let len = u16::from_le_bytes([reader.src[reader.in_pos], reader.src[reader.in_pos + 1]]);
                    let nlen = u16::from_le_bytes([reader.src[reader.in_pos + 2], reader.src[reader.in_pos + 3]]);
                    reader.in_pos += 4;

                    if len != !nlen {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    let len = len as usize;
                    if writer.dst.len().saturating_sub(writer.dst_pos) < len {
                        return Err(TTZipStatus::ErrExtractionFailed);
                    }
                    if reader.src.len().saturating_sub(reader.in_pos) < len {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    writer.dst[writer.dst_pos..writer.dst_pos + len].copy_from_slice(&reader.src[reader.in_pos..reader.in_pos + len]);
                    writer.dst_pos += len;
                    reader.in_pos += len;
                }
                1 => {
                    // Static Huffman block
                    reader.bitbuf >>= 3;
                    reader.bitsleft -= 3;

                    if !self.static_codes_loaded {
                        let mut i = 0;
                        while i < 144 { self.lens[i] = 8; i += 1; }
                        while i < 256 { self.lens[i] = 9; i += 1; }
                        while i < 280 { self.lens[i] = 7; i += 1; }
                        while i < 288 { self.lens[i] = 8; i += 1; }
                        while i < 320 { self.lens[i] = 5; i += 1; }

                        build_decode_table_impl(
                            &self.lens[288..320],
                            32,
                            &OFFSET_DECODE_RESULTS,
                            OFFSET_TABLEBITS,
                            15,
                            &mut self.offset_table,
                        )?;
                        build_decode_table_impl(
                            &self.lens[..288],
                            288,
                            &LITLEN_DECODE_RESULTS,
                            LITLEN_TABLEBITS,
                            15,
                            &mut self.litlen_table,
                        )?;
                        self.static_codes_loaded = true;
                    }

                    decode_huffman_block(&mut reader, &mut writer, self)?;
                }
                2 => {
                    // Dynamic Huffman block
                    self.static_codes_loaded = false;

                    let num_litlen_syms = 257 + (((reader.bitbuf >> 3) & 0x1F) as usize);
                    let num_offset_syms = 1 + (((reader.bitbuf >> 8) & 0x1F) as usize);
                    let num_explicit_precode_lens = 4 + (((reader.bitbuf >> 13) & 0xF) as usize);

                    self.precode_lens[DEFLATE_PRECODE_LENS_PERMUTATION[0] as usize] =
                        ((reader.bitbuf >> 17) & 0x7) as u8;
                    reader.bitbuf >>= 20;
                    reader.bitsleft -= 20;

                    refill_bits_ctx(&mut reader)?;

                    let mut i = 1;
                    while i < num_explicit_precode_lens {
                        if (reader.bitsleft as u8) < 3 {
                            refill_bits_ctx(&mut reader)?;
                        }
                        self.precode_lens[DEFLATE_PRECODE_LENS_PERMUTATION[i] as usize] = (reader.bitbuf & 0x7) as u8;
                        reader.bitbuf >>= 3;
                        reader.bitsleft -= 3;
                        i += 1;
                    }
                    while i < DEFLATE_NUM_PRECODE_SYMS {
                        self.precode_lens[DEFLATE_PRECODE_LENS_PERMUTATION[i] as usize] = 0;
                        i += 1;
                    }

                    build_decode_table_impl(
                        &self.precode_lens,
                        DEFLATE_NUM_PRECODE_SYMS,
                        &PRECODE_DECODE_RESULTS,
                        PRECODE_TABLEBITS,
                        7,
                        &mut self.precode_table,
                    )?;

                    let total_syms = num_litlen_syms + num_offset_syms;
                    let mut sym_idx = 0;
                    while sym_idx < total_syms {
                        if (reader.bitsleft as u8) < 14 {
                            refill_bits_ctx(&mut reader)?;
                        }
                        let entry = self.precode_table[(reader.bitbuf & 0x7F) as usize];
                        reader.bitbuf >>= entry as u8;
                        reader.bitsleft -= (entry as u8) as u32;
                        let presym = (entry >> 16) as usize;

                        if presym < 16 {
                            self.lens[sym_idx] = presym as u8;
                            sym_idx += 1;
                        } else if presym == 16 {
                            if sym_idx == 0 {
                                return Err(TTZipStatus::ErrCorruptHeader);
                            }
                            let rep_val = self.lens[sym_idx - 1];
                            let rep_count = 3 + ((reader.bitbuf & 0x3) as usize);
                            reader.bitbuf >>= 2;
                            reader.bitsleft -= 2;
                            if sym_idx + rep_count > total_syms + 138 {
                                return Err(TTZipStatus::ErrCorruptHeader);
                            }
                            for k in 0..rep_count {
                                self.lens[sym_idx + k] = rep_val;
                            }
                            sym_idx += rep_count;
                        } else if presym == 17 {
                            let rep_count = 3 + ((reader.bitbuf & 0x7) as usize);
                            reader.bitbuf >>= 3;
                            reader.bitsleft -= 3;
                            if sym_idx + rep_count > total_syms + 138 {
                                return Err(TTZipStatus::ErrCorruptHeader);
                            }
                            for k in 0..rep_count {
                                self.lens[sym_idx + k] = 0;
                            }
                            sym_idx += rep_count;
                        } else {
                            let rep_count = 11 + ((reader.bitbuf & 0x7F) as usize);
                            reader.bitbuf >>= 7;
                            reader.bitsleft -= 7;
                            if sym_idx + rep_count > total_syms + 138 {
                                return Err(TTZipStatus::ErrCorruptHeader);
                            }
                            for k in 0..rep_count {
                                self.lens[sym_idx + k] = 0;
                            }
                            sym_idx += rep_count;
                        }
                    }

                    if sym_idx != total_syms {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }

                    build_decode_table_impl(
                        &self.lens[num_litlen_syms..total_syms],
                        num_offset_syms,
                        &OFFSET_DECODE_RESULTS,
                        OFFSET_TABLEBITS,
                        15,
                        &mut self.offset_table,
                    )?;
                    build_decode_table_impl(
                        &self.lens[..num_litlen_syms],
                        num_litlen_syms,
                        &LITLEN_DECODE_RESULTS,
                        LITLEN_TABLEBITS,
                        15,
                        &mut self.litlen_table,
                    )?;

                    decode_huffman_block(&mut reader, &mut writer, self)?;
                }
                _ => {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
            }

            if is_final {
                break;
            }
        }

        let bits_avail = (reader.bitsleft as u8) as usize;
        let unused_bytes = bits_avail >> 3;
        if reader.overread_count > unused_bytes {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        Ok(writer.dst_pos)
    }
}

// MARK: - Huffman Block Decoder (Fast Loop & Generic Loop)

fn decode_huffman_block(
    reader: &mut BitStreamReader<'_>,
    writer: &mut DstBufferCursor<'_>,
    d: &LibdeflateDecompressor,
) -> Result<(), TTZipStatus> {
    let in_fast_end = reader.src.len().saturating_sub(FASTLOOP_MAX_BYTES_READ);
    let dst_fast_end = writer.dst.len().saturating_sub(FASTLOOP_MAX_BYTES_WRITTEN);

    // Fast Loop Execution
    if reader.in_pos < in_fast_end && writer.dst_pos < dst_fast_end {
        refill_branchless_ctx(reader);
        let mut entry = d.litlen_table[(reader.bitbuf & 0x7FF) as usize];

        loop {
            let mut saved_bitbuf = reader.bitbuf;
            reader.bitbuf >>= entry as u8;
            reader.bitsleft -= (entry as u8) as u32;

            if (entry & HUFFDEC_LITERAL) != 0 {
                let lit = (entry >> 16) as u8;
                entry = d.litlen_table[(reader.bitbuf & 0x7FF) as usize];
                saved_bitbuf = reader.bitbuf;
                reader.bitbuf >>= entry as u8;
                reader.bitsleft -= (entry as u8) as u32;
                writer.dst[writer.dst_pos] = lit;
                writer.dst_pos += 1;

                if (entry & HUFFDEC_LITERAL) != 0 {
                    let lit2 = (entry >> 16) as u8;
                    entry = d.litlen_table[(reader.bitbuf & 0x7FF) as usize];
                    saved_bitbuf = reader.bitbuf;
                    reader.bitbuf >>= entry as u8;
                    reader.bitsleft -= (entry as u8) as u32;
                    writer.dst[writer.dst_pos] = lit2;
                    writer.dst_pos += 1;

                    if (entry & HUFFDEC_LITERAL) != 0 {
                        let lit3 = (entry >> 16) as u8;
                        entry = d.litlen_table[(reader.bitbuf & 0x7FF) as usize];
                        refill_branchless_ctx(reader);
                        writer.dst[writer.dst_pos] = lit3;
                        writer.dst_pos += 1;
                        if reader.in_pos >= in_fast_end || writer.dst_pos >= dst_fast_end {
                            break;
                        }
                        continue;
                    }
                }
            }

            if (entry & HUFFDEC_EXCEPTIONAL) != 0 {
                if (entry & HUFFDEC_END_OF_BLOCK) != 0 {
                    return Ok(());
                }
                let sub_bits = ((entry >> 8) & 0x3F) as usize;
                let sub_idx = (entry >> 16) as usize + ((reader.bitbuf as usize) & ((1 << sub_bits) - 1));
                entry = d.litlen_table[sub_idx];
                saved_bitbuf = reader.bitbuf;
                reader.bitbuf >>= entry as u8;
                reader.bitsleft -= (entry as u8) as u32;

                if (entry & HUFFDEC_LITERAL) != 0 {
                    let lit = (entry >> 16) as u8;
                    entry = d.litlen_table[(reader.bitbuf & 0x7FF) as usize];
                    refill_branchless_ctx(reader);
                    writer.dst[writer.dst_pos] = lit;
                    writer.dst_pos += 1;
                    if reader.in_pos >= in_fast_end || writer.dst_pos >= dst_fast_end {
                        break;
                    }
                    continue;
                }
                if (entry & HUFFDEC_END_OF_BLOCK) != 0 {
                    return Ok(());
                }
            }

            let mut length = (entry >> 16) as usize;
            let len_shift = ((entry >> 8) & 0xFF) as u8;
            let len_mask = (1u64 << (entry as u8)) - 1;
            length += ((saved_bitbuf & len_mask) >> len_shift) as usize;

            let mut off_entry = d.offset_table[(reader.bitbuf & 0xFF) as usize];
            if (off_entry & HUFFDEC_EXCEPTIONAL) != 0 {
                if (reader.bitsleft as u8) < 28 {
                    refill_branchless_ctx(reader);
                }
                reader.bitbuf >>= OFFSET_TABLEBITS;
                reader.bitsleft -= OFFSET_TABLEBITS as u32;
                let sub_bits = ((off_entry >> 8) & 0x3F) as usize;
                let sub_idx = (off_entry >> 16) as usize + ((reader.bitbuf as usize) & ((1 << sub_bits) - 1));
                off_entry = d.offset_table[sub_idx];
            } else if (reader.bitsleft as u8) < 28 {
                refill_branchless_ctx(reader);
            }

            saved_bitbuf = reader.bitbuf;
            reader.bitbuf >>= off_entry as u8;
            reader.bitsleft -= (off_entry as u8) as u32;
            let mut offset = (off_entry >> 16) as usize;
            let off_shift = ((off_entry >> 8) & 0xFF) as u8;
            let off_mask = (1u64 << (off_entry as u8)) - 1;
            offset += ((saved_bitbuf & off_mask) >> off_shift) as usize;

            if offset == 0 || offset > writer.dst_pos {
                return Err(TTZipStatus::ErrCorruptHeader);
            }

            entry = d.litlen_table[(reader.bitbuf & 0x7FF) as usize];
            refill_branchless_ctx(reader);

            let match_end = writer.dst_pos + length;
            let src_pos = writer.dst_pos - offset;

            if offset >= 8 {
                // Tier 1: 5 x 8B (40B) unrolled Wild Copy
                let mut s = src_pos;
                let mut d_cur = writer.dst_pos;
                unsafe {
                    let base_ptr = writer.dst.as_mut_ptr();
                    let w0 = std::ptr::read_unaligned(base_ptr.add(s) as *const [u8; 8]);
                    std::ptr::write_unaligned(base_ptr.add(d_cur) as *mut [u8; 8], w0);
                    s += 8; d_cur += 8;
                    let w1 = std::ptr::read_unaligned(base_ptr.add(s) as *const [u8; 8]);
                    std::ptr::write_unaligned(base_ptr.add(d_cur) as *mut [u8; 8], w1);
                    s += 8; d_cur += 8;
                    let w2 = std::ptr::read_unaligned(base_ptr.add(s) as *const [u8; 8]);
                    std::ptr::write_unaligned(base_ptr.add(d_cur) as *mut [u8; 8], w2);
                    s += 8; d_cur += 8;
                    let w3 = std::ptr::read_unaligned(base_ptr.add(s) as *const [u8; 8]);
                    std::ptr::write_unaligned(base_ptr.add(d_cur) as *mut [u8; 8], w3);
                    s += 8; d_cur += 8;
                    let w4 = std::ptr::read_unaligned(base_ptr.add(s) as *const [u8; 8]);
                    std::ptr::write_unaligned(base_ptr.add(d_cur) as *mut [u8; 8], w4);
                    s += 8; d_cur += 8;
                    while d_cur < match_end {
                        let w = std::ptr::read_unaligned(base_ptr.add(s) as *const [u8; 8]);
                        std::ptr::write_unaligned(base_ptr.add(d_cur) as *mut [u8; 8], w);
                        s += 8; d_cur += 8;
                    }
                }
                writer.dst_pos = match_end;
            } else if offset == 1 {
                // Tier 2: SIMD 0x0101010101010101 broadcast pattern expansion
                let byte = writer.dst[writer.dst_pos - 1];
                let v = [byte; 8];
                let mut d_cur = writer.dst_pos;
                unsafe {
                    let base_ptr = writer.dst.as_mut_ptr();
                    std::ptr::write_unaligned(base_ptr.add(d_cur) as *mut [u8; 8], v);
                    d_cur += 8;
                    std::ptr::write_unaligned(base_ptr.add(d_cur) as *mut [u8; 8], v);
                    d_cur += 8;
                    std::ptr::write_unaligned(base_ptr.add(d_cur) as *mut [u8; 8], v);
                    d_cur += 8;
                    std::ptr::write_unaligned(base_ptr.add(d_cur) as *mut [u8; 8], v);
                    d_cur += 8;
                    std::ptr::write_unaligned(base_ptr.add(d_cur) as *mut [u8; 8], v);
                    d_cur += 8;
                    while d_cur < match_end {
                        std::ptr::write_unaligned(base_ptr.add(d_cur) as *mut [u8; 8], v);
                        d_cur += 8;
                    }
                }
                writer.dst_pos = match_end;
            } else {
                // Tier 3: Step-slide self-expanding vector copy
                let mut s = src_pos;
                let mut d_cur = writer.dst_pos;
                unsafe {
                    let base_ptr = writer.dst.as_mut_ptr();
                    let w0 = std::ptr::read_unaligned(base_ptr.add(s) as *const [u8; 8]);
                    std::ptr::write_unaligned(base_ptr.add(d_cur) as *mut [u8; 8], w0);
                    s += offset; d_cur += offset;
                    let w1 = std::ptr::read_unaligned(base_ptr.add(s) as *const [u8; 8]);
                    std::ptr::write_unaligned(base_ptr.add(d_cur) as *mut [u8; 8], w1);
                    s += offset; d_cur += offset;
                    while d_cur < match_end {
                        let w = std::ptr::read_unaligned(base_ptr.add(s) as *const [u8; 8]);
                        std::ptr::write_unaligned(base_ptr.add(d_cur) as *mut [u8; 8], w);
                        s += offset; d_cur += offset;
                    }
                }
                writer.dst_pos = match_end;
            }

            if reader.in_pos >= in_fast_end || writer.dst_pos >= dst_fast_end {
                break;
            }
        }
    }

    // Generic Boundary Loop (safe byte-by-byte boundary loop)
    loop {
        refill_bits_ctx(reader)?;
        let mut entry = d.litlen_table[(reader.bitbuf & 0x7FF) as usize];
        let mut saved_bitbuf = reader.bitbuf;
        reader.bitbuf >>= entry as u8;
        reader.bitsleft -= (entry as u8) as u32;

        if (entry & HUFFDEC_SUBTABLE_POINTER) != 0 {
            let sub_bits = ((entry >> 8) & 0x3F) as usize;
            let sub_idx = (entry >> 16) as usize + ((reader.bitbuf as usize) & ((1 << sub_bits) - 1));
            entry = d.litlen_table[sub_idx];
            saved_bitbuf = reader.bitbuf;
            reader.bitbuf >>= entry as u8;
            reader.bitsleft -= (entry as u8) as u32;
        }

        if (entry & HUFFDEC_LITERAL) != 0 {
            if writer.dst_pos >= writer.dst.len() {
                return Err(TTZipStatus::ErrExtractionFailed);
            }
            writer.dst[writer.dst_pos] = (entry >> 16) as u8;
            writer.dst_pos += 1;
            continue;
        }

        if (entry & HUFFDEC_END_OF_BLOCK) != 0 {
            return Ok(());
        }

        let mut length = (entry >> 16) as usize;
        let len_shift = ((entry >> 8) & 0xFF) as u8;
        let len_mask = (1u64 << (entry as u8)) - 1;
        length += ((saved_bitbuf & len_mask) >> len_shift) as usize;

        if writer.dst.len().saturating_sub(writer.dst_pos) < length {
            return Err(TTZipStatus::ErrExtractionFailed);
        }

        if (reader.bitsleft as u8) < 28 {
            refill_bits_ctx(reader)?;
        }

        let mut off_entry = d.offset_table[(reader.bitbuf & 0xFF) as usize];
        if (off_entry & HUFFDEC_EXCEPTIONAL) != 0 {
            reader.bitbuf >>= OFFSET_TABLEBITS;
            reader.bitsleft -= OFFSET_TABLEBITS as u32;
            let sub_bits = ((off_entry >> 8) & 0x3F) as usize;
            let sub_idx = (off_entry >> 16) as usize + ((reader.bitbuf as usize) & ((1 << sub_bits) - 1));
            off_entry = d.offset_table[sub_idx];
            if (reader.bitsleft as u8) < 18 {
                refill_bits_ctx(reader)?;
            }
        }

        saved_bitbuf = reader.bitbuf;
        reader.bitbuf >>= off_entry as u8;
        reader.bitsleft -= (off_entry as u8) as u32;
        let mut offset = (off_entry >> 16) as usize;
        let off_shift = ((off_entry >> 8) & 0xFF) as u8;
        let off_mask = (1u64 << (off_entry as u8)) - 1;
        offset += ((saved_bitbuf & off_mask) >> off_shift) as usize;

        if offset == 0 || offset > writer.dst_pos {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let start_s = writer.dst_pos - offset;
        for s in (start_s..).take(length) {
            writer.dst[writer.dst_pos] = writer.dst[s];
            writer.dst_pos += 1;
        }
    }
}

// MARK: - Top-Level Entry Point

/// Decompresses raw RFC 1951 Deflate byte stream `src` into destination slice `dst`.
///
/// Returns the number of decompressed bytes written on success, or a specific `TTZipStatus` error.
pub fn deflate_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    let mut decompressor = LibdeflateDecompressor::new();
    decompressor.decompress(src, dst)
}
