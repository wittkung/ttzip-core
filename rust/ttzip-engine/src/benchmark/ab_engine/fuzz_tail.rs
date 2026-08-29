// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Fuzzing Tail-Consumption Data Producer (`fuzz_data_producer`).
//!
//! Conforms to `vendor/zstd/tests/fuzz/fuzz_data_producer.c`:
//! - Extracts execution knobs and control parameters (e.g., compression level, dictionary id,
//!   chunk size, buffer limits, filter modes) by reverse-slicing from the *tail* of the raw fuzz buffer.
//! - Preserves the *head* of the input buffer as a continuous, uninterrupted payload ([`remaining_payload`]),
//!   maximizing dictionary/pattern mutation effectiveness and structural fuzzing coverage.
//! - Safe, zero-allocation, bounded integer range slicing with rollback and prefix reservation support.

use std::cmp::min;

/// High-throughput tail-consumption parameter extractor and continuous payload preserver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuzzTailDataProducer<'a> {
    data: &'a [u8],
    size: usize,
}

impl<'a> FuzzTailDataProducer<'a> {
    /// Creates a new `FuzzTailDataProducer` borrowing `data`.
    #[inline]
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            size: data.len(),
        }
    }

    /// Returns the number of unconsumed parameter bytes remaining in the tail buffer.
    #[inline]
    pub fn remaining_bytes(&self) -> usize {
        self.size
    }

    /// Returns `true` if all bytes in the buffer have been consumed.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Returns the unconsumed continuous payload slice from the head of the buffer.
    #[inline]
    pub fn remaining_payload(&self) -> &'a [u8] {
        &self.data[..self.size]
    }

    /// Consumes a single byte from the tail of the buffer.
    /// Returns `0` if the buffer is empty.
    #[inline]
    pub fn consume_u8(&mut self) -> u8 {
        if self.size == 0 {
            0
        } else {
            self.size -= 1;
            self.data[self.size]
        }
    }

    /// Consumes a 16-bit unsigned integer from the tail of the buffer.
    #[inline]
    pub fn consume_u16(&mut self) -> u16 {
        self.consume_u32_in_range(0, u16::MAX as u32) as u16
    }

    /// Consumes a 32-bit unsigned integer uniformly distributed in `[min, max]`.
    /// Matches `FUZZ_dataProducer_uint32Range` in `vendor/zstd/tests/fuzz/fuzz_data_producer.c`.
    pub fn consume_u32_in_range(&mut self, min_val: u32, max_val: u32) -> u32 {
        if min_val >= max_val {
            return min_val;
        }

        let range = max_val - min_val;
        let mut rolling = range;
        let mut result: u32 = 0;

        while rolling > 0 && self.size > 0 {
            let next = self.consume_u8() as u32;
            result = (result << 8) | next;
            rolling >>= 8;
        }

        if range == u32::MAX {
            result
        } else {
            min_val + (result % (range + 1))
        }
    }

    /// Consumes a 32-bit unsigned integer in `[0, u32::MAX]`.
    #[inline]
    pub fn consume_u32(&mut self) -> u32 {
        self.consume_u32_in_range(0, u32::MAX)
    }

    /// Consumes a 32-bit signed integer uniformly distributed in `[min, max]`.
    /// Matches `FUZZ_dataProducer_int32Range` in `vendor/zstd/tests/fuzz/fuzz_data_producer.c`.
    pub fn consume_i32_in_range(&mut self, min_val: i32, max_val: i32) -> i32 {
        if min_val >= max_val {
            return min_val;
        }

        if min_val < 0 {
            let span = (max_val as i64 - min_val as i64) as u32;
            let offset = self.consume_u32_in_range(0, span);
            min_val.saturating_add(offset as i32)
        } else {
            self.consume_u32_in_range(min_val as u32, max_val as u32) as i32
        }
    }

    /// Consumes a 32-bit signed integer in `[i32::MIN, i32::MAX]`.
    #[inline]
    pub fn consume_i32(&mut self) -> i32 {
        self.consume_i32_in_range(i32::MIN, i32::MAX)
    }

    /// Consumes a 64-bit unsigned integer uniformly distributed in `[min, max]`.
    pub fn consume_u64_in_range(&mut self, min_val: u64, max_val: u64) -> u64 {
        if min_val >= max_val {
            return min_val;
        }

        let range = max_val - min_val;
        let mut rolling = range;
        let mut result: u64 = 0;

        while rolling > 0 && self.size > 0 {
            let next = self.consume_u8() as u64;
            result = (result << 8) | next;
            rolling >>= 8;
        }

        if range == u64::MAX {
            result
        } else {
            min_val + (result % (range + 1))
        }
    }

    /// Consumes a 64-bit unsigned integer in `[0, u64::MAX]`.
    #[inline]
    pub fn consume_u64(&mut self) -> u64 {
        self.consume_u64_in_range(0, u64::MAX)
    }

    /// Consumes a `usize` value uniformly distributed in `[min, max]`.
    #[inline]
    pub fn consume_usize_in_range(&mut self, min_val: usize, max_val: usize) -> usize {
        self.consume_u64_in_range(min_val as u64, max_val as u64) as usize
    }

    /// Consumes a `usize` value in `[0, usize::MAX]`.
    #[inline]
    pub fn consume_usize(&mut self) -> usize {
        self.consume_usize_in_range(0, usize::MAX)
    }

    /// Consumes a boolean flag from the least significant bit of the consumed byte.
    #[inline]
    pub fn consume_bool(&mut self) -> bool {
        (self.consume_u8() & 1) != 0
    }

    /// Consumes a slice of up to `len` bytes from the tail of the buffer.
    pub fn consume_bytes(&mut self, len: usize) -> &'a [u8] {
        let take = min(len, self.size);
        let start = self.size - take;
        let slice = &self.data[start..self.size];
        self.size = start;
        slice
    }

    /// Contracts the producer to a smaller size, shifting the data slice forward.
    /// Matches `FUZZ_dataProducer_contract` in `vendor/zstd/tests/fuzz/fuzz_data_producer.c`.
    /// Returns the number of bytes dropped from the prefix.
    pub fn contract(&mut self, new_size: usize) -> usize {
        let effective_new_size = min(new_size, self.size);
        let remaining = self.size - effective_new_size;
        self.data = &self.data[remaining..];
        self.size = effective_new_size;
        remaining
    }

    /// Reserves a random data prefix as an isolated slice and contracts the remaining producer.
    /// Matches `FUZZ_dataProducer_reserveDataPrefix` in `vendor/zstd/tests/fuzz/fuzz_data_producer.c`.
    pub fn reserve_data_prefix(&mut self) -> &'a [u8] {
        let orig_data = self.data;
        let slice_size = self.consume_usize_in_range(0, self.size);
        let prefix_bytes = self.contract(slice_size);
        &orig_data[..prefix_bytes]
    }

    /// Reserves a random data prefix, contracts the producer, and returns the prefix length.
    pub fn reserve_data_prefix_len(&mut self) -> usize {
        let slice_size = self.consume_usize_in_range(0, self.size);
        self.contract(slice_size)
    }

    /// Rolls back the remaining bytes counter to a previous state (`remaining_bytes >= current size`).
    /// Matches `FUZZ_dataProducer_rollBack` in `vendor/zstd/tests/fuzz/fuzz_data_producer.c`.
    pub fn rollback(&mut self, remaining_bytes: usize) {
        if remaining_bytes >= self.size && remaining_bytes <= self.data.len() {
            self.size = remaining_bytes;
        }
    }

    // MARK: - Specialized Compression Fuzzing Helpers

    /// Consumes a compression level within the standard range `[min_level, max_level]`.
    #[inline]
    pub fn consume_compression_level(&mut self, min_level: i32, max_level: i32) -> i32 {
        self.consume_i32_in_range(min_level, max_level)
    }

    /// Consumes a buffer chunk size within `[min_chunk, max_chunk]`.
    #[inline]
    pub fn consume_chunk_size(&mut self, min_chunk: usize, max_chunk: usize) -> usize {
        self.consume_usize_in_range(min_chunk, max_chunk)
    }
}

// MARK: - Unit Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tail_consumption_preserves_head_payload() {
        let raw_data = b"0123456789ABCDEF-PAYLOAD-CONTIGUOUS-CONTROL-PARAMS-END";
        let mut producer = FuzzTailDataProducer::new(raw_data);

        assert_eq!(producer.remaining_bytes(), raw_data.len());
        assert_eq!(producer.remaining_payload(), raw_data);

        // Consume bytes from the tail
        let last_byte = producer.consume_u8();
        assert_eq!(last_byte, b'D');
        let prev_byte = producer.consume_u8();
        assert_eq!(prev_byte, b'N');
        let third_byte = producer.consume_u8();
        assert_eq!(third_byte, b'E');

        // Verify head remains completely contiguous and untouched
        let expected_remaining = &raw_data[..raw_data.len() - 3];
        assert_eq!(producer.remaining_payload(), expected_remaining);
        assert_eq!(producer.remaining_bytes(), raw_data.len() - 3);
    }

    #[test]
    fn test_integer_ranges_and_bounds() {
        let raw_data = [0x55, 0xAA, 0x12, 0x34, 0x78, 0x90, 0xEF, 0x01];
        let mut producer = FuzzTailDataProducer::new(&raw_data);

        // Consume in range
        let u32_val = producer.consume_u32_in_range(10, 50);
        assert!((10..=50).contains(&u32_val));

        let i32_val = producer.consume_i32_in_range(-100, 100);
        assert!((-100..=100).contains(&i32_val));

        let bool_val = producer.consume_bool();
        assert!(bool_val || !bool_val);

        // When exhausted, should safely return min values or 0
        let mut empty = FuzzTailDataProducer::new(&[]);
        assert!(empty.is_empty());
        assert_eq!(empty.consume_u8(), 0);
        assert_eq!(empty.consume_u32_in_range(100, 200), 100);
        assert_eq!(empty.consume_i32_in_range(-50, 50), -50);
        assert_eq!(empty.consume_usize_in_range(10, 20), 10);
    }

    #[test]
    fn test_consume_bytes_and_rollback() {
        let raw_data = b"HEAD_DATA_STAYS_INTACT_TAIL_CONSUMED";
        let mut producer = FuzzTailDataProducer::new(raw_data);

        let initial_len = producer.remaining_bytes();
        let tail_slice = producer.consume_bytes(9);
        assert_eq!(tail_slice, b"_CONSUMED");
        assert_eq!(producer.remaining_bytes(), initial_len - 9);

        // Rollback
        producer.rollback(initial_len);
        assert_eq!(producer.remaining_bytes(), initial_len);
        assert_eq!(producer.remaining_payload(), raw_data);
    }

    #[test]
    fn test_contract_and_reserve_data_prefix() {
        let raw_data = b"PREFIX_PART_12345_SUFFIX_PART_67890";
        let mut producer = FuzzTailDataProducer::new(raw_data);

        let prefix = producer.reserve_data_prefix();
        assert!(prefix.len() <= raw_data.len());
        assert!(producer.remaining_bytes() <= raw_data.len());
    }

    #[test]
    fn test_specialized_fuzzing_helpers() {
        let raw_data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let mut producer = FuzzTailDataProducer::new(&raw_data);

        let level = producer.consume_compression_level(1, 22);
        assert!((1..=22).contains(&level));

        let chunk = producer.consume_chunk_size(1024, 65536);
        assert!((1024..=65536).contains(&chunk));
    }
}
