// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, high-throughput streaming decompressor for DEFLATE, Zlib, and Gzip streams.
//!
//! Provides [`LibdeflateReader`], an adapter implementing [`std::io::Read`] with
//! bounded 64KB micro-buffering, RFC 1950 (Zlib) / RFC 1952 (Gzip) container framing
//! validation, incremental Adler-32 / CRC-32 on-the-fly checksum verification, and
//! transparent concatenated Gzip member support.

use super::checksum::{adler32_update, crc32_update};
use super::container::ContainerFormat;
use crate::types::TTZipStatus;
use flate2::{Decompress, FlushDecompress, Status};
use std::io::{self, Error, ErrorKind, Read};

/// Default internal micro-buffer capacity for streaming DEFLATE decompression (64 KB).
pub const DEFAULT_DECOMPRESS_BUFFER_SIZE: usize = 64 * 1024;

/// Internal finite state machine driving container framing and payload decompression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReaderState {
    /// Parsing and validating container header (RFC 1950 Zlib / RFC 1952 Gzip).
    Header,
    /// Streaming and decompressing DEFLATE payload chunks.
    Payload,
    /// Parsing and verifying container trailer checksums (Adler-32 / CRC-32 + ISIZE).
    Trailer,
    /// Stream cleanly finished; subsequent reads return EOF (0 bytes).
    Done,
}

/// High-throughput streaming decompressor for DEFLATE, Zlib, and Gzip containers.
///
/// Implements [`std::io::Read`] with bounded 64KB micro-buffering, multi-member
/// concatenated stream support, and strict RFC checksum verification.
pub struct LibdeflateReader<R: Read> {
    /// Underlying source byte stream.
    inner: R,
    /// Container framing format.
    format: ContainerFormat,
    /// Streaming decompression engine.
    decompressor: Decompress,
    /// Current lifecycle state of the stream parser.
    state: ReaderState,
    /// Internal micro-buffer for incoming compressed bytes.
    in_buf: Vec<u8>,
    /// Read cursor into `in_buf`.
    in_pos: usize,
    /// Valid data limit in `in_buf`.
    in_limit: usize,
    /// Internal micro-buffer for decompressed output bytes.
    out_buf: Vec<u8>,
    /// Read cursor into `out_buf`.
    out_pos: usize,
    /// Valid decompressed data limit in `out_buf`.
    out_limit: usize,
    /// Running Adler-32 checksum of decompressed data (RFC 1950).
    adler: u32,
    /// Running IEEE 802.3 CRC-32 checksum of decompressed data (RFC 1952).
    crc: u32,
    /// Total number of uncompressed bytes produced by this stream.
    total_out: u64,
    /// Total number of uncompressed bytes in the current gzip member.
    member_total_out: u64,
    /// Total number of compressed bytes consumed from inner stream.
    total_in: u64,
    /// Flag indicating whether underlying reader reached EOF.
    inner_eof: bool,
}

impl<R: Read> LibdeflateReader<R> {
    /// Creates a new `LibdeflateReader` wrapping `reader` with the specified container format.
    pub fn new(reader: R, format: ContainerFormat) -> Result<Self, TTZipStatus> {
        let in_capacity = DEFAULT_DECOMPRESS_BUFFER_SIZE;
        let out_capacity = DEFAULT_DECOMPRESS_BUFFER_SIZE;

        let initial_state = match format {
            ContainerFormat::Raw => ReaderState::Payload,
            ContainerFormat::Zlib | ContainerFormat::Gzip => ReaderState::Header,
        };

        Ok(Self {
            inner: reader,
            format,
            decompressor: Decompress::new(false),
            state: initial_state,
            in_buf: vec![0u8; in_capacity],
            in_pos: 0,
            in_limit: 0,
            out_buf: vec![0u8; out_capacity],
            out_pos: 0,
            out_limit: 0,
            adler: 1,
            crc: 0,
            total_out: 0,
            member_total_out: 0,
            total_in: 0,
            inner_eof: false,
        })
    }

    /// Creates a new `LibdeflateReader` configured for raw RFC 1951 DEFLATE streams.
    #[inline]
    pub fn new_raw(reader: R) -> Result<Self, TTZipStatus> {
        Self::new(reader, ContainerFormat::Raw)
    }

    /// Creates a new `LibdeflateReader` configured for RFC 1950 Zlib container streams.
    #[inline]
    pub fn new_zlib(reader: R) -> Result<Self, TTZipStatus> {
        Self::new(reader, ContainerFormat::Zlib)
    }

    /// Creates a new `LibdeflateReader` configured for RFC 1952 Gzip container streams.
    #[inline]
    pub fn new_gzip(reader: R) -> Result<Self, TTZipStatus> {
        Self::new(reader, ContainerFormat::Gzip)
    }

    /// Returns the container framing format of this reader.
    #[inline]
    pub fn format(&self) -> ContainerFormat {
        self.format
    }

    /// Returns the total number of uncompressed bytes produced so far.
    #[inline]
    pub fn total_out(&self) -> u64 {
        self.total_out
    }

    /// Returns the total number of compressed bytes consumed from the underlying reader.
    #[inline]
    pub fn total_in(&self) -> u64 {
        self.total_in
    }

    /// Returns the current running Adler-32 checksum of the decompressed stream.
    #[inline]
    pub fn checksum_adler32(&self) -> u32 {
        self.adler
    }

    /// Returns the current running CRC-32 checksum of the decompressed stream.
    #[inline]
    pub fn checksum_crc32(&self) -> u32 {
        self.crc
    }

    /// Gets a shared reference to the underlying reader.
    #[inline]
    pub fn get_ref(&self) -> &R {
        &self.inner
    }

    /// Gets a mutable reference to the underlying reader.
    #[inline]
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    /// Unwraps this `LibdeflateReader`, returning the underlying reader.
    #[inline]
    pub fn into_inner(self) -> R {
        self.inner
    }

    // MARK: - Internal Buffer Management

    /// Ensures at least `min_bytes` are available in `in_buf[in_pos..in_limit]`.
    fn fill_in_buf(&mut self, min_bytes: usize) -> io::Result<bool> {
        if self.in_pos > 0 {
            let remaining = self.in_limit - self.in_pos;
            if remaining > 0 {
                self.in_buf.copy_within(self.in_pos..self.in_limit, 0);
            }
            self.in_pos = 0;
            self.in_limit = remaining;
        }

        if min_bytes > self.in_buf.len() {
            self.in_buf.resize(min_bytes.max(self.in_buf.len() * 2), 0);
        }

        while (self.in_limit - self.in_pos) < min_bytes && !self.inner_eof {
            let read_slot = &mut self.in_buf[self.in_limit..];
            if read_slot.is_empty() {
                break;
            }
            match self.inner.read(read_slot) {
                Ok(0) => {
                    self.inner_eof = true;
                    break;
                }
                Ok(n) => {
                    self.in_limit += n;
                }
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }

        Ok((self.in_limit - self.in_pos) >= min_bytes)
    }

    // MARK: - Framing Parsers

    /// Parses and validates the container header according to `self.format`.
    fn parse_header(&mut self) -> io::Result<()> {
        match self.format {
            ContainerFormat::Raw => {
                self.state = ReaderState::Payload;
                Ok(())
            }
            ContainerFormat::Zlib => self.parse_zlib_header(),
            ContainerFormat::Gzip => self.parse_gzip_header(),
        }
    }

    /// Parses and validates RFC 1950 zlib container header (2 bytes).
    fn parse_zlib_header(&mut self) -> io::Result<()> {
        if !self.fill_in_buf(2)? {
            return Err(Error::new(
                ErrorKind::UnexpectedEof,
                "unexpected EOF reading zlib header",
            ));
        }

        let cmf = self.in_buf[self.in_pos];
        let flg = self.in_buf[self.in_pos + 1];

        // 1. Header checksum check: (CMF * 256 + FLG) must be a multiple of 31
        let check = ((cmf as u16) << 8) | (flg as u16);
        if !check.is_multiple_of(31) {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("invalid zlib header checksum: CMF=0x{cmf:02X}, FLG=0x{flg:02X}"),
            ));
        }

        // 2. Compression method check: CM=8 (DEFLATE)
        let cm = cmf & 0x0F;
        if cm != 8 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("unsupported zlib compression method: {cm} (expected 8)"),
            ));
        }

        // 3. Window size check: CINFO <= 7 (32KB max window)
        let cinfo = (cmf >> 4) & 0x0F;
        if cinfo > 7 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("invalid zlib window size CINFO: {cinfo} > 7"),
            ));
        }

        // 4. Preset dictionary check: FDICT flag
        let fdict = (flg & 0x20) != 0;
        if fdict {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "preset zlib dictionary (FDICT) is not supported",
            ));
        }

        self.in_pos += 2;
        self.total_in += 2;
        self.decompressor.reset(false);
        self.adler = 1;
        self.state = ReaderState::Payload;
        Ok(())
    }

    /// Parses and validates RFC 1952 gzip container header (10+ bytes).
    fn parse_gzip_header(&mut self) -> io::Result<()> {
        if !self.fill_in_buf(10)? {
            return Err(Error::new(
                ErrorKind::UnexpectedEof,
                "unexpected EOF reading gzip header",
            ));
        }

        let id1 = self.in_buf[self.in_pos];
        let id2 = self.in_buf[self.in_pos + 1];
        if id1 != 0x1F || id2 != 0x8B {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("invalid gzip magic bytes: 0x{id1:02X} 0x{id2:02X} (expected 0x1F 0x8B)"),
            ));
        }

        let cm = self.in_buf[self.in_pos + 2];
        if cm != 8 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("unsupported gzip compression method: {cm} (expected 8)"),
            ));
        }

        let flg = self.in_buf[self.in_pos + 3];
        self.in_pos += 10;
        self.total_in += 10;

        // FEXTRA flag (0x04)
        if (flg & 0x04) != 0 {
            if !self.fill_in_buf(2)? {
                return Err(Error::new(
                    ErrorKind::UnexpectedEof,
                    "unexpected EOF reading gzip extra length",
                ));
            }
            let xlen = u16::from_le_bytes([self.in_buf[self.in_pos], self.in_buf[self.in_pos + 1]]) as usize;
            self.in_pos += 2;
            self.total_in += 2;

            if !self.fill_in_buf(xlen)? {
                return Err(Error::new(
                    ErrorKind::UnexpectedEof,
                    "unexpected EOF reading gzip extra field",
                ));
            }
            self.in_pos += xlen;
            self.total_in += xlen as u64;
        }

        // FNAME flag (0x08)
        if (flg & 0x08) != 0 {
            self.skip_zero_terminated_string("filename")?;
        }

        // FCOMMENT flag (0x10)
        if (flg & 0x10) != 0 {
            self.skip_zero_terminated_string("comment")?;
        }

        // FHCRC flag (0x02)
        if (flg & 0x02) != 0 {
            if !self.fill_in_buf(2)? {
                return Err(Error::new(
                    ErrorKind::UnexpectedEof,
                    "unexpected EOF reading gzip header CRC16",
                ));
            }
            self.in_pos += 2;
            self.total_in += 2;
        }

        self.decompressor.reset(false);
        self.crc = 0;
        self.member_total_out = 0;
        self.state = ReaderState::Payload;
        Ok(())
    }

    /// Skips zero-terminated string (FNAME / FCOMMENT).
    fn skip_zero_terminated_string(&mut self, field_name: &str) -> io::Result<()> {
        loop {
            while self.in_pos < self.in_limit {
                let byte = self.in_buf[self.in_pos];
                self.in_pos += 1;
                self.total_in += 1;
                if byte == 0 {
                    return Ok(());
                }
            }

            if !self.fill_in_buf(1)? {
                return Err(Error::new(
                    ErrorKind::UnexpectedEof,
                    format!("unexpected EOF reading gzip {field_name} string"),
                ));
            }
        }
    }

    // MARK: - Payload Decompression Engine

    /// Fills `out_buf` with decompressed data by driving the payload decompression state.
    fn fill_next_decompressed_chunk(&mut self) -> io::Result<bool> {
        loop {
            let avail_in = self.in_limit - self.in_pos;
            if avail_in == 0 && !self.fill_in_buf(1)? {
                // Inner stream underflow
                match self.format {
                    ContainerFormat::Raw => {
                        self.state = ReaderState::Done;
                        return Ok(false);
                    }
                    ContainerFormat::Zlib | ContainerFormat::Gzip => {
                        return Err(Error::new(
                            ErrorKind::UnexpectedEof,
                            "unexpected EOF in compressed DEFLATE payload",
                        ));
                    }
                }
            }

            let prev_in = self.decompressor.total_in();
            let prev_out = self.decompressor.total_out();

            let status = self.decompressor.decompress(
                &self.in_buf[self.in_pos..self.in_limit],
                &mut self.out_buf[..],
                FlushDecompress::None,
            ).map_err(|e| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("corrupt or invalid DEFLATE payload data: {e:?}"),
                )
            })?;

            let in_consumed = (self.decompressor.total_in() - prev_in) as usize;
            let out_produced = (self.decompressor.total_out() - prev_out) as usize;

            self.in_pos += in_consumed;
            self.total_in += in_consumed as u64;

            if out_produced > 0 {
                self.out_pos = 0;
                self.out_limit = out_produced;

                let chunk = &self.out_buf[..out_produced];
                self.adler = adler32_update(self.adler, chunk);
                self.crc = crc32_update(self.crc, chunk);
                self.total_out = self.total_out.wrapping_add(out_produced as u64);
                self.member_total_out = self.member_total_out.wrapping_add(out_produced as u64);

                if status == Status::StreamEnd {
                    match self.format {
                        ContainerFormat::Raw => self.state = ReaderState::Done,
                        ContainerFormat::Zlib | ContainerFormat::Gzip => {
                            self.state = ReaderState::Trailer;
                        }
                    }
                }
                return Ok(true);
            }

            if status == Status::StreamEnd {
                match self.format {
                    ContainerFormat::Raw => self.state = ReaderState::Done,
                    ContainerFormat::Zlib | ContainerFormat::Gzip => {
                        self.state = ReaderState::Trailer;
                    }
                }
                return Ok(false);
            }

            if in_consumed == 0 && out_produced == 0 {
                let prev_avail = self.in_limit - self.in_pos;
                if !self.fill_in_buf(prev_avail + 1)? {
                    return Err(Error::new(
                        ErrorKind::UnexpectedEof,
                        "unexpected EOF in compressed DEFLATE payload",
                    ));
                }
            }
        }
    }

    // MARK: - Trailer Verification

    /// Parses and verifies container trailer checksums (RFC 1950 Adler-32 / RFC 1952 CRC-32 & ISIZE).
    fn verify_trailer(&mut self) -> io::Result<()> {
        match self.format {
            ContainerFormat::Raw => {
                self.state = ReaderState::Done;
                Ok(())
            }
            ContainerFormat::Zlib => {
                if !self.fill_in_buf(4)? {
                    return Err(Error::new(
                        ErrorKind::UnexpectedEof,
                        "unexpected EOF reading zlib Adler-32 trailer",
                    ));
                }

                let expected_adler = u32::from_be_bytes([
                    self.in_buf[self.in_pos],
                    self.in_buf[self.in_pos + 1],
                    self.in_buf[self.in_pos + 2],
                    self.in_buf[self.in_pos + 3],
                ]);
                self.in_pos += 4;
                self.total_in += 4;

                if expected_adler != self.adler {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "zlib Adler-32 mismatch: stream=0x{:08X}, computed=0x{:08X}",
                            expected_adler, self.adler
                        ),
                    ));
                }

                self.state = ReaderState::Done;
                Ok(())
            }
            ContainerFormat::Gzip => {
                if !self.fill_in_buf(8)? {
                    return Err(Error::new(
                        ErrorKind::UnexpectedEof,
                        "unexpected EOF reading gzip CRC-32 / ISIZE trailer",
                    ));
                }

                let expected_crc = u32::from_le_bytes([
                    self.in_buf[self.in_pos],
                    self.in_buf[self.in_pos + 1],
                    self.in_buf[self.in_pos + 2],
                    self.in_buf[self.in_pos + 3],
                ]);
                let expected_isize = u32::from_le_bytes([
                    self.in_buf[self.in_pos + 4],
                    self.in_buf[self.in_pos + 5],
                    self.in_buf[self.in_pos + 6],
                    self.in_buf[self.in_pos + 7],
                ]);
                self.in_pos += 8;
                self.total_in += 8;

                if expected_crc != self.crc {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "gzip CRC-32 mismatch: stream=0x{:08X}, computed=0x{:08X}",
                            expected_crc, self.crc
                        ),
                    ));
                }

                let actual_isize = self.member_total_out as u32;
                if expected_isize != actual_isize {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "gzip ISIZE mismatch: stream={expected_isize}, computed={actual_isize}",
                        ),
                    ));
                }

                // Check for concatenated GZIP members
                if self.fill_in_buf(1)? {
                    self.state = ReaderState::Header;
                } else {
                    self.state = ReaderState::Done;
                }
                Ok(())
            }
        }
    }
}

// MARK: - std::io::Read Implementation

impl<R: Read> Read for LibdeflateReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        loop {
            if self.out_pos < self.out_limit {
                let available = self.out_limit - self.out_pos;
                let to_copy = buf.len().min(available);
                buf[..to_copy].copy_from_slice(&self.out_buf[self.out_pos..self.out_pos + to_copy]);
                self.out_pos += to_copy;
                return Ok(to_copy);
            }

            match self.state {
                ReaderState::Header => {
                    self.parse_header()?;
                }
                ReaderState::Payload => {
                    let has_data = self.fill_next_decompressed_chunk()?;
                    if !has_data && self.out_pos >= self.out_limit && self.state == ReaderState::Done {
                        return Ok(0);
                    }
                }
                ReaderState::Trailer => {
                    self.verify_trailer()?;
                }
                ReaderState::Done => {
                    return Ok(0);
                }
            }
        }
    }
}
