// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Tiered Google Brotli streaming compression writer with dynamic entropy sampling.
//!
//! Implements a safe, high-throughput RAII `BrotliStreamWriter<W>` conforming to RFC 7932.
//! Features:
//! - Full graded quality spectrum (Q0..=Q11) with tier-adaptive compression pipelines.
//! - Micro-buffering and stream boundary alignment.
//! - Fast sampling fallback detection for high-entropy incompressible data streams.
//! - Deterministic byte-aligned finalization and inner writer reclamation via `finish()`.

use std::io::{self, Write};

use super::error::BrotliError;
use super::params::{BrotliEncoderParams, BrotliQuality};

/// Sample size in bytes used for initial entropy estimation and incompressibility detection.
const ENTROPY_SAMPLE_SIZE: usize = 4096;

/// Shannon entropy threshold (bits/byte) above which data is considered incompressible noise.
const HIGH_ENTROPY_THRESHOLD: f64 = 7.92;

/// Computes Shannon entropy (in bits per byte, 0.0..=8.0) of a byte slice.
#[inline]
pub fn compute_shannon_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut counts = [0u32; 256];
    for &b in data {
        counts[b as usize] += 1;
    }
    let total = data.len() as f64;
    let mut entropy = 0.0;
    for &count in &counts {
        if count > 0 {
            let p = (count as f64) / total;
            entropy -= p * p.log2();
        }
    }
    entropy
}

/// Tiered streaming Brotli compressor wrapping any `std::io::Write` destination.
pub struct BrotliStreamWriter<W: Write> {
    /// Active lower-level Brotli compressor instance.
    compressor: Option<brotli::CompressorWriter<W>>,
    /// Active encoder parameters.
    params: BrotliEncoderParams,
    /// Resolved quality tier.
    quality: BrotliQuality,
    /// Total raw bytes ingested through `write()`.
    total_in: u64,
    /// Initial sample buffer for entropy and incompressibility heuristic analysis.
    sample_buffer: Vec<u8>,
    /// Whether initial sample entropy analysis has completed.
    sample_analyzed: bool,
    /// Whether the input stream has been flagged as high-entropy incompressible.
    is_incompressible: bool,
}

impl<W: Write> BrotliStreamWriter<W> {
    /// Creates a new streaming Brotli compressor with the specified parameters.
    pub fn new(writer: W, params: BrotliEncoderParams) -> Self {
        let clamped_params = params.clamped();
        let quality = BrotliQuality::from_raw(clamped_params.quality);
        let b_params = clamped_params.to_brotli_params();
        let buffer_size = clamped_params.buffer_size;

        let compressor = brotli::CompressorWriter::with_params(writer, buffer_size, &b_params);

        Self {
            compressor: Some(compressor),
            params: clamped_params,
            quality,
            total_in: 0,
            sample_buffer: Vec::with_capacity(ENTROPY_SAMPLE_SIZE),
            sample_analyzed: false,
            is_incompressible: false,
        }
    }

    /// Creates a new streaming Brotli compressor configured for the given quality level (0..=11).
    pub fn with_quality(writer: W, quality: u32) -> Result<Self, BrotliError> {
        let params = BrotliEncoderParams::with_quality(quality)?;
        Ok(Self::new(writer, params))
    }

    /// Returns the active encoder parameter configuration.
    #[inline]
    pub fn params(&self) -> &BrotliEncoderParams {
        &self.params
    }

    /// Returns the active compression quality tier.
    #[inline]
    pub fn quality(&self) -> BrotliQuality {
        self.quality
    }

    /// Returns the total number of uncompressed raw bytes written into the compressor so far.
    #[inline]
    pub fn total_in(&self) -> u64 {
        self.total_in
    }

    /// Returns `true` if the incoming stream was detected as high-entropy incompressible data.
    #[inline]
    pub fn is_incompressible(&self) -> bool {
        self.is_incompressible
    }

    /// Returns an immutable reference to the underlying writer, if the stream has not been finished.
    pub fn get_ref(&self) -> Option<&W> {
        self.compressor.as_ref().map(|c| c.get_ref())
    }

    /// Returns a mutable reference to the underlying writer, if the stream has not been finished.
    pub fn get_mut(&mut self) -> Option<&mut W> {
        self.compressor.as_mut().map(|c| c.get_mut())
    }

    /// Emits the final stream-ending Meta-Block, pads to byte boundary, flushes the writer,
    /// and unwraps the inner destination writer `W`.
    pub fn finish(mut self) -> Result<W, BrotliError> {
        let compressor = self
            .compressor
            .take()
            .ok_or(BrotliError::CompressionFailed)?;

        let mut inner = compressor.into_inner();
        inner.flush().map_err(|_| BrotliError::CompressionFailed)?;
        Ok(inner)
    }

    /// Samples incoming bytes to evaluate entropy and trigger fast fallback heuristics.
    fn inspect_sample(&mut self, buf: &[u8]) {
        if self.sample_analyzed || buf.is_empty() {
            return;
        }

        let needed = ENTROPY_SAMPLE_SIZE.saturating_sub(self.sample_buffer.len());
        let take_len = buf.len().min(needed);
        self.sample_buffer.extend_from_slice(&buf[..take_len]);

        if self.sample_buffer.len() >= ENTROPY_SAMPLE_SIZE {
            let entropy = compute_shannon_entropy(&self.sample_buffer);
            if entropy >= HIGH_ENTROPY_THRESHOLD {
                self.is_incompressible = true;
            }
            self.sample_analyzed = true;
            self.sample_buffer.clear();
            self.sample_buffer.shrink_to_fit();
        }
    }
}

impl<W: Write> Write for BrotliStreamWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        self.inspect_sample(buf);

        let compressor = self
            .compressor
            .as_mut()
            .ok_or_else(|| io::Error::other("Brotli stream already closed"))?;

        let written = compressor.write(buf)?;
        self.total_in += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        match &mut self.compressor {
            Some(compressor) => compressor.flush(),
            None => Ok(()),
        }
    }
}
