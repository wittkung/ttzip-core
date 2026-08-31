// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, high-performance Zopfli multi-format encoder supporting Raw Deflate, Zlib, and Gzip.
//!
//! # Supported RFC Standards
//!
//! - **RFC 1951 (Raw DEFLATE)**: Optimal dynamic Huffman blocks without headers or trailers.
//! - **RFC 1950 (Zlib)**: CMF/FLG container with 32 KB window and Big-Endian Adler-32 verification.
//! - **RFC 1952 (Gzip)**: 10-byte Unix container header with Little-Endian CRC-32 and ISIZE trailer.

use super::block_split::ZopfliBlockSplitter;
use super::shortest_path::{
    get_dist_slot, ZopfliToken, END_OF_BLOCK_SYM, EXTRA_LENGTH_BITS, EXTRA_OFFSET_BITS,
    FIRST_LEN_SYM, LENGTH_BASE, LENGTH_SLOT_MAP, OFFSET_BASE,
};
use super::squeeze::{BlockStats, ZopfliOptions, ZopfliSqueeze};
use crate::codecs::libdeflate::huffman::{
    compute_num_explicit_precode_lens, compute_precode_items, deflate_make_huffman_code,
    FastBitWriterVec, DEFLATE_EXTRA_PRECODE_BITS, DEFLATE_NUM_PRECODE_SYMS,
    DEFLATE_PRECODE_LENS_PERMUTATION,
};
use crate::crypto::{adler32, crc32};
use crate::types::TTZipStatus;

// MARK: - Container Formats

/// Compression container wrapper format for Zopfli encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZopfliFormat {
    /// Pure RFC 1951 raw DEFLATE stream.
    Deflate,
    /// RFC 1950 zlib stream with Adler-32 checksum.
    Zlib,
    /// RFC 1952 gzip stream with CRC-32 and input byte length trailer.
    Gzip,
}

// MARK: - Dynamic Block Emission

/// Emits a single RFC 1951 dynamic Huffman block into the bitwriter.
fn emit_dynamic_block(
    writer: &mut FastBitWriterVec,
    stats: &BlockStats,
    is_last: bool,
) {
    // 1. Block header: BFINAL (1 bit), BTYPE = 10 dynamic (2 bits)
    let bfinal = if is_last { 1u64 } else { 0u64 };
    writer.add_bits(bfinal, 1);
    writer.add_bits(2, 2);
    writer.flush_bits();

    // 2. Precode tree headers
    let mut combined_lens = Vec::with_capacity(stats.num_litlen_syms + stats.num_dist_syms);
    combined_lens.extend_from_slice(&stats.litlen_lens[..stats.num_litlen_syms]);
    combined_lens.extend_from_slice(&stats.dist_lens[..stats.num_dist_syms]);

    let mut precode_freqs = [0u32; DEFLATE_NUM_PRECODE_SYMS];
    let mut precode_items = Vec::with_capacity(combined_lens.len());
    compute_precode_items(&combined_lens, &mut precode_freqs, &mut precode_items);

    let mut precode_lens = [0u8; DEFLATE_NUM_PRECODE_SYMS];
    let mut precode_codes = [0u32; DEFLATE_NUM_PRECODE_SYMS];
    deflate_make_huffman_code(
        DEFLATE_NUM_PRECODE_SYMS,
        7,
        &precode_freqs,
        &mut precode_lens,
        &mut precode_codes,
    );

    let num_explicit_precode = compute_num_explicit_precode_lens(&precode_lens);

    // HLIT (5 bits), HDIST (5 bits), HCLEN (4 bits)
    let hlit = (stats.num_litlen_syms - 257) as u64;
    let hdist = (stats.num_dist_syms - 1) as u64;
    let hclen = (num_explicit_precode - 4) as u64;

    writer.add_bits(hlit, 5);
    writer.add_bits(hdist, 5);
    writer.add_bits(hclen, 4);
    writer.flush_bits();

    // Precode code lengths
    for i in 0..num_explicit_precode {
        let sym = DEFLATE_PRECODE_LENS_PERMUTATION[i] as usize;
        writer.add_bits(precode_lens[sym] as u64, 3);
    }
    writer.flush_bits();

    // Precode encoded code lengths
    for &item in &precode_items {
        let sym = (item & 0x1F) as usize;
        let extra = (item >> 5) as u64;
        let extra_bits = DEFLATE_EXTRA_PRECODE_BITS[sym] as u32;

        writer.add_bits(precode_codes[sym] as u64, precode_lens[sym] as u32);
        if extra_bits > 0 {
            writer.add_bits(extra, extra_bits);
        }
        writer.flush_bits();
    }

    // 3. Emit compressed LZ77 data stream
    for token in &stats.tokens {
        match *token {
            ZopfliToken::Literal(lit) => {
                let l = lit as usize;
                writer.add_bits(stats.litlen_codes[l] as u64, stats.litlen_lens[l] as u32);
                writer.flush_bits();
            }
            ZopfliToken::Match { length, distance } => {
                let lslot = LENGTH_SLOT_MAP[length as usize] as usize;
                let len_sym = FIRST_LEN_SYM + lslot;
                writer.add_bits(stats.litlen_codes[len_sym] as u64, stats.litlen_lens[len_sym] as u32);

                let extra_len_bits = EXTRA_LENGTH_BITS[lslot] as u32;
                if extra_len_bits > 0 {
                    let base = LENGTH_BASE[lslot];
                    let extra_val = (length - base) as u64;
                    writer.add_bits(extra_val, extra_len_bits);
                }

                let dslot = get_dist_slot(distance);
                writer.add_bits(stats.dist_codes[dslot] as u64, stats.dist_lens[dslot] as u32);

                let extra_dist_bits = EXTRA_OFFSET_BITS[dslot] as u32;
                if extra_dist_bits > 0 {
                    let base = OFFSET_BASE[dslot];
                    let extra_val = (distance - base) as u64;
                    writer.add_bits(extra_val, extra_dist_bits);
                }
                writer.flush_bits();
            }
        }
    }

    // 4. Emit End of Block (EOB = 256)
    writer.add_bits(stats.litlen_codes[END_OF_BLOCK_SYM] as u64, stats.litlen_lens[END_OF_BLOCK_SYM] as u32);
    writer.flush_bits();
}

// MARK: - Zopfli Encoder Structure

/// Safe, high-performance Zopfli multi-format encoder.
pub struct ZopfliEncoder {
    options: ZopfliOptions,
}

impl Default for ZopfliEncoder {
    fn default() -> Self {
        Self::new(ZopfliOptions::default())
    }
}

impl ZopfliEncoder {
    /// Creates a new `ZopfliEncoder` with specified options.
    pub fn new(options: ZopfliOptions) -> Self {
        Self { options }
    }

    /// Compresses input `data` using specified `format`.
    pub fn compress(&self, data: &[u8], format: ZopfliFormat) -> Result<Vec<u8>, TTZipStatus> {
        let mut out = Vec::with_capacity(data.len() / 2 + 64);

        // 1. Container Header
        match format {
            ZopfliFormat::Deflate => {}
            ZopfliFormat::Zlib => {
                // RFC 1950 header: CMF = 0x78 (32KB window, Deflate), FLG = 0xDA (max compression, check mod 31 == 0)
                out.push(0x78);
                out.push(0xDA);
            }
            ZopfliFormat::Gzip => {
                // RFC 1952 header (10 bytes)
                out.push(0x1F); // ID1
                out.push(0x8B); // ID2
                out.push(0x08); // CM = Deflate
                out.push(0x00); // FLG = 0
                out.extend_from_slice(&[0, 0, 0, 0]); // MTIME
                out.push(0x02); // XFL = 2 (maximum compression)
                out.push(0x03); // OS = 3 (Unix)
            }
        }

        // 2. Deflate Payload
        if data.is_empty() {
            // RFC 1951 fixed Huffman empty block (BFINAL=1, BTYPE=01, EOB=0b0000000 -> 0x03, 0x00)
            out.push(0x03);
            out.push(0x00);
        } else {
            // Partition input into optimal blocks
            let block_ranges = ZopfliBlockSplitter::split_into_ranges(
                data,
                0,
                data.len(),
                self.options.max_block_splits,
            );

            let mut bit_writer = FastBitWriterVec::with_capacity(data.len() / 2 + 32);
            let mut squeeze = ZopfliSqueeze::new();
            let num_blocks = block_ranges.len();

            for (idx, &(from, to)) in block_ranges.iter().enumerate() {
                let is_last = idx + 1 == num_blocks;
                let stats = squeeze.squeeze(data, from, to, &self.options);
                emit_dynamic_block(&mut bit_writer, &stats, is_last);
            }

            let deflate_payload = bit_writer.finish();
            out.extend_from_slice(&deflate_payload);
        }

        // 3. Container Trailer
        match format {
            ZopfliFormat::Deflate => {}
            ZopfliFormat::Zlib => {
                let checksum = adler32(data);
                out.extend_from_slice(&checksum.to_be_bytes());
            }
            ZopfliFormat::Gzip => {
                let checksum = crc32(data);
                let isize = (data.len() as u32).to_le_bytes();
                out.extend_from_slice(&checksum.to_le_bytes());
                out.extend_from_slice(&isize);
            }
        }

        Ok(out)
    }

    /// Compresses data as raw RFC 1951 Deflate.
    #[inline]
    pub fn compress_deflate(&self, data: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
        self.compress(data, ZopfliFormat::Deflate)
    }

    /// Compresses data as RFC 1950 Zlib.
    #[inline]
    pub fn compress_zlib(&self, data: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
        self.compress(data, ZopfliFormat::Zlib)
    }

    /// Compresses data as RFC 1952 Gzip.
    #[inline]
    pub fn compress_gzip(&self, data: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
        self.compress(data, ZopfliFormat::Gzip)
    }
}

// MARK: - Facade Convenience Functions

/// Compresses a buffer with Zopfli using specified format and options.
pub fn zopfli_compress(
    src: &[u8],
    format: ZopfliFormat,
    options: &ZopfliOptions,
) -> Result<Vec<u8>, TTZipStatus> {
    let encoder = ZopfliEncoder::new(*options);
    encoder.compress(src, format)
}

/// Compresses a buffer with Zopfli as raw Deflate.
pub fn zopfli_compress_deflate(
    src: &[u8],
    options: &ZopfliOptions,
) -> Result<Vec<u8>, TTZipStatus> {
    zopfli_compress(src, ZopfliFormat::Deflate, options)
}

/// Compresses a buffer with Zopfli as Zlib container.
pub fn zopfli_compress_zlib(
    src: &[u8],
    options: &ZopfliOptions,
) -> Result<Vec<u8>, TTZipStatus> {
    zopfli_compress(src, ZopfliFormat::Zlib, options)
}

/// Compresses a buffer with Zopfli as Gzip container.
pub fn zopfli_compress_gzip(
    src: &[u8],
    options: &ZopfliOptions,
) -> Result<Vec<u8>, TTZipStatus> {
    zopfli_compress(src, ZopfliFormat::Gzip, options)
}
