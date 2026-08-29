// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Micro-Buffer Chunking & Streaming Escape Validation Engine (`test_chunked`).
//!
//! Conforms to `test_custom_decompressor.c` validation philosophy:
//! - Stresses streaming decompressors under extreme, pathological output chunking steps:
//!   `MICRO_CHUNK_STEPS: &[usize] = &[1, 2, 3, 7, 15, 259, 1024, 4096]`.
//! - Validates that decompression state machines properly suspend and resume at arbitrary
//!   byte boundaries (`avail_out = 1, 2, 3, 7, 15, 259`), maintaining exact 100% byte-for-byte
//!   fidelity against original uncompressed payloads upon reaching EOF.
//! - Exercises fixed-step chunking, staircase variable chunking, and dual-ended input/output slicing.

use std::io::{Cursor, Read};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::codecs::{
    brotli::stream::BrotliDecompressorReader,
    snappy::frame::is_framed_snappy,
    zstd::stream::ZstdStreamReader,
};
use crate::types::TTZipStatus;

/// Pathological micro-buffer output chunking ladder steps.
pub const MICRO_CHUNK_STEPS: &[usize] = &[1, 2, 3, 7, 15, 259, 1024, 4096];

/// Default stress test staircase chunk pattern for variable-step evaluation.
pub const STAIRCASE_CHUNK_PATTERN: &[usize] = &[1, 3, 7, 2, 15, 259, 1, 7, 1024, 3, 4096];

/// Supported streaming codec formats for micro-chunk validation.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MicroChunkCodec {
    /// Zstandard (.zst) streaming frame.
    Zstd,
    /// Brotli (.br) streaming stream.
    Brotli,
    /// Snappy (.sz) framed stream.
    SnappyFramed,
    /// Raw uncompressed / passthrough stream.
    RawPassthrough,
}

impl MicroChunkCodec {
    /// Canonical name of the codec format.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Zstd => "Zstandard",
            Self::Brotli => "Brotli",
            Self::SnappyFramed => "SnappyFramed",
            Self::RawPassthrough => "RawPassthrough",
        }
    }
}

/// Detailed outcome metrics for a single micro-chunk decompression pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MicroChunkPassResult {
    /// Codec format evaluated.
    pub codec: MicroChunkCodec,
    /// Chunk step size used for this pass.
    pub chunk_size: usize,
    /// Total uncompressed bytes recovered.
    pub bytes_decompressed: usize,
    /// Expected uncompressed payload length.
    pub expected_bytes: usize,
    /// Total number of distinct `read()` iterations executed.
    pub read_iterations: usize,
    /// Execution duration in nanoseconds.
    pub duration_nanos: u64,
    /// Whether the decompressed stream perfectly matches the original payload byte-for-byte.
    pub passed: bool,
    /// Error description if the pass failed.
    pub error: Option<String>,
}

/// Comprehensive multi-step micro-chunk validation report for a single codec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MicroChunkCodecReport {
    /// Codec format evaluated.
    pub codec: MicroChunkCodec,
    /// Individual pass results for each step in `MICRO_CHUNK_STEPS`.
    pub step_results: Vec<MicroChunkPassResult>,
    /// Pass result for the variable staircase stress test.
    pub staircase_result: Option<MicroChunkPassResult>,
    /// Whether all configured steps and tests passed with 100% fidelity.
    pub all_passed: bool,
}

/// Bounded stream wrapper enforcing maximum read chunk size per `read()` invocation.
pub struct MicroChunkBoundedReader<R: Read> {
    inner: R,
    max_chunk_size: usize,
}

impl<R: Read> MicroChunkBoundedReader<R> {
    /// Creates a new bounded reader clamping reads to `max_chunk_size`.
    pub fn new(inner: R, max_chunk_size: usize) -> Self {
        Self {
            inner,
            max_chunk_size: max_chunk_size.max(1),
        }
    }

    /// Updates the maximum allowable chunk size dynamically.
    pub fn set_max_chunk_size(&mut self, max_chunk_size: usize) {
        self.max_chunk_size = max_chunk_size.max(1);
    }

    /// Consumes the adapter, returning the underlying reader.
    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl<R: Read> Read for MicroChunkBoundedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let clamped_len = buf.len().min(self.max_chunk_size);
        self.inner.read(&mut buf[..clamped_len])
    }
}

/// Core engine for micro-buffer chunking and streaming state-machine validation.
pub struct MicroChunkStreamValidator;

impl MicroChunkStreamValidator {
    /// Validates an arbitrary `std::io::Read` decompressor using a fixed micro-chunk step size.
    ///
    /// Iterates `reader.read(&mut buf[..chunk_size])` until EOF, accumulating decompressed bytes
    /// and performing a byte-for-byte equality assertion against `expected_payload`.
    pub fn validate_reader<R: Read>(
        mut reader: R,
        expected_payload: &[u8],
        chunk_size: usize,
        codec: MicroChunkCodec,
    ) -> MicroChunkPassResult {
        let chunk_size = chunk_size.max(1);
        let mut buffer = vec![0u8; chunk_size];
        let mut decompressed = Vec::with_capacity(expected_payload.len());
        let mut read_iterations = 0;
        let start = Instant::now();

        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    read_iterations += 1;
                    decompressed.extend_from_slice(&buffer[..n]);
                }
                Err(err) => {
                    let elapsed = start.elapsed().as_nanos() as u64;
                    return MicroChunkPassResult {
                        codec,
                        chunk_size,
                        bytes_decompressed: decompressed.len(),
                        expected_bytes: expected_payload.len(),
                        read_iterations,
                        duration_nanos: elapsed,
                        passed: false,
                        error: Some(format!("I/O read error at chunk #{}: {}", read_iterations, err)),
                    };
                }
            }
        }

        let elapsed = start.elapsed().as_nanos() as u64;
        let matches = decompressed == expected_payload;
        let error = if !matches {
            Some(format!(
                "Decompressed payload mismatch: got {} bytes, expected {} bytes",
                decompressed.len(),
                expected_payload.len()
            ))
        } else {
            None
        };

        MicroChunkPassResult {
            codec,
            chunk_size,
            bytes_decompressed: decompressed.len(),
            expected_bytes: expected_payload.len(),
            read_iterations,
            duration_nanos: elapsed,
            passed: matches,
            error,
        }
    }

    /// Validates an arbitrary `std::io::Read` decompressor using a variable staircase chunk pattern.
    pub fn validate_reader_staircase<R: Read>(
        mut reader: R,
        expected_payload: &[u8],
        pattern: &[usize],
        codec: MicroChunkCodec,
    ) -> MicroChunkPassResult {
        let pattern = if pattern.is_empty() {
            STAIRCASE_CHUNK_PATTERN
        } else {
            pattern
        };

        let max_pattern_step = pattern.iter().copied().max().unwrap_or(4096);
        let mut buffer = vec![0u8; max_pattern_step];
        let mut decompressed = Vec::with_capacity(expected_payload.len());
        let mut read_iterations = 0;
        let start = Instant::now();

        loop {
            let step = pattern[read_iterations % pattern.len()].max(1);
            match reader.read(&mut buffer[..step]) {
                Ok(0) => break,
                Ok(n) => {
                    read_iterations += 1;
                    decompressed.extend_from_slice(&buffer[..n]);
                }
                Err(err) => {
                    let elapsed = start.elapsed().as_nanos() as u64;
                    return MicroChunkPassResult {
                        codec,
                        chunk_size: 0,
                        bytes_decompressed: decompressed.len(),
                        expected_bytes: expected_payload.len(),
                        read_iterations,
                        duration_nanos: elapsed,
                        passed: false,
                        error: Some(format!("I/O read error at staircase chunk #{}: {}", read_iterations, err)),
                    };
                }
            }
        }

        let elapsed = start.elapsed().as_nanos() as u64;
        let matches = decompressed == expected_payload;
        let error = if !matches {
            Some(format!(
                "Staircase decompressed payload mismatch: got {} bytes, expected {} bytes",
                decompressed.len(),
                expected_payload.len()
            ))
        } else {
            None
        };

        MicroChunkPassResult {
            codec,
            chunk_size: 0,
            bytes_decompressed: decompressed.len(),
            expected_bytes: expected_payload.len(),
            read_iterations,
            duration_nanos: elapsed,
            passed: matches,
            error,
        }
    }

    /// Constructs a format-specific streaming reader from compressed bytes.
    pub fn create_codec_reader(
        codec: MicroChunkCodec,
        compressed_data: &[u8],
    ) -> Result<Box<dyn Read + '_>, TTZipStatus> {
        let cursor = Cursor::new(compressed_data);
        match codec {
            MicroChunkCodec::Zstd => {
                let reader = ZstdStreamReader::new(cursor)
                    .map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                Ok(Box::new(reader))
            }
            MicroChunkCodec::Brotli => {
                let reader = BrotliDecompressorReader::new(cursor, 4096);
                Ok(Box::new(reader))
            }
            MicroChunkCodec::SnappyFramed => {
                if !is_framed_snappy(compressed_data) && !compressed_data.is_empty() {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
                let reader = snap::read::FrameDecoder::new(cursor);
                Ok(Box::new(reader))
            }
            MicroChunkCodec::RawPassthrough => Ok(Box::new(cursor)),
        }
    }

    /// Compresses `payload` using the specified codec to produce a valid streaming stream.
    pub fn compress_codec(codec: MicroChunkCodec, payload: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
        match codec {
            MicroChunkCodec::Zstd => {
                use crate::codecs::zstd::stream::ZstdStreamWriter;
                let mut writer = ZstdStreamWriter::with_level(Vec::new(), 3)?;
                std::io::Write::write_all(&mut writer, payload)
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                writer.finish()
            }
            MicroChunkCodec::Brotli => {
                use crate::codecs::brotli::stream::{BrotliCompressorWriter, BrotliConfig};
                let config = BrotliConfig {
                    quality: 6,
                    lgwin: 22,
                    buffer_size: 4096,
                };
                let mut out = Vec::new();
                let mut writer = BrotliCompressorWriter::new(&mut out, &config);
                std::io::Write::write_all(&mut writer, payload)
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                std::io::Write::flush(&mut writer)
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                drop(writer);
                Ok(out)
            }
            MicroChunkCodec::SnappyFramed => {
                use crate::codecs::snappy::frame::snappy_frame_encode_to_vec;
                snappy_frame_encode_to_vec(payload)
            }
            MicroChunkCodec::RawPassthrough => Ok(payload.to_vec()),
        }
    }

    /// Executes exhaustive micro-chunk ladder validation for a given codec and payload.
    pub fn validate_codec_exhaustive(
        codec: MicroChunkCodec,
        payload: &[u8],
        custom_steps: Option<&[usize]>,
    ) -> Result<MicroChunkCodecReport, TTZipStatus> {
        let compressed = Self::compress_codec(codec, payload)?;
        let steps = custom_steps.unwrap_or(MICRO_CHUNK_STEPS);
        let mut step_results = Vec::with_capacity(steps.len());
        let mut all_passed = true;

        for &step in steps {
            let reader = Self::create_codec_reader(codec, &compressed)?;
            let res = Self::validate_reader(reader, payload, step, codec);
            if !res.passed {
                all_passed = false;
            }
            step_results.push(res);
        }

        // Run variable staircase stress test
        let staircase_reader = Self::create_codec_reader(codec, &compressed)?;
        let staircase_result = Self::validate_reader_staircase(
            staircase_reader,
            payload,
            STAIRCASE_CHUNK_PATTERN,
            codec,
        );
        if !staircase_result.passed {
            all_passed = false;
        }

        Ok(MicroChunkCodecReport {
            codec,
            step_results,
            staircase_result: Some(staircase_result),
            all_passed,
        })
    }

    /// Runs exhaustive micro-chunk validation across all supported built-in streaming codecs.
    pub fn validate_all_codecs(
        payload: &[u8],
    ) -> Result<Vec<MicroChunkCodecReport>, TTZipStatus> {
        let codecs = [
            MicroChunkCodec::Zstd,
            MicroChunkCodec::Brotli,
            MicroChunkCodec::SnappyFramed,
            MicroChunkCodec::RawPassthrough,
        ];

        let mut reports = Vec::with_capacity(codecs.len());
        for &codec in &codecs {
            let report = Self::validate_codec_exhaustive(codec, payload, None)?;
            reports.push(report);
        }

        Ok(reports)
    }
}
