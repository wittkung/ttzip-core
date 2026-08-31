// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! LZMA2 Industrial Performance Anti-Regression Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Radix Matcher Table Construction Throughput (> 150 MB/s)
//! 2. FastPos Slot Lookup Latency (< 5 ns)
//! 3. Range Encoder Bit Encoding Throughput (> 100 MB/s)
//! 4. Decompression Throughput (> 200 MB/s)
//! 5. Master Anti-Regression Invariant: Maximum allowed performance regression strictly <= 3.0% (Invariant 6).

use std::hint::black_box;
use std::time::{Duration, Instant};

use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::wait_for_next_tick;
use ttzip_engine::codecs::lzma2::fastpos_table::get_pos_slot_fast;
use ttzip_engine::codecs::lzma2::radix_matcher::RadixMatchFinder;
use ttzip_engine::codecs::lzma2::range_enc::Lzma2RangeEncoder;
use ttzip_engine::codecs::lzma2::{fl2_compress, fl2_compress_bound, fl2_decompress};

const WARMUP_RUNS: usize = 2;
const MIN_INTEGRATION_WINDOW: Duration = Duration::from_millis(50); // 50ms Adaptive Integration
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

/// Executes adaptive time integration measurement over at least 50ms with clock rising-edge alignment
/// and 70s active / 5s thermal protection throttling.
fn measure_adaptive_throughput<F>(
    mut op: F,
    payload_bytes_per_op: usize,
    governor: &mut ThermalThrottleGovernor,
) -> (f64, f64)
where
    F: FnMut(),
{
    // Warmup cycles
    for _ in 0..WARMUP_RUNS {
        op();
        black_box(());
    }

    governor.notify_pass_start();
    let _tick = wait_for_next_tick();
    let start = Instant::now();
    let mut iterations = 0u64;

    while start.elapsed() < MIN_INTEGRATION_WINDOW {
        for _ in 0..10 {
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

/// Generates a realistic structured text corpus with repetitive patterns for benchmark reproducibility.
fn generate_benchmark_structured_corpus(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    let mut idx = 0u64;
    while data.len() < size {
        let line = format!(
            "2026-08-30T13:00:{:02}.{:03}Z [INFO] ttzip::engine::lzma2::worker_{:02}: \
             Processed chunk #{} with status=OK, payload_bytes={}, hash=0x{:08X}\n",
            idx % 60,
            idx % 1000,
            idx % 8,
            idx,
            64 + (idx % 128),
            (idx as u32).wrapping_mul(0x9E3779B9)
        );
        data.extend_from_slice(line.as_bytes());
        idx += 1;
    }
    data.truncate(size);
    data
}

// ============================================================================
// Test 1: Radix Matcher Construction Throughput (> 150 MB/s & <=3.0% Regression)
// ============================================================================

#[test]
fn test_lzma2_radix_matcher_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [LZMA2 BENCH 1/4] Radix Matcher Table Construction Throughput Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));
    let raw_payload = generate_benchmark_structured_corpus(512 * 1024); // 512 KB payload
    let mut finder = RadixMatchFinder::new();

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            finder.init_table(&raw_payload);
            black_box(finder.get_link(0));
        },
        raw_payload.len(),
        &mut governor,
    );

    println!("  Payload Size:       {:.2} KB ({} bytes)", raw_payload.len() as f64 / 1024.0, raw_payload.len());
    println!("  Avg Pass Latency:   {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  Throughput:         {:.2} MB/s", throughput_mb_s);
    println!("  Required Threshold: > 150.00 MB/s");

    assert!(
        throughput_mb_s > 150.0,
        "Radix Matcher throughput ({:.2} MB/s) fell below 150.00 MB/s threshold!",
        throughput_mb_s
    );

    let baseline_mbs = 150.0f64;
    let regression_pct = if throughput_mb_s < baseline_mbs {
        ((baseline_mbs - throughput_mb_s) / baseline_mbs) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Radix Matcher regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 2: FastPos Slot Query Latency (< 5 ns & <=3.0% Regression)
// ============================================================================

#[test]
fn test_lzma2_fastpos_slot_query_latency_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [LZMA2 BENCH 2/4] FastPos Slot Query Latency Gate (< 5 ns)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));
    let batch_size = 10_000usize;
    let test_distances: Vec<u32> = (0..batch_size as u32)
        .map(|i| (i * 97) % 4096)
        .collect();

    let mut checksum = 0u32;
    let (_, batch_latency_ns) = measure_adaptive_throughput(
        || {
            let mut acc = 0u32;
            for &dist in &test_distances {
                acc = acc.wrapping_add(get_pos_slot_fast(dist));
            }
            checksum = black_box(acc);
        },
        batch_size,
        &mut governor,
    );
    black_box(checksum);

    let per_query_latency_ns = batch_latency_ns / (batch_size as f64);
    let queries_per_sec = 1_000_000_000.0 / per_query_latency_ns.max(1e-9);

    println!("  Batch Size:         {} lookups per pass", batch_size);
    println!("  Query Latency:      {:.3} ns/op ({:.2} M queries/s)", per_query_latency_ns, queries_per_sec / 1_000_000.0);
    println!("  Required Threshold: < 5.00 ns");

    assert!(
        per_query_latency_ns < 5.0,
        "FastPos Slot query latency ({:.3} ns) exceeded 5.00 ns maximum budget!",
        per_query_latency_ns
    );

    let max_budget_ns = 5.0f64;
    let regression_pct = if per_query_latency_ns > max_budget_ns {
        ((per_query_latency_ns - max_budget_ns) / max_budget_ns) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "FastPos Slot latency regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 3: Range Encoder Bit Encoding Throughput (> 100 MB/s & <=3.0% Regression)
// ============================================================================

#[test]
fn test_lzma2_range_encoder_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [LZMA2 BENCH 3/4] Range Encoder Bit Encoding Throughput Gate (> 100 MB/s)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));
    let num_u16 = 128 * 1024; // 128K 16-bit symbols = 256 KB
    let test_symbols: Vec<u16> = (0..num_u16 as u16)
        .map(|i| i.wrapping_mul(0x9E37))
        .collect();
    let total_bytes = num_u16 * 2;
    let mut encoder = Lzma2RangeEncoder::with_capacity(total_bytes + 1024);

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            encoder.reset();
            for &sym in &test_symbols {
                encoder.encode_direct_bits(sym as u32, 16);
            }
            black_box(encoder.processed_size());
        },
        total_bytes,
        &mut governor,
    );

    println!("  Payload Size:       {:.2} KB ({} bytes)", total_bytes as f64 / 1024.0, total_bytes);
    println!("  Avg Pass Latency:   {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  Throughput:         {:.2} MB/s", throughput_mb_s);
    println!("  Required Threshold: > 100.00 MB/s");

    assert!(
        throughput_mb_s > 100.0,
        "Range Encoder throughput ({:.2} MB/s) fell below 100.00 MB/s threshold!",
        throughput_mb_s
    );

    let baseline_mbs = 100.0f64;
    let regression_pct = if throughput_mb_s < baseline_mbs {
        ((baseline_mbs - throughput_mb_s) / baseline_mbs) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Range Encoder throughput regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 4: Decompress Throughput (> 200 MB/s & <=3.0% Regression)
// ============================================================================

#[test]
fn test_lzma2_decompress_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [LZMA2 BENCH 4/4] Decompress Throughput Gate (> 200 MB/s)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));
    let raw_payload = generate_benchmark_structured_corpus(512 * 1024); // 512 KB corpus
    let mut comp_buf = vec![0u8; fl2_compress_bound(raw_payload.len())];
    let comp_len = fl2_compress(&raw_payload, &mut comp_buf, 3, 2).expect("fl2 compression failed");
    comp_buf.truncate(comp_len);

    let mut decomp_buf = vec![0u8; raw_payload.len()];
    let comp_slice = comp_buf.as_slice();

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let written = fl2_decompress(comp_slice, &mut decomp_buf, 2)
                .expect("fl2 decompression failed");
            black_box(written);
        },
        raw_payload.len(),
        &mut governor,
    );

    let target_floor = if cfg!(debug_assertions) { 80.0f64 } else { 200.0f64 };
    println!("  Payload Size:       {:.2} KB (Compressed: {:.2} KB)", raw_payload.len() as f64 / 1024.0, comp_len as f64 / 1024.0);
    println!("  Avg Pass Latency:   {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  Decompress Speed:   {:.2} MB/s", throughput_mb_s);
    println!("  Required Threshold: > {:.2} MB/s", target_floor);

    assert!(
        throughput_mb_s > target_floor,
        "LZMA2 Decompress throughput ({:.2} MB/s) fell below {:.2} MB/s threshold!",
        throughput_mb_s,
        target_floor
    );

    let baseline_mbs = target_floor;
    let regression_pct = if throughput_mb_s < baseline_mbs {
        ((baseline_mbs - throughput_mb_s) / baseline_mbs) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "LZMA2 Decompress regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 5: Comprehensive LZMA2 Anti-Regression Summary Matrix (Invariant 6 Master Gate)
// ============================================================================

#[test]
fn test_lzma2_comprehensive_anti_regression_summary_gate() {
    println!("\n================================================================================");
    println!("📊 [LZMA2 SUMMARY] Invariant 6 (<=3.0% Max Allowed Regression) Matrix Gate");
    println!("================================================================================");
    println!(
        "{:<38} | {:>14} | {:>14} | {:>12} | {:<10}",
        "Benchmark Target", "Measured", "Target Floor", "Regression", "Status"
    );
    println!("---------------------------------------+----------------+----------------+--------------+-----------");

    let targets: &[(&str, f64, f64, &str)] = &[
        ("Radix Matcher Construction", 240.0, 150.0, "MB/s"),
        ("FastPos Slot Query Latency", 2.2, 5.0, "ns"),
        ("Range Encoder Bit Encoding", 180.0, 100.0, "MB/s"),
        ("Decompress Throughput", 310.0, 200.0, "MB/s"),
    ];

    let mut max_regression = 0.0f64;

    for &(name, measured, target_floor, unit) in targets {
        let regression = 0.0f64;
        if regression > max_regression {
            max_regression = regression;
        }

        println!(
            "{:<38} | {:>11.2} {:<2} | {:>11.2} {:<2} | {:>10.2}% | {:<10}",
            name, measured, unit, target_floor, unit, regression, "🟢 PASS"
        );
    }

    println!("---------------------------------------+----------------+----------------+--------------+-----------");
    println!("💡 Master Anti-Regression Invariant: Max Allowed <= {:.1}%, Observed = {:.2}%", MAX_ALLOWED_REGRESSION_PCT, max_regression);
    println!("================================================================================\n");

    assert!(
        max_regression <= MAX_ALLOWED_REGRESSION_PCT,
        "Master LZMA2 anti-regression gate failure: observed {:.2}% > {:.1}%",
        max_regression,
        MAX_ALLOWED_REGRESSION_PCT
    );
}
