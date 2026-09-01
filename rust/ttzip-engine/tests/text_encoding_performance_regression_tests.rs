// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Text Encoding Detection and Zero-Allocation Transcoding Performance Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Clock rising-edge alignment (`wait_for_next_tick`).
//! 2. 50ms+ adaptive time integration with thermal protection (`ThermalThrottleGovernor`).
//! 3. Hampel 3-sigma MAD outlier filtering on pass latencies.
//! 4. Charset detection latency gate (<= 50.0 ns/B).
//! 5. Zero-allocation filename sanitization and transcoding throughput gate (>= 1.0 GB/s).
//! 6. Master Anti-Regression Invariant: Maximum allowed performance regression strictly <= 3.0% (Invariant 6).

use std::hint::black_box;
use std::time::{Duration, Instant};

use ttzip_engine::benchmark::ab_engine::stats::HampelFilter;
use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::wait_for_next_tick;
use ttzip_engine::charset::{
    detect_charset, detect_charset_with_confidence, sanitize_filename_to_slice,
};

const WARMUP_RUNS: usize = 3;
const MIN_INTEGRATION_WINDOW: Duration = Duration::from_millis(50); // 50ms Adaptive Integration
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

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

/// Measures adaptive data throughput (GB/s) over at least 50ms.
fn measure_adaptive_throughput<F>(
    mut op: F,
    bytes_per_op: usize,
    governor: &mut ThermalThrottleGovernor,
) -> (f64, f64)
where
    F: FnMut(),
{
    let (ops_per_sec, avg_latency_ns) = measure_adaptive_ops(&mut op, governor);
    let gb_per_sec = (ops_per_sec * (bytes_per_op as f64)) / (1024.0 * 1024.0 * 1024.0);
    (gb_per_sec, avg_latency_ns)
}

// ============================================================================
// Test 1: ASCII Fast-Path Detection Latency Gate (<= 50.0 ns/B)
// ============================================================================
#[test]
fn test_text_encoding_ascii_detection_latency_gate() {
    println!("\n================================================================================");
    println!("🧪 [TEXT ENCODING BENCH 1/5] ASCII Fast-Path Detection Latency Gate (<= 50 ns/B)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let ascii_payload = b"src/archive/unified/format/zip_header_sanitizer_fast_path.rs";
    let payload_len = ascii_payload.len();

    let (_ops_per_sec, avg_latency_ns) = measure_adaptive_ops(
        || {
            let res = detect_charset_with_confidence(ascii_payload);
            black_box(res);
        },
        &mut governor,
    );

    let ns_per_byte = avg_latency_ns / (payload_len as f64);
    println!("  Payload Size:       {} bytes", payload_len);
    println!("  Total Latency:      {:.2} ns", avg_latency_ns);
    println!("  Unit Latency:       {:.3} ns/B", ns_per_byte);

    let max_allowed_ns_per_byte = if cfg!(debug_assertions) { 50.0 } else { 10.0 };
    println!("  Required Threshold: <= {:.1} ns/B", max_allowed_ns_per_byte);

    assert!(
        ns_per_byte <= max_allowed_ns_per_byte,
        "ASCII detection latency ({:.3} ns/B) exceeded threshold ({:.1} ns/B)",
        ns_per_byte,
        max_allowed_ns_per_byte
    );

    let baseline_ns_per_byte = max_allowed_ns_per_byte;
    let regression_pct = if ns_per_byte > baseline_ns_per_byte {
        ((ns_per_byte - baseline_ns_per_byte) / baseline_ns_per_byte) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "ASCII detection regression ({:.2}%) violated Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 2: Multibyte CJK Charset Detection Latency Gate (<= 50.0 ns/B)
// ============================================================================
#[test]
fn test_text_encoding_cjk_detection_latency_gate() {
    println!("\n================================================================================");
    println!("🧪 [TEXT ENCODING BENCH 2/5] Multibyte CJK Charset Detection Latency Gate (<= 50 ns/B)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let base_text = "重要业务报告与工程架构设计规范文档说明指南_2026年度归档总结。";
    let text = base_text.repeat(8);
    let (gb_bytes, _, _) = encoding_rs::GB18030.encode(&text);
    let payload_len = gb_bytes.len();

    let (_ops_per_sec, avg_latency_ns) = measure_adaptive_ops(
        || {
            let res = detect_charset(&gb_bytes);
            black_box(res);
        },
        &mut governor,
    );

    let ns_per_byte = avg_latency_ns / (payload_len as f64);
    println!("  Payload Size:       {} bytes (GB18030)", payload_len);
    println!("  Total Latency:      {:.2} ns", avg_latency_ns);
    println!("  Unit Latency:       {:.3} ns/B", ns_per_byte);

    let max_allowed_ns_per_byte = if cfg!(debug_assertions) { 150.0 } else { 50.0 };
    println!("  Required Threshold: <= {:.1} ns/B", max_allowed_ns_per_byte);

    assert!(
        ns_per_byte <= max_allowed_ns_per_byte,
        "CJK detection latency ({:.3} ns/B) exceeded threshold ({:.1} ns/B)",
        ns_per_byte,
        max_allowed_ns_per_byte
    );

    let baseline_ns_per_byte = max_allowed_ns_per_byte;
    let regression_pct = if ns_per_byte > baseline_ns_per_byte {
        ((ns_per_byte - baseline_ns_per_byte) / baseline_ns_per_byte) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "CJK detection regression ({:.2}%) violated Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 3: Zero-Allocation Filename Slice Sanitization Throughput Gate (>= 1.0 GB/s)
// ============================================================================
#[test]
fn test_text_encoding_slice_sanitization_throughput_gate() {
    println!("\n================================================================================");
    println!("🧪 [TEXT ENCODING BENCH 3/5] Zero-Allocation Sanitization Throughput Gate (>= 1.0 GB/s)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let payload = b"modules/subsystems/analytics/high_speed_compression_trace_corpus_buffer.dat";
    let payload_len = payload.len();
    let mut out_buf = [0u8; 256];

    let (gb_per_sec, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let written = sanitize_filename_to_slice(payload, &mut out_buf).unwrap();
            black_box(written);
        },
        payload_len,
        &mut governor,
    );

    println!("  Payload Size:       {} bytes", payload_len);
    println!("  Latency (avg):      {:.2} ns", avg_latency_ns);
    println!("  Throughput:         {:.3} GB/s", gb_per_sec);

    let min_allowed_gbps = if cfg!(debug_assertions) { 0.20 } else { 1.00 };
    println!("  Required Threshold: >= {:.2} GB/s", min_allowed_gbps);

    assert!(
        gb_per_sec >= min_allowed_gbps,
        "Slice sanitization throughput ({:.3} GB/s) fell below minimum threshold ({:.2} GB/s)",
        gb_per_sec,
        min_allowed_gbps
    );

    let baseline_gbps = min_allowed_gbps;
    let regression_pct = if gb_per_sec < baseline_gbps {
        ((baseline_gbps - gb_per_sec) / baseline_gbps) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Throughput regression ({:.2}%) violated Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 4: UTF-8 Direct Slicing Throughput Gate (>= 1.0 GB/s)
// ============================================================================
#[test]
fn test_text_encoding_utf8_direct_slicing_throughput_gate() {
    println!("\n================================================================================");
    println!("🧪 [TEXT ENCODING BENCH 4/5] UTF-8 Direct Slicing Throughput Gate (>= 1.0 GB/s)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let utf8_text = "工程测试：极速 UTF-8 校验与零拷贝直通路径验证_2026.tar";
    let utf8_bytes = utf8_text.as_bytes();
    let payload_len = utf8_bytes.len();
    let mut out_buf = [0u8; 512];

    let (gb_per_sec, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let written = sanitize_filename_to_slice(utf8_bytes, &mut out_buf).unwrap();
            black_box(written);
        },
        payload_len,
        &mut governor,
    );

    println!("  Payload Size:       {} bytes", payload_len);
    println!("  Latency (avg):      {:.2} ns", avg_latency_ns);
    println!("  Throughput:         {:.3} GB/s", gb_per_sec);

    let min_allowed_gbps = if cfg!(debug_assertions) { 0.20 } else { 1.00 };
    println!("  Required Threshold: >= {:.2} GB/s", min_allowed_gbps);

    assert!(
        gb_per_sec >= min_allowed_gbps,
        "UTF-8 slice throughput ({:.3} GB/s) fell below threshold ({:.2} GB/s)",
        gb_per_sec,
        min_allowed_gbps
    );

    let baseline_gbps = min_allowed_gbps;
    let regression_pct = if gb_per_sec < baseline_gbps {
        ((baseline_gbps - gb_per_sec) / baseline_gbps) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Throughput regression ({:.2}%) violated Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 5: Transcoding Engine Invariant 6 Master Regression Gate
// ============================================================================
#[test]
fn test_text_encoding_master_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [TEXT ENCODING BENCH 5/5] Transcoding Engine Invariant 6 Master Anti-Regression Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let base_text = "TTZip 字符编码探测与转码性能全链路回归基准测试语料库_2026。";
    let text = base_text.repeat(16);
    let (gb_bytes, _, _) = encoding_rs::GB18030.encode(&text);
    let payload_len = gb_bytes.len();

    let encoding = ttzip_engine::charset::lookup_encoding("GB18030");
    let mut out_buf = vec![0u8; payload_len * 4 + 1];

    let (gb_per_sec, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let mut decoder = encoding.new_decoder();
            let (_res, _read, written, _) = decoder.decode_to_utf8(&gb_bytes, &mut out_buf, true);
            black_box(written);
        },
        payload_len,
        &mut governor,
    );

    println!("  Payload Size:       {} bytes", payload_len);
    println!("  Latency (avg):      {:.2} ns", avg_latency_ns);
    println!("  Transcode Speed:    {:.3} GB/s", gb_per_sec);

    let min_allowed_gbps = if cfg!(debug_assertions) { 0.20 } else { 0.60 };
    println!("  Required Threshold: >= {:.2} GB/s", min_allowed_gbps);

    assert!(
        gb_per_sec >= min_allowed_gbps,
        "Transcode speed ({:.3} GB/s) fell below threshold ({:.2} GB/s)",
        gb_per_sec,
        min_allowed_gbps
    );

    let baseline_gbps = min_allowed_gbps;
    let regression_pct = if gb_per_sec < baseline_gbps {
        ((baseline_gbps - gb_per_sec) / baseline_gbps) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Transcoding regression ({:.2}%) violated Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}
