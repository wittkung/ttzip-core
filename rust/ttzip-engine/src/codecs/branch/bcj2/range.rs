// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 11-Bit 258-Context Adaptive Binary Range Coder for BCJ2.
//!
//! Provides the bitstream entropy encoder and decoder governing the status
//! stream (`StreamRc`) in the 7-Zip BCJ2 4-Stream executable filter.

use std::io::{self, Read, Write};

/// Total precision bits for bit model probabilities in BCJ2 (11 bits).
pub const NUM_BIT_MODEL_TOTAL_BITS: usize = 11;

/// Total scale of the probability model (2^11 = 2048).
pub const BIT_MODEL_TOTAL: u32 = 1 << NUM_BIT_MODEL_TOTAL_BITS;

/// Adaptation rate shift for updating probability models (5 bits, 1/32 adjustment).
pub const NUM_MOVE_BITS: usize = 5;

/// Initial probability value representing 50% likelihood (1024).
pub const PROB_INIT_VAL: u16 = (BIT_MODEL_TOTAL / 2) as u16;

/// Total number of probability contexts for BCJ2 (256 for 0xE8 previous bytes + 2 for 0xE9/0x0F).
pub const NUM_BCJ2_PROBS: usize = 258;

/// Normalization boundary threshold for range coder registers (2^24 = 0x0100_0000).
pub const TOP_VALUE: u32 = 1 << 24;

/// 258-Context probability state table for BCJ2 binary range coding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bcj2RangeDecoderProbs {
    /// Array of 258 probability models initialized to 1024 (50% likelihood).
    pub probs: [u16; NUM_BCJ2_PROBS],
}

impl Default for Bcj2RangeDecoderProbs {
    #[inline]
    fn default() -> Self {
        Self {
            probs: [PROB_INIT_VAL; NUM_BCJ2_PROBS],
        }
    }
}

impl Bcj2RangeDecoderProbs {
    /// Creates a new table of probability models initialized to `PROB_INIT_VAL` (1024).
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            probs: [PROB_INIT_VAL; NUM_BCJ2_PROBS],
        }
    }

    /// Resets all 258 probability models back to `PROB_INIT_VAL` (1024).
    #[inline]
    pub fn reset(&mut self) {
        self.probs.fill(PROB_INIT_VAL);
    }

    /// Returns a slice view of the probability models.
    #[inline]
    #[must_use]
    pub const fn as_slice(&self) -> &[u16; NUM_BCJ2_PROBS] {
        &self.probs
    }

    /// Returns a mutable slice view of the probability models.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u16; NUM_BCJ2_PROBS] {
        &mut self.probs
    }
}

impl std::ops::Index<usize> for Bcj2RangeDecoderProbs {
    type Output = u16;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.probs[index]
    }
}

impl std::ops::IndexMut<usize> for Bcj2RangeDecoderProbs {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.probs[index]
    }
}

impl AsRef<[u16]> for Bcj2RangeDecoderProbs {
    #[inline]
    fn as_ref(&self) -> &[u16] {
        &self.probs
    }
}

impl AsMut<[u16]> for Bcj2RangeDecoderProbs {
    #[inline]
    fn as_mut(&mut self) -> &mut [u16] {
        &mut self.probs
    }
}

/// Streaming Adaptive Binary Range Decoder for BCJ2 status bitstream.
#[derive(Debug)]
pub struct Bcj2RangeDecoder<R: Read> {
    /// Current code register.
    pub code: u32,
    /// Current range interval width.
    pub range: u32,
    /// Underlying input stream.
    pub inner: R,
}

impl<R: Read> Bcj2RangeDecoder<R> {
    /// Creates a new `Bcj2RangeDecoder` by reading 5 initial bootstrap bytes from `inner`.
    ///
    /// # Errors
    /// Returns `io::Error` if reading the initial 5 bytes fails or encounters premature EOF.
    pub fn new(mut inner: R) -> io::Result<Self> {
        let mut buf = [0u8; 5];
        inner.read_exact(&mut buf)?;

        let mut code = 0u32;
        for &b in &buf {
            code = (code << 8) | (b as u32);
        }

        Ok(Self {
            code,
            range: 0xFFFF_FFFF,
            inner,
        })
    }

    /// Decodes a single bit (0 or 1) using the given adaptive probability context.
    ///
    /// Updates the probability model in-place according to:
    /// - Bit 0: `*prob += (2048 - *prob) >> 5`
    /// - Bit 1: `*prob -= *prob >> 5`
    ///
    /// # Errors
    /// Returns `io::Error` if renormalizing reads fail from the underlying stream.
    #[inline]
    pub fn decode_bit(&mut self, prob: &mut u16) -> io::Result<u32> {
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

        while self.range < TOP_VALUE {
            let mut byte_buf = [0u8; 1];
            self.inner.read_exact(&mut byte_buf)?;
            self.range <<= 8;
            self.code = (self.code << 8) | (byte_buf[0] as u32);
        }

        Ok(bit)
    }

    /// Decodes a single unmodeled direct bit (50% uniform probability) using branchless arithmetic.
    ///
    /// # Errors
    /// Returns `io::Error` if renormalizing reads fail from the underlying stream.
    #[inline]
    pub fn decode_direct_bit(&mut self) -> io::Result<u32> {
        self.range >>= 1;
        self.code = self.code.wrapping_sub(self.range);
        let t = 0u32.wrapping_sub(self.code >> 31);
        let bit = (t.wrapping_add(1)) & 1;
        self.code = self.code.wrapping_add(self.range & t);

        if self.range < TOP_VALUE {
            let mut byte_buf = [0u8; 1];
            self.inner.read_exact(&mut byte_buf)?;
            self.range <<= 8;
            self.code = (self.code << 8) | (byte_buf[0] as u32);
        }

        Ok(bit)
    }

    /// Returns a reference to the underlying reader.
    #[inline]
    pub const fn get_ref(&self) -> &R {
        &self.inner
    }

    /// Returns a mutable reference to the underlying reader.
    #[inline]
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    /// Unwraps the decoder, returning the underlying reader.
    #[inline]
    pub fn into_inner(self) -> R {
        self.inner
    }
}

/// Streaming Adaptive Binary Range Encoder for BCJ2 status bitstream.
#[derive(Debug)]
pub struct Bcj2RangeEncoder<W: Write> {
    /// 64-bit low range register for carry propagation.
    pub low: u64,
    /// Current range interval width.
    pub range: u32,
    /// Count of pending buffered 0xFF carry bytes.
    pub cache_size: u64,
    /// Pending output byte buffer.
    pub cache: u8,
    /// Underlying output writer sink.
    pub inner: W,
}

impl<W: Write> Bcj2RangeEncoder<W> {
    /// Creates a new `Bcj2RangeEncoder` wrapping `inner`.
    #[inline]
    pub const fn new(inner: W) -> Self {
        Self {
            low: 0,
            range: 0xFFFF_FFFF,
            cache_size: 1,
            cache: 0,
            inner,
        }
    }

    /// Encodes a single bit (0 or 1) using the given adaptive probability context.
    ///
    /// Updates the probability model in-place according to:
    /// - Bit 0: `*prob += (2048 - *prob) >> 5`
    /// - Bit 1: `*prob -= *prob >> 5`
    ///
    /// # Errors
    /// Returns `io::Error` if emitting normalized bytes to the underlying writer fails.
    #[inline]
    pub fn encode_bit(&mut self, prob: &mut u16, bit: u32) -> io::Result<()> {
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
            self.shift_low()?;
        }

        Ok(())
    }

    /// Encodes a single unmodeled direct bit (50% uniform probability).
    ///
    /// # Errors
    /// Returns `io::Error` if emitting normalized bytes to the underlying writer fails.
    #[inline]
    pub fn encode_direct_bit(&mut self, bit: u32) -> io::Result<()> {
        self.range >>= 1;
        if (bit & 1) == 1 {
            self.low += self.range as u64;
        }
        if self.range < TOP_VALUE {
            self.range <<= 8;
            self.shift_low()?;
        }
        Ok(())
    }

    /// Normalizes and emits cached bytes handling 0xFF carry cascades.
    #[inline]
    fn shift_low(&mut self) -> io::Result<()> {
        let low_hi = (self.low >> 32) as u32;
        if low_hi != 0 || self.low < 0xFF00_0000 {
            let mut temp = self.cache;
            loop {
                self.inner.write_all(&[temp.wrapping_add(low_hi as u8)])?;
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
        Ok(())
    }

    /// Flushes all remaining low-range bits to finalize the bitstream.
    ///
    /// # Errors
    /// Returns `io::Error` if writing flushed bytes to the inner writer fails.
    pub fn flush(&mut self) -> io::Result<()> {
        for _ in 0..5 {
            self.shift_low()?;
        }
        self.inner.flush()
    }

    /// Flushes the encoder and unwraps the underlying writer.
    ///
    /// # Errors
    /// Returns `io::Error` if flushing fails.
    pub fn finish(mut self) -> io::Result<W> {
        self.flush()?;
        Ok(self.inner)
    }

    /// Returns a reference to the underlying writer.
    #[inline]
    pub const fn get_ref(&self) -> &W {
        &self.inner
    }

    /// Returns a mutable reference to the underlying writer.
    #[inline]
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }
}
