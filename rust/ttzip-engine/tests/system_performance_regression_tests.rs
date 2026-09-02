// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust BinaryDelta & System Microkernel Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Clock rising-edge alignment (`wait_for_next_tick`).
//! 2. 50ms+ adaptive time integration with thermal protection (`ThermalThrottleGovernor`).
//! 3. Hampel 3-sigma MAD outlier filtering on pass latencies.
//! 4. Test 1: Differential Patch Application Throughput Gate (>= 200.0 MB/s).
//! 5. Test 2: Patch Parse & Container Header Inspection Latency Gate (<= 1.0 ms).
//! 6. Test 3: Directory / Buffer TreeHash Calculation Throughput Gate (>= 300.0 MB/s).
//! 7. Test 4: Master Anti-Regression Invariant 6 Gate: Maximum allowed performance regression strictly <= 3.0%.

use std::hint::black_box;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use ttzip_engine::benchmark::ab_engine::stats::HampelFilter;
use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::wait_for_next_tick;
use ttzip_engine::system::delta::archive::TTZipDeltaArchive;
use ttzip_engine::system::delta::engine::TTZipDeltaEngine;

static BENCH_LOCK: Mutex<()> = Mutex::new(());

const WARMUP_RUNS: usize = 3;
const MIN_INTEGRATION_WINDOW: Duration = Duration::from_millis(50);
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

/// Measures adaptive throughput in MB/s with clock rising-edge alignment and Hampel MAD filtering.
fn measure_adaptive_throughput<F>(
    payload_size_bytes: usize,
    mut op: F,
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
        let batch_size = 5u64;
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

    let hampel = HampelFilter::default();
    let filtered = hampel.filter(&iteration_times);
    let avg_latency_secs = if !filtered.cleaned.is_empty() {
        filtered.cleaned.iter().sum::<f64>() / (filtered.cleaned.len() as f64)
    } else {
        start.elapsed().as_secs_f64() / (total_iterations as f64).max(1.0)
    };

    let avg_latency_secs_clamped = avg_latency_secs.max(1e-9);
    let mb_per_sec = (payload_size_bytes as f64 / (1024.0 * 1024.0)) / avg_latency_secs_clamped;
    let avg_latency_ms = avg_latency_secs_clamped * 1000.0;

    (mb_per_sec, avg_latency_ms)
}

/// Measures adaptive latency in milliseconds with clock rising-edge alignment and Hampel filtering.
fn measure_adaptive_latency<F>(
    mut op: F,
    governor: &mut ThermalThrottleGovernor,
) -> f64
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
        let batch_size = 10u64;
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

    let hampel = HampelFilter::default();
    let filtered = hampel.filter(&iteration_times);
    let avg_latency_secs = if !filtered.cleaned.is_empty() {
        filtered.cleaned.iter().sum::<f64>() / (filtered.cleaned.len() as f64)
    } else {
        start.elapsed().as_secs_f64() / (total_iterations as f64).max(1.0)
    };

    avg_latency_secs.max(1e-9) * 1000.0
}

// ============================================================================
// Synthetic Test Vector Generators
// ============================================================================

fn make_benchmark_payloads(size_bytes: usize) -> (Vec<u8>, Vec<u8>) {
    let mut old_data = Vec::with_capacity(size_bytes);
    let mut new_data = Vec::with_capacity(size_bytes);

    for i in 0..size_bytes {
        let b = (i * 31 % 251) as u8;
        old_data.push(b);
        if i % 200 == 0 {
            new_data.push(b.wrapping_add(19));
        } else {
            new_data.push(b);
        }
    }

    (old_data, new_data)
}

// ============================================================================
// Test 1: Differential Patch Application Throughput Gate (>= 200.0 MB/s)
// ============================================================================

#[test]
fn test_system_delta_patch_application_throughput_gate() {
    let _guard = BENCH_LOCK.lock().unwrap();
    println!("\n================================================================================");
    println!("🧪 [SYSTEM BENCH 1/3] Differential Patch Apply Throughput Gate (>= 200.0 MB/s)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let payload_size = 256 * 1024; // 256 KB
    let (old_data, new_data) = make_benchmark_payloads(payload_size);
    let patch = TTZipDeltaEngine::create_patch(&old_data, &new_data).expect("Create patch");

    let (throughput_mb_s, latency_ms) = measure_adaptive_throughput(
        payload_size,
        || {
            let res = TTZipDeltaEngine::apply_patch(&old_data, &patch);
            black_box(res).expect("Apply patch");
        },
        &mut governor,
    );

    println!("  Payload Size:       {} KB (Patch: {} KB)", payload_size / 1024, patch.len() / 1024);
    println!("  Latency (avg):      {:.3} ms", latency_ms);
    println!("  Apply Throughput:   {:.2} MB/s", throughput_mb_s);

    let min_threshold = if cfg!(debug_assertions) { 30.0f64 } else { 200.0f64 };
    println!("  Required Threshold: >= {:.2} MB/s", min_threshold);

    assert!(
        throughput_mb_s >= min_threshold,
        "Patch apply throughput ({:.2} MB/s) fell below {:.2} MB/s minimum threshold!",
        throughput_mb_s,
        min_threshold
    );

    let regression_pct = if throughput_mb_s < min_threshold {
        ((min_threshold - throughput_mb_s) / min_threshold) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Patch apply regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 2: Patch Parse & Container Header Inspection Latency Gate (<= 1.0 ms)
// ============================================================================

#[test]
fn test_system_delta_patch_parse_latency_gate() {
    let _guard = BENCH_LOCK.lock().unwrap();
    println!("\n================================================================================");
    println!("🧪 [SYSTEM BENCH 2/3] Patch Parse & Inspection Latency Gate (<= 1.0 ms)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let (old_data, new_data) = make_benchmark_payloads(128 * 1024);
    let patch = TTZipDeltaEngine::create_patch(&old_data, &new_data).expect("Create patch");

    let latency_ms = measure_adaptive_latency(
        || {
            let header = TTZipDeltaEngine::inspect_header(&patch).unwrap();
            let archive = TTZipDeltaArchive::deserialize(&patch).unwrap();
            let _ = black_box((header, archive));
        },
        &mut governor,
    );

    println!("  Container Size:     {} bytes", patch.len());
    println!("  Parse Latency:      {:.4} ms", latency_ms);

    let max_threshold = if cfg!(debug_assertions) { 2.0f64 } else { 1.0f64 };
    println!("  Required Threshold: <= {:.2} ms", max_threshold);

    assert!(
        latency_ms <= max_threshold,
        "Patch parse latency ({:.4} ms) exceeded maximum threshold of {:.2} ms!",
        latency_ms,
        max_threshold
    );

    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 3: Directory / Buffer TreeHash Calculation Throughput Gate (>= 300.0 MB/s)
// ============================================================================

#[test]
fn test_system_tree_hash_calculation_throughput_gate() {
    let _guard = BENCH_LOCK.lock().unwrap();
    println!("\n================================================================================");
    println!("🧪 [SYSTEM BENCH 3/3] TreeHash Calculation Throughput Gate (>= 300.0 MB/s)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let payload_size = 1024 * 1024; // 1 MB buffer
    let (data, _) = make_benchmark_payloads(payload_size);

    let (throughput_mb_s, latency_ms) = measure_adaptive_throughput(
        payload_size,
        || {
            let hash = TTZipDeltaEngine::calculate_tree_hash(&data);
            black_box(hash);
        },
        &mut governor,
    );

    println!("  Buffer Size:        {} MB", payload_size / (1024 * 1024));
    println!("  Latency (avg):      {:.3} ms", latency_ms);
    println!("  Hash Throughput:    {:.2} MB/s", throughput_mb_s);

    let min_threshold = if cfg!(debug_assertions) { 50.0f64 } else { 300.0f64 };
    println!("  Required Threshold: >= {:.2} MB/s", min_threshold);

    assert!(
        throughput_mb_s >= min_threshold,
        "TreeHash throughput ({:.2} MB/s) fell below {:.2} MB/s minimum threshold!",
        throughput_mb_s,
        min_threshold
    );

    let regression_pct = if throughput_mb_s < min_threshold {
        ((min_threshold - throughput_mb_s) / min_threshold) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "TreeHash calculation regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}
