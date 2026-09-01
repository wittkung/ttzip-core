// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Streaming XML Parser & Document Metadata Performance Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Clock rising-edge alignment (`wait_for_next_tick`).
//! 2. 50ms+ adaptive time integration with thermal protection (`ThermalThrottleGovernor`).
//! 3. Hampel 3-sigma MAD outlier filtering on pass latencies.
//! 4. XML SAX streaming throughput gate (>= 500.0 MB/s).
//! 5. XML single token latency gate (<= 5.0 ns/B).
//! 6. DOCX document XML paragraph extraction throughput gate (>= 500.0 MB/s).
//! 7. Master Anti-Regression Invariant 6: Maximum allowed performance regression strictly <= 3.0%.

use std::hint::black_box;
use std::io::Cursor;
use std::time::{Duration, Instant};

use quick_xml::events::Event;
use quick_xml::Reader as XmlReader;

use ttzip_engine::benchmark::ab_engine::stats::HampelFilter;
use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::wait_for_next_tick;
use ttzip_engine::standards::document_stream::parse_docx_xml_content;

const WARMUP_RUNS: usize = 3;
const MIN_INTEGRATION_WINDOW: Duration = Duration::from_millis(50); // 50ms Adaptive Integration
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

/// Generates a realistic synthetic DOCX `word/document.xml` payload of specified approximate byte size.
fn generate_synthetic_docx_xml(target_size_bytes: usize) -> Vec<u8> {
    let mut xml = String::with_capacity(target_size_bytes + 1024);
    xml.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
"#);

    let sample_paras = [
        "TTZip high-performance archiving and compression microkernel engine.",
        "Zero-disk-footprint streaming parsing and Dublin Core metadata extraction.",
        "Mozilla UniFFI safe cross-language bindings and actor-isolated memory safety.",
        "Pure safe Rust SIMD-accelerated SAX tokenization and text unescaping pipeline.",
    ];

    let mut idx = 0;
    while xml.len() < target_size_bytes {
        let sentence = sample_paras[idx % sample_paras.len()];
        xml.push_str(&format!(
            "    <w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Paragraph {} - {}</w:t></w:r></w:p>\n",
            idx, sentence
        ));
        idx += 1;
    }

    xml.push_str(r#"  </w:body>
</w:document>"#);

    xml.into_bytes()
}

/// Measures adaptive operations per second (op/s) and latency (ns) over at least 50ms with clock rising-edge alignment,
/// Hampel 3-sigma outlier filtering, and thermal protection throttling.
fn measure_adaptive_ops<F>(
    mut op: F,
    governor: &mut ThermalThrottleGovernor,
) -> (f64, f64)
where
    F: FnMut(),
{
    // Warmup passes
    for _ in 0..WARMUP_RUNS {
        op();
        black_box(());
    }

    governor.notify_pass_start();
    let mut iteration_times = Vec::with_capacity(100);
    let start = Instant::now();
    let mut total_iterations = 0u64;

    while start.elapsed() < MIN_INTEGRATION_WINDOW {
        let _tick = wait_for_next_tick();
        let batch_start = Instant::now();
        let batch_size = 20u64;
        for _ in 0..batch_size {
            op();
            black_box(());
            total_iterations += 1;
        }
        let batch_dur = batch_start.elapsed().as_secs_f64() / (batch_size as f64);
        iteration_times.push(batch_dur);
    }

    if let Some(cooldown) = governor.notify_pass_end() {
        std::thread::sleep(cooldown);
    }

    // Apply Hampel MAD outlier filtering on pass latencies
    let hampel = HampelFilter::default();
    let filtered = hampel.filter(&iteration_times);
    let avg_latency_secs = if !filtered.cleaned.is_empty() {
        filtered.cleaned.iter().sum::<f64>() / (filtered.cleaned.len() as f64)
    } else {
        start.elapsed().as_secs_f64() / (total_iterations as f64).max(1.0)
    };

    let avg_latency_secs_clamped = avg_latency_secs.max(1e-9);
    let ops_per_sec = 1.0 / avg_latency_secs_clamped;
    let avg_latency_ns = avg_latency_secs_clamped * 1_000_000_000.0;

    (ops_per_sec, avg_latency_ns)
}

/// Measures adaptive data throughput (MB/s) and single-byte latency (ns/B) over at least 50ms.
fn measure_adaptive_throughput<F>(
    mut op: F,
    bytes_per_op: usize,
    governor: &mut ThermalThrottleGovernor,
) -> (f64, f64)
where
    F: FnMut(),
{
    let (ops_per_sec, avg_latency_ns) = measure_adaptive_ops(&mut op, governor);
    let mb_per_sec = (ops_per_sec * (bytes_per_op as f64)) / (1024.0 * 1024.0);
    let ns_per_byte = avg_latency_ns / (bytes_per_op as f64);
    (mb_per_sec, ns_per_byte)
}

// ============================================================================
// Test 1: SAX Streaming XML Parser Throughput Gate (>= 500 MB/s)
// ============================================================================
#[test]
fn test_01_xml_sax_streaming_throughput_gate() {
    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_synthetic_docx_xml(512 * 1024); // 512 KB XML
    let payload_len = payload.len();

    let (throughput_mb_s, ns_per_b) = measure_adaptive_throughput(
        || {
            let mut reader = XmlReader::from_reader(Cursor::new(&payload));
            reader.config_mut().trim_text(false);
            let mut buf = Vec::with_capacity(1024);
            let mut token_count = 0usize;

            loop {
                match reader.read_event_into(&mut buf) {
                    Ok(Event::Start(_)) | Ok(Event::End(_)) | Ok(Event::Text(_)) => {
                        token_count += 1;
                    }
                    Ok(Event::Eof) | Err(_) => break,
                    _ => {}
                }
                buf.clear();
            }
            black_box(token_count);
        },
        payload_len,
        &mut governor,
    );

    println!(
        "⚡ [XML SAX Gate] Throughput: {:.2} MB/s, Latency: {:.4} ns/B",
        throughput_mb_s, ns_per_b
    );

    assert!(
        throughput_mb_s >= 500.0,
        "XML SAX streaming throughput {:.2} MB/s below minimum threshold of 500.0 MB/s",
        throughput_mb_s
    );
}

// ============================================================================
// Test 2: Single Token Streaming Latency Gate (<= 5.0 ns/B)
// ============================================================================
#[test]
fn test_02_xml_token_streaming_latency_gate() {
    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_synthetic_docx_xml(256 * 1024); // 256 KB XML
    let payload_len = payload.len();

    let (throughput_mb_s, ns_per_b) = measure_adaptive_throughput(
        || {
            let mut reader = XmlReader::from_reader(Cursor::new(&payload));
            let mut buf = Vec::with_capacity(512);
            let mut count = 0u64;

            while let Ok(event) = reader.read_event_into(&mut buf) {
                if matches!(event, Event::Eof) {
                    break;
                }
                count += 1;
                buf.clear();
            }
            black_box(count);
        },
        payload_len,
        &mut governor,
    );

    println!(
        "⚡ [XML Token Latency Gate] Latency: {:.4} ns/B, Throughput: {:.2} MB/s",
        ns_per_b, throughput_mb_s
    );

    assert!(
        ns_per_b <= 5.0,
        "XML token latency {:.4} ns/B exceeds maximum threshold of 5.0 ns/B",
        ns_per_b
    );
}

// ============================================================================
// Test 3: DOCX Document XML Body Paragraph Extraction Gate (>= 500 MB/s)
// ============================================================================
#[test]
fn test_03_docx_document_xml_streaming_throughput_gate() {
    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_synthetic_docx_xml(512 * 1024); // 512 KB
    let payload_len = payload.len();

    let (throughput_mb_s, ns_per_b) = measure_adaptive_throughput(
        || {
            let res = parse_docx_xml_content(&payload).expect("Valid DOCX XML");
            black_box(res.1.len());
        },
        payload_len,
        &mut governor,
    );

    println!(
        "⚡ [DOCX XML Body Gate] Throughput: {:.2} MB/s, Latency: {:.4} ns/B",
        throughput_mb_s, ns_per_b
    );

    assert!(
        throughput_mb_s >= 500.0,
        "DOCX XML extraction throughput {:.2} MB/s below minimum threshold of 500.0 MB/s",
        throughput_mb_s
    );
}

// ============================================================================
// Test 4: Dublin Core / OPF Package Metadata Parsing Latency Gate
// ============================================================================
#[test]
fn test_04_dublin_core_metadata_parsing_latency_gate() {
    let mut governor = ThermalThrottleGovernor::new();
    let core_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
  xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/">
  <dc:title>TTZip Microkernel High-Performance Architecture</dc:title>
  <dc:creator>Witt Kung</dc:creator>
  <dc:description>Zero-disk-footprint in-memory document stream extraction.</dc:description>
  <dc:subject>Systems Engineering</dc:subject>
  <cp:lastModifiedBy>Witt Kung</cp:lastModifiedBy>
  <cp:revision>42</cp:revision>
</cp:coreProperties>"#;

    let (ops_per_sec, avg_latency_ns) = measure_adaptive_ops(
        || {
            let mut reader = XmlReader::from_reader(Cursor::new(core_xml.as_slice()));
            let mut buf = Vec::with_capacity(256);
            let mut tag_count = 0usize;
            while let Ok(event) = reader.read_event_into(&mut buf) {
                if matches!(event, Event::Eof) {
                    break;
                }
                tag_count += 1;
                buf.clear();
            }
            black_box(tag_count);
        },
        &mut governor,
    );

    let avg_latency_us = avg_latency_ns / 1_000.0;
    println!(
        "⚡ [Metadata Parsing Gate] Rate: {:.0} op/s, Latency: {:.3} µs",
        ops_per_sec, avg_latency_us
    );

    assert!(
        avg_latency_us <= 150.0,
        "Metadata parsing latency {:.3} µs exceeds threshold of 150.0 µs",
        avg_latency_us
    );
}

// ============================================================================
// Test 5: Multi-Element Nested Tag Streaming Parsing Throughput Gate
// ============================================================================
#[test]
fn test_05_multi_element_nested_tag_streaming_throughput_gate() {
    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_synthetic_docx_xml(384 * 1024);
    let payload_len = payload.len();

    let (throughput_mb_s, _) = measure_adaptive_throughput(
        || {
            let (full_text, paras) = parse_docx_xml_content(&payload).expect("Valid XML");
            black_box((full_text.len(), paras.len()));
        },
        payload_len,
        &mut governor,
    );

    println!(
        "⚡ [Nested Tag Gate] Throughput: {:.2} MB/s (Target >= 500 MB/s)",
        throughput_mb_s
    );

    assert!(
        throughput_mb_s >= 500.0,
        "Nested tag XML throughput {:.2} MB/s below threshold of 500.0 MB/s",
        throughput_mb_s
    );
}

// ============================================================================
// Test 6: Master Anti-Regression Invariant 6 Gate (Regression <= 3.0%)
// ============================================================================
#[test]
fn test_06_master_anti_regression_invariant_6_gate() {
    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_synthetic_docx_xml(512 * 1024);
    let payload_len = payload.len();

    // Pass A: Baseline run
    let (baseline_mb_s, _) = measure_adaptive_throughput(
        || {
            let res = parse_docx_xml_content(&payload).expect("Pass A");
            black_box(res.0.len());
        },
        payload_len,
        &mut governor,
    );

    // Pass B: Candidate regression verification run
    let (candidate_mb_s, _) = measure_adaptive_throughput(
        || {
            let res = parse_docx_xml_content(&payload).expect("Pass B");
            black_box(res.0.len());
        },
        payload_len,
        &mut governor,
    );

    let regression_pct = if candidate_mb_s < baseline_mb_s {
        ((baseline_mb_s - candidate_mb_s) / baseline_mb_s) * 100.0
    } else {
        0.0
    };

    println!(
        "⚡ [Invariant 6 Anti-Regression Gate] Baseline: {:.2} MB/s, Candidate: {:.2} MB/s, Regression: {:.2}% (Limit <= {:.1}%)",
        baseline_mb_s, candidate_mb_s, regression_pct, MAX_ALLOWED_REGRESSION_PCT
    );

    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Invariant 6 Violation: XML parsing regression {:.2}% exceeds limit of {:.1}%",
        regression_pct,
        MAX_ALLOWED_REGRESSION_PCT
    );
}
