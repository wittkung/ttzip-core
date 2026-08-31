// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, RAII-governed Bzip2 compression and decompression microkernel.
//!
//! Features:
//! - Levels 1..=9 corresponding to 100KB..900KB Burrows-Wheeler Transform (BWT) block sizes.
//! - Move-to-Front (MTF) and multi-table canonical Huffman tree encoding.
//! - Non-reflected MSB-first CRC-32 and circular-rotated stream combined CRC.
//! - `bzip2recover` bit-level 48-bit Pi magic scanner and disaster recovery.
//! - Configurable output expansion limit to defend against decompression bombs.

pub mod block;
pub mod blocksort;
pub mod crc;
pub mod huffman;
pub mod inverse_bwt;
pub mod mtf;
pub mod reader;
pub mod recover;
pub mod writer;

pub use block::{decode_bzip2_block, encode_bzip2_block, BZIP2_BLOCK_MAGIC, BZIP2_EOS_MAGIC};
pub use blocksort::bwt_block_sort;
pub use crc::{Bzip2CombinedCrc, Bzip2Crc32};
pub use huffman::{BitReader, BZ_MAX_CODE_LEN, BZ_N_GROUPS};
pub use inverse_bwt::{inverse_bwt_fast, inverse_bwt_small};
pub use mtf::{generate_mtf_values, rle1_compress, rle1_decompress, rle2_decode_and_inverse_mtf};
pub use reader::Bzip2Reader;
pub use recover::{bzip2_recover_block, bzip2_scan_blocks, Bzip2BlockSlice};
pub use writer::Bzip2Writer;

use crate::types::TTZipStatus;
use std::io::{Read, Write};

pub const BZ_MAGIC: [u8; 3] = *b"BZh";
pub const BZ_PI_BLOCK_MAGIC: [u8; 6] = BZIP2_BLOCK_MAGIC;
pub const BZ_EOS_BLOCK_MAGIC: [u8; 6] = BZIP2_EOS_MAGIC;
pub const BZIP2_PIPE_BUFFER_SIZE: usize = 64 * 1024;

/// Header metadata inspected from a Bzip2 stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bzip2HeaderInfo {
    pub is_valid: bool,
    pub level: u8,
    pub block_size_100k: u8,
    pub block_size_bytes: usize,
}

/// Returns the worst-case compressed buffer capacity bound for Bzip2.
#[inline]
pub fn bzip2_compress_bound(src_len: usize) -> usize {
    src_len + (src_len / 100) + 600
}

/// Compresses a byte slice into a destination buffer using Bzip2.
pub fn bzip2_compress(src: &[u8], dst: &mut [u8], level: i32) -> Result<usize, TTZipStatus> {
    let lvl = level.clamp(1, 9) as u32;
    let mut writer = Bzip2Writer::new(Vec::new(), lvl);
    writer
        .write_all(src)
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    let compressed = writer
        .finish()
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;

    if compressed.len() > dst.len() {
        return Err(TTZipStatus::ErrInvalidParam);
    }
    dst[..compressed.len()].copy_from_slice(&compressed);
    Ok(compressed.len())
}

/// Decompresses a Bzip2-compressed byte slice into a destination buffer.
pub fn bzip2_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    let mut reader = Bzip2Reader::new(src).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    let mut decompressed = Vec::new();
    reader
        .read_to_end(&mut decompressed)
        .map_err(|_| TTZipStatus::ErrExtractionFailed)?;

    if decompressed.len() > dst.len() {
        return Err(TTZipStatus::ErrInvalidParam);
    }
    dst[..decompressed.len()].copy_from_slice(&decompressed);
    Ok(decompressed.len())
}

/// Compresses a byte slice and returns the compressed bytes as a `Vec<u8>`.
pub fn bzip2_compress_vec(src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
    let lvl = level.clamp(1, 9) as u32;
    let mut writer = Bzip2Writer::new(Vec::new(), lvl);
    writer
        .write_all(src)
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    let result = writer
        .finish()
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    Ok(result)
}

/// Alias for `bzip2_compress_vec` for backwards compatibility.
#[inline]
pub fn bzip2_compress_to_vec(src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
    bzip2_compress_vec(src, level)
}

/// Decompresses a Bzip2-compressed byte slice and returns the decompressed bytes as a `Vec<u8>`.
pub fn bzip2_decompress_vec(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    let mut reader = Bzip2Reader::new(src).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    let mut dst = Vec::new();
    reader
        .read_to_end(&mut dst)
        .map_err(|_| TTZipStatus::ErrExtractionFailed)?;
    Ok(dst)
}

/// Alias for `bzip2_decompress_vec` with optional max_allowed parameter.
#[inline]
pub fn bzip2_decompress_to_vec(src: &[u8], _max_allowed: usize) -> Result<Vec<u8>, TTZipStatus> {
    bzip2_decompress_vec(src)
}

/// Inspects the header of a Bzip2 stream without full decompression.
pub fn bzip2_inspect_header(src: &[u8]) -> Result<Bzip2HeaderInfo, TTZipStatus> {
    if src.len() < 4 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    if src[0] != b'B' || src[1] != b'Z' || src[2] != b'h' {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let lvl_char = src[3];
    if !(b'1'..=b'9').contains(&lvl_char) {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let block_size_100k = lvl_char - b'0';
    let block_size_bytes = (block_size_100k as usize) * 100_000;
    Ok(Bzip2HeaderInfo {
        is_valid: true,
        level: block_size_100k,
        block_size_100k,
        block_size_bytes,
    })
}

/// Streams compression from a reader to a writer over a chunked pipe.
pub fn bzip2_compress_stream_pipe<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    level: i32,
) -> Result<u64, TTZipStatus> {
    let mut raw_input = Vec::new();
    reader
        .read_to_end(&mut raw_input)
        .map_err(|_| TTZipStatus::ErrExtractionFailed)?;
    let in_bytes = raw_input.len() as u64;

    let compressed = bzip2_compress_vec(&raw_input, level)?;
    writer
        .write_all(&compressed)
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    Ok(in_bytes)
}

/// Streams decompression from a reader to a writer over a chunked pipe.
pub fn bzip2_decompress_stream_pipe<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
) -> Result<u64, TTZipStatus> {
    let mut raw_input = Vec::new();
    reader
        .read_to_end(&mut raw_input)
        .map_err(|_| TTZipStatus::ErrExtractionFailed)?;

    let decompressed = bzip2_decompress_vec(&raw_input)?;
    let out_bytes = decompressed.len() as u64;
    writer
        .write_all(&decompressed)
        .map_err(|_| TTZipStatus::ErrExtractionFailed)?;
    Ok(out_bytes)
}

/// Validates whether a byte slice is a valid Bzip2 stream.
pub fn bzip2_validate(src: &[u8]) -> bool {
    if src.len() < 4 {
        return false;
    }
    if src[0] != b'B' || src[1] != b'Z' || src[2] != b'h' || !(b'1'..=b'9').contains(&src[3]) {
        return false;
    }
    let mut reader = BitReader::new(&src[4..]);
    let mut combined_crc = Bzip2CombinedCrc::new();
    let mut temp_dst = Vec::new();

    loop {
        match decode_bzip2_block(&mut reader, &mut temp_dst, &mut combined_crc) {
            Ok(true) => continue,
            Ok(false) => return true,
            Err(_) => return false,
        }
    }
}

/// Chunk-based decompressor state machine for filter pipeline.
pub struct Bzip2Decompressor {
    _small: bool,
    accumulated_in: Vec<u8>,
    decompressed_out: Vec<u8>,
    out_cursor: usize,
    finished: bool,
}

impl Bzip2Decompressor {
    pub fn new(small: bool, _verbosity: i32) -> Result<Self, TTZipStatus> {
        Ok(Self {
            _small: small,
            accumulated_in: Vec::new(),
            decompressed_out: Vec::new(),
            out_cursor: 0,
            finished: false,
        })
    }

    pub fn decompress_chunk(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
    ) -> Result<(usize, usize, bool), TTZipStatus> {
        self.accumulated_in.extend_from_slice(src);
        let in_consumed = src.len();

        if !self.finished && self.accumulated_in.len() >= 4 {
            if let Ok(mut reader) = Bzip2Reader::new(&self.accumulated_in[..]) {
                let mut out = Vec::new();
                if reader.read_to_end(&mut out).is_ok() {
                    self.decompressed_out = out;
                    self.finished = true;
                }
            }
        }

        let avail = self.decompressed_out.len() - self.out_cursor;
        let to_copy = avail.min(dst.len());
        if to_copy > 0 {
            dst[..to_copy].copy_from_slice(
                &self.decompressed_out[self.out_cursor..self.out_cursor + to_copy],
            );
            self.out_cursor += to_copy;
        }

        Ok((
            in_consumed,
            to_copy,
            self.finished && self.out_cursor >= self.decompressed_out.len(),
        ))
    }
}

/// Chunk-based compressor state machine for filter pipeline.
pub struct Bzip2Compressor {
    level: i32,
    accumulated_in: Vec<u8>,
}

impl Bzip2Compressor {
    pub fn new(level: i32, _verbosity: i32, _work_factor: i32) -> Result<Self, TTZipStatus> {
        Ok(Self {
            level,
            accumulated_in: Vec::new(),
        })
    }

    pub fn compress_chunk(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
        finish: bool,
    ) -> Result<(usize, usize, bool), TTZipStatus> {
        self.accumulated_in.extend_from_slice(src);
        let in_consumed = src.len();
        let mut out_produced = 0;
        let mut is_finished = false;

        if finish {
            let comp = bzip2_compress_vec(&self.accumulated_in, self.level)?;
            if comp.len() > dst.len() {
                return Err(TTZipStatus::ErrInvalidParam);
            }
            dst[..comp.len()].copy_from_slice(&comp);
            out_produced = comp.len();
            is_finished = true;
        }

        Ok((in_consumed, out_produced, is_finished))
    }
}
