// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! ARM64 (AArch64) BCJ Hardware Instruction Filter (Filter ID `0x0A`).
//!
//! Converts 26-bit relative branch offsets in ARM64 `BL` (`0x94000000`, +/-128 MiB reach)
//! and 21-bit split page displacements in `ADRP` (`0x90000000`, +/-512 MiB range filtering
//! with $PC \gg 12$ 4KB page alignment) into absolute values.

use super::{BranchFilter, FILTER_ID_ARM64};

/// ARM64 (AArch64) branch conversion filter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BcjArm64;

impl BcjArm64 {
    /// Creates a new `BcjArm64` filter.
    pub fn new() -> Self {
        Self
    }

    /// Transforms ARM64 BL and ADRP instructions in-place.
    pub fn transform(&self, buffer: &mut [u8], now_pos: u32, is_encoder: bool) -> usize {
        let size = buffer.len() & !3;
        let mut i = 0;

        while i < size {
            let mut pc = now_pos.wrapping_add(i as u32);
            let mut instr = u32::from_le_bytes([
                buffer[i],
                buffer[i + 1],
                buffer[i + 2],
                buffer[i + 3],
            ]);

            if (instr >> 26) == 0x25 {
                // BL instruction (+/- 128 MiB full 26-bit conversion)
                let src = instr;
                instr = 0x9400_0000;

                pc >>= 2;
                if !is_encoder {
                    pc = 0u32.wrapping_sub(pc);
                }

                instr |= src.wrapping_add(pc) & 0x03FF_FFFF;
                buffer[i..i + 4].copy_from_slice(&instr.to_le_bytes());
            } else if (instr & 0x9F00_0000) == 0x9000_0000 {
                // ADRP instruction (+/- 512 MiB range filter)
                let src = ((instr >> 29) & 3) | ((instr >> 3) & 0x001F_FFFC);

                if (src.wrapping_add(0x0002_0000) & 0x001C_0000) != 0 {
                    i += 4;
                    continue;
                }

                instr &= 0x9000_001F;

                pc >>= 12;
                if !is_encoder {
                    pc = 0u32.wrapping_sub(pc);
                }

                let dest = src.wrapping_add(pc);
                instr |= (dest & 3) << 29;
                instr |= (dest & 0x0003_FFFC) << 3;
                instr |= (0u32.wrapping_sub(dest & 0x0002_0000)) & 0x00E0_0000;

                buffer[i..i + 4].copy_from_slice(&instr.to_le_bytes());
            }

            i += 4;
        }

        i
    }
}

impl BranchFilter for BcjArm64 {
    #[inline]
    fn filter_id(&self) -> u64 {
        FILTER_ID_ARM64
    }

    #[inline]
    fn alignment(&self) -> usize {
        4
    }

    #[inline]
    fn unfiltered_max(&self) -> usize {
        4
    }

    #[inline]
    fn encode(&mut self, buf: &mut [u8], now_pos: u32) -> usize {
        self.transform(buf, now_pos, true)
    }

    #[inline]
    fn decode(&mut self, buf: &mut [u8], now_pos: u32) -> usize {
        self.transform(buf, now_pos, false)
    }

    #[inline]
    fn reset(&mut self) {
        // Stateless filter
    }
}
