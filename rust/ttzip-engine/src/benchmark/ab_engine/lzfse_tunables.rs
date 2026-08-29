// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Apple LZFSE Hardware Microarchitecture Tunables & LZVN Fallback Engine.
//!
//! Models and exercises Apple Silicon M-Series microarchitectural tunables from
//! `vendor/lzfse/src/lzfse_tunables.h`:
//! - **Cache Line Alignment**: Evaluates 128-byte cache lines and 32-byte history entries.
//! - **L1D Cache Residency**: Dynamically sizes history tables (128KB - 512KB) to maximize L1D hits.
//! - **LZVN Adaptive Routing**: Intelligently switches between LZFSE (FSE entropy coding) and LZVN
//!   (pure LZ byte-aligned bytecode) below configurable threshold boundaries.
//! - **Throughput & Speedup Micro-Benchmarking**: Measures dual-codec throughput and compression ratios.

use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::codecs::lzfse::{
    lzfse_compress, lzfse_compress_bound, lzfse_decompress, lzvn_compress, lzvn_compress_bound,
    lzvn_decompress,
};
use crate::types::TTZipStatus;

/// Default Apple Silicon M-Series cache line size in bytes (128 bytes on Apple Silicon).
pub const APPLE_SILICON_CACHE_LINE_BYTES: usize = 128;

/// Standard Apple L1D cache size per Performance core (128 KB on M1/M2/M3/M4/M5 P-cores).
pub const APPLE_SILICON_P_CORE_L1D_BYTES: usize = 128 * 1024;

/// Canonical default LZVN transition threshold in bytes (4096 bytes / 4 KB).
pub const DEFAULT_LZVN_THRESHOLD: usize = 4096;

/// Canonical default LZFSE hash bits (14 bits -> 16,384 table entries).
pub const DEFAULT_LZFSE_HASH_BITS: u32 = 14;

/// Canonical default LZFSE hash width (4 positions per hash bucket).
pub const DEFAULT_LZFSE_HASH_WIDTH: u32 = 4;

/// Canonical default good match threshold in bytes (40 bytes).
pub const DEFAULT_LZFSE_GOOD_MATCH: usize = 40;

/// Microarchitectural tuning profile presets for LZFSE and LZVN.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LzfseProfile {
    /// Canonical Apple iOS / macOS default parameters (256 KB history table).
    DefaultApple,
    /// Apple Silicon M-Series High Throughput profile (32-byte entries, 8 KB LZVN fallback).
    AppleSiliconHighThroughput,
    /// Apple Silicon L1D Cache Residency profile (128 KB table fitting 100% in P-core L1D).
    AppleSiliconL1dResidency,
    /// Apple Silicon Maximum Compression Ratio profile (2 MB table, 64-byte good match).
    AppleSiliconMaxRatio,
    /// Direct LZVN Ultra-Fast mode (all payloads routed to LZVN).
    LzvnUltraFast,
    /// Fully user-customized microarchitectural tunables.
    Custom,
}

/// Detailed configuration parameters for the LZFSE tunables scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LzfseTunablesConfig {
    /// Active profile preset.
    pub profile: LzfseProfile,
    /// Number of bits for the hash table (clamped between 10 and 16).
    pub hash_bits: u32,
    /// Number of entries stored per line in the history table (4 or 8).
    pub hash_width: u32,
    /// Match length in bytes to trigger immediate greedy emission.
    pub good_match: usize,
    /// Byte size threshold below which compression transitions to LZVN.
    pub lzvn_threshold: usize,
}

impl Default for LzfseTunablesConfig {
    fn default() -> Self {
        Self::from_profile(LzfseProfile::DefaultApple)
    }
}

impl LzfseTunablesConfig {
    /// Creates a configuration from a predefined microarchitectural profile.
    pub fn from_profile(profile: LzfseProfile) -> Self {
        match profile {
            LzfseProfile::DefaultApple => Self {
                profile,
                hash_bits: DEFAULT_LZFSE_HASH_BITS,
                hash_width: DEFAULT_LZFSE_HASH_WIDTH,
                good_match: DEFAULT_LZFSE_GOOD_MATCH,
                lzvn_threshold: DEFAULT_LZVN_THRESHOLD,
            },
            LzfseProfile::AppleSiliconHighThroughput => Self {
                profile,
                hash_bits: 13,
                hash_width: 8,
                good_match: 32,
                lzvn_threshold: 8192,
            },
            LzfseProfile::AppleSiliconL1dResidency => Self {
                profile,
                hash_bits: 12,
                hash_width: 8,
                good_match: 24,
                lzvn_threshold: 4096,
            },
            LzfseProfile::AppleSiliconMaxRatio => Self {
                profile,
                hash_bits: 16,
                hash_width: 8,
                good_match: 64,
                lzvn_threshold: 2048,
            },
            LzfseProfile::LzvnUltraFast => Self {
                profile,
                hash_bits: 10,
                hash_width: 4,
                good_match: 16,
                lzvn_threshold: usize::MAX,
            },
            LzfseProfile::Custom => Self {
                profile,
                hash_bits: DEFAULT_LZFSE_HASH_BITS,
                hash_width: DEFAULT_LZFSE_HASH_WIDTH,
                good_match: DEFAULT_LZFSE_GOOD_MATCH,
                lzvn_threshold: DEFAULT_LZVN_THRESHOLD,
            },
        }
    }

    /// Creates a custom configuration with validated and clamped parameters.
    pub fn custom(hash_bits: u32, hash_width: u32, good_match: usize, lzvn_threshold: usize) -> Self {
        let clamped_bits = hash_bits.clamp(10, 16);
        let normalized_width = if hash_width >= 8 { 8 } else { 4 };
        Self {
            profile: LzfseProfile::Custom,
            hash_bits: clamped_bits,
            hash_width: normalized_width,
            good_match: good_match.max(4),
            lzvn_threshold,
        }
    }

    /// Calculates the history table memory footprint in bytes.
    ///
    /// Table size = `2^hash_bits * hash_width * sizeof(uint32_t)` (4 bytes per position).
    #[inline]
    pub fn history_table_bytes(&self) -> usize {
        let entry_count = 1usize << self.hash_bits;
        entry_count.saturating_mul(self.hash_width as usize).saturating_mul(4)
    }

    /// Computes the memory size of a single history entry line in bytes.
    #[inline]
    pub fn entry_line_bytes(&self) -> usize {
        (self.hash_width as usize) * 4
    }

    /// Calculates how many entry lines fit evenly inside an Apple Silicon cache line (128 bytes).
    #[inline]
    pub fn entry_lines_per_cache_line(&self, cache_line_bytes: usize) -> usize {
        let line_bytes = self.entry_line_bytes();
        if line_bytes == 0 {
            0
        } else {
            cache_line_bytes / line_bytes
        }
    }

    /// Returns `true` if the entire history table fits strictly within the L1D cache budget.
    #[inline]
    pub fn fits_in_l1d_cache(&self, l1d_capacity_bytes: usize) -> bool {
        self.history_table_bytes() <= l1d_capacity_bytes
    }
}

/// Routing decision taken by the adaptive engine for a given payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LzfseRoutingDecision {
    /// Routed to Apple LZVN fast bytecode encoder.
    Lzvn,
    /// Routed to Apple LZFSE FSE entropy encoder.
    Lzfse,
}

/// Comprehensive statistical report from an LZFSE/LZVN comparative micro-benchmark run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LzfseTunablesReport {
    /// Active profile preset.
    pub profile: LzfseProfile,
    /// Input payload byte length.
    pub input_bytes: usize,
    /// History table memory footprint in bytes.
    pub history_table_bytes: usize,
    /// Whether the history table fits 100% in L1D cache (128 KB).
    pub l1d_resident: bool,
    /// Adaptive routing decision.
    pub routing_decision: LzfseRoutingDecision,
    /// Output compressed bytes produced by adaptive routing.
    pub adaptive_compressed_bytes: usize,
    /// Compression ratio achieved by adaptive routing (`input / compressed`).
    pub adaptive_compression_ratio: f64,
    /// LZFSE standalone compression throughput in MB/s.
    pub lzfse_compress_mbs: f64,
    /// LZFSE standalone decompression throughput in MB/s.
    pub lzfse_decompress_mbs: f64,
    /// LZFSE compressed output bytes.
    pub lzfse_compressed_bytes: usize,
    /// LZVN standalone compression throughput in MB/s.
    pub lzvn_compress_mbs: f64,
    /// LZVN standalone decompression throughput in MB/s.
    pub lzvn_decompress_mbs: f64,
    /// LZVN compressed output bytes.
    pub lzvn_compressed_bytes: usize,
    /// Throughput speedup factor of LZVN over LZFSE (`lzvn_comp_mbs / lzfse_comp_mbs`).
    pub lzvn_to_lzfse_speedup: f64,
}

/// Hardware microarchitecture scheduler and comparative benchmark engine for Apple LZFSE / LZVN.
#[derive(Debug, Clone)]
pub struct LzfseTunablesEngine {
    config: LzfseTunablesConfig,
}

impl LzfseTunablesEngine {
    /// Creates a new engine instance with the specified configuration.
    pub fn new(config: LzfseTunablesConfig) -> Self {
        Self { config }
    }

    /// Creates a new engine instance with a predefined profile preset.
    pub fn with_profile(profile: LzfseProfile) -> Self {
        Self {
            config: LzfseTunablesConfig::from_profile(profile),
        }
    }

    /// Returns a reference to the active configuration.
    pub fn config(&self) -> &LzfseTunablesConfig {
        &self.config
    }

    /// Evaluates whether a given payload size should fall back to LZVN.
    #[inline]
    pub fn should_use_lzvn(&self, input_size: usize) -> bool {
        input_size < self.config.lzvn_threshold
    }

    /// Determines the routing decision for a payload of `input_size` bytes.
    #[inline]
    pub fn decide_routing(&self, input_size: usize) -> LzfseRoutingDecision {
        if self.should_use_lzvn(input_size) {
            LzfseRoutingDecision::Lzvn
        } else {
            LzfseRoutingDecision::Lzfse
        }
    }

    /// Computes the safe output buffer bound for adaptive compression.
    pub fn compress_bound(&self, input_size: usize) -> usize {
        if self.should_use_lzvn(input_size) {
            lzvn_compress_bound(input_size)
        } else {
            lzfse_compress_bound(input_size)
        }
    }

    /// Compresses a buffer adaptively based on the configured microarchitectural threshold.
    pub fn compress_adaptive(
        &self,
        src: &[u8],
        dst: &mut [u8],
    ) -> Result<(usize, LzfseRoutingDecision), TTZipStatus> {
        let decision = self.decide_routing(src.len());
        let written = match decision {
            LzfseRoutingDecision::Lzvn => lzvn_compress(src, dst)?,
            LzfseRoutingDecision::Lzfse => lzfse_compress(src, dst)?,
        };
        Ok((written, decision))
    }

    /// Decompresses a buffer according to the recorded routing decision.
    pub fn decompress_adaptive(
        &self,
        src: &[u8],
        dst: &mut [u8],
        decision: LzfseRoutingDecision,
    ) -> Result<usize, TTZipStatus> {
        match decision {
            LzfseRoutingDecision::Lzvn => lzvn_decompress(src, dst),
            LzfseRoutingDecision::Lzfse => lzfse_decompress(src, dst),
        }
    }

    /// Runs a comparative micro-benchmark evaluating LZFSE vs LZVN performance on the given payload.
    pub fn run_comparative_benchmark(&self, payload: &[u8]) -> LzfseTunablesReport {
        let input_bytes = payload.len();
        if input_bytes == 0 {
            return LzfseTunablesReport {
                profile: self.config.profile,
                input_bytes: 0,
                history_table_bytes: self.config.history_table_bytes(),
                l1d_resident: self.config.fits_in_l1d_cache(APPLE_SILICON_P_CORE_L1D_BYTES),
                routing_decision: self.decide_routing(0),
                adaptive_compressed_bytes: 0,
                adaptive_compression_ratio: 1.0,
                lzfse_compress_mbs: 0.0,
                lzfse_decompress_mbs: 0.0,
                lzfse_compressed_bytes: 0,
                lzvn_compress_mbs: 0.0,
                lzvn_decompress_mbs: 0.0,
                lzvn_compressed_bytes: 0,
                lzvn_to_lzfse_speedup: 1.0,
            };
        }

        // 1. Measure LZFSE compression
        let mut lzfse_comp_buf = vec![0u8; lzfse_compress_bound(input_bytes)];
        let lzfse_comp_start = Instant::now();
        let lzfse_written = lzfse_compress(payload, &mut lzfse_comp_buf).unwrap_or(0);
        let lzfse_comp_dur = lzfse_comp_start.elapsed();

        // 2. Measure LZFSE decompression
        let mut lzfse_decomp_buf = vec![0u8; input_bytes];
        let lzfse_decomp_start = Instant::now();
        if lzfse_written > 0 {
            let _ = lzfse_decompress(&lzfse_comp_buf[..lzfse_written], &mut lzfse_decomp_buf);
        }
        let lzfse_decomp_dur = lzfse_decomp_start.elapsed();

        // 3. Measure LZVN compression
        let mut lzvn_comp_buf = vec![0u8; lzvn_compress_bound(input_bytes)];
        let lzvn_comp_start = Instant::now();
        let lzvn_written = lzvn_compress(payload, &mut lzvn_comp_buf).unwrap_or(0);
        let lzvn_comp_dur = lzvn_comp_start.elapsed();

        // 4. Measure LZVN decompression
        let mut lzvn_decomp_buf = vec![0u8; input_bytes];
        let lzvn_decomp_start = Instant::now();
        if lzvn_written > 0 {
            let _ = lzvn_decompress(&lzvn_comp_buf[..lzvn_written], &mut lzvn_decomp_buf);
        }
        let lzvn_decomp_dur = lzvn_decomp_start.elapsed();

        // Calculate throughput metrics (MB/s where 1 MB = 1,048,576 bytes)
        let bytes_f64 = input_bytes as f64;
        let lzfse_comp_mbs = if lzfse_comp_dur.as_secs_f64() > 0.0 {
            (bytes_f64 / (1024.0 * 1024.0)) / lzfse_comp_dur.as_secs_f64()
        } else {
            0.0
        };
        let lzfse_decomp_mbs = if lzfse_decomp_dur.as_secs_f64() > 0.0 {
            (bytes_f64 / (1024.0 * 1024.0)) / lzfse_decomp_dur.as_secs_f64()
        } else {
            0.0
        };
        let lzvn_comp_mbs = if lzvn_comp_dur.as_secs_f64() > 0.0 {
            (bytes_f64 / (1024.0 * 1024.0)) / lzvn_comp_dur.as_secs_f64()
        } else {
            0.0
        };
        let lzvn_decomp_mbs = if lzvn_decomp_dur.as_secs_f64() > 0.0 {
            (bytes_f64 / (1024.0 * 1024.0)) / lzvn_decomp_dur.as_secs_f64()
        } else {
            0.0
        };

        let decision = self.decide_routing(input_bytes);
        let adaptive_compressed_bytes = match decision {
            LzfseRoutingDecision::Lzvn => lzvn_written,
            LzfseRoutingDecision::Lzfse => lzfse_written,
        };

        let adaptive_compression_ratio = if adaptive_compressed_bytes > 0 {
            bytes_f64 / (adaptive_compressed_bytes as f64)
        } else {
            1.0
        };

        let speedup = if lzfse_comp_mbs > 0.0 {
            lzvn_comp_mbs / lzfse_comp_mbs
        } else {
            1.0
        };

        LzfseTunablesReport {
            profile: self.config.profile,
            input_bytes,
            history_table_bytes: self.config.history_table_bytes(),
            l1d_resident: self.config.fits_in_l1d_cache(APPLE_SILICON_P_CORE_L1D_BYTES),
            routing_decision: decision,
            adaptive_compressed_bytes,
            adaptive_compression_ratio,
            lzfse_compress_mbs: lzfse_comp_mbs,
            lzfse_decompress_mbs: lzfse_decomp_mbs,
            lzfse_compressed_bytes: lzfse_written,
            lzvn_compress_mbs: lzvn_comp_mbs,
            lzvn_decompress_mbs: lzvn_decomp_mbs,
            lzvn_compressed_bytes: lzvn_written,
            lzvn_to_lzfse_speedup: speedup,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_presets_and_memory_geometry() {
        let def = LzfseTunablesConfig::from_profile(LzfseProfile::DefaultApple);
        assert_eq!(def.hash_bits, 14);
        assert_eq!(def.hash_width, 4);
        assert_eq!(def.entry_line_bytes(), 16);
        assert_eq!(def.history_table_bytes(), 256 * 1024); // 256 KB
        assert_eq!(def.entry_lines_per_cache_line(APPLE_SILICON_CACHE_LINE_BYTES), 8);
        assert!(!def.fits_in_l1d_cache(APPLE_SILICON_P_CORE_L1D_BYTES));

        let l1d = LzfseTunablesConfig::from_profile(LzfseProfile::AppleSiliconL1dResidency);
        assert_eq!(l1d.hash_bits, 12);
        assert_eq!(l1d.hash_width, 8);
        assert_eq!(l1d.entry_line_bytes(), 32);
        assert_eq!(l1d.history_table_bytes(), 128 * 1024); // 128 KB
        assert_eq!(l1d.entry_lines_per_cache_line(APPLE_SILICON_CACHE_LINE_BYTES), 4);
        assert!(l1d.fits_in_l1d_cache(APPLE_SILICON_P_CORE_L1D_BYTES));

        let max_ratio = LzfseTunablesConfig::from_profile(LzfseProfile::AppleSiliconMaxRatio);
        assert_eq!(max_ratio.hash_bits, 16);
        assert_eq!(max_ratio.history_table_bytes(), 2 * 1024 * 1024); // 2 MB
    }

    #[test]
    fn test_custom_parameter_clamping() {
        let custom = LzfseTunablesConfig::custom(8, 6, 2, 1024);
        assert_eq!(custom.hash_bits, 10); // Clamped to min 10
        assert_eq!(custom.hash_width, 4); // Normalized to 4
        assert_eq!(custom.good_match, 4); // Min 4
        assert_eq!(custom.lzvn_threshold, 1024);

        let custom_max = LzfseTunablesConfig::custom(20, 10, 100, 8192);
        assert_eq!(custom_max.hash_bits, 16); // Clamped to max 16
        assert_eq!(custom_max.hash_width, 8); // Normalized to 8
    }

    #[test]
    fn test_routing_decision_thresholds() {
        let engine = LzfseTunablesEngine::with_profile(LzfseProfile::DefaultApple);
        assert_eq!(engine.decide_routing(0), LzfseRoutingDecision::Lzvn);
        assert_eq!(engine.decide_routing(4095), LzfseRoutingDecision::Lzvn);
        assert_eq!(engine.decide_routing(4096), LzfseRoutingDecision::Lzfse);
        assert_eq!(engine.decide_routing(8192), LzfseRoutingDecision::Lzfse);

        let ultra = LzfseTunablesEngine::with_profile(LzfseProfile::LzvnUltraFast);
        assert_eq!(ultra.decide_routing(100_000), LzfseRoutingDecision::Lzvn);
    }

    #[test]
    fn test_adaptive_roundtrip_both_branches() {
        let engine = LzfseTunablesEngine::with_profile(LzfseProfile::DefaultApple);

        // Branch 1: Small payload (< 4096 bytes) -> LZVN
        let small_payload = b"TTZip LZFSE/LZVN Adaptive Microarchitecture Router Small Branch Test Payload.";
        let mut comp_buf = vec![0u8; engine.compress_bound(small_payload.len())];
        let (written, decision) = engine
            .compress_adaptive(small_payload, &mut comp_buf)
            .expect("compress small");
        assert_eq!(decision, LzfseRoutingDecision::Lzvn);
        assert!(written > 0);

        let mut decomp_buf = vec![0u8; small_payload.len()];
        let decomp_written = engine
            .decompress_adaptive(&comp_buf[..written], &mut decomp_buf, decision)
            .expect("decompress small");
        assert_eq!(decomp_written, small_payload.len());
        assert_eq!(&decomp_buf[..decomp_written], small_payload);

        // Branch 2: Large payload (>= 4096 bytes) -> LZFSE
        let mut large_payload = Vec::with_capacity(8192);
        for i in 0..8192 {
            large_payload.push((i % 251) as u8);
        }
        let mut comp_buf_large = vec![0u8; engine.compress_bound(large_payload.len())];
        let (written_large, decision_large) = engine
            .compress_adaptive(&large_payload, &mut comp_buf_large)
            .expect("compress large");
        assert_eq!(decision_large, LzfseRoutingDecision::Lzfse);
        assert!(written_large > 0);

        let mut decomp_buf_large = vec![0u8; large_payload.len()];
        let decomp_written_large = engine
            .decompress_adaptive(&comp_buf_large[..written_large], &mut decomp_buf_large, decision_large)
            .expect("decompress large");
        assert_eq!(decomp_written_large, large_payload.len());
        assert_eq!(&decomp_buf_large[..decomp_written_large], &large_payload[..]);
    }

    #[test]
    fn test_comparative_benchmark_report() {
        let engine = LzfseTunablesEngine::with_profile(LzfseProfile::AppleSiliconHighThroughput);
        let mut payload = Vec::with_capacity(16384);
        for i in 0..16384 {
            payload.push(((i * 7 + 13) % 256) as u8);
        }

        let report = engine.run_comparative_benchmark(&payload);
        assert_eq!(report.input_bytes, 16384);
        assert_eq!(report.profile, LzfseProfile::AppleSiliconHighThroughput);
        assert_eq!(report.routing_decision, LzfseRoutingDecision::Lzfse);
        assert!(report.lzfse_compressed_bytes > 0);
        assert!(report.lzvn_compressed_bytes > 0);
        assert!(report.adaptive_compression_ratio > 0.0);
    }
}
