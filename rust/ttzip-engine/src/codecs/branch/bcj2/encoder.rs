// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! BCJ2 1-In-4-Out Multi-Stream Allocation Encoder.
//!
//! Scans executable bytecode and separates CALL (`0xE8`) and JMP (`0xE9`)
//! instructions into 4 independent streams (Main, Call, Jump, and RangeCoder Status).

use super::range_coder::{RangeEncoder, NUM_BCJ2_PROBS, PROB_INIT_VAL};

/// Output container holding the four distinct streams produced by BCJ2 encoding.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bcj2Streams {
    /// Stream 0: Main stream containing unmodified non-branch bytes and opcode markers.
    pub main: Vec<u8>,
    /// Stream 1: Call stream containing 32-bit big-endian absolute destinations for 0xE8 CALLs.
    pub call: Vec<u8>,
    /// Stream 2: Jump stream containing 32-bit big-endian absolute destinations for 0xE9 JMPs.
    pub jump: Vec<u8>,
    /// Stream 3: RangeCoder control bitstream indicating whether opcodes were converted.
    pub rc: Vec<u8>,
}

impl Bcj2Streams {
    /// Creates an empty container for BCJ2 output streams with allocated capacities.
    #[must_use]
    pub fn with_capacities(main_cap: usize, call_cap: usize, jump_cap: usize, rc_cap: usize) -> Self {
        Self {
            main: Vec::with_capacity(main_cap),
            call: Vec::with_capacity(call_cap),
            jump: Vec::with_capacity(jump_cap),
            rc: Vec::with_capacity(rc_cap),
        }
    }

    /// Total combined byte size across all 4 streams.
    #[inline]
    #[must_use]
    pub fn total_bytes(&self) -> usize {
        self.main.len() + self.call.len() + self.jump.len() + self.rc.len()
    }
}

/// 1-In-4-Out BCJ2 Multi-Stream Encoder.
#[derive(Debug, Clone, Copy, Default)]
pub struct Bcj2Encoder {
    ip: u32,
}

impl Bcj2Encoder {
    /// Creates a new BCJ2 encoder starting at instruction pointer 0.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self { ip: 0 }
    }

    /// Creates a new BCJ2 encoder with a specific base instruction pointer offset.
    #[inline]
    #[must_use]
    pub const fn with_ip(ip: u32) -> Self {
        Self { ip }
    }

    /// Encodes a raw byte slice into 4 BCJ2 output streams.
    pub fn encode(&self, src: &[u8]) -> Bcj2Streams {
        let len = src.len();
        let mut streams = Bcj2Streams::with_capacities(
            len,
            len / 8,
            len / 16,
            (len / 32).max(16),
        );

        let mut rc = RangeEncoder::new();
        let mut probs = [PROB_INIT_VAL; NUM_BCJ2_PROBS];

        let mut pos = 0;
        let mut prev_byte = 0u8;

        while pos < len {
            let b = src[pos];
            if (b == 0xE8 || b == 0xE9) && pos + 5 <= len {
                let rel = u32::from_le_bytes([
                    src[pos + 1],
                    src[pos + 2],
                    src[pos + 3],
                    src[pos + 4],
                ]);
                let dest = self
                    .ip
                    .wrapping_add(pos as u32)
                    .wrapping_add(5)
                    .wrapping_add(rel);

                let ctx = if b == 0xE8 {
                    prev_byte as usize
                } else {
                    256
                };

                // Bit 1 signals that this opcode was converted into an absolute branch
                rc.encode_bit(&mut probs[ctx], 1, &mut streams.rc);
                streams.main.push(b);

                let dest_be = dest.to_be_bytes();
                if b == 0xE8 {
                    streams.call.extend_from_slice(&dest_be);
                } else {
                    streams.jump.extend_from_slice(&dest_be);
                }

                prev_byte = src[pos + 4];
                pos += 5;
            } else {
                if b == 0xE8 || b == 0xE9 {
                    let ctx = if b == 0xE8 {
                        prev_byte as usize
                    } else {
                        256
                    };
                    rc.encode_bit(&mut probs[ctx], 0, &mut streams.rc);
                }
                streams.main.push(b);
                prev_byte = b;
                pos += 1;
            }
        }

        rc.finish(&mut streams.rc);
        streams
    }
}

/// Freestanding convenience function to encode a byte slice with BCJ2.
pub fn encode_bcj2(src: &[u8], ip: u32) -> Bcj2Streams {
    Bcj2Encoder::with_ip(ip).encode(src)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bcj2_encode_streams_structure() {
        let input = vec![
            0x90, // NOP
            0xE8, 0x10, 0x00, 0x00, 0x00, // CALL +16
            0x90, // NOP
            0xE9, 0x20, 0x00, 0x00, 0x00, // JMP +32
            0xCC, // INT3
        ];

        let streams = encode_bcj2(&input, 0x1000);
        assert_eq!(streams.main, vec![0x90, 0xE8, 0x90, 0xE9, 0xCC]);
        assert_eq!(streams.call.len(), 4);
        assert_eq!(streams.jump.len(), 4);
        assert!(!streams.rc.is_empty());

        // CALL at pos 1: dest = 0x1000 + 1 + 5 + 0x10 = 0x1016
        assert_eq!(streams.call, 0x0000_1016u32.to_be_bytes());

        // JMP at pos 7: dest = 0x1000 + 7 + 5 + 0x20 = 0x102C
        assert_eq!(streams.jump, 0x0000_102Cu32.to_be_bytes());
    }
}
