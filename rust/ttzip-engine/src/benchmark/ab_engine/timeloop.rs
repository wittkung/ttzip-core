// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! LZ4 Time-Loop Steady-State Benchmark & Single-Core Peak Throughput Engine.
//!
//! Inspired by Yann Collet's canonical benchmark harness in `vendor/lz4/programs/bench.c`:
//! - Integrates physical execution duration over a steady-state time window
//!   (`TIMELOOP_MICROS = 1_900_000`, 1.9s) to bypass CPU scaling ramps and cold-cache transients.
//! - Employs a Best-of-6 (`NB_TESTS = 6`) filtering model to capture peak single-core throughput.
//! - Leverages clock rising-edge alignment (`wait_for_next_tick`) to eliminate sub-tick quantization jitter.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::benchmark::ab_engine::timing::wait_for_next_tick_instant;

/// Target integration window in microseconds (1.9 seconds golden steady-state window).
pub const TIMELOOP_MICROS: u64 = 1_900_000;

/// Default number of test passes for Best-of-N steady-state filtering.
pub const NB_TESTS: usize = 6;

/// Default warmup loop count prior to starting measurement passes.
pub const DEFAULT_WARMUP_LOOPS: usize = 2;

/// Configuration parameters for [`Lz4TimeLoopBenchEngine`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeLoopConfig {
    /// Target duration per test pass in microseconds.
    pub timeloop_micros: u64,
    /// Total number of test passes to execute (e.g., Best-of-6).
    pub nb_tests: usize,
    /// Number of warmup iterations prior to running passes.
    pub warmup_loops: usize,
}

impl Default for TimeLoopConfig {
    fn default() -> Self {
        Self {
            timeloop_micros: TIMELOOP_MICROS,
            nb_tests: NB_TESTS,
            warmup_loops: DEFAULT_WARMUP_LOOPS,
        }
    }
}

impl TimeLoopConfig {
    /// Creates a configuration with customized window and pass count.
    pub fn new(timeloop_micros: u64, nb_tests: usize) -> Self {
        Self {
            timeloop_micros,
            nb_tests: nb_tests.max(1),
            warmup_loops: 0,
        }
    }

    /// Sets the warmup loop count.
    pub fn with_warmup(mut self, warmup_loops: usize) -> Self {
        self.warmup_loops = warmup_loops;
        self
    }
}

/// Detailed execution metrics for a single time-loop measurement pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeLoopPassResult {
    /// 1-based index of this measurement pass.
    pub pass_index: usize,
    /// Number of closure iterations completed during the integration window.
    pub loop_count: u64,
    /// Total payload bytes processed during this pass (`loop_count * payload_len`).
    pub total_bytes: u64,
    /// Measured physical duration in microseconds.
    pub elapsed_micros: f64,
    /// Measured physical duration in seconds.
    pub elapsed_secs: f64,
    /// Measured throughput in MB/s (1 MB = 1,048,576 bytes).
    pub throughput_mbs: f64,
    /// Average latency per single closure pass in nanoseconds.
    pub avg_latency_nanos: f64,
}

/// Aggregated statistical report across all time-loop measurement passes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeLoopStats {
    /// Best single-core throughput in MB/s (peak sustainable speed).
    pub best_throughput_mbs: f64,
    /// Average duration per closure execution in nanoseconds during the best pass.
    pub best_duration_per_pass_nanos: f64,
    /// Index of the best pass (1-based).
    pub best_pass_index: usize,
    /// Number of test passes executed.
    pub runs: usize,
    /// Configured target integration window in microseconds.
    pub timeloop_micros: u64,
    /// Total payload bytes processed across all passes.
    pub total_bytes_processed: u64,
    /// Cumulative physical execution duration across all passes in seconds.
    pub total_duration_secs: f64,
    /// Arithmetic mean throughput across all passes in MB/s.
    pub mean_throughput_mbs: f64,
    /// Median throughput across all passes in MB/s.
    pub median_throughput_mbs: f64,
    /// Minimum throughput recorded across passes in MB/s.
    pub min_throughput_mbs: f64,
    /// Maximum throughput recorded across passes in MB/s.
    pub max_throughput_mbs: f64,
    /// Sample standard deviation of throughput in MB/s.
    pub std_dev_mbs: f64,
    /// Relative Standard Error percentage (RSE %).
    pub rse_pct: f64,
    /// Individual breakdown of each measurement pass.
    pub passes: Vec<TimeLoopPassResult>,
}

/// LZ4-style time-loop steady-state benchmark engine.
#[derive(Debug, Clone)]
pub struct Lz4TimeLoopBenchEngine {
    config: TimeLoopConfig,
}

impl Default for Lz4TimeLoopBenchEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Lz4TimeLoopBenchEngine {
    /// Creates a new engine instance with default configuration (1.9s window, Best-of-6).
    pub fn new() -> Self {
        Self {
            config: TimeLoopConfig::default(),
        }
    }

    /// Creates an engine with custom configuration.
    pub fn with_config(config: TimeLoopConfig) -> Self {
        Self { config }
    }

    /// Creates an engine with custom integration window and test count.
    pub fn with_params(timeloop_micros: u64, nb_tests: usize) -> Self {
        Self {
            config: TimeLoopConfig::new(timeloop_micros, nb_tests),
        }
    }

    /// Returns a reference to the active configuration.
    pub fn config(&self) -> &TimeLoopConfig {
        &self.config
    }

    /// Benchmarks a closure using dual-loop physical time integration and Best-of-N filtering.
    ///
    /// # Arguments
    /// - `payload`: Source data slice being processed.
    /// - `runs`: Number of passes to execute. If `0`, defaults to `self.config.nb_tests`.
    /// - `f`: Closure performing the compression or decompression operation.
    pub fn benchmark_timeloop<F>(&self, payload: &[u8], runs: usize, mut f: F) -> TimeLoopStats
    where
        F: FnMut(),
    {
        let num_runs = if runs == 0 { self.config.nb_tests } else { runs };
        let target_duration = Duration::from_micros(self.config.timeloop_micros);
        let payload_len = payload.len() as u64;

        // Warmup phase
        for _ in 0..self.config.warmup_loops {
            f();
        }

        let mut passes = Vec::with_capacity(num_runs);
        let mut total_bytes_processed: u64 = 0;
        let mut total_duration_secs: f64 = 0.0;

        for pass_idx in 1..=num_runs {
            let start = wait_for_next_tick_instant();
            let mut loop_count: u64 = 0;

            while start.elapsed() < target_duration || loop_count == 0 {
                f();
                loop_count += 1;
            }

            let elapsed = start.elapsed();
            let elapsed_secs = elapsed.as_secs_f64();
            let elapsed_micros = elapsed_secs * 1_000_000.0;
            let pass_bytes = loop_count.saturating_mul(payload_len);

            let throughput_mbs = if elapsed_secs > 0.0 && payload_len > 0 {
                (pass_bytes as f64 / (1024.0 * 1024.0)) / elapsed_secs
            } else {
                0.0
            };

            let avg_latency_nanos = if loop_count > 0 {
                (elapsed.as_nanos() as f64) / (loop_count as f64)
            } else {
                0.0
            };

            total_bytes_processed = total_bytes_processed.saturating_add(pass_bytes);
            total_duration_secs += elapsed_secs;

            passes.push(TimeLoopPassResult {
                pass_index: pass_idx,
                loop_count,
                total_bytes: pass_bytes,
                elapsed_micros,
                elapsed_secs,
                throughput_mbs,
                avg_latency_nanos,
            });
        }

        Self::compute_aggregated_stats(
            passes,
            num_runs,
            self.config.timeloop_micros,
            total_bytes_processed,
            total_duration_secs,
            payload_len,
        )
    }

    /// Computes statistical aggregations and filters best-of-N metrics.
    fn compute_aggregated_stats(
        passes: Vec<TimeLoopPassResult>,
        runs: usize,
        timeloop_micros: u64,
        total_bytes_processed: u64,
        total_duration_secs: f64,
        payload_len: u64,
    ) -> TimeLoopStats {
        if passes.is_empty() {
            return TimeLoopStats {
                best_throughput_mbs: 0.0,
                best_duration_per_pass_nanos: 0.0,
                best_pass_index: 0,
                runs,
                timeloop_micros,
                total_bytes_processed: 0,
                total_duration_secs: 0.0,
                mean_throughput_mbs: 0.0,
                median_throughput_mbs: 0.0,
                min_throughput_mbs: 0.0,
                max_throughput_mbs: 0.0,
                std_dev_mbs: 0.0,
                rse_pct: 0.0,
                passes,
            };
        }

        // Identify best pass: highest throughput if payload > 0, otherwise lowest latency
        let mut best_pass_idx = 0;
        if payload_len > 0 {
            let mut max_tp = -1.0;
            for (idx, p) in passes.iter().enumerate() {
                if p.throughput_mbs > max_tp {
                    max_tp = p.throughput_mbs;
                    best_pass_idx = idx;
                }
            }
        } else {
            let mut min_lat = f64::MAX;
            for (idx, p) in passes.iter().enumerate() {
                if p.avg_latency_nanos < min_lat {
                    min_lat = p.avg_latency_nanos;
                    best_pass_idx = idx;
                }
            }
        }

        let best_pass = &passes[best_pass_idx];
        let best_throughput_mbs = best_pass.throughput_mbs;
        let best_duration_per_pass_nanos = best_pass.avg_latency_nanos;
        let best_pass_index = best_pass.pass_index;

        let n = passes.len() as f64;
        let sum_tp: f64 = passes.iter().map(|p| p.throughput_mbs).sum();
        let mean_throughput_mbs = sum_tp / n;

        // Sorted throughputs for median / min / max
        let mut sorted_tp: Vec<f64> = passes.iter().map(|p| p.throughput_mbs).collect();
        sorted_tp.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let min_throughput_mbs = sorted_tp.first().copied().unwrap_or(0.0);
        let max_throughput_mbs = sorted_tp.last().copied().unwrap_or(0.0);

        let median_throughput_mbs = if sorted_tp.is_empty() {
            0.0
        } else if sorted_tp.len() % 2 == 1 {
            sorted_tp[sorted_tp.len() / 2]
        } else {
            let mid = sorted_tp.len() / 2;
            (sorted_tp[mid - 1] + sorted_tp[mid]) / 2.0
        };

        let variance = if passes.len() > 1 {
            let sum_sq_diff: f64 = passes
                .iter()
                .map(|p| (p.throughput_mbs - mean_throughput_mbs).powi(2))
                .sum();
            sum_sq_diff / (n - 1.0)
        } else {
            0.0
        };

        let std_dev_mbs = variance.sqrt();
        let std_error = if n > 0.0 { std_dev_mbs / n.sqrt() } else { 0.0 };
        let rse_pct = if mean_throughput_mbs > 1e-12 {
            (std_error / mean_throughput_mbs) * 100.0
        } else {
            0.0
        };

        TimeLoopStats {
            best_throughput_mbs,
            best_duration_per_pass_nanos,
            best_pass_index,
            runs,
            timeloop_micros,
            total_bytes_processed,
            total_duration_secs,
            mean_throughput_mbs,
            median_throughput_mbs,
            min_throughput_mbs,
            max_throughput_mbs,
            std_dev_mbs,
            rse_pct,
            passes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wait_for_next_tick_instant() {
        let t1 = wait_for_next_tick_instant();
        let t2 = wait_for_next_tick_instant();
        assert!(t2 >= t1);
    }

    #[test]
    fn test_timeloop_engine_defaults() {
        let engine = Lz4TimeLoopBenchEngine::new();
        assert_eq!(engine.config().timeloop_micros, TIMELOOP_MICROS);
        assert_eq!(engine.config().nb_tests, NB_TESTS);
        assert_eq!(engine.config().warmup_loops, DEFAULT_WARMUP_LOOPS);
    }

    #[test]
    fn test_timeloop_bench_fast_execution() {
        // Fast 5ms window with Best-of-3
        let engine = Lz4TimeLoopBenchEngine::with_params(5_000, 3);
        let payload = vec![0xABu8; 64 * 1024]; // 64 KB

        let mut counter = 0usize;
        let stats = engine.benchmark_timeloop(&payload, 3, || {
            counter = counter.wrapping_add(1);
            std::hint::black_box(counter);
        });

        assert_eq!(stats.runs, 3);
        assert_eq!(stats.passes.len(), 3);
        assert!(stats.best_throughput_mbs > 0.0);
        assert!(stats.best_duration_per_pass_nanos > 0.0);
        assert!(stats.best_pass_index >= 1 && stats.best_pass_index <= 3);
        assert!(stats.total_bytes_processed > 0);
        assert!(stats.total_duration_secs > 0.0);
        assert!(stats.max_throughput_mbs >= stats.min_throughput_mbs);
        assert!((stats.best_throughput_mbs - stats.max_throughput_mbs).abs() < 1e-6);

        for pass in &stats.passes {
            assert!(pass.loop_count > 0);
            assert!(pass.elapsed_micros >= 4_000.0); // Close to or above 5ms
            assert_eq!(pass.total_bytes, pass.loop_count * (payload.len() as u64));
            assert!(pass.throughput_mbs > 0.0);
            assert!(pass.avg_latency_nanos > 0.0);
        }
    }

    #[test]
    fn test_timeloop_bench_zero_payload() {
        let engine = Lz4TimeLoopBenchEngine::with_params(2_000, 2);
        let payload = b"";

        let mut dummy = 0u64;
        let stats = engine.benchmark_timeloop(payload, 2, || {
            dummy = dummy.wrapping_add(42);
            std::hint::black_box(dummy);
        });

        assert_eq!(stats.runs, 2);
        assert_eq!(stats.total_bytes_processed, 0);
        assert_eq!(stats.best_throughput_mbs, 0.0);
        assert!(stats.best_duration_per_pass_nanos > 0.0);
    }

    #[test]
    fn test_timeloop_throughput_math_accuracy() {
        // Mathematical validation of (bytes / MB) / secs
        let pass = TimeLoopPassResult {
            pass_index: 1,
            loop_count: 1000,
            total_bytes: 1000 * 1024 * 1024, // 1000 MB
            elapsed_micros: 1_000_000.0,
            elapsed_secs: 1.0,
            throughput_mbs: 1000.0,
            avg_latency_nanos: 1_000_000.0,
        };

        let passes = vec![pass.clone()];
        let stats = Lz4TimeLoopBenchEngine::compute_aggregated_stats(
            passes,
            1,
            1_000_000,
            pass.total_bytes,
            pass.elapsed_secs,
            1024 * 1024,
        );

        assert!((stats.best_throughput_mbs - 1000.0).abs() < 1e-9);
        assert!((stats.mean_throughput_mbs - 1000.0).abs() < 1e-9);
        assert!((stats.median_throughput_mbs - 1000.0).abs() < 1e-9);
        assert_eq!(stats.std_dev_mbs, 0.0);
        assert_eq!(stats.rse_pct, 0.0);
    }
}
