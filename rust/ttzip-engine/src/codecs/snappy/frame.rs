// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Official Snappy Framing Format (.sz) streaming chunk parsing, emission, and FSM.
//!
//! Conforms strictly to the Google Snappy framing format specification with:
//! - 10-byte stream identifier verification (`0xff 0x06 0x00 0x00 sNaPpY`)
//! - 4-byte chunk header decoding and encoding (1-byte chunk type + 3-byte little-endian length)
//! - 64KB uncompressed chunk boundary guarantees
//! - Robust finite state machine transitions across stream frames
//! - Castagnoli CRC-32C checksum verification

use crate::codecs::snappy::error::SnappyError;
use crate::codecs::snappy::framed_reader::SnappyFramedReader;
use crate::codecs::snappy::framed_writer::SnappyFramedWriter;
use crate::types::TTZipStatus;
use std::io::{Cursor, Read, Write};

/// Standard Snappy framing stream identifier chunk (10 bytes):
/// `[0xff, 0x06, 0x00, 0x00, 0x73, 0x4e, 0x61, 0x50, 0x70, 0x59]`.
pub const STREAM_IDENTIFIER_CHUNK: [u8; 10] =
    [0xff, 0x06, 0x00, 0x00, 0x73, 0x4e, 0x61, 0x50, 0x70, 0x59];

/// Standard Snappy framing magic ASCII bytes: `"sNaPpY"` (`[0x73, 0x4e, 0x61, 0x50, 0x70, 0x59]`).
pub const STREAM_IDENTIFIER_MAGIC: [u8; 6] = [0x73, 0x4e, 0x61, 0x50, 0x70, 0x59];

/// Backward-compatible alias for the standard 10-byte stream identifier.
pub const SNAPPY_STREAM_IDENTIFIER: [u8; 10] = STREAM_IDENTIFIER_CHUNK;

/// Maximum uncompressed raw data allowed per chunk according to the Snappy framing specification (64KB).
pub const MAX_UNCOMPRESSED_CHUNK_SIZE: usize = 65536;

/// Backward-compatible alias for maximum uncompressed raw chunk size.
pub const SNAPPY_MAX_CHUNK_SIZE: usize = MAX_UNCOMPRESSED_CHUNK_SIZE;

/// Maximum legal uncompressed chunk payload size including the 4-byte masked CRC-32C checksum (65540 bytes).
pub const MAX_UNCOMPRESSED_CHUNK_PAYLOAD_SIZE: usize = MAX_UNCOMPRESSED_CHUNK_SIZE + 4;

/// Maximum legal 24-bit payload length (2^24 - 1 = 16,777,215 bytes).
pub const MAX_CHUNK_PAYLOAD_SIZE: usize = 16_777_215;

/// Size in bytes of a Snappy framing chunk header (1-byte type + 3-byte length).
pub const CHUNK_HEADER_SIZE: usize = 4;

/// Size in bytes of a Snappy framing masked CRC-32C checksum.
pub const CRC_SIZE: usize = 4;

pub use super::crc::{mask_crc32c, unmask_crc32c};

/// Snappy Framing Format chunk identifier type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SnappyChunkType {
    /// Compressed data chunk (`0x00`). Payload begins with 4-byte masked CRC-32C followed by Snappy compressed bitstream.
    Compressed,
    /// Uncompressed raw data chunk (`0x01`). Payload begins with 4-byte masked CRC-32C followed by raw uncompressed bytes.
    Uncompressed,
    /// Reserved unskippable chunk (`0x02..=0x7f`). Decoders MUST fail immediately when encountered.
    ReservedUnskippable(u8),
    /// Reserved skippable chunk (`0x80..=0xfd`). Decoders MUST skip payload and resume decoding.
    ReservedSkippable(u8),
    /// Padding chunk (`0xfe`). Payload bytes should be skipped without verification.
    Padding,
    /// Stream identifier chunk (`0xff`). Payload must contain exactly 6 bytes: `"sNaPpY"`.
    StreamIdentifier,
}

impl SnappyChunkType {
    /// Parses a 1-byte raw chunk tag into a strongly typed `SnappyChunkType`.
    #[inline]
    pub const fn from_u8(tag: u8) -> Self {
        match tag {
            0x00 => Self::Compressed,
            0x01 => Self::Uncompressed,
            0xfe => Self::Padding,
            0xff => Self::StreamIdentifier,
            0x02..=0x7f => Self::ReservedUnskippable(tag),
            0x80..=0xfd => Self::ReservedSkippable(tag),
        }
    }

    /// Converts this `SnappyChunkType` back into its 1-byte raw wire tag.
    #[inline]
    pub const fn as_u8(&self) -> u8 {
        match *self {
            Self::Compressed => 0x00,
            Self::Uncompressed => 0x01,
            Self::Padding => 0xfe,
            Self::StreamIdentifier => 0xff,
            Self::ReservedUnskippable(tag) | Self::ReservedSkippable(tag) => tag,
        }
    }

    /// Returns `true` if this chunk type is skippable by a framing decompressor.
    #[inline]
    pub const fn is_skippable(&self) -> bool {
        matches!(
            self,
            Self::Padding | Self::ReservedSkippable(_) | Self::StreamIdentifier
        )
    }

    /// Returns `true` if this chunk type is an unskippable reserved chunk that must trigger an error.
    #[inline]
    pub const fn is_unskippable(&self) -> bool {
        matches!(self, Self::ReservedUnskippable(_))
    }
}

/// Snappy framing 4-byte chunk header (1-byte type + 3-byte little-endian payload length).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnappyChunkHeader {
    /// Strongly-typed framing chunk type.
    pub chunk_type: SnappyChunkType,
    /// Length of the subsequent payload in bytes (0..=16,777,215).
    pub payload_len: usize,
}

impl SnappyChunkHeader {
    /// Creates a new `SnappyChunkHeader` with specified chunk type and payload length.
    #[inline]
    pub const fn new(chunk_type: SnappyChunkType, payload_len: usize) -> Self {
        Self {
            chunk_type,
            payload_len,
        }
    }

    /// Parses a 4-byte slice into a `SnappyChunkHeader`.
    ///
    /// # Errors
    /// Returns `SnappyError::BlockTooLarge` if payload length exceeds 16,777,215 bytes,
    /// `SnappyError::UnsupportedChunkType` if encountering an unskippable reserved chunk,
    /// or `SnappyError::CorruptHeader` if chunk invariants are violated (e.g. uncompressed chunk payload > 65540 bytes).
    #[inline]
    pub fn parse(src: &[u8; 4]) -> Result<Self, SnappyError> {
        let chunk_type = SnappyChunkType::from_u8(src[0]);
        let payload_len =
            (src[1] as usize) | ((src[2] as usize) << 8) | ((src[3] as usize) << 16);

        if payload_len > MAX_CHUNK_PAYLOAD_SIZE {
            return Err(SnappyError::BlockTooLarge {
                size: payload_len,
                max: MAX_CHUNK_PAYLOAD_SIZE,
            });
        }

        match chunk_type {
            SnappyChunkType::StreamIdentifier => {
                if payload_len != STREAM_IDENTIFIER_MAGIC.len() {
                    return Err(SnappyError::CorruptHeader(format!(
                        "Stream identifier payload length must be 6 bytes, found {}",
                        payload_len
                    )));
                }
            }
            SnappyChunkType::Uncompressed => {
                if payload_len > MAX_UNCOMPRESSED_CHUNK_PAYLOAD_SIZE {
                    return Err(SnappyError::BlockTooLarge {
                        size: payload_len,
                        max: MAX_UNCOMPRESSED_CHUNK_PAYLOAD_SIZE,
                    });
                }
                if payload_len < CRC_SIZE {
                    return Err(SnappyError::CorruptHeader(format!(
                        "Uncompressed chunk payload length {} is too small for CRC-32C",
                        payload_len
                    )));
                }
            }
            SnappyChunkType::Compressed => {
                if payload_len < CRC_SIZE {
                    return Err(SnappyError::CorruptHeader(format!(
                        "Compressed chunk payload length {} is too small for CRC-32C",
                        payload_len
                    )));
                }
            }
            SnappyChunkType::ReservedUnskippable(tag) => {
                return Err(SnappyError::UnsupportedChunkType(tag));
            }
            SnappyChunkType::ReservedSkippable(_) | SnappyChunkType::Padding => {}
        }

        Ok(Self {
            chunk_type,
            payload_len,
        })
    }

    /// Emits the 4-byte chunk header into the destination buffer in little-endian format.
    #[inline]
    pub fn emit(&self, dst: &mut [u8; 4]) {
        dst[0] = self.chunk_type.as_u8();
        dst[1] = (self.payload_len & 0xFF) as u8;
        dst[2] = ((self.payload_len >> 8) & 0xFF) as u8;
        dst[3] = ((self.payload_len >> 16) & 0xFF) as u8;
    }

    /// Returns the 4-byte serialized header representation.
    #[inline]
    pub fn to_bytes(&self) -> [u8; 4] {
        let mut bytes = [0u8; 4];
        self.emit(&mut bytes);
        bytes
    }
}

/// Validates that the input slice begins with the standard 10-byte Snappy stream identifier.
///
/// Returns the number of bytes consumed (`10`) on success.
///
/// # Errors
/// Returns `SnappyError::UnexpectedEof` if `src` is shorter than 10 bytes,
/// or `SnappyError::InvalidMagicHeader` if the identifier bytes do not match.
#[inline]
pub fn validate_stream_identifier(src: &[u8]) -> Result<usize, SnappyError> {
    if src.len() < STREAM_IDENTIFIER_CHUNK.len() {
        return Err(SnappyError::UnexpectedEof);
    }
    if src[..STREAM_IDENTIFIER_CHUNK.len()] != STREAM_IDENTIFIER_CHUNK {
        return Err(SnappyError::InvalidMagicHeader);
    }
    Ok(STREAM_IDENTIFIER_CHUNK.len())
}

/// Checks if data begins with standard Snappy stream identifier without returning errors.
#[inline]
pub fn is_framed_snappy(data: &[u8]) -> bool {
    validate_stream_identifier(data).is_ok()
}

/// State of the Snappy framing finite state machine (FSM).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SnappyFrameFsmState {
    /// Awaiting the mandatory initial 10-byte stream identifier chunk (`0xff 0x06 0x00 0x00 sNaPpY`).
    ExpectIdentifier,
    /// Awaiting the 4-byte chunk header (`[type, len_low, len_mid, len_high]`).
    ReadChunkHeader,
    /// Processing chunk payload data.
    ProcessPayload,
    /// Terminal state after stream completion or clean end of chunks.
    Done,
}

/// A streaming finite state machine tracker for Snappy framed chunk streams.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnappyFrameFsm {
    state: SnappyFrameFsmState,
    current_header: Option<SnappyChunkHeader>,
    seen_identifier: bool,
}

impl Default for SnappyFrameFsm {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl SnappyFrameFsm {
    /// Creates a new FSM initialized in `ExpectIdentifier` state.
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: SnappyFrameFsmState::ExpectIdentifier,
            current_header: None,
            seen_identifier: false,
        }
    }

    /// Returns the current FSM state.
    #[inline]
    pub const fn state(&self) -> SnappyFrameFsmState {
        self.state
    }

    /// Returns the current chunk header if in `ProcessPayload` state.
    #[inline]
    pub const fn current_header(&self) -> Option<SnappyChunkHeader> {
        self.current_header
    }

    /// Returns `true` if the stream identifier has been successfully validated.
    #[inline]
    pub const fn has_seen_identifier(&self) -> bool {
        self.seen_identifier
    }

    /// Resets the FSM to initial state.
    #[inline]
    pub fn reset(&mut self) {
        self.state = SnappyFrameFsmState::ExpectIdentifier;
        self.current_header = None;
        self.seen_identifier = false;
    }

    /// Advances the FSM by validating a 10-byte stream identifier chunk.
    pub fn feed_identifier(&mut self, src: &[u8]) -> Result<usize, SnappyError> {
        match self.state {
            SnappyFrameFsmState::ExpectIdentifier => {
                let consumed = validate_stream_identifier(src)?;
                self.seen_identifier = true;
                self.state = SnappyFrameFsmState::ReadChunkHeader;
                Ok(consumed)
            }
            SnappyFrameFsmState::ReadChunkHeader | SnappyFrameFsmState::ProcessPayload => {
                // Secondary stream identifier chunk within stream is valid per spec (ignored)
                let consumed = validate_stream_identifier(src)?;
                self.state = SnappyFrameFsmState::ReadChunkHeader;
                Ok(consumed)
            }
            SnappyFrameFsmState::Done => Err(SnappyError::InvalidParam(
                "Cannot feed stream identifier into terminated FSM".to_string(),
            )),
        }
    }

    /// Advances the FSM by parsing a 4-byte chunk header.
    pub fn feed_header(&mut self, header_bytes: &[u8; 4]) -> Result<SnappyChunkHeader, SnappyError> {
        match self.state {
            SnappyFrameFsmState::ExpectIdentifier => Err(SnappyError::InvalidMagicHeader),
            SnappyFrameFsmState::ReadChunkHeader => {
                let header = SnappyChunkHeader::parse(header_bytes)?;
                self.current_header = Some(header);
                self.state = SnappyFrameFsmState::ProcessPayload;
                Ok(header)
            }
            SnappyFrameFsmState::ProcessPayload => Err(SnappyError::InvalidParam(
                "Cannot feed new header while payload processing is in progress".to_string(),
            )),
            SnappyFrameFsmState::Done => Err(SnappyError::InvalidParam(
                "Cannot feed header into terminated FSM".to_string(),
            )),
        }
    }

    /// Completes payload processing for the current chunk and transitions state.
    pub fn finish_payload(&mut self) -> Result<(), SnappyError> {
        match self.state {
            SnappyFrameFsmState::ProcessPayload => {
                self.current_header = None;
                self.state = SnappyFrameFsmState::ReadChunkHeader;
                Ok(())
            }
            _ => Err(SnappyError::InvalidParam(
                "Cannot finish payload when not in ProcessPayload state".to_string(),
            )),
        }
    }

    /// Signals clean end-of-stream.
    pub fn finish_stream(&mut self) -> Result<(), SnappyError> {
        match self.state {
            SnappyFrameFsmState::ReadChunkHeader => {
                self.state = SnappyFrameFsmState::Done;
                Ok(())
            }
            SnappyFrameFsmState::Done => Ok(()),
            SnappyFrameFsmState::ExpectIdentifier | SnappyFrameFsmState::ProcessPayload => {
                Err(SnappyError::UnexpectedEof)
            }
        }
    }
}

/// Encodes raw buffer into Snappy framing format (.sz) in a pre-allocated destination buffer.
pub fn snappy_frame_encode(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    let mut cursor = Cursor::new(dst);
    {
        let mut encoder = SnappyFramedWriter::new(&mut cursor);
        encoder
            .write_all(src)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        encoder
            .flush()
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    }
    let written = cursor.position() as usize;
    Ok(written)
}

/// Encodes raw buffer into Snappy framing format (.sz) returning `Vec<u8>`.
pub fn snappy_frame_encode_to_vec(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    let mut out = Vec::with_capacity(src.len() + 64);
    {
        let mut encoder = SnappyFramedWriter::new(&mut out);
        encoder
            .write_all(src)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        encoder
            .flush()
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    }
    Ok(out)
}

/// Decodes Snappy framing format (.sz) buffer into a pre-allocated destination buffer.
pub fn snappy_frame_decode(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    let mut decoder = SnappyFramedReader::new(src);
    let mut total_read = 0;
    while total_read < dst.len() {
        match decoder.read(&mut dst[total_read..]) {
            Ok(0) => break,
            Ok(n) => total_read += n,
            Err(_) => return Err(TTZipStatus::ErrExtractionFailed),
        }
    }
    Ok(total_read)
}

/// Decodes Snappy framing format (.sz) buffer into `Vec<u8>` with optional max size limit.
pub fn snappy_frame_decode_to_vec(src: &[u8], max_allowed: usize) -> Result<Vec<u8>, TTZipStatus> {
    if !is_framed_snappy(src) {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let mut decoder = SnappyFramedReader::new(src);
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        match decoder.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if out.len() + n > max_allowed {
                    return Err(TTZipStatus::ErrExtractionFailed);
                }
                out.extend_from_slice(&buf[..n]);
            }
            Err(_) => return Err(TTZipStatus::ErrCorruptHeader),
        }
    }
    Ok(out)
}

/// Validates the integrity of a Snappy framed buffer (.sz) using CRC-32C verification.
///
/// Returns `true` if the stream identifier is valid, all chunk headers are well-formed,
/// and all uncompressed chunk payloads match their embedded Castagnoli CRC-32C checksums.
pub fn snappy_frame_validate(src: &[u8]) -> bool {
    if !is_framed_snappy(src) {
        return false;
    }
    let mut cursor = Cursor::new(src);
    snappy_frame_validate_reader(&mut cursor)
}

/// Validates the integrity of a Snappy framed stream (.sz) from a `Read` source using a stack buffer.
pub fn snappy_frame_validate_reader<R: Read>(reader: &mut R) -> bool {
    let mut decoder = SnappyFramedReader::new(reader);
    let mut stack_buf = [0u8; SNAPPY_MAX_CHUNK_SIZE];
    loop {
        match decoder.read(&mut stack_buf) {
            Ok(0) => return true,
            Ok(_) => continue,
            Err(_) => return false,
        }
    }
}

/// Computes upper bound on encoded framing stream length.
#[inline]
pub fn snappy_frame_max_encoded_length(src_len: usize) -> usize {
    if src_len == 0 {
        return SNAPPY_STREAM_IDENTIFIER.len();
    }
    let num_chunks = src_len.div_ceil(SNAPPY_MAX_CHUNK_SIZE);
    SNAPPY_STREAM_IDENTIFIER.len()
        + num_chunks
            * (8 + crate::codecs::snappy::raw_encoder::max_compressed_len(SNAPPY_MAX_CHUNK_SIZE))
}

