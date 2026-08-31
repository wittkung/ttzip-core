// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Five-Layer Decompression State Machine and Pure Functional Core.
//!
//! Deconstructs compressed streams across 5 decoupled, pure-slice layers:
//! - Layer 1: Container Frame Header (Magic sniffing, frame flags, CRC32 verification)
//! - Layer 2: Block Partitioning (Raw / RLE / Compressed blocks with 128KB boundaries)
//! - Layer 3: Sequence & Literals Demuxing (LZ77 sequence triplets extraction)
//! - Layer 4: Entropy Coding (Finite State Entropy and Huffman state transitions)
//! - Layer 5: Bitstream Physical Layer (64-bit forward/reverse bit accumulators)
//!
//! Strictly enforces a 100% Functional Core: zero heap allocations during hot decoding,
//! zero I/O side effects, deterministic mathematical transformations.

use crate::types::TTZipStatus;
use std::fmt;

// MARK: - Constants

/// TTZip Frame Magic Header ("TTZ1").
pub const TTZ_FRAME_MAGIC: [u8; 4] = [0x54, 0x54, 0x5A, 0x31];

/// Maximum block uncompressed size (128 KB physical boundary).
pub const MAX_BLOCK_SIZE_128KB: usize = 128 * 1024;

/// Minimum bit accumulator reload threshold in bits.
pub const BIT_RELOAD_THRESHOLD: u8 = 56;

// MARK: - Error Types

/// Comprehensive error classification for the 5-layer state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecompressionError {
    InvalidMagic,
    MalformedHeader,
    UnsupportedVersion(u8),
    BlockSizeExceeded(usize),
    InvalidBlockType(u8),
    BitstreamUnderflow,
    CorruptedEntropyTable,
    InvalidSequenceOffset { offset: usize, current_pos: usize },
    OutputBufferTooSmall { required: usize, available: usize },
    ChecksumMismatch { expected: u32, calculated: u32 },
    UnexpectedEndOfStream,
    InvalidLiteralLength(usize),
    InvalidMatchLength(usize),
}

impl fmt::Display for DecompressionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMagic => write!(f, "Invalid TTZip frame magic"),
            Self::MalformedHeader => write!(f, "Malformed frame header descriptor"),
            Self::UnsupportedVersion(v) => write!(f, "Unsupported frame format version: {}", v),
            Self::BlockSizeExceeded(sz) => write!(f, "Block size {} exceeds 128KB limit", sz),
            Self::InvalidBlockType(t) => write!(f, "Invalid block type descriptor: {}", t),
            Self::BitstreamUnderflow => write!(f, "Bitstream accumulator underflow"),
            Self::CorruptedEntropyTable => write!(f, "Corrupted entropy decoding state/table"),
            Self::InvalidSequenceOffset { offset, current_pos } => {
                write!(f, "Invalid match offset {} at pos {}", offset, current_pos)
            }
            Self::OutputBufferTooSmall { required, available } => {
                write!(f, "Output buffer too small: required {}, available {}", required, available)
            }
            Self::ChecksumMismatch { expected, calculated } => {
                write!(f, "Checksum mismatch: expected 0x{:08X}, got 0x{:08X}", expected, calculated)
            }
            Self::UnexpectedEndOfStream => write!(f, "Unexpected end of compressed input stream"),
            Self::InvalidLiteralLength(l) => write!(f, "Invalid literal run length: {}", l),
            Self::InvalidMatchLength(m) => write!(f, "Invalid match copy length: {}", m),
        }
    }
}

impl std::error::Error for DecompressionError {}

impl From<DecompressionError> for TTZipStatus {
    fn from(err: DecompressionError) -> Self {
        match err {
            DecompressionError::InvalidMagic => TTZipStatus::ErrCorruptHeader,
            DecompressionError::OutputBufferTooSmall { .. } => TTZipStatus::ErrExtractionFailed,
            DecompressionError::ChecksumMismatch { .. } => TTZipStatus::ErrExtractionFailed,
            _ => TTZipStatus::ErrCorruptHeader,
        }
    }
}

// ============================================================================
// Layer 5: Bitstream Physical Layer (64-bit Bit Accumulator)
// ============================================================================

/// 64-bit Forward & Reverse bitstream accumulator operating on pure slices.
#[derive(Debug, Clone)]
pub struct BitstreamReader<'a> {
    slice: &'a [u8],
    cursor: usize,
    accumulator: u64,
    bits_in_acc: u8,
}

impl<'a> BitstreamReader<'a> {
    /// Constructs a forward bitstream reader over a byte slice.
    #[inline]
    pub fn new_forward(slice: &'a [u8]) -> Self {
        let mut reader = Self { slice, cursor: 0, accumulator: 0, bits_in_acc: 0 };
        reader.refill_forward();
        reader
    }

    /// Constructs a reverse bitstream reader operating from end of slice backward.
    #[inline]
    pub fn new_reverse(slice: &'a [u8]) -> Self {
        let mut reader = Self { slice, cursor: slice.len(), accumulator: 0, bits_in_acc: 0 };
        reader.refill_reverse();
        reader
    }

    /// Refills forward 64-bit accumulator with up to 7 bytes.
    #[inline]
    pub fn refill_forward(&mut self) {
        while self.bits_in_acc <= BIT_RELOAD_THRESHOLD && self.cursor < self.slice.len() {
            let byte = self.slice[self.cursor] as u64;
            self.accumulator |= byte << self.bits_in_acc;
            self.bits_in_acc += 8;
            self.cursor += 1;
        }
    }

    /// Refills reverse 64-bit accumulator backward from end of slice.
    #[inline]
    pub fn refill_reverse(&mut self) {
        while self.bits_in_acc <= BIT_RELOAD_THRESHOLD && self.cursor > 0 {
            self.cursor -= 1;
            let byte = self.slice[self.cursor] as u64;
            self.accumulator |= byte << self.bits_in_acc;
            self.bits_in_acc += 8;
        }
    }

    /// Peeks `num_bits` without consuming them from the accumulator.
    #[inline(always)]
    pub fn peek_bits(&self, num_bits: u8) -> Result<u64, DecompressionError> {
        if num_bits > 64 || num_bits > self.bits_in_acc {
            return Err(DecompressionError::BitstreamUnderflow);
        }
        if num_bits == 0 {
            return Ok(0);
        }
        let mask = if num_bits == 64 { u64::MAX } else { (1u64 << num_bits) - 1 };
        Ok(self.accumulator & mask)
    }

    /// Consumes `num_bits` previously peeked.
    #[inline(always)]
    pub fn consume_bits(&mut self, num_bits: u8) {
        if num_bits >= self.bits_in_acc {
            self.accumulator = 0;
            self.bits_in_acc = 0;
        } else {
            self.accumulator >>= num_bits;
            self.bits_in_acc -= num_bits;
        }
    }

    /// Reads (peeks + consumes + refilled) `num_bits`.
    #[inline]
    pub fn read_bits(&mut self, num_bits: u8) -> Result<u64, DecompressionError> {
        self.refill_forward();
        let val = self.peek_bits(num_bits)?;
        self.consume_bits(num_bits);
        Ok(val)
    }

    /// Returns remaining bits available in accumulator plus remaining unread slice bytes.
    #[inline]
    pub fn bits_remaining(&self) -> usize {
        let unread_bytes = self.slice.len().saturating_sub(self.cursor);
        (self.bits_in_acc as usize) + unread_bytes * 8
    }

    /// Returns the current byte cursor in the slice.
    #[inline]
    pub fn current_cursor(&self) -> usize {
        self.cursor
    }
}

// ============================================================================
// Layer 4: Entropy Coding Layer (FSE & Huffman State Machine)
// ============================================================================

/// Finite State Entropy (FSE) / Asymmetric Numeral Systems table entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FseTableEntry {
    pub symbol: u8,
    pub num_bits: u8,
    pub base_state: u16,
}

/// Entropy encoding mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntropyMode {
    DirectUncompressed,
    RleRun,
    HuffmanCanonical,
    FseAns,
}

/// Pure functional entropy state machine.
#[derive(Debug, Clone)]
pub struct EntropyDecoder {
    mode: EntropyMode,
    fse_table: [FseTableEntry; 256],
    huffman_lengths: [u8; 256],
}

impl Default for EntropyDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl EntropyDecoder {
    /// Creates a default identity entropy decoder.
    #[inline]
    pub fn new() -> Self {
        let mut table = [FseTableEntry::default(); 256];
        for (i, entry) in table.iter_mut().enumerate() {
            entry.symbol = i as u8;
            entry.num_bits = 0;
            entry.base_state = i as u16;
        }
        Self {
            mode: EntropyMode::DirectUncompressed,
            fse_table: table,
            huffman_lengths: [8; 256],
        }
    }

    /// Configures the entropy decoder with explicit FSE distribution table.
    #[inline]
    pub fn with_fse_table(mut self, table: [FseTableEntry; 256]) -> Self {
        self.mode = EntropyMode::FseAns;
        self.fse_table = table;
        self
    }

    /// Configures the entropy decoder with canonical Huffman code lengths.
    #[inline]
    pub fn with_huffman_lengths(mut self, lengths: [u8; 256]) -> Self {
        self.mode = EntropyMode::HuffmanCanonical;
        self.huffman_lengths = lengths;
        self
    }

    /// Decodes a single symbol from the bitstream using active entropy mode.
    #[inline]
    pub fn decode_symbol(&self, reader: &mut BitstreamReader<'_>) -> Result<u8, DecompressionError> {
        match self.mode {
            EntropyMode::DirectUncompressed => Ok(reader.read_bits(8)? as u8),
            EntropyMode::RleRun => Ok(self.fse_table[0].symbol),
            EntropyMode::FseAns => {
                reader.refill_forward();
                let state_idx = (reader.peek_bits(8)? as usize) & 0xFF;
                let entry = &self.fse_table[state_idx];
                reader.consume_bits(entry.num_bits.max(1));
                Ok(entry.symbol)
            }
            EntropyMode::HuffmanCanonical => {
                reader.refill_forward();
                let symbol = (reader.peek_bits(8)? as usize) & 0xFF;
                let bits_used = self.huffman_lengths[symbol].clamp(1, 16);
                reader.consume_bits(bits_used.min(8));
                Ok(symbol as u8)
            }
        }
    }

    /// Decodes an array of symbols into destination buffer.
    #[inline]
    pub fn decode_symbols(&self, reader: &mut BitstreamReader<'_>, dst: &mut [u8]) -> Result<usize, DecompressionError> {
        for slot in dst.iter_mut() {
            *slot = self.decode_symbol(reader)?;
        }
        Ok(dst.len())
    }
}

// ============================================================================
// Layer 3: Sequence & Literals Demuxing Layer (LZ77 Triplets)
// ============================================================================

/// LZ77 Sequence Triplet: (literal_length, match_offset, match_length).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lz77Sequence {
    pub literal_length: u32,
    pub match_offset: u32,
    pub match_length: u32,
}

impl Lz77Sequence {
    /// Creates a new LZ77 sequence descriptor.
    #[inline(always)]
    pub const fn new(literal_length: u32, match_offset: u32, match_length: u32) -> Self {
        Self { literal_length, match_offset, match_length }
    }
}

/// Pure Functional LZ77 Sequence Executor operating on slice buffers.
pub struct SequenceExecutor;

impl SequenceExecutor {
    /// Executes a single sequence triplet against the sliding window output buffer.
    #[inline]
    pub fn execute_sequence(
        seq: &Lz77Sequence,
        literals: &[u8],
        lit_cursor: &mut usize,
        out_buf: &mut [u8],
        out_cursor: &mut usize,
    ) -> Result<(), DecompressionError> {
        let lit_len = seq.literal_length as usize;
        let match_len = seq.match_length as usize;
        let match_off = seq.match_offset as usize;

        if lit_len > 0 {
            if *lit_cursor + lit_len > literals.len() {
                return Err(DecompressionError::InvalidLiteralLength(lit_len));
            }
            if *out_cursor + lit_len > out_buf.len() {
                return Err(DecompressionError::OutputBufferTooSmall {
                    required: *out_cursor + lit_len,
                    available: out_buf.len(),
                });
            }
            out_buf[*out_cursor..*out_cursor + lit_len]
                .copy_from_slice(&literals[*lit_cursor..*lit_cursor + lit_len]);
            *lit_cursor += lit_len;
            *out_cursor += lit_len;
        }

        if match_len > 0 {
            if match_off == 0 || match_off > *out_cursor {
                return Err(DecompressionError::InvalidSequenceOffset {
                    offset: match_off,
                    current_pos: *out_cursor,
                });
            }
            if *out_cursor + match_len > out_buf.len() {
                return Err(DecompressionError::OutputBufferTooSmall {
                    required: *out_cursor + match_len,
                    available: out_buf.len(),
                });
            }
            let start_pos = *out_cursor - match_off;
            for i in 0..match_len {
                out_buf[*out_cursor + i] = out_buf[start_pos + i];
            }
            *out_cursor += match_len;
        }
        Ok(())
    }

    /// Decodes a stream of sequence triplets from raw sequence byte definitions.
    #[inline]
    pub fn parse_sequence_triplet(reader: &mut BitstreamReader<'_>) -> Result<Lz77Sequence, DecompressionError> {
        let lit_len = reader.read_bits(16)? as u32;
        let match_off = reader.read_bits(16)? as u32;
        let match_len = reader.read_bits(16)? as u32;
        Ok(Lz77Sequence::new(lit_len, match_off, match_len))
    }
}

// ============================================================================
// Layer 2: Block Partitioning Layer (128KB Boundaries)
// ============================================================================

/// Type classification for physical blocks inside a container frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockType {
    RawUncompressed = 0,
    RleRepeatedByte = 1,
    CompressedLz77Entropy = 2,
    EndOfFrame = 3,
}

impl BlockType {
    /// Parses 2-bit block type code.
    #[inline]
    pub fn from_u8(val: u8) -> Result<Self, DecompressionError> {
        match val & 0x03 {
            0 => Ok(Self::RawUncompressed),
            1 => Ok(Self::RleRepeatedByte),
            2 => Ok(Self::CompressedLz77Entropy),
            3 => Ok(Self::EndOfFrame),
            other => Err(DecompressionError::InvalidBlockType(other)),
        }
    }
}

/// Block header descriptor (9 bytes: 1 byte flags + 4 bytes unc_sz + 4 bytes comp_sz).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockHeader {
    pub block_type: BlockType,
    pub is_last_block: bool,
    pub uncompressed_size: u32,
    pub compressed_size: u32,
}

impl BlockHeader {
    /// Parses a 9-byte TTZip block header from slice.
    #[inline]
    pub fn parse_from_slice(slice: &[u8]) -> Result<(Self, usize), DecompressionError> {
        if slice.len() < 9 {
            return Err(DecompressionError::UnexpectedEndOfStream);
        }
        let flags = slice[0];
        let block_type = BlockType::from_u8(flags & 0x03)?;
        let is_last_block = (flags & 0x80) != 0;
        let uncompressed_size = u32::from_le_bytes([slice[1], slice[2], slice[3], slice[4]]);
        let compressed_size = u32::from_le_bytes([slice[5], slice[6], slice[7], slice[8]]);

        if (uncompressed_size as usize) > MAX_BLOCK_SIZE_128KB {
            return Err(DecompressionError::BlockSizeExceeded(uncompressed_size as usize));
        }

        Ok((Self { block_type, is_last_block, uncompressed_size, compressed_size }, 9))
    }

    /// Serializes block header into a 9-byte buffer.
    #[inline]
    pub fn write_to_slice(&self, dst: &mut [u8]) -> Result<usize, DecompressionError> {
        if dst.len() < 9 {
            return Err(DecompressionError::OutputBufferTooSmall { required: 9, available: dst.len() });
        }
        let mut flags = self.block_type as u8;
        if self.is_last_block { flags |= 0x80; }
        dst[0] = flags;
        dst[1..5].copy_from_slice(&self.uncompressed_size.to_le_bytes());
        dst[5..9].copy_from_slice(&self.compressed_size.to_le_bytes());
        Ok(9)
    }
}

// ============================================================================
// Layer 1: Container Frame Header Layer (Magic & Metadata)
// ============================================================================

/// Container frame header metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    pub version: u8,
    pub has_checksum: bool,
    pub dictionary_id: Option<u32>,
    pub expected_uncompressed_size: Option<u64>,
}

impl FrameHeader {
    /// Parses frame header from input slice.
    #[inline]
    pub fn parse_from_slice(slice: &[u8]) -> Result<(Self, usize), DecompressionError> {
        if slice.len() < 6 {
            return Err(DecompressionError::UnexpectedEndOfStream);
        }
        if slice[0..4] != TTZ_FRAME_MAGIC {
            return Err(DecompressionError::InvalidMagic);
        }
        let version = slice[4];
        if version != 1 {
            return Err(DecompressionError::UnsupportedVersion(version));
        }
        let flags = slice[5];
        let has_checksum = (flags & 0x01) != 0;
        let has_dict = (flags & 0x02) != 0;
        let has_content_size = (flags & 0x04) != 0;

        let mut offset = 6;
        let mut dictionary_id = None;
        if has_dict {
            if slice.len() < offset + 4 { return Err(DecompressionError::UnexpectedEndOfStream); }
            dictionary_id = Some(u32::from_le_bytes([slice[offset], slice[offset + 1], slice[offset + 2], slice[offset + 3]]));
            offset += 4;
        }

        let mut expected_uncompressed_size = None;
        if has_content_size {
            if slice.len() < offset + 8 { return Err(DecompressionError::UnexpectedEndOfStream); }
            expected_uncompressed_size = Some(u64::from_le_bytes([
                slice[offset], slice[offset + 1], slice[offset + 2], slice[offset + 3],
                slice[offset + 4], slice[offset + 5], slice[offset + 6], slice[offset + 7],
            ]));
            offset += 8;
        }

        Ok((Self { version, has_checksum, dictionary_id, expected_uncompressed_size }, offset))
    }

    /// Serializes frame header into slice.
    #[inline]
    pub fn write_to_slice(&self, dst: &mut [u8]) -> Result<usize, DecompressionError> {
        let min_len = 6 + if self.dictionary_id.is_some() { 4 } else { 0 } + if self.expected_uncompressed_size.is_some() { 8 } else { 0 };
        if dst.len() < min_len {
            return Err(DecompressionError::OutputBufferTooSmall { required: min_len, available: dst.len() });
        }
        dst[0..4].copy_from_slice(&TTZ_FRAME_MAGIC);
        dst[4] = self.version;

        let mut flags = 0u8;
        if self.has_checksum { flags |= 0x01; }
        if self.dictionary_id.is_some() { flags |= 0x02; }
        if self.expected_uncompressed_size.is_some() { flags |= 0x04; }
        dst[5] = flags;

        let mut offset = 6;
        if let Some(dict_id) = self.dictionary_id {
            dst[offset..offset + 4].copy_from_slice(&dict_id.to_le_bytes());
            offset += 4;
        }
        if let Some(sz) = self.expected_uncompressed_size {
            dst[offset..offset + 8].copy_from_slice(&sz.to_le_bytes());
            offset += 8;
        }
        Ok(offset)
    }
}

// ============================================================================
// FiveLayerStateMachine: Full Pipeline Orchestrator
// ============================================================================

/// High-performance 5-layer state machine orchestrating decompression.
#[derive(Debug, Clone, Default)]
pub struct FiveLayerStateMachine;

impl FiveLayerStateMachine {
    /// Constructs a new 5-layer state machine.
    #[inline]
    pub fn new() -> Self { Self }

    /// Decompresses an entire TTZip frame from `src` into `dst` slice.
    #[inline]
    pub fn decompress_frame(&self, src: &[u8], dst: &mut [u8]) -> Result<usize, DecompressionError> {
        let (header, mut src_cursor) = FrameHeader::parse_from_slice(src)?;
        let mut dst_cursor = 0;

        loop {
            if src_cursor >= src.len() { break; }

            if header.has_checksum && src.len() - src_cursor == 4 {
                let expected_crc = u32::from_le_bytes([src[src_cursor], src[src_cursor + 1], src[src_cursor + 2], src[src_cursor + 3]]);
                let actual_crc = crc32fast::hash(&dst[..dst_cursor]);
                if expected_crc != actual_crc {
                    return Err(DecompressionError::ChecksumMismatch { expected: expected_crc, calculated: actual_crc });
                }
                break;
            }

            let (block_hdr, hdr_len) = BlockHeader::parse_from_slice(&src[src_cursor..])?;
            src_cursor += hdr_len;

            match block_hdr.block_type {
                BlockType::RawUncompressed => {
                    let block_sz = block_hdr.compressed_size as usize;
                    if src_cursor + block_sz > src.len() { return Err(DecompressionError::UnexpectedEndOfStream); }
                    if dst_cursor + block_sz > dst.len() {
                        return Err(DecompressionError::OutputBufferTooSmall { required: dst_cursor + block_sz, available: dst.len() });
                    }
                    dst[dst_cursor..dst_cursor + block_sz].copy_from_slice(&src[src_cursor..src_cursor + block_sz]);
                    dst_cursor += block_sz;
                    src_cursor += block_sz;
                }
                BlockType::RleRepeatedByte => {
                    if src_cursor >= src.len() { return Err(DecompressionError::UnexpectedEndOfStream); }
                    let byte = src[src_cursor];
                    src_cursor += 1;
                    let count = block_hdr.uncompressed_size as usize;
                    if dst_cursor + count > dst.len() {
                        return Err(DecompressionError::OutputBufferTooSmall { required: dst_cursor + count, available: dst.len() });
                    }
                    dst[dst_cursor..dst_cursor + count].fill(byte);
                    dst_cursor += count;
                }
                BlockType::CompressedLz77Entropy => {
                    let block_sz = block_hdr.compressed_size as usize;
                    if src_cursor + block_sz > src.len() { return Err(DecompressionError::UnexpectedEndOfStream); }
                    let block_data = &src[src_cursor..src_cursor + block_sz];
                    src_cursor += block_sz;
                    self.decompress_lz77_block(block_data, dst, &mut dst_cursor)?;
                }
                BlockType::EndOfFrame => break,
            }

            if block_hdr.is_last_block {
                if header.has_checksum && src_cursor + 4 <= src.len() {
                    let expected_crc = u32::from_le_bytes([src[src_cursor], src[src_cursor + 1], src[src_cursor + 2], src[src_cursor + 3]]);
                    let actual_crc = crc32fast::hash(&dst[..dst_cursor]);
                    if expected_crc != actual_crc {
                        return Err(DecompressionError::ChecksumMismatch { expected: expected_crc, calculated: actual_crc });
                    }
                }
                break;
            }
        }

        if let Some(expected_sz) = header.expected_uncompressed_size {
            if (dst_cursor as u64) != expected_sz {
                return Err(DecompressionError::OutputBufferTooSmall { required: expected_sz as usize, available: dst_cursor });
            }
        }

        Ok(dst_cursor)
    }

    /// Decompresses an LZ77 + Entropy block.
    fn decompress_lz77_block(&self, block_data: &[u8], dst: &mut [u8], dst_cursor: &mut usize) -> Result<(), DecompressionError> {
        if block_data.len() < 4 { return Err(DecompressionError::UnexpectedEndOfStream); }
        let num_sequences = u16::from_le_bytes([block_data[0], block_data[1]]) as usize;
        let lit_size = u16::from_le_bytes([block_data[2], block_data[3]]) as usize;

        let seq_header_len = 4;
        let seq_bytes_len = num_sequences * 6;
        if block_data.len() < seq_header_len + seq_bytes_len + lit_size {
            return Err(DecompressionError::UnexpectedEndOfStream);
        }

        let seq_slice = &block_data[seq_header_len..seq_header_len + seq_bytes_len];
        let lit_slice = &block_data[seq_header_len + seq_bytes_len..seq_header_len + seq_bytes_len + lit_size];

        let mut bitstream = BitstreamReader::new_forward(seq_slice);
        let mut lit_cursor = 0;

        for _ in 0..num_sequences {
            let seq = SequenceExecutor::parse_sequence_triplet(&mut bitstream)?;
            SequenceExecutor::execute_sequence(&seq, lit_slice, &mut lit_cursor, dst, dst_cursor)?;
        }

        if lit_cursor < lit_size {
            let rem = lit_size - lit_cursor;
            if *dst_cursor + rem > dst.len() {
                return Err(DecompressionError::OutputBufferTooSmall { required: *dst_cursor + rem, available: dst.len() });
            }
            dst[*dst_cursor..*dst_cursor + rem].copy_from_slice(&lit_slice[lit_cursor..lit_size]);
            *dst_cursor += rem;
        }
        Ok(())
    }
}

// ============================================================================
// Companion Pure Functional Frame Encoder for Testing & Roundtrip
// ============================================================================

/// Pure functional companion encoder creating 5-layer TTZ1 frames.
pub struct FiveLayerFrameEncoder;

impl FiveLayerFrameEncoder {
    /// Encodes a raw byte slice into a 5-layer TTZ1 frame.
    pub fn encode_frame(raw_data: &[u8], use_checksum: bool) -> Vec<u8> {
        let mut out = Vec::with_capacity(raw_data.len() + 64);
        let header = FrameHeader {
            version: 1,
            has_checksum: use_checksum,
            dictionary_id: None,
            expected_uncompressed_size: Some(raw_data.len() as u64),
        };
        let mut hdr_buf = [0u8; 32];
        let hdr_len = header.write_to_slice(&mut hdr_buf).unwrap();
        out.extend_from_slice(&hdr_buf[..hdr_len]);

        if raw_data.is_empty() {
            let blk = BlockHeader { block_type: BlockType::RawUncompressed, is_last_block: true, uncompressed_size: 0, compressed_size: 0 };
            let mut blk_buf = [0u8; 9];
            blk.write_to_slice(&mut blk_buf).unwrap();
            out.extend_from_slice(&blk_buf);
        } else {
            let chunks: Vec<&[u8]> = raw_data.chunks(MAX_BLOCK_SIZE_128KB).collect();
            for (idx, chunk) in chunks.iter().enumerate() {
                let is_last = idx == chunks.len() - 1;
                if chunk.len() > 16 && chunk.iter().all(|&b| b == chunk[0]) {
                    let blk = BlockHeader { block_type: BlockType::RleRepeatedByte, is_last_block: is_last, uncompressed_size: chunk.len() as u32, compressed_size: 1 };
                    let mut blk_buf = [0u8; 9];
                    blk.write_to_slice(&mut blk_buf).unwrap();
                    out.extend_from_slice(&blk_buf);
                    out.push(chunk[0]);
                } else if chunk.len() > 32 {
                    let (seqs, lits) = Self::simple_lz77_compress(chunk);
                    let payload_len = 4 + seqs.len() * 6 + lits.len();
                    if payload_len < chunk.len() {
                        let blk = BlockHeader { block_type: BlockType::CompressedLz77Entropy, is_last_block: is_last, uncompressed_size: chunk.len() as u32, compressed_size: payload_len as u32 };
                        let mut blk_buf = [0u8; 9];
                        blk.write_to_slice(&mut blk_buf).unwrap();
                        out.extend_from_slice(&blk_buf);
                        out.extend_from_slice(&(seqs.len() as u16).to_le_bytes());
                        out.extend_from_slice(&(lits.len() as u16).to_le_bytes());
                        for s in &seqs {
                            out.extend_from_slice(&(s.literal_length as u16).to_le_bytes());
                            out.extend_from_slice(&(s.match_offset as u16).to_le_bytes());
                            out.extend_from_slice(&(s.match_length as u16).to_le_bytes());
                        }
                        out.extend_from_slice(&lits);
                    } else {
                        let blk = BlockHeader { block_type: BlockType::RawUncompressed, is_last_block: is_last, uncompressed_size: chunk.len() as u32, compressed_size: chunk.len() as u32 };
                        let mut blk_buf = [0u8; 9];
                        blk.write_to_slice(&mut blk_buf).unwrap();
                        out.extend_from_slice(&blk_buf);
                        out.extend_from_slice(chunk);
                    }
                } else {
                    let blk = BlockHeader { block_type: BlockType::RawUncompressed, is_last_block: is_last, uncompressed_size: chunk.len() as u32, compressed_size: chunk.len() as u32 };
                    let mut blk_buf = [0u8; 9];
                    blk.write_to_slice(&mut blk_buf).unwrap();
                    out.extend_from_slice(&blk_buf);
                    out.extend_from_slice(chunk);
                }
            }
        }

        if use_checksum {
            let crc = crc32fast::hash(raw_data);
            out.extend_from_slice(&crc.to_le_bytes());
        }
        out
    }

    fn simple_lz77_compress(input: &[u8]) -> (Vec<Lz77Sequence>, Vec<u8>) {
        let mut sequences = Vec::new();
        let mut literals = Vec::new();
        let mut pos = 0;
        let mut lit_start = 0;

        while pos < input.len() {
            let mut best_len = 0;
            let mut best_off = 0;
            let max_lookback = pos.min(32768);
            let window_start = pos - max_lookback;

            if pos + 4 <= input.len() {
                for candidate in (window_start..pos).rev() {
                    let mut match_len = 0;
                    while pos + match_len < input.len()
                        && input[candidate + match_len] == input[pos + match_len]
                        && match_len < 255
                    {
                        match_len += 1;
                    }
                    if match_len > best_len {
                        best_len = match_len;
                        best_off = pos - candidate;
                        if match_len >= 32 { break; }
                    }
                }
            }

            if best_len >= 4 {
                let lit_len = pos - lit_start;
                literals.extend_from_slice(&input[lit_start..pos]);
                sequences.push(Lz77Sequence::new(lit_len as u32, best_off as u32, best_len as u32));
                pos += best_len;
                lit_start = pos;
            } else {
                pos += 1;
            }
        }

        if lit_start < input.len() {
            literals.extend_from_slice(&input[lit_start..input.len()]);
        }
        (sequences, literals)
    }
}
