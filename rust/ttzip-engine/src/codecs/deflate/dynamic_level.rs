// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Dynamic compression level and Deflate block type runtime scheduler.
//!
//! Provides:
//! - Online level switching (Levels 0..=9) without reallocating session state.
//! - RFC 1951 block type adaptive arbitration (`Stored`, `StaticHuffman`, `DynamicHuffman`).
//! - Feedback-driven adaptive throughput budgeting and dynamic level scaling.

/// RFC 1951 Deflate block type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeflateBlockType {
    /// RFC 1951 BTYPE=00: Uncompressed raw stored block (5-byte header, 0% CPU Huffman overhead).
    Stored,
    /// RFC 1951 BTYPE=01: Pre-defined fixed/static Huffman trees (zero tree description overhead).
    StaticHuffman,
    /// RFC 1951 BTYPE=02: Custom dynamic Huffman trees with optimal 2-level precode tables.
    DynamicHuffman,
}

/// Operational target profile for the dynamic scheduler.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PerformanceProfile {
    /// Prioritizes maximum throughput over ratio.
    MaxSpeed,
    /// Balanced throughput and compression ratio (standard default).
    Balanced,
    /// Prioritizes maximum compression ratio.
    MaxCompression,
    /// Dynamically shifts compression level to satisfy a target throughput budget in MB/s.
    AdaptiveBudget {
        /// Target minimum throughput in MB/s.
        target_mb_per_sec: f64,
    },
}

/// Runtime scheduler performance metrics.
#[derive(Debug, Clone, Copy, Default)]
pub struct SchedulerMetrics {
    /// Total uncompressed input bytes processed.
    pub total_input_bytes: u64,
    /// Total compressed output bytes emitted.
    pub total_output_bytes: u64,
    /// Total execution time elapsed in nanoseconds.
    pub total_elapsed_nanos: u64,
    /// Total block decisions made.
    pub block_count: u64,
}

impl SchedulerMetrics {
    /// Computes historical average throughput in MB/s.
    pub fn average_mb_per_sec(&self) -> f64 {
        if self.total_elapsed_nanos == 0 {
            return 0.0;
        }
        let seconds = (self.total_elapsed_nanos as f64) / 1_000_000_000.0;
        let mb = (self.total_input_bytes as f64) / (1024.0 * 1024.0);
        mb / seconds
    }

    /// Computes overall compression ratio (compressed / uncompressed).
    pub fn compression_ratio(&self) -> f64 {
        if self.total_input_bytes == 0 {
            return 1.0;
        }
        (self.total_output_bytes as f64) / (self.total_input_bytes as f64)
    }
}

/// Dynamic compression level and block type scheduler.
#[derive(Debug, Clone)]
pub struct DynamicLevelScheduler {
    /// Current active compression level (0..=9).
    current_level: u32,
    /// Target performance profile.
    profile: PerformanceProfile,
    /// Rolling metrics accumulator.
    metrics: SchedulerMetrics,
}

impl Default for DynamicLevelScheduler {
    fn default() -> Self {
        Self::new(6, PerformanceProfile::Balanced)
    }
}

impl DynamicLevelScheduler {
    /// Creates a new scheduler initialized with level and performance profile.
    pub fn new(initial_level: u32, profile: PerformanceProfile) -> Self {
        Self {
            current_level: initial_level.clamp(0, 9),
            profile,
            metrics: SchedulerMetrics::default(),
        }
    }

    /// Returns the current active compression level (0..=9).
    #[inline]
    pub fn current_level(&self) -> u32 {
        self.current_level
    }

    /// Dynamically switches the active compression level at runtime.
    pub fn set_level(&mut self, level: u32) {
        self.current_level = level.clamp(0, 9);
    }

    /// Updates the target performance profile.
    pub fn set_profile(&mut self, profile: PerformanceProfile) {
        self.profile = profile;
        match profile {
            PerformanceProfile::MaxSpeed => self.current_level = 1,
            PerformanceProfile::Balanced => self.current_level = 6,
            PerformanceProfile::MaxCompression => self.current_level = 9,
            PerformanceProfile::AdaptiveBudget { .. } => {}
        }
    }

    /// Returns the active performance profile.
    #[inline]
    pub fn profile(&self) -> PerformanceProfile {
        self.profile
    }

    /// Returns runtime statistics and metrics.
    #[inline]
    pub fn metrics(&self) -> &SchedulerMetrics {
        &self.metrics
    }

    /// Selects the optimal RFC 1951 Deflate block type for a given block.
    ///
    /// Evaluates block size, match density, and compression level to minimize total bit cost.
    #[inline]
    pub fn select_block_type(
        &self,
        block_size: usize,
        num_matches: usize,
        is_high_entropy: bool,
    ) -> DeflateBlockType {
        // Level 0 or empty/trivial blocks use Stored
        if self.current_level == 0 || block_size == 0 {
            return DeflateBlockType::Stored;
        }

        // Incompressible data without sufficient matches is emitted as Stored
        if is_high_entropy && num_matches == 0 {
            return DeflateBlockType::Stored;
        }

        // Very small blocks (< 128 bytes) or low-match blocks avoid Dynamic Huffman tree overhead
        if block_size < 128 || (block_size < 512 && num_matches < 4) {
            return DeflateBlockType::StaticHuffman;
        }

        // Standard and high levels for compressible payloads use Dynamic Huffman
        DeflateBlockType::DynamicHuffman
    }

    /// Records block execution feedback and dynamically adjusts level if using `AdaptiveBudget`.
    pub fn record_feedback(
        &mut self,
        uncompressed_bytes: usize,
        compressed_bytes: usize,
        elapsed_nanos: u64,
    ) {
        self.metrics.total_input_bytes += uncompressed_bytes as u64;
        self.metrics.total_output_bytes += compressed_bytes as u64;
        self.metrics.total_elapsed_nanos += elapsed_nanos;
        self.metrics.block_count += 1;

        if let PerformanceProfile::AdaptiveBudget { target_mb_per_sec } = self.profile {
            if elapsed_nanos > 0 && uncompressed_bytes > 0 {
                let block_seconds = (elapsed_nanos as f64) / 1_000_000_000.0;
                let block_mb = (uncompressed_bytes as f64) / (1024.0 * 1024.0);
                let instant_mb_per_sec = block_mb / block_seconds;

                // Adjust level dynamically
                if instant_mb_per_sec < target_mb_per_sec * 0.85 && self.current_level > 1 {
                    // Running behind throughput budget: step down level to speed up
                    self.current_level -= 1;
                } else if instant_mb_per_sec > target_mb_per_sec * 1.30 && self.current_level < 9 {
                    // Outperforming throughput budget: step up level to improve compression ratio
                    self.current_level += 1;
                }
            }
        }
    }
}
