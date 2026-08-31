// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! `bzip2recover` bit-level 48-bit Pi magic scanner and disaster recovery engine.
//!
//! Scans unaligned bitstreams for the 48-bit block magic (0x314159265359) and EOS magic
//! (0x177245385090), isolating corrupted partitions and salvaging healthy blocks into
//! standalone, fully compliant `.bz2` streams.

use crate::types::TTZipStatus;
use super::block::{BZIP2_EOS_MAGIC, BitWriter};

pub const BLOCK_MAGIC_U64: u64 = 0x0000_3141_5926_5359;
pub const EOS_MAGIC_U64: u64 = 0x0000_1772_4538_5090;

/// Identified valid block slice in a bitstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bzip2BlockSlice {
    pub index: usize,
    pub bit_start: u64,
    pub bit_end: u64,
}

/// Bit-level scanner for locating all 48-bit block markers in a `.bz2` bitstream.
pub fn bzip2_scan_blocks(data: &[u8]) -> Vec<Bzip2BlockSlice> {
    let mut slices = Vec::new();
    let total_bits = (data.len() as u64) * 8;
    if total_bits < 48 {
        return slices;
    }

    let mut window: u64 = 0;
    let mut starts = Vec::new();

    for bit_idx in 0..total_bits {
        let byte_idx = (bit_idx / 8) as usize;
        let bit_in_byte = 7 - (bit_idx % 8);
        let bit = ((data[byte_idx] >> bit_in_byte) & 1) as u64;

        window = ((window << 1) | bit) & 0x0000_FFFF_FFFF_FFFF;

        if bit_idx >= 47 {
            let pattern_start = bit_idx - 47;
            if window == BLOCK_MAGIC_U64 {
                starts.push((pattern_start, false));
            } else if window == EOS_MAGIC_U64 {
                starts.push((pattern_start, true));
            }
        }
    }

    for i in 0..starts.len() {
        let (start_bit, is_eos) = starts[i];
        if is_eos {
            continue;
        }

        let end_bit = if i + 1 < starts.len() {
            starts[i + 1].0
        } else {
            total_bits
        };

        if end_bit > start_bit + 48 {
            slices.push(Bzip2BlockSlice {
                index: slices.len() + 1,
                bit_start: start_bit,
                bit_end: end_bit,
            });
        }
    }

    slices
}

/// Reconstructs an isolated, standalone `.bz2` archive from a single salvaged block slice.
pub fn bzip2_recover_block(data: &[u8], slice: &Bzip2BlockSlice) -> Result<Vec<u8>, TTZipStatus> {
    if slice.bit_end <= slice.bit_start || slice.bit_end > (data.len() as u64) * 8 {
        return Err(TTZipStatus::ErrExtractionFailed);
    }

    let mut writer = BitWriter::new();

    // 1. Write standard stream header: 'B', 'Z', 'h', '9'
    writer.write_bits(b'B' as u32, 8);
    writer.write_bits(b'Z' as u32, 8);
    writer.write_bits(b'h' as u32, 8);
    writer.write_bits(b'9' as u32, 8);

    // 2. Stream all bits of the slice from bit_start to bit_end
    for bit_idx in slice.bit_start..slice.bit_end {
        let byte_idx = (bit_idx / 8) as usize;
        let bit_in_byte = 7 - (bit_idx % 8);
        let bit = ((data[byte_idx] >> bit_in_byte) & 1) as u32;
        writer.write_bit(bit);
    }

    // 3. Synthesize 48-bit Stream Trailer: 0x177245385090
    for &b in &BZIP2_EOS_MAGIC {
        writer.write_bits(b as u32, 8);
    }

    // Read stored block CRC from bits (bit_start + 48 .. bit_start + 80)
    let mut block_crc: u32 = 0;
    if slice.bit_start + 80 <= slice.bit_end {
        for bit_idx in (slice.bit_start + 48)..(slice.bit_start + 80) {
            let byte_idx = (bit_idx / 8) as usize;
            let bit_in_byte = 7 - (bit_idx % 8);
            let bit = ((data[byte_idx] >> bit_in_byte) & 1) as u32;
            block_crc = (block_crc << 1) | bit;
        }
    }

    // Combined CRC for a single block is equal to block_crc (0.rotate_left(1) ^ crc = crc)
    writer.write_bits(block_crc, 32);

    // 4. Byte-align with zero padding
    writer.flush_to_byte_boundary();

    Ok(writer.buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bzip2_scan_empty() {
        let empty = b"";
        let slices = bzip2_scan_blocks(empty);
        assert!(slices.is_empty());
    }
}
