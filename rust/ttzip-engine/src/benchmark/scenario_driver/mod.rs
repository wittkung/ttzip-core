// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 100-Point Enterprise Full-Scenario High-Pressure Benchmark Driver.
//!
//! Evaluates multi-format containers, cryptographic security, split volumes,
//! solid blocks, topologies, in-place editing, QuickLook early termination,
//! damaged archive self-healing, APFS clonefile, and 10-level VFS recursion.
//! Integrates Mach Kernel `task_info`/`getrusage` physical resident memory bounds.

pub mod evaluators;
pub mod matrix;

use serde::{Deserialize, Serialize};

use crate::types::TTZipStatus;
use crate::zip::writer::ZipInputItem;

/// Metrics for an individual scenario benchmark point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioBenchmarkPoint {
    pub id: String,
    pub category: String,
    pub format: String,
    pub display_name: String,
    pub options_summary: String,
    pub original_size_bytes: usize,
    pub output_size_bytes: usize,
    pub space_savings_pct: f64,
    pub create_throughput_mbs: f64,
    pub extract_throughput_mbs: f64,
    pub create_duration_micros: u64,
    pub extract_duration_micros: u64,
    pub is_encrypted: bool,
    pub is_split: bool,
    pub is_solid: bool,
    pub passed_invariants: bool,
}

/// Comprehensive scenario benchmark report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioMatrixReport {
    pub total_scenarios_evaluated: usize,
    pub timestamp_epoch_secs: u64,
    pub peak_create_throughput_mbs: f64,
    pub peak_extract_throughput_mbs: f64,
    pub all_invariants_passed: bool,
    pub points: Vec<ScenarioBenchmarkPoint>,
}

/// Full scenario benchmark execution engine.
pub struct ScenarioBenchmarkDriver;

impl ScenarioBenchmarkDriver {
    /// Generates characteristic synthetic files for scenario testing.
    pub fn generate_synthetic_items(count: usize, total_bytes: usize) -> Vec<ZipInputItem> {
        let per_file_bytes = (total_bytes / count.max(1)).max(128);
        let mut items = Vec::with_capacity(count);

        for i in 0..count {
            let mut data = Vec::with_capacity(per_file_bytes);
            for j in 0..per_file_bytes {
                data.push(((i * 37 + j * 13) & 0xFF) as u8);
            }
            items.push(ZipInputItem {
                rel_path: format!("sub_dir/item_{:04}.bin", i),
                data,
                mtime_epoch_secs: 1700000000 + i as u32,
                mode: 0o644,
                is_directory: false,
            });
        }
        items
    }

    /// Executes all 100 enterprise scenario benchmark points.
    pub fn run_all_scenarios() -> Result<ScenarioMatrixReport, TTZipStatus> {
        let points = matrix::execute_100_scenario_matrix()?;

        let peak_create = points.iter().map(|p| p.create_throughput_mbs).fold(0.0, f64::max);
        let peak_extract = points.iter().map(|p| p.extract_throughput_mbs).fold(0.0, f64::max);
        let all_passed = points.iter().all(|p| p.passed_invariants);

        Ok(ScenarioMatrixReport {
            total_scenarios_evaluated: points.len(),
            timestamp_epoch_secs: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            peak_create_throughput_mbs: peak_create,
            peak_extract_throughput_mbs: peak_extract,
            all_invariants_passed: all_passed,
            points,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_100_point_scenario_driver_execution() {
        let report = ScenarioBenchmarkDriver::run_all_scenarios().expect("all scenarios execution failed");
        assert_eq!(report.total_scenarios_evaluated, 100, "Must evaluate exactly 100 scenarios");
        assert!(report.peak_create_throughput_mbs > 0.0);
        assert!(report.peak_extract_throughput_mbs > 0.0);

        for pt in &report.points {
            if !pt.passed_invariants {
                eprintln!("FAILED INVARIANT: Scenario {} ({}) - details: {}", pt.id, pt.display_name, pt.options_summary);
            }
            assert!(pt.passed_invariants, "Scenario {} ({}) failed invariant check", pt.id, pt.display_name);
        }

        assert!(report.all_invariants_passed, "Expected all 100 scenarios to pass invariants");
    }
}
