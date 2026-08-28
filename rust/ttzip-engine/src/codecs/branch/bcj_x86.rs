// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! x86 / x86-64 Branch Conversion Filter (BCJ x86).
//!
//! Converts relative branch targets in x86 CALL (`0xE8`) and JMP (`0xE9`)
//! instructions to absolute addresses using a 5-byte sliding window state machine
//! to prevent false-positive aliasing and ensure 100% bi-directional roundtrip fidelity.

use super::BranchFilter;

/// Checks whether the most significant byte of a 32-bit displacement matches 0x00 or 0xFF.
#[inline(always)]
fn test_86_ms_byte(b: u8) -> bool {
    b == 0x00 || b == 0xFF
}

/// Normalizes x86 CALL (0xE8) and JMP (0xE9) relative branch targets to absolute addresses in-place.
///
/// Returns the number of bytes processed.
pub fn x86_encode(data: &mut [u8], ip: u32) -> usize {
    x86_transform(data, ip, true)
}

/// Restores x86 CALL (0xE8) and JMP (0xE9) relative branch targets from absolute addresses in-place.
///
/// Returns the number of bytes processed.
pub fn x86_decode(data: &mut [u8], ip: u32) -> usize {
    x86_transform(data, ip, false)
}

/// Core transformation loop with 5-byte sliding window state machine.
fn x86_transform(buffer: &mut [u8], now_pos: u32, is_encoder: bool) -> usize {
    static MASK_TO_BIT_NUMBER: [u32; 5] = [0, 1, 2, 2, 3];

    let size = buffer.len();
    if size < 5 {
        return 0;
    }

    let limit = size - 5;
    let mut buffer_pos = 0;
    let mut prev_mask = 0u32;
    let mut prev_pos = now_pos.wrapping_sub(5);

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

            buffer[buffer_pos + 4] = (!(((dest >> 24) & 1).wrapping_sub(1))) as u8;
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

    buffer_pos
}

/// Zero-cost stateless x86 Branch Filter implementing `BranchFilter`.
#[derive(Debug, Clone, Copy, Default)]
pub struct BcjX86;

impl BcjX86 {
    /// Creates a new instance of the x86 BCJ filter.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl BranchFilter for BcjX86 {
    #[inline]
    fn encode(&self, data: &mut [u8], ip: u32) -> usize {
        x86_encode(data, ip)
    }

    #[inline]
    fn decode(&self, data: &mut [u8], ip: u32) -> usize {
        x86_decode(data, ip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_x86_call_and_jmp_roundtrip() {
        // 0xE8 0x00 0x01 0x00 0x00 (CALL +256)
        // 0x90 (NOP)
        // 0xE9 0xF0 0xFE 0xFF 0xFF (JMP -272)
        let original = vec![
            0xE8, 0x00, 0x01, 0x00, 0x00,
            0x90,
            0xE9, 0xF0, 0xFE, 0xFF, 0xFF,
        ];
        let mut buffer = original.clone();

        let proc_enc = x86_encode(&mut buffer, 0x1000);
        assert!(proc_enc >= 11);
        assert_ne!(buffer, original);

        let proc_dec = x86_decode(&mut buffer, 0x1000);
        assert_eq!(proc_dec, proc_enc);
        assert_eq!(buffer, original);
    }

    #[test]
    fn test_x86_sliding_window_false_positive_prevention() {
        // Contains 0xE8 in displacement where MSB is not 0x00 or 0xFF
        let original = vec![0xE8, 0x12, 0x34, 0x56, 0x78, 0xE9, 0x00, 0x00, 0x00, 0x00];
        let mut buffer = original.clone();

        x86_encode(&mut buffer, 0);
        // The first 0xE8 had MSB 0x78 (not 0x00/0xFF), so its displacement is not modified
        assert_eq!(buffer[1..5], original[1..5]);

        x86_decode(&mut buffer, 0);
        assert_eq!(buffer, original);
    }
}
