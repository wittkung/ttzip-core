// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, high-performance pure-Rust DEFLATE (RFC 1951), zlib (RFC 1950), and gzip (RFC 1952)
//! multi-level compression pipelines and high-level facade APIs.
//!
//! # Algorithmic Architecture
//!
//! - **Level 0 (Store)**: Pure RFC 1951 `BTYPE=00` uncompressed blocks at memory-bus speeds.
//! - **Level 1..=3 (Fast Greedy)**: Dual-slot [`HtMatchfinder`] hash table with 4-byte prefix matching.
//! - **Level 4..=9 (Lazy Evaluation)**: Dual [`HcMatchfinder`] 3-byte direct table + 4-byte hash chains with lazy match evaluation.
//! - **Level 10..=12 (Near-Optimal DP)**: Top-down [`BtMatchfinder`] binary trees + [`OptParser`] dynamic programming with EM refinement.

use crate::types::TTZipStatus;
use super::bt_matchfinder::{BtMatchfinder, DEFLATE_MAX_MATCH_LEN};
use super::checksum::{adler32_compute, crc32_compute};
use super::huffman::{
    deflate_make_huffman_code, FastBitWriterVec, PrecodeEncoder,
    DEFLATE_EXTRA_PRECODE_BITS, DEFLATE_PRECODE_LENS_PERMUTATION,
    MAX_LITLEN_CODEWORD_LEN, MAX_OFFSET_CODEWORD_LEN,
};
use super::matchfinder::{HcMatchfinder, HtMatchfinder};
use super::opt_parser::{
    build_matches_cache, optimize_parse_em, CostModel, SequenceItem,
    DEFLATE_END_OF_BLOCK, DEFLATE_FIRST_LEN_SYM, DEFLATE_NUM_LITLEN_SYMS,
    DEFLATE_NUM_OFFSET_SYMS, EXTRA_LENGTH_BITS, EXTRA_OFFSET_BITS, LENGTH_SLOT_MAP,
};
use crate::codecs::deflate::decompressor::{DeflateDecompressError, DeflateDecompressor};
use crate::codecs::deflate::{
    deflate_decompress as internal_deflate_decompress,
    gzip_decompress as internal_gzip_decompress,
    zlib_decompress as internal_zlib_decompress,
};

// MARK: - Constants

/// RFC 1951 Length slot base values (slots 0..=28 for symbols 257..=285).
pub const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
    131, 163, 195, 227, 258,
];

/// RFC 1951 Distance/Offset slot base values (slots 0..=29).
pub const OFFSET_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];

/// Maximum uncompressed block chunk size in bytes (64 KB).
const BLOCK_CHUNK_SIZE: usize = 65536;

// MARK: - Imports
use super::container::ContainerFormat;

// MARK: - Level 0: Pure Store Uncompressed Compression

/// Compresses data as pure RFC 1951 `BTYPE=00` Store uncompressed blocks.
pub fn deflate_compress_store(src: &[u8]) -> Vec<u8> {
    if src.is_empty() {
        return vec![0x01, 0x00, 0x00, 0xFF, 0xFF];
    }
    let mut out = Vec::with_capacity(src.len() + (src.len() / 65535 + 1) * 5);
    let mut offset = 0;
    while offset < src.len() {
        let chunk_len = (src.len() - offset).min(65535);
        let is_last = offset + chunk_len == src.len();
        let bhdr = if is_last { 0x01u8 } else { 0x00u8 };
        let len_u16 = chunk_len as u16;
        let nlen_u16 = !len_u16;
        out.push(bhdr);
        out.extend_from_slice(&len_u16.to_le_bytes());
        out.extend_from_slice(&nlen_u16.to_le_bytes());
        out.extend_from_slice(&src[offset..offset + chunk_len]);
        offset += chunk_len;
    }
    out
}

// MARK: - Dynamic Huffman Block Emitter

/// Encodes parsed sequence items into a dynamic Huffman DEFLATE block on `writer`.
pub(crate) fn encode_block_to_writer(
    writer: &mut FastBitWriterVec,
    seq: &[SequenceItem],
    is_last: bool,
) {
    // 1. Tally symbol frequencies
    let mut litlen_freqs = [0u32; DEFLATE_NUM_LITLEN_SYMS];
    let mut offset_freqs = [0u32; DEFLATE_NUM_OFFSET_SYMS];

    for item in seq {
        match *item {
            SequenceItem::Literal(lit) => {
                litlen_freqs[lit as usize] += 1;
            }
            SequenceItem::Match { length, offset } => {
                let slot = LENGTH_SLOT_MAP[length as usize] as usize;
                litlen_freqs[DEFLATE_FIRST_LEN_SYM + slot] += 1;
                let off_slot = CostModel::offset_slot(offset);
                offset_freqs[off_slot] += 1;
            }
        }
    }
    litlen_freqs[DEFLATE_END_OF_BLOCK] += 1;

    // 2. Generate canonical Huffman code lengths and codewords
    let mut litlen_lens = [0u8; DEFLATE_NUM_LITLEN_SYMS];
    let mut litlen_codewords = [0u32; DEFLATE_NUM_LITLEN_SYMS];
    deflate_make_huffman_code(
        DEFLATE_NUM_LITLEN_SYMS,
        MAX_LITLEN_CODEWORD_LEN,
        &litlen_freqs,
        &mut litlen_lens,
        &mut litlen_codewords,
    );

    let mut offset_lens = [0u8; DEFLATE_NUM_OFFSET_SYMS];
    let mut offset_codewords = [0u32; DEFLATE_NUM_OFFSET_SYMS];
    deflate_make_huffman_code(
        DEFLATE_NUM_OFFSET_SYMS,
        MAX_OFFSET_CODEWORD_LEN,
        &offset_freqs,
        &mut offset_lens,
        &mut offset_codewords,
    );

    // 3. Encode precode header
    let header = PrecodeEncoder::encode_header(&litlen_lens, &offset_lens);

    // 4. Emit block header: BFINAL (1 bit) + BTYPE=10 (2 bits, Dynamic Huffman)
    let bfinal_bit = if is_last { 1u64 } else { 0u64 };
    let block_header = bfinal_bit | (2u64 << 1);
    writer.add_bits(block_header, 3);
    writer.flush_bits();

    // HLIT, HDIST, HCLEN
    let hlit = (header.num_litlen_syms - 257) as u64;
    writer.add_bits(hlit, 5);

    let hdist = (header.num_offset_syms - 1) as u64;
    writer.add_bits(hdist, 5);

    let hclen = (header.num_explicit_lens - 4) as u64;
    writer.add_bits(hclen, 4);
    writer.flush_bits();

    // Precode lengths
    for i in 0..header.num_explicit_lens {
        let sym = DEFLATE_PRECODE_LENS_PERMUTATION[i] as usize;
        let code_len = header.precode_lens[sym] as u64;
        writer.add_bits(code_len, 3);
    }
    writer.flush_bits();

    // Precode items
    for &item in &header.items {
        let sym = (item & 0x1F) as usize;
        let extra = (item >> 5) as u64;
        let codeword = header.precode_codewords[sym] as u64;
        let len = header.precode_lens[sym] as u32;
        writer.add_bits(codeword, len);
        let extra_bits_count = DEFLATE_EXTRA_PRECODE_BITS[sym] as u32;
        if extra_bits_count > 0 {
            writer.add_bits(extra, extra_bits_count);
        }
        writer.flush_bits();
    }

    // 5. Emit compressed literal/length sequence
    for item in seq {
        match *item {
            SequenceItem::Literal(lit) => {
                let code = litlen_codewords[lit as usize] as u64;
                let len = litlen_lens[lit as usize] as u32;
                writer.add_bits(code, len);
                writer.flush_bits();
            }
            SequenceItem::Match { length, offset } => {
                let slot = LENGTH_SLOT_MAP[length as usize] as usize;
                let sym = DEFLATE_FIRST_LEN_SYM + slot;
                let code = litlen_codewords[sym] as u64;
                let len = litlen_lens[sym] as u32;
                writer.add_bits(code, len);

                let extra_len_bits = EXTRA_LENGTH_BITS[slot] as u32;
                if extra_len_bits > 0 {
                    let base = LENGTH_BASE[slot];
                    let extra_val = (length - base) as u64;
                    writer.add_bits(extra_val, extra_len_bits);
                }

                let off_slot = CostModel::offset_slot(offset);
                let off_code = offset_codewords[off_slot] as u64;
                let off_len = offset_lens[off_slot] as u32;
                writer.add_bits(off_code, off_len);

                let extra_off_bits = EXTRA_OFFSET_BITS[off_slot] as u32;
                if extra_off_bits > 0 {
                    let base = OFFSET_BASE[off_slot];
                    let extra_val = (offset - base) as u64;
                    writer.add_bits(extra_val, extra_off_bits);
                }
                writer.flush_bits();
            }
        }
    }

    // Emit End of Block symbol (256)
    let eob_code = litlen_codewords[DEFLATE_END_OF_BLOCK] as u64;
    let eob_len = litlen_lens[DEFLATE_END_OF_BLOCK] as u32;
    writer.add_bits(eob_code, eob_len);
    writer.flush_bits();
}

// MARK: - Match Parsing Pipelines

/// Parses input using `HtMatchfinder` for fast greedy compression (Levels 1..=3).
pub(crate) fn parse_greedy_ht(block: &[u8], level: i32) -> Vec<SequenceItem> {
    let mut mf = HtMatchfinder::new();
    let mut seq = Vec::with_capacity(block.len() / 2 + 8);
    let mut pos = 0;
    let nice_len = match level {
        1 => 8,
        2 => 16,
        _ => 32,
    };

    while pos < block.len() {
        if pos + 4 <= block.len() {
            let (len, offset) = mf.longest_match(block, pos, DEFLATE_MAX_MATCH_LEN, nice_len);
            if len >= 4 {
                seq.push(SequenceItem::Match {
                    length: len as u16,
                    offset: offset as u16,
                });
                mf.skip_bytes(block, pos + 1, len - 1);
                pos += len;
            } else {
                seq.push(SequenceItem::Literal(block[pos]));
                pos += 1;
            }
        } else {
            seq.push(SequenceItem::Literal(block[pos]));
            pos += 1;
        }
    }
    seq
}

/// Parses input using `HcMatchfinder` with Lazy match evaluation (Levels 4..=9).
pub(crate) fn parse_lazy_hc(block: &[u8], level: i32) -> Vec<SequenceItem> {
    let mut mf = HcMatchfinder::new();
    let mut seq = Vec::with_capacity(block.len() / 2 + 8);
    let mut pos = 0;

    let (max_search_depth, nice_len, max_lazy) = match level {
        4 => (4, 16, 4),
        5 => (8, 32, 16),
        6 => (16, 32, 32),
        7 => (32, 64, 64),
        8 => (64, 128, 128),
        _ => (128, 258, 258),
    };

    while pos < block.len() {
        if pos + 4 <= block.len() {
            let (len1, offset1) = mf.longest_match(
                block,
                pos,
                DEFLATE_MAX_MATCH_LEN,
                nice_len,
                max_search_depth,
            );
            if len1 >= 3 {
                if len1 >= nice_len || len1 >= max_lazy || pos + 1 >= block.len() {
                    seq.push(SequenceItem::Match {
                        length: len1 as u16,
                        offset: offset1 as u16,
                    });
                    if len1 > 1 {
                        mf.skip_bytes(block, pos + 1, len1 - 1);
                    }
                    pos += len1;
                } else {
                    let (len2, offset2) = mf.longest_match(
                        block,
                        pos + 1,
                        DEFLATE_MAX_MATCH_LEN,
                        nice_len,
                        max_search_depth,
                    );
                    if len2 > len1 {
                        seq.push(SequenceItem::Literal(block[pos]));
                        pos += 1;
                        seq.push(SequenceItem::Match {
                            length: len2 as u16,
                            offset: offset2 as u16,
                        });
                        if len2 > 1 {
                            mf.skip_bytes(block, pos + 1, len2 - 1);
                        }
                        pos += len2;
                    } else {
                        seq.push(SequenceItem::Match {
                            length: len1 as u16,
                            offset: offset1 as u16,
                        });
                        if len1 > 2 {
                            mf.skip_bytes(block, pos + 2, len1 - 2);
                        }
                        pos += len1;
                    }
                }
            } else {
                seq.push(SequenceItem::Literal(block[pos]));
                pos += 1;
            }
        } else {
            seq.push(SequenceItem::Literal(block[pos]));
            pos += 1;
        }
    }
    seq
}

/// Parses input using `BtMatchfinder` and near-optimal DP with EM refinement (Levels 10..=12).
pub(crate) fn parse_opt_bt(block: &[u8], level: i32) -> Vec<SequenceItem> {
    if block.is_empty() {
        return Vec::new();
    }
    let (nice_len, max_search_depth, max_passes) = match level {
        10 => (32, 32, 2),
        11 => (64, 64, 3),
        _ => (258, 128, 4),
    };
    let mut mf = BtMatchfinder::new();
    let cache = build_matches_cache(&mut mf, block, nice_len, max_search_depth);
    let (seq, _) = optimize_parse_em(block, &cache, max_passes);
    seq
}

// MARK: - Core Deflate Compression Engine

/// Compresses `src` using pure-Rust Libdeflate compression pipeline at the specified `level` (0..=12).
///
/// # Level Mapping
/// - `Level 0`: Pure Store uncompressed blocks (RFC 1951 BTYPE=00) at memory-bus speeds.
/// - `Level 1..=3`: `HtMatchfinder` fast greedy matching.
/// - `Level 4..=9`: `HcMatchfinder` hash chain lazy parsing.
/// - `Level 10..=12`: `BtMatchfinder` binary tree matchfinder with near-optimal DP parsing.
pub fn deflate_compress(src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
    let effective_level = if level < 0 { 6 } else { level.clamp(0, 12) };
    if effective_level == 0 {
        return Ok(deflate_compress_store(src));
    }

    let mut writer = FastBitWriterVec::with_capacity((src.len() / 2).max(128));
    let mut offset = 0;

    if src.is_empty() {
        encode_block_to_writer(&mut writer, &[], true);
        return Ok(writer.finish());
    }

    while offset < src.len() {
        let chunk_len = (src.len() - offset).min(BLOCK_CHUNK_SIZE);
        let is_last = offset + chunk_len == src.len();
        let chunk = &src[offset..offset + chunk_len];

        let seq = match effective_level {
            1..=3 => parse_greedy_ht(chunk, effective_level),
            4..=9 => parse_lazy_hc(chunk, effective_level),
            10..=12 => parse_opt_bt(chunk, effective_level),
            _ => parse_lazy_hc(chunk, 6),
        };

        encode_block_to_writer(&mut writer, &seq, is_last);
        offset += chunk_len;
    }

    Ok(writer.finish())
}

// MARK: - High-Level Facade APIs

/// High-level facade for raw RFC 1951 DEFLATE compression.
pub fn libdeflate_deflate_compress(src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
    deflate_compress(src, level)
}

/// High-level facade for raw RFC 1951 DEFLATE decompression into a pre-allocated buffer.
pub fn libdeflate_deflate_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    internal_deflate_decompress(src, dst)
}

/// High-level facade for RFC 1950 zlib compression (2-byte header + Deflate payload + Adler-32).
pub fn libdeflate_zlib_compress(src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
    let effective_level = if level < 0 { 6 } else { level.clamp(0, 12) };
    let cmf = 0x78u8;
    let flg = match effective_level {
        0 | 1 => 0x01u8,
        2..=5 => 0x5Eu8,
        6..=8 => 0x9Cu8,
        _ => 0xDAu8,
    };

    let deflate_payload = deflate_compress(src, effective_level)?;
    let adler = adler32_compute(src);

    let mut out = Vec::with_capacity(2 + deflate_payload.len() + 4);
    out.push(cmf);
    out.push(flg);
    out.extend_from_slice(&deflate_payload);
    out.extend_from_slice(&adler.to_be_bytes());
    Ok(out)
}

/// High-level facade for RFC 1950 zlib decompression into a pre-allocated buffer.
pub fn libdeflate_zlib_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    internal_zlib_decompress(src, dst)
}

/// High-level facade for RFC 1952 gzip compression (10-byte header + Deflate payload + CRC-32 + ISIZE).
pub fn libdeflate_gzip_compress(src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
    let effective_level = if level < 0 { 6 } else { level.clamp(0, 12) };
    let xfl = if effective_level >= 9 {
        2u8
    } else if effective_level <= 2 {
        4u8
    } else {
        0u8
    };

    let deflate_payload = deflate_compress(src, effective_level)?;
    let crc = crc32_compute(src);
    let isize = src.len() as u32;

    let mut out = Vec::with_capacity(10 + deflate_payload.len() + 8);
    // RFC 1952 Gzip 10-byte header
    out.extend_from_slice(&[0x1F, 0x8B, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, xfl, 0xFF]);
    out.extend_from_slice(&deflate_payload);
    // RFC 1952 Gzip 8-byte trailer
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&isize.to_le_bytes());
    Ok(out)
}

/// High-level facade for RFC 1952 gzip decompression into a pre-allocated buffer.
pub fn libdeflate_gzip_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    internal_gzip_decompress(src, dst)
}

pub fn libdeflate_validate(src: &[u8], format: ContainerFormat) -> Result<bool, TTZipStatus> {
    if src.is_empty() {
        return Ok(false);
    }
    match format {
        ContainerFormat::Raw => {
            let mut decompressor = DeflateDecompressor::new()?;
            let mut test_dst = [0u8; 4096];
            match decompressor.decompress_precise(src, &mut test_dst) {
                Ok(_) | Err(DeflateDecompressError::InsufficientSpace) => Ok(true),
                Err(DeflateDecompressError::BadData) | Err(DeflateDecompressError::ShortOutput) => {
                    Ok(false)
                }
            }
        }
        ContainerFormat::Zlib => {
            if src.len() < 6 {
                return Ok(false);
            }
            let cmf = src[0] as u16;
            let flg = src[1] as u16;
            if !(cmf * 256 + flg).is_multiple_of(31) {
                return Ok(false);
            }
            if (src[0] & 0x0F) != 8 || (src[0] >> 4) > 7 {
                return Ok(false);
            }
            let mut decompressor = DeflateDecompressor::new()?;
            let mut test_dst = [0u8; 4096];
            match decompressor.zlib_decompress_precise(src, &mut test_dst) {
                Ok(_) | Err(DeflateDecompressError::InsufficientSpace) => Ok(true),
                Err(DeflateDecompressError::BadData) | Err(DeflateDecompressError::ShortOutput) => {
                    Ok(false)
                }
            }
        }
        ContainerFormat::Gzip => {
            if src.len() < 18 {
                return Ok(false);
            }
            if src[0] != 0x1F || src[1] != 0x8B || src[2] != 8 {
                return Ok(false);
            }
            let mut decompressor = DeflateDecompressor::new()?;
            let mut test_dst = [0u8; 4096];
            match decompressor.gzip_decompress_precise(src, &mut test_dst) {
                Ok(_) | Err(DeflateDecompressError::InsufficientSpace) => Ok(true),
                Err(DeflateDecompressError::BadData) | Err(DeflateDecompressError::ShortOutput) => {
                    Ok(false)
                }
            }
        }
    }
}
