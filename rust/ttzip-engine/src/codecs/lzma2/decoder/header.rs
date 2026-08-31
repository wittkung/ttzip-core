// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! LZMA2 Chunk Header definitions, error types, and control byte parsing.

use crate::codecs::lzma::range_coder::RangeCoderError;
use crate::types::TTZipStatus;

/// Maximum allowable LZMA2 uncompressed chunk payload size (2 MiB = 2,097,152 bytes).
pub const LZMA2_MAX_UNPACK_CHUNK_SIZE: usize = 2 * 1024 * 1024;

/// Maximum allowable LZMA2 compressed chunk payload size (64 KiB = 65,536 bytes).
pub const LZMA2_MAX_PACK_CHUNK_SIZE: usize = 64 * 1024;

/// LZMA2 Chunk Decoding and Parsing Errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lzma2DecodeError {
    /// Invalid control byte encountered in chunk header (e.g. 0x03..=0x7F).
    InvalidControlByte(u8),
    /// Incomplete chunk header or unexpected end of input stream.
    TruncatedHeader,
    /// Incomplete payload bytes for the parsed chunk header.
    TruncatedPayload { expected: usize, available: usize },
    /// Corrupted range coder bitstream.
    CorruptBitstream(String),
    /// Stream payload data violates LZMA2 format constraints.
    CorruptData(String),
    /// Match distance exceeds available sliding dictionary history.
    InvalidDistance { distance: usize, dict_len: usize },
    /// Destination output buffer capacity exceeded.
    OutputBufferOverflow,
    /// Out of memory or dictionary capacity exceeded.
    OutOfMemory,
}

impl std::fmt::Display for Lzma2DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidControlByte(b) => {
                write!(f, "Invalid LZMA2 control byte: 0x{b:02X} (reserved range 0x03..=0x7F)")
            }
            Self::TruncatedHeader => write!(f, "Truncated LZMA2 chunk header"),
            Self::TruncatedPayload { expected, available } => write!(
                f,
                "Truncated LZMA2 payload: expected {expected} bytes, available {available} bytes"
            ),
            Self::CorruptBitstream(msg) => write!(f, "Corrupted LZMA2 range bitstream: {msg}"),
            Self::CorruptData(msg) => write!(f, "Corrupted LZMA2 payload data: {msg}"),
            Self::InvalidDistance { distance, dict_len } => write!(
                f,
                "Invalid match distance {distance} exceeds sliding dictionary history ({dict_len} bytes)"
            ),
            Self::OutputBufferOverflow => write!(f, "Output buffer capacity exceeded during LZMA2 decoding"),
            Self::OutOfMemory => write!(f, "Memory budget exceeded during LZMA2 stream allocation"),
        }
    }
}

impl std::error::Error for Lzma2DecodeError {}

impl From<Lzma2DecodeError> for TTZipStatus {
    fn from(err: Lzma2DecodeError) -> Self {
        match err {
            Lzma2DecodeError::InvalidControlByte(_) | Lzma2DecodeError::TruncatedHeader => {
                TTZipStatus::ErrCorruptHeader
            }
            Lzma2DecodeError::TruncatedPayload { .. }
            | Lzma2DecodeError::CorruptBitstream(_)
            | Lzma2DecodeError::CorruptData(_)
            | Lzma2DecodeError::InvalidDistance { .. }
            | Lzma2DecodeError::OutputBufferOverflow => TTZipStatus::ErrExtractionFailed,
            Lzma2DecodeError::OutOfMemory => TTZipStatus::ErrOutOfMemory,
        }
    }
}

impl From<RangeCoderError> for Lzma2DecodeError {
    fn from(err: RangeCoderError) -> Self {
        match err {
            RangeCoderError::UnexpectedEof => {
                Lzma2DecodeError::CorruptBitstream("Unexpected EOF in range coder".to_string())
            }
            RangeCoderError::CorruptBitstream(msg) => {
                Lzma2DecodeError::CorruptBitstream(msg.to_string())
            }
            RangeCoderError::InvalidBitTreeSymbol => {
                Lzma2DecodeError::CorruptData("Invalid bit-tree symbol".to_string())
            }
        }
    }
}

/// LZMA2 Chunk Header Specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lzma2ChunkHeader {
    /// End of Stream (EOS) marker (control byte `0x00`).
    Eos,
    /// Uncompressed chunk with dictionary reset (control byte `0x01`).
    UncompressedResetDict {
        /// Size of uncompressed data payload (`1..=65536` bytes).
        unpack_size: usize,
    },
    /// Uncompressed chunk without dictionary reset (control byte `0x02`).
    UncompressedNoReset {
        /// Size of uncompressed data payload (`1..=65536` bytes).
        unpack_size: usize,
    },
    /// Compressed LZMA chunk (control byte `0x80..=0xFF`).
    Compressed {
        /// Reset mode (`0` = none, `1` = state, `2` = state+probs, `3` = state+probs+dict).
        mode: u8,
        /// Size of unpacked uncompressed data (`1..=2097152` bytes).
        unpack_size: usize,
        /// Size of packed compressed data payload (`1..=65536` bytes).
        pack_size: usize,
        /// Optional packed properties byte (`lc + lp*9 + pb*45`) for modes 2 and 3.
        props: Option<u8>,
    },
}

impl Lzma2ChunkHeader {
    /// Parses an LZMA2 chunk header from a byte slice.
    ///
    /// Returns `Ok(Some((header, header_bytes_consumed)))` on successful parse,
    /// `Ok(None)` if more input bytes are required to complete the header,
    /// or `Err(Lzma2DecodeError)` if an illegal control byte is encountered.
    pub fn parse(src: &[u8]) -> Result<Option<(Self, usize)>, Lzma2DecodeError> {
        if src.is_empty() {
            return Ok(None);
        }

        let control = src[0];
        match control {
            0x00 => Ok(Some((Self::Eos, 1))),
            0x01 => {
                if src.len() < 3 {
                    return Ok(None);
                }
                let unpack_size = (((src[1] as usize) << 8) | (src[2] as usize)) + 1;
                Ok(Some((Self::UncompressedResetDict { unpack_size }, 3)))
            }
            0x02 => {
                if src.len() < 3 {
                    return Ok(None);
                }
                let unpack_size = (((src[1] as usize) << 8) | (src[2] as usize)) + 1;
                Ok(Some((Self::UncompressedNoReset { unpack_size }, 3)))
            }
            0x03..=0x7F => Err(Lzma2DecodeError::InvalidControlByte(control)),
            0x80..=0xFF => {
                let mode = (control >> 5) & 0x03;
                let header_len = if mode >= 2 { 6 } else { 5 };
                if src.len() < header_len {
                    return Ok(None);
                }
                let unpack_size = (((control as usize & 0x1F) << 16)
                    | ((src[1] as usize) << 8)
                    | (src[2] as usize))
                    + 1;
                let pack_size = (((src[3] as usize) << 8) | (src[4] as usize)) + 1;
                let props = if mode >= 2 { Some(src[5]) } else { None };

                Ok(Some((
                    Self::Compressed {
                        mode,
                        unpack_size,
                        pack_size,
                        props,
                    },
                    header_len,
                )))
            }
        }
    }

    /// Returns `true` if this header is an End-of-Stream marker.
    #[inline(always)]
    pub const fn is_eos(&self) -> bool {
        matches!(self, Self::Eos)
    }

    /// Returns the uncompressed unpack size represented by this header.
    #[inline(always)]
    pub const fn unpack_size(&self) -> usize {
        match self {
            Self::Eos => 0,
            Self::UncompressedResetDict { unpack_size }
            | Self::UncompressedNoReset { unpack_size }
            | Self::Compressed { unpack_size, .. } => *unpack_size,
        }
    }

    /// Returns the packed payload size represented by this header.
    #[inline(always)]
    pub const fn pack_size(&self) -> usize {
        match self {
            Self::Eos => 0,
            Self::UncompressedResetDict { unpack_size }
            | Self::UncompressedNoReset { unpack_size } => *unpack_size,
            Self::Compressed { pack_size, .. } => *pack_size,
        }
    }

    /// Writes an uncompressed chunk (header + data) to a destination vector.
    pub fn write_uncompressed_chunk(dst: &mut Vec<u8>, data: &[u8], reset_dict: bool) {
        let control = if reset_dict { 0x01 } else { 0x02 };
        let size_minus_1 = (data.len().saturating_sub(1)) as u16;
        dst.push(control);
        dst.extend_from_slice(&size_minus_1.to_be_bytes());
        dst.extend_from_slice(data);
    }

    /// Writes an EOS marker byte (0x00) to a destination vector.
    #[inline(always)]
    pub fn write_eos(dst: &mut Vec<u8>) {
        dst.push(0x00);
    }
}
