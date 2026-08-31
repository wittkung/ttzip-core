// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Bzip2 48-bit Pi magic Block container parser and bitstream encoder.
//!
//! Magic constants:
//! - Block Header: 0x314159265359 (BCD representation of Pi)
//! - Stream Trailer: 0x177245385090 (BCD representation of Sqrt(Pi))

use crate::types::TTZipStatus;
use super::crc::{Bzip2Crc32, Bzip2CombinedCrc};
use super::blocksort::bwt_block_sort;
use super::mtf::{generate_mtf_values, rle2_decode_and_inverse_mtf, rle1_compress, rle1_decompress, MAX_ALPHA_SIZE};
use super::huffman::{
    hb_make_code_lengths, hb_assign_codes, hb_create_decode_tables,
    huffman_decode_symbol, BitReader, BZ_N_GROUPS, BZ_G_SIZE, BZ_MAX_CODE_LEN,
};
use super::inverse_bwt::inverse_bwt_fast;

pub const BZIP2_BLOCK_MAGIC: [u8; 6] = [0x31, 0x41, 0x59, 0x26, 0x53, 0x59];
pub const BZIP2_EOS_MAGIC: [u8; 6] = [0x17, 0x72, 0x45, 0x38, 0x50, 0x90];

/// MSB-first BitWriter for serializing Bzip2 blocks.
pub struct BitWriter {
    pub buf: Vec<u8>,
    bit_buf: u64,
    bits_live: u32,
}

impl Default for BitWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl BitWriter {
    pub fn new() -> Self {
        Self {
            buf: Vec::with_capacity(1024),
            bit_buf: 0,
            bits_live: 0,
        }
    }

    #[inline]
    pub fn write_bit(&mut self, bit: u32) {
        self.write_bits(bit & 1, 1);
    }

    #[inline]
    pub fn write_bits(&mut self, val: u32, n: u32) {
        if n == 0 {
            return;
        }
        self.bit_buf = (self.bit_buf << n) | (val as u64 & ((1u64 << n) - 1));
        self.bits_live += n;
        while self.bits_live >= 8 {
            let shift = self.bits_live - 8;
            let byte = (self.bit_buf >> shift) as u8;
            self.buf.push(byte);
            self.bits_live -= 8;
            if self.bits_live == 0 {
                self.bit_buf = 0;
            } else {
                self.bit_buf &= (1u64 << self.bits_live) - 1;
            }
        }
    }

    pub fn flush_to_byte_boundary(&mut self) {
        if self.bits_live > 0 {
            let shift = 8 - self.bits_live;
            let byte = ((self.bit_buf & ((1u64 << self.bits_live) - 1)) << shift) as u8;
            self.buf.push(byte);
            self.bit_buf = 0;
            self.bits_live = 0;
        }
    }
}

/// Encodes a single block of raw data into Bzip2 bitstream format.
pub fn encode_bzip2_block(
    raw_block: &[u8],
    writer: &mut BitWriter,
    combined_crc: &mut Bzip2CombinedCrc,
) -> Result<(), TTZipStatus> {
    if raw_block.is_empty() {
        return Ok(());
    }

    // 1. Calculate Block CRC
    let block_crc = Bzip2Crc32::calculate(raw_block);
    combined_crc.update_block(block_crc);

    // 2. RLE1 input preprocessing
    let mut rle1_buf = Vec::with_capacity(raw_block.len() + 32);
    rle1_compress(raw_block, &mut rle1_buf);

    // 3. BWT Block Sort
    let (orig_ptr, transformed_l) = bwt_block_sort(&rle1_buf, 30)?;

    // 4. In-use symbol bitmap
    let mut in_use = [false; 256];
    for &b in &transformed_l {
        in_use[b as usize] = true;
    }

    // 5. MTF Transform + RLE2 zero runs
    let mut mtf_symbols = Vec::with_capacity(transformed_l.len());
    let mut mtf_freq = [0u32; MAX_ALPHA_SIZE];
    generate_mtf_values(&transformed_l, &in_use, &mut mtf_symbols, &mut mtf_freq);

    let mut n_in_use = 0;
    for &used in &in_use {
        if used {
            n_in_use += 1;
        }
    }
    let alpha_size = n_in_use + 2;

    // 6. Write Block Header
    for &b in &BZIP2_BLOCK_MAGIC {
        writer.write_bits(b as u32, 8);
    }
    writer.write_bits(block_crc, 32);
    writer.write_bit(0); // randomised = 0
    writer.write_bits(orig_ptr as u32, 24);

    // 7. Write Symbol In-Use 16-bit 2-Level Bitmap
    for i in 0..16 {
        let mut group_used = false;
        for j in 0..16 {
            if in_use[i * 16 + j] {
                group_used = true;
                break;
            }
        }
        writer.write_bit(if group_used { 1 } else { 0 });
    }
    for i in 0..16 {
        let mut group_used = false;
        for j in 0..16 {
            if in_use[i * 16 + j] {
                group_used = true;
                break;
            }
        }
        if group_used {
            for j in 0..16 {
                writer.write_bit(if in_use[i * 16 + j] { 1 } else { 0 });
            }
        }
    }

    // 8. Huffman Tree Construction (Single tree fallback or multi-table)
    let n_groups = 2;
    let n_selectors = mtf_symbols.len().div_ceil(BZ_G_SIZE);

    writer.write_bits(n_groups as u32, 3);
    writer.write_bits(n_selectors as u32, 15);

    // Selectors: all set to table 0 (unary MTF = 0 -> bit '0')
    for _ in 0..n_selectors {
        writer.write_bit(0);
    }

    // Create Canonical Huffman lengths for each table
    let mut lengths = vec![vec![0u8; alpha_size]; n_groups];
    for t in 0..n_groups {
        hb_make_code_lengths(&mut lengths[t], &mtf_freq[0..alpha_size], alpha_size, 17);
    }

    // Emit Delta Code Lengths for each table
    for t in 0..n_groups {
        let mut curr = lengths[t][0];
        writer.write_bits(curr as u32, 5);
        for i in 0..alpha_size {
            let target = lengths[t][i];
            while curr < target {
                writer.write_bits(0b10, 2); // Increment
                curr += 1;
            }
            while curr > target {
                writer.write_bits(0b11, 2); // Decrement
                curr -= 1;
            }
            writer.write_bit(0); // Accept
        }
    }

    // Compute codes
    let mut codes = vec![0i32; alpha_size];
    let mut min_len = 20;
    let mut max_len = 1;
    for &l in &lengths[0] {
        if l > 0 {
            min_len = min_len.min(l as usize);
            max_len = max_len.max(l as usize);
        }
    }
    hb_assign_codes(&mut codes, &lengths[0], min_len, max_len, alpha_size);

    // 9. Emit Huffman-encoded MTF Payload
    for &sym in &mtf_symbols {
        let sym_idx = sym as usize;
        let code = codes[sym_idx] as u32;
        let len = lengths[0][sym_idx] as u32;
        writer.write_bits(code, len);
    }

    Ok(())
}

/// Decodes a single Bzip2 block from a bit reader.
pub fn decode_bzip2_block(
    reader: &mut BitReader,
    dst: &mut Vec<u8>,
    combined_crc: &mut Bzip2CombinedCrc,
) -> Result<bool, TTZipStatus> {
    // 1. Read first byte to determine if Block Magic (0x31) or EOS Magic (0x17)
    let first_byte = match reader.read_bits(8) {
        Ok(b) => b as u8,
        Err(TTZipStatus::ErrCorruptHeader) => return Ok(false), // Normal EOF
        Err(e) => return Err(e),
    };

    if first_byte == BZIP2_EOS_MAGIC[0] {
        // Read remaining 5 bytes of EOS Magic
        for &expected in &BZIP2_EOS_MAGIC[1..] {
            let b = reader.read_bits(8)? as u8;
            if b != expected {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
        }
        let stored_combined_crc = reader.read_bits(32)?;
        if stored_combined_crc != combined_crc.finalize() {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        return Ok(false); // End of stream reached
    }

    if first_byte != BZIP2_BLOCK_MAGIC[0] {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    for &expected in &BZIP2_BLOCK_MAGIC[1..] {
        let b = reader.read_bits(8)? as u8;
        if b != expected {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
    }

    let stored_block_crc = reader.read_bits(32)?;
    let _randomised = reader.read_bit()?;
    let orig_ptr = reader.read_bits(24)? as usize;

    // 2. Read In-Use Bitmaps
    let mut in_use16 = [false; 16];
    for i in 0..16 {
        in_use16[i] = reader.read_bit()? == 1;
    }

    let mut in_use = [false; 256];
    let mut n_in_use = 0;
    for i in 0..16 {
        if in_use16[i] {
            for j in 0..16 {
                if reader.read_bit()? == 1 {
                    in_use[i * 16 + j] = true;
                    n_in_use += 1;
                }
            }
        }
    }

    if n_in_use == 0 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let alpha_size = n_in_use + 2;

    // 3. Read Huffman Metadata
    let n_groups = reader.read_bits(3)? as usize;
    if !(2..=BZ_N_GROUPS).contains(&n_groups) {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let n_selectors = reader.read_bits(15)? as usize;
    if n_selectors == 0 || n_selectors > 18002 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    // Read Unary MTF Selectors
    let mut pos = (0..n_groups as u8).collect::<Vec<_>>();
    let mut selectors = Vec::with_capacity(n_selectors);
    for _ in 0..n_selectors {
        let mut count = 0;
        while reader.read_bit()? == 1 {
            count += 1;
            if count >= n_groups {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
        }
        let sel = pos[count];
        pos.remove(count);
        pos.insert(0, sel);
        selectors.push(sel as usize);
    }

    // 4. Read Delta Code Lengths and build decode tables
    let mut lengths = vec![vec![0u8; alpha_size]; n_groups];
    let mut limits = vec![vec![0i32; BZ_MAX_CODE_LEN + 2]; n_groups];
    let mut bases = vec![vec![0i32; BZ_MAX_CODE_LEN + 2]; n_groups];
    let mut perms = vec![vec![0i32; alpha_size]; n_groups];
    let mut min_lens = vec![20; n_groups];

    for t in 0..n_groups {
        let mut curr = reader.read_bits(5)? as u8;
        for i in 0..alpha_size {
            loop {
                if reader.read_bit()? == 0 {
                    break;
                }
                if reader.read_bit()? == 0 {
                    curr += 1;
                } else {
                    if curr == 0 {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    curr -= 1;
                }
                if !(1..=20).contains(&curr) {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
            }
            if !(1..=20).contains(&curr) {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            lengths[t][i] = curr;
        }

        let mut min_l = 20;
        let mut max_l = 1;
        for &l in &lengths[t] {
            if l > 0 {
                min_l = min_l.min(l as usize);
                max_l = max_l.max(l as usize);
            }
        }
        if min_l < 1 || max_l > 20 || min_l > max_l {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        min_lens[t] = min_l;

        hb_create_decode_tables(
            &mut limits[t],
            &mut bases[t],
            &mut perms[t],
            &lengths[t],
            min_l,
            max_l,
            alpha_size,
        );
    }

    // 5. Decode Huffman Symbols into MTF Sequence
    let mut mtf_symbols = Vec::new();
    let mut group_idx = 0;
    let mut group_pos = 0;
    let eob = (n_in_use + 1) as u16;

    loop {
        if group_pos == 0 {
            if group_idx >= n_selectors {
                return Err(TTZipStatus::ErrExtractionFailed);
            }
            group_pos = BZ_G_SIZE;
        }
        group_pos -= 1;
        let g_sel = selectors[group_idx];
        if group_pos == 0 {
            group_idx += 1;
        }

        let sym = huffman_decode_symbol(
            reader,
            &limits[g_sel],
            &bases[g_sel],
            &perms[g_sel],
            min_lens[g_sel],
        )?;

        if sym == eob {
            mtf_symbols.push(sym);
            break;
        }
        mtf_symbols.push(sym);
    }

    // 6. Inverse MTF + RLE2 to restore L column
    let mut transformed_l = Vec::new();
    rle2_decode_and_inverse_mtf(&mtf_symbols, &in_use, &mut transformed_l)?;

    // 7. Inverse BWT to restore RLE1 stream
    let mut rle1_restored = vec![0u8; transformed_l.len()];
    inverse_bwt_fast(&transformed_l, orig_ptr, &mut rle1_restored)?;

    // 8. Inverse RLE1 to restore original decompressed payload
    let mut uncompressed_block = Vec::new();
    rle1_decompress(&rle1_restored, &mut uncompressed_block)?;

    // 9. Verify Block CRC & update combined CRC
    let calc_crc = Bzip2Crc32::calculate(&uncompressed_block);
    if calc_crc != stored_block_crc {
        return Err(TTZipStatus::ErrExtractionFailed);
    }
    combined_crc.update_block(calc_crc);

    dst.extend_from_slice(&uncompressed_block);
    Ok(true) // Successfully decoded one block
}
