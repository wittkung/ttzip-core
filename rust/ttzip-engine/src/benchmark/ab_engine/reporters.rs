// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! TTZip Declarative A/B Benchmarking Engine - Layer 5: Multimodal Report Exporters.
//!
//! Exporters:
//! 1. `AsciiTableReporter`: Terminal console output with ANSI color highlights and box borders.
//! 2. `JsonTelemetryReporter`: RFC 8259 structured JSON telemetry payload with summary statistics.
//! 3. `MarkdownCommentReporter`: GitHub PR comment ready Markdown tables with expandable `<details>`.

use serde::{Deserialize, Serialize};

use crate::benchmark::ab_engine::orchestrator::AbBenchmarkReport;
use crate::benchmark::ab_engine::stats::DecisionVerdict;

// ============================================================================
// ASCII Table Console Reporter
// ============================================================================

/// Terminal console table renderer supporting ANSI 24-bit/8-color high-contrast styles.
pub struct AsciiTableReporter;

impl AsciiTableReporter {
    /// Renders an `AbBenchmarkReport` as a styled ASCII table with ANSI color highlights.
    pub fn render(report: &AbBenchmarkReport) -> String {
        Self::render_internal(report, true)
    }

    /// Renders an `AbBenchmarkReport` as a plain uncolored ASCII table (for logs/files).
    pub fn render_plain(report: &AbBenchmarkReport) -> String {
        Self::render_internal(report, false)
    }

    fn render_internal(report: &AbBenchmarkReport, ansi: bool) -> String {
        let mut out = String::with_capacity(4096);

        let green = if ansi { "\x1b[32m" } else { "" };
        let red = if ansi { "\x1b[31m" } else { "" };
        let yellow = if ansi { "\x1b[33m" } else { "" };
        let bold = if ansi { "\x1b[1m" } else { "" };
        let dim = if ansi { "\x1b[90m" } else { "" };
        let reset = if ansi { "\x1b[0m" } else { "" };

        // Banner Header
        out.push_str(&format!(
            "\n{}TTZip Declarative A/B Performance Suite{}\n",
            bold, reset
        ));
        out.push_str(&format!(
            "{}Corpus:{} {} ({:.2} MB) {}|{} {}Filter:{} {} {}|{} {}Samples:{} {} {}|{} {}α:{} {}\n\n",
            dim, reset, report.corpus_uri, report.corpus_size_bytes as f64 / (1024.0 * 1024.0),
            dim, reset,
            dim, reset, report.target_filter,
            dim, reset,
            dim, reset, report.config.measurement_rounds,
            dim, reset,
            dim, reset, report.config.p_value_threshold,
        ));

        // Table Header
        out.push_str("┌───────────────────────────────────┬──────────────┬──────────────┬──────────┬─────────────────┬──────────┬───────────┐\n");
        out.push_str("│ Target URI                        │ Baseline     │ Candidate    │ Delta %  │ Speedup [95%CI] │ p-value  │ Verdict   │\n");
        out.push_str("├───────────────────────────────────┼──────────────┼──────────────┼──────────┼─────────────────┼──────────┼───────────┤\n");

        if report.items.is_empty() {
            out.push_str("│ (No matching benchmark targets evaluated)                                                                   │\n");
        } else {
            for item in &report.items {
                let uri_display = if item.descriptor.uri.len() > 33 {
                    format!("{}...", &item.descriptor.uri[..30])
                } else {
                    item.descriptor.uri.clone()
                };

                let base_str = format_throughput_or_latency(item.throughput_a_mbs, item.stats_a.mean_nanos);
                let cand_str = format_throughput_or_latency(item.throughput_b_mbs, item.stats_b.mean_nanos);
                let delta_str = format_delta_percentage(item.comparison.delta_pct);
                let speedup_str = format!(
                    "{:.2}x [{:.2}]",
                    item.speedup_ratio,
                    item.comparison.speedup_ci.lower
                );
                let p_val_str = if item.comparison.t_test.p_value < 0.0001 {
                    "< 0.0001".to_string()
                } else {
                    format!("{:.4}", item.comparison.t_test.p_value)
                };

                let (verdict_str, color) = match item.verdict {
                    DecisionVerdict::SignificantSpeedup => ("SPEEDUP", green),
                    DecisionVerdict::SignificantRegression => ("REGRESS", red),
                    DecisionVerdict::NeutralNoise => ("NEUTRAL", yellow),
                };

                out.push_str(&format!(
                    "│ {:<33} │ {:>12} │ {:>12} │ {:>8} │ {:>15} │ {:>8} │ {}{:<9}{} │\n",
                    uri_display,
                    base_str,
                    cand_str,
                    delta_str,
                    speedup_str,
                    p_val_str,
                    color,
                    verdict_str,
                    reset,
                ));
            }
        }

        out.push_str("└───────────────────────────────────┴──────────────┴──────────────┴──────────┴─────────────────┴──────────┴───────────┘\n");

        // Summary Box
        let status_color = if report.overall_passed { green } else { red };
        let status_text = if report.overall_passed { "PASSED" } else { "FAILED (REGRESSION DETECTED)" };

        out.push_str(&format!(
            "Summary: {} Total: {} | Speedup: {} | Regress: {} | Neutral: {}\n",
            dim, report.total_targets, report.speedup_count, report.regression_count, report.neutral_count
        ));
        out.push_str(&format!(
            "Quality Gate: {}{}[{}]{}\n\n",
            bold, status_color, status_text, reset
        ));

        out
    }
}

// ============================================================================
// JSON Telemetry Reporter (RFC 8259)
// ============================================================================

/// Structured RFC 8259 JSON exporter for machine consumption and CI integration.
pub struct JsonTelemetryReporter;

/// JSON envelope for A/B benchmark telemetry.
#[derive(Debug, Clone, Serialize)]
pub struct JsonTelemetryEnvelope<'a> {
    pub schema_version: &'static str,
    pub timestamp_epoch_secs: u64,
    pub target_filter: &'a str,
    pub corpus_uri: &'a str,
    pub corpus_size_bytes: usize,
    pub summary: JsonTelemetrySummary,
    pub report: &'a AbBenchmarkReport,
}

/// Aggregated summary statistics for JSON telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonTelemetrySummary {
    pub total_targets: usize,
    pub passed_targets: usize,
    pub speedup_count: usize,
    pub regression_count: usize,
    pub neutral_count: usize,
    pub overall_passed: bool,
}

impl JsonTelemetryReporter {
    /// Renders an `AbBenchmarkReport` as a formatted JSON payload.
    pub fn render(report: &AbBenchmarkReport) -> String {
        let envelope = JsonTelemetryEnvelope {
            schema_version: "1.0.0",
            timestamp_epoch_secs: report.timestamp_epoch_secs,
            target_filter: &report.target_filter,
            corpus_uri: &report.corpus_uri,
            corpus_size_bytes: report.corpus_size_bytes,
            summary: JsonTelemetrySummary {
                total_targets: report.total_targets,
                passed_targets: report.passed_targets,
                speedup_count: report.speedup_count,
                regression_count: report.regression_count,
                neutral_count: report.neutral_count,
                overall_passed: report.overall_passed,
            },
            report,
        };

        serde_json::to_string_pretty(&envelope).unwrap_or_else(|_| "{}".to_string())
    }
}

// ============================================================================
// GitHub Markdown PR Comment Reporter
// ============================================================================

/// Markdown comment generator designed for GitHub Actions and Pull Request status reports.
pub struct MarkdownCommentReporter;

impl MarkdownCommentReporter {
    /// Renders an `AbBenchmarkReport` as a GitHub PR friendly Markdown string.
    pub fn render(report: &AbBenchmarkReport) -> String {
        let mut md = String::with_capacity(4096);

        let status_badge = if report.overall_passed {
            "🟢 **`PASSED`**"
        } else {
            "🔴 **`FAILED (REGRESSION)`**"
        };

        let corpus_mb = report.corpus_size_bytes as f64 / (1024.0 * 1024.0);

        md.push_str("## 🚀 TTZip Declarative A/B Benchmark Report\n\n");
        md.push_str(&format!(
            "> **Corpus**: `{}` ({:.2} MB) &nbsp;|&nbsp; **Target Filter**: `{}` &nbsp;|&nbsp; **Quality Gate**: {}\n",
            report.corpus_uri, corpus_mb, report.target_filter, status_badge
        ));
        md.push_str(&format!(
            "> **Summary**: **{}** Total Targets &nbsp;|&nbsp; 🟢 **{}** Speedup &nbsp;|&nbsp; 🔴 **{}** Regression &nbsp;|&nbsp; ⚪ **{}** Neutral\n\n",
            report.total_targets, report.speedup_count, report.regression_count, report.neutral_count
        ));

        // Primary Results Table
        md.push_str("| Target URI | Baseline | Candidate | Delta (%) | Speedup (95% CI) | p-value | Verdict | Gate |\n");
        md.push_str("| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |\n");

        if report.items.is_empty() {
            md.push_str("| *(No matching benchmark targets evaluated)* | - | - | - | - | - | - | - |\n");
        } else {
            for item in &report.items {
                let base_str = format_throughput_or_latency(item.throughput_a_mbs, item.stats_a.mean_nanos);
                let cand_str = format_throughput_or_latency(item.throughput_b_mbs, item.stats_b.mean_nanos);
                let delta_str = format_delta_percentage(item.comparison.delta_pct);
                let speedup_str = format!(
                    "**{:.2}x** [{:.2}, {:.2}]",
                    item.speedup_ratio,
                    item.comparison.speedup_ci.lower,
                    item.comparison.speedup_ci.upper
                );
                let p_val_str = if item.comparison.t_test.p_value < 0.0001 {
                    "`< 0.0001`".to_string()
                } else {
                    format!("`{:.4}`", item.comparison.t_test.p_value)
                };

                let verdict_badge = match item.verdict {
                    DecisionVerdict::SignificantSpeedup => "🟢 Speedup",
                    DecisionVerdict::SignificantRegression => "🔴 Regression",
                    DecisionVerdict::NeutralNoise => "⚪ Neutral",
                };

                let gate_badge = if item.passed_gate { "✅ Pass" } else { "❌ Fail" };

                md.push_str(&format!(
                    "| `{}` | {} | {} | **{}** | {} | {} | {} | {} |\n",
                    item.descriptor.uri,
                    base_str,
                    cand_str,
                    delta_str,
                    speedup_str,
                    p_val_str,
                    verdict_badge,
                    gate_badge,
                ));
            }
        }

        md.push_str("\n<details>\n<summary>🔍 <b>Statistical Measurement Breakdown & Degrees of Freedom</b></summary>\n\n");
        md.push_str("| Target URI | Baseline (Mean ± MAD) | Candidate (Mean ± MAD) | t-Statistic | DOF | RSE (%) | Outliers |\n");
        md.push_str("| :--- | :--- | :--- | :--- | :--- | :--- | :--- |\n");

        for item in &report.items {
            let base_mean = format_duration(item.stats_a.mean_nanos);
            let base_mad = format_duration(item.stats_a.mad_nanos);
            let cand_mean = format_duration(item.stats_b.mean_nanos);
            let cand_mad = format_duration(item.stats_b.mad_nanos);

            md.push_str(&format!(
                "| `{}` | {} ± {} | {} ± {} | {:.2} | {:.1} | {:.2}% vs {:.2}% | {} / {} |\n",
                item.descriptor.uri,
                base_mean,
                base_mad,
                cand_mean,
                cand_mad,
                item.comparison.t_test.t_statistic,
                item.comparison.t_test.degrees_of_freedom,
                item.stats_a.rse_pct,
                item.stats_b.rse_pct,
                item.stats_a.outliers_count,
                item.stats_b.outliers_count,
            ));
        }

        md.push_str("\n</details>\n");

        md
    }
}

// ============================================================================
// Formatting Helpers
// ============================================================================

/// Formats throughput in MB/s or latency if throughput is negligible.
fn format_throughput_or_latency(mbs: f64, nanos: f64) -> String {
    if mbs > 0.01 {
        format!("{:.1} MB/s", mbs)
    } else {
        format_duration(nanos)
    }
}

/// Formats duration nanoseconds into human readable string.
fn format_duration(nanos: f64) -> String {
    if nanos < 1_000.0 {
        format!("{:.1} ns", nanos)
    } else if nanos < 1_000_000.0 {
        format!("{:.2} µs", nanos / 1_000.0)
    } else if nanos < 1_000_000_000.0 {
        format!("{:.2} ms", nanos / 1_000_000.0)
    } else {
        format!("{:.2} s", nanos / 1_000_000_000.0)
    }
}

/// Formats percentage delta with sign.
fn format_delta_percentage(delta: f64) -> String {
    if delta > 0.0 {
        format!("+{:+.2}%", delta)
    } else {
        format!("{:+.2}%", delta)
    }
}
