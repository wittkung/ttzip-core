// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 11-bit 258-Context Adaptive Binary Range Coder for BCJ2.
//!
//! Provides the bitstream entropy encoder and decoder governing the status
//! stream in the 7z BCJ2 4-Stream executable filter.

/// Total precision bits for bit model probabilities.
pub const NUM_BIT_MODEL_TOTAL_BITS: usize = 11;
/// Total scale of the probability model (2^11 = 2048).
pub const BIT_MODEL_TOTAL: u32 = 1 << NUM_BIT_MODEL_TOTAL_BITS;
/// Adaptation rate shift for updating probability models.
pub const NUM_MOVE_BITS: usize = 5;
/// Initial probability value representing 50% likelihood (1024).
pub const PROB_INIT_VAL: u16 = (BIT_MODEL_TOTAL / 2) as u16;
/// Total number of probability contexts for BCJ2 (256 for 0xE8 previous bytes + 1 for 0xE9 + 1 spare).
pub const NUM_BCJ2_PROBS: usize = 258;

/// Adaptive Binary Range Encoder.
#[derive(Debug, Clone)]
pub struct RangeEncoder {
    low: u64,
    range: u32,
    cache_size: u64,
    cache: u8,
}

impl Default for RangeEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl RangeEncoder {
    /// Creates a new RangeEncoder initialized to full range.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            low: 0,
            range: 0xFFFF_FFFF,
            cache_size: 1,
            cache: 0,
        }
    }

    /// Encodes a single bit (0 or 1) using the given adaptive probability context.
    #[inline]
    pub fn encode_bit(&mut self, prob: &mut u16, bit: u32, out: &mut Vec<u8>) {
        let bound = (self.range >> NUM_BIT_MODEL_TOTAL_BITS) * (*prob as u32);
        if bit == 0 {
            self.range = bound;
            *prob += ((BIT_MODEL_TOTAL as u16) - *prob) >> NUM_MOVE_BITS;
        } else {
            self.low += bound as u64;
            self.range -= bound;
            *prob -= *prob >> NUM_MOVE_BITS;
        }
        while self.range < (1 << 24) {
            self.range <<= 8;
            self.shift_low(out);
        }
    }

    /// Normalizes and emits cached bytes to output sink.
    #[inline]
    fn shift_low(&mut self, out: &mut Vec<u8>) {
        let low_hi = (self.low >> 32) as u32;
        if low_hi != 0 || self.low < 0xFF00_0000 {
            let mut temp = self.cache;
            loop {
                out.push(temp.wrapping_add(low_hi as u8));
                temp = 0xFF;
                self.cache_size -= 1;
                if self.cache_size == 0 {
                    break;
                }
            }
            self.cache = (self.low >> 24) as u8;
        }
        self.cache_size += 1;
        self.low = ((self.low as u32) << 8) as u64;
    }

    /// Flushes all remaining low-range bits to finalize the bitstream.
    pub fn finish(&mut self, out: &mut Vec<u8>) {
        for _ in 0..5 {
            self.shift_low(out);
        }
    }
}

/// Adaptive Binary Range Decoder for zero-copy streaming bit decompression.
#[derive(Debug, Clone)]
pub struct RangeDecoder<'a> {
    range: u32,
    code: u32,
    src: &'a [u8],
    pos: usize,
}

impl<'a> RangeDecoder<'a> {
    /// Creates a new RangeDecoder reading from the slice and pre-buffering 5 initial bytes.
    #[must_use]
    pub fn new(src: &'a [u8]) -> Self {
        let mut rd = Self {
            range: 0xFFFF_FFFF,
            code: 0,
            src,
            pos: 0,
        };
        for _ in 0..5 {
            let b = rd.read_byte();
            rd.code = (rd.code << 8) | (b as u32);
        }
        rd
    }

    #[inline(always)]
    fn read_byte(&mut self) -> u8 {
        if self.pos < self.src.len() {
            let b = self.src[self.pos];
            self.pos += 1;
            b
        } else {
            0
        }
    }

    /// Decodes a single bit (0 or 1) using the given adaptive probability context.
    #[inline]
    pub fn decode_bit(&mut self, prob: &mut u16) -> u32 {
        let bound = (self.range >> NUM_BIT_MODEL_TOTAL_BITS) * (*prob as u32);
        let bit = if self.code < bound {
            self.range = bound;
            *prob += ((BIT_MODEL_TOTAL as u16) - *prob) >> NUM_MOVE_BITS;
            0
        } else {
            self.range -= bound;
            self.code -= bound;
            *prob -= *prob >> NUM_MOVE_BITS;
            1
        };
        while self.range < (1 << 24) {
            self.range <<= 8;
            let b = self.read_byte();
            self.code = (self.code << 8) | (b as u32);
        }
        bit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_range_coder_bit_roundtrip() {
        let test_bits = vec![
            (0usize, 0u32),
            (0, 1),
            (1, 1),
            (1, 0),
            (256, 1),
            (256, 1),
            (256, 0),
            (128, 0),
            (128, 1),
            (128, 1),
            (128, 0),
        ];

        let mut encoder = RangeEncoder::new();
        let mut enc_probs = [PROB_INIT_VAL; NUM_BCJ2_PROBS];
        let mut stream = Vec::new();

        for &(ctx, bit) in &test_bits {
            encoder.encode_bit(&mut enc_probs[ctx], bit, &mut stream);
        }
        encoder.finish(&mut stream);

        let mut decoder = RangeDecoder::new(&stream);
        let mut dec_probs = [PROB_INIT_VAL; NUM_BCJ2_PROBS];

        for &(ctx, expected_bit) in &test_bits {
            let decoded_bit = decoder.decode_bit(&mut dec_probs[ctx]);
            assert_eq!(decoded_bit, expected_bit, "Bit mismatch at ctx {}", ctx);
        }
    }
}
