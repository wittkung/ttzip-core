// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zero-branch bitstream accumulator (BitWriter) and fast register BitReader.
//!
//! Designed for high-throughput LSB-first bitstream packing/unpacking in Deflate, Brotli,
//! and custom entropy encoders.

/// High-performance LSB-first bitstream writer with a wide register accumulator.
#[derive(Debug, Clone, Default)]
pub struct BitWriter {
    buf: Vec<u8>,
    bitbuf: u128,
    bitcount: u32,
}

impl BitWriter {
    /// Creates a new empty `BitWriter`.
    pub fn new() -> Self {
        Self {
            buf: Vec::new(),
            bitbuf: 0,
            bitcount: 0,
        }
    }

    /// Creates a new `BitWriter` with pre-allocated buffer capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
            bitbuf: 0,
            bitcount: 0,
        }
    }

    /// Writes `nbits` (1..=64) of `value` into the bitstream (LSB first).
    #[inline]
    pub fn write_bits(&mut self, value: u64, nbits: u32) {
        if nbits == 0 {
            return;
        }
        let nbits = nbits.min(64);
        let mask = if nbits >= 64 { !0u64 } else { (1u64 << nbits) - 1 };
        self.bitbuf |= ((value & mask) as u128) << self.bitcount;
        self.bitcount += nbits;

        if self.bitcount >= 64 {
            self.flush_bits();
        }
    }

    /// Flushes all complete bytes from the accumulator into the buffer
    /// using unaligned 64-bit wide writes.
    #[inline]
    pub fn flush_bits(&mut self) {
        let bytes = (self.bitcount >> 3) as usize;
        if bytes > 0 {
            let old_len = self.buf.len();
            self.buf.reserve(bytes + 8);
            unsafe {
                let ptr = self.buf.as_mut_ptr().add(old_len);
                std::ptr::write_unaligned(ptr as *mut u64, (self.bitbuf as u64).to_le());
                if bytes > 8 {
                    std::ptr::write_unaligned(
                        ptr.add(8) as *mut u64,
                        ((self.bitbuf >> 64) as u64).to_le(),
                    );
                }
                self.buf.set_len(old_len + bytes);
            }
            let shift = (bytes * 8) as u32;
            if shift < 128 {
                self.bitbuf >>= shift;
            } else {
                self.bitbuf = 0;
            }
            self.bitcount -= (bytes * 8) as u32;
        }
    }

    /// Pads the bitstream with zeros up to the next byte boundary and flushes all pending bytes.
    #[inline]
    pub fn flush_to_byte_boundary(&mut self) {
        let remainder = self.bitcount & 7;
        if remainder != 0 {
            self.bitcount += 8 - remainder;
        }
        self.flush_bits();
    }

    /// Finalizes the bitstream, aligning to byte boundary and returning the underlying buffer.
    pub fn finish(mut self) -> Vec<u8> {
        self.flush_to_byte_boundary();
        self.buf
    }

    /// Returns a slice of the already flushed bytes.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    /// Returns the number of flushed bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Returns true if no bits have been written.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty() && self.bitcount == 0
    }

    /// Returns the number of uncommitted bits currently in the accumulator.
    #[inline]
    pub fn uncommitted_bits(&self) -> u32 {
        self.bitcount
    }

    /// Returns total bits written (flushed bytes * 8 + uncommitted bits).
    #[inline]
    pub fn total_bits_written(&self) -> usize {
        self.buf.len() * 8 + self.bitcount as usize
    }
}

/// High-throughput LSB-first bitstream reader with register fast refill.
#[derive(Debug, Clone)]
pub struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    bitbuf: u128,
    bitcount: u32,
}

impl<'a> BitReader<'a> {
    /// Creates a new `BitReader` over a byte slice.
    pub fn new(data: &'a [u8]) -> Self {
        let mut reader = Self {
            data,
            pos: 0,
            bitbuf: 0,
            bitcount: 0,
        };
        reader.refill_bits();
        reader
    }

    /// Refills the register from the input buffer.
    #[inline]
    pub fn refill_bits(&mut self) {
        while self.bitcount <= 64 && self.pos < self.data.len() {
            let remaining = self.data.len() - self.pos;
            if remaining >= 8 {
                unsafe {
                    let ptr = self.data.as_ptr().add(self.pos);
                    let raw = u64::from_le(std::ptr::read_unaligned(ptr as *const u64));
                    self.bitbuf |= (raw as u128) << self.bitcount;
                    self.bitcount += 64;
                    self.pos += 8;
                }
            } else {
                let byte = self.data[self.pos] as u128;
                self.bitbuf |= byte << self.bitcount;
                self.bitcount += 8;
                self.pos += 1;
            }
        }
    }

    /// Peeks `nbits` (1..=64) from the bitstream without consuming them.
    #[inline]
    pub fn peek_bits(&mut self, nbits: u32) -> Option<u64> {
        if nbits == 0 {
            return Some(0);
        }
        if nbits > 64 {
            return None;
        }
        if self.bitcount < nbits {
            self.refill_bits();
            if self.bitcount < nbits {
                return None;
            }
        }
        let mask = if nbits >= 64 { !0u64 } else { (1u64 << nbits) - 1 };
        Some((self.bitbuf as u64) & mask)
    }

    /// Consumes `nbits` from the bitstream.
    #[inline]
    pub fn consume_bits(&mut self, nbits: u32) {
        let to_consume = nbits.min(self.bitcount);
        if to_consume >= 128 {
            self.bitbuf = 0;
            self.bitcount = 0;
        } else {
            self.bitbuf >>= to_consume;
            self.bitcount -= to_consume;
        }
    }

    /// Reads `nbits` (1..=64) from the bitstream, advancing the cursor.
    #[inline]
    pub fn read_bits(&mut self, nbits: u32) -> Option<u64> {
        let val = self.peek_bits(nbits)?;
        self.consume_bits(nbits);
        Some(val)
    }

    /// Discards bits until aligned to the next byte boundary.
    #[inline]
    pub fn align_to_byte(&mut self) {
        let remainder = self.bitcount & 7;
        if remainder != 0 {
            self.consume_bits(remainder);
        }
    }

    /// Returns the total estimated number of remaining bits in the stream.
    #[inline]
    pub fn bits_remaining(&self) -> usize {
        self.bitcount as usize + (self.data.len() - self.pos) * 8
    }

    /// Returns true if no bits remain to be read.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bits_remaining() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bit_writer_single_bits() {
        let mut writer = BitWriter::new();
        let bits = [1, 0, 1, 1, 0, 0, 1, 0];
        for &b in &bits {
            writer.write_bits(b, 1);
        }
        let bytes = writer.finish();
        assert_eq!(bytes.len(), 1);
        assert_eq!(bytes[0], 0b01001101);
    }

    #[test]
    fn test_bit_writer_flush_to_byte_boundary() {
        let mut writer = BitWriter::new();
        writer.write_bits(0x05, 3); // 101 in binary
        let bytes = writer.finish();
        assert_eq!(bytes.len(), 1);
        assert_eq!(bytes[0], 0x05);
    }

    #[test]
    fn test_bit_writer_reader_roundtrip_variable_sizes() {
        let mut writer = BitWriter::new();
        let values: Vec<(u64, u32)> = vec![
            (0x1, 1),
            (0x3, 2),
            (0xA, 4),
            (0x1F, 5),
            (0xABCD, 16),
            (0x12345678, 32),
            (0xFEDCBA9876543210, 64),
            (0x3F, 6),
            (0x7FF, 11),
        ];

        for &(val, bits) in &values {
            writer.write_bits(val, bits);
        }

        let encoded = writer.finish();
        let mut reader = BitReader::new(&encoded);

        for &(expected, bits) in &values {
            let actual = reader.read_bits(bits).expect("Should read bits successfully");
            let mask = if bits >= 64 { !0 } else { (1u64 << bits) - 1 };
            assert_eq!(actual, expected & mask, "Mismatch for {} bits read", bits);
        }
    }

    #[test]
    fn test_bit_reader_peek_and_consume() {
        let mut writer = BitWriter::new();
        writer.write_bits(0x42, 8);
        writer.write_bits(0x99, 8);
        let encoded = writer.finish();

        let mut reader = BitReader::new(&encoded);
        assert_eq!(reader.peek_bits(8), Some(0x42));
        assert_eq!(reader.peek_bits(8), Some(0x42));
        reader.consume_bits(8);
        assert_eq!(reader.peek_bits(8), Some(0x99));
        assert_eq!(reader.read_bits(8), Some(0x99));
        assert_eq!(reader.read_bits(1), None);
    }

    #[test]
    fn test_bit_reader_align_to_byte() {
        let mut writer = BitWriter::new();
        writer.write_bits(0x7, 3);
        writer.flush_to_byte_boundary();
        writer.write_bits(0xAA, 8);
        let encoded = writer.finish();

        let mut reader = BitReader::new(&encoded);
        assert_eq!(reader.read_bits(3), Some(0x7));
        reader.align_to_byte();
        assert_eq!(reader.read_bits(8), Some(0xAA));
    }

    #[test]
    fn test_bitstream_large_pseudorandom_roundtrip() {
        let mut writer = BitWriter::new();
        let mut expected = Vec::new();
        let mut state: u64 = 0x123456789ABCDEF0;

        for _ in 0..1000 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let bits = ((state % 64) + 1) as u32;
            let val = state;
            let mask = if bits >= 64 { !0 } else { (1u64 << bits) - 1 };
            let masked_val = val & mask;
            expected.push((masked_val, bits));
            writer.write_bits(masked_val, bits);
        }

        let encoded = writer.finish();
        let mut reader = BitReader::new(&encoded);

        for (val, bits) in expected {
            let actual = reader.read_bits(bits).expect("Stream must not run out of bits");
            assert_eq!(actual, val);
        }
    }
}
