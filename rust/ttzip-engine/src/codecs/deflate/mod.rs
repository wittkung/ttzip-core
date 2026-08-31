// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe RAII wrapper, modern zlib-ng matching, corpus evaluation, and dynamic scheduling for DEFLATE.
//!
//! Provides:
//! - Ultra-fast, zero-copy DEFLATE (RFC 1951), zlib (RFC 1950), and gzip (RFC 1952) compression.
//! - Modern zlib-ng style sliding window matchfinder ([`ZlibNgMatcher`]).
//! - 8 industrial-grade mathematical synthetic corpus generators ([`SyntheticCorpus`]).
//! - Sub-15ns dual-engine intelligent routing arbitrator ([`DeflateEngineArbitrator`]).
//! - Runtime dynamic level and RFC 1951 block type scheduler ([`DynamicLevelScheduler`]).

pub mod arbitrator;
pub mod compressor;
pub mod corpus_generators;
pub mod decompressor;
pub mod dynamic_level;
pub(crate) mod ffi;
mod pool;
pub mod zlib_ng_match;

#[cfg(test)]
mod tests;

pub use arbitrator::{DeflateEngineArbitrator, DeflateEngineChoice, DeflateWorkloadHint};
pub use compressor::{DeflateCompressor, DeflateStrategy};
pub use corpus_generators::{SyntheticCorpus, SyntheticCorpusKind};
pub use decompressor::{DeflateDecompressError, DeflateDecompressor};
pub use dynamic_level::{
    DeflateBlockType, DynamicLevelScheduler, PerformanceProfile, SchedulerMetrics,
};
pub use ffi::LibdeflateResult;
pub use pool::*;
pub use zlib_ng_match::{
    match_length_fast, DeflateToken, Match as ZlibNgMatch, MatcherConfig, ZlibNgMatcher,
    HASH_MASK, HASH_SIZE, MAX_MATCH, MIN_MATCH, WINDOW_MASK, WINDOW_SIZE,
};

use crate::types::TTZipStatus;

/// RFC 1951 Deflate sliding window size in bytes (32 KB = 32768 bytes).
pub const DEFLATE_WINDOW_SIZE_BYTES: usize = 32 * 1024;

/// Alias for RFC 1951 Deflate sliding window size.
pub const DEFLATE_SLIDING_WINDOW_SIZE: usize = DEFLATE_WINDOW_SIZE_BYTES;

// MARK: - High-Level Zero-Copy Helpers

/// Zero-copy raw DEFLATE compression using thread-local pooled compressor.
pub fn deflate_compress(src: &[u8], dst: &mut [u8], level: i32) -> Result<usize, TTZipStatus> {
    with_thread_local_compressor(level, |c| c.compress(src, dst))
}

/// Zero-copy raw DEFLATE decompression using thread-local pooled decompressor.
pub fn deflate_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    with_thread_local_decompressor(|d| d.decompress(src, dst))
}

/// Zero-copy zlib compression using thread-local pooled compressor.
pub fn zlib_compress(src: &[u8], dst: &mut [u8], level: i32) -> Result<usize, TTZipStatus> {
    with_thread_local_compressor(level, |c| c.zlib_compress(src, dst))
}

/// Zero-copy zlib decompression using thread-local pooled decompressor.
pub fn zlib_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    with_thread_local_decompressor(|d| d.zlib_decompress(src, dst))
}

/// Zero-copy gzip compression using thread-local pooled compressor.
pub fn gzip_compress(src: &[u8], dst: &mut [u8], level: i32) -> Result<usize, TTZipStatus> {
    with_thread_local_compressor(level, |c| c.gzip_compress(src, dst))
}

/// Zero-copy gzip decompression using thread-local pooled decompressor.
pub fn gzip_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    with_thread_local_decompressor(|d| d.gzip_decompress(src, dst))
}

/// Zero-allocation, constant-time upper bound calculation for raw DEFLATE compression.
#[inline]
pub fn deflate_compress_bound(in_len: usize, level: i32) -> usize {
    if level == 0 {
        if in_len == 0 {
            5
        } else {
            in_len + in_len.div_ceil(65535) * 5
        }
    } else {
        unsafe { ffi::libdeflate_deflate_compress_bound(std::ptr::null_mut(), in_len) }
    }
}

/// Zero-allocation, constant-time upper bound calculation for zlib compression.
#[inline]
pub fn zlib_compress_bound(in_len: usize, level: i32) -> usize {
    if level == 0 {
        deflate_compress_bound(in_len, 0) + 6
    } else {
        unsafe { ffi::libdeflate_zlib_compress_bound(std::ptr::null_mut(), in_len) }
    }
}

/// Zero-allocation, constant-time upper bound calculation for gzip compression.
#[inline]
pub fn gzip_compress_bound(in_len: usize, level: i32) -> usize {
    if level == 0 {
        deflate_compress_bound(in_len, 0) + 18
    } else {
        unsafe { ffi::libdeflate_gzip_compress_bound(std::ptr::null_mut(), in_len) }
    }
}
