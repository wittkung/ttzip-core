// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! BCJ2 4-Stream Lock-Free Micro-Buffered Streaming Arbitrator and Decoder.
//!
//! Implements the 7-Zip BCJ2 4-In-1-Out executable pre-filter decompression pipeline
//! with dedicated 64KB micro-buffers per input channel, ensuring total resident memory
//! is strictly locked to $\le 256\text{KB}$.

use super::range_coder::{BIT_MODEL_TOTAL, NUM_BCJ2_PROBS, NUM_BIT_MODEL_TOTAL_BITS, NUM_MOVE_BITS, PROB_INIT_VAL};
use std::io::{self, Read};

/// Micro-buffer size allocated per stream channel (64 KB).
pub const MICRO_BUFFER_SIZE: usize = 64 * 1024;

/// Dedicated 64KB heap-allocated ring/shift micro-buffer for a single streaming source.
pub struct MicroBuffer<R> {
    inner: R,
    buf: Box<[u8; MICRO_BUFFER_SIZE]>,
    pos: usize,
    len: usize,
    eof_reached: bool,
}

impl<R> MicroBuffer<R> {
    /// Creates a new `MicroBuffer` wrapping an input stream.
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            buf: Box::new([0u8; MICRO_BUFFER_SIZE]),
            pos: 0,
            len: 0,
            eof_reached: false,
        }
    }

    /// Returns the number of unconsumed bytes currently held in the micro-buffer.
    #[inline]
    #[must_use]
    pub const fn available(&self) -> usize {
        self.len - self.pos
    }

    /// Returns true if the underlying reader has reported EOF and all buffered bytes have been consumed.
    #[inline]
    #[must_use]
    pub const fn is_eof(&self) -> bool {
        self.eof_reached && self.pos >= self.len
    }
}

impl<R: Read> MicroBuffer<R> {
    /// Refills the micro-buffer from the underlying reader after shifting unread bytes to index 0.
    pub fn refill(&mut self) -> io::Result<bool> {
        if self.pos > 0 && self.pos < self.len {
            self.buf.copy_within(self.pos..self.len, 0);
            self.len -= self.pos;
            self.pos = 0;
        } else if self.pos >= self.len {
            self.pos = 0;
            self.len = 0;
        }

        if self.len >= MICRO_BUFFER_SIZE || self.eof_reached {
            return Ok(!self.is_eof());
        }

        let n = self.inner.read(&mut self.buf[self.len..])?;
        if n == 0 {
            self.eof_reached = true;
        } else {
            self.len += n;
        }

        Ok(!self.is_eof())
    }

    /// Reads a single byte from the micro-buffer, refilling from the inner reader if necessary.
    #[inline]
    pub fn read_byte(&mut self) -> io::Result<Option<u8>> {
        if self.pos >= self.len {
            if !self.refill()? {
                return Ok(None);
            }
            if self.pos >= self.len {
                return Ok(None);
            }
        }
        let b = self.buf[self.pos];
        self.pos += 1;
        Ok(Some(b))
    }

    /// Reads exactly `out.len()` bytes into the given destination slice.
    pub fn read_exact(&mut self, out: &mut [u8]) -> io::Result<()> {
        let mut filled = 0;
        while filled < out.len() {
            if self.pos >= self.len {
                if !self.refill()? {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Premature EOF while reading from MicroBuffer",
                    ));
                }
                if self.pos >= self.len {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Premature EOF while reading from MicroBuffer",
                    ));
                }
            }

            let available = self.len - self.pos;
            let needed = out.len() - filled;
            let to_copy = available.min(needed);
            out[filled..filled + to_copy].copy_from_slice(&self.buf[self.pos..self.pos + to_copy]);
            self.pos += to_copy;
            filled += to_copy;
        }
        Ok(())
    }

    /// Reads a 32-bit unsigned big-endian integer from the micro-buffer.
    #[inline]
    pub fn read_u32_be(&mut self) -> io::Result<u32> {
        let mut bytes = [0u8; 4];
        self.read_exact(&mut bytes)?;
        Ok(u32::from_be_bytes(bytes))
    }
}

/// Adaptive 258-context binary range decoder wrapping a streaming input micro-buffer.
pub struct Bcj2RangeDecoder<R> {
    stream: MicroBuffer<R>,
    range: u32,
    code: u32,
    probs: [u16; NUM_BCJ2_PROBS],
    initialized: bool,
}

impl<R> Bcj2RangeDecoder<R> {
    /// Creates a new uninitialized `Bcj2RangeDecoder` wrapping the stream reader.
    pub fn new(reader: R) -> Self {
        Self {
            stream: MicroBuffer::new(reader),
            range: 0xFFFF_FFFF,
            code: 0,
            probs: [PROB_INIT_VAL; NUM_BCJ2_PROBS],
            initialized: false,
        }
    }
}

impl<R: Read> Bcj2RangeDecoder<R> {
    /// Bootstraps the 5 initial code bytes from the range coder stream.
    fn init(&mut self) -> io::Result<()> {
        for _ in 0..5 {
            let b = self.stream.read_byte()?.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Unexpected EOF while initializing BCJ2 RangeDecoder header",
                )
            })?;
            self.code = (self.code << 8) | (b as u32);
        }
        self.initialized = true;
        Ok(())
    }

    /// Decodes a single decision bit (0 or 1) using the designated probability context index.
    #[inline]
    pub fn decode_bit(&mut self, ctx: usize) -> io::Result<u32> {
        if !self.initialized {
            self.init()?;
        }

        let prob = &mut self.probs[ctx];
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

        while self.range < (1 << 24) {
            self.range <<= 8;
            let b = self.stream.read_byte()?.unwrap_or(0);
            self.code = (self.code << 8) | (b as u32);
        }

        Ok(bit)
    }
}

/// BCJ2 4-Stream Lock-Free Micro-Buffered Streaming Arbitrator.
///
/// Merges four asynchronous input streams (`main_stream`, `call_stream`, `jump_stream`,
/// and `rc_stream`) into a single byte-exact reconstructed executable instruction stream.
///
/// Total resident memory is strictly bounded by $4 \times 64\text{KB} = 256\text{KB}$.
pub struct Bcj2StreamArbitrator<R0, R1, R2, R3> {
    main_stream: MicroBuffer<R0>,
    call_stream: MicroBuffer<R1>,
    jump_stream: MicroBuffer<R2>,
    rc_stream: Bcj2RangeDecoder<R3>,
    prev_byte: u8,
    current_ip: u32,
    uncompressed_limit: u64,
    produced_bytes: u64,
    pending_buf: [u8; 5],
    pending_len: u8,
    pending_pos: u8,
}

impl<R0, R1, R2, R3> Bcj2StreamArbitrator<R0, R1, R2, R3> {
    /// Creates a new `Bcj2StreamArbitrator` with an uncompressed size limit.
    pub fn with_limit(
        main: R0,
        call: R1,
        jump: R2,
        rc: R3,
        base_ip: u32,
        uncompressed_limit: u64,
    ) -> Self {
        Self {
            main_stream: MicroBuffer::new(main),
            call_stream: MicroBuffer::new(call),
            jump_stream: MicroBuffer::new(jump),
            rc_stream: Bcj2RangeDecoder::new(rc),
            prev_byte: 0,
            current_ip: base_ip,
            uncompressed_limit,
            produced_bytes: 0,
            pending_buf: [0u8; 5],
            pending_len: 0,
            pending_pos: 0,
        }
    }

    /// Creates a new `Bcj2StreamArbitrator` with unlimited output length.
    pub fn new(main: R0, call: R1, jump: R2, rc: R3, base_ip: u32) -> Self {
        Self::with_limit(main, call, jump, rc, base_ip, u64::MAX)
    }

    /// Returns the current runtime instruction pointer.
    #[inline]
    #[must_use]
    pub const fn current_ip(&self) -> u32 {
        self.current_ip
    }

    /// Returns the total number of uncompressed bytes produced so far.
    #[inline]
    #[must_use]
    pub const fn produced_bytes(&self) -> u64 {
        self.produced_bytes
    }

    /// Returns the previous output byte used as context for range coder probability selection.
    #[inline]
    #[must_use]
    pub const fn prev_byte(&self) -> u8 {
        self.prev_byte
    }

    /// Returns the configured uncompressed size limit.
    #[inline]
    #[must_use]
    pub const fn uncompressed_limit(&self) -> u64 {
        self.uncompressed_limit
    }
}

impl<R0: Read, R1: Read, R2: Read, R3: Read> Bcj2StreamArbitrator<R0, R1, R2, R3> {
    /// Decompresses and merges a chunk of data into `output`.
    ///
    /// Returns the number of bytes written to `output`.
    pub fn decode_chunk(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() || self.produced_bytes >= self.uncompressed_limit {
            return Ok(0);
        }

        let mut written = 0usize;

        loop {
            // 1. Flush any pending staged bytes from a previously split 5-byte instruction
            while (self.pending_pos as usize) < (self.pending_len as usize)
                && written < output.len()
                && self.produced_bytes < self.uncompressed_limit
            {
                let b = self.pending_buf[self.pending_pos as usize];
                output[written] = b;
                written += 1;
                self.pending_pos += 1;
                self.produced_bytes += 1;
                self.current_ip = self.current_ip.wrapping_add(1);
                self.prev_byte = b;
            }

            if written == output.len() || self.produced_bytes >= self.uncompressed_limit {
                return Ok(written);
            }

            if self.pending_pos >= self.pending_len {
                self.pending_pos = 0;
                self.pending_len = 0;
            }

            // 2. Fetch next byte from Main stream
            let opt_b = self.main_stream.read_byte()?;
            let b = match opt_b {
                Some(byte) => byte,
                None => return Ok(written),
            };

            // 3. Evaluate opcode branch condition
            if b != 0xE8 && b != 0xE9 {
                // Non-branch literal: write directly to output
                output[written] = b;
                written += 1;
                self.produced_bytes += 1;
                self.current_ip = self.current_ip.wrapping_add(1);
                self.prev_byte = b;
                continue;
            }

            // Branch candidate opcode (0xE8 CALL or 0xE9 JMP)
            let ctx = if b == 0xE8 {
                self.prev_byte as usize
            } else {
                256
            };

            let bit = self.rc_stream.decode_bit(ctx)?;
            if bit == 0 {
                // False branch / literal opcode
                output[written] = b;
                written += 1;
                self.produced_bytes += 1;
                self.current_ip = self.current_ip.wrapping_add(1);
                self.prev_byte = b;
            } else {
                // True branch: pull 4-byte big-endian absolute address
                let target_abs = if b == 0xE8 {
                    self.call_stream.read_u32_be()?
                } else {
                    self.jump_stream.read_u32_be()?
                };

                let next_ip = self.current_ip.wrapping_add(5);
                let offset_rel = target_abs.wrapping_sub(next_ip);
                let rel_le = offset_rel.to_le_bytes();

                self.pending_buf[0] = b;
                self.pending_buf[1] = rel_le[0];
                self.pending_buf[2] = rel_le[1];
                self.pending_buf[3] = rel_le[2];
                self.pending_buf[4] = rel_le[3];
                self.pending_len = 5;
                self.pending_pos = 0;
            }
        }
    }
}

impl<R0: Read, R1: Read, R2: Read, R3: Read> Read for Bcj2StreamArbitrator<R0, R1, R2, R3> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.decode_chunk(buf)
    }
}
