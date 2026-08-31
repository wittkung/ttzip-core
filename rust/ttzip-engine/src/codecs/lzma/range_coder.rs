// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Rust Branchless Binary Range Coder Microkernel for LZMA and LZMA2.
//!
//! Implements the 11-bit probability model range coder microkernel adhering strictly to
//! the 7-Zip LZMA reference specification. Features branchless direct bit decoding,
//! zero-allocation bit-tree and reverse bit-tree traversals, and match-byte guided literal decoding.

use crate::types::TTZipStatus;

/// Total precision bits for bit model probabilities in LZMA.
pub const NUM_BIT_MODEL_TOTAL_BITS: usize = 11;

/// Total scale of the probability model (2^11 = 2048).
pub const BIT_MODEL_TOTAL: u32 = 1 << NUM_BIT_MODEL_TOTAL_BITS;

/// Adaptation rate shift for updating probability models.
pub const NUM_MOVE_BITS: usize = 5;

/// Normalization boundary threshold for range coder registers (2^24 = 0x0100_0000).
pub const TOP_VALUE: u32 = 1 << 24;

/// Initial probability value representing 50% likelihood (1024).
pub const PROB_INIT_VAL: u16 = (BIT_MODEL_TOTAL / 2) as u16;

/// Range coder decoding and encoding error types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeCoderError {
    /// Bitstream ended unexpectedly during initialization or normalization.
    UnexpectedEof,
    /// Bitstream corruption detected (e.g. invalid first byte or range underflow).
    CorruptBitstream(&'static str),
    /// Symbol exceeds maximum capacity of the bit tree.
    InvalidBitTreeSymbol,
}

impl std::fmt::Display for RangeCoderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedEof => write!(f, "Unexpected EOF in LZMA range coder bitstream"),
            Self::CorruptBitstream(msg) => write!(f, "Corrupted LZMA range coder bitstream: {msg}"),
            Self::InvalidBitTreeSymbol => write!(f, "Invalid bit-tree symbol in range coder"),
        }
    }
}

impl std::error::Error for RangeCoderError {}

impl From<RangeCoderError> for TTZipStatus {
    fn from(err: RangeCoderError) -> Self {
        match err {
            RangeCoderError::UnexpectedEof => TTZipStatus::ErrCorruptHeader,
            RangeCoderError::CorruptBitstream(_) | RangeCoderError::InvalidBitTreeSymbol => {
                TTZipStatus::ErrExtractionFailed
            }
        }
    }
}

/// Adaptive Binary Range Decoder for zero-allocation streaming bit decompression.
#[derive(Debug, Clone)]
pub struct RangeDecoder<'a> {
    range: u32,
    code: u32,
    src: &'a [u8],
    pos: usize,
}

impl<'a> RangeDecoder<'a> {
    /// Creates a new `RangeDecoder` by reading from the slice and pre-buffering 5 initial bytes.
    ///
    /// # Errors
    /// Returns `RangeCoderError::UnexpectedEof` if `src` contains fewer than 5 bytes.
    #[inline]
    pub fn new(src: &'a [u8]) -> Result<Self, RangeCoderError> {
        if src.len() < 5 {
            return Err(RangeCoderError::UnexpectedEof);
        }
        let mut rd = Self {
            range: 0xFFFF_FFFF,
            code: 0,
            src,
            pos: 0,
        };
        for _ in 0..5 {
            let b = rd.read_byte()?;
            rd.code = (rd.code << 8) | (b as u32);
        }
        Ok(rd)
    }

    /// Creates a new `RangeDecoder` without prebuffering 5 bytes (for relaxed/manual initialization).
    #[inline]
    #[must_use]
    pub const fn new_raw(src: &'a [u8]) -> Self {
        Self {
            range: 0xFFFF_FFFF,
            code: 0,
            src,
            pos: 0,
        }
    }

    /// Returns the current value of the `range` register.
    #[inline(always)]
    pub const fn range(&self) -> u32 {
        self.range
    }

    /// Returns the current value of the `code` register.
    #[inline(always)]
    pub const fn code(&self) -> u32 {
        self.code
    }

    /// Returns the total number of bytes consumed from the source slice.
    #[inline(always)]
    pub const fn pos(&self) -> usize {
        self.pos
    }

    /// Returns `true` if the source slice has been completely read.
    #[inline(always)]
    pub const fn is_eof(&self) -> bool {
        self.pos >= self.src.len()
    }

    /// Reads a single raw byte from the source buffer.
    #[inline(always)]
    fn read_byte(&mut self) -> Result<u8, RangeCoderError> {
        if self.pos < self.src.len() {
            let b = self.src[self.pos];
            self.pos += 1;
            Ok(b)
        } else {
            Err(RangeCoderError::UnexpectedEof)
        }
    }

    /// Decodes a single bit (0 or 1) using the given adaptive probability context.
    ///
    /// Probability model updates:
    /// - Bit 0: `prob += (BIT_MODEL_TOTAL - prob) >> NUM_MOVE_BITS`
    /// - Bit 1: `prob -= prob >> NUM_MOVE_BITS`
    #[inline(always)]
    pub fn decode_bit(&mut self, prob: &mut u16) -> Result<u32, RangeCoderError> {
        let p = (*prob as u32).min(BIT_MODEL_TOTAL);
        let bound = (self.range >> NUM_BIT_MODEL_TOTAL_BITS) * p;
        let bit = if self.code < bound {
            self.range = bound;
            let diff = (BIT_MODEL_TOTAL as u16).saturating_sub(*prob);
            *prob = prob.saturating_add(diff >> NUM_MOVE_BITS);
            0
        } else {
            self.range = self.range.saturating_sub(bound);
            self.code = self.code.saturating_sub(bound);
            *prob = prob.saturating_sub(*prob >> NUM_MOVE_BITS);
            1
        };

        if self.range < TOP_VALUE {
            self.range <<= 8;
            let b = self.read_byte()?;
            self.code = (self.code << 8) | (b as u32);
        }

        Ok(bit)
    }

    /// Decodes multiple unmodeled direct bits using 7-Zip branchless arithmetic.
    ///
    /// Branchless logic:
    /// ```text
    /// t = 0 - (code >> 31);
    /// res = (res << 1) + (t + 1);
    /// code += range & t;
    /// ```
    #[inline]
    pub fn decode_direct_bits(&mut self, num_bits: u32) -> Result<u32, RangeCoderError> {
        let mut res = 0u32;
        for _ in 0..num_bits {
            self.range >>= 1;
            self.code = self.code.wrapping_sub(self.range);
            let t = 0u32.wrapping_sub(self.code >> 31);
            res = (res << 1).wrapping_add(t.wrapping_add(1));
            self.code = self.code.wrapping_add(self.range & t);

            if self.range < TOP_VALUE {
                self.range <<= 8;
                let b = self.read_byte()?;
                self.code = (self.code << 8) | (b as u32);
            }
        }
        Ok(res)
    }

    /// Decodes a binary tree symbol with `num_bits` levels (MSB first).
    #[inline]
    pub fn decode_bit_tree(
        &mut self,
        probs: &mut [u16],
        num_bits: u32,
    ) -> Result<u32, RangeCoderError> {
        let mut symbol = 1usize;
        for _ in 0..num_bits {
            if symbol >= probs.len() {
                return Err(RangeCoderError::InvalidBitTreeSymbol);
            }
            let bit = self.decode_bit(&mut probs[symbol])?;
            symbol = (symbol << 1) | (bit as usize);
        }
        Ok((symbol as u32) - (1 << num_bits))
    }

    /// Decodes a reverse binary tree symbol with `num_bits` levels (LSB first).
    #[inline]
    pub fn decode_reverse_bit_tree(
        &mut self,
        probs: &mut [u16],
        num_bits: u32,
    ) -> Result<u32, RangeCoderError> {
        let mut symbol = 1usize;
        let mut res = 0u32;
        for i in 0..num_bits {
            if symbol >= probs.len() {
                return Err(RangeCoderError::InvalidBitTreeSymbol);
            }
            let bit = self.decode_bit(&mut probs[symbol])?;
            symbol = (symbol << 1) | (bit as usize);
            res |= bit << i;
        }
        Ok(res)
    }

    /// Decodes a standard literal byte (8 bits MSB first) from a 0x300 sub-array.
    #[inline]
    pub fn decode_literal_byte(&mut self, probs: &mut [u16]) -> Result<u8, RangeCoderError> {
        let mut symbol = 1usize;
        while symbol < 0x100 {
            let bit = self.decode_bit(&mut probs[symbol])?;
            symbol = (symbol << 1) | (bit as usize);
        }
        Ok((symbol & 0xFF) as u8)
    }

    /// Decodes a literal byte with match-byte context guidance for states >= 7.
    #[inline]
    pub fn decode_matched_byte(
        &mut self,
        probs: &mut [u16],
        mut match_byte: u8,
    ) -> Result<u8, RangeCoderError> {
        let mut symbol = 1usize;
        while symbol < 0x100 {
            let match_bit = ((match_byte >> 7) & 1) as usize;
            match_byte <<= 1;
            let prob_idx = 0x100 + (match_bit << 8) + symbol;
            let bit = self.decode_bit(&mut probs[prob_idx])?;
            symbol = (symbol << 1) | (bit as usize);

            if match_bit != (bit as usize) {
                while symbol < 0x100 {
                    let bit = self.decode_bit(&mut probs[symbol])?;
                    symbol = (symbol << 1) | (bit as usize);
                }
                break;
            }
        }
        Ok((symbol & 0xFF) as u8)
    }
}

/// Adaptive Binary Range Encoder for bitstream serialization.
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
    /// Creates a new `RangeEncoder` initialized to full range.
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
        while self.range < TOP_VALUE {
            self.range <<= 8;
            self.shift_low(out);
        }
    }

    /// Encodes multiple unmodeled direct bits.
    #[inline]
    pub fn encode_direct_bits(&mut self, value: u32, num_bits: u32, out: &mut Vec<u8>) {
        for i in (0..num_bits).rev() {
            self.range >>= 1;
            if ((value >> i) & 1) == 1 {
                self.low += self.range as u64;
            }
            if self.range < TOP_VALUE {
                self.range <<= 8;
                self.shift_low(out);
            }
        }
    }

    /// Encodes a binary tree symbol with `num_bits` levels (MSB first).
    #[inline]
    pub fn encode_bit_tree(
        &mut self,
        probs: &mut [u16],
        symbol: u32,
        num_bits: u32,
        out: &mut Vec<u8>,
    ) {
        let mut m = 1usize;
        for i in (0..num_bits).rev() {
            let bit = (symbol >> i) & 1;
            self.encode_bit(&mut probs[m], bit, out);
            m = (m << 1) | (bit as usize);
        }
    }

    /// Encodes a reverse binary tree symbol with `num_bits` levels (LSB first).
    #[inline]
    pub fn encode_reverse_bit_tree(
        &mut self,
        probs: &mut [u16],
        symbol: u32,
        num_bits: u32,
        out: &mut Vec<u8>,
    ) {
        let mut m = 1usize;
        for i in 0..num_bits {
            let bit = (symbol >> i) & 1;
            self.encode_bit(&mut probs[m], bit, out);
            m = (m << 1) | (bit as usize);
        }
    }

    /// Encodes a standard literal byte (8 bits MSB first).
    #[inline]
    pub fn encode_literal_byte(&mut self, probs: &mut [u16], byte: u8, out: &mut Vec<u8>) {
        let mut symbol = 1usize;
        for i in (0..8).rev() {
            let bit = ((byte >> i) & 1) as u32;
            self.encode_bit(&mut probs[symbol], bit, out);
            symbol = (symbol << 1) | (bit as usize);
        }
    }

    /// Encodes a literal byte with match-byte context guidance for states >= 7.
    #[inline]
    pub fn encode_matched_byte(
        &mut self,
        probs: &mut [u16],
        byte: u8,
        mut match_byte: u8,
        out: &mut Vec<u8>,
    ) {
        let mut symbol = 1usize;
        let mut same = true;
        for i in (0..8).rev() {
            let bit = ((byte >> i) & 1) as u32;
            if same {
                let match_bit = ((match_byte >> 7) & 1) as usize;
                match_byte <<= 1;
                let prob_idx = 0x100 + (match_bit << 8) + symbol;
                self.encode_bit(&mut probs[prob_idx], bit, out);
                symbol = (symbol << 1) | (bit as usize);
                if match_bit != (bit as usize) {
                    same = false;
                }
            } else {
                self.encode_bit(&mut probs[symbol], bit, out);
                symbol = (symbol << 1) | (bit as usize);
            }
        }
    }

    /// Flushes all remaining buffered bytes and finalizes the bitstream.
    pub fn finish(&mut self, out: &mut Vec<u8>) {
        for _ in 0..5 {
            self.shift_low(out);
        }
    }
}
