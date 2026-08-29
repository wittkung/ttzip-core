// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! TTZip Declarative A/B Benchmarking Engine - Layer 4: Scheduler & Orchestrator.
//!
//! Orchestrates:
//! - Target resolution via `TargetRegistry` (wildcard matching, glob filters)
//! - Corpus loading via `CorpusRegistry`
//! - Symmetric interleaved cross-directional sampling (A/B/B/A pattern)
//! - Measurement kernel integration (`MeasurementEngine`, `WelchStudentTTest`, `HampelFilter`)
//! - Quality gate verification and offline baseline snapshot comparison

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::benchmark::ab_engine::corpus_provider::CorpusRegistry;
use crate::benchmark::ab_engine::stats::{
    sync_to_next_tick, ComparisonStats, DecisionVerdict, MeasurementConfig, MeasurementEngine,
    MeasurementStats,
};
use crate::benchmark::ab_engine::target::{
    target_recommended_payload_size, BenchmarkTarget, TargetDescriptor, TargetRegistry,
};
use crate::types::TTZipStatus;

// ============================================================================
// Orchestrator Configuration
// ============================================================================

/// Configuration parameters for the A/B benchmark orchestrator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbOrchestratorConfig {
    /// Number of untimed warmup iterations per target before sampling.
    pub warmup_rounds: usize,
    /// Number of timed measurement iterations per target.
    pub measurement_rounds: usize,
    /// Maximum allowed regression percentage before failing the quality gate (e.g. 3.0 for 3.0%).
    pub max_allowed_regression: f64,
    /// p-value significance threshold for Welch's t-test (e.g. 0.05 for 95% confidence).
    pub p_value_threshold: f64,
    /// Whether to enable Hampel 3-sigma MAD robust outlier filtering.
    pub hampel_filter: bool,
    /// Multiplier `k` for Hampel outlier threshold (default 3.0).
    pub hampel_k: f64,
    /// Target Relative Standard Error percentage for early convergence (e.g. 0.5%).
    pub target_rse_pct: f64,
}

impl Default for AbOrchestratorConfig {
    fn default() -> Self {
        Self {
            warmup_rounds: 3,
            measurement_rounds: 20,
            max_allowed_regression: 3.0,
            p_value_threshold: 0.05,
            hampel_filter: true,
            hampel_k: 3.0,
            target_rse_pct: 0.5,
        }
    }
}

// ============================================================================
// Report & Snapshot Models
// ============================================================================

/// Evaluation result for an individual target in an A/B benchmark suite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetAbReportItem {
    /// Target metadata descriptor.
    pub descriptor: TargetDescriptor,
    /// Corpus identifier or URI used for testing.
    pub corpus_uri: String,
    /// Input corpus payload size in bytes.
    pub corpus_size_bytes: usize,
    /// Baseline (Run A) measurement statistics.
    pub stats_a: MeasurementStats,
    /// Candidate (Run B) measurement statistics.
    pub stats_b: MeasurementStats,
    /// Detailed statistical comparison between baseline A and candidate B.
    pub comparison: ComparisonStats,
    /// Baseline throughput in MB/s.
    pub throughput_a_mbs: f64,
    /// Candidate throughput in MB/s.
    pub throughput_b_mbs: f64,
    /// Speedup ratio (throughput_b / throughput_a or time_a / time_b).
    pub speedup_ratio: f64,
    /// Statistical decision verdict (Speedup, Regression, Neutral).
    pub verdict: DecisionVerdict,
    /// Whether this target passed the quality gate (no unacceptable regression).
    pub passed_gate: bool,
}

/// Comprehensive A/B benchmark execution report across a suite of targets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbBenchmarkReport {
    /// Timestamp when benchmark was executed (seconds since Unix epoch).
    pub timestamp_epoch_secs: u64,
    /// Target filter pattern applied (e.g. "codec/zstd/*", "*").
    pub target_filter: String,
    /// Corpus identifier or URI used.
    pub corpus_uri: String,
    /// Input corpus size in bytes.
    pub corpus_size_bytes: usize,
    /// Orchestrator configuration used.
    pub config: AbOrchestratorConfig,
    /// Evaluated target report items.
    pub items: Vec<TargetAbReportItem>,
    /// Total number of evaluated targets.
    pub total_targets: usize,
    /// Number of targets that passed the regression gate.
    pub passed_targets: usize,
    /// Number of targets showing statistically significant speedup.
    pub speedup_count: usize,
    /// Number of targets showing statistically significant regression.
    pub regression_count: usize,
    /// Number of neutral / noise targets.
    pub neutral_count: usize,
    /// Whether all targets passed the regression gate.
    pub overall_passed: bool,
}

impl AbBenchmarkReport {
    /// Serializes report to JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserializes report from JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Extracts an offline baseline snapshot from this report.
    pub fn to_baseline_snapshot(&self, use_candidate: bool) -> AbBaselineSnapshot {
        let mut snapshot = AbBaselineSnapshot::new(self.timestamp_epoch_secs);
        for item in &self.items {
            let (stats, throughput) = if use_candidate {
                (item.stats_b.clone(), item.throughput_b_mbs)
            } else {
                (item.stats_a.clone(), item.throughput_a_mbs)
            };
            snapshot.insert(BaselineSnapshotEntry {
                descriptor: item.descriptor.clone(),
                corpus_uri: item.corpus_uri.clone(),
                corpus_size_bytes: item.corpus_size_bytes,
                stats,
                throughput_mbs: throughput,
            });
        }
        snapshot
    }
}

/// Single target snapshot entry within an offline baseline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BaselineSnapshotEntry {
    /// Target descriptor.
    pub descriptor: TargetDescriptor,
    /// Corpus identifier.
    pub corpus_uri: String,
    /// Input size in bytes.
    pub corpus_size_bytes: usize,
    /// Measurement statistics.
    pub stats: MeasurementStats,
    /// Throughput in MB/s.
    pub throughput_mbs: f64,
}

/// Offline baseline snapshot for regression tracking across commits/releases.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbBaselineSnapshot {
    /// Timestamp when baseline was recorded.
    pub timestamp_epoch_secs: u64,
    /// Optional Git commit hash or tag.
    pub git_commit: Option<String>,
    /// Map of target URI to snapshot entry.
    pub entries: HashMap<String, BaselineSnapshotEntry>,
}

impl AbBaselineSnapshot {
    /// Creates a new empty baseline snapshot.
    pub fn new(timestamp_epoch_secs: u64) -> Self {
        Self {
            timestamp_epoch_secs,
            git_commit: None,
            entries: HashMap::new(),
        }
    }

    /// Inserts a target entry into the snapshot.
    pub fn insert(&mut self, entry: BaselineSnapshotEntry) {
        self.entries.insert(entry.descriptor.uri.clone(), entry);
    }

    /// Retrieves a target entry by URI.
    pub fn get(&self, uri: &str) -> Option<&BaselineSnapshotEntry> {
        self.entries.get(uri)
    }

    /// Returns the number of entries in the snapshot.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the snapshot is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Serializes snapshot to JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserializes snapshot from JSON string, supporting direct snapshot, full report, or telemetry envelope formats.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        if let Ok(snapshot) = serde_json::from_str::<Self>(json) {
            return Ok(snapshot);
        }
        if let Ok(report) = serde_json::from_str::<AbBenchmarkReport>(json) {
            return Ok(report.to_baseline_snapshot(false));
        }
        #[derive(Deserialize)]
        struct EnvelopeHelper {
            report: AbBenchmarkReport,
        }
        if let Ok(env) = serde_json::from_str::<EnvelopeHelper>(json) {
            return Ok(env.report.to_baseline_snapshot(false));
        }
        serde_json::from_str::<Self>(json)
    }
}

// ============================================================================
// A/B Benchmark Orchestrator Engine
// ============================================================================

/// Master scheduler and orchestrator for declarative A/B benchmark suites.
#[derive(Debug, Clone, Default)]
pub struct AbEngineOrchestrator;

impl AbEngineOrchestrator {
    /// Creates a new orchestrator instance.
    pub fn new() -> Self {
        Self
    }

    /// Executes an end-to-end A/B benchmark suite matching target filter and corpus URI.
    /// Executes an end-to-end A/B benchmark suite matching target filter and corpus URI.
    pub fn run_ab_benchmark(
        &self,
        target_filter: &str,
        corpus_uri: &str,
        size_bytes: usize,
        config: &AbOrchestratorConfig,
    ) -> Result<AbBenchmarkReport, TTZipStatus> {
        let targets = if target_filter == "*" || target_filter == "core" || target_filter == "default" {
            TargetRegistry::default_core().filter_targets("*")
        } else if target_filter == "all" || target_filter == "**" {
            TargetRegistry::default_full().filter_targets("*")
        } else {
            TargetRegistry::default_full().filter_targets(target_filter)
        };

        let corpus_bytes = CorpusRegistry::global()
            .generate(corpus_uri, size_bytes)?;

        self.run_ab_benchmark_with_targets(&targets, &corpus_bytes, corpus_uri, target_filter, config)
    }

    /// Executes benchmark suite on pre-resolved targets and input buffer.
    pub fn run_ab_benchmark_with_targets(
        &self,
        targets: &[Arc<dyn BenchmarkTarget>],
        corpus_bytes: &[u8],
        corpus_uri: &str,
        target_filter: &str,
        config: &AbOrchestratorConfig,
    ) -> Result<AbBenchmarkReport, TTZipStatus> {
        let mut items = Vec::with_capacity(targets.len());

        for target in targets {
            let item = self.run_paired_target(
                target.as_ref(),
                target.as_ref(),
                corpus_bytes,
                corpus_uri,
                config,
            )?;
            items.push(item);
        }

        Ok(self.build_report(items, target_filter, corpus_uri, corpus_bytes.len(), config))
    }

    /// Executes benchmark against an offline baseline snapshot.
    pub fn run_ab_benchmark_against_baseline(
        &self,
        target_filter: &str,
        corpus_uri: &str,
        size_bytes: usize,
        baseline: &AbBaselineSnapshot,
        config: &AbOrchestratorConfig,
    ) -> Result<AbBenchmarkReport, TTZipStatus> {
        let targets = if target_filter == "*" || target_filter == "core" || target_filter == "default" {
            TargetRegistry::default_core().filter_targets("*")
        } else if target_filter == "all" || target_filter == "**" {
            TargetRegistry::default_full().filter_targets("*")
        } else {
            TargetRegistry::default_full().filter_targets(target_filter)
        };

        let corpus_bytes = CorpusRegistry::global()
            .generate(corpus_uri, size_bytes)?;

        let mut items = Vec::with_capacity(targets.len());

        for target in &targets {
            let uri = &target.descriptor().uri;
            if let Some(base_entry) = baseline.get(uri) {
                let cand_stats = self.run_single_target(target.as_ref(), &corpus_bytes, config)?;
                let effective_size = target_recommended_payload_size(uri, corpus_bytes.len());
                let item = self.evaluate_candidate_vs_baseline(
                    target.descriptor(),
                    corpus_uri,
                    effective_size,
                    base_entry.stats.clone(),
                    cand_stats,
                    config,
                );
                items.push(item);
            } else {
                let item = self.run_paired_target(
                    target.as_ref(),
                    target.as_ref(),
                    &corpus_bytes,
                    corpus_uri,
                    config,
                )?;
                items.push(item);
            }
        }

        Ok(self.build_report(items, target_filter, corpus_uri, corpus_bytes.len(), config))
    }

    /// Runs paired symmetric interleaved cross-directional sampling (A/B/B/A pattern).
    pub fn run_paired_target(
        &self,
        target_a: &dyn BenchmarkTarget,
        target_b: &dyn BenchmarkTarget,
        corpus_bytes: &[u8],
        corpus_uri: &str,
        config: &AbOrchestratorConfig,
    ) -> Result<TargetAbReportItem, TTZipStatus> {
        let effective_size = target_recommended_payload_size(&target_a.descriptor().uri, corpus_bytes.len());
        let effective_bytes = if effective_size < corpus_bytes.len() {
            &corpus_bytes[..effective_size]
        } else {
            corpus_bytes
        };

        // 1. Warmup passes with duration probe
        let warmup_start = std::time::Instant::now();
        for _ in 0..config.warmup_rounds.max(1) {
            target_a.execute_pass(effective_bytes)?;
            target_b.execute_pass(effective_bytes)?;
        }
        let warmup_elapsed = warmup_start.elapsed();

        // 2. Adaptive measurement rounds: if single pass takes > 40ms, scale down rounds to cap total target duration <= 250ms
        let rounds = if warmup_elapsed > std::time::Duration::from_millis(200) {
            2
        } else if warmup_elapsed > std::time::Duration::from_millis(40) {
            4
        } else {
            config.measurement_rounds.max(4)
        };

        let mut samples_a = Vec::with_capacity(rounds);
        let mut samples_b = Vec::with_capacity(rounds);

        for round in 0..rounds {
            if round % 2 == 0 {
                // A then B
                let start_a = sync_to_next_tick();
                target_a.execute_pass(effective_bytes)?;
                samples_a.push(start_a.elapsed().as_nanos() as f64);

                let start_b = sync_to_next_tick();
                target_b.execute_pass(effective_bytes)?;
                samples_b.push(start_b.elapsed().as_nanos() as f64);
            } else {
                // B then A
                let start_b = sync_to_next_tick();
                target_b.execute_pass(effective_bytes)?;
                samples_b.push(start_b.elapsed().as_nanos() as f64);

                let start_a = sync_to_next_tick();
                target_a.execute_pass(effective_bytes)?;
                samples_a.push(start_a.elapsed().as_nanos() as f64);
            }
        }

        // 3. Compute statistics via MeasurementEngine
        let engine = MeasurementEngine::new(MeasurementConfig {
            warmup_iterations: 0,
            min_iterations: rounds.min(10),
            max_iterations: rounds,
            target_rse_pct: config.target_rse_pct,
            enable_hampel: config.hampel_filter,
            hampel_k: config.hampel_k,
        });

        let stats_a = engine.compute_stats(&samples_a, true);
        let stats_b = engine.compute_stats(&samples_b, true);

        Ok(self.evaluate_candidate_vs_baseline(
            target_b.descriptor(),
            corpus_uri,
            effective_bytes.len(),
            stats_a,
            stats_b,
            config,
        ))
    }

    /// Measures a single target with warmup and execution passes.
    pub fn run_single_target(
        &self,
        target: &dyn BenchmarkTarget,
        corpus_bytes: &[u8],
        config: &AbOrchestratorConfig,
    ) -> Result<MeasurementStats, TTZipStatus> {
        let effective_size = target_recommended_payload_size(&target.descriptor().uri, corpus_bytes.len());
        let effective_bytes = if effective_size < corpus_bytes.len() {
            &corpus_bytes[..effective_size]
        } else {
            corpus_bytes
        };

        let warmup_start = std::time::Instant::now();
        for _ in 0..config.warmup_rounds.max(1) {
            target.execute_pass(effective_bytes)?;
        }
        let warmup_elapsed = warmup_start.elapsed();

        let rounds = if warmup_elapsed > std::time::Duration::from_millis(200) {
            2
        } else if warmup_elapsed > std::time::Duration::from_millis(40) {
            4
        } else {
            config.measurement_rounds.max(4)
        };

        let mut samples = Vec::with_capacity(rounds);

        for _ in 0..rounds {
            let start = sync_to_next_tick();
            target.execute_pass(effective_bytes)?;
            samples.push(start.elapsed().as_nanos() as f64);
        }

        let engine = MeasurementEngine::new(MeasurementConfig {
            warmup_iterations: 0,
            min_iterations: rounds.min(10),
            max_iterations: rounds,
            target_rse_pct: config.target_rse_pct,
            enable_hampel: config.hampel_filter,
            hampel_k: config.hampel_k,
        });

        Ok(engine.compute_stats(&samples, true))
    }

    /// Evaluates candidate stats against baseline stats.
    fn evaluate_candidate_vs_baseline(
        &self,
        descriptor: &TargetDescriptor,
        corpus_uri: &str,
        corpus_size_bytes: usize,
        stats_a: MeasurementStats,
        stats_b: MeasurementStats,
        config: &AbOrchestratorConfig,
    ) -> TargetAbReportItem {
        let comparison = MeasurementEngine::compare(
            stats_a.clone(),
            stats_b.clone(),
            config.p_value_threshold,
        );

        let throughput_a_mbs = calc_throughput_mbs(corpus_size_bytes, stats_a.mean_nanos);
        let throughput_b_mbs = calc_throughput_mbs(corpus_size_bytes, stats_b.mean_nanos);
        let speedup_ratio = comparison.speedup_ratio;
        let verdict = comparison.verdict;

        let is_unacceptable_regression = verdict == DecisionVerdict::SignificantRegression
            && comparison.delta_pct > config.max_allowed_regression;
        let passed_gate = !is_unacceptable_regression;

        TargetAbReportItem {
            descriptor: descriptor.clone(),
            corpus_uri: corpus_uri.to_string(),
            corpus_size_bytes,
            stats_a,
            stats_b,
            comparison,
            throughput_a_mbs,
            throughput_b_mbs,
            speedup_ratio,
            verdict,
            passed_gate,
        }
    }

    /// Aggregates items into a final `AbBenchmarkReport`.
    fn build_report(
        &self,
        items: Vec<TargetAbReportItem>,
        target_filter: &str,
        corpus_uri: &str,
        corpus_size_bytes: usize,
        config: &AbOrchestratorConfig,
    ) -> AbBenchmarkReport {
        let now_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let total_targets = items.len();
        let passed_targets = items.iter().filter(|i| i.passed_gate).count();
        let speedup_count = items
            .iter()
            .filter(|i| i.verdict == DecisionVerdict::SignificantSpeedup)
            .count();
        let regression_count = items
            .iter()
            .filter(|i| i.verdict == DecisionVerdict::SignificantRegression)
            .count();
        let neutral_count = items
            .iter()
            .filter(|i| i.verdict == DecisionVerdict::NeutralNoise)
            .count();
        let overall_passed = total_targets == passed_targets;

        AbBenchmarkReport {
            timestamp_epoch_secs: now_epoch,
            target_filter: target_filter.to_string(),
            corpus_uri: corpus_uri.to_string(),
            corpus_size_bytes,
            config: config.clone(),
            items,
            total_targets,
            passed_targets,
            speedup_count,
            regression_count,
            neutral_count,
            overall_passed,
        }
    }
}

/// Helper function to compute throughput in Megabytes per second (MB/s).
#[inline]
pub fn calc_throughput_mbs(bytes: usize, duration_nanos: f64) -> f64 {
    if duration_nanos <= 1e-9 || bytes == 0 {
        return 0.0;
    }
    let mb = bytes as f64 / (1024.0 * 1024.0);
    let secs = duration_nanos / 1_000_000_000.0;
    mb / secs
}
