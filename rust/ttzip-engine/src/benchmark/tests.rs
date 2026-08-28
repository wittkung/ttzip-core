// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use crate::benchmark::*;

#[test]
fn test_benchmark_corpus_generation_all_types() {
    let types = [
        BenchmarkCorpusType::Calgary,
        BenchmarkCorpusType::Silesia,
        BenchmarkCorpusType::Xml,
        BenchmarkCorpusType::Random,
        BenchmarkCorpusType::Binary,
    ];

    for ct in types {
        let data = BenchmarkCorpusGenerator::generate(ct, 4096);
        assert_eq!(data.len(), 4096);
        assert!(!data.is_empty());
    }
}

#[test]
fn test_matrix_codecs_roundtrip_all_families() {
    let corpus = BenchmarkCorpusGenerator::generate(BenchmarkCorpusType::Silesia, 8192);

    let test_configs = [
        MatrixCodecConfig::new("Libdeflate", 6, "Libdeflate L6"),
        MatrixCodecConfig::new("Zstd", 3, "Zstd L3"),
        MatrixCodecConfig::new("LZ4", 1, "LZ4 Fast 1"),
        MatrixCodecConfig::new("LZFSE", 1, "Apple LZFSE"),
        MatrixCodecConfig::new("Snappy", 1, "Snappy"),
        MatrixCodecConfig::new("Brotli", 4, "Brotli Q4"),
        MatrixCodecConfig::new("Bzip2", 5, "Bzip2 L5"),
    ];

    for cfg in &test_configs {
        println!("--> Testing codec: {}", cfg.display_name);
        let comp = MatrixCodecDriver::compress(cfg, &corpus)
            .unwrap_or_else(|_| panic!("Compression failed for {}", cfg.display_name));
        assert!(!comp.is_empty());

        let decomp = MatrixCodecDriver::decompress(cfg, &comp, corpus.len())
            .unwrap_or_else(|_| panic!("Decompression failed for {}", cfg.display_name));
        assert_eq!(decomp, corpus);
    }
}

#[test]
fn test_50_point_matrix_gate_execution() {
    let report = BenchmarkMatrixRunner::run_matrix(BenchmarkCorpusType::Xml, 32 * 1024, 1)
        .expect("Matrix gate run failed");

    assert!(report.total_points_evaluated >= 50);
    assert!(report.pareto_optimal_count > 0);
    assert!(report.peak_compress_throughput_mbs > 0.0);
    assert!(report.peak_decompress_throughput_mbs > 0.0);
    assert!(report.max_space_savings_pct > 0.0);
    assert!(report.passed_gate);
}

#[test]
fn test_fritsch_carlson_spline_monotonicity() {
    let points = vec![
        SplinePoint::new(10.0, 10.0),
        SplinePoint::new(20.0, 15.0),
        SplinePoint::new(30.0, 60.0),
        SplinePoint::new(40.0, 80.0),
        SplinePoint::new(50.0, 85.0),
    ];

    let spline = FritschCarlsonSpline::new(points).expect("spline creation");

    // Verify monotonicity across interpolation query points
    let mut prev_y = spline.interpolate(10.0);
    for step in 11..=50 {
        let x = step as f64;
        let y = spline.interpolate(x);
        assert!(y >= prev_y, "Monotonicity violated at x={}: y={} < prev={}", x, y, prev_y);
        prev_y = y;
    }

    let svg_path = spline.to_svg_bezier_path(|x, y| (x * 10.0, y * 10.0));
    assert!(svg_path.starts_with("M 100.00,100.00"));
    assert!(svg_path.contains(" C "));
}

#[test]
fn test_svg_and_html_dashboard_generation() {
    let report = BenchmarkMatrixRunner::run_matrix(BenchmarkCorpusType::Calgary, 32 * 1024, 1)
        .expect("Matrix run");

    let svg = BenchmarkPlotter::generate_svg(&report, 800, 450);
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("TTZip Pareto Frontier:"));
    assert!(svg.ends_with("</svg>"));

    let html = BenchmarkPlotter::generate_html_dashboard(&report);
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("TTZip Multi-Codec Benchmark Dashboard"));
    assert!(html.contains("Matrix Benchmark Points"));
    assert!(html.contains("<svg"));
}

#[test]
fn test_binary_delta_auditor() {
    let raw = BenchmarkCorpusGenerator::generate(BenchmarkCorpusType::Silesia, 16 * 1024);
    let cfg = MatrixCodecConfig::new("Zstd", 3, "Zstd L3");
    let compressed = MatrixCodecDriver::compress(&cfg, &raw).expect("compress");

    let report = BinaryDeltaAuditor::audit(&raw, &compressed, 4096, "Zstd L3");
    assert_eq!(report.total_raw_bytes, 16 * 1024);
    assert_eq!(report.total_compressed_bytes, compressed.len());
    assert_eq!(report.total_segments, 4);
    assert!(report.mean_raw_entropy > 0.0);
    assert!(report.byte_divergence_score > 0.0);

    let json = report.to_json().expect("to json");
    assert!(json.contains("overall_space_savings_pct"));

    let md = report.to_markdown();
    assert!(md.contains("# Binary Delta & Divergence Audit"));
}

#[test]
fn test_benchmark_corpus_8_mathematical_generators() {
    const N: usize = 65536;

    let text = BenchmarkCorpusGenerator::gen_text_data(N);
    assert_eq!(text.len(), N);
    let h_text = compute_shannon_entropy(&text);
    assert!(h_text > 3.0 && h_text < 5.5, "Text entropy expected ~4.0-5.0, got {}", h_text);

    let short_match = BenchmarkCorpusGenerator::gen_short_match_data(N);
    assert_eq!(short_match.len(), N);
    let h_short = compute_shannon_entropy(&short_match);
    assert!(h_short >= 2.0 && h_short <= 8.0, "Short match entropy expected 2.0-8.0, got {}", h_short);

    let dna = BenchmarkCorpusGenerator::gen_dna_data(N);
    assert_eq!(dna.len(), N);
    let h_dna = compute_shannon_entropy(&dna);
    assert!(h_dna > 1.8 && h_dna < 2.2, "DNA 4-symbol entropy expected ~2.0, got {}", h_dna);

    let noise = BenchmarkCorpusGenerator::gen_incompressible_noise(N);
    assert_eq!(noise.len(), N);
    let h_noise = compute_shannon_entropy(&noise);
    assert!(h_noise > 7.95, "Incompressible noise entropy expected >7.95, got {}", h_noise);

    let literals = BenchmarkCorpusGenerator::gen_literals_data(N);
    assert_eq!(literals.len(), N);
    let h_lit = compute_shannon_entropy(&literals);
    assert!(h_lit > 5.0 && h_lit < 7.5, "Literals entropy expected ~6.5, got {}", h_lit);

    let macho = BenchmarkCorpusGenerator::gen_binary_macho_data(N);
    assert_eq!(macho.len(), N);
    assert_eq!(&macho[0..4], &[0xCF, 0xFA, 0xED, 0xFE]); // Mach-O 64-bit magic

    let rgb_real = BenchmarkCorpusGenerator::gen_realistic_rgb_data(N);
    assert_eq!(rgb_real.len(), N);

    let rgb_striped = BenchmarkCorpusGenerator::gen_striped_rgb_data(N);
    assert_eq!(rgb_striped.len(), N);
    let h_striped = compute_shannon_entropy(&rgb_striped);
    assert!(h_striped < 2.0, "Striped RGB entropy expected low, got {}", h_striped);
}

#[test]
fn test_benchmark_corpus_all_enum_types() {
    let all_types = [
        BenchmarkCorpusType::Calgary,
        BenchmarkCorpusType::Silesia,
        BenchmarkCorpusType::Xml,
        BenchmarkCorpusType::Random,
        BenchmarkCorpusType::Binary,
        BenchmarkCorpusType::TextData,
        BenchmarkCorpusType::ShortMatch,
        BenchmarkCorpusType::Dna,
        BenchmarkCorpusType::Noise,
        BenchmarkCorpusType::Literals,
        BenchmarkCorpusType::MachOBinary,
        BenchmarkCorpusType::RealisticRgb,
        BenchmarkCorpusType::StripedRgb,
    ];

    for ct in all_types {
        assert_eq!(BenchmarkCorpusType::from_i32(ct as i32), ct);
        assert!(!ct.name().is_empty());
        assert!(!ct.corpus_id().is_empty());
        let buf = BenchmarkCorpusGenerator::generate(ct, 4096);
        assert_eq!(buf.len(), 4096);
    }

    assert_eq!(BenchmarkCorpusType::from_str_id("text"), Some(BenchmarkCorpusType::TextData));
    assert_eq!(BenchmarkCorpusType::from_str_id("short_match"), Some(BenchmarkCorpusType::ShortMatch));
    assert_eq!(BenchmarkCorpusType::from_str_id("dna"), Some(BenchmarkCorpusType::Dna));
    assert_eq!(BenchmarkCorpusType::from_str_id("noise"), Some(BenchmarkCorpusType::Noise));
    assert_eq!(BenchmarkCorpusType::from_str_id("literals"), Some(BenchmarkCorpusType::Literals));
    assert_eq!(BenchmarkCorpusType::from_str_id("mixed"), Some(BenchmarkCorpusType::MachOBinary));
    assert_eq!(BenchmarkCorpusType::from_str_id("realistic_rgb"), Some(BenchmarkCorpusType::RealisticRgb));
    assert_eq!(BenchmarkCorpusType::from_str_id("striped_rgb"), Some(BenchmarkCorpusType::StripedRgb));
}

#[test]
fn test_multimodal_corpus_loader_functionality() {
    let loader = MultimodalCorpusLoader::global();

    // 1. Silesia loading (12 files)
    let silesia_entries = loader.load_all_silesia();
    assert_eq!(silesia_entries.len(), 12);
    for entry in &silesia_entries {
        assert!(!entry.name.is_empty());
        assert!(entry.size_bytes > 0);
        assert!(entry.shannon_entropy >= 0.0 && entry.shannon_entropy <= 8.0);
        assert_eq!(entry.data.len(), entry.size_bytes);
    }

    // 2. Fat Mach-O archive loading
    let macho_entry = loader.load_macho_vendor_archive(Some(64 * 1024));
    assert!(macho_entry.size_bytes > 0);
    assert!(macho_entry.size_bytes <= 64 * 1024);

    // 3. Test PDF loading
    let pdf_entry = loader.load_test_pdf(Some(64 * 1024));
    assert!(pdf_entry.size_bytes > 0);
    assert!(pdf_entry.size_bytes <= 64 * 1024);

    // 4. 4K image samples loading
    let img_entries = loader.load_image_samples(5);
    assert!(!img_entries.is_empty());
    for img in &img_entries {
        assert!(img.size_bytes > 0);
    }
}

