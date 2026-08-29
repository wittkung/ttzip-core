// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Adaptive Timed Function Benchmark Engine (`BMK_benchTimedFn`).
//!
//! Provides adaptive workload estimation targeting a golden integration window (default 1000ms),
//! combined with Best-of-N interrupt rejection and Hampel MAD robust statistical filtering.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::benchmark::ab_engine::stats::{HampelFilter, WelchStudentTTest};
use crate::benchmark::ab_engine::timing::{get_hardware_monotonic_nanos, wait_for_next_tick};

/// Default target benchmark integration duration in milliseconds (1000ms golden window).
pub const DEFAULT_TARGET_DURATION_MS: u64 = 1000;

/// Default target duration as `Duration`.
pub const DEFAULT_TARGET_DURATION: Duration = Duration::from_millis(DEFAULT_TARGET_DURATION_MS);

/// Configuration for adaptive timed function benchmarking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimedFnConfig {
    /// Target execution integration duration per round (default 1000ms).
    pub target_duration: Duration,
    /// Number of benchmark measurement rounds (default 5).
    pub num_rounds: usize,
    /// Number of probe runs executed to estimate per-iteration cost (default 1).
    pub probe_runs: usize,
    /// Minimum allowed loop iterations per round (default 1).
    pub min_loops: usize,
    /// Maximum allowed loop iterations per round (default 100,000,000).
    pub max_loops: usize,
    /// Whether to apply Hampel 3-sigma MAD filter to eliminate OS interrupt spikes.
    pub enable_hampel: bool,
    /// Multiplier `k` for Hampel outlier threshold (default 3.0).
    pub hampel_k: f64,
    /// Whether to synchronize with the monotonic clock rising edge before each round.
    pub rising_edge_sync: bool,
}

impl Default for TimedFnConfig {
    fn default() -> Self {
        Self {
            target_duration: DEFAULT_TARGET_DURATION,
            num_rounds: 5,
            probe_runs: 1,
            min_loops: 1,
            max_loops: 100_000_000,
            enable_hampel: true,
            hampel_k: 3.0,
            rising_edge_sync: true,
        }
    }
}

/// Comprehensive statistical result of an adaptive timed function benchmark pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimedFnResult {
    /// Target duration requested for integration in nanoseconds.
    pub target_duration_ns: u64,
    /// Estimated loops executed in each benchmark round.
    pub estimated_loops_per_round: usize,
    /// Total number of rounds executed.
    pub num_rounds: usize,
    /// Best (minimum) round duration in nanoseconds (Best-of-N lowest interrupt floor).
    pub best_round_duration_ns: u64,
    /// Best per-iteration duration in nanoseconds (`best_round_duration_ns / loops`).
    pub best_ns_per_iteration: f64,
    /// Arithmetic mean per-iteration duration across Hampel-cleaned rounds.
    pub mean_ns_per_iteration: f64,
    /// Median per-iteration duration across rounds.
    pub median_ns_per_iteration: f64,
    /// Sample standard deviation across cleaned rounds.
    pub std_dev_ns: f64,
    /// Median Absolute Deviation (MAD) across rounds.
    pub mad_ns: f64,
    /// Relative Standard Error percentage (RSE %).
    pub rse_pct: f64,
    /// Raw round durations in nanoseconds.
    pub round_durations_ns: Vec<u64>,
    /// Cleaned per-iteration durations in nanoseconds.
    pub clean_per_iteration_ns: Vec<f64>,
    /// Number of round outliers identified and removed by Hampel MAD filter.
    pub outliers_count: usize,
    /// Total workload iterations executed across all measurement rounds.
    pub total_iterations: usize,
    /// Total active measured duration across all rounds in nanoseconds.
    pub total_active_nanos: u64,
}

impl TimedFnResult {
    /// Computes throughput in MB/s based on the Best-of-N iteration time.
    #[inline]
    pub fn throughput_mbs_from_best(&self, payload_bytes: usize) -> f64 {
        if self.best_ns_per_iteration <= 0.0 || payload_bytes == 0 {
            return 0.0;
        }
        let elapsed_secs = self.best_ns_per_iteration / 1_000_000_000.0;
        (payload_bytes as f64 / (1024.0 * 1024.0)) / elapsed_secs
    }

    /// Computes throughput in MB/s based on the Hampel-cleaned mean iteration time.
    #[inline]
    pub fn throughput_mbs_from_mean(&self, payload_bytes: usize) -> f64 {
        if self.mean_ns_per_iteration <= 0.0 || payload_bytes == 0 {
            return 0.0;
        }
        let elapsed_secs = self.mean_ns_per_iteration / 1_000_000_000.0;
        (payload_bytes as f64 / (1024.0 * 1024.0)) / elapsed_secs
    }

    /// Calculates CPU Cycles Per Byte (CPB) at a nominal clock frequency in GHz.
    #[inline]
    pub fn calc_cpb_from_best(&self, payload_bytes: usize, freq_ghz: f64) -> f64 {
        if payload_bytes == 0 || self.best_ns_per_iteration <= 0.0 || freq_ghz <= 0.0 {
            return 0.0;
        }
        let cycles = (self.best_ns_per_iteration / 1_000_000_000.0) * freq_ghz * 1_000_000_000.0;
        cycles / (payload_bytes as f64)
    }
}

/// Adaptive workload estimation and high-precision timed function benchmarking engine.
#[derive(Debug, Clone)]
pub struct TimedFnBenchmarkEngine {
    pub config: TimedFnConfig,
}

impl Default for TimedFnBenchmarkEngine {
    fn default() -> Self {
        Self::new(TimedFnConfig::default())
    }
}

impl TimedFnBenchmarkEngine {
    /// Creates a benchmark engine with specified configuration.
    pub fn new(config: TimedFnConfig) -> Self {
        Self { config }
    }

    /// Creates a benchmark engine configured with a custom target duration.
    pub fn with_target_duration(target_duration: Duration) -> Self {
        Self::new(TimedFnConfig {
            target_duration,
            ..TimedFnConfig::default()
        })
    }

    /// Estimates the number of loop iterations required to saturate the target duration.
    ///
    /// Executes initial probe iterations to measure single-call latency, then computes:
    /// `num_loops = (target_ns / iter_ns).clamp(min_loops, max_loops)`.
    pub fn estimate_loops<F: FnMut()>(&self, f: &mut F) -> usize {
        let target_ns = self.config.target_duration.as_nanos() as u64;
        let mut probe_count: usize = 1;
        let mut elapsed_ns: u64 = 0;

        // Geometric probing until a measurable duration (>= 5,000 ns / 5us) is observed
        while probe_count <= self.config.max_loops {
            let start = if self.config.rising_edge_sync {
                wait_for_next_tick()
            } else {
                get_hardware_monotonic_nanos()
            };
            for _ in 0..probe_count {
                f();
            }
            let end = get_hardware_monotonic_nanos();
            elapsed_ns = end.saturating_sub(start);
            if elapsed_ns >= 5_000 {
                break;
            }
            probe_count = (probe_count * 2).min(self.config.max_loops + 1);
            if probe_count > self.config.max_loops {
                break;
            }
        }

        let iter_ns = if elapsed_ns > 0 {
            (elapsed_ns as f64) / (probe_count as f64)
        } else {
            1.0
        };

        let estimated = ((target_ns as f64) / iter_ns).round() as usize;
        estimated.clamp(self.config.min_loops, self.config.max_loops).max(1)
    }

    /// Benchmarks a closure by adaptively scaling loop counts to saturate target duration,
    /// executing multiple rounds with rising-edge alignment and Hampel MAD / Best-of-N filtering.
    pub fn bench<F: FnMut()>(&self, mut f: F) -> TimedFnResult {
        // Step 1: Probe and estimate loops needed for target duration
        let loops = self.estimate_loops(&mut f);
        let num_rounds = self.config.num_rounds.max(1);

        let mut round_durations_ns = Vec::with_capacity(num_rounds);

        // Step 2: Execute multi-round sampling
        for _ in 0..num_rounds {
            let start = if self.config.rising_edge_sync {
                wait_for_next_tick()
            } else {
                get_hardware_monotonic_nanos()
            };

            for _ in 0..loops {
                f();
            }

            let end = get_hardware_monotonic_nanos();
            let elapsed = end.saturating_sub(start).max(1);
            round_durations_ns.push(elapsed);
        }

        // Step 3: Compute Best-of-N round duration (lowest interrupt floor)
        let best_round = (*round_durations_ns.iter().min().unwrap_or(&1)).max(1);
        let best_ns_per_iteration = best_round as f64 / loops as f64;

        // Step 4: Convert round times to per-iteration durations
        let raw_per_iter_ns: Vec<f64> = round_durations_ns
            .iter()
            .map(|&d| d.max(1) as f64 / loops as f64)
            .collect();

        // Step 5: Apply Hampel MAD filtering to reject OS context-switch / interrupt spikes
        let (clean_per_iteration_ns, outliers_count, median_ns, mad_ns) =
            if self.config.enable_hampel && raw_per_iter_ns.len() >= 3 {
                let res = HampelFilter::new(self.config.hampel_k).filter(&raw_per_iter_ns);
                (res.cleaned, res.outliers.len(), res.median, res.mad)
            } else {
                let median = HampelFilter::calc_median(&raw_per_iter_ns);
                (raw_per_iter_ns.clone(), 0, median, 0.0)
            };

        // Step 6: Compute statistical mean, standard deviation, and RSE%
        let (mean_ns, var_ns) = WelchStudentTTest::sample_mean_and_variance(&clean_per_iteration_ns);
        let std_dev_ns = var_ns.sqrt();
        let se_ns = if clean_per_iteration_ns.is_empty() {
            0.0
        } else {
            std_dev_ns / (clean_per_iteration_ns.len() as f64).sqrt()
        };
        let rse_pct = if mean_ns > 1e-15 {
            (se_ns / mean_ns) * 100.0
        } else {
            0.0
        };

        let total_active_nanos = round_durations_ns.iter().sum();
        let total_iterations = loops * num_rounds;

        TimedFnResult {
            target_duration_ns: self.config.target_duration.as_nanos() as u64,
            estimated_loops_per_round: loops,
            num_rounds,
            best_round_duration_ns: best_round,
            best_ns_per_iteration,
            mean_ns_per_iteration: mean_ns,
            median_ns_per_iteration: median_ns,
            std_dev_ns,
            mad_ns,
            rse_pct,
            round_durations_ns,
            clean_per_iteration_ns,
            outliers_count,
            total_iterations,
            total_active_nanos,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timed_fn_config_defaults() {
        let cfg = TimedFnConfig::default();
        assert_eq!(cfg.target_duration, Duration::from_millis(1000));
        assert_eq!(cfg.num_rounds, 5);
        assert_eq!(cfg.probe_runs, 1);
        assert!(cfg.enable_hampel);
        assert!(cfg.rising_edge_sync);
    }

    #[test]
    fn test_timed_fn_adaptive_loop_estimation() {
        let engine = TimedFnBenchmarkEngine::with_target_duration(Duration::from_millis(5));
        let mut counter = 0u64;

        let loops = engine.estimate_loops(&mut || {
            for _ in 0..1_000 {
                counter = counter.wrapping_add(1);
            }
        });

        assert!(loops >= 1);
        assert!(counter > 0);
    }

    #[test]
    fn test_timed_fn_bench_execution_and_filtering() {
        let engine = TimedFnBenchmarkEngine::new(TimedFnConfig {
            target_duration: Duration::from_millis(5),
            num_rounds: 5,
            probe_runs: 1,
            min_loops: 1,
            max_loops: 100_000,
            enable_hampel: true,
            hampel_k: 3.0,
            rising_edge_sync: true,
        });

        let mut acc = 0u64;
        let result = engine.bench(|| {
            for j in 0..100 {
                acc = acc.wrapping_add(std::hint::black_box(j));
            }
            std::hint::black_box(acc);
        });

        assert_eq!(result.num_rounds, 5);
        assert!(result.estimated_loops_per_round >= 1);
        assert!(result.best_ns_per_iteration > 0.0);
        assert!(result.mean_ns_per_iteration > 0.0);
        assert!(result.best_ns_per_iteration <= result.mean_ns_per_iteration * 1.5);
        assert_eq!(result.clean_per_iteration_ns.len() + result.outliers_count, 5);

        // Throughput calculations
        let tp_best = result.throughput_mbs_from_best(1024 * 1024);
        assert!(tp_best > 0.0);
        let tp_mean = result.throughput_mbs_from_mean(1024 * 1024);
        assert!(tp_mean > 0.0);
        let cpb = result.calc_cpb_from_best(1024, 3.5);
        assert!(cpb > 0.0);
    }
}
