// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Structure-aware reverse-consuming fuzz data producer for deterministic testing.
//!
//! Consumes scalar configuration parameters and metadata from the tail (end)
//! of a fuzzed byte slice, reserving the unshifted head slice as a pristine
//! continuous payload. This ensures that parameter mutations do not shift or
//! disrupt structured file format magic bytes and headers located at offset 0.
//!
//! Provides zero-panic, zero out-of-bounds guarantees with safe degradation
//! when insufficient bytes are available.

/// Structure-aware fuzzer input provider with reverse tail consumption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuzzDataProducer<'a> {
    data: &'a [u8],
}

impl<'a> FuzzDataProducer<'a> {
    /// Creates a new `FuzzDataProducer` wrapping the provided byte slice.
    #[inline]
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    /// Returns the number of remaining unconsumed bytes.
    #[inline]
    pub fn remaining_bytes(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if there are no remaining bytes.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Consumes a single byte from the tail of the buffer.
    ///
    /// Returns `0` if the buffer is empty.
    #[inline]
    pub fn consume_u8(&mut self) -> u8 {
        if self.data.is_empty() {
            return 0;
        }
        let last_idx = self.data.len() - 1;
        let val = self.data[last_idx];
        self.data = &self.data[..last_idx];
        val
    }

    /// Consumes a signed 8-bit integer from the tail of the buffer.
    #[inline]
    pub fn consume_i8(&mut self) -> i8 {
        self.consume_u8() as i8
    }

    /// Consumes a 16-bit unsigned integer (little-endian) from the tail.
    ///
    /// Zero-pads if fewer than 2 bytes remain; returns `0` if empty.
    #[inline]
    pub fn consume_u16(&mut self) -> u16 {
        let mut bytes = [0u8; 2];
        let take = self.data.len().min(2);
        if take == 0 {
            return 0;
        }
        let start = self.data.len() - take;
        bytes[..take].copy_from_slice(&self.data[start..]);
        self.data = &self.data[..start];
        u16::from_le_bytes(bytes)
    }

    /// Consumes a 16-bit signed integer (little-endian) from the tail.
    #[inline]
    pub fn consume_i16(&mut self) -> i16 {
        self.consume_u16() as i16
    }

    /// Consumes a 32-bit unsigned integer (little-endian) from the tail.
    ///
    /// Zero-pads if fewer than 4 bytes remain; returns `0` if empty.
    #[inline]
    pub fn consume_u32(&mut self) -> u32 {
        let mut bytes = [0u8; 4];
        let take = self.data.len().min(4);
        if take == 0 {
            return 0;
        }
        let start = self.data.len() - take;
        bytes[..take].copy_from_slice(&self.data[start..]);
        self.data = &self.data[..start];
        u32::from_le_bytes(bytes)
    }

    /// Consumes a 32-bit signed integer (little-endian) from the tail.
    #[inline]
    pub fn consume_i32(&mut self) -> i32 {
        self.consume_u32() as i32
    }

    /// Consumes a 64-bit unsigned integer (little-endian) from the tail.
    ///
    /// Zero-pads if fewer than 8 bytes remain; returns `0` if empty.
    #[inline]
    pub fn consume_u64(&mut self) -> u64 {
        let mut bytes = [0u8; 8];
        let take = self.data.len().min(8);
        if take == 0 {
            return 0;
        }
        let start = self.data.len() - take;
        bytes[..take].copy_from_slice(&self.data[start..]);
        self.data = &self.data[..start];
        u64::from_le_bytes(bytes)
    }

    /// Consumes a 64-bit signed integer (little-endian) from the tail.
    #[inline]
    pub fn consume_i64(&mut self) -> i64 {
        self.consume_u64() as i64
    }

    /// Consumes a boolean flag from the tail.
    ///
    /// Returns `true` if the least-significant bit of the consumed byte is 1.
    #[inline]
    pub fn consume_bool(&mut self) -> bool {
        (self.consume_u8() & 1) != 0
    }

    /// Consumes an 8-bit unsigned integer clamped within `[min, max]`.
    #[inline]
    pub fn consume_u8_range(&mut self, min: u8, max: u8) -> u8 {
        if min >= max {
            return min;
        }
        let span = (max as u16) - (min as u16) + 1;
        let val = self.consume_u8() as u16;
        (min as u16 + (val % span)) as u8
    }

    /// Consumes a 16-bit unsigned integer clamped within `[min, max]`.
    #[inline]
    pub fn consume_u16_range(&mut self, min: u16, max: u16) -> u16 {
        if min >= max {
            return min;
        }
        let span = (max as u32) - (min as u32) + 1;
        let val = self.consume_u16() as u32;
        (min as u32 + (val % span)) as u16
    }

    /// Consumes a 32-bit unsigned integer clamped within `[min, max]`.
    #[inline]
    pub fn consume_u32_range(&mut self, min: u32, max: u32) -> u32 {
        if min >= max {
            return min;
        }
        let span = (max as u64) - (min as u64) + 1;
        let val = self.consume_u32() as u64;
        (min as u64 + (val % span)) as u32
    }

    /// Consumes a 64-bit unsigned integer clamped within `[min, max]`.
    #[inline]
    pub fn consume_u64_range(&mut self, min: u64, max: u64) -> u64 {
        if min >= max {
            return min;
        }
        let span = (max as u128) - (min as u128) + 1;
        let val = self.consume_u64() as u128;
        (min as u128 + (val % span)) as u64
    }

    /// Consumes a `usize` integer clamped within `[min, max]`.
    #[inline]
    pub fn consume_usize_range(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            return min;
        }
        let span = (max as u128) - (min as u128) + 1;
        let val = self.consume_u64() as u128;
        (min as u128 + (val % span)) as usize
    }

    /// Consumes up to `len` bytes from the tail of the buffer.
    ///
    /// If fewer than `len` bytes remain, returns all remaining bytes.
    #[inline]
    pub fn consume_bytes(&mut self, len: usize) -> &'a [u8] {
        let take = self.data.len().min(len);
        let start = self.data.len() - take;
        let slice = &self.data[start..];
        self.data = &self.data[..start];
        slice
    }

    /// Consumes up to `len` bytes from the tail as an owned `Vec<u8>`.
    #[inline]
    pub fn consume_vec(&mut self, len: usize) -> Vec<u8> {
        self.consume_bytes(len).to_vec()
    }

    /// Consumes up to `max_len` bytes from the tail as a UTF-8 string (lossy decoded).
    #[inline]
    pub fn consume_string(&mut self, max_len: usize) -> String {
        let bytes = self.consume_bytes(max_len);
        String::from_utf8_lossy(bytes).into_owned()
    }

    /// Consumes all remaining bytes as the continuous prefix payload.
    ///
    /// Leaves the producer in an empty state (`remaining_bytes() == 0`).
    #[inline]
    pub fn reserve_data_prefix(&mut self) -> &'a [u8] {
        let prefix = self.data;
        self.data = &[];
        prefix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reverse_consumption_order() {
        let raw = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let mut producer = FuzzDataProducer::new(&raw);

        assert_eq!(producer.remaining_bytes(), 10);
        assert!(!producer.is_empty());

        let last = producer.consume_u8();
        assert_eq!(last, 10);
        assert_eq!(producer.remaining_bytes(), 9);

        let u16_val = producer.consume_u16();
        assert_eq!(u16_val, u16::from_le_bytes([8, 9]));
        assert_eq!(producer.remaining_bytes(), 7);

        let u32_val = producer.consume_u32();
        assert_eq!(u32_val, u32::from_le_bytes([4, 5, 6, 7]));
        assert_eq!(producer.remaining_bytes(), 3);

        let prefix = producer.reserve_data_prefix();
        assert_eq!(prefix, &[1, 2, 3]);
        assert_eq!(producer.remaining_bytes(), 0);
        assert!(producer.is_empty());
    }

    #[test]
    fn test_range_clamping() {
        let raw = [0x55, 0xAA, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
        let mut producer = FuzzDataProducer::new(&raw);

        let u32_val = producer.consume_u32_range(1, 9);
        assert!((1..=9).contains(&u32_val));

        let usize_val = producer.consume_usize_range(512, 65536);
        assert!((512..=65536).contains(&usize_val));

        let invalid_range = producer.consume_u32_range(100, 50);
        assert_eq!(invalid_range, 100);
    }

    #[test]
    fn test_safe_degradation_on_empty() {
        let raw: [u8; 0] = [];
        let mut producer = FuzzDataProducer::new(&raw);

        assert_eq!(producer.consume_u8(), 0);
        assert_eq!(producer.consume_u16(), 0);
        assert_eq!(producer.consume_u32(), 0);
        assert_eq!(producer.consume_u64(), 0);
        assert!(!producer.consume_bool());
        assert_eq!(producer.consume_u32_range(5, 10), 5);
        assert_eq!(producer.consume_usize_range(100, 200), 100);
        assert!(producer.consume_bytes(10).is_empty());
        assert!(producer.reserve_data_prefix().is_empty());
    }

    #[test]
    fn test_partial_bytes_degradation() {
        let raw = [0x42u8, 0x13];
        let mut producer = FuzzDataProducer::new(&raw);

        let u32_val = producer.consume_u32();
        assert_eq!(u32_val, u32::from_le_bytes([0x42, 0x13, 0, 0]));
        assert!(producer.is_empty());
    }

    #[test]
    fn test_string_and_bytes_extraction() {
        let raw = b"PayloadHeaderHereSecretPassword123";
        let mut producer = FuzzDataProducer::new(raw);

        let pass = producer.consume_string(11);
        assert_eq!(pass, "Password123");

        let secret = producer.consume_string(6);
        assert_eq!(secret, "Secret");

        let prefix = producer.reserve_data_prefix();
        assert_eq!(prefix, b"PayloadHeaderHere");
    }
}
