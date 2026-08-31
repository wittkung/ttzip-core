// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! BCJ2 4-Stream Zero-Deadlock Demand-Driven Streaming Arbitrator and Decoder.
//!
//! Implements the 7-Zip BCJ2 4-In-1-Out executable pre-filter decompression pipeline
//! with a fine-grained state machine (`Bcj2State`), 258-context adaptive binary
//! range decoding, and demand-driven pulling across four asynchronous streams:
//! 1. `StreamMain` (Stream 0): Opcode bytes, literals, and branch indicators.
//! 2. `StreamCall` (Stream 1): 32-bit big-endian absolute targets for CALL (`0xE8`).
//! 3. `StreamJump` (Stream 2): 32-bit big-endian absolute targets for JMP (`0xE9`).
//! 4. `StreamRc`   (Stream 3): Adaptive range coder status bitstream.
//!
//! Total resident memory in streaming mode is strictly bounded by $4 \times 64\text{KB} = 256\text{KB}$.

use crate::types::TTZipStatus;
use std::io::{self, Read, Write};

/// Number of precision bits for binary range coder probability models.
pub const NUM_BIT_MODEL_TOTAL_BITS: usize = 11;
/// Total probability weight (2^11 = 2048).
pub const BIT_MODEL_TOTAL: u32 = 1 << NUM_BIT_MODEL_TOTAL_BITS;
/// Adaptation rate shift parameter.
pub const NUM_MOVE_BITS: usize = 5;
/// Initial probability value representing 50% likelihood (1024).
pub const PROB_INIT_VAL: u16 = (BIT_MODEL_TOTAL / 2) as u16;
/// Total number of probability contexts for BCJ2 (256 for 0xE8 + 2 for 0xE9/0x0F).
pub const NUM_BCJ2_PROBS: usize = 258;

/// Dedicated micro-buffer size per input stream in streaming reader (64 KB).
pub const BCJ2_STREAM_BUFFER_SIZE: usize = 64 * 1024;
/// Total number of input streams in BCJ2 topology.
pub const NUM_STREAMS: usize = 4;

/// Channel identifiers for the 4 BCJ2 input streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum Bcj2StreamId {
    /// Stream 0: Literals and opcode indicators.
    StreamMain = 0,
    /// Stream 1: 32-bit big-endian absolute targets for CALL (0xE8) instructions.
    StreamCall = 1,
    /// Stream 2: 32-bit big-endian absolute targets for JMP (0xE9) instructions.
    StreamJump = 2,
    /// Stream 3: Range coder status bitstream.
    StreamRc = 3,
}

impl Bcj2StreamId {
    /// Converts a zero-based stream index (0..3) to `Bcj2StreamId`.
    #[inline]
    #[must_use]
    pub const fn from_index(idx: usize) -> Option<Self> {
        match idx {
            0 => Some(Self::StreamMain),
            1 => Some(Self::StreamCall),
            2 => Some(Self::StreamJump),
            3 => Some(Self::StreamRc),
            _ => None,
        }
    }

    /// Returns the zero-based numeric index of the stream channel.
    #[inline]
    #[must_use]
    pub const fn as_index(self) -> usize {
        self as usize
    }
}

/// Range decoder streaming state machine core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bcj2RangeState {
    /// Current range interval bound.
    pub range: u32,
    /// Current code point accumulator.
    pub code: u32,
}

impl Default for Bcj2RangeState {
    fn default() -> Self {
        Self {
            range: 0xFFFF_FFFF,
            code: 0,
        }
    }
}

/// Fine-grained state machine governing the BCJ2 stream arbitration cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bcj2State {
    /// Reading 5 initial bytes from StreamRc to bootstrap range decoder.
    InitRc { bytes_read: u8 },
    /// Ready for next byte from StreamMain.
    FetchMain,
    /// Renormalizing range coder after evaluating branch decision bit.
    RenormalizeRc { opcode: u8, bit: u8 },
    /// Reading 4-byte big-endian absolute destination from Call or Jump stream.
    FetchDestination { opcode: u8, bytes_read: u8 },
    /// Flushing output bytes (literal byte or opcode + 4-byte displacement).
    FlushOutput { out_len: u8, out_pos: u8 },
    /// StreamMain reached EOF and all decoded data has been flushed.
    Finished,
}

/// Non-blocking arbitration status returned by `Bcj2StreamArbitrator::process`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bcj2ArbitratorStatus {
    /// Arbitrator requires more input from the designated stream channel.
    NeedsMoreInput(Bcj2StreamId),
    /// Arbitrator requires more writable space in the output buffer.
    NeedsMoreOutput,
    /// All streams have finished and decompressed output is fully flushed.
    Finished,
}

/// BCJ2 4-In-1-Out Zero-Deadlock Streaming Arbitrator.
///
/// Operates on arbitrary chunk slices and guarantees lossless suspension and
/// resumption across single-byte boundaries without heap allocations.
#[derive(Debug, Clone)]
pub struct Bcj2StreamArbitrator {
    /// Base instruction pointer / program counter.
    ip: u64,
    /// 258 probability models for range decoding.
    probs: [u16; NUM_BCJ2_PROBS],
    /// Range decoder core register state.
    rc_state: Bcj2RangeState,
    /// Previous emitted byte for context index selection.
    prev_byte: u8,
    /// Current arbitrator lifecycle state.
    state: Bcj2State,
    /// Assembled 4-byte branch destination.
    dest_buf: [u8; 4],
    /// Pending output buffer (up to 5 bytes for opcode + rel32 displacement).
    out_buf: [u8; 5],
    /// Total decompressed bytes produced.
    total_out: u64,
}

impl Default for Bcj2StreamArbitrator {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Bcj2StreamArbitrator {
    /// Creates a new arbitrator starting at instruction pointer `ip`.
    #[must_use]
    pub fn new(ip: u64) -> Self {
        Self {
            ip,
            probs: [PROB_INIT_VAL; NUM_BCJ2_PROBS],
            rc_state: Bcj2RangeState::default(),
            prev_byte: 0,
            state: Bcj2State::InitRc { bytes_read: 0 },
            dest_buf: [0; 4],
            out_buf: [0; 5],
            total_out: 0,
        }
    }

    /// Resets the arbitrator to initial state with a new instruction pointer.
    pub fn reset(&mut self, ip: u64) {
        self.ip = ip;
        self.probs = [PROB_INIT_VAL; NUM_BCJ2_PROBS];
        self.rc_state = Bcj2RangeState::default();
        self.prev_byte = 0;
        self.state = Bcj2State::InitRc { bytes_read: 0 };
        self.dest_buf = [0; 4];
        self.out_buf = [0; 5];
        self.total_out = 0;
    }

    /// Returns the current lifecycle state of the arbitrator.
    #[inline]
    #[must_use]
    pub const fn state(&self) -> Bcj2State {
        self.state
    }

    /// Returns the current instruction pointer.
    #[inline]
    #[must_use]
    pub const fn current_ip(&self) -> u64 {
        self.ip
    }

    /// Returns total decompressed bytes produced.
    #[inline]
    #[must_use]
    pub const fn total_out(&self) -> u64 {
        self.total_out
    }

    /// Processes slices from the 4 input channels and writes decompressed bytes to `out`.
    ///
    /// Slices are advanced in-place as data is consumed or produced.
    pub fn process(
        &mut self,
        main: &mut &[u8],
        call: &mut &[u8],
        jump: &mut &[u8],
        rc: &mut &[u8],
        out: &mut &mut [u8],
        main_is_eof: bool,
    ) -> Result<Bcj2ArbitratorStatus, TTZipStatus> {
        loop {
            match self.state {
                Bcj2State::InitRc { mut bytes_read } => {
                    while bytes_read < 5 {
                        if rc.is_empty() {
                            self.state = Bcj2State::InitRc { bytes_read };
                            return Ok(Bcj2ArbitratorStatus::NeedsMoreInput(Bcj2StreamId::StreamRc));
                        }
                        let b = rc[0];
                        *rc = &rc[1..];
                        self.rc_state.code = (self.rc_state.code << 8) | (b as u32);
                        bytes_read += 1;
                    }
                    self.state = Bcj2State::FetchMain;
                }
                Bcj2State::FetchMain => {
                    if main.is_empty() {
                        if main_is_eof {
                            self.state = Bcj2State::Finished;
                            return Ok(Bcj2ArbitratorStatus::Finished);
                        }
                        return Ok(Bcj2ArbitratorStatus::NeedsMoreInput(Bcj2StreamId::StreamMain));
                    }
                    let b = main[0];
                    *main = &main[1..];

                    if b == 0xE8 || b == 0xE9 {
                        let ctx = if b == 0xE8 {
                            self.prev_byte as usize
                        } else if self.prev_byte == 0x0F {
                            257
                        } else {
                            256
                        };

                        let bound = (self.rc_state.range >> NUM_BIT_MODEL_TOTAL_BITS)
                            * (self.probs[ctx] as u32);

                        let bit = if self.rc_state.code < bound {
                            self.rc_state.range = bound;
                            self.probs[ctx] +=
                                (BIT_MODEL_TOTAL as u16 - self.probs[ctx]) >> NUM_MOVE_BITS;
                            0
                        } else {
                            self.rc_state.range -= bound;
                            self.rc_state.code -= bound;
                            self.probs[ctx] -= self.probs[ctx] >> NUM_MOVE_BITS;
                            1
                        };

                        self.state = Bcj2State::RenormalizeRc { opcode: b, bit };
                    } else {
                        self.out_buf[0] = b;
                        self.state = Bcj2State::FlushOutput {
                            out_len: 1,
                            out_pos: 0,
                        };
                    }
                }
                Bcj2State::RenormalizeRc { opcode, bit } => {
                    while self.rc_state.range < (1 << 24) {
                        if rc.is_empty() {
                            return Ok(Bcj2ArbitratorStatus::NeedsMoreInput(Bcj2StreamId::StreamRc));
                        }
                        let b = rc[0];
                        *rc = &rc[1..];
                        self.rc_state.range <<= 8;
                        self.rc_state.code = (self.rc_state.code << 8) | (b as u32);
                    }

                    if bit == 0 {
                        self.out_buf[0] = opcode;
                        self.state = Bcj2State::FlushOutput {
                            out_len: 1,
                            out_pos: 0,
                        };
                    } else {
                        self.dest_buf = [0; 4];
                        self.state = Bcj2State::FetchDestination {
                            opcode,
                            bytes_read: 0,
                        };
                    }
                }
                Bcj2State::FetchDestination {
                    opcode,
                    mut bytes_read,
                } => {
                    let stream_id = if opcode == 0xE8 {
                        Bcj2StreamId::StreamCall
                    } else {
                        Bcj2StreamId::StreamJump
                    };

                    while bytes_read < 4 {
                        let (is_empty, b) = if opcode == 0xE8 {
                            if call.is_empty() {
                                (true, 0)
                            } else {
                                let byte = (*call)[0];
                                *call = &(*call)[1..];
                                (false, byte)
                            }
                        } else if jump.is_empty() {
                            (true, 0)
                        } else {
                            let byte = (*jump)[0];
                            *jump = &(*jump)[1..];
                            (false, byte)
                        };

                        if is_empty {
                            self.state = Bcj2State::FetchDestination { opcode, bytes_read };
                            return Ok(Bcj2ArbitratorStatus::NeedsMoreInput(stream_id));
                        }

                        self.dest_buf[bytes_read as usize] = b;
                        bytes_read += 1;
                    }

                    let dest = u32::from_be_bytes(self.dest_buf);
                    // 64-bit sign-extended displacement reconstruction: Target = PC + 5 + Offset
                    let next_ip = self.ip.wrapping_add(5);
                    let rel = (dest as u64).wrapping_sub(next_ip) as u32;
                    let rel_le = rel.to_le_bytes();

                    self.out_buf[0] = opcode;
                    self.out_buf[1] = rel_le[0];
                    self.out_buf[2] = rel_le[1];
                    self.out_buf[3] = rel_le[2];
                    self.out_buf[4] = rel_le[3];

                    self.state = Bcj2State::FlushOutput {
                        out_len: 5,
                        out_pos: 0,
                    };
                }
                Bcj2State::FlushOutput {
                    out_len,
                    mut out_pos,
                } => {
                    while out_pos < out_len {
                        if out.is_empty() {
                            self.state = Bcj2State::FlushOutput { out_len, out_pos };
                            return Ok(Bcj2ArbitratorStatus::NeedsMoreOutput);
                        }

                        let b = self.out_buf[out_pos as usize];
                        let (first, rest) = std::mem::take(out).split_at_mut(1);
                        first[0] = b;
                        *out = rest;

                        self.prev_byte = b;
                        self.ip = self.ip.wrapping_add(1);
                        self.total_out = self.total_out.wrapping_add(1);
                        out_pos += 1;
                    }

                    self.state = Bcj2State::FetchMain;
                }
                Bcj2State::Finished => {
                    return Ok(Bcj2ArbitratorStatus::Finished);
                }
            }
        }
    }

    /// Convenience chunk processor returning consumed and written byte counts.
    pub fn process_chunk(
        &mut self,
        mut main: &[u8],
        mut call: &[u8],
        mut jump: &[u8],
        mut rc: &[u8],
        out: &mut [u8],
        main_is_eof: bool,
    ) -> Result<(usize, usize, usize, usize, usize, Bcj2ArbitratorStatus), TTZipStatus> {
        let main_orig_len = main.len();
        let call_orig_len = call.len();
        let jump_orig_len = jump.len();
        let rc_orig_len = rc.len();
        let out_orig_len = out.len();

        let mut out_slice = out;
        let status = self.process(
            &mut main,
            &mut call,
            &mut jump,
            &mut rc,
            &mut out_slice,
            main_is_eof,
        )?;

        let main_consumed = main_orig_len - main.len();
        let call_consumed = call_orig_len - call.len();
        let jump_consumed = jump_orig_len - jump.len();
        let rc_consumed = rc_orig_len - rc.len();
        let out_written = out_orig_len - out_slice.len();

        Ok((
            main_consumed,
            call_consumed,
            jump_consumed,
            rc_consumed,
            out_written,
            status,
        ))
    }
}

/// Helper function to shift unconsumed bytes to buffer origin and refill from reader.
#[inline]
fn refill_buffer<R: Read>(
    reader: &mut R,
    buf: &mut [u8; BCJ2_STREAM_BUFFER_SIZE],
    pos: &mut usize,
    len: &mut usize,
) -> io::Result<usize> {
    if *pos > 0 && *pos < *len {
        buf.copy_within(*pos..*len, 0);
        *len -= *pos;
        *pos = 0;
    } else if *pos >= *len {
        *pos = 0;
        *len = 0;
    }

    if *len >= BCJ2_STREAM_BUFFER_SIZE {
        return Ok(0);
    }

    let n = reader.read(&mut buf[*len..])?;
    *len += n;
    Ok(n)
}

/// Bounded-memory, zero-deadlock demand-driven streaming reader for 4-Stream BCJ2 decompression.
///
/// Holds 4 dedicated 64KB micro-buffers (one per channel), guaranteeing a total resident memory
/// footprint strictly $\le 256\text{KB}$. Pulls bytes from input readers strictly on-demand.
pub struct Bcj2StreamReader<R0, R1, R2, R3> {
    arbitrator: Bcj2StreamArbitrator,
    main_reader: R0,
    call_reader: R1,
    jump_reader: R2,
    rc_reader: R3,
    main_buf: Box<[u8; BCJ2_STREAM_BUFFER_SIZE]>,
    call_buf: Box<[u8; BCJ2_STREAM_BUFFER_SIZE]>,
    jump_buf: Box<[u8; BCJ2_STREAM_BUFFER_SIZE]>,
    rc_buf: Box<[u8; BCJ2_STREAM_BUFFER_SIZE]>,
    main_pos: usize,
    main_len: usize,
    call_pos: usize,
    call_len: usize,
    jump_pos: usize,
    jump_len: usize,
    rc_pos: usize,
    rc_len: usize,
    main_is_eof: bool,
}

impl<R0: Read, R1: Read, R2: Read, R3: Read> Bcj2StreamReader<R0, R1, R2, R3> {
    /// Creates a new `Bcj2StreamReader` from 4 input streams and a starting instruction pointer.
    pub fn new(main_reader: R0, call_reader: R1, jump_reader: R2, rc_reader: R3, ip: u64) -> Self {
        Self {
            arbitrator: Bcj2StreamArbitrator::new(ip),
            main_reader,
            call_reader,
            jump_reader,
            rc_reader,
            main_buf: Box::new([0u8; BCJ2_STREAM_BUFFER_SIZE]),
            call_buf: Box::new([0u8; BCJ2_STREAM_BUFFER_SIZE]),
            jump_buf: Box::new([0u8; BCJ2_STREAM_BUFFER_SIZE]),
            rc_buf: Box::new([0u8; BCJ2_STREAM_BUFFER_SIZE]),
            main_pos: 0,
            main_len: 0,
            call_pos: 0,
            call_len: 0,
            jump_pos: 0,
            jump_len: 0,
            rc_pos: 0,
            rc_len: 0,
            main_is_eof: false,
        }
    }

    /// Returns an immutable reference to the underlying stream arbitrator.
    #[inline]
    #[must_use]
    pub const fn arbitrator(&self) -> &Bcj2StreamArbitrator {
        &self.arbitrator
    }

    /// Refills the demanded channel buffer from its underlying reader.
    fn refill_stream(&mut self, stream_id: Bcj2StreamId) -> io::Result<usize> {
        match stream_id {
            Bcj2StreamId::StreamMain => refill_buffer(
                &mut self.main_reader,
                &mut self.main_buf,
                &mut self.main_pos,
                &mut self.main_len,
            ),
            Bcj2StreamId::StreamCall => refill_buffer(
                &mut self.call_reader,
                &mut self.call_buf,
                &mut self.call_pos,
                &mut self.call_len,
            ),
            Bcj2StreamId::StreamJump => refill_buffer(
                &mut self.jump_reader,
                &mut self.jump_buf,
                &mut self.jump_pos,
                &mut self.jump_len,
            ),
            Bcj2StreamId::StreamRc => refill_buffer(
                &mut self.rc_reader,
                &mut self.rc_buf,
                &mut self.rc_pos,
                &mut self.rc_len,
            ),
        }
    }
}

impl<R0: Read, R1: Read, R2: Read, R3: Read> Read for Bcj2StreamReader<R0, R1, R2, R3> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let orig_len = buf.len();
        let mut out_slice = buf;

        loop {
            let mut main_slice = &self.main_buf[self.main_pos..self.main_len];
            let mut call_slice = &self.call_buf[self.call_pos..self.call_len];
            let mut jump_slice = &self.jump_buf[self.jump_pos..self.jump_len];
            let mut rc_slice = &self.rc_buf[self.rc_pos..self.rc_len];

            let main_before = main_slice.len();
            let call_before = call_slice.len();
            let jump_before = jump_slice.len();
            let rc_before = rc_slice.len();

            let res = self.arbitrator.process(
                &mut main_slice,
                &mut call_slice,
                &mut jump_slice,
                &mut rc_slice,
                &mut out_slice,
                self.main_is_eof,
            );

            self.main_pos += main_before - main_slice.len();
            self.call_pos += call_before - call_slice.len();
            self.jump_pos += jump_before - jump_slice.len();
            self.rc_pos += rc_before - rc_slice.len();

            let bytes_written = orig_len - out_slice.len();

            match res {
                Ok(Bcj2ArbitratorStatus::Finished) => {
                    return Ok(bytes_written);
                }
                Ok(Bcj2ArbitratorStatus::NeedsMoreOutput) => {
                    return Ok(bytes_written);
                }
                Ok(Bcj2ArbitratorStatus::NeedsMoreInput(stream_id)) => {
                    let bytes_refilled = self.refill_stream(stream_id)?;
                    if bytes_refilled == 0 {
                        if stream_id == Bcj2StreamId::StreamMain {
                            self.main_is_eof = true;
                            continue;
                        } else {
                            return Err(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                format!(
                                    "BCJ2 auxiliary stream {:?} reached unexpected EOF",
                                    stream_id
                                ),
                            ));
                        }
                    }
                    continue;
                }
                Err(e) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("BCJ2 decoding error: {:?}", e),
                    ));
                }
            }
        }
    }
}

/// Freestanding convenience function to decompress 4 BCJ2 streams directly into a writer sink.
pub fn decode_bcj2_stream<R0: Read, R1: Read, R2: Read, R3: Read, W: Write>(
    main: R0,
    call: R1,
    jump: R2,
    rc: R3,
    mut writer: W,
    ip: u64,
) -> io::Result<u64> {
    let mut reader = Bcj2StreamReader::new(main, call, jump, rc, ip);
    io::copy(&mut reader, &mut writer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codecs::branch::bcj2::encoder::encode_bcj2;

    #[test]
    fn test_stream_id_roundtrip() {
        assert_eq!(Bcj2StreamId::from_index(0), Some(Bcj2StreamId::StreamMain));
        assert_eq!(Bcj2StreamId::from_index(1), Some(Bcj2StreamId::StreamCall));
        assert_eq!(Bcj2StreamId::from_index(2), Some(Bcj2StreamId::StreamJump));
        assert_eq!(Bcj2StreamId::from_index(3), Some(Bcj2StreamId::StreamRc));
        assert_eq!(Bcj2StreamId::from_index(4), None);
        assert_eq!(Bcj2StreamId::StreamMain.as_index(), 0);
        assert_eq!(Bcj2StreamId::StreamCall.as_index(), 1);
        assert_eq!(Bcj2StreamId::StreamJump.as_index(), 2);
        assert_eq!(Bcj2StreamId::StreamRc.as_index(), 3);
    }

    #[test]
    fn test_arbitrator_single_byte_steps() {
        let code = vec![
            0x55, 0x48, 0x89, 0xE5,
            0xE8, 0x20, 0x00, 0x00, 0x00,
            0x90,
            0xE9, 0x40, 0x00, 0x00, 0x00,
            0x5D, 0xC3,
        ];
        let encoded = encode_bcj2(&code, 0x1000);

        let mut arb = Bcj2StreamArbitrator::new(0x1000);
        let mut main_slice = &encoded.main[..];
        let mut call_slice = &encoded.call[..];
        let mut jump_slice = &encoded.jump[..];
        let mut rc_slice = &encoded.rc[..];

        let mut output = vec![0u8; code.len()];
        let mut out_slice = &mut output[..];

        let status = arb
            .process(
                &mut main_slice,
                &mut call_slice,
                &mut jump_slice,
                &mut rc_slice,
                &mut out_slice,
                true,
            )
            .expect("process should succeed");

        assert_eq!(status, Bcj2ArbitratorStatus::Finished);
        assert_eq!(output, code);
        assert_eq!(arb.total_out(), code.len() as u64);
    }

    #[test]
    fn test_stream_reader_pipe() {
        let code = vec![
            0x90, 0x90,
            0xE8, 0x10, 0x00, 0x00, 0x00,
            0xCC,
            0xE9, 0x20, 0x00, 0x00, 0x00,
        ];
        let encoded = encode_bcj2(&code, 0x2000);

        let mut out = Vec::new();
        let bytes_copied = decode_bcj2_stream(
            &encoded.main[..],
            &encoded.call[..],
            &encoded.jump[..],
            &encoded.rc[..],
            &mut out,
            0x2000,
        )
        .expect("decode_bcj2_stream");

        assert_eq!(bytes_copied, code.len() as u64);
        assert_eq!(out, code);
    }
}
