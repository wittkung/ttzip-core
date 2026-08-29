// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI High-Performance Benchmark, MIPS Rating, Pareto Analysis, and Matrix Gate Suite.
//!
//! Provides typed, memory-safe, and Swift 6 Sendable exports for:
//! 1. 50-Point Matrix Gate Evaluation & Custom Benchmark Runs (`ttzip_bench_run_gate`, `ttzip_bench_run_matrix`)
//! 2. 2D Pareto Frontier & Convex Hull Analysis (`ttzip_bench_calculate_pareto_frontier`)
//! 3. Vector SVG & Standalone HTML Dashboard Generation (`ttzip_bench_generate_svg_pareto`, `ttzip_bench_generate_html_dashboard`)
//! 4. 7-Zip Aligned MIPS Hardware Evaluation (`ttzip_bench_run_mips`)
//! 5. Deterministic Mathematical & Multimodal Corpus Generators (`generate_synthetic_benchmark_dataset`, `ttzip_bench_generate_corpus_bytes`, `ttzip_bench_list_silesia_entries`)
//! 6. 24-Point Enterprise Scenario Matrix (`ttzip_bench_run_all_scenarios`)

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use super::types::TTZipError;
use crate::benchmark::corpus::{BenchmarkCorpusGenerator, BenchmarkCorpusType};
use crate::benchmark::mips::MIPSHardwareBenchmarkEngine;
use crate::benchmark::multimodal_loader::{
    compute_shannon_entropy, SILESIA_STANDARD_FILES,
};
use crate::benchmark::pareto::{calculate_pareto_frontier, ParetoCodecPoint};
use crate::benchmark::plotter::BenchmarkPlotter;
use crate::benchmark::runner::{BenchmarkMatrixRunner, BenchmarkPointResult};
use crate::benchmark::scenario_driver::ScenarioBenchmarkDriver;

const IO_BUFFER_SIZE: usize = 1024 * 1024; // 1MB Stream Buffer
const CHUNK_SIZE: usize = 4 * 1024 * 1024; // 4MB Chunk

// MARK: - Strongly Typed UniFFI Models

/// Strongly typed corpus types for benchmark dataset selection.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum UniFFICorpusType {
    Calgary,
    Silesia,
    Xml,
    Random,
    Binary,
    TextData,
    ShortMatch,
    Dna,
    Noise,
    Literals,
    MachOBinary,
    RealisticRgb,
    StripedRgb,
}

impl From<UniFFICorpusType> for BenchmarkCorpusType {
    fn from(c: UniFFICorpusType) -> Self {
        match c {
            UniFFICorpusType::Calgary => BenchmarkCorpusType::Calgary,
            UniFFICorpusType::Silesia => BenchmarkCorpusType::Silesia,
            UniFFICorpusType::Xml => BenchmarkCorpusType::Xml,
            UniFFICorpusType::Random => BenchmarkCorpusType::Random,
            UniFFICorpusType::Binary => BenchmarkCorpusType::Binary,
            UniFFICorpusType::TextData => BenchmarkCorpusType::TextData,
            UniFFICorpusType::ShortMatch => BenchmarkCorpusType::ShortMatch,
            UniFFICorpusType::Dna => BenchmarkCorpusType::Dna,
            UniFFICorpusType::Noise => BenchmarkCorpusType::Noise,
            UniFFICorpusType::Literals => BenchmarkCorpusType::Literals,
            UniFFICorpusType::MachOBinary => BenchmarkCorpusType::MachOBinary,
            UniFFICorpusType::RealisticRgb => BenchmarkCorpusType::RealisticRgb,
            UniFFICorpusType::StripedRgb => BenchmarkCorpusType::StripedRgb,
        }
    }
}

impl From<BenchmarkCorpusType> for UniFFICorpusType {
    fn from(c: BenchmarkCorpusType) -> Self {
        match c {
            BenchmarkCorpusType::Calgary => UniFFICorpusType::Calgary,
            BenchmarkCorpusType::Silesia => UniFFICorpusType::Silesia,
            BenchmarkCorpusType::Xml => UniFFICorpusType::Xml,
            BenchmarkCorpusType::Random => UniFFICorpusType::Random,
            BenchmarkCorpusType::Binary => UniFFICorpusType::Binary,
            BenchmarkCorpusType::TextData => UniFFICorpusType::TextData,
            BenchmarkCorpusType::ShortMatch => UniFFICorpusType::ShortMatch,
            BenchmarkCorpusType::Dna => UniFFICorpusType::Dna,
            BenchmarkCorpusType::Noise => UniFFICorpusType::Noise,
            BenchmarkCorpusType::Literals => UniFFICorpusType::Literals,
            BenchmarkCorpusType::MachOBinary => UniFFICorpusType::MachOBinary,
            BenchmarkCorpusType::RealisticRgb => UniFFICorpusType::RealisticRgb,
            BenchmarkCorpusType::StripedRgb => UniFFICorpusType::StripedRgb,
        }
    }
}

/// Metrics for an individual algorithm benchmark point.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFIBenchmarkPointResult {
    pub algorithm: String,
    pub level: i32,
    pub display_name: String,
    pub original_size_bytes: u64,
    pub compressed_size_bytes: u64,
    pub compression_ratio: f64,
    pub space_savings_pct: f64,
    pub compress_throughput_mbs: f64,
    pub decompress_throughput_mbs: f64,
    pub compress_time_nanos: u64,
    pub decompress_time_nanos: u64,
    pub pareto_rank: u32,
    pub is_pareto_optimal: bool,
    pub is_on_convex_hull: bool,
}

impl From<BenchmarkPointResult> for UniFFIBenchmarkPointResult {
    fn from(p: BenchmarkPointResult) -> Self {
        Self {
            algorithm: p.algorithm,
            level: p.level,
            display_name: p.display_name,
            original_size_bytes: p.original_size_bytes as u64,
            compressed_size_bytes: p.compressed_size_bytes as u64,
            compression_ratio: p.compression_ratio,
            space_savings_pct: p.space_savings_pct,
            compress_throughput_mbs: p.compress_throughput_mbs,
            decompress_throughput_mbs: p.decompress_throughput_mbs,
            compress_time_nanos: p.compress_time_nanos,
            decompress_time_nanos: p.decompress_time_nanos,
            pareto_rank: p.pareto_rank,
            is_pareto_optimal: p.is_pareto_optimal,
            is_on_convex_hull: p.is_on_convex_hull,
        }
    }
}

/// Comprehensive benchmark matrix report across all evaluated codecs.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFIBenchmarkMatrixReport {
    pub corpus_type: UniFFICorpusType,
    pub corpus_name: String,
    pub corpus_size_bytes: u64,
    pub timestamp_epoch_secs: u64,
    pub total_points_evaluated: u32,
    pub pareto_optimal_count: u32,
    pub peak_compress_throughput_mbs: f64,
    pub peak_decompress_throughput_mbs: f64,
    pub max_space_savings_pct: f64,
    pub points: Vec<UniFFIBenchmarkPointResult>,
    pub passed_gate: bool,
}

/// 2D Pareto and Upper Convex Hull point representation for compression codecs.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFIParetoCodecPoint {
    pub codec_name: String,
    pub compression_ratio: f64,
    pub speed_mb_s: f64,
    pub memory_mb: f64,
    pub pareto_rank: u32,
    pub is_pareto_optimal: bool,
    pub is_on_convex_hull: bool,
}

impl From<ParetoCodecPoint> for UniFFIParetoCodecPoint {
    fn from(p: ParetoCodecPoint) -> Self {
        Self {
            codec_name: p.codec_name,
            compression_ratio: p.compression_ratio,
            speed_mb_s: p.speed_mb_s,
            memory_mb: p.memory_mb,
            pareto_rank: p.pareto_rank,
            is_pareto_optimal: p.is_pareto_optimal,
            is_on_convex_hull: p.is_on_convex_hull,
        }
    }
}

impl From<UniFFIParetoCodecPoint> for ParetoCodecPoint {
    fn from(p: UniFFIParetoCodecPoint) -> Self {
        let mut point = ParetoCodecPoint::new(p.codec_name, p.compression_ratio, p.speed_mb_s, p.memory_mb);
        point.pareto_rank = p.pareto_rank;
        point.is_pareto_optimal = p.is_pareto_optimal;
        point.is_on_convex_hull = p.is_on_convex_hull;
        point
    }
}

/// Standardized 7-Zip aligned MIPS hardware benchmark telemetry.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFIMipsBenchmarkResult {
    pub dictionary_size_mb: u32,
    pub thread_count: u32,
    pub compress_mips: f64,
    pub decompress_mips: f64,
    pub total_mips: f64,
    pub compress_speed_mbs: f64,
    pub decompress_speed_mbs: f64,
    pub cpu_usage_percent: f64,
    pub rating_per_usage_mips: f64,
}

/// Metadata descriptor for a multi-modal corpus entry.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFIMultimodalEntryMetadata {
    pub name: String,
    pub kind_name: String,
    pub size_bytes: u64,
    pub shannon_entropy: f64,
    pub is_synthetic: bool,
}

/// Metrics for an individual enterprise scenario benchmark point.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFIScenarioBenchmarkPoint {
    pub id: String,
    pub category: String,
    pub format: String,
    pub display_name: String,
    pub options_summary: String,
    pub original_size_bytes: u64,
    pub output_size_bytes: u64,
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

/// Comprehensive 24-point enterprise scenario benchmark report.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFIScenarioMatrixReport {
    pub total_scenarios_evaluated: u32,
    pub timestamp_epoch_secs: u64,
    pub peak_create_throughput_mbs: f64,
    pub peak_extract_throughput_mbs: f64,
    pub all_invariants_passed: bool,
    pub points: Vec<UniFFIScenarioBenchmarkPoint>,
}

// MARK: - UniFFI Export Functions

/// Executes standard 50-point Matrix Gate pass.
#[uniffi::export]
pub fn ttzip_bench_run_gate() -> Result<UniFFIBenchmarkMatrixReport, TTZipError> {
    let report = BenchmarkMatrixRunner::run_gate().map_err(|st| TTZipError::EngineError { code: st as i32 })?;
    Ok(UniFFIBenchmarkMatrixReport {
        corpus_type: report.corpus_type.into(),
        corpus_name: report.corpus_name,
        corpus_size_bytes: report.corpus_size_bytes as u64,
        timestamp_epoch_secs: report.timestamp_epoch_secs,
        total_points_evaluated: report.total_points_evaluated as u32,
        pareto_optimal_count: report.pareto_optimal_count as u32,
        peak_compress_throughput_mbs: report.peak_compress_throughput_mbs,
        peak_decompress_throughput_mbs: report.peak_decompress_throughput_mbs,
        max_space_savings_pct: report.max_space_savings_pct,
        points: report.points.into_iter().map(Into::into).collect(),
        passed_gate: report.passed_gate,
    })
}

/// Executes matrix benchmark for specified corpus and size.
#[uniffi::export]
pub fn ttzip_bench_run_matrix(
    corpus_type: UniFFICorpusType,
    corpus_size_bytes: u64,
    iterations: u32,
) -> Result<UniFFIBenchmarkMatrixReport, TTZipError> {
    let ct = BenchmarkCorpusType::from(corpus_type);
    let report = BenchmarkMatrixRunner::run_matrix(ct, corpus_size_bytes as usize, iterations as usize)
        .map_err(|st| TTZipError::EngineError { code: st as i32 })?;
    Ok(UniFFIBenchmarkMatrixReport {
        corpus_type: report.corpus_type.into(),
        corpus_name: report.corpus_name,
        corpus_size_bytes: report.corpus_size_bytes as u64,
        timestamp_epoch_secs: report.timestamp_epoch_secs,
        total_points_evaluated: report.total_points_evaluated as u32,
        pareto_optimal_count: report.pareto_optimal_count as u32,
        peak_compress_throughput_mbs: report.peak_compress_throughput_mbs,
        peak_decompress_throughput_mbs: report.peak_decompress_throughput_mbs,
        max_space_savings_pct: report.max_space_savings_pct,
        points: report.points.into_iter().map(Into::into).collect(),
        passed_gate: report.passed_gate,
    })
}

/// Calculates 2D Pareto frontier and Upper Convex Hull on codec points.
#[uniffi::export]
pub fn ttzip_bench_calculate_pareto_frontier(
    points: Vec<UniFFIParetoCodecPoint>,
) -> Vec<UniFFIParetoCodecPoint> {
    let mut raw_points: Vec<ParetoCodecPoint> = points.into_iter().map(Into::into).collect();
    calculate_pareto_frontier(&mut raw_points);
    raw_points.into_iter().map(Into::into).collect()
}

/// Generates standalone SVG vector scatter plot with Fritsch-Carlson Pareto spline.
#[uniffi::export]
pub fn ttzip_bench_generate_svg_pareto(
    corpus_type: UniFFICorpusType,
    width: u32,
    height: u32,
) -> Result<String, TTZipError> {
    let ct = BenchmarkCorpusType::from(corpus_type);
    let report = BenchmarkMatrixRunner::run_matrix(ct, 64 * 1024, 1)
        .map_err(|st| TTZipError::EngineError { code: st as i32 })?;
    Ok(BenchmarkPlotter::generate_svg(&report, width, height))
}

/// Generates standalone interactive HTML dashboard for matrix benchmark.
#[uniffi::export]
pub fn ttzip_bench_generate_html_dashboard(
    corpus_type: UniFFICorpusType,
) -> Result<String, TTZipError> {
    let ct = BenchmarkCorpusType::from(corpus_type);
    let report = BenchmarkMatrixRunner::run_matrix(ct, 64 * 1024, 1)
        .map_err(|st| TTZipError::EngineError { code: st as i32 })?;
    Ok(BenchmarkPlotter::generate_html_dashboard(&report))
}

/// Executes a standardized 7-Zip aligned MIPS hardware benchmark pass.
#[uniffi::export]
pub fn ttzip_bench_run_mips(
    dictionary_size_mb: u32,
    thread_count: u32,
    iterations: u32,
) -> Result<UniFFIMipsBenchmarkResult, TTZipError> {
    let result = MIPSHardwareBenchmarkEngine::run_benchmark(dictionary_size_mb, thread_count, iterations)
        .map_err(|st| TTZipError::EngineError { code: st as i32 })?;
    Ok(UniFFIMipsBenchmarkResult {
        dictionary_size_mb: result.dictionary_size_mb,
        thread_count: result.thread_count,
        compress_mips: result.compress_mips,
        decompress_mips: result.decompress_mips,
        total_mips: result.total_mips,
        compress_speed_mbs: result.compress_speed_mbs,
        decompress_speed_mbs: result.decompress_speed_mbs,
        cpu_usage_percent: result.cpu_usage_percent,
        rating_per_usage_mips: result.rating_per_usage_mips,
    })
}

/// Generates a synthetic benchmark dataset file using mathematical generators.
#[uniffi::export]
pub fn generate_synthetic_benchmark_dataset(
    target_path: String,
    target_bytes: u64,
    profile_name: String,
) -> Result<(), TTZipError> {
    let path = Path::new(&target_path);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| TTZipError::IoError {
                message: format!("Failed to create parent directory for dataset: {}", e),
            })?;
        }
    }

    let file = File::create(path).map_err(|e| TTZipError::IoError {
        message: format!("Failed to create benchmark dataset file at {}: {}", target_path, e),
    })?;
    let mut writer = BufWriter::with_capacity(IO_BUFFER_SIZE, file);

    let corpus_type = BenchmarkCorpusType::from_str_id(&profile_name)
        .unwrap_or(BenchmarkCorpusType::Noise);

    let mut written: u64 = 0;
    while written < target_bytes {
        let current_chunk_size = std::cmp::min((target_bytes - written) as usize, CHUNK_SIZE);
        let chunk = BenchmarkCorpusGenerator::generate(corpus_type, current_chunk_size);
        writer.write_all(&chunk).map_err(|e| TTZipError::IoError {
            message: format!("Failed writing benchmark dataset chunk at offset {}: {}", written, e),
        })?;
        written += current_chunk_size as u64;
    }

    writer.flush().map_err(|e| TTZipError::IoError {
        message: format!("Failed to flush benchmark dataset file: {}", e),
    })?;

    Ok(())
}

/// Generates in-memory synthetic corpus bytes directly.
#[uniffi::export]
pub fn ttzip_bench_generate_corpus_bytes(
    corpus_type: UniFFICorpusType,
    size_bytes: u64,
) -> Vec<u8> {
    let ct = BenchmarkCorpusType::from(corpus_type);
    BenchmarkCorpusGenerator::generate(ct, size_bytes as usize)
}

/// Lists standard Silesia multi-modal corpus catalog metadata.
#[uniffi::export]
pub fn ttzip_bench_list_silesia_entries() -> Vec<UniFFIMultimodalEntryMetadata> {
    SILESIA_STANDARD_FILES
        .iter()
        .map(|&(name, size, ct)| {
            let synthetic_chunk = BenchmarkCorpusGenerator::generate(ct, size.min(65536));
            let entropy = compute_shannon_entropy(&synthetic_chunk);
            UniFFIMultimodalEntryMetadata {
                name: name.to_string(),
                kind_name: "Silesia".to_string(),
                size_bytes: size as u64,
                shannon_entropy: entropy,
                is_synthetic: true,
            }
        })
        .collect()
}

/// Executes all 100 enterprise full-scenario benchmark points.
#[uniffi::export]
pub fn ttzip_bench_run_all_scenarios() -> Result<UniFFIScenarioMatrixReport, TTZipError> {
    let report = ScenarioBenchmarkDriver::run_all_scenarios()
        .map_err(|st| TTZipError::EngineError { code: st as i32 })?;

    let points: Vec<UniFFIScenarioBenchmarkPoint> = report
        .points
        .into_iter()
        .map(|p| UniFFIScenarioBenchmarkPoint {
            id: p.id,
            category: p.category,
            format: p.format,
            display_name: p.display_name,
            options_summary: p.options_summary,
            original_size_bytes: p.original_size_bytes as u64,
            output_size_bytes: p.output_size_bytes as u64,
            space_savings_pct: p.space_savings_pct,
            create_throughput_mbs: p.create_throughput_mbs,
            extract_throughput_mbs: p.extract_throughput_mbs,
            create_duration_micros: p.create_duration_micros,
            extract_duration_micros: p.extract_duration_micros,
            is_encrypted: p.is_encrypted,
            is_split: p.is_split,
            is_solid: p.is_solid,
            passed_invariants: p.passed_invariants,
        })
        .collect();

    Ok(UniFFIScenarioMatrixReport {
        total_scenarios_evaluated: report.total_scenarios_evaluated as u32,
        timestamp_epoch_secs: report.timestamp_epoch_secs,
        peak_create_throughput_mbs: report.peak_create_throughput_mbs,
        peak_extract_throughput_mbs: report.peak_extract_throughput_mbs,
        all_invariants_passed: report.all_invariants_passed,
        points,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_uniffi_benchmark_exports() {
        let gate_res = ttzip_bench_run_gate();
        assert!(gate_res.is_ok());
        let gate_report = gate_res.unwrap();
        assert!(gate_report.passed_gate);
        assert!(!gate_report.points.is_empty());

        let svg_res = ttzip_bench_generate_svg_pareto(UniFFICorpusType::TextData, 800, 450);
        assert!(svg_res.is_ok());
        assert!(svg_res.unwrap().contains("<svg"));

        let html_res = ttzip_bench_generate_html_dashboard(UniFFICorpusType::TextData);
        assert!(html_res.is_ok());
        assert!(html_res.unwrap().contains("<!DOCTYPE html>"));

        let silesia_list = ttzip_bench_list_silesia_entries();
        assert_eq!(silesia_list.len(), 12);

        let scenario_res = ttzip_bench_run_all_scenarios();
        assert!(scenario_res.is_ok());
        let scenario_report = scenario_res.unwrap();
        assert_eq!(scenario_report.total_scenarios_evaluated, 100);
        assert!(scenario_report.all_invariants_passed);

        let temp = tempdir().unwrap();
        let target_file = temp.path().join("dataset.bin");
        let gen_res = generate_synthetic_benchmark_dataset(
            target_file.to_string_lossy().to_string(),
            65536,
            "text".to_string(),
        );
        assert!(gen_res.is_ok());
        assert_eq!(std::fs::metadata(&target_file).unwrap().len(), 65536);
    }
}

