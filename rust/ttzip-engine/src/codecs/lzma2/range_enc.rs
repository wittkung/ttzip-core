// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 64-bit Single-Branch Binary Range Encoder Microkernel for LZMA and LZMA2.
//!
//! Features:
//! - 64-bit `low` register eliminating intermediate carry overflow checks in arithmetic updates.
//! - Constant-time single-branch carry resolution with cascading `0xFF` rollover handling.
//! - High-throughput Direct Bits encoding with branchless register shifts.
//! - 2048-entry precomputed 4-bit fractional bit price lookup table (`PROB_PRICES`) for optimal parsing.
//! - Zero-allocation binary tree, reverse binary tree, and matched literal byte encoding primitives.

use crate::codecs::lzma::{
    BIT_MODEL_TOTAL, NUM_BIT_MODEL_TOTAL_BITS, NUM_MOVE_BITS, TOP_VALUE,
};

/// Number of fractional precision bits for probability price modeling (4 bits = 1/16th bit).
pub const NUM_BIT_PRICE_SHIFT_BITS: usize = 4;

/// Price unit corresponding to exactly 1.0 bit (1 << 4 = 16).
pub const BIT_PRICE_UNIT: u32 = 1 << NUM_BIT_PRICE_SHIFT_BITS;

/// Total size of the probability price lookup table (2048 entries).
pub const PROB_TABLE_SIZE: usize = 2048;

/// Precomputed 2048-entry lookup table mapping 11-bit probabilities (0..2047) to 4-bit fractional bit prices.
pub static PROB_PRICES: [u16; PROB_TABLE_SIZE] = generate_prob_prices_table();

/// Compile-time constant function to compute the 2048-entry bit price table adhering to the 7-Zip reference model.
const fn generate_prob_prices_table() -> [u16; PROB_TABLE_SIZE] {
    let mut table = [0u16; PROB_TABLE_SIZE];
    let mut i = 0;
    while i < PROB_TABLE_SIZE {
        table[i] = compute_prob_price(i);
        i += 1;
    }
    table
}

/// Computes fractional bit price for a single probability value.
const fn compute_prob_price(prob: usize) -> u16 {
    if prob == 0 {
        return (NUM_BIT_MODEL_TOTAL_BITS as u16) << NUM_BIT_PRICE_SHIFT_BITS;
    }
    if prob >= BIT_MODEL_TOTAL as usize {
        return 0;
    }
    let mut val = prob as u32;
    let mut bit_count = 0u32;
    let mut j = 0;
    while j < NUM_BIT_PRICE_SHIFT_BITS {
        val = val.wrapping_mul(val);
        bit_count <<= 1;
        while val >= (1 << 16) {
            val >>= 1;
            bit_count += 1;
        }
        j += 1;
    }
    let total_bits = (NUM_BIT_MODEL_TOTAL_BITS << NUM_BIT_PRICE_SHIFT_BITS) as u32;
    let price = total_bits.saturating_sub(15).saturating_sub(bit_count);
    price as u16
}

/// Returns the fractional bit price (in 1/16th bit units) for encoding bit `0` at probability `prob`.
#[inline(always)]
pub fn get_price_0(prob: u16) -> u32 {
    let p = (prob as usize).min(PROB_TABLE_SIZE - 1);
    PROB_PRICES[p] as u32
}

/// Returns the fractional bit price (in 1/16th bit units) for encoding bit `1` at probability `prob`.
#[inline(always)]
pub fn get_price_1(prob: u16) -> u32 {
    let complement = (BIT_MODEL_TOTAL as usize).saturating_sub(prob as usize);
    let p = complement.min(PROB_TABLE_SIZE - 1);
    PROB_PRICES[p] as u32
}

/// Returns the fractional bit price (in 1/16th bit units) for encoding `bit` (0 or 1) at probability `prob`.
#[inline(always)]
pub fn get_price(prob: u16, bit: u32) -> u32 {
    if bit == 0 {
        get_price_0(prob)
    } else {
        get_price_1(prob)
    }
}

/// Returns the fractional bit price for encoding direct unmodeled bits.
#[inline(always)]
pub const fn get_direct_bits_price(num_bits: u32) -> u32 {
    num_bits << NUM_BIT_PRICE_SHIFT_BITS
}

/// Computes total price for encoding a binary tree symbol (MSB first).
#[inline]
pub fn get_bit_tree_price(probs: &[u16], symbol: u32, num_bits: u32) -> u32 {
    let mut price = 0u32;
    let mut m = 1usize;
    for i in (0..num_bits).rev() {
        let bit = (symbol >> i) & 1;
        price += get_price(probs[m], bit);
        m = (m << 1) | (bit as usize);
    }
    price
}

/// Computes total price for encoding a reverse binary tree symbol (LSB first).
#[inline]
pub fn get_reverse_bit_tree_price(probs: &[u16], symbol: u32, num_bits: u32) -> u32 {
    let mut price = 0u32;
    let mut m = 1usize;
    for i in 0..num_bits {
        let bit = (symbol >> i) & 1;
        price += get_price(probs[m], bit);
        m = (m << 1) | (bit as usize);
    }
    price
}

/// 64-bit Single-Branch Binary Range Encoder for LZMA and LZMA2.
///
/// Employs a 64-bit `low` register to eliminate 32-bit carry overflow checks during arithmetic updates,
/// resolving carry cascades in constant time during byte normalization.
#[derive(Debug, Clone)]
pub struct Lzma2RangeEncoder {
    low: u64,
    range: u32,
    cache: u8,
    cache_size: u64,
    buf: Vec<u8>,
}

impl Default for Lzma2RangeEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Lzma2RangeEncoder {
    /// Creates a new `Lzma2RangeEncoder` initialized with full range (`0xFFFF_FFFF`).
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            low: 0,
            range: 0xFFFF_FFFF,
            cache: 0,
            cache_size: 1,
            buf: Vec::new(),
        }
    }

    /// Creates a new `Lzma2RangeEncoder` with pre-allocated output buffer capacity.
    #[inline]
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            low: 0,
            range: 0xFFFF_FFFF,
            cache: 0,
            cache_size: 1,
            buf: Vec::with_capacity(capacity),
        }
    }

    /// Resets the range encoder state for a new compression chunk.
    #[inline]
    pub fn reset(&mut self) {
        self.low = 0;
        self.range = 0xFFFF_FFFF;
        self.cache = 0;
        self.cache_size = 1;
        self.buf.clear();
    }

    /// Returns current range register value.
    #[inline(always)]
    pub const fn range(&self) -> u32 {
        self.range
    }

    /// Returns current low register value.
    #[inline(always)]
    pub const fn low(&self) -> u64 {
        self.low
    }

    /// Returns current cached byte.
    #[inline(always)]
    pub const fn cache(&self) -> u8 {
        self.cache
    }

    /// Returns current cache size (number of pending 0xFF bytes).
    #[inline(always)]
    pub const fn cache_size(&self) -> u64 {
        self.cache_size
    }

    /// Returns an immutable reference to the internal accumulated output buffer.
    #[inline(always)]
    pub fn buffer(&self) -> &[u8] {
        &self.buf
    }

    /// Returns the number of encoded bytes generated so far (including pending cache estimation).
    #[inline(always)]
    pub fn processed_size(&self) -> usize {
        self.buf.len() + (self.cache_size as usize)
    }

    /// 64-bit single-branch normalization and constant-time carry resolution.
    ///
    /// Shifts out the top byte of `low` to the target `out` buffer. If a carry occurred
    /// (`low >> 32 != 0`), it propagates by adding `1` to `cache` and rolling over all
    /// pending `0xFF` bytes to `0x00` in a single unrolled pass.
    #[inline]
    pub fn shift_low(&mut self, out: &mut Vec<u8>) {
        let low_hi = (self.low >> 32) as u32;
        let low_lo = self.low as u32;
        if low_lo < 0xFF00_0000 || low_hi != 0 {
            let mut temp = self.cache;
            loop {
                out.push(temp.wrapping_add(low_hi as u8));
                temp = 0xFF;
                self.cache_size -= 1;
                if self.cache_size == 0 {
                    break;
                }
            }
            self.cache = (low_lo >> 24) as u8;
        }
        self.cache_size += 1;
        self.low = (low_lo << 8) as u64;
    }

    /// Normalizes and flushes pending bytes directly into the internal buffer.
    #[inline(always)]
    fn shift_low_internal(&mut self) {
        let low_hi = (self.low >> 32) as u32;
        let low_lo = self.low as u32;
        if low_lo < 0xFF00_0000 || low_hi != 0 {
            let mut temp = self.cache;
            loop {
                self.buf.push(temp.wrapping_add(low_hi as u8));
                temp = 0xFF;
                self.cache_size -= 1;
                if self.cache_size == 0 {
                    break;
                }
            }
            self.cache = (low_lo >> 24) as u8;
        }
        self.cache_size += 1;
        self.low = (low_lo << 8) as u64;
    }

    /// Encodes a single adaptive bit into the internal buffer and updates the probability context.
    #[inline]
    pub fn encode_bit(&mut self, prob: &mut u16, bit: u32) {
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
            self.shift_low_internal();
        }
    }

    /// Encodes a single adaptive bit into an explicit destination buffer.
    #[inline]
    pub fn encode_bit_with_sink(&mut self, prob: &mut u16, bit: u32, out: &mut Vec<u8>) {
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

    /// Encodes unmodeled direct bits into the internal buffer.
    #[inline]
    pub fn encode_direct_bits(&mut self, value: u32, num_bits: u32) {
        for i in (0..num_bits).rev() {
            self.range >>= 1;
            if ((value >> i) & 1) == 1 {
                self.low += self.range as u64;
            }
            if self.range < TOP_VALUE {
                self.range <<= 8;
                self.shift_low_internal();
            }
        }
    }

    /// Encodes unmodeled direct bits into an explicit destination buffer.
    #[inline]
    pub fn encode_direct_bits_with_sink(&mut self, value: u32, num_bits: u32, out: &mut Vec<u8>) {
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
    pub fn encode_bit_tree(&mut self, probs: &mut [u16], symbol: u32, num_bits: u32) {
        let mut m = 1usize;
        for i in (0..num_bits).rev() {
            let bit = (symbol >> i) & 1;
            self.encode_bit(&mut probs[m], bit);
            m = (m << 1) | (bit as usize);
        }
    }

    /// Encodes a reverse binary tree symbol with `num_bits` levels (LSB first).
    #[inline]
    pub fn encode_reverse_bit_tree(&mut self, probs: &mut [u16], symbol: u32, num_bits: u32) {
        let mut m = 1usize;
        for i in 0..num_bits {
            let bit = (symbol >> i) & 1;
            self.encode_bit(&mut probs[m], bit);
            m = (m << 1) | (bit as usize);
        }
    }

    /// Encodes a standard literal byte (8 bits MSB first).
    #[inline]
    pub fn encode_literal_byte(&mut self, probs: &mut [u16], byte: u8) {
        let mut symbol = 1usize;
        for i in (0..8).rev() {
            let bit = ((byte >> i) & 1) as u32;
            self.encode_bit(&mut probs[symbol], bit);
            symbol = (symbol << 1) | (bit as usize);
        }
    }

    /// Encodes a literal byte with match-byte context guidance for states >= 7.
    #[inline]
    pub fn encode_matched_byte(&mut self, probs: &mut [u16], byte: u8, mut match_byte: u8) {
        let mut symbol = 1usize;
        let mut same = true;
        for i in (0..8).rev() {
            let bit = ((byte >> i) & 1) as u32;
            if same {
                let match_bit = ((match_byte >> 7) & 1) as usize;
                match_byte <<= 1;
                let prob_idx = 0x100 + (match_bit << 8) + symbol;
                self.encode_bit(&mut probs[prob_idx], bit);
                symbol = (symbol << 1) | (bit as usize);
                if match_bit != (bit as usize) {
                    same = false;
                }
            } else {
                self.encode_bit(&mut probs[symbol], bit);
                symbol = (symbol << 1) | (bit as usize);
            }
        }
    }

    /// Flushes final 5 bytes of the range encoder into the target sink buffer.
    #[inline]
    pub fn flush(&mut self, out: &mut Vec<u8>) {
        for _ in 0..5 {
            self.shift_low(out);
        }
    }

    /// Finalizes encoding by flushing 5 termination bytes into the internal buffer.
    #[inline]
    pub fn finish(&mut self) -> &[u8] {
        for _ in 0..5 {
            self.shift_low_internal();
        }
        &self.buf
    }

    /// Consumes the encoder, finalizes the bitstream, and returns the accumulated vector.
    #[inline]
    pub fn into_vec(mut self) -> Vec<u8> {
        let _ = self.finish();
        self.buf
    }
}
