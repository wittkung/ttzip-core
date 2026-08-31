// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! RFC 7932 64-bit register bitstream reader with zero-branch prefetch and byte rewind.

use super::error::BrotliError;

/// High-throughput LSB-first bitstream reader using a 64-bit wide register accumulator.
///
/// Designed for low-latency, zero-heap entropy decoding and block parsing in Brotli decompression.
#[derive(Debug, Clone)]
pub struct BrotliBitReader<'a> {
    /// 64-bit wide register accumulator containing buffered bitstream data.
    pub val: u64,
    /// Number of valid, unconsumed bits currently buffered in `val`.
    pub bit_pos: u32,
    /// Underlying input byte slice.
    pub input: &'a [u8],
    /// Current byte offset cursor in `input`.
    pub pos: usize,
}

impl<'a> BrotliBitReader<'a> {
    /// Creates a new `BrotliBitReader` initialized over the given byte slice and prefills the window.
    pub fn new(input: &'a [u8]) -> Self {
        let mut reader = Self {
            val: 0,
            bit_pos: 0,
            input,
            pos: 0,
        };
        reader.fill_window();
        reader
    }

    /// Prefetches up to 32 bits into the 64-bit accumulator when `bit_pos <= 32`.
    ///
    /// Performs unaligned 32-bit little-endian loads on the fast path, or byte-by-byte loads near EOF.
    #[inline]
    pub fn fill_window(&mut self) {
        if self.bit_pos <= 32 {
            let remaining = self.input.len().saturating_sub(self.pos);
            if remaining >= 4 {
                let chunk: [u8; 4] = self.input[self.pos..self.pos + 4]
                    .try_into()
                    .expect("slice length guaranteed >= 4");
                let raw = u32::from_le_bytes(chunk) as u64;
                self.val |= raw << self.bit_pos;
                self.bit_pos += 32;
                self.pos += 4;
            } else {
                while self.pos < self.input.len() && self.bit_pos <= 56 {
                    let byte = self.input[self.pos] as u64;
                    self.val |= byte << self.bit_pos;
                    self.bit_pos += 8;
                    self.pos += 1;
                }
            }
        }
    }

    /// Peeks `n` (0..=32) least-significant bits from the accumulator without consuming them.
    #[inline]
    pub fn peek_bits(&self, n: u32) -> u32 {
        if n == 0 {
            0
        } else if n >= 32 {
            self.val as u32
        } else {
            (self.val as u32) & ((1u32 << n) - 1)
        }
    }

    /// Drops `n` bits from the accumulator by shifting right and decrementing `bit_pos`.
    #[inline]
    pub fn drop_bits(&mut self, n: u32) {
        if n >= 64 {
            self.val = 0;
            self.bit_pos = 0;
        } else {
            self.val >>= n;
            self.bit_pos = self.bit_pos.saturating_sub(n);
        }
    }

    /// Reads `n` (0..=32) bits from the stream, refilling the accumulator as necessary.
    ///
    /// Returns `Err(BrotliError::UnexpectedEof)` if there are not enough bits available.
    #[inline]
    pub fn read_bits(&mut self, n: u32) -> Result<u32, BrotliError> {
        if n == 0 {
            return Ok(0);
        }
        if self.bit_pos < n {
            self.fill_window();
            if self.bit_pos < n {
                return Err(BrotliError::UnexpectedEof);
            }
        }
        let bits = self.peek_bits(n);
        self.drop_bits(n);
        Ok(bits)
    }

    /// Reads a single 8-bit byte from the bitstream.
    #[inline]
    pub fn read_byte(&mut self) -> Result<u8, BrotliError> {
        self.read_bits(8).map(|b| b as u8)
    }

    /// Aligns the bitstream to the next byte boundary, verifying that all skipped padding bits are zero.
    ///
    /// Per RFC 7932, any non-zero padding bits within a byte boundary jump cause a decoding failure.
    #[inline]
    pub fn jump_to_byte_boundary(&mut self) -> Result<(), BrotliError> {
        let pad_bits = self.bit_pos & 7;
        if pad_bits > 0 {
            let pad = self.peek_bits(pad_bits);
            if pad != 0 {
                return Err(BrotliError::InvalidPadding);
            }
            self.drop_bits(pad_bits);
        }
        Ok(())
    }

    /// Rewinds unconsumed whole bytes from the 64-bit accumulator back to the slice cursor `pos`.
    ///
    /// Clears the accumulator and returns the exact number of fully consumed whole bytes.
    #[inline]
    pub fn unload(&mut self) -> usize {
        let unconsumed_bytes = (self.bit_pos >> 3) as usize;
        let consumed = self.pos.saturating_sub(unconsumed_bytes);
        self.pos = consumed;
        self.val = 0;
        self.bit_pos = 0;
        consumed
    }

    /// Returns the total number of unconsumed bits currently buffered in the accumulator.
    #[inline]
    pub fn unconsumed_bits(&self) -> u32 {
        self.bit_pos
    }

    /// Returns the number of unconsumed padding bits remaining in the current byte before boundary alignment (0..=7).
    #[inline]
    pub fn bits_remaining_in_byte(&self) -> u32 {
        self.bit_pos & 7
    }

    /// Returns true if the reader has exhausted both the input slice and the accumulator.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bit_pos == 0 && self.pos >= self.input.len()
    }
}
