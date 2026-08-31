// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! ARM (32-bit Little-Endian) BCJ Hardware Instruction Filter (Filter ID `0x07`).
//!
//! Converts 24-bit relative displacement in ARM `BL` instructions (`0xEB` condition opcode)
//! to absolute addresses with +8-byte instruction pipelining compensation ($PC + 8$).

use super::{BranchFilter, FILTER_ID_ARM};

/// ARM 32-bit little-endian branch conversion filter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BcjArm;

impl BcjArm {
    /// Creates a new `BcjArm` filter.
    pub fn new() -> Self {
        Self
    }

    /// Transforms ARM 32-bit BL instructions in-place.
    pub fn transform(&self, buffer: &mut [u8], now_pos: u32, is_encoder: bool) -> usize {
        let size = buffer.len() & !3;
        let mut i = 0;

        while i < size {
            if buffer[i + 3] == 0xEB {
                let mut src = ((buffer[i + 2] as u32) << 16)
                    | ((buffer[i + 1] as u32) << 8)
                    | (buffer[i] as u32);
                src <<= 2;

                let dest = if is_encoder {
                    now_pos.wrapping_add(i as u32).wrapping_add(8).wrapping_add(src)
                } else {
                    src.wrapping_sub(now_pos.wrapping_add(i as u32).wrapping_add(8))
                };

                let dest = dest >> 2;
                buffer[i] = dest as u8;
                buffer[i + 1] = (dest >> 8) as u8;
                buffer[i + 2] = (dest >> 16) as u8;
            }
            i += 4;
        }

        i
    }
}

impl BranchFilter for BcjArm {
    #[inline]
    fn filter_id(&self) -> u64 {
        FILTER_ID_ARM
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
