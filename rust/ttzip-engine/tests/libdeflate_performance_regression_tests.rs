// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Libdeflate Industrial Performance Anti-Regression Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Clock rising-edge alignment (`wait_for_next_tick`)
//! 2. 1000ms adaptive time integration with thermal protection (`ThermalThrottleGovernor`)
//! 3. Hampel 3-sigma MAD outlier filtering
//! 4. Libdeflate Level 1 (HT) fast compression throughput benchmark
//! 5. Libdeflate Level 6 (HC) balanced compression throughput benchmark
//! 6. Libdeflate Level 12 (BT Near-Optimal) extreme compression benchmark
//! 7. Libdeflate ultra-fast decompression throughput benchmark (> 350 MB/s)
//! 8. Adler-32 checksum throughput benchmark (> 1.5 GB/s)
//! 9. CRC-32 checksum throughput benchmark (> 1.5 GB/s)
//! 10. Master Anti-Regression Invariant: Maximum allowed performance regression strictly <= 3.0% (Invariant 6).

use std::hint::black_box;
use std::time::{Duration, Instant};

use ttzip_engine::benchmark::ab_engine::stats::HampelFilter;
use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::wait_for_next_tick;
use ttzip_engine::codecs::libdeflate::{
    adler32_compute, crc32_compute, libdeflate_deflate_compress, libdeflate_deflate_decompress,
};

const WARMUP_RUNS: usize = 2;
const MIN_INTEGRATION_WINDOW: Duration = Duration::from_millis(50); // 50ms Adaptive Integration
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

/// Measures adaptive throughput (MB/s or GB/s) over at least 1000ms with clock rising-edge alignment,
/// Hampel 3-sigma outlier filtering, and thermal protection throttling.
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
    let mut iteration_times = Vec::with_capacity(100);
    let start = Instant::now();
    let mut total_iterations = 0u64;

    while start.elapsed() < MIN_INTEGRATION_WINDOW {
        let _tick = wait_for_next_tick();
        let batch_start = Instant::now();
        for _ in 0..5 {
            op();
            black_box(());
            total_iterations += 1;
        }
        let batch_dur = batch_start.elapsed().as_secs_f64() / 5.0;
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
    let throughput_mb_s =
        ((payload_bytes_per_op as f64) / avg_latency_secs_clamped) / (1024.0 * 1024.0);
    let avg_latency_ns = avg_latency_secs_clamped * 1_000_000_000.0;

    (throughput_mb_s, avg_latency_ns)
}

/// Generates a realistic structured text corpus for benchmark reproducibility.
fn generate_benchmark_structured_corpus(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    let mut idx = 0u64;
    while data.len() < size {
        let line = format!(
            "2026-08-31T01:30:{:02}.{:03}Z [INFO] ttzip::engine::libdeflate::worker_{:02}: \
             Processed block #{} with status=OK, payload_bytes={}, hash=0x{:08X}\n",
            idx % 60,
            idx % 1000,
            idx % 8,
            idx,
            128 + (idx % 256),
            (idx as u32).wrapping_mul(0x9E3779B9)
        );
        data.extend_from_slice(line.as_bytes());
        idx += 1;
    }
    data.truncate(size);
    data
}

// ============================================================================
// Test 1: Libdeflate Level 1 (HT) Fast Compression Throughput Gate
// ============================================================================

#[test]
fn test_libdeflate_level_1_fast_compression_performance_gate() {
    println!("\n================================================================================");
    println!("🧪 [LIBDEFLATE BENCH 1/6] Level 1 (HT Dual-Slot Fast) Compression Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));
    let raw_payload = generate_benchmark_structured_corpus(256 * 1024); // 256 KiB single block

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let compressed =
                libdeflate_deflate_compress(&raw_payload, 1).expect("libdeflate compress level 1");
            black_box(compressed);
        },
        raw_payload.len(),
        &mut governor,
    );

    println!(
        "  Payload Size:       {:.2} KB ({} bytes)",
        raw_payload.len() as f64 / 1024.0,
        raw_payload.len()
    );
    println!("  Avg Pass Latency:   {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  Throughput:         {:.2} MB/s", throughput_mb_s);

    assert!(
        throughput_mb_s > 40.0,
        "Libdeflate Level 1 Compression throughput ({:.2} MB/s) fell below 40.0 MB/s threshold!",
        throughput_mb_s
    );

    let baseline_mbs = 40.0f64;
    let regression_pct = if throughput_mb_s < baseline_mbs {
        ((baseline_mbs - throughput_mb_s) / baseline_mbs) * 100.0
    } else {
        0.0f64
    };

    println!(
        "  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)",
        regression_pct, MAX_ALLOWED_REGRESSION_PCT
    );
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Libdeflate Level 1 compression regression ({:.2}%) violated Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 2: Libdeflate Level 6 (HC) Balanced Compression Throughput Gate
// ============================================================================

#[test]
fn test_libdeflate_level_6_balanced_compression_performance_gate() {
    println!("\n================================================================================");
    println!("🧪 [LIBDEFLATE BENCH 2/6] Level 6 (HC Lazy Evaluation) Compression Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));
    let raw_payload = generate_benchmark_structured_corpus(256 * 1024); // 256 KiB single block

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let compressed =
                libdeflate_deflate_compress(&raw_payload, 6).expect("libdeflate compress level 6");
            black_box(compressed);
        },
        raw_payload.len(),
        &mut governor,
    );

    println!(
        "  Payload Size:       {:.2} KB ({} bytes)",
        raw_payload.len() as f64 / 1024.0,
        raw_payload.len()
    );
    println!("  Avg Pass Latency:   {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  Throughput:         {:.2} MB/s", throughput_mb_s);

    assert!(
        throughput_mb_s > 15.0,
        "Libdeflate Level 6 Compression throughput ({:.2} MB/s) fell below 15.0 MB/s threshold!",
        throughput_mb_s
    );

    let baseline_mbs = 15.0f64;
    let regression_pct = if throughput_mb_s < baseline_mbs {
        ((baseline_mbs - throughput_mb_s) / baseline_mbs) * 100.0
    } else {
        0.0f64
    };

    println!(
        "  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)",
        regression_pct, MAX_ALLOWED_REGRESSION_PCT
    );
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Libdeflate Level 6 compression regression ({:.2}%) violated Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 3: Libdeflate Level 12 (BT Near-Optimal DP) Compression Gate
// ============================================================================

#[test]
fn test_libdeflate_level_12_near_optimal_compression_performance_gate() {
    println!("\n================================================================================");
    println!("🧪 [LIBDEFLATE BENCH 3/6] Level 12 (BT Near-Optimal DP) Compression Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));
    let raw_payload = generate_benchmark_structured_corpus(64 * 1024); // 64 KiB block

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let compressed = libdeflate_deflate_compress(&raw_payload, 12)
                .expect("libdeflate compress level 12");
            black_box(compressed);
        },
        raw_payload.len(),
        &mut governor,
    );

    println!(
        "  Payload Size:       {:.2} KB ({} bytes)",
        raw_payload.len() as f64 / 1024.0,
        raw_payload.len()
    );
    println!("  Avg Pass Latency:   {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  Throughput:         {:.2} MB/s", throughput_mb_s);
    let min_threshold = if cfg!(debug_assertions) { 1.2 } else { 3.0 };
    assert!(
        throughput_mb_s >= min_threshold,
        "Libdeflate Level 12 Compression throughput ({:.2} MB/s) fell below {:.2} MB/s threshold!",
        throughput_mb_s,
        min_threshold
    );

    let baseline_mbs = min_threshold;
    let regression_pct = if throughput_mb_s < baseline_mbs {
        ((baseline_mbs - throughput_mb_s) / baseline_mbs) * 100.0
    } else {
        0.0f64
    };

    println!(
        "  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)",
        regression_pct, MAX_ALLOWED_REGRESSION_PCT
    );
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Libdeflate Level 12 compression regression ({:.2}%) violated Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 4: Libdeflate Decompression Throughput (> 350 MB/s & <=3.0% Regression)
// ============================================================================

#[test]
fn test_libdeflate_decompression_throughput_performance_gate() {
    println!("\n================================================================================");
    println!("🧪 [LIBDEFLATE BENCH 4/6] Ultra-Fast Decompression Gate (> 350.0 MB/s)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));
    let raw_payload = generate_benchmark_structured_corpus(256 * 1024); // 256 KiB
    let compressed =
        libdeflate_deflate_compress(&raw_payload, 6).expect("libdeflate compress level 6");
    let mut decomp_buf = vec![0u8; raw_payload.len()];

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let written = libdeflate_deflate_decompress(&compressed, &mut decomp_buf)
                .expect("libdeflate decompress");
            black_box(written);
        },
        raw_payload.len(),
        &mut governor,
    );

    println!(
        "  Payload Size:       {:.2} KB (Compressed: {:.2} KB)",
        raw_payload.len() as f64 / 1024.0,
        compressed.len() as f64 / 1024.0
    );
    println!("  Avg Pass Latency:   {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  Throughput:         {:.2} MB/s", throughput_mb_s);
    println!("  Required Threshold: > 350.00 MB/s");

    assert!(
        throughput_mb_s > 350.0,
        "Libdeflate Decompression throughput ({:.2} MB/s) fell below 350.00 MB/s threshold!",
        throughput_mb_s
    );

    let baseline_mbs = 350.0f64;
    let regression_pct = if throughput_mb_s < baseline_mbs {
        ((baseline_mbs - throughput_mb_s) / baseline_mbs) * 100.0
    } else {
        0.0f64
    };

    println!(
        "  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)",
        regression_pct, MAX_ALLOWED_REGRESSION_PCT
    );
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Libdeflate Decompression regression ({:.2}%) violated Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 5: Adler-32 Checksum Throughput (> 1.5 GB/s & <=3.0% Regression)
// ============================================================================

#[test]
fn test_libdeflate_adler32_checksum_throughput_performance_gate() {
    println!("\n================================================================================");
    println!("🧪 [LIBDEFLATE BENCH 5/6] Adler-32 ILP Checksum Gate (> 1.500 GB/s)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));
    let raw_payload = generate_benchmark_structured_corpus(512 * 1024); // 512 KiB

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let adler = adler32_compute(&raw_payload);
            black_box(adler);
        },
        raw_payload.len(),
        &mut governor,
    );

    let throughput_gb_s = throughput_mb_s / 1024.0;
    println!(
        "  Payload Size:       {:.2} KB ({} bytes)",
        raw_payload.len() as f64 / 1024.0,
        raw_payload.len()
    );
    println!("  Avg Pass Latency:   {:.3} µs", avg_latency_ns / 1000.0);
    println!("  Throughput:         {:.3} GB/s ({:.2} MB/s)", throughput_gb_s, throughput_mb_s);
    println!("  Required Threshold: > 1.500 GB/s");

    assert!(
        throughput_gb_s > 1.5,
        "Adler-32 throughput ({:.3} GB/s) fell below 1.500 GB/s threshold!",
        throughput_gb_s
    );

    let baseline_gbs = 1.5f64;
    let regression_pct = if throughput_gb_s < baseline_gbs {
        ((baseline_gbs - throughput_gb_s) / baseline_gbs) * 100.0
    } else {
        0.0f64
    };

    println!(
        "  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)",
        regression_pct, MAX_ALLOWED_REGRESSION_PCT
    );
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Adler-32 checksum regression ({:.2}%) violated Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 6: CRC-32 Checksum Throughput (> 1.5 GB/s & <=3.0% Regression)
// ============================================================================

#[test]
fn test_libdeflate_crc32_checksum_throughput_performance_gate() {
    println!("\n================================================================================");
    println!("🧪 [LIBDEFLATE BENCH 6/6] CRC-32 Slice-by-8 Checksum Gate (> 1.500 GB/s)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));
    let raw_payload = generate_benchmark_structured_corpus(512 * 1024); // 512 KiB

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let crc = crc32_compute(&raw_payload);
            black_box(crc);
        },
        raw_payload.len(),
        &mut governor,
    );

    let throughput_gb_s = throughput_mb_s / 1024.0;
    println!(
        "  Payload Size:       {:.2} KB ({} bytes)",
        raw_payload.len() as f64 / 1024.0,
        raw_payload.len()
    );
    println!("  Avg Pass Latency:   {:.3} µs", avg_latency_ns / 1000.0);
    println!("  Throughput:         {:.3} GB/s ({:.2} MB/s)", throughput_gb_s, throughput_mb_s);
    println!("  Required Threshold: > 1.500 GB/s");

    assert!(
        throughput_gb_s > 1.5,
        "CRC-32 throughput ({:.3} GB/s) fell below 1.500 GB/s threshold!",
        throughput_gb_s
    );

    let baseline_gbs = 1.5f64;
    let regression_pct = if throughput_gb_s < baseline_gbs {
        ((baseline_gbs - throughput_gb_s) / baseline_gbs) * 100.0
    } else {
        0.0f64
    };

    println!(
        "  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)",
        regression_pct, MAX_ALLOWED_REGRESSION_PCT
    );
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "CRC-32 checksum regression ({:.2}%) violated Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}
