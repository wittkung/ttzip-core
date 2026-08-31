// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe Pure-Rust Brotli sliding ring buffer with 542-byte write-ahead slack (RFC 7932).

use super::error::BrotliError;
use super::window::{BROTLI_LARGE_MAX_WINDOW_BITS, BROTLI_MIN_WINDOW_BITS};

/// Write-ahead slack size (in bytes) allocated beyond the power-of-two window size.
///
/// Per RFC 7932 and C Brotli reference implementation:
/// - 255 prefix + 32 base word + 255 suffix = 542 bytes for maximum transformed dictionary word insertion.
/// - Fast SIMD/128-bit vector copy overshoot absorption without bounds check branching in hot loops.
pub const RING_BUFFER_WRITE_AHEAD_SLACK: usize = 542;

/// High-performance sliding ring buffer for Brotli streaming decompression.
///
/// Manages window history, LZ77 backward reference match copy (with overlap run-length expansion),
/// wrapping across power-of-two window boundaries, and streaming output draining.
#[derive(Debug, Clone)]
pub struct BrotliDecoderRingBuffer {
    /// Internal byte buffer allocated with `size + RING_BUFFER_WRITE_AHEAD_SLACK` capacity.
    pub buffer: Box<[u8]>,
    /// Logical sliding window size (must be power of two, e.g., 1 << window_bits).
    pub size: usize,
    /// Bitmask for fast modulo wrapping (`size - 1`).
    pub mask: usize,
    /// Monotonically advancing total written byte counter.
    pub pos: usize,
    /// Monotonically advancing total drained byte counter.
    pub tail: usize,
    /// Window bits exponent (10..=30).
    pub window_bits: u8,
}

impl BrotliDecoderRingBuffer {
    /// Creates a new `BrotliDecoderRingBuffer` for the given window bits exponent.
    ///
    /// # Errors
    /// Returns `BrotliError::InvalidWindowBits` if `window_bits` is not in `10..=30`.
    pub fn new(window_bits: u8) -> Result<Self, BrotliError> {
        if !(BROTLI_MIN_WINDOW_BITS..=BROTLI_LARGE_MAX_WINDOW_BITS).contains(&window_bits) {
            return Err(BrotliError::InvalidWindowBits(window_bits));
        }

        let size = 1usize << window_bits;
        let total_capacity = size
            .checked_add(RING_BUFFER_WRITE_AHEAD_SLACK)
            .ok_or(BrotliError::InvalidWindowBits(window_bits))?;

        let buffer = vec![0u8; total_capacity].into_boxed_slice();

        Ok(Self {
            buffer,
            size,
            mask: size - 1,
            pos: 0,
            tail: 0,
            window_bits,
        })
    }

    /// Creates a ring buffer from an explicit power-of-two size.
    ///
    /// # Errors
    /// Returns `BrotliError::InvalidWindowBits` if `size` is not a power of two or out of range.
    pub fn with_size(size: usize) -> Result<Self, BrotliError> {
        if !size.is_power_of_two() {
            return Err(BrotliError::InvalidWindowBits(0));
        }
        let window_bits = size.trailing_zeros() as u8;
        Self::new(window_bits)
    }

    /// Writes a single decoded literal byte into the ring buffer and advances `pos`.
    #[inline]
    pub fn write_byte(&mut self, byte: u8) {
        let idx = self.pos & self.mask;
        self.buffer[idx] = byte;
        self.pos += 1;
    }

    /// Copies an LZ77 backward reference match into the ring buffer.
    ///
    /// Handles overlapping copy ranges (run-length expansion where `distance < length`)
    /// and boundary wrapping across the power-of-two window with zero heap allocation.
    ///
    /// # Errors
    /// Returns `BrotliError::CorruptHeader` if `distance == 0` or exceeds the sliding window size / available history.
    #[inline]
    pub fn copy_match(&mut self, distance: usize, length: usize) -> Result<(), BrotliError> {
        if distance == 0 || distance > self.size || distance > self.pos {
            return Err(BrotliError::CorruptHeader(format!(
                "Invalid backward reference distance: {distance}, pos: {}, window_size: {}",
                self.pos, self.size
            )));
        }

        for _ in 0..length {
            let src_idx = (self.pos - distance) & self.mask;
            let byte = self.buffer[src_idx];
            let dst_idx = self.pos & self.mask;
            self.buffer[dst_idx] = byte;
            self.pos += 1;
        }

        Ok(())
    }

    /// Copies a contiguous byte slice into the ring buffer, wrapping across window boundaries as needed.
    #[inline]
    pub fn copy_slice(&mut self, src: &[u8]) {
        for &byte in src {
            let dst_idx = self.pos & self.mask;
            self.buffer[dst_idx] = byte;
            self.pos += 1;
        }
    }

    /// Drains available decoded data from the ring buffer into `out`, advancing `tail`.
    ///
    /// Returns the exact number of bytes copied into `out`.
    pub fn drain_to(&mut self, out: &mut [u8]) -> usize {
        let avail = self.available_data();
        let to_drain = avail.min(out.len());
        if to_drain == 0 {
            return 0;
        }

        let start_idx = self.tail & self.mask;
        let first_chunk_len = (self.size - start_idx).min(to_drain);

        out[..first_chunk_len]
            .copy_from_slice(&self.buffer[start_idx..start_idx + first_chunk_len]);

        if to_drain > first_chunk_len {
            let second_chunk_len = to_drain - first_chunk_len;
            out[first_chunk_len..to_drain].copy_from_slice(&self.buffer[..second_chunk_len]);
        }

        self.tail += to_drain;
        to_drain
    }

    /// Returns the number of unconsumed (undrained) bytes currently available in the ring buffer.
    #[inline]
    pub fn available_data(&self) -> usize {
        self.pos.saturating_sub(self.tail)
    }

    /// Returns `true` if all produced data has been drained (`pos == tail`).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.pos == self.tail
    }

    /// Returns the current write position index.
    #[inline]
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Returns the current drain read cursor index.
    #[inline]
    pub fn tail(&self) -> usize {
        self.tail
    }

    /// Returns the logical sliding window size.
    #[inline]
    pub fn size(&self) -> usize {
        self.size
    }

    /// Returns the sliding window mask (`size - 1`).
    #[inline]
    pub fn mask(&self) -> usize {
        self.mask
    }

    /// Returns the window bits exponent.
    #[inline]
    pub fn window_bits(&self) -> u8 {
        self.window_bits
    }

    /// Returns the total physical capacity of the allocated buffer including write-ahead slack.
    #[inline]
    pub fn total_capacity(&self) -> usize {
        self.buffer.len()
    }

    /// Returns the byte at the specified absolute stream position index modulo window size.
    #[inline]
    pub fn get_byte_at(&self, abs_pos: usize) -> u8 {
        self.buffer[abs_pos & self.mask]
    }

    /// Returns the byte `back_offset` bytes before current `pos` (1-based offset).
    #[inline]
    pub fn get_recent_byte(&self, back_offset: usize) -> Option<u8> {
        if back_offset == 0 || back_offset > self.pos {
            None
        } else {
            Some(self.buffer[(self.pos - back_offset) & self.mask])
        }
    }

    /// Resets ring buffer cursors and zeroes the memory buffer.
    pub fn reset(&mut self) {
        self.pos = 0;
        self.tail = 0;
        self.buffer.fill(0);
    }
}
