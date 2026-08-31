// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Thread-safe preloaded LZ4 dictionary context with fast and slow loading strategies.

use crate::codecs::lz4::dict::compressor::Lz4DictCompressor;
use crate::codecs::lz4::dict::decompress::{
    lz4_decompress_safe_ext_dict, lz4_decompress_safe_ext_dict_partial,
    lz4_decompress_safe_ext_dict_to_vec,
};
use crate::codecs::lz4::hash::lz4_hash4;
use crate::codecs::lz4::matchfinder::{LZ4_64K_LIMIT, LZ4_HASH_LOG, LZ4_HASH_SIZE, MINMATCH};
use crate::types::TTZipStatus;

#[inline(always)]
fn read_u32(src: &[u8], pos: usize) -> u32 {
    let b: [u8; 4] = src[pos..pos + 4].try_into().unwrap();
    u32::from_le_bytes(b)
}

// MARK: - Preloaded Dictionary Structure

/// Thread-safe preloaded LZ4 dictionary for zero-copy attached compression and decompression.
#[derive(Debug, Clone)]
pub struct Lz4PreloadedDict {
    dict_data: Vec<u8>,
    dict_id: Option<u32>,
    table: Box<[u32; LZ4_HASH_SIZE]>,
    effective_offset: usize,
    effective_len: usize,
}

unsafe impl Send for Lz4PreloadedDict {}
unsafe impl Sync for Lz4PreloadedDict {}

impl Lz4PreloadedDict {
    /// Creates a preloaded dictionary from raw bytes using fast strided indexing.
    pub fn new(dict_data: &[u8]) -> Self {
        Self::load_dict_fast(dict_data, None)
    }

    /// Creates a preloaded dictionary with an explicit numeric dictionary identifier.
    pub fn with_dict_id(dict_data: &[u8], dict_id: u32) -> Self {
        Self::load_dict_fast(dict_data, Some(dict_id))
    }

    /// Loads and indexes a dictionary using fast strided stepping (step = 3).
    pub fn load_dict_fast(dict_data: &[u8], dict_id: Option<u32>) -> Self {
        let total_len = dict_data.len();
        let max_window = LZ4_64K_LIMIT;
        let effective_len = total_len.min(max_window);
        let effective_offset = total_len.saturating_sub(effective_len);
        let effective_slice = &dict_data[effective_offset..effective_offset + effective_len];

        let mut table = Box::new([0u32; LZ4_HASH_SIZE]);

        if effective_len >= MINMATCH {
            let limit = effective_len - MINMATCH;
            let mut pos = 0usize;
            while pos <= limit {
                let seq = read_u32(effective_slice, pos);
                let h = lz4_hash4(seq, LZ4_HASH_LOG) as usize;
                table[h] = (pos + 1) as u32;
                pos += 3;
            }
            let last_pos = limit;
            let last_seq = read_u32(effective_slice, last_pos);
            let last_h = lz4_hash4(last_seq, LZ4_HASH_LOG) as usize;
            table[last_h] = (last_pos + 1) as u32;
        }

        Self {
            dict_data: dict_data.to_vec(),
            dict_id,
            table,
            effective_offset,
            effective_len,
        }
    }

    /// Loads and indexes a dictionary using 1-byte step secondary scan for 100% match coverage.
    pub fn load_dict_slow(dict_data: &[u8], dict_id: Option<u32>) -> Self {
        let total_len = dict_data.len();
        let max_window = LZ4_64K_LIMIT;
        let effective_len = total_len.min(max_window);
        let effective_offset = total_len.saturating_sub(effective_len);
        let effective_slice = &dict_data[effective_offset..effective_offset + effective_len];

        let mut table = Box::new([0u32; LZ4_HASH_SIZE]);

        if effective_len >= MINMATCH {
            let limit = effective_len - MINMATCH;
            for pos in 0..=limit {
                let seq = read_u32(effective_slice, pos);
                let h = lz4_hash4(seq, LZ4_HASH_LOG) as usize;
                table[h] = (pos + 1) as u32;
            }
        }

        Self {
            dict_data: dict_data.to_vec(),
            dict_id,
            table,
            effective_offset,
            effective_len,
        }
    }

    /// Returns the raw full dictionary slice.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.dict_data
    }

    /// Returns the effective active dictionary tail slice (up to 64KB).
    #[inline]
    pub fn effective_slice(&self) -> &[u8] {
        &self.dict_data[self.effective_offset..self.effective_offset + self.effective_len]
    }

    /// Returns the total length of the dictionary in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.dict_data.len()
    }

    /// Returns true if the dictionary is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.dict_data.is_empty()
    }

    /// Returns the optional dictionary ID.
    #[inline]
    pub fn dict_id(&self) -> Option<u32> {
        self.dict_id
    }

    /// Returns the precomputed 32768-entry dictionary hash table.
    #[inline]
    pub fn dict_table(&self) -> &[u32; LZ4_HASH_SIZE] {
        &self.table
    }

    /// Compresses a block using this preloaded dictionary.
    pub fn compress(
        &self,
        src: &[u8],
        dst: &mut [u8],
        acceleration: i32,
    ) -> Result<usize, TTZipStatus> {
        let mut compressor = Lz4DictCompressor::new();
        compressor.attach_dictionary(self).with_acceleration(acceleration);
        compressor.compress(src, dst)
    }

    /// Compresses a block into a newly allocated `Vec<u8>` using this preloaded dictionary.
    pub fn compress_to_vec(&self, src: &[u8], acceleration: i32) -> Result<Vec<u8>, TTZipStatus> {
        let mut compressor = Lz4DictCompressor::new();
        compressor.attach_dictionary(self).with_acceleration(acceleration);
        compressor.compress_to_vec(src)
    }

    /// Decompresses a block that was compressed using this preloaded dictionary.
    pub fn decompress(&self, src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
        lz4_decompress_safe_ext_dict(src, dst, self.as_slice())
    }

    /// Decompresses a block into a newly allocated `Vec<u8>` using this preloaded dictionary.
    pub fn decompress_to_vec(
        &self,
        src: &[u8],
        uncompressed_len: usize,
    ) -> Result<Vec<u8>, TTZipStatus> {
        lz4_decompress_safe_ext_dict_to_vec(src, uncompressed_len, self.as_slice())
    }

    /// Partially decompresses an LZ4 block until at least `target_output_size` bytes are decoded.
    pub fn decompress_partial(
        &self,
        src: &[u8],
        dst: &mut [u8],
        target_output_size: usize,
    ) -> Result<usize, TTZipStatus> {
        lz4_decompress_safe_ext_dict_partial(src, dst, self.as_slice(), target_output_size)
    }
}
