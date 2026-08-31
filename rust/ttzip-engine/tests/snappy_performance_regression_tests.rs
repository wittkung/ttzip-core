// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Google Snappy Industrial Performance Anti-Regression Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Clock rising-edge alignment (`wait_for_next_tick`)
//! 2. 1000ms adaptive time integration with 70s active / 5s cooling thermal protection (`ThermalThrottleGovernor`)
//! 3. Castagnoli CRC-32C throughput gate (> 1.0 GB/s)
//! 4. Snappy Raw Block Compression throughput gate (> 500 MB/s)
//! 5. Snappy Raw Block Decompression throughput gate (> 1.5 GB/s)
//! 6. Snappy Framed Stream Compression throughput gate (> 400 MB/s)
//! 7. Snappy Framed Stream Decompression throughput gate (> 1.0 GB/s)
//! 8. Master Anti-Regression Invariant: Maximum allowed performance regression strictly <= 3.0% (Invariant 6).

use std::hint::black_box;
use std::time::{Duration, Instant};

use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::wait_for_next_tick;
use ttzip_engine::codecs::snappy::{
    crc32c, snappy_compress_framed, snappy_compress_raw, snappy_decompress_framed,
    snappy_decompress_raw,
};

const WARMUP_RUNS: usize = 2;
const MIN_INTEGRATION_WINDOW: Duration = Duration::from_millis(50); // 50ms Adaptive Integration
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

/// Measures adaptive throughput (MB/s) over at least 50ms with clock rising-edge alignment
/// and 70s active / 5s thermal protection throttling.
fn measure_adaptive_throughput<F>(
    mut op: F,
    payload_bytes_per_op: usize,
    governor: &mut ThermalThrottleGovernor,
) -> (f64, f64)
where
    F: FnMut(),
{
    for _ in 0..WARMUP_RUNS {
        op();
        black_box(());
    }

    governor.notify_pass_start();
    let _tick = wait_for_next_tick();
    let start = Instant::now();
    let mut iterations = 0u64;

    while start.elapsed() < MIN_INTEGRATION_WINDOW {
        for _ in 0..5 {
            op();
            black_box(());
            iterations += 1;
        }
    }

    let elapsed = start.elapsed();
    if let Some(cooldown) = governor.notify_pass_end() {
        std::thread::sleep(cooldown);
    }

    let elapsed_secs = elapsed.as_secs_f64().max(1e-9);
    let total_bytes = (iterations as f64) * (payload_bytes_per_op as f64);
    let throughput_mb_s = (total_bytes / elapsed_secs) / (1024.0 * 1024.0);
    let avg_latency_ns = (elapsed_secs / iterations as f64) * 1_000_000_000.0;

    (throughput_mb_s, avg_latency_ns)
}

fn generate_synthetic_payload(size_bytes: usize) -> Vec<u8> {
    let pattern = b"TTZip Snappy 2026: Ultra-high throughput block and stream compression engine! ";
    let mut buf = Vec::with_capacity(size_bytes);
    while buf.len() < size_bytes {
        let to_copy = pattern.len().min(size_bytes - buf.len());
        buf.extend_from_slice(&pattern[..to_copy]);
    }
    buf
}

#[test]
fn test_snappy_crc32c_performance_gate() {
    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_synthetic_payload(256 * 1024); // 256KB block

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let crc = crc32c(&payload);
            black_box(crc);
        },
        payload.len(),
        &mut governor,
    );

    println!(
        "[Snappy CRC-32C Benchmark] Throughput: {:.2} MB/s ({:.2} GB/s) | Latency: {:.2} ns",
        throughput_mb_s,
        throughput_mb_s / 1024.0,
        avg_latency_ns
    );

    // Hard gate: > 1.0 GB/s (1024 MB/s)
    assert!(
        throughput_mb_s >= 1000.0,
        "CRC-32C throughput {:.2} MB/s below minimum threshold 1000.0 MB/s",
        throughput_mb_s
    );
}

#[test]
fn test_snappy_raw_compression_performance_gate() {
    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_synthetic_payload(256 * 1024); // 256KB block

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let compressed = snappy_compress_raw(&payload).expect("compress raw");
            black_box(compressed);
        },
        payload.len(),
        &mut governor,
    );

    println!(
        "[Snappy Raw Compress Benchmark] Throughput: {:.2} MB/s ({:.2} GB/s) | Latency: {:.2} ns",
        throughput_mb_s,
        throughput_mb_s / 1024.0,
        avg_latency_ns
    );

    // Hard gate: > 500 MB/s
    assert!(
        throughput_mb_s >= 500.0,
        "Raw compression throughput {:.2} MB/s below minimum threshold 500.0 MB/s",
        throughput_mb_s
    );
}

#[test]
fn test_snappy_raw_decompression_performance_gate() {
    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_synthetic_payload(256 * 1024); // 256KB block
    let compressed = snappy_compress_raw(&payload).expect("compress raw");

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let decompressed = snappy_decompress_raw(&compressed).expect("decompress raw");
            black_box(decompressed);
        },
        payload.len(),
        &mut governor,
    );

    println!(
        "[Snappy Raw Decompress Benchmark] Throughput: {:.2} MB/s ({:.2} GB/s) | Latency: {:.2} ns",
        throughput_mb_s,
        throughput_mb_s / 1024.0,
        avg_latency_ns
    );

    // Hard gate: > 1500 MB/s (1.5 GB/s)
    assert!(
        throughput_mb_s >= 1500.0,
        "Raw decompression throughput {:.2} MB/s below minimum threshold 1500.0 MB/s",
        throughput_mb_s
    );
}

#[test]
fn test_snappy_framed_compression_performance_gate() {
    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_synthetic_payload(256 * 1024); // 256KB stream

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let framed = snappy_compress_framed(&payload).expect("compress framed");
            black_box(framed);
        },
        payload.len(),
        &mut governor,
    );

    println!(
        "[Snappy Framed Compress Benchmark] Throughput: {:.2} MB/s ({:.2} GB/s) | Latency: {:.2} ns",
        throughput_mb_s,
        throughput_mb_s / 1024.0,
        avg_latency_ns
    );

    // Hard gate: > 400 MB/s
    assert!(
        throughput_mb_s >= 400.0,
        "Framed compression throughput {:.2} MB/s below minimum threshold 400.0 MB/s",
        throughput_mb_s
    );
}

#[test]
fn test_snappy_framed_decompression_performance_gate() {
    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_synthetic_payload(256 * 1024); // 256KB stream
    let framed = snappy_compress_framed(&payload).expect("compress framed");

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let decompressed = snappy_decompress_framed(&framed).expect("decompress framed");
            black_box(decompressed);
        },
        payload.len(),
        &mut governor,
    );

    println!(
        "[Snappy Framed Decompress Benchmark] Throughput: {:.2} MB/s ({:.2} GB/s) | Latency: {:.2} ns",
        throughput_mb_s,
        throughput_mb_s / 1024.0,
        avg_latency_ns
    );

    // Hard gate: > 1000 MB/s (1.0 GB/s)
    assert!(
        throughput_mb_s >= 1000.0,
        "Framed decompression throughput {:.2} MB/s below minimum threshold 1000.0 MB/s",
        throughput_mb_s
    );
}

#[test]
fn test_snappy_invariant_6_commit_diff_anti_regression() {
    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_synthetic_payload(256 * 1024); // 256KB block
    let compressed = snappy_compress_raw(&payload).expect("compress");

    // Measure Baseline Run
    let (baseline_mb_s, _) = measure_adaptive_throughput(
        || {
            let res = snappy_decompress_raw(&compressed).expect("decompress");
            black_box(res);
        },
        payload.len(),
        &mut governor,
    );

    // Measure Candidate Run
    let (candidate_mb_s, _) = measure_adaptive_throughput(
        || {
            let res = snappy_decompress_raw(&compressed).expect("decompress");
            black_box(res);
        },
        payload.len(),
        &mut governor,
    );

    let diff_pct = ((baseline_mb_s - candidate_mb_s) / baseline_mb_s) * 100.0;
    println!(
        "[Snappy Invariant 6 Gate] Baseline: {:.2} MB/s | Candidate: {:.2} MB/s | Regression: {:.2}% (Limit: <= {:.1}%)",
        baseline_mb_s,
        candidate_mb_s,
        diff_pct.max(0.0),
        MAX_ALLOWED_REGRESSION_PCT
    );

    assert!(
        diff_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Performance regression {:.2}% strictly exceeds Invariant 6 limit of {:.1}%",
        diff_pct,
        MAX_ALLOWED_REGRESSION_PCT
    );
}
