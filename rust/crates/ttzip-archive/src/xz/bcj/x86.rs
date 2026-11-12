// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! x86 (IA-32 and x86-64) BCJ Hardware Instruction Filter (Filter ID `0x04`).
//!
//! Converts 32-bit relative displacement fields in x86 `CALL` (`0xE8`) and `JMP` (`0xE9`)
//! instructions to absolute addresses and back. Maintains a 5-byte bitmask state machine
//! (`prev_mask`, `Test86MSByte`) across arbitrary chunk boundaries.

use super::{BranchFilter, FILTER_ID_X86};

/// Bit positions lookup for previous displacement MSBs matching bitmasks.
const MASK_TO_BIT_NUMBER: [u32; 5] = [0, 1, 2, 2, 3];

/// Tests whether the most significant byte of an x86 displacement indicates a small negative or positive jump.
#[inline(always)]
pub fn test_86_ms_byte(b: u8) -> bool {
    b == 0x00 || b == 0xFF
}

/// x86 / x86-64 BCJ stateful branch conversion filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BcjX86 {
    /// Mask of recently observed 0xE8/0xE9 candidates and MSB properties.
    pub prev_mask: u32,
    /// Absolute position of the previous candidate relative to `now_pos`.
    pub prev_pos: u32,
}

impl Default for BcjX86 {
    fn default() -> Self {
        Self::new()
    }
}

impl BcjX86 {
    /// Creates a new `BcjX86` filter with initial reset state.
    pub fn new() -> Self {
        Self {
            prev_mask: 0,
            prev_pos: 0u32.wrapping_sub(5),
        }
    }

    /// Transforms relative or absolute addresses in `buffer` starting at global stream offset `now_pos`.
    pub fn transform(&mut self, buffer: &mut [u8], now_pos: u32, is_encoder: bool) -> usize {
        let size = buffer.len();
        if size < 5 {
            return 0;
        }

        if now_pos.wrapping_sub(self.prev_pos) > 5 {
            self.prev_pos = now_pos.wrapping_sub(5);
        }

        let limit = size - 5;
        let mut buffer_pos = 0;
        let mut prev_mask = self.prev_mask;
        let mut prev_pos = self.prev_pos;

        while buffer_pos <= limit {
            let b0 = buffer[buffer_pos];
            if b0 != 0xE8 && b0 != 0xE9 {
                buffer_pos += 1;
                continue;
            }

            let curr_pos = now_pos.wrapping_add(buffer_pos as u32);
            prev_mask = Self::advance_prev_mask(prev_mask, curr_pos.wrapping_sub(prev_pos));
            prev_pos = curr_pos;

            let b = buffer[buffer_pos + 4];
            if Self::can_transform_candidate(b, prev_mask) {
                let adj = curr_pos.wrapping_add(5);
                Self::rewrite_displacement(&mut buffer[buffer_pos..buffer_pos + 5], adj, prev_mask, is_encoder);
                buffer_pos += 5;
                prev_mask = 0;
            } else {
                buffer_pos += 1;
                prev_mask |= 1;
                if test_86_ms_byte(b) {
                    prev_mask |= 0x10;
                }
            }
        }

        self.prev_mask = prev_mask;
        self.prev_pos = prev_pos;
        buffer_pos
    }

    #[inline(always)]
    fn advance_prev_mask(mut prev_mask: u32, offset: u32) -> u32 {
        if offset > 5 {
            return 0;
        }
        for _ in 0..offset {
            prev_mask = (prev_mask & 0x77) << 1;
        }
        prev_mask
    }

    #[inline(always)]
    fn can_transform_candidate(msb: u8, prev_mask: u32) -> bool {
        let shifted = prev_mask >> 1;
        test_86_ms_byte(msb) && shifted <= 4 && shifted != 3
    }

    #[inline(always)]
    fn rewrite_displacement(slot: &mut [u8], adj: u32, prev_mask: u32, is_encoder: bool) {
        let mut src = u32::from_le_bytes([slot[1], slot[2], slot[3], slot[4]]);
        let mut dest;
        loop {
            dest = if is_encoder {
                src.wrapping_add(adj)
            } else {
                src.wrapping_sub(adj)
            };

            if prev_mask == 0 {
                break;
            }

            let i = MASK_TO_BIT_NUMBER[(prev_mask >> 1) as usize];
            let b_check = (dest >> (24 - i * 8)) as u8;
            if !test_86_ms_byte(b_check) {
                break;
            }

            src = dest ^ ((1u32 << (32 - i * 8)).wrapping_sub(1));
        }

        let msb = (!(((dest >> 24) & 1).wrapping_sub(1))) as u8;
        slot[4] = msb;
        slot[3] = (dest >> 16) as u8;
        slot[2] = (dest >> 8) as u8;
        slot[1] = dest as u8;
    }
}

impl BranchFilter for BcjX86 {
    #[inline]
    fn filter_id(&self) -> u64 {
        FILTER_ID_X86
    }

    #[inline]
    fn alignment(&self) -> usize {
        1
    }

    #[inline]
    fn unfiltered_max(&self) -> usize {
        5
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
        self.prev_mask = 0;
        self.prev_pos = 0u32.wrapping_sub(5);
    }
}
