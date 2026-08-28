// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! BCJ2 4-In-1-Out Multi-Stream Recombination Decoder.
//!
//! Merges Main, Call, Jump, and RangeCoder Status streams back into the
//! original byte-exact executable x86 machine code.

use crate::types::TTZipStatus;
use super::range_coder::{RangeDecoder, NUM_BCJ2_PROBS, PROB_INIT_VAL};

/// 4-In-1-Out BCJ2 Recombination Decoder.
#[derive(Debug, Clone, Copy, Default)]
pub struct Bcj2Decoder {
    ip: u32,
}

impl Bcj2Decoder {
    /// Creates a new BCJ2 decoder starting at base instruction pointer 0.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self { ip: 0 }
    }

    /// Creates a new BCJ2 decoder with a specific instruction pointer offset.
    #[inline]
    #[must_use]
    pub const fn with_ip(ip: u32) -> Self {
        Self { ip }
    }

    /// Merges 4 BCJ2 input streams into the original decompressed byte vector.
    pub fn decode(
        &self,
        main: &[u8],
        call: &[u8],
        jump: &[u8],
        rc: &[u8],
    ) -> Result<Vec<u8>, TTZipStatus> {
        let mut out = Vec::with_capacity(main.len() + call.len() + jump.len());
        let mut rc_dec = RangeDecoder::new(rc);
        let mut probs = [PROB_INIT_VAL; NUM_BCJ2_PROBS];

        let mut call_pos = 0;
        let mut jump_pos = 0;
        let mut prev_byte = 0u8;

        for &b in main {
            if b == 0xE8 || b == 0xE9 {
                let ctx = if b == 0xE8 {
                    prev_byte as usize
                } else {
                    256
                };
                let bit = rc_dec.decode_bit(&mut probs[ctx]);
                if bit == 1 {
                    let dest = if b == 0xE8 {
                        if call_pos + 4 > call.len() {
                            return Err(TTZipStatus::ErrCorruptHeader);
                        }
                        let d = u32::from_be_bytes([
                            call[call_pos],
                            call[call_pos + 1],
                            call[call_pos + 2],
                            call[call_pos + 3],
                        ]);
                        call_pos += 4;
                        d
                    } else {
                        if jump_pos + 4 > jump.len() {
                            return Err(TTZipStatus::ErrCorruptHeader);
                        }
                        let d = u32::from_be_bytes([
                            jump[jump_pos],
                            jump[jump_pos + 1],
                            jump[jump_pos + 2],
                            jump[jump_pos + 3],
                        ]);
                        jump_pos += 4;
                        d
                    };

                    let current_ip = self.ip.wrapping_add(out.len() as u32).wrapping_add(5);
                    let rel = dest.wrapping_sub(current_ip);
                    let rel_le = rel.to_le_bytes();

                    out.push(b);
                    out.extend_from_slice(&rel_le);
                    prev_byte = rel_le[3];
                    continue;
                }
            }
            out.push(b);
            prev_byte = b;
        }

        Ok(out)
    }
}

/// Freestanding convenience function to decode 4 BCJ2 streams.
pub fn decode_bcj2(
    main: &[u8],
    call: &[u8],
    jump: &[u8],
    rc: &[u8],
    ip: u32,
) -> Result<Vec<u8>, TTZipStatus> {
    Bcj2Decoder::with_ip(ip).decode(main, call, jump, rc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codecs::branch::bcj2::encoder::encode_bcj2;

    #[test]
    fn test_bcj2_roundtrip_bit_exact() {
        let original = vec![
            0x55, 0x48, 0x89, 0xE5, // PUSH RBP; MOV RBP, RSP
            0xE8, 0x20, 0x00, 0x00, 0x00, // CALL +32
            0x90, 0x90,
            0xE9, 0x40, 0x00, 0x00, 0x00, // JMP +64
            0x5D, 0xC3, // POP RBP; RET
        ];

        let streams = encode_bcj2(&original, 0x4000);
        let restored = decode_bcj2(
            &streams.main,
            &streams.call,
            &streams.jump,
            &streams.rc,
            0x4000,
        )
        .expect("BCJ2 decode failed");

        assert_eq!(restored, original);
    }

    #[test]
    fn test_bcj2_corrupt_call_stream_detected() {
        let original = vec![0xE8, 0x10, 0x00, 0x00, 0x00];
        let mut streams = encode_bcj2(&original, 0);
        // Truncate call stream
        streams.call.clear();

        let res = decode_bcj2(
            &streams.main,
            &streams.call,
            &streams.jump,
            &streams.rc,
            0,
        );
        assert_eq!(res, Err(TTZipStatus::ErrCorruptHeader));
    }
}
