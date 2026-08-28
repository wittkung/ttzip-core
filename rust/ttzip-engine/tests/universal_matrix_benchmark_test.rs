// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Universal 8x8 Matrix Benchmark & Codec Regression Test Suite.
//!
//! Validates Phase 4 & Phase 5 performance invariants:
//! 1. 8 Mathematical Synthetic Corpora Generation and Entropy Bounds
//! 2. 8 Compression Algorithms (Deflate, Zstd, LZMA2, Brotli, Bzip2, Snappy, LZ4, LZFSE)
//! 3. 100% Roundtrip Fidelity Across All Codec x Corpus Combinations
//! 4. Incompressible Noise and Extreme Boundary Degradation Handling
//! 5. 50-Point Matrix Gate Execution and Pareto Frontier Optimality

use ttzip_engine::analytics::entropy::compute_shannon_entropy;
use ttzip_engine::benchmark::codecs_driver::{
    BrotliBenchmarkDriver, Bzip2BenchmarkDriver, CodecBenchmarkDriver, DeflateBenchmarkDriver,
    Lz4BenchmarkDriver, LzfseBenchmarkDriver, Lzma2BenchmarkDriver, MatrixCodecDriver,
    SnappyBenchmarkDriver, ZstdBenchmarkDriver,
};
use ttzip_engine::benchmark::corpus::{BenchmarkCorpusGenerator, BenchmarkCorpusType};
use ttzip_engine::benchmark::runner::BenchmarkMatrixRunner;

const BENCH_CORPUS_SIZE: usize = 64 * 1024; // 64KB per test cell

// MARK: - 1. Corpus Generation & Shannon Entropy Bounds

#[test]
fn test_all_8_mathematical_corpora_generation_and_entropy_bounds() {
    let test_cases = [
        (BenchmarkCorpusType::TextData, "TextData", 3.0, 7.0),
        (BenchmarkCorpusType::ShortMatch, "ShortMatch", 3.0, 8.0),
        (BenchmarkCorpusType::Dna, "Dna", 1.5, 2.5),
        (BenchmarkCorpusType::Noise, "Noise", 7.80, 8.0),
        (BenchmarkCorpusType::Literals, "Literals", 4.0, 8.0),
        (BenchmarkCorpusType::MachOBinary, "MachOBinary", 3.0, 8.0),
        (BenchmarkCorpusType::RealisticRgb, "RealisticRgb", 3.0, 8.0),
        (BenchmarkCorpusType::StripedRgb, "StripedRgb", 0.5, 8.0),
    ];

    for (corpus_type, name, min_entropy, max_entropy) in test_cases {
        let data = BenchmarkCorpusGenerator::generate(corpus_type, BENCH_CORPUS_SIZE);
        assert_eq!(
            data.len(),
            BENCH_CORPUS_SIZE,
            "Corpus {} must match requested size",
            name
        );

        let entropy = compute_shannon_entropy(&data);
        assert!(
            entropy >= min_entropy && entropy <= max_entropy,
            "Corpus {} entropy {:.4} out of expected range [{:.2}, {:.2}]",
            name,
            entropy,
            min_entropy,
            max_entropy
        );
    }
}

// MARK: - 2. Full 8x8 Matrix Roundtrip Fidelity

#[test]
fn test_universal_matrix_8_codecs_by_8_corpora_roundtrip_fidelity() {
    let corpora = [
        BenchmarkCorpusType::TextData,
        BenchmarkCorpusType::ShortMatch,
        BenchmarkCorpusType::Dna,
        BenchmarkCorpusType::Noise,
        BenchmarkCorpusType::Literals,
        BenchmarkCorpusType::MachOBinary,
        BenchmarkCorpusType::RealisticRgb,
        BenchmarkCorpusType::StripedRgb,
    ];

    let deflate = DeflateBenchmarkDriver;
    let zstd = ZstdBenchmarkDriver;
    let lzma2 = Lzma2BenchmarkDriver;
    let brotli = BrotliBenchmarkDriver;
    let bzip2 = Bzip2BenchmarkDriver;
    let snappy = SnappyBenchmarkDriver;
    let lz4 = Lz4BenchmarkDriver;
    let lzfse = LzfseBenchmarkDriver;

    let drivers: Vec<(&str, &dyn CodecBenchmarkDriver, i32)> = vec![
        ("Deflate-L6", &deflate, 6),
        ("Deflate-L12", &deflate, 12),
        ("Zstd-L3", &zstd, 3),
        ("Zstd-L19", &zstd, 19),
        ("LZMA2-L3", &lzma2, 3),
        ("LZMA2-L9", &lzma2, 9),
        ("Brotli-Q4", &brotli, 4),
        ("Brotli-Q11", &brotli, 11),
        ("Bzip2-L1", &bzip2, 1),
        ("Bzip2-L9", &bzip2, 9),
        ("Snappy-Raw", &snappy, 1),
        ("Snappy-Framed", &snappy, 2),
        ("LZ4-Fast", &lz4, 1),
        ("LZ4-HC", &lz4, 19),
        ("LZFSE", &lzfse, 1),
    ];

    for &corpus_type in &corpora {
        let corpus_data = BenchmarkCorpusGenerator::generate(corpus_type, BENCH_CORPUS_SIZE);

        for &(codec_name, driver, level) in &drivers {
            let compressed = driver
                .bench_compress(&corpus_data, level)
                .unwrap_or_else(|e| panic!("Compression failed for {} on {:?}: {:?}", codec_name, corpus_type, e));

            assert!(
                !compressed.is_empty(),
                "Compressed output for {} on {:?} must not be empty",
                codec_name,
                corpus_type
            );

            let decompressed = driver
                .bench_decompress(&compressed, corpus_data.len())
                .unwrap_or_else(|e| panic!("Decompression failed for {} on {:?}: {:?}", codec_name, corpus_type, e));

            assert_eq!(
                decompressed,
                corpus_data,
                "Byte-exact mismatch for {} on {:?}",
                codec_name,
                corpus_type
            );
        }
    }
}

// MARK: - 3. Deflate, Zstd, and LZMA2 Level Sweeps

#[test]
fn test_matrix_deflate_all_levels_0_to_12() {
    let corpus_data = BenchmarkCorpusGenerator::generate(BenchmarkCorpusType::TextData, 32 * 1024);
    let driver = DeflateBenchmarkDriver;

    for level in 0..=12 {
        let compressed = driver.bench_compress(&corpus_data, level).expect("deflate compress");
        let decompressed = driver.bench_decompress(&compressed, corpus_data.len()).expect("deflate decompress");
        assert_eq!(decompressed, corpus_data, "Deflate L{} mismatch", level);

        if level == 0 {
            assert!(
                compressed.len() >= corpus_data.len(),
                "Store mode L0 should not shrink"
            );
        } else {
            assert!(
                compressed.len() < corpus_data.len(),
                "Deflate L{} should compress text",
                level
            );
        }
    }
}

#[test]
fn test_matrix_zstd_levels_sweep() {
    let corpus_data = BenchmarkCorpusGenerator::generate(BenchmarkCorpusType::TextData, 32 * 1024);
    let driver = ZstdBenchmarkDriver;

    let test_levels = [1, 3, 7, 11, 15, 19, 22, 100];
    for &level in &test_levels {
        let compressed = driver.bench_compress(&corpus_data, level).expect("zstd compress");
        let decompressed = driver.bench_decompress(&compressed, corpus_data.len()).expect("zstd decompress");
        assert_eq!(decompressed, corpus_data, "Zstd L{} mismatch", level);
    }
}

#[test]
fn test_matrix_lzma2_all_levels_0_to_9() {
    let corpus_data = BenchmarkCorpusGenerator::generate(BenchmarkCorpusType::ShortMatch, 32 * 1024);
    let driver = Lzma2BenchmarkDriver;

    for level in 0..=9 {
        let compressed = driver.bench_compress(&corpus_data, level).expect("lzma2 compress");
        let decompressed = driver.bench_decompress(&compressed, corpus_data.len()).expect("lzma2 decompress");
        assert_eq!(decompressed, corpus_data, "LZMA2 L{} mismatch", level);
    }
}

// MARK: - 4. Unified Matrix Runner & Gate Verification

#[test]
fn test_unified_matrix_runner_and_gate_pass() {
    let report = BenchmarkMatrixRunner::run_matrix(BenchmarkCorpusType::TextData, 64 * 1024, 1)
        .expect("matrix runner execution failed");

    assert!(!report.points.is_empty(), "Report must contain evaluated codec points");
    assert!(report.total_points_evaluated >= 15, "Expected >= 15 configurations");
    assert!(report.pareto_optimal_count > 0, "Expected at least 1 Pareto optimal point");
    assert!(report.peak_compress_throughput_mbs > 0.0, "Peak compression speed > 0");
    assert!(report.peak_decompress_throughput_mbs > 0.0, "Peak decompression speed > 0");

    let gate_report = BenchmarkMatrixRunner::run_gate().expect("gate pass failed");
    assert!(gate_report.passed_gate, "Matrix gate must pass");
}

// MARK: - 5. Extreme Boundary & Degenerate Payloads

#[test]
fn test_matrix_extreme_boundaries_and_empty_inputs() {
    let empty_payload: [u8; 0] = [];
    let single_byte = [0x5Au8];
    let all_zeros = [0u8; 4096];

    let configs = MatrixCodecDriver::all_matrix_configs();
    for cfg in configs.iter().take(10) {
        // 1. Empty payload
        let c_empty = MatrixCodecDriver::compress(cfg, &empty_payload).expect("compress empty");
        let d_empty = MatrixCodecDriver::decompress(cfg, &c_empty, 0).expect("decompress empty");
        assert_eq!(d_empty.len(), 0);

        // 2. Single byte
        let c_single = MatrixCodecDriver::compress(cfg, &single_byte).expect("compress 1 byte");
        let d_single = MatrixCodecDriver::decompress(cfg, &c_single, 1).expect("decompress 1 byte");
        assert_eq!(d_single, single_byte);

        // 3. Repeated zeros (extreme compression ratio)
        let c_zeros = MatrixCodecDriver::compress(cfg, &all_zeros).expect("compress zeros");
        let d_zeros = MatrixCodecDriver::decompress(cfg, &c_zeros, all_zeros.len()).expect("decompress zeros");
        assert_eq!(d_zeros, all_zeros);
    }
}
