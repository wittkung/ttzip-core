// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Meta-block stream parsing and finite state machine (FSM) for RFC 7932 Brotli decompression.

use super::bit_reader::BrotliBitReader;
use super::error::BrotliError;

/// Type classification for Brotli meta-blocks per RFC 7932 Section 9.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetaBlockType {
    /// Uncompressed raw byte block with byte-aligned payload.
    Uncompressed,
    /// Metadata block containing non-decompressed metadata to be skipped or intercepted.
    Metadata,
    /// Entropy-coded block with dynamic Huffman codes, context maps, and LZ77 commands.
    Compressed,
    /// Empty terminal block marking stream termination.
    Empty,
}

/// Decoded meta-block header structure containing block boundaries and format semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetaBlockHeader {
    /// Whether this is the final meta-block in the Brotli stream (`ISLAST`).
    pub is_last: bool,
    /// Whether this final meta-block is completely empty (`ISLASTEMPTY`).
    pub is_last_empty: bool,
    /// Semantic type of the meta-block.
    pub block_type: MetaBlockType,
    /// Uncompressed length of the payload in bytes (`MLEN`, `MSKIPBYTES + 1`, or `0`).
    pub uncompressed_len: usize,
}

impl MetaBlockHeader {
    /// Parses a meta-block header from an LSB-first `BrotliBitReader` per RFC 7932 Section 9.2.
    ///
    /// # Grammatical Flow
    /// 1. Reads 1-bit `ISLAST`.
    /// 2. If `ISLAST == 1`, reads 1-bit `ISLASTEMPTY`. If `1`, returns `MetaBlockType::Empty` (`len = 0`).
    /// 3. Reads 2-bit `MNIBBLES`:
    ///    - If `MNIBBLES == 3`: Metadata block.
    ///      - Reads 1-bit `RESERVED` (must be `0`).
    ///      - Reads 2-bit `MSKIPBYTES` (0..=3).
    ///      - If `MSKIPBYTES > 0`, reads `MSKIPBYTES` 8-bit bytes (verifying no exuberant high-order zero byte).
    ///      - Aligns to byte boundary with zero-padding check (`jump_to_byte_boundary`).
    ///    - If `MNIBBLES < 3`:
    ///      - `size_nibbles = MNIBBLES + 4` (4..=6).
    ///      - Reads `size_nibbles` 4-bit nibbles (verifying no exuberant high-order zero nibble when `size_nibbles > 4`).
    ///      - If `!ISLAST`, reads 1-bit `ISUNCOMPRESSED`:
    ///        - If `1`, aligns to byte boundary with zero-padding check, yielding `MetaBlockType::Uncompressed`.
    ///        - If `0`, yields `MetaBlockType::Compressed`.
    ///      - If `ISLAST`, always yields `MetaBlockType::Compressed`.
    ///
    /// # Errors
    /// - `BrotliError::UnexpectedEof` if the bitstream ends prematurely.
    /// - `BrotliError::InvalidPadding` if non-zero padding bits exist during byte alignment.
    /// - `BrotliError::CorruptHeader` if non-zero reserved bits or exuberant nibbles/bytes are present.
    pub fn parse(br: &mut BrotliBitReader<'_>) -> Result<Self, BrotliError> {
        let is_last = br.read_bits(1)? != 0;
        if is_last {
            let is_last_empty = br.read_bits(1)? != 0;
            if is_last_empty {
                return Ok(Self {
                    is_last: true,
                    is_last_empty: true,
                    block_type: MetaBlockType::Empty,
                    uncompressed_len: 0,
                });
            }
        }

        let mnibbles = br.read_bits(2)?;
        if mnibbles == 3 {
            // Metadata meta-block
            let reserved = br.read_bits(1)?;
            if reserved != 0 {
                return Err(BrotliError::CorruptHeader(
                    "Invalid non-zero reserved bit in metadata block header".into(),
                ));
            }

            let mskipbytes = br.read_bits(2)?;
            let uncompressed_len = if mskipbytes == 0 {
                0
            } else {
                let mut mlen = 0usize;
                for i in 0..mskipbytes {
                    let byte = br.read_bits(8)?;
                    if i + 1 == mskipbytes && mskipbytes > 1 && byte == 0 {
                        return Err(BrotliError::CorruptHeader(
                            "Exuberant high-order byte in metadata block length (RFC 7932 Section 9.2)"
                                .into(),
                        ));
                    }
                    mlen |= (byte as usize) << (i * 8);
                }
                mlen + 1
            };

            br.jump_to_byte_boundary()?;

            return Ok(Self {
                is_last,
                is_last_empty: false,
                block_type: MetaBlockType::Metadata,
                uncompressed_len,
            });
        }

        // mnibbles in 0..=2: size_nibbles = mnibbles + 4 (4, 5, or 6 nibbles)
        let size_nibbles = (mnibbles + 4) as usize;
        let mut mlen = 0usize;
        for i in 0..size_nibbles {
            let nibble = br.read_bits(4)?;
            if i + 1 == size_nibbles && size_nibbles > 4 && nibble == 0 {
                return Err(BrotliError::CorruptHeader(
                    "Exuberant high-order nibble in meta-block length (RFC 7932 Section 9.2)"
                        .into(),
                ));
            }
            mlen |= (nibble as usize) << (i * 4);
        }
        let uncompressed_len = mlen + 1;

        let block_type = if !is_last {
            let is_uncompressed = br.read_bits(1)? != 0;
            if is_uncompressed {
                br.jump_to_byte_boundary()?;
                MetaBlockType::Uncompressed
            } else {
                MetaBlockType::Compressed
            }
        } else {
            MetaBlockType::Compressed
        };

        Ok(Self {
            is_last,
            is_last_empty: false,
            block_type,
            uncompressed_len,
        })
    }
}

/// Finite state machine (FSM) states managing Brotli streaming decompression lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrotliDecoderFsmState {
    /// Initial uninitialized state before reading stream header.
    Init,
    /// Reading sliding window bits prefix (WBITS).
    ReadWindowBits,
    /// Reading meta-block header (ISLAST, length, type).
    ReadMetaBlockHeader,
    /// Streaming uncompressed raw byte block payload.
    UncompressedData,
    /// Decoding compressed Huffman trees, context maps, and LZ77 commands.
    CompressedCommands,
    /// Skipping metadata block payload.
    MetadataSkip,
    /// Stream decompression completed successfully.
    Done,
}

impl BrotliDecoderFsmState {
    /// Returns `true` if the state machine has reached terminal completion (`Done`).
    #[inline]
    pub fn is_done(&self) -> bool {
        matches!(self, Self::Done)
    }

    /// Returns `true` if the state represents active block payload processing.
    #[inline]
    pub fn is_processing_payload(&self) -> bool {
        matches!(
            self,
            Self::UncompressedData | Self::CompressedCommands | Self::MetadataSkip
        )
    }
}
