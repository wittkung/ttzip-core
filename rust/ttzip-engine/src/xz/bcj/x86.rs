// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

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
            let offset = curr_pos.wrapping_sub(prev_pos);
            prev_pos = curr_pos;

            if offset > 5 {
                prev_mask = 0;
            } else {
                for _ in 0..offset {
                    prev_mask &= 0x77;
                    prev_mask <<= 1;
                }
            }

            let b = buffer[buffer_pos + 4];

            if test_86_ms_byte(b) && (prev_mask >> 1) <= 4 && (prev_mask >> 1) != 3 {
                let mut src = ((b as u32) << 24)
                    | ((buffer[buffer_pos + 3] as u32) << 16)
                    | ((buffer[buffer_pos + 2] as u32) << 8)
                    | (buffer[buffer_pos + 1] as u32);

                let mut dest;
                loop {
                    let adj = now_pos.wrapping_add(buffer_pos as u32).wrapping_add(5);
                    if is_encoder {
                        dest = src.wrapping_add(adj);
                    } else {
                        dest = src.wrapping_sub(adj);
                    }

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
                buffer[buffer_pos + 4] = msb;
                buffer[buffer_pos + 3] = (dest >> 16) as u8;
                buffer[buffer_pos + 2] = (dest >> 8) as u8;
                buffer[buffer_pos + 1] = dest as u8;
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
