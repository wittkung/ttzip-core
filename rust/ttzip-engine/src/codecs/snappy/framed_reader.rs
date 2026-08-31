// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe-Rust streaming framing decompressor for Google Snappy format (.sz).
//!
//! Conforms to the official Snappy Framing Format specification with standard
//! Castagnoli CRC-32C verification, supporting concatenated streams, skipped chunks,
//! and bounded 64KB stack/heap dual buffering with zero memory leaks.

use crate::codecs::snappy::crc::{crc32c, mask_crc32c};
use crate::codecs::snappy::error::SnappyError;
use crate::codecs::snappy::frame::{
    SnappyChunkHeader, SnappyChunkType, SnappyFrameFsmState, CHUNK_HEADER_SIZE, CRC_SIZE,
    MAX_UNCOMPRESSED_CHUNK_PAYLOAD_SIZE, SNAPPY_MAX_CHUNK_SIZE, STREAM_IDENTIFIER_MAGIC,
};
use crate::codecs::snappy::raw_decoder::{raw_decompress, raw_uncompressed_length};
use std::io::{self, Error, ErrorKind, Read};

/// High-performance pure Safe-Rust unidirectional streaming decompressor for Snappy framed data (.sz).
///
/// Implements `std::io::Read` to provide a streaming decompression interface with fixed 64KB internal
/// buffer bounds, strict CRC-32C verification, transparent concatenated stream handling, and robust
/// error interception for corrupted chunks or invalid framing sequences.
pub struct SnappyFramedReader<R: Read> {
    /// Underlying source reader delivering compressed Snappy framed stream.
    reader: R,
    /// Current state in the framing FSM.
    fsm_state: SnappyFrameFsmState,
    /// Fixed 64KB uncompressed chunk buffer.
    decompressed_chunk_buf: [u8; SNAPPY_MAX_CHUNK_SIZE],
    /// Current read cursor index within `decompressed_chunk_buf`.
    buf_pos: usize,
    /// Number of valid decompressed bytes available in `decompressed_chunk_buf`.
    buf_limit: usize,
    /// Reusable dynamic buffer for reading raw chunk payloads from `reader`.
    raw_chunk_buf: Vec<u8>,
}

impl<R: Read> SnappyFramedReader<R> {
    /// Creates a new `SnappyFramedReader` wrapping the provided `reader`.
    #[inline]
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            fsm_state: SnappyFrameFsmState::ExpectIdentifier,
            decompressed_chunk_buf: [0u8; SNAPPY_MAX_CHUNK_SIZE],
            buf_pos: 0,
            buf_limit: 0,
            raw_chunk_buf: Vec::new(),
        }
    }

    /// Unwraps this `SnappyFramedReader`, returning the underlying reader.
    #[inline]
    pub fn into_inner(self) -> R {
        self.reader
    }

    /// Gets a shared reference to the underlying reader.
    #[inline]
    pub fn get_ref(&self) -> &R {
        &self.reader
    }

    /// Gets a mutable reference to the underlying reader.
    #[inline]
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.reader
    }

    /// Returns the current FSM state of the framed reader.
    #[inline]
    pub fn fsm_state(&self) -> SnappyFrameFsmState {
        self.fsm_state
    }

    /// Consumes and discards exactly `len` bytes from the underlying reader.
    fn skip_bytes(&mut self, mut remaining: usize) -> io::Result<()> {
        while remaining > 0 {
            let to_read = remaining.min(self.decompressed_chunk_buf.len());
            self.reader
                .read_exact(&mut self.decompressed_chunk_buf[..to_read])?;
            remaining -= to_read;
        }
        Ok(())
    }

    /// Fills `decompressed_chunk_buf` with the uncompressed payload of the next valid data chunk.
    ///
    /// Drives the framing state machine, skipping stream identifiers, padding, and skippable chunks.
    /// Returns `Ok(true)` if a new data chunk was successfully decoded, `Ok(false)` on clean EOF,
    /// or `Err` if a framing violation, CRC mismatch, or I/O error occurred.
    fn fill_next_chunk(&mut self) -> io::Result<bool> {
        loop {
            // 1. Attempt to read the 1st byte (Chunk Type) of the 4-byte chunk header
            let mut type_buf = [0u8; 1];
            match self.reader.read(&mut type_buf) {
                Ok(0) => {
                    // Clean EOF
                    self.fsm_state = SnappyFrameFsmState::Done;
                    return Ok(false);
                }
                Ok(1) => {}
                Ok(_) => unreachable!(),
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }

            // 2. Read the remaining 3 bytes for 24-bit little-endian chunk length
            let mut header_raw = [0u8; CHUNK_HEADER_SIZE];
            header_raw[0] = type_buf[0];
            self.reader.read_exact(&mut header_raw[1..])?;

            // 3. Parse chunk header structure and validate invariants
            let header = SnappyChunkHeader::parse(&header_raw)
                .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;

            match header.chunk_type {
                SnappyChunkType::StreamIdentifier => {
                    let mut magic = [0u8; 6];
                    self.reader.read_exact(&mut magic)?;
                    if magic != STREAM_IDENTIFIER_MAGIC {
                        return Err(Error::new(
                            ErrorKind::InvalidData,
                            SnappyError::InvalidMagicHeader,
                        ));
                    }
                    self.fsm_state = SnappyFrameFsmState::ReadChunkHeader;
                    // Transparently continue loop to process next chunk
                    continue;
                }
                _ => {
                    // Non-identifier chunk: require that the stream identifier was already encountered
                    if self.fsm_state == SnappyFrameFsmState::ExpectIdentifier {
                        return Err(Error::new(
                            ErrorKind::InvalidData,
                            SnappyError::InvalidMagicHeader,
                        ));
                    }

                    match header.chunk_type {
                        SnappyChunkType::Compressed => {
                            if header.payload_len < CRC_SIZE {
                                return Err(Error::new(
                                    ErrorKind::InvalidData,
                                    SnappyError::CorruptHeader(
                                        "Compressed chunk length smaller than 4-byte checksum".to_string(),
                                    ),
                                ));
                            }

                            // Read 4-byte Masked CRC32C
                            let mut crc_buf = [0u8; CRC_SIZE];
                            self.reader.read_exact(&mut crc_buf)?;
                            let expected_masked_crc = u32::from_le_bytes(crc_buf);

                            let comp_len = header.payload_len - CRC_SIZE;
                            self.raw_chunk_buf.resize(comp_len, 0);
                            self.reader.read_exact(&mut self.raw_chunk_buf)?;

                            // Verify uncompressed length does not exceed 64KB chunk bound
                            let uncompressed_len = raw_uncompressed_length(&self.raw_chunk_buf)
                                .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;

                            if uncompressed_len > SNAPPY_MAX_CHUNK_SIZE {
                                return Err(Error::new(
                                    ErrorKind::InvalidData,
                                    SnappyError::BlockTooLarge {
                                        size: uncompressed_len,
                                        max: SNAPPY_MAX_CHUNK_SIZE,
                                    },
                                ));
                            }

                            let decomp_len = raw_decompress(
                                &self.raw_chunk_buf,
                                &mut self.decompressed_chunk_buf[..uncompressed_len],
                            )
                            .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;

                            // Verify Castagnoli Masked CRC32C against uncompressed payload
                            let actual_crc = crc32c(&self.decompressed_chunk_buf[..decomp_len]);
                            let actual_masked_crc = mask_crc32c(actual_crc);

                            if actual_masked_crc != expected_masked_crc {
                                return Err(Error::new(
                                    ErrorKind::InvalidData,
                                    SnappyError::Crc32cMismatch {
                                        expected: expected_masked_crc,
                                        actual: actual_masked_crc,
                                    },
                                ));
                            }

                            self.buf_pos = 0;
                            self.buf_limit = decomp_len;
                            self.fsm_state = SnappyFrameFsmState::ReadChunkHeader;
                            return Ok(true);
                        }
                        SnappyChunkType::Uncompressed => {
                            if header.payload_len < CRC_SIZE {
                                return Err(Error::new(
                                    ErrorKind::InvalidData,
                                    SnappyError::CorruptHeader(
                                        "Uncompressed chunk length smaller than 4-byte checksum".to_string(),
                                    ),
                                ));
                            }

                            let uncomp_len = header.payload_len - CRC_SIZE;
                            if header.payload_len > MAX_UNCOMPRESSED_CHUNK_PAYLOAD_SIZE {
                                return Err(Error::new(
                                    ErrorKind::InvalidData,
                                    SnappyError::BlockTooLarge {
                                        size: uncomp_len,
                                        max: SNAPPY_MAX_CHUNK_SIZE,
                                    },
                                ));
                            }

                            // Read 4-byte Masked CRC32C
                            let mut crc_buf = [0u8; CRC_SIZE];
                            self.reader.read_exact(&mut crc_buf)?;
                            let expected_masked_crc = u32::from_le_bytes(crc_buf);

                            // Read raw uncompressed bytes directly into decompressed chunk buffer
                            self.reader
                                .read_exact(&mut self.decompressed_chunk_buf[..uncomp_len])?;

                            // Verify Castagnoli Masked CRC32C
                            let actual_crc = crc32c(&self.decompressed_chunk_buf[..uncomp_len]);
                            let actual_masked_crc = mask_crc32c(actual_crc);

                            if actual_masked_crc != expected_masked_crc {
                                return Err(Error::new(
                                    ErrorKind::InvalidData,
                                    SnappyError::Crc32cMismatch {
                                        expected: expected_masked_crc,
                                        actual: actual_masked_crc,
                                    },
                                ));
                            }

                            self.buf_pos = 0;
                            self.buf_limit = uncomp_len;
                            self.fsm_state = SnappyFrameFsmState::ReadChunkHeader;
                            return Ok(true);
                        }
                        SnappyChunkType::Padding | SnappyChunkType::ReservedSkippable(_) => {
                            self.skip_bytes(header.payload_len)?;
                            self.fsm_state = SnappyFrameFsmState::ReadChunkHeader;
                            continue;
                        }
                        SnappyChunkType::ReservedUnskippable(tag) => {
                            return Err(Error::new(
                                ErrorKind::InvalidData,
                                SnappyError::UnsupportedChunkType(tag),
                            ));
                        }
                        SnappyChunkType::StreamIdentifier => unreachable!(),
                    }
                }
            }
        }
    }
}

impl<R: Read> Read for SnappyFramedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        // If internal buffer has been completely consumed, refill from the next valid data chunk
        if self.buf_pos >= self.buf_limit {
            self.buf_pos = 0;
            self.buf_limit = 0;

            if self.fsm_state == SnappyFrameFsmState::Done {
                return Ok(0);
            }

            let has_more = self.fill_next_chunk()?;
            if !has_more {
                return Ok(0);
            }
        }

        let available = self.buf_limit - self.buf_pos;
        let to_copy = available.min(buf.len());
        buf[..to_copy].copy_from_slice(&self.decompressed_chunk_buf[self.buf_pos..self.buf_pos + to_copy]);
        self.buf_pos += to_copy;

        Ok(to_copy)
    }
}
