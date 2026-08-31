// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Export Layer for TTZip Deflate Dual-Engine Optimization.
//!
//! Exposes:
//! - [`UniFFIDeflateEngine`]: Dual-engine selection (Hardware C-libdeflate vs. Pure-Rust Near-Optimal DP).
//! - [`UniFFIDeflateArbitrationStrategy`]: Dynamic arbitration heuristics (Speed, Ratio, Balanced, DynamicAdaptive).
//! - [`UniFFIDeflateLevel`]: Strongly typed compression level representation (0..=12).
//! - [`UniFFIDeflateStats`]: Detailed compression performance telemetry metrics.
//! - [`UniFFISyntheticCorpusType`]: 8 mathematical synthetic benchmark corpus generators.

use std::time::Instant;
use crate::uniffi_api::types::TTZipError;

// MARK: - Enums & Models

/// Dual-engine selection for RFC 1951 Deflate compression.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum UniFFIDeflateEngine {
    /// Hardware-accelerated SIMD vectorised C-libdeflate engine.
    LibdeflateHardware,
    /// Pure-Rust Near-Optimal Dynamic Programming OptParser with EM refinement.
    PureRustNearOptimalDp,
}

/// Dynamic arbitration strategy for selecting optimal Deflate engine and compression level.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum UniFFIDeflateArbitrationStrategy {
    /// Maximize processing speed and throughput; always selects `LibdeflateHardware`.
    SpeedFirst,
    /// Maximize compression ratio; selects `PureRustNearOptimalDp` with high levels.
    RatioFirst,
    /// Balance speed and ratio based on payload volume thresholds (<= 64KB DP, > 64KB Hardware).
    Balanced,
    /// Analyzes entropy and data characteristics dynamically to pick the optimal engine.
    DynamicAdaptive,
}

/// Strongly typed compression level for Deflate pipelines.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum UniFFIDeflateLevel {
    /// RFC 1951 BTYPE=00 uncompressed store blocks (Level 0).
    Store,
    /// Fast greedy match parsing (Level 1).
    Fast,
    /// Default balanced compression (Level 6).
    DefaultLevel,
    /// Maximum lazy match evaluation (Level 9).
    Maximum,
    /// Ultra Near-Optimal DP with EM refinement (Level 12).
    UltraDp,
    /// Custom compression level in range 0..=12.
    Custom { level: i32 },
}

impl UniFFIDeflateLevel {
    /// Resolves the raw integer level in range 0..=12.
    pub fn to_raw_level(&self) -> i32 {
        match self {
            Self::Store => 0,
            Self::Fast => 1,
            Self::DefaultLevel => 6,
            Self::Maximum => 9,
            Self::UltraDp => 12,
            Self::Custom { level } => (*level).clamp(0, 12),
        }
    }
}

/// Detailed compression telemetry and performance metrics.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFIDeflateStats {
    pub engine: UniFFIDeflateEngine,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub compression_ratio: f64,
    pub duration_nanos: u64,
    pub throughput_mbs: f64,
}

/// 8 Representative mathematical synthetic corpus types for empirical benchmarking.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum UniFFISyntheticCorpusType {
    /// Uniform zero byte run (0x00, zero entropy).
    AllZeros,
    /// Structured redundant timestamped log stream.
    TextRedundant,
    /// Repeated ASCII phrase sequences.
    HighlyRepetitive,
    /// High-entropy pseudo-random bytes (~8.0 bits/byte).
    UniformRandom,
    /// Low-entropy 4-bit nibble distribution (~2.0 bits/byte).
    LowEntropyNibbles,
    /// Structured JSON / C / Swift source code AST tokens.
    AsciiSourceCode,
    /// Simulated Mach-O / ARM64 / x86_64 executable machine code bytecode.
    BinaryExecutable,
    /// Zipfian power-law frequency distribution.
    ExponentialDecay,
}

// MARK: - Deterministic PRNG for Synthetic Corpus Generation

struct DeterministicPrng {
    state: u64,
}

impl DeterministicPrng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x517c_c1b7_2722_0a95 } else { seed },
        }
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    #[inline]
    fn next_u8(&mut self) -> u8 {
        (self.next_u64() & 0xFF) as u8
    }
}

// MARK: - Synthetic Corpus Generator

/// Generates a deterministic mathematical synthetic corpus of specified type and byte size.
#[uniffi::export]
pub fn uniffi_generate_synthetic_corpus(
    corpus_type: UniFFISyntheticCorpusType,
    size_bytes: u64,
    seed: Option<u64>,
) -> Vec<u8> {
    let size = size_bytes as usize;
    if size == 0 {
        return Vec::new();
    }

    let mut prng = DeterministicPrng::new(seed.unwrap_or(0x1234_5678_9ABC_DEF0));

    match corpus_type {
        UniFFISyntheticCorpusType::AllZeros => vec![0u8; size],
        UniFFISyntheticCorpusType::TextRedundant => {
            let mut out = Vec::with_capacity(size);
            let mut line_idx = 0usize;
            while out.len() < size {
                let line = format!(
                    "[2026-08-31 23:59:{:02}.{:03}] [INFO] [ttzip::engine::dual] Record #{:06} processed payload_size=65536 bytes status=OK checksum=0x{:08X}\n",
                    line_idx % 60,
                    line_idx % 1000,
                    line_idx,
                    (line_idx as u32).wrapping_mul(0x9E37_79B9)
                );
                let remaining = size - out.len();
                let bytes = line.as_bytes();
                if bytes.len() <= remaining {
                    out.extend_from_slice(bytes);
                } else {
                    out.extend_from_slice(&bytes[..remaining]);
                    break;
                }
                line_idx += 1;
            }
            out
        }
        UniFFISyntheticCorpusType::HighlyRepetitive => {
            let pattern = b"TTZip High-Performance Native Compression Engine RFC1951 Deflate Dual-Engine Optimizer.\n";
            let mut out = Vec::with_capacity(size);
            while out.len() < size {
                let remaining = size - out.len();
                let to_copy = remaining.min(pattern.len());
                out.extend_from_slice(&pattern[..to_copy]);
            }
            out
        }
        UniFFISyntheticCorpusType::UniformRandom => {
            let mut out = vec![0u8; size];
            for byte in out.iter_mut() {
                *byte = prng.next_u8();
            }
            out
        }
        UniFFISyntheticCorpusType::LowEntropyNibbles => {
            let alphabet = b"0123456789ABCDEF";
            let mut out = vec![0u8; size];
            for byte in out.iter_mut() {
                *byte = alphabet[(prng.next_u8() & 0x0F) as usize];
            }
            out
        }
        UniFFISyntheticCorpusType::AsciiSourceCode => {
            let templates = [
                b"func optimizeDeflatePayload(_ buffer: inout [UInt8]) -> Int { return buffer.count * 2 }\n" as &[u8],
                b"struct CompressionPipelineDescriptor: Sendable { let algorithm: String; let level: Int }\n",
                b"// TTZip Architecture Invariant: Zero-allocation and bounds-first guarantee\n",
                b"{\"name\": \"TTZip\", \"version\": \"2.0.0\", \"license\": \"BSD-3-Clause OR Apache-2.0\", \"active\": true},\n",
                b"pub fn compress_block_parallel(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> { Ok(0) }\n",
            ];
            let mut out = Vec::with_capacity(size);
            let mut idx = 0;
            while out.len() < size {
                let chunk = templates[idx % templates.len()];
                let remaining = size - out.len();
                let to_copy = remaining.min(chunk.len());
                out.extend_from_slice(&chunk[..to_copy]);
                idx += 1;
            }
            out
        }
        UniFFISyntheticCorpusType::BinaryExecutable => {
            // Generates synthetic x86_64 / ARM64 machine code instructions
            let mut out = Vec::with_capacity(size);
            let mut ip = 0x1000u32;
            while out.len() < size {
                let choice = prng.next_u8() % 6;
                let inst: &[u8] = match choice {
                    0 => &[0x55, 0x48, 0x89, 0xE5], // push rbp; mov rbp, rsp
                    1 => &[0x48, 0x83, 0xEC, 0x20], // sub rsp, 32
                    2 => &[0x48, 0x89, 0x7D, 0xE8], // mov [rbp-24], rdi
                    3 => &[0x31, 0xC0, 0x5D, 0xC3], // xor eax, eax; pop rbp; ret
                    4 => &[0x1F, 0x20, 0x03, 0xD5], // ARM64: nop
                    _ => &[0xC0, 0x03, 0x5F, 0xD6], // ARM64: ret
                };
                let remaining = size - out.len();
                let to_copy = remaining.min(inst.len());
                out.extend_from_slice(&inst[..to_copy]);
                ip = ip.wrapping_add(to_copy as u32);
            }
            out
        }
        UniFFISyntheticCorpusType::ExponentialDecay => {
            // Zipfian distribution: symbol k chosen with probability proportional to (1 / (k + 1)^1.2)
            let mut out = vec![0u8; size];
            for byte in out.iter_mut() {
                let r = (prng.next_u64() % 10000) as f64 / 10000.0;
                // Inverse CDF sampling approximation for alpha ~ 1.2
                let val = (1.0 / (1.0 - r * 0.95).powf(0.8) - 1.0) as u8;
                *byte = val;
            }
            out
        }
    }
}

// MARK: - Dual Deflate Compression & Decompression Operations

/// Compresses a memory buffer with the specified Deflate engine and compression level.
#[uniffi::export]
pub fn uniffi_deflate_dual_compress(
    engine: UniFFIDeflateEngine,
    src: Vec<u8>,
    level: UniFFIDeflateLevel,
) -> Result<Vec<u8>, TTZipError> {
    let raw_level = level.to_raw_level();
    match engine {
        UniFFIDeflateEngine::LibdeflateHardware => {
            let bound = crate::codecs::deflate::deflate_compress_bound(src.len(), raw_level);
            let mut dst = vec![0u8; bound];
            let written = crate::codecs::deflate::deflate_compress(&src, &mut dst, raw_level)
                .map_err(|st| TTZipError::EngineError { code: st as i32 })?;
            dst.truncate(written);
            Ok(dst)
        }
        UniFFIDeflateEngine::PureRustNearOptimalDp => {
            crate::codecs::libdeflate::deflate_compress(&src, raw_level)
                .map_err(|st| TTZipError::EngineError { code: st as i32 })
        }
    }
}

/// Decompresses raw DEFLATE bytes into the expected uncompressed buffer.
#[uniffi::export]
pub fn uniffi_deflate_dual_decompress(
    engine: UniFFIDeflateEngine,
    src: Vec<u8>,
    expected_uncompressed_size: u64,
) -> Result<Vec<u8>, TTZipError> {
    let mut dst = vec![0u8; expected_uncompressed_size as usize];
    match engine {
        UniFFIDeflateEngine::LibdeflateHardware => {
            let written = crate::codecs::deflate::deflate_decompress(&src, &mut dst)
                .map_err(|st| TTZipError::EngineError { code: st as i32 })?;
            dst.truncate(written);
            Ok(dst)
        }
        UniFFIDeflateEngine::PureRustNearOptimalDp => {
            let written = crate::codecs::libdeflate::libdeflate_deflate_decompress(&src, &mut dst)
                .map_err(|st| TTZipError::EngineError { code: st as i32 })?;
            dst.truncate(written);
            Ok(dst)
        }
    }
}

/// Arbitrates and selects the best Deflate engine given a strategy, payload size, and estimated entropy.
#[uniffi::export]
pub fn uniffi_deflate_dual_arbitrate(
    strategy: UniFFIDeflateArbitrationStrategy,
    uncompressed_size: u64,
    estimated_entropy: Option<f64>,
) -> UniFFIDeflateEngine {
    match strategy {
        UniFFIDeflateArbitrationStrategy::SpeedFirst => UniFFIDeflateEngine::LibdeflateHardware,
        UniFFIDeflateArbitrationStrategy::RatioFirst => UniFFIDeflateEngine::PureRustNearOptimalDp,
        UniFFIDeflateArbitrationStrategy::Balanced => {
            if uncompressed_size <= 65536 {
                UniFFIDeflateEngine::PureRustNearOptimalDp
            } else {
                UniFFIDeflateEngine::LibdeflateHardware
            }
        }
        UniFFIDeflateArbitrationStrategy::DynamicAdaptive => {
            let entropy = estimated_entropy.unwrap_or(5.0);
            if entropy > 7.2 {
                // High entropy data benefits more from high-throughput hardware engine
                UniFFIDeflateEngine::LibdeflateHardware
            } else if uncompressed_size <= 131072 {
                // Small/medium structured data gains significant compression ratio from OptParser DP
                UniFFIDeflateEngine::PureRustNearOptimalDp
            } else {
                UniFFIDeflateEngine::LibdeflateHardware
            }
        }
    }
}

/// Benchmarks both Deflate engines on the provided input data and returns comparative telemetry statistics.
#[uniffi::export]
pub fn uniffi_deflate_dual_benchmark(
    src: Vec<u8>,
    level: UniFFIDeflateLevel,
) -> Result<Vec<UniFFIDeflateStats>, TTZipError> {
    let uncompressed_size = src.len() as u64;
    let mut stats = Vec::with_capacity(2);

    for engine in [UniFFIDeflateEngine::LibdeflateHardware, UniFFIDeflateEngine::PureRustNearOptimalDp] {
        let start = Instant::now();
        let compressed = uniffi_deflate_dual_compress(engine, src.clone(), level)?;
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

        stats.push(UniFFIDeflateStats {
            engine,
            uncompressed_size,
            compressed_size,
            compression_ratio,
            duration_nanos,
            throughput_mbs,
        });
    }

    Ok(stats)
}

/// Lossless roundtrip verification test for Deflate compression and decompression.
#[uniffi::export]
pub fn uniffi_deflate_dual_verify_roundtrip(
    src: Vec<u8>,
    level: UniFFIDeflateLevel,
) -> Result<bool, TTZipError> {
    for engine in [UniFFIDeflateEngine::LibdeflateHardware, UniFFIDeflateEngine::PureRustNearOptimalDp] {
        let compressed = uniffi_deflate_dual_compress(engine, src.clone(), level)?;
        let decompressed = uniffi_deflate_dual_decompress(engine, compressed, src.len() as u64)?;
        if decompressed != src {
            return Ok(false);
        }
    }
    Ok(true)
}

// MARK: - Unit Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthetic_corpus_generation_all_8_types() {
        let types = [
            UniFFISyntheticCorpusType::AllZeros,
            UniFFISyntheticCorpusType::TextRedundant,
            UniFFISyntheticCorpusType::HighlyRepetitive,
            UniFFISyntheticCorpusType::UniformRandom,
            UniFFISyntheticCorpusType::LowEntropyNibbles,
            UniFFISyntheticCorpusType::AsciiSourceCode,
            UniFFISyntheticCorpusType::BinaryExecutable,
            UniFFISyntheticCorpusType::ExponentialDecay,
        ];

        for corpus_type in types {
            let corpus = uniffi_generate_synthetic_corpus(corpus_type, 4096, Some(42));
            assert_eq!(corpus.len(), 4096, "Corpus size must match requested 4096 bytes");
        }
    }

    #[test]
    fn test_dual_engine_roundtrip_all_levels() {
        let payload = uniffi_generate_synthetic_corpus(UniFFISyntheticCorpusType::AsciiSourceCode, 8192, Some(100));
        let levels = [
            UniFFIDeflateLevel::Store,
            UniFFIDeflateLevel::Fast,
            UniFFIDeflateLevel::DefaultLevel,
            UniFFIDeflateLevel::Maximum,
            UniFFIDeflateLevel::UltraDp,
        ];

        for lvl in levels {
            let ok = uniffi_deflate_dual_verify_roundtrip(payload.clone(), lvl).expect("roundtrip");
            assert!(ok, "Roundtrip verification must pass for level {:?}", lvl);
        }
    }

    #[test]
    fn test_arbitration_strategy() {
        let eng_speed = uniffi_deflate_dual_arbitrate(UniFFIDeflateArbitrationStrategy::SpeedFirst, 1000, None);
        assert_eq!(eng_speed, UniFFIDeflateEngine::LibdeflateHardware);

        let eng_ratio = uniffi_deflate_dual_arbitrate(UniFFIDeflateArbitrationStrategy::RatioFirst, 1000, None);
        assert_eq!(eng_ratio, UniFFIDeflateEngine::PureRustNearOptimalDp);

        let eng_balanced_small = uniffi_deflate_dual_arbitrate(UniFFIDeflateArbitrationStrategy::Balanced, 32768, None);
        assert_eq!(eng_balanced_small, UniFFIDeflateEngine::PureRustNearOptimalDp);

        let eng_balanced_large = uniffi_deflate_dual_arbitrate(UniFFIDeflateArbitrationStrategy::Balanced, 1048576, None);
        assert_eq!(eng_balanced_large, UniFFIDeflateEngine::LibdeflateHardware);
    }

    #[test]
    fn test_benchmark_telemetry() {
        let payload = uniffi_generate_synthetic_corpus(UniFFISyntheticCorpusType::TextRedundant, 16384, Some(7));
        let stats = uniffi_deflate_dual_benchmark(payload, UniFFIDeflateLevel::DefaultLevel).expect("benchmark");
        assert_eq!(stats.len(), 2);
        assert!(stats[0].compressed_size > 0);
        assert!(stats[1].compressed_size > 0);
        assert!(stats[0].compression_ratio < 100.0);
        assert!(stats[1].compression_ratio < 100.0);
    }
}
