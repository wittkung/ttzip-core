// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Export Layer for TTZip Zopfli Extreme Compression Engine.
//!
//! Provides typed bindings for Google's Zopfli iterative entropy-optimizing Deflate
//! algorithm across raw DEFLATE (RFC 1951), Zlib (RFC 1950), and Gzip (RFC 1952) streams.

use std::io::Read;
use std::sync::Arc;
use std::time::Instant;

use crate::codecs::zopfli::{
    zopfli_compress, ZopfliFormat as NativeZopfliFormat, ZopfliOptions as NativeZopfliOptions,
};
use crate::uniffi_api::callback::{UniFFICancellationToken, UniFFIProgressCallback};
use crate::uniffi_api::types::TTZipError;

// MARK: - Enums & Models

/// Target container format for Zopfli compression.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum UniFFIZopfliFormat {
    /// Raw RFC 1951 Deflate byte stream without headers or checksums.
    Deflate,
    /// RFC 1950 Zlib stream with 2-byte header and Adler-32 trailing checksum.
    Zlib,
    /// RFC 1952 Gzip stream with 10-byte header, timestamp, and CRC32 checksum.
    Gzip,
}

/// Predefined optimization profiles balancing speed vs extreme compression density.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum UniFFIZopfliPreset {
    /// Fast pass: 5 iterations, 5 block splits.
    Fast,
    /// Balanced standard pass: 15 iterations, 15 block splits.
    Balanced,
    /// Maximum density pass: 30 iterations, 30 block splits.
    Maximum,
    /// Ultra extreme pass: 100 iterations, 50 block splits.
    Ultra,
}

/// Strongly typed configuration options for Zopfli compression passes.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIZopfliOptions {
    /// Maximum number of forward/backward optimization iterations (default: 15).
    pub iteration_count: u64,
    /// Stop iterations early if no cost reduction occurs for N consecutive passes (0 = disabled).
    pub iterations_without_improvement: u64,
    /// Maximum number of dynamic Deflate block splits (0 = unlimited, default: 15).
    pub maximum_block_splits: u16,
    /// Enables or disables dynamic block splitting heuristics.
    pub block_splitting: bool,
}

impl Default for UniFFIZopfliOptions {
    fn default() -> Self {
        Self {
            iteration_count: 15,
            iterations_without_improvement: 15,
            maximum_block_splits: 15,
            block_splitting: true,
        }
    }
}

/// Compression performance telemetry and analytical metrics for Zopfli runs.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFIZopfliStats {
    pub format: UniFFIZopfliFormat,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub compression_ratio: f64,
    pub duration_nanos: u64,
    pub throughput_mbs: f64,
    pub iterations: u64,
}

// MARK: - Internal Conversion Helpers

fn to_native_options(opts: &UniFFIZopfliOptions) -> NativeZopfliOptions {
    let splits = if opts.block_splitting {
        opts.maximum_block_splits as usize
    } else {
        0
    };
    let chain = match opts.iteration_count {
        0..=5 => 256,
        6..=15 => 1024,
        16..=30 => 2048,
        _ => 4096,
    };
    NativeZopfliOptions {
        num_iterations: opts.iteration_count.max(1) as usize,
        max_block_splits: splits,
        max_chain: chain,
    }
}

fn to_native_format(fmt: UniFFIZopfliFormat) -> NativeZopfliFormat {
    match fmt {
        UniFFIZopfliFormat::Deflate => NativeZopfliFormat::Deflate,
        UniFFIZopfliFormat::Zlib => NativeZopfliFormat::Zlib,
        UniFFIZopfliFormat::Gzip => NativeZopfliFormat::Gzip,
    }
}

// MARK: - Exported Free Functions

/// Resolves standard preset options for Zopfli compression.
#[uniffi::export]
pub fn uniffi_zopfli_options_for_preset(preset: UniFFIZopfliPreset) -> UniFFIZopfliOptions {
    match preset {
        UniFFIZopfliPreset::Fast => UniFFIZopfliOptions {
            iteration_count: 5,
            iterations_without_improvement: 5,
            maximum_block_splits: 5,
            block_splitting: true,
        },
        UniFFIZopfliPreset::Balanced => UniFFIZopfliOptions {
            iteration_count: 15,
            iterations_without_improvement: 15,
            maximum_block_splits: 15,
            block_splitting: true,
        },
        UniFFIZopfliPreset::Maximum => UniFFIZopfliOptions {
            iteration_count: 30,
            iterations_without_improvement: 30,
            maximum_block_splits: 30,
            block_splitting: true,
        },
        UniFFIZopfliPreset::Ultra => UniFFIZopfliOptions {
            iteration_count: 100,
            iterations_without_improvement: 100,
            maximum_block_splits: 50,
            block_splitting: true,
        },
    }
}

/// Compresses a buffer with Zopfli using specified format and options.
#[uniffi::export]
pub fn uniffi_zopfli_compress(
    format: UniFFIZopfliFormat,
    data: Vec<u8>,
    options: UniFFIZopfliOptions,
) -> Result<Vec<u8>, TTZipError> {
    uniffi_zopfli_compress_cancellable(format, data, options, None)
}

/// Compresses a buffer with Zopfli with cancellation token support.
#[uniffi::export]
pub fn uniffi_zopfli_compress_cancellable(
    format: UniFFIZopfliFormat,
    data: Vec<u8>,
    options: UniFFIZopfliOptions,
    cancellation_token: Option<Arc<UniFFICancellationToken>>,
) -> Result<Vec<u8>, TTZipError> {
    uniffi_zopfli_compress_with_progress(format, data, options, None, cancellation_token)
}

/// Compresses a buffer with Zopfli with fine-grained progress and cancellation support.
#[uniffi::export]
pub fn uniffi_zopfli_compress_with_progress(
    format: UniFFIZopfliFormat,
    data: Vec<u8>,
    options: UniFFIZopfliOptions,
    callback: Option<Box<dyn UniFFIProgressCallback>>,
    cancellation_token: Option<Arc<UniFFICancellationToken>>,
) -> Result<Vec<u8>, TTZipError> {
    if let Some(ref tok) = cancellation_token {
        if tok.is_cancelled() {
            return Err(TTZipError::Cancelled);
        }
    }

    if let Some(ref cb) = callback {
        if !cb.on_progress(0, data.len() as u64, None) {
            return Err(TTZipError::Cancelled);
        }
    }

    let native_options = to_native_options(&options);
    let native_format = to_native_format(format);

    let compressed = zopfli_compress(&data, native_format, &native_options)
        .map_err(|st| TTZipError::EngineError { code: st as i32 })?;

    if let Some(ref tok) = cancellation_token {
        if tok.is_cancelled() {
            return Err(TTZipError::Cancelled);
        }
    }

    if let Some(ref cb) = callback {
        if !cb.on_progress(data.len() as u64, data.len() as u64, None) {
            return Err(TTZipError::Cancelled);
        }
    }

    Ok(compressed)
}

/// Decompresses a Zopfli-compressed byte stream back into uncompressed data.
#[uniffi::export]
pub fn uniffi_zopfli_decompress(
    format: UniFFIZopfliFormat,
    compressed: Vec<u8>,
) -> Result<Vec<u8>, TTZipError> {
    match format {
        UniFFIZopfliFormat::Deflate => {
            let mut decoder = flate2::read::DeflateDecoder::new(&compressed[..]);
            let mut out = Vec::new();
            decoder
                .read_to_end(&mut out)
                .map_err(|e| TTZipError::io_error(e, "Deflate decompression failed"))?;
            Ok(out)
        }
        UniFFIZopfliFormat::Zlib => {
            let mut decoder = flate2::read::ZlibDecoder::new(&compressed[..]);
            let mut out = Vec::new();
            decoder
                .read_to_end(&mut out)
                .map_err(|e| TTZipError::io_error(e, "Zlib decompression failed"))?;
            Ok(out)
        }
        UniFFIZopfliFormat::Gzip => {
            let mut decoder = flate2::read::GzDecoder::new(&compressed[..]);
            let mut out = Vec::new();
            decoder
                .read_to_end(&mut out)
                .map_err(|e| TTZipError::io_error(e, "Gzip decompression failed"))?;
            Ok(out)
        }
    }
}

/// Performs a lossless roundtrip compression and decompression check.
#[uniffi::export]
pub fn uniffi_zopfli_verify_roundtrip(
    format: UniFFIZopfliFormat,
    data: Vec<u8>,
    options: UniFFIZopfliOptions,
) -> Result<bool, TTZipError> {
    let compressed = uniffi_zopfli_compress(format, data.clone(), options)?;
    let decompressed = uniffi_zopfli_decompress(format, compressed)?;
    Ok(decompressed == data)
}

/// Executes a Zopfli compression benchmark and returns performance statistics.
#[uniffi::export]
pub fn uniffi_zopfli_benchmark(
    data: Vec<u8>,
    options: UniFFIZopfliOptions,
    format: UniFFIZopfliFormat,
) -> Result<UniFFIZopfliStats, TTZipError> {
    let uncompressed_size = data.len() as u64;
    let iterations = options.iteration_count;

    let start = Instant::now();
    let compressed = uniffi_zopfli_compress(format, data, options)?;
    let elapsed = start.elapsed();

    let duration_nanos = elapsed.as_nanos() as u64;
    let compressed_size = compressed.len() as u64;

    let compression_ratio = if uncompressed_size > 0 {
        (compressed_size as f64 / uncompressed_size as f64) * 100.0
    } else {
        100.0
    };

    let secs = elapsed.as_secs_f64();
    let throughput_mbs = if secs > 0.0 {
        (uncompressed_size as f64 / (1024.0 * 1024.0)) / secs
    } else {
        0.0
    };

    Ok(UniFFIZopfliStats {
        format,
        uncompressed_size,
        compressed_size,
        compression_ratio,
        duration_nanos,
        throughput_mbs,
        iterations,
    })
}

// MARK: - UniFFIZopfliOptimizer Class

/// Stateful UniFFI object wrapper for managing and running Zopfli optimizations.
#[derive(uniffi::Object)]
pub struct UniFFIZopfliOptimizer {
    options: UniFFIZopfliOptions,
}

#[uniffi::export]
impl UniFFIZopfliOptimizer {
    /// Constructs a new optimizer instance with custom configuration options.
    #[uniffi::constructor]
    pub fn new(options: UniFFIZopfliOptions) -> Arc<Self> {
        Arc::new(Self { options })
    }

    /// Constructs a new optimizer instance from a preset.
    #[uniffi::constructor]
    pub fn with_preset(preset: UniFFIZopfliPreset) -> Arc<Self> {
        Arc::new(Self {
            options: uniffi_zopfli_options_for_preset(preset),
        })
    }

    /// Returns the configured options for this optimizer.
    pub fn options(&self) -> UniFFIZopfliOptions {
        self.options.clone()
    }

    /// Compresses a buffer with the configured options.
    pub fn compress(&self, format: UniFFIZopfliFormat, data: Vec<u8>) -> Result<Vec<u8>, TTZipError> {
        uniffi_zopfli_compress(format, data, self.options.clone())
    }

    /// Compresses a buffer with cancellation support.
    pub fn compress_cancellable(
        &self,
        format: UniFFIZopfliFormat,
        data: Vec<u8>,
        cancellation_token: Option<Arc<UniFFICancellationToken>>,
    ) -> Result<Vec<u8>, TTZipError> {
        uniffi_zopfli_compress_cancellable(format, data, self.options.clone(), cancellation_token)
    }

    /// Compresses a buffer with progress callback and cancellation token.
    pub fn compress_with_progress(
        &self,
        format: UniFFIZopfliFormat,
        data: Vec<u8>,
        callback: Option<Box<dyn UniFFIProgressCallback>>,
        cancellation_token: Option<Arc<UniFFICancellationToken>>,
    ) -> Result<Vec<u8>, TTZipError> {
        uniffi_zopfli_compress_with_progress(
            format,
            data,
            self.options.clone(),
            callback,
            cancellation_token,
        )
    }

    /// Benchmarks compression on the given payload.
    pub fn benchmark(
        &self,
        format: UniFFIZopfliFormat,
        data: Vec<u8>,
    ) -> Result<UniFFIZopfliStats, TTZipError> {
        uniffi_zopfli_benchmark(data, self.options.clone(), format)
    }

    /// Verifies lossless roundtrip correctness.
    pub fn verify_roundtrip(
        &self,
        format: UniFFIZopfliFormat,
        data: Vec<u8>,
    ) -> Result<bool, TTZipError> {
        uniffi_zopfli_verify_roundtrip(format, data, self.options.clone())
    }
}

// MARK: - Unit Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_presets_options_mapping() {
        let fast = uniffi_zopfli_options_for_preset(UniFFIZopfliPreset::Fast);
        assert_eq!(fast.iteration_count, 5);
        assert_eq!(fast.maximum_block_splits, 5);

        let ultra = uniffi_zopfli_options_for_preset(UniFFIZopfliPreset::Ultra);
        assert_eq!(ultra.iteration_count, 100);
        assert_eq!(ultra.maximum_block_splits, 50);
    }

    #[test]
    fn test_zopfli_roundtrip_all_formats() {
        let payload = b"TTZip Zopfli Extreme Compression Engine RFC1951 RFC1950 RFC1952 Verification Payload 1234567890\n".repeat(10);
        let opts = uniffi_zopfli_options_for_preset(UniFFIZopfliPreset::Fast);

        for format in [
            UniFFIZopfliFormat::Deflate,
            UniFFIZopfliFormat::Zlib,
            UniFFIZopfliFormat::Gzip,
        ] {
            let ok = uniffi_zopfli_verify_roundtrip(format, payload.clone(), opts.clone())
                .expect("roundtrip verification must succeed");
            assert!(ok, "Format {:?} roundtrip must be lossless", format);
        }
    }

    #[test]
    fn test_zopfli_cancellation_token() {
        let payload = vec![0x42u8; 10000];
        let opts = uniffi_zopfli_options_for_preset(UniFFIZopfliPreset::Fast);
        let token = UniFFICancellationToken::new();
        token.cancel();

        let res = uniffi_zopfli_compress_cancellable(
            UniFFIZopfliFormat::Deflate,
            payload,
            opts,
            Some(token),
        );
        assert!(matches!(res, Err(TTZipError::Cancelled)));
    }

    #[test]
    fn test_zopfli_benchmark_telemetry() {
        let payload = b"Structured telemetry log line for TTZip extreme zopfli benchmark verification\n".repeat(20);
        let opts = uniffi_zopfli_options_for_preset(UniFFIZopfliPreset::Fast);
        let stats = uniffi_zopfli_benchmark(payload, opts, UniFFIZopfliFormat::Gzip).expect("benchmark");

        assert_eq!(stats.format, UniFFIZopfliFormat::Gzip);
        assert!(stats.compressed_size > 0);
        assert!(stats.compression_ratio < 100.0);
        assert!(stats.duration_nanos > 0);
    }

    #[test]
    fn test_zopfli_optimizer_object() {
        let optimizer = UniFFIZopfliOptimizer::with_preset(UniFFIZopfliPreset::Fast);
        let payload = b"UniFFIZopfliOptimizer stateful instance test data string 2026\n".repeat(5);

        let compressed = optimizer
            .compress(UniFFIZopfliFormat::Zlib, payload.clone())
            .expect("compression");
        assert!(!compressed.is_empty());

        let ok = optimizer
            .verify_roundtrip(UniFFIZopfliFormat::Zlib, payload)
            .expect("roundtrip");
        assert!(ok);
    }
}
