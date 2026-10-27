// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Power-of-two circular ring buffer for LZMA2 match reference decoding.
//!
//! Eliminates memory moves and reallocations by utilizing branchless bitwise
//! masking (`pos & mask`) over a fixed-capacity buffer.

use super::header::Lzma2DecodeError;

/// Maximum default sliding dictionary size (64 MiB).
pub const LZMA2_DEFAULT_DICT_SIZE: usize = 64 * 1024 * 1024;

/// Power-of-two circular ring buffer for LZMA2 sliding dictionary history.
#[derive(Debug, Clone)]
pub struct Lzma2RingDict {
    buffer: Box<[u8]>,
    mask: usize,
    pos: usize,
    total_written: usize,
    max_size: usize,
}

/// Backwards-compatible alias for the LZMA2 sliding dictionary.
pub type Lzma2Dict = Lzma2RingDict;

impl Lzma2RingDict {
    /// Creates a new circular ring dictionary with a given maximum window capacity.
    ///
    /// The underlying buffer is allocated to the smallest power of two that is at least
    /// `max_size` (minimum 4096 bytes) to allow branchless `pos & mask` addressing.
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        let effective_max = max_size.max(1);
        let cap = effective_max.max(4096).next_power_of_two();
        let mask = cap - 1;
        Self {
            buffer: vec![0u8; cap].into_boxed_slice(),
            mask,
            pos: 0,
            total_written: 0,
            max_size: effective_max,
        }
    }

    /// Clears dictionary history, resetting write head and total written count to zero.
    pub fn reset(&mut self) {
        self.pos = 0;
        self.total_written = 0;
    }

    /// Returns the number of history bytes currently accessible in the sliding window.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.total_written.min(self.max_size)
    }

    /// Returns `true` if the dictionary contains no history bytes.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.total_written == 0
    }

    /// Returns the total number of uncompressed bytes written across chunks.
    #[inline(always)]
    pub const fn total_written(&self) -> usize {
        self.total_written
    }

    /// Retrieves the most recently emitted byte (or `0` if history is empty).
    #[inline(always)]
    pub fn last_byte(&self) -> u8 {
        if self.total_written == 0 {
            0
        } else {
            self.buffer[self.pos.wrapping_sub(1) & self.mask]
        }
    }

    /// Retrieves a byte at a 0-based distance offset (`0` = 1 byte back, `1` = 2 bytes back).
    ///
    /// # Errors
    /// Returns `Lzma2DecodeError::InvalidDistance` if `distance > dict.len()`.
    #[inline(always)]
    pub fn get_byte_at_distance(&self, distance_offset: usize) -> Result<u8, Lzma2DecodeError> {
        let dist = distance_offset + 1;
        let accessible_len = self.len();
        if dist > accessible_len {
            return Err(Lzma2DecodeError::InvalidDistance {
                distance: dist,
                dict_len: accessible_len,
            });
        }
        let read_idx = self.pos.wrapping_sub(dist) & self.mask;
        Ok(self.buffer[read_idx])
    }

    /// Appends a single byte to the dictionary using branchless index masking.
    #[inline(always)]
    pub fn put_byte(&mut self, b: u8) {
        let write_idx = self.pos & self.mask;
        self.buffer[write_idx] = b;
        self.pos = self.pos.wrapping_add(1);
        self.total_written += 1;
    }

    /// Appends a slice of bytes to the dictionary with vectorized chunk copies.
    #[inline]
    pub fn put_slice(&mut self, slice: &[u8]) {
        let slice_len = slice.len();
        if slice_len == 0 {
            return;
        }
        self.total_written += slice_len;

        let buf_len = self.buffer.len();
        let (slice_to_write, start_offset) = if slice_len > buf_len {
            let excess = slice_len - buf_len;
            (&slice[excess..], excess)
        } else {
            (slice, 0)
        };

        let start_pos = (self.pos + start_offset) & self.mask;
        let end_pos = start_pos + slice_to_write.len();

        if end_pos <= buf_len {
            self.buffer[start_pos..end_pos].copy_from_slice(slice_to_write);
        } else {
            let first_len = buf_len - start_pos;
            self.buffer[start_pos..].copy_from_slice(&slice_to_write[..first_len]);
            self.buffer[..slice_to_write.len() - first_len].copy_from_slice(&slice_to_write[first_len..]);
        }
        self.pos = self.pos.wrapping_add(slice_len);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_dict_wrap_around_and_continuity() {
        let mut dict = Lzma2RingDict::new(16);
        for i in 0..100u8 {
            dict.put_byte(i);
        }
        assert_eq!(dict.len(), 16);
        assert_eq!(dict.total_written(), 100);
        assert_eq!(dict.last_byte(), 99);

        for dist in 0..16 {
            let expected = 99 - (dist as u8);
            assert_eq!(dict.get_byte_at_distance(dist).unwrap(), expected);
        }
        assert!(dict.get_byte_at_distance(16).is_err());
    }

    #[test]
    fn test_ring_dict_put_slice_wrapping() {
        let mut dict = Lzma2RingDict::new(4096);
        let data: Vec<u8> = (0..5000).map(|i| (i % 256) as u8).collect();
        dict.put_slice(&data);

        assert_eq!(dict.len(), 4096);
        assert_eq!(dict.total_written(), 5000);
        assert_eq!(dict.last_byte(), (4999 % 256) as u8);

        for dist in 0..100 {
            let expected = ((4999 - dist) % 256) as u8;
            assert_eq!(dict.get_byte_at_distance(dist).unwrap(), expected);
        }
    }
}
