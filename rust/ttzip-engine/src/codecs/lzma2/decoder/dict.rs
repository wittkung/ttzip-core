// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Resizable Sliding Dictionary History Buffer for LZMA2 match reference decoding.

use super::header::Lzma2DecodeError;

/// Maximum default sliding dictionary size (64 MiB).
pub const LZMA2_DEFAULT_DICT_SIZE: usize = 64 * 1024 * 1024;

/// Sliding Dictionary Window buffer for tracking uncompressed history and match resolution.
#[derive(Debug, Clone)]
pub struct Lzma2Dict {
    buffer: Vec<u8>,
    max_size: usize,
    total_written: usize,
}

impl Lzma2Dict {
    /// Creates a new sliding dictionary with a given maximum capacity.
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(max_size.min(1024 * 1024)),
            max_size,
            total_written: 0,
        }
    }

    /// Clears dictionary history, resetting position and total written count to zero.
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.total_written = 0;
    }

    /// Returns the number of history bytes currently buffered in the sliding window.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Returns `true` if the dictionary contains no history bytes.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Returns the total number of uncompressed bytes written across chunks.
    #[inline(always)]
    pub const fn total_written(&self) -> usize {
        self.total_written
    }

    /// Retrieves the most recently emitted byte (or `0` if history is empty).
    #[inline(always)]
    pub fn last_byte(&self) -> u8 {
        self.buffer.last().copied().unwrap_or(0)
    }

    /// Retrieves a byte at a 0-based distance offset (`0` = 1 byte back, `1` = 2 bytes back).
    ///
    /// # Errors
    /// Returns `Lzma2DecodeError::InvalidDistance` if `distance > dict.len()`.
    #[inline(always)]
    pub fn get_byte_at_distance(&self, distance_offset: usize) -> Result<u8, Lzma2DecodeError> {
        let dist = distance_offset + 1;
        let buf_len = self.buffer.len();
        if dist > buf_len {
            return Err(Lzma2DecodeError::InvalidDistance {
                distance: dist,
                dict_len: buf_len,
            });
        }
        Ok(self.buffer[buf_len - dist])
    }

    /// Appends a single byte to the dictionary, compacting when capacity threshold is reached.
    #[inline]
    pub fn put_byte(&mut self, b: u8) {
        self.buffer.push(b);
        self.total_written += 1;
        self.maybe_compact();
    }

    /// Appends a slice of bytes to the dictionary, compacting when capacity threshold is reached.
    #[inline]
    pub fn put_slice(&mut self, slice: &[u8]) {
        self.buffer.extend_from_slice(slice);
        self.total_written += slice.len();
        self.maybe_compact();
    }

    #[inline]
    fn maybe_compact(&mut self) {
        if self.buffer.len() > self.max_size.saturating_mul(2) && self.max_size > 0 {
            let trim_len = self.buffer.len() - self.max_size;
            self.buffer.drain(0..trim_len);
        }
    }
}
