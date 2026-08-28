// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! ARM64 (AArch64) Branch Conversion Filter (BCJ ARM64).
//!
//! Converts relative branch targets in ARM64 BL (`0x94..0x97`) and
//! ADRP (`0x90/0x9F`) instructions to absolute addresses to reduce entropy
//! and increase match lengths for LZMA/LZMA2 compressors.

use super::BranchFilter;

/// ARM64 instruction opcode masks and identifiers.
const ARM64_BL_MASK: u32 = 0xFC00_0000;
const ARM64_BL_OPCODE: u32 = 0x9400_0000;
const ARM64_BL_IMM_MASK: u32 = 0x03FF_FFFF;

const ARM64_ADRP_MASK: u32 = 0x9F00_0000;
const ARM64_ADRP_OPCODE: u32 = 0x9000_0000;
const ARM64_ADRP_IMM_MASK: u32 = 0x001F_FFFF;
const ARM64_ADRP_PRESERVE_MASK: u32 = 0x9F00_001F;

/// Normalizes ARM64 BL and ADRP relative branch targets to absolute addresses in-place.
///
/// Returns the number of bytes processed (aligned to 4-byte boundaries).
pub fn arm64_encode(data: &mut [u8], ip: u32) -> usize {
    arm64_transform(data, ip, true)
}

/// Restores ARM64 BL and ADRP relative branch targets from absolute addresses in-place.
///
/// Returns the number of bytes processed (aligned to 4-byte boundaries).
pub fn arm64_decode(data: &mut [u8], ip: u32) -> usize {
    arm64_transform(data, ip, false)
}

/// Core transformation loop for ARM64 BL and ADRP instructions.
#[inline]
fn arm64_transform(data: &mut [u8], ip: u32, is_encode: bool) -> usize {
    let aligned_len = data.len() & !3;
    let mut pos = 0;

    while pos < aligned_len {
        let mut instr = u32::from_le_bytes([
            data[pos],
            data[pos + 1],
            data[pos + 2],
            data[pos + 3],
        ]);

        if (instr & ARM64_BL_MASK) == ARM64_BL_OPCODE {
            // BL instruction: 26-bit PC-relative word offset
            let src = instr & ARM64_BL_IMM_MASK;
            let pc_word = (ip.wrapping_add(pos as u32)) >> 2;
            let converted = if is_encode {
                src.wrapping_add(pc_word) & ARM64_BL_IMM_MASK
            } else {
                src.wrapping_sub(pc_word) & ARM64_BL_IMM_MASK
            };
            instr = (instr & ARM64_BL_MASK) | converted;
            let bytes = instr.to_le_bytes();
            data[pos..pos + 4].copy_from_slice(&bytes);
        } else if (instr & ARM64_ADRP_MASK) == ARM64_ADRP_OPCODE {
            // ADRP instruction: 21-bit PC-relative 4KB page displacement
            // immlo: bits 30..29, immhi: bits 23..5
            let immlo = (instr >> 29) & 0x3;
            let immhi = (instr >> 5) & 0x7_FFFF;
            let src = immlo | (immhi << 2);

            let pc_page = (ip.wrapping_add(pos as u32)) >> 12;
            let converted = if is_encode {
                src.wrapping_add(pc_page) & ARM64_ADRP_IMM_MASK
            } else {
                src.wrapping_sub(pc_page) & ARM64_ADRP_IMM_MASK
            };

            let new_immlo = (converted & 0x3) << 29;
            let new_immhi = ((converted >> 2) & 0x7_FFFF) << 5;
            instr = (instr & ARM64_ADRP_PRESERVE_MASK) | new_immlo | new_immhi;
            let bytes = instr.to_le_bytes();
            data[pos..pos + 4].copy_from_slice(&bytes);
        }

        pos += 4;
    }

    aligned_len
}

/// Zero-cost stateless ARM64 Branch Filter implementing `BranchFilter`.
#[derive(Debug, Clone, Copy, Default)]
pub struct BcjArm64;

impl BcjArm64 {
    /// Creates a new instance of the ARM64 BCJ filter.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl BranchFilter for BcjArm64 {
    #[inline]
    fn encode(&self, data: &mut [u8], ip: u32) -> usize {
        arm64_encode(data, ip)
    }

    #[inline]
    fn decode(&self, data: &mut [u8], ip: u32) -> usize {
        arm64_decode(data, ip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arm64_bl_roundtrip() {
        // BL #0x1000 -> imm26 = 0x400 (0x94000400)
        let original_instr: u32 = 0x9400_0400;
        let mut buffer = original_instr.to_le_bytes().to_vec();

        let processed = arm64_encode(&mut buffer, 0x10000);
        assert_eq!(processed, 4);

        let encoded_instr = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
        assert_ne!(encoded_instr, original_instr);

        let decoded = arm64_decode(&mut buffer, 0x10000);
        assert_eq!(decoded, 4);

        let restored_instr = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
        assert_eq!(restored_instr, original_instr);
    }

    #[test]
    fn test_arm64_adrp_roundtrip() {
        // ADRP X0, #0x2000 -> imm = 2 -> immlo = 2 (0b10), immhi = 0, Rd = 0
        // instr = 0x90000000 | (2 << 29) = 0xD0000000
        let original_instr: u32 = 0xD000_0000;
        let mut buffer = original_instr.to_le_bytes().to_vec();

        let processed = arm64_encode(&mut buffer, 0x4000);
        assert_eq!(processed, 4);

        let encoded_instr = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
        assert_ne!(encoded_instr, original_instr);

        let decoded = arm64_decode(&mut buffer, 0x4000);
        assert_eq!(decoded, 4);

        let restored_instr = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
        assert_eq!(restored_instr, original_instr);
    }

    #[test]
    fn test_arm64_unaligned_tail_preservation() {
        let mut buffer = vec![0x94, 0x00, 0x04, 0x00, 0xAA, 0xBB, 0xCC];
        let original_tail = buffer[4..].to_vec();

        let processed = arm64_encode(&mut buffer, 0);
        assert_eq!(processed, 4);
        assert_eq!(&buffer[4..], &original_tail[..]);

        arm64_decode(&mut buffer, 0);
        assert_eq!(&buffer[4..], &original_tail[..]);
        assert_eq!(buffer[0..4], [0x94, 0x00, 0x04, 0x00]);
    }
}
