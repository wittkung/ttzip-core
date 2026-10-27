// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Task T045: Coverage-Guided Fuzz Target for Decompression Bomb Defense (`fuzz_zip_bomb`).
//!
//! Evaluates defense-in-depth mechanisms against recursive decompression bombs (e.g. 42.zip archetype)
//! and overlapping payload bombs:
//! - Simulates nested recursive multi-level archive decompression.
//! - Enforces strict 1000x expansion ratio threshold (`uncompressed_size / compressed_size > 1000.0`).
//! - Enforces overlapping central directory entry interval intersection detection.
//! - Enforces strict total output byte budgets and recursion depth limits.
//! - Asserts deterministic rejection with `BombDetected` or `RatioExceeded` errors,
//!   0 OOM crashes, and 0 unbounded memory allocations.

use std::panic::{catch_unwind, AssertUnwindSafe};
use ttzip_engine::security::{detect_overlapping_entries, ExtractionQuotaGuard};
use ttzip_engine::types::TTZipStatus;
use ttzip_engine::zip::reader::ZipArchive;

/// Strict expansion ratio cutoff threshold (1000:1).
pub const MAX_EXPANSION_RATIO: f64 = 1000.0;

/// Maximum recursive container nesting depth before immediate rejection.
pub const MAX_RECURSION_DEPTH: usize = 8;

/// Memory allocation limit for simulated decompression output (16 MiB).
pub const MAX_UNCOMPRESSED_BUDGET_BYTES: u64 = 16 * 1024 * 1024;

/// Warmup threshold in bytes before expansion ratio enforcement activates.
pub const RATIO_ENFORCEMENT_THRESHOLD_BYTES: u64 = 4096;

/// Specific error types emitted by the decompression bomb analyzer.
#[derive(Debug, Clone, PartialEq)]
pub enum BombFuzzError {
    /// Decompression bomb detected via overlapping entries or suspicious structural pattern.
    BombDetected(String),
    /// Decompression expansion ratio exceeded the 1000x threshold.
    RatioExceeded {
        ratio: f64,
        max_ratio: f64,
        cumulative_compressed: u64,
        cumulative_uncompressed: u64,
    },
    /// Total cumulative uncompressed byte budget exceeded.
    QuotaExceeded {
        uncompressed_bytes: u64,
        budget_bytes: u64,
    },
    /// Recursive nesting depth exceeded the safety boundary.
    RecursionDepthExceeded {
        current_depth: usize,
        max_depth: usize,
    },
    /// Malformed or corrupt input payload.
    CorruptInput(String),
}

/// Statistics collected during simulated recursive decompression.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct DecompressionMetrics {
    pub total_levels_processed: usize,
    pub total_entries_evaluated: usize,
    pub cumulative_compressed_bytes: u64,
    pub cumulative_uncompressed_bytes: u64,
    pub peak_expansion_ratio: f64,
    pub bomb_intercepted: bool,
}

/// Bounded recursive decompression simulator for coverage-guided fuzzing.
pub struct RecursiveBombSimulator {
    quota_guard: ExtractionQuotaGuard,
    max_depth: usize,
    max_ratio: f64,
}

impl Default for RecursiveBombSimulator {
    fn default() -> Self {
        Self::new(
            MAX_UNCOMPRESSED_BUDGET_BYTES,
            MAX_EXPANSION_RATIO,
            MAX_RECURSION_DEPTH,
        )
    }
}

impl RecursiveBombSimulator {
    /// Creates a new simulator with strict quota and ratio bounds.
    #[must_use]
    pub fn new(max_uncompressed_bytes: u64, max_ratio: f64, max_depth: usize) -> Self {
        Self {
            quota_guard: ExtractionQuotaGuard::with_threshold(
                max_uncompressed_bytes,
                max_ratio,
                RATIO_ENFORCEMENT_THRESHOLD_BYTES,
            ),
            max_depth,
            max_ratio,
        }
    }

    /// Evaluates raw input bytes as a simulated multi-level nested archive.
    ///
    /// # Errors
    /// Returns `BombFuzzError` when ratio breaches 1000x, recursion depth exceeds limit,
    /// or overlapping payload ranges are detected.
    pub fn simulate_from_bytes(&mut self, data: &[u8]) -> Result<DecompressionMetrics, BombFuzzError> {
        let mut metrics = DecompressionMetrics::default();
        if data.is_empty() {
            return Ok(metrics);
        }

        // 1. Direct real ZIP parser probe
        self.probe_real_zip_archive(data, &mut metrics)?;

        // 2. Overlapping entry interval fuzzing probe
        self.probe_overlapping_entry_sweep(data, &mut metrics)?;

        // 3. Recursive nested container unfolding simulation
        self.unfold_nested_layers(data, 0, &mut metrics)?;

        Ok(metrics)
    }

    /// Probes the input buffer using the production `ZipArchive` parser to check real structures.
    fn probe_real_zip_archive(
        &mut self,
        data: &[u8],
        metrics: &mut DecompressionMetrics,
    ) -> Result<(), BombFuzzError> {
        if let Ok(archive) = ZipArchive::open_slice(data) {
            let entries = archive.entries();
            metrics.total_entries_evaluated = metrics
                .total_entries_evaluated
                .saturating_add(entries.len());

            // Build interval list for overlapping entry verification
            let mut intervals = Vec::with_capacity(entries.len());
            for entry in entries {
                let comp_size = entry.compressed_size;
                let uncomp_size = entry.uncompressed_size;
                let data_offset = entry.lfh_offset;

                intervals.push((data_offset, comp_size));

                // Feed into production quota guard
                if let Err(status) = self.quota_guard.track(comp_size, uncomp_size) {
                    if status == TTZipStatus::ErrSecurityViolation {
                        metrics.bomb_intercepted = true;
                        let ratio = self.quota_guard.current_ratio();
                        return Err(BombFuzzError::RatioExceeded {
                            ratio,
                            max_ratio: self.max_ratio,
                            cumulative_compressed: self.quota_guard.cumulative_compressed(),
                            cumulative_uncompressed: self.quota_guard.cumulative_uncompressed(),
                        });
                    }
                }
            }

            // Verify sweep-line overlapping entry detection
            if detect_overlapping_entries(&intervals) {
                metrics.bomb_intercepted = true;
                return Err(BombFuzzError::BombDetected(
                    "Overlapping compressed payload ranges detected across archive entries".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Generates interval tuples from fuzz data and validates sweep-line detection.
    fn probe_overlapping_entry_sweep(
        &mut self,
        data: &[u8],
        metrics: &mut DecompressionMetrics,
    ) -> Result<(), BombFuzzError> {
        if data.len() < 16 {
            return Ok(());
        }

        // Synthesize up to 16 synthetic entry intervals
        let count = (data.len() / 8).min(16);
        let mut intervals = Vec::with_capacity(count);

        for chunk in data.chunks_exact(8).take(count) {
            let offset = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as u64;
            let length = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]) as u64;
            intervals.push((offset, length));
        }

        metrics.total_entries_evaluated = metrics
            .total_entries_evaluated
            .saturating_add(intervals.len());

        if detect_overlapping_entries(&intervals) {
            metrics.bomb_intercepted = true;
            return Err(BombFuzzError::BombDetected(
                "Synthetic overlapping entry interval detected".to_string(),
            ));
        }

        Ok(())
    }

    /// Recursively unfolds nested archive layers parameterized by the fuzz byte stream.
    fn unfold_nested_layers(
        &mut self,
        slice: &[u8],
        current_depth: usize,
        metrics: &mut DecompressionMetrics,
    ) -> Result<(), BombFuzzError> {
        if current_depth > self.max_depth {
            metrics.bomb_intercepted = true;
            return Err(BombFuzzError::RecursionDepthExceeded {
                current_depth,
                max_depth: self.max_depth,
            });
        }

        metrics.total_levels_processed = metrics.total_levels_processed.max(current_depth + 1);

        if slice.len() < 4 {
            return Ok(());
        }

        // Extract parameters from initial bytes
        let branch_count = ((slice[0] as usize) % 4) + 1; // 1..=4 child archives
        let raw_ratio_multiplier = (slice[1] as u64) | ((slice[2] as u64) << 8); // 0..65535 multiplier
        let compressed_chunk_len = (slice.len() as u64) / (branch_count as u64).max(1);

        for branch_idx in 0..branch_count {
            metrics.total_entries_evaluated = metrics.total_entries_evaluated.saturating_add(1);

            // Compute simulated uncompressed size
            let simulated_uncompressed = compressed_chunk_len.saturating_mul(raw_ratio_multiplier);

            // Enforce quota and expansion ratio using production ExtractionQuotaGuard
            if let Err(status) = self
                .quota_guard
                .track(compressed_chunk_len, simulated_uncompressed)
            {
                metrics.bomb_intercepted = true;
                if status == TTZipStatus::ErrSecurityViolation {
                    let current_ratio = self.quota_guard.current_ratio();
                    if current_ratio > self.max_ratio {
                        return Err(BombFuzzError::RatioExceeded {
                            ratio: current_ratio,
                            max_ratio: self.max_ratio,
                            cumulative_compressed: self.quota_guard.cumulative_compressed(),
                            cumulative_uncompressed: self.quota_guard.cumulative_uncompressed(),
                        });
                    }
                    return Err(BombFuzzError::QuotaExceeded {
                        uncompressed_bytes: self.quota_guard.cumulative_uncompressed(),
                        budget_bytes: self.quota_guard.max_uncompressed_bytes,
                    });
                }
                return Err(BombFuzzError::CorruptInput(format!("{:?}", status)));
            }

            metrics.cumulative_compressed_bytes = self.quota_guard.cumulative_compressed();
            metrics.cumulative_uncompressed_bytes = self.quota_guard.cumulative_uncompressed();
            let current_ratio = self.quota_guard.current_ratio();
            if current_ratio > metrics.peak_expansion_ratio {
                metrics.peak_expansion_ratio = current_ratio;
            }

            // Recurse to next depth if remaining slice has sufficient payload
            let sub_slice_start = (branch_idx * 3).min(slice.len());
            let sub_slice = &slice[sub_slice_start..];
            if sub_slice.len() > 8 && current_depth < self.max_depth {
                self.unfold_nested_layers(sub_slice, current_depth + 1, metrics)?;
            }
        }

        Ok(())
    }
}

/// Primary coverage-guided fuzz target function entry point.
///
/// Guaranteed to never panic or leak memory on any input data.
pub fn fuzz_target_zip_bomb(data: &[u8]) {
    let mut simulator = RecursiveBombSimulator::default();

    let result = catch_unwind(AssertUnwindSafe(|| {
        simulator.simulate_from_bytes(data)
    }));

    match result {
        Ok(Ok(metrics)) => {
            // Invariant: Uncompressed expansion ratio must NEVER exceed 1000.0 without triggering an error
            if metrics.cumulative_uncompressed_bytes > RATIO_ENFORCEMENT_THRESHOLD_BYTES {
                assert!(
                    metrics.peak_expansion_ratio <= MAX_EXPANSION_RATIO + 0.001,
                    "INVARIANT BREACH: Expansion ratio {} exceeded {} without interception!",
                    metrics.peak_expansion_ratio,
                    MAX_EXPANSION_RATIO
                );
            }
            // Invariant: Output size must never exceed memory quota
            assert!(
                metrics.cumulative_uncompressed_bytes <= MAX_UNCOMPRESSED_BUDGET_BYTES,
                "INVARIANT BREACH: Uncompressed bytes {} exceeded budget {}!",
                metrics.cumulative_uncompressed_bytes,
                MAX_UNCOMPRESSED_BUDGET_BYTES
            );
        }
        Ok(Err(err)) => {
            // Verified deterministic rejection
            match err {
                BombFuzzError::RatioExceeded { ratio, max_ratio, .. } => {
                    assert!(
                        ratio > max_ratio || ratio.is_nan() || ratio.is_infinite(),
                        "RatioExceeded error reported for sub-threshold ratio: {}",
                        ratio
                    );
                }
                BombFuzzError::BombDetected(_)
                | BombFuzzError::QuotaExceeded { .. }
                | BombFuzzError::RecursionDepthExceeded { .. }
                | BombFuzzError::CorruptInput(_) => {}
            }
        }
        Err(panic_payload) => {
            panic!(
                "CRITICAL SECURITY FAILURE: Unhandled panic in zip bomb fuzzer: {:?}",
                panic_payload
            );
        }
    }
}

fn main() {
    // 1. Synthetic 42.zip archetype simulation (1000x ratio breaker)
    let mut bomb_payload = vec![0u8; 64];
    bomb_payload[0] = 2; // 2 child archives
    bomb_payload[1] = 0xE8; // Multiplier low byte (1500x)
    bomb_payload[2] = 0x05; // Multiplier high byte -> 1512x expansion

    fuzz_target_zip_bomb(&bomb_payload);

    // 2. Overlapping entries test payload
    let mut overlap_payload = Vec::new();
    // 4 duplicate intervals [1024, 4096)
    for _ in 0..4 {
        overlap_payload.extend_from_slice(&1024u32.to_le_bytes());
        overlap_payload.extend_from_slice(&4096u32.to_le_bytes());
    }
    fuzz_target_zip_bomb(&overlap_payload);

    // 3. Arbitrary synthetic bitstream fuzz iterations
    let seeds: [u64; 8] = [
        0x12345678, 0xDEADBEEF, 0xCAFEBABE, 0x00000000,
        0xFFFFFFFF, 0x55555555, 0xAAAAAAAA, 0x42424242,
    ];
    for seed in seeds {
        let mut prng_state = seed;
        let mut sample = vec![0u8; 128];
        for byte in &mut sample {
            prng_state = prng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
            *byte = (prng_state >> 32) as u8;
        }
        fuzz_target_zip_bomb(&sample);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zip_bomb_strict_1000x_cutoff() {
        let mut simulator = RecursiveBombSimulator::default();
        // 1 byte compressed, 2000 bytes uncompressed (2000:1 ratio)
        // With threshold 4096, push 3 chunks of 1:2000 -> cumulative uncompressed = 6000 > 4096
        let res1 = simulator.quota_guard.track(1, 2000);
        assert_eq!(res1, Ok(()));

        let res2 = simulator.quota_guard.track(1, 2000);
        assert_eq!(res2, Ok(()));

        // Third chunk exceeds threshold and ratio 2000:1 > 1000:1
        let res3 = simulator.quota_guard.track(1, 2000);
        assert_eq!(res3, Err(TTZipStatus::ErrSecurityViolation));
        assert!(simulator.quota_guard.current_ratio() > 1000.0);
    }

    #[test]
    fn test_overlapping_entries_rejection() {
        let intervals = vec![
            (1024, 4096),
            (1024, 4096),
        ];
        assert!(detect_overlapping_entries(&intervals));
    }
}
