// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe-Rust Google Snappy block compression, decompression, and length probe.
//!
//! Powered by zero-copy SIMD and SWAR acceleration in `raw_encoder` and `raw_decoder`.

use crate::codecs::snappy::raw_decoder::{
    raw_decompress, raw_decompress_to_vec, raw_uncompressed_length, raw_validate,
};
use crate::codecs::snappy::raw_encoder::{
    max_compressed_len, raw_compress, raw_compress_to_vec,
};
use crate::codecs::snappy::varint::decode_varint32;
use crate::types::TTZipStatus;

/// Computes upper bound on compressed bytes for a given raw input size.
#[inline]
pub fn snappy_compress_bound(src_size: usize) -> usize {
    max_compressed_len(src_size)
}

/// Safely parses an unsigned varint (LEB128) from byte slice without panic or overflow.
///
/// Returns `Some((uncompressed_len, bytes_consumed))` on success. Max 5 bytes (u32 range) per Snappy spec.
#[inline]
pub fn parse_varint(src: &[u8]) -> Option<(usize, usize)> {
    decode_varint32(src).ok().map(|(val, len)| (val as usize, len))
}

/// Parses uncompressed length from a raw Snappy varint header safely.
#[inline]
pub fn snappy_uncompressed_length(src: &[u8]) -> Result<usize, TTZipStatus> {
    raw_uncompressed_length(src).map_err(Into::into)
}

/// Validates integrity of a Snappy compressed buffer with bounded uncompressed length limit in O(1) memory.
///
/// Executes a pure-Rust bytecode verification state machine with ZERO heap allocations.
#[inline]
pub fn snappy_validate_bounded(src: &[u8], max_uncompressed_len: usize) -> bool {
    raw_validate(src, max_uncompressed_len)
}

/// Validates integrity of a Snappy compressed buffer without decompressing or allocating heap memory.
#[inline]
pub fn snappy_validate(src: &[u8]) -> bool {
    raw_validate(src, usize::MAX)
}

/// Compresses a memory block using pure Rust Google Snappy raw format into a pre-allocated destination buffer.
#[inline]
pub fn snappy_compress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    raw_compress(src, dst).map_err(Into::into)
}

/// Decompresses a raw Snappy compressed block into a pre-allocated destination buffer.
#[inline]
pub fn snappy_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    raw_decompress(src, dst).map_err(Into::into)
}

/// Compresses a memory slice into a newly allocated `Vec<u8>`.
#[inline]
pub fn snappy_compress_to_vec(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    raw_compress_to_vec(src).map_err(Into::into)
}

/// Decompresses a raw Snappy slice into a newly allocated `Vec<u8>`.
#[inline]
pub fn snappy_decompress_to_vec(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    raw_decompress_to_vec(src).map_err(Into::into)
}
