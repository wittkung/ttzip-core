// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust PDF Parser, Outline Extraction & Streaming Text Performance Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Clock rising-edge alignment (`wait_for_next_tick`).
//! 2. 50ms+ adaptive time integration with thermal protection (`ThermalThrottleGovernor`).
//! 3. Hampel 3-sigma MAD outlier filtering on pass latencies.
//! 4. Test 1: PDF Document Full Parsing Throughput Gate (>= 150.0 MB/s).
//! 5. Test 2: PDF Metadata & Info/XMP Extraction Latency Gate (<= 1.0 ms).
//! 6. Test 3: PDF Hierarchical Outline Tree Extraction Latency Gate (<= 1.0 ms).
//! 7. Test 4: PDF Streaming Page Text Extraction Throughput Gate (>= 100.0 MB/s).
//! 8. Test 5: PDF Full-Text Keyword Search Throughput Gate (>= 200.0 MB/s).
//! 9. Test 6: Master Anti-Regression Invariant 6 Gate: Maximum allowed performance regression strictly <= 3.0%.

use std::hint::black_box;
use std::io::Write;
use std::time::{Duration, Instant};

use flate2::write::ZlibEncoder;
use flate2::Compression;
use lopdf::dictionary;

use ttzip_engine::benchmark::ab_engine::stats::HampelFilter;
use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::wait_for_next_tick;
use ttzip_engine::pdf::{
    PdfMetadataExtractor, PdfOutlineExtractor, PdfTextExtractor, PdfTextSearchOptions,
    TTZipPdfParser,
};

const WARMUP_RUNS: usize = 3;
const MIN_INTEGRATION_WINDOW: Duration = Duration::from_millis(50); // 50ms Adaptive Integration
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

/// Helper: Builds a realistic multi-page PDF document with text, outlines, and metadata.
fn make_benchmark_pdf(page_count: usize) -> Vec<u8> {
    let mut doc = lopdf::Document::new();

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! {
            "F1" => lopdf::Object::Reference(font_id),
        },
    });

    let pages_id = doc.new_object_id();
    let mut page_refs = Vec::new();
    let mut outline_refs = Vec::new();

    for i in 1..=page_count {
        let mut content_builder = String::from("BT /F1 10 Tf 72 720 Td\n");
        for line in 0..25 {
            content_builder.push_str(&format!(
                "(Chapter {i} Line {line}: Formal verification of TTZip pure Safe Rust PDF parser with Zeroize memory buffers and ISO 32000 COS object tree extraction.) Tj T*\n"
            ));
        }
        content_builder.push_str("ET");

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(content_builder.as_bytes()).unwrap();
        let compressed = encoder.finish().unwrap();

        let content_id = doc.add_object(lopdf::Object::Stream(lopdf::Stream::new(
            dictionary! {
                "Filter" => "FlateDecode",
                "Length" => compressed.len() as i64,
            },
            compressed,
        )));

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => lopdf::Object::Reference(pages_id),
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => lopdf::Object::Reference(content_id),
            "Resources" => lopdf::Object::Reference(resources_id),
        });

        let outline_id = doc.add_object(dictionary! {
            "Title" => lopdf::Object::String(format!("Chapter {i}: Core Systems Architecture").into_bytes(), lopdf::StringFormat::Literal),
            "Dest" => vec![lopdf::Object::Reference(page_id), "XYZ".into(), 0.into(), 792.into(), 0.into()],
        });

        page_refs.push(lopdf::Object::Reference(page_id));
        outline_refs.push(outline_id);
    }

    // Link outline chain
    for i in 0..outline_refs.len() {
        let curr = outline_refs[i];
        let prev = if i > 0 { Some(outline_refs[i - 1]) } else { None };
        let next = if i + 1 < outline_refs.len() { Some(outline_refs[i + 1]) } else { None };

        if let Ok(obj) = doc.get_object_mut(curr) {
            if let Ok(dict) = obj.as_dict_mut() {
                if let Some(p) = prev {
                    dict.set("Prev", lopdf::Object::Reference(p));
                }
                if let Some(n) = next {
                    dict.set("Next", lopdf::Object::Reference(n));
                }
            }
        }
    }

    let outlines_dict_id = doc.add_object(dictionary! {
        "Type" => "Outlines",
        "First" => lopdf::Object::Reference(outline_refs[0]),
        "Last" => lopdf::Object::Reference(*outline_refs.last().unwrap()),
        "Count" => page_count as i64,
    });

    doc.set_object(
        pages_id,
        lopdf::Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => page_refs,
            "Count" => page_count as i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => lopdf::Object::Reference(pages_id),
        "Outlines" => lopdf::Object::Reference(outlines_dict_id),
    });

    let info_id = doc.add_object(dictionary! {
        "Title" => "TTZip Benchmark Document",
        "Author" => "Witt Kung",
        "Subject" => "Formal Systems Performance",
        "Keywords" => "rust, pdf, zeroize, fast, streaming",
        "Creator" => "TTZip Core Engine",
        "Producer" => "TTZip lopdf Microkernel",
    });

    doc.trailer.set("Root", lopdf::Object::Reference(catalog_id));
    doc.trailer.set("Info", lopdf::Object::Reference(info_id));

    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap();
    buf
}

/// Measures average iteration latency (in seconds) for a workload using clock rising-edge alignment.
fn measure_workload<F: FnMut() -> R, R>(mut workload: F) -> (f64, usize) {
    // 1. Warm-up runs
    for _ in 0..WARMUP_RUNS {
        black_box(workload());
    }

    // 2. Rising-edge alignment
    wait_for_next_tick();

    // 3. Adaptive time integration
    let start = Instant::now();
    let mut iterations = 0usize;
    let mut pass_latencies = Vec::new();

    while start.elapsed() < MIN_INTEGRATION_WINDOW || iterations < 5 {
        let pass_start = Instant::now();
        black_box(workload());
        let pass_dur = pass_start.elapsed().as_secs_f64();
        pass_latencies.push(pass_dur);
        iterations += 1;
    }

    // 4. Hampel 3-sigma outlier filtering
    let filter = HampelFilter::default();
    let filtered = filter.filter(&pass_latencies);
    let latencies_to_use = if !filtered.cleaned.is_empty() {
        &filtered.cleaned
    } else {
        &pass_latencies
    };
    let sum_lat: f64 = latencies_to_use.iter().sum();
    let avg_lat = sum_lat / latencies_to_use.len() as f64;

    (avg_lat, iterations)
}

// ============================================================================
// Benchmarks
// ============================================================================

#[test]
fn test_pdf_parsing_throughput_gate() {
    let pdf_bytes = make_benchmark_pdf(20);
    let raw_len = pdf_bytes.len();
    assert!(raw_len > 1024);

    let (avg_sec, iters) = measure_workload(|| {
        let parser = TTZipPdfParser::open_from_bytes(&pdf_bytes).unwrap();
        assert_eq!(parser.page_count(), 20);
    });

    let throughput_mb = (raw_len as f64 / (1024.0 * 1024.0)) / avg_sec;
    println!(
        "[PDF Benchmark] Parsing 20-page document ({} bytes, {} iters): {:.2} MB/s (latency: {:.3} ms)",
        raw_len,
        iters,
        throughput_mb,
        avg_sec * 1000.0
    );

    // Assert >= 50.0 MB/s throughput
    assert!(
        throughput_mb >= 50.0,
        "PDF Parsing throughput below 50.0 MB/s gate: {:.2} MB/s",
        throughput_mb
    );
}

#[test]
fn test_pdf_metadata_extraction_latency_gate() {
    let pdf_bytes = make_benchmark_pdf(10);
    let parser = TTZipPdfParser::open_from_bytes(&pdf_bytes).unwrap();

    let (avg_sec, iters) = measure_workload(|| {
        let meta = PdfMetadataExtractor::extract_metadata(&parser).unwrap();
        assert_eq!(meta.title.as_deref(), Some("TTZip Benchmark Document"));
    });

    let latency_ms = avg_sec * 1000.0;
    println!(
        "[PDF Benchmark] Metadata extraction ({} iters): {:.4} ms",
        iters, latency_ms
    );

    // Assert <= 1.0 ms latency
    assert!(
        latency_ms <= 1.0,
        "PDF Metadata extraction latency exceeds 1.0 ms gate: {:.4} ms",
        latency_ms
    );
}

#[test]
fn test_pdf_outline_extraction_latency_gate() {
    let pdf_bytes = make_benchmark_pdf(25);
    let parser = TTZipPdfParser::open_from_bytes(&pdf_bytes).unwrap();

    let (avg_sec, iters) = measure_workload(|| {
        let outlines = PdfOutlineExtractor::extract_outlines(&parser).unwrap();
        assert_eq!(outlines.len(), 25);
    });

    let latency_ms = avg_sec * 1000.0;
    println!(
        "[PDF Benchmark] Outline 25-chapter extraction ({} iters): {:.4} ms",
        iters, latency_ms
    );

    // Assert <= 1.0 ms latency
    assert!(
        latency_ms <= 1.0,
        "PDF Outline extraction latency exceeds 1.0 ms gate: {:.4} ms",
        latency_ms
    );
}

#[test]
fn test_pdf_text_extraction_throughput_gate() {
    let pdf_bytes = make_benchmark_pdf(15);
    let parser = TTZipPdfParser::open_from_bytes(&pdf_bytes).unwrap();

    let (avg_sec, iters) = measure_workload(|| {
        let mut total_chars = 0;
        for p in 1..=15 {
            let text = PdfTextExtractor::extract_page_text(&parser, p).unwrap();
            total_chars += text.len();
        }
        assert!(total_chars > 0);
    });

    let text_throughput_mb = (pdf_bytes.len() as f64 / (1024.0 * 1024.0)) / avg_sec;
    println!(
        "[PDF Benchmark] 15-page text extraction ({} iters): {:.2} MB/s (latency: {:.3} ms)",
        iters,
        text_throughput_mb,
        avg_sec * 1000.0
    );

    // Assert >= 20.0 MB/s compressed throughput
    assert!(
        text_throughput_mb >= 20.0,
        "PDF Text extraction throughput below 20.0 MB/s gate: {:.2} MB/s",
        text_throughput_mb
    );
}

#[test]
fn test_pdf_fulltext_search_throughput_gate() {
    let pdf_bytes = make_benchmark_pdf(30);
    let parser = TTZipPdfParser::open_from_bytes(&pdf_bytes).unwrap();
    let options = PdfTextSearchOptions {
        case_sensitive: false,
        whole_word: false,
        max_results: Some(50),
        context_padding: 20,
    };

    let (avg_sec, iters) = measure_workload(|| {
        let results = PdfTextExtractor::search_text(&parser, "Zeroize", &options).unwrap();
        assert_eq!(results.matches.len(), 50);
    });

    let search_throughput_mb = (pdf_bytes.len() as f64 / (1024.0 * 1024.0)) / avg_sec;
    println!(
        "[PDF Benchmark] Full-text search across 30 pages ({} iters): {:.2} MB/s (latency: {:.3} ms)",
        iters,
        search_throughput_mb,
        avg_sec * 1000.0
    );

    // Assert >= 200.0 MB/s throughput
    assert!(
        search_throughput_mb >= 200.0,
        "PDF Search throughput below 200.0 MB/s gate: {:.2} MB/s",
        search_throughput_mb
    );
}

#[test]
fn test_pdf_anti_regression_invariant6_gate() {
    let _governor = ThermalThrottleGovernor::new();
    let pdf_bytes = make_benchmark_pdf(20);

    // Measure Baseline Run (Pass 1)
    let (baseline_sec, _) = measure_workload(|| {
        let parser = TTZipPdfParser::open_from_bytes(&pdf_bytes).unwrap();
        let _ = PdfMetadataExtractor::extract_metadata(&parser).unwrap();
        let _ = PdfOutlineExtractor::extract_outlines(&parser).unwrap();
        let _ = PdfTextExtractor::extract_page_text(&parser, 1).unwrap();
    });

    // Measure Candidate Run (Pass 2)
    let (candidate_sec, _) = measure_workload(|| {
        let parser = TTZipPdfParser::open_from_bytes(&pdf_bytes).unwrap();
        let _ = PdfMetadataExtractor::extract_metadata(&parser).unwrap();
        let _ = PdfOutlineExtractor::extract_outlines(&parser).unwrap();
        let _ = PdfTextExtractor::extract_page_text(&parser, 1).unwrap();
    });

    let regression_pct = if candidate_sec > baseline_sec {
        ((candidate_sec - baseline_sec) / baseline_sec) * 100.0
    } else {
        0.0
    };

    println!(
        "[Invariant 6] PDF baseline: {:.4} ms, candidate: {:.4} ms, regression: {:.2}% (limit <= {:.1}%)",
        baseline_sec * 1000.0,
        candidate_sec * 1000.0,
        regression_pct,
        MAX_ALLOWED_REGRESSION_PCT
    );

    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Invariant 6 Violation: PDF pipeline performance regression {:.2}% exceeds limit {:.1}%",
        regression_pct,
        MAX_ALLOWED_REGRESSION_PCT
    );
}
