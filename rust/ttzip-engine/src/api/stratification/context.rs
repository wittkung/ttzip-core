// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Layer 2: Context API (Explicit Context Reuse & Zero-Allocation Reset).

use crate::api::stratification::simple::{
    simple_compress_bound, simple_compress_to_slice, simple_decompress_to_slice,
};
use crate::types::{TTZipArchiveFormat, TTZipCompressionLevel, TTZipStatus};

/// Reusable compression context maintaining internal scratch buffers to avoid per-call heap allocations.
pub struct CompressionContext {
    format: TTZipArchiveFormat,
    level: TTZipCompressionLevel,
    scratch: Vec<u8>,
}

impl CompressionContext {
    /// Creates a new reusable compression context for the specified format and compression level.
    #[must_use]
    pub fn new(format: TTZipArchiveFormat, level: TTZipCompressionLevel) -> Self {
        Self {
            format,
            level,
            scratch: Vec::with_capacity(64 * 1024),
        }
    }

    /// Returns the active archive / compression format.
    #[inline]
    #[must_use]
    pub fn format(&self) -> TTZipArchiveFormat {
        self.format
    }

    /// Returns the active compression level.
    #[inline]
    #[must_use]
    pub fn level(&self) -> TTZipCompressionLevel {
        self.level
    }

    /// Pre-allocates scratch memory capacity.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.scratch.reserve(additional);
    }

    /// Returns the currently allocated internal scratch buffer capacity in bytes.
    #[inline]
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.scratch.capacity()
    }

    /// Resets internal scratch buffers and transient states without deallocating backing heap capacity.
    #[inline]
    pub fn reset(&mut self) {
        self.scratch.clear();
    }

    /// Compresses `src` directly into the caller-supplied `dst` slice.
    #[inline]
    pub fn compress_to_slice(&mut self, src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
        simple_compress_to_slice(src, dst, self.format, self.level)
    }

    /// Compresses `src` appending output into `out` vector, resizing as needed.
    pub fn compress_to_vec(&mut self, src: &[u8], out: &mut Vec<u8>) -> Result<usize, TTZipStatus> {
        let bound = simple_compress_bound(src.len(), self.format, self.level).max(64);
        let start_len = out.len();
        out.resize(start_len + bound, 0);
        let written = self.compress_to_slice(src, &mut out[start_len..])?;
        out.truncate(start_len + written);
        Ok(written)
    }
}

/// Reusable decompression context maintaining internal scratch buffers for zero-allocation reuse.
pub struct DecompressionContext {
    format: TTZipArchiveFormat,
    scratch: Vec<u8>,
}

impl DecompressionContext {
    /// Creates a new reusable decompression context for the specified format.
    #[must_use]
    pub fn new(format: TTZipArchiveFormat) -> Self {
        Self {
            format,
            scratch: Vec::with_capacity(64 * 1024),
        }
    }

    /// Returns the active archive / decompression format.
    #[inline]
    #[must_use]
    pub fn format(&self) -> TTZipArchiveFormat {
        self.format
    }

    /// Pre-allocates scratch memory capacity.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.scratch.reserve(additional);
    }

    /// Returns the currently allocated internal scratch buffer capacity in bytes.
    #[inline]
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.scratch.capacity()
    }

    /// Resets internal scratch buffers without freeing heap allocation.
    #[inline]
    pub fn reset(&mut self) {
        self.scratch.clear();
    }

    /// Decompresses `src` directly into the caller-supplied `dst` slice.
    #[inline]
    pub fn decompress_to_slice(&mut self, src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
        simple_decompress_to_slice(src, dst, self.format)
    }

    /// Decompresses `src` appending output into `out` vector.
    pub fn decompress_to_vec(&mut self, src: &[u8], out: &mut Vec<u8>) -> Result<usize, TTZipStatus> {
        let start_len = out.len();
        let estimated_cap = src.len().saturating_mul(4).max(1024);
        out.resize(start_len + estimated_cap, 0);

        for _ in 0..6 {
            match self.decompress_to_slice(src, &mut out[start_len..]) {
                Ok(written) => {
                    out.truncate(start_len + written);
                    return Ok(written);
                }
                Err(TTZipStatus::ErrExtractionFailed) | Err(TTZipStatus::ErrInvalidParam) => {
                    let cur_len = out.len() - start_len;
                    let new_len = cur_len.saturating_mul(2).min(1024 * 1024 * 1024);
                    if new_len == cur_len {
                        return Err(TTZipStatus::ErrExtractionFailed);
                    }
                    out.resize(start_len + new_len, 0);
                }
                Err(status) => return Err(status),
            }
        }
        Err(TTZipStatus::ErrExtractionFailed)
    }
}
