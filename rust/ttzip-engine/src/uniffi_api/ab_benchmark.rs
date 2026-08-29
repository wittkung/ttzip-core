// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! UniFFI exported bindings for declarative A/B benchmark orchestration and multimodal exporters.

use crate::benchmark::ab_engine::{
    AbBaselineSnapshot, AbBenchmarkReport, AbEngineOrchestrator, AbOrchestratorConfig,
    AsciiTableReporter, DecisionVerdict, JsonTelemetryReporter, MarkdownCommentReporter,
    TargetAbReportItem,
};
use crate::uniffi_api::types::TTZipError;

// MARK: - A/B Declarative Engine UniFFI Models

/// Strongly typed configuration record for UniFFI A/B benchmark orchestrator.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFIAbOrchestratorConfig {
    pub warmup_rounds: u32,
    pub measurement_rounds: u32,
    pub max_allowed_regression: f64,
    pub p_value_threshold: f64,
    pub hampel_filter: bool,
    pub hampel_k: f64,
    pub target_rse_pct: f64,
}

impl From<UniFFIAbOrchestratorConfig> for AbOrchestratorConfig {
    fn from(c: UniFFIAbOrchestratorConfig) -> Self {
        Self {
            warmup_rounds: c.warmup_rounds as usize,
            measurement_rounds: c.measurement_rounds as usize,
            max_allowed_regression: c.max_allowed_regression,
            p_value_threshold: c.p_value_threshold,
            hampel_filter: c.hampel_filter,
            hampel_k: c.hampel_k,
            target_rse_pct: c.target_rse_pct,
        }
    }
}

impl From<AbOrchestratorConfig> for UniFFIAbOrchestratorConfig {
    fn from(c: AbOrchestratorConfig) -> Self {
        Self {
            warmup_rounds: c.warmup_rounds as u32,
            measurement_rounds: c.measurement_rounds as u32,
            max_allowed_regression: c.max_allowed_regression,
            p_value_threshold: c.p_value_threshold,
            hampel_filter: c.hampel_filter,
            hampel_k: c.hampel_k,
            target_rse_pct: c.target_rse_pct,
        }
    }
}

/// Statistical decision verdict for UniFFI boundary.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum UniFFIDecisionVerdict {
    SignificantSpeedup,
    SignificantRegression,
    NeutralNoise,
}

impl From<DecisionVerdict> for UniFFIDecisionVerdict {
    fn from(v: DecisionVerdict) -> Self {
        match v {
            DecisionVerdict::SignificantSpeedup => UniFFIDecisionVerdict::SignificantSpeedup,
            DecisionVerdict::SignificantRegression => UniFFIDecisionVerdict::SignificantRegression,
            DecisionVerdict::NeutralNoise => UniFFIDecisionVerdict::NeutralNoise,
        }
    }
}

/// Evaluation result for an individual target in an A/B benchmark suite.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFITargetAbReportItem {
    pub target_uri: String,
    pub target_name: String,
    pub category: String,
    pub corpus_uri: String,
    pub corpus_size_bytes: u64,
    pub baseline_mean_nanos: f64,
    pub candidate_mean_nanos: f64,
    pub baseline_throughput_mbs: f64,
    pub candidate_throughput_mbs: f64,
    pub speedup_ratio: f64,
    pub delta_pct: f64,
    pub delta_pct_lower: f64,
    pub delta_pct_upper: f64,
    pub p_value: f64,
    pub verdict: UniFFIDecisionVerdict,
    pub passed_gate: bool,
}

impl From<TargetAbReportItem> for UniFFITargetAbReportItem {
    fn from(i: TargetAbReportItem) -> Self {
        Self {
            target_uri: i.descriptor.uri,
            target_name: i.descriptor.name,
            category: i.descriptor.category.as_str().to_string(),
            corpus_uri: i.corpus_uri,
            corpus_size_bytes: i.corpus_size_bytes as u64,
            baseline_mean_nanos: i.stats_a.mean_nanos,
            candidate_mean_nanos: i.stats_b.mean_nanos,
            baseline_throughput_mbs: i.throughput_a_mbs,
            candidate_throughput_mbs: i.throughput_b_mbs,
            speedup_ratio: i.speedup_ratio,
            delta_pct: i.comparison.delta_pct,
            delta_pct_lower: i.comparison.delta_pct_ci.lower,
            delta_pct_upper: i.comparison.delta_pct_ci.upper,
            p_value: i.comparison.t_test.p_value,
            verdict: i.verdict.into(),
            passed_gate: i.passed_gate,
        }
    }
}

/// Comprehensive A/B benchmark execution report for UniFFI boundary.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFIAbBenchmarkReport {
    pub timestamp_epoch_secs: u64,
    pub target_filter: String,
    pub corpus_uri: String,
    pub corpus_size_bytes: u64,
    pub total_targets: u32,
    pub passed_targets: u32,
    pub speedup_count: u32,
    pub regression_count: u32,
    pub neutral_count: u32,
    pub overall_passed: bool,
    pub items: Vec<UniFFITargetAbReportItem>,
}

impl From<AbBenchmarkReport> for UniFFIAbBenchmarkReport {
    fn from(r: AbBenchmarkReport) -> Self {
        Self {
            timestamp_epoch_secs: r.timestamp_epoch_secs,
            target_filter: r.target_filter,
            corpus_uri: r.corpus_uri,
            corpus_size_bytes: r.corpus_size_bytes as u64,
            total_targets: r.total_targets as u32,
            passed_targets: r.passed_targets as u32,
            speedup_count: r.speedup_count as u32,
            regression_count: r.regression_count as u32,
            neutral_count: r.neutral_count as u32,
            overall_passed: r.overall_passed,
            items: r.items.into_iter().map(Into::into).collect(),
        }
    }
}

/// Executes an end-to-end A/B benchmark suite matching target filter and corpus URI.
#[uniffi::export]
pub fn ttzip_bench_run_ab_benchmark(
    target_filter: String,
    corpus_uri: String,
    size_bytes: u64,
    config: Option<UniFFIAbOrchestratorConfig>,
) -> Result<UniFFIAbBenchmarkReport, TTZipError> {
    let orchestrator = AbEngineOrchestrator::new();
    let cfg = config.map(Into::into).unwrap_or_default();
    let report = orchestrator
        .run_ab_benchmark(&target_filter, &corpus_uri, size_bytes as usize, &cfg)
        .map_err(|st| TTZipError::EngineError { code: st as i32 })?;
    Ok(report.into())
}

/// Compares candidates against an offline baseline JSON snapshot.
#[uniffi::export]
pub fn ttzip_bench_compare_against_baseline(
    target_filter: String,
    corpus_uri: String,
    size_bytes: u64,
    baseline_json: String,
    config: Option<UniFFIAbOrchestratorConfig>,
) -> Result<UniFFIAbBenchmarkReport, TTZipError> {
    let baseline = AbBaselineSnapshot::from_json(&baseline_json)
        .map_err(|e| TTZipError::CorruptHeader {
            offset: 0,
            details: format!("Failed to parse JSON: {e}"),
        })?;
    let orchestrator = AbEngineOrchestrator::new();
    let cfg = config.map(Into::into).unwrap_or_default();
    let report = orchestrator
        .run_ab_benchmark_against_baseline(
            &target_filter,
            &corpus_uri,
            size_bytes as usize,
            &baseline,
            &cfg,
        )
        .map_err(|st| TTZipError::EngineError { code: st as i32 })?;
    Ok(report.into())
}

/// Executes an end-to-end A/B benchmark suite and directly returns the serialized report JSON string.
#[uniffi::export]
pub fn ttzip_bench_run_ab_benchmark_json(
    target_filter: String,
    corpus_uri: String,
    size_bytes: u64,
    config: Option<UniFFIAbOrchestratorConfig>,
) -> Result<String, TTZipError> {
    let orchestrator = AbEngineOrchestrator::new();
    let cfg = config.map(Into::into).unwrap_or_default();
    let report = orchestrator
        .run_ab_benchmark(&target_filter, &corpus_uri, size_bytes as usize, &cfg)
        .map_err(|st| TTZipError::EngineError { code: st as i32 })?;
    report
        .to_json()
        .map_err(|_| TTZipError::EngineError { code: -1 })
}

/// Compares candidates against an offline baseline JSON snapshot and directly returns the report JSON string.
#[uniffi::export]
pub fn ttzip_bench_compare_against_baseline_json(
    target_filter: String,
    corpus_uri: String,
    size_bytes: u64,
    baseline_json: String,
    config: Option<UniFFIAbOrchestratorConfig>,
) -> Result<String, TTZipError> {
    let baseline = AbBaselineSnapshot::from_json(&baseline_json)
        .map_err(|e| TTZipError::CorruptHeader {
            offset: 0,
            details: format!("Failed to parse JSON: {e}"),
        })?;
    let orchestrator = AbEngineOrchestrator::new();
    let cfg = config.map(Into::into).unwrap_or_default();
    let report = orchestrator
        .run_ab_benchmark_against_baseline(
            &target_filter,
            &corpus_uri,
            size_bytes as usize,
            &baseline,
            &cfg,
        )
        .map_err(|st| TTZipError::EngineError { code: st as i32 })?;
    report
        .to_json()
        .map_err(|_| TTZipError::EngineError { code: -1 })
}

/// Renders benchmark report JSON as an ASCII terminal table.
#[uniffi::export]
pub fn ttzip_bench_render_ab_ascii(
    report_json: String,
    ansi_color: bool,
) -> Result<String, TTZipError> {
    let report = AbBenchmarkReport::from_json(&report_json)
        .map_err(|e| TTZipError::CorruptHeader {
            offset: 0,
            details: format!("Failed to parse report JSON: {e}"),
        })?;
    Ok(if ansi_color {
        AsciiTableReporter::render(&report)
    } else {
        AsciiTableReporter::render_plain(&report)
    })
}

/// Renders benchmark report JSON as a GitHub PR Markdown comment.
#[uniffi::export]
pub fn ttzip_bench_render_ab_markdown(report_json: String) -> Result<String, TTZipError> {
    let report = AbBenchmarkReport::from_json(&report_json)
        .map_err(|e| TTZipError::CorruptHeader {
            offset: 0,
            details: format!("Failed to parse report JSON: {e}"),
        })?;
    Ok(MarkdownCommentReporter::render(&report))
}

/// Renders benchmark report JSON into RFC 8259 formatted telemetry.
#[uniffi::export]
pub fn ttzip_bench_render_ab_json(report_json: String) -> Result<String, TTZipError> {
    let report = AbBenchmarkReport::from_json(&report_json)
        .map_err(|e| TTZipError::CorruptHeader {
            offset: 0,
            details: format!("Failed to parse report JSON: {e}"),
        })?;
    Ok(JsonTelemetryReporter::render(&report))
}

/// Creates a baseline snapshot JSON string from a benchmark report JSON string.
#[uniffi::export]
pub fn ttzip_bench_create_baseline_snapshot(
    report_json: String,
    use_candidate: bool,
) -> Result<String, TTZipError> {
    let report = AbBenchmarkReport::from_json(&report_json)
        .map_err(|e| TTZipError::CorruptHeader {
            offset: 0,
            details: format!("Failed to parse report JSON: {e}"),
        })?;
    let snapshot = report.to_baseline_snapshot(use_candidate);
    snapshot
        .to_json()
        .map_err(|_| TTZipError::EngineError { code: -1 })
}
