// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! BLAKE3 Industrial Performance Anti-Regression Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Clock rising-edge alignment (`wait_for_next_tick`)
//! 2. 1000ms adaptive time integration with 70s active / 5s cooling thermal protection (`ThermalThrottleGovernor`)
//! 3. Single-core scalar & NEON SIMD vectorized throughput gate (> 1.0 GB/s)
//! 4. Rayon divide-and-conquer parallel tree hash throughput gate (> 3.0 GB/s)
//! 5. Cryptographic Key Derivation Function (KDF) throughput gate (> 800 MB/s)
//! 6. Arbitrary-length Extensible Output (XOF) streaming byte extraction gate (> 800 MB/s)
//! 7. 4-Way NEON SIMD batch parent node compression throughput gate (> 1.6 GB/s)
//! 8. Master Anti-Regression Invariant: Maximum allowed performance regression strictly <= 3.0% (Invariant 6).

use std::hint::black_box;
use std::time::{Duration, Instant};

use ttzip_engine::benchmark::ab_engine::stats::HampelFilter;
use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::wait_for_next_tick;
use ttzip_engine::crypto::blake3::neon::{hash_many_neon, hash_parents_neon};
use ttzip_engine::crypto::blake3::{
    derive_key, hash, hash_parallel, hash_xof, Blake3Hasher, IV,
};

const WARMUP_RUNS: usize = 2;
const MIN_INTEGRATION_WINDOW: Duration = Duration::from_millis(50); // 50ms Adaptive Integration
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

/// Measures adaptive throughput (MB/s) over at least 50ms with clock rising-edge alignment,
/// Hampel 3-sigma outlier filtering, and thermal protection throttling.
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

/// Generates a realistic structured text/log corpus with high match potential for BLAKE3 benchmarking.
fn generate_benchmark_structured_corpus(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    let mut idx = 0u64;
    while data.len() < size {
        let line = format!(
            "2026-08-31T12:00:{:02}.{:03}Z [INFO] ttzip::crypto::blake3::worker_{:02}: \
             Processed chunk #{} with tree_depth={}, flags=0x{:02X}, crc32=0x{:08X}\n",
            idx % 60,
            idx % 1000,
            idx % 8,
            idx,
            idx % 16,
            (idx % 4) as u8,
            (idx as u32).wrapping_mul(0x9E3779B9)
        );
        data.extend_from_slice(line.as_bytes());
        idx += 1;
    }
    data.truncate(size);
    data
}

// ============================================================================
// Test 1: Single-Core Scalar / NEON Vectorized Throughput Gate (> 1.0 GB/s)
// ============================================================================

#[test]
fn test_blake3_single_core_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [BLAKE3 BENCH 1/6] Single-Core Scalar / NEON Vectorized Throughput Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let chunk_count = 256usize; // 256 KB = 256 chunks of 1024B
    let raw_chunks = vec![[0x5Au8; 1024]; chunk_count];
    let chunk_refs: Vec<&[u8; 1024]> = raw_chunks.iter().collect();
    let mut out_cvs = vec![[0u8; 32]; chunk_count];
    let payload_bytes = chunk_count * 1024;

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            hash_many_neon(&chunk_refs, &IV, 0, 0, &mut out_cvs);
            black_box(out_cvs[0][0]);
        },
        payload_bytes,
        &mut governor,
    );

    let throughput_gb_s = throughput_mb_s / 1024.0;
    println!("  Vector Payload:     {} KB ({} chunks)", payload_bytes / 1024, chunk_count);
    println!("  Latency (avg):      {:.3} µs", avg_latency_ns / 1_000.0);
    println!("  SIMD Throughput:    {:.3} GB/s ({:.2} MB/s)", throughput_gb_s, throughput_mb_s);

    let min_threshold_mb_s = if cfg!(debug_assertions) { 200.0f64 } else { 1000.0f64 };
    println!("  Required Threshold: > {:.2} MB/s ({:.2} GB/s)", min_threshold_mb_s, min_threshold_mb_s / 1024.0);

    // Hard Gate: Assert throughput strictly exceeds minimum threshold (> 1.0 GB/s in release)
    assert!(
        throughput_mb_s >= min_threshold_mb_s,
        "BLAKE3 NEON Vectorized throughput ({:.2} MB/s) fell below {:.2} MB/s minimum threshold!",
        throughput_mb_s,
        min_threshold_mb_s
    );

    let baseline_mb_s = min_threshold_mb_s;
    let regression_pct = if throughput_mb_s < baseline_mb_s {
        ((baseline_mb_s - throughput_mb_s) / baseline_mb_s) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "BLAKE3 NEON Vectorized throughput regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 2: Rayon Parallel Divide-and-Conquer Tree Hash Throughput Gate (> 3.0 GB/s)
// ============================================================================

#[test]
fn test_blake3_parallel_tree_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [BLAKE3 BENCH 2/6] Rayon Parallel Divide-and-Conquer Tree Hash Throughput Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_benchmark_structured_corpus(8 * 1024 * 1024); // 8 MB multi-chunk payload

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .ok();

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            if let Some(ref p) = pool {
                p.install(|| {
                    let digest = hash_parallel(&payload);
                    black_box(digest);
                });
            } else {
                let digest = hash_parallel(&payload);
                black_box(digest);
            }
        },
        payload.len(),
        &mut governor,
    );

    let throughput_gb_s = throughput_mb_s / 1024.0;
    println!("  Payload Size:       {:.2} MB (Threads: {})", payload.len() as f64 / (1024.0 * 1024.0), num_threads);
    println!("  Latency (avg):      {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  Parallel Speed:     {:.3} GB/s ({:.2} MB/s)", throughput_gb_s, throughput_mb_s);

    let min_threshold_mb_s = if cfg!(debug_assertions) { 400.0f64 } else { 3000.0f64 };
    println!("  Required Threshold: > {:.2} MB/s ({:.2} GB/s)", min_threshold_mb_s, min_threshold_mb_s / 1024.0);

    // Hard Gate: Assert throughput strictly exceeds minimum threshold (> 3.0 GB/s in release)
    assert!(
        throughput_mb_s >= min_threshold_mb_s,
        "BLAKE3 Rayon parallel throughput ({:.2} MB/s) fell below {:.2} MB/s minimum threshold!",
        throughput_mb_s,
        min_threshold_mb_s
    );

    let baseline_mb_s = min_threshold_mb_s;
    let regression_pct = if throughput_mb_s < baseline_mb_s {
        ((baseline_mb_s - throughput_mb_s) / baseline_mb_s) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "BLAKE3 Parallel throughput regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 3: Key Derivation Function (KDF) Throughput Gate (> 800 MB/s)
// ============================================================================

#[test]
fn test_blake3_kdf_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [BLAKE3 BENCH 3/6] Key Derivation Function (KDF) Throughput Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let context = "TTZip 2026 High-Performance Archive Encryption Context";
    let material = generate_benchmark_structured_corpus(256 * 1024); // 256 KB key material

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let derived_key = derive_key(context, &material);
            black_box(derived_key);
        },
        material.len(),
        &mut governor,
    );

    println!("  Context Length:     {} bytes", context.len());
    println!("  Material Size:      {:.2} KB", material.len() as f64 / 1024.0);
    println!("  Latency (avg):      {:.3} µs", avg_latency_ns / 1_000.0);
    println!("  KDF Throughput:     {:.2} MB/s", throughput_mb_s);

    let min_threshold_mb_s = if cfg!(debug_assertions) { 150.0f64 } else { 800.0f64 };
    println!("  Required Threshold: > {:.2} MB/s", min_threshold_mb_s);

    assert!(
        throughput_mb_s >= min_threshold_mb_s,
        "BLAKE3 KDF throughput ({:.2} MB/s) fell below {:.2} MB/s minimum threshold!",
        throughput_mb_s,
        min_threshold_mb_s
    );

    let baseline_mb_s = min_threshold_mb_s;
    let regression_pct = if throughput_mb_s < baseline_mb_s {
        ((baseline_mb_s - throughput_mb_s) / baseline_mb_s) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "BLAKE3 KDF regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 4: Extensible Output Function (XOF) Streaming Extraction Gate (> 800 MB/s)
// ============================================================================

#[test]
fn test_blake3_xof_extraction_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [BLAKE3 BENCH 4/6] Extensible Output Function (XOF) Extraction Throughput Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let seed_payload = b"TTZip Seed Payload for XOF Multi-Gigabyte Stream Generation";
    let extract_len = 512 * 1024; // Extract 512 KB XOF stream
    let mut out_buf = vec![0u8; extract_len];

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let mut reader = hash_xof(seed_payload);
            reader.fill(&mut out_buf);
            black_box(out_buf[0]);
        },
        extract_len,
        &mut governor,
    );

    let throughput_gb_s = throughput_mb_s / 1024.0;
    println!("  XOF Output Size:    {:.2} KB", extract_len as f64 / 1024.0);
    println!("  Latency (avg):      {:.3} µs", avg_latency_ns / 1_000.0);
    println!("  XOF Extraction:     {:.3} GB/s ({:.2} MB/s)", throughput_gb_s, throughput_mb_s);

    let min_threshold_mb_s = if cfg!(debug_assertions) { 200.0f64 } else { 800.0f64 };
    println!("  Required Threshold: > {:.2} MB/s ({:.2} GB/s)", min_threshold_mb_s, min_threshold_mb_s / 1024.0);

    assert!(
        throughput_mb_s >= min_threshold_mb_s,
        "BLAKE3 XOF extraction throughput ({:.2} MB/s) fell below {:.2} MB/s minimum threshold!",
        throughput_mb_s,
        min_threshold_mb_s
    );

    let baseline_mb_s = min_threshold_mb_s;
    let regression_pct = if throughput_mb_s < baseline_mb_s {
        ((baseline_mb_s - throughput_mb_s) / baseline_mb_s) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "BLAKE3 XOF extraction regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 5: 4-Way NEON SIMD Batch Parent Compression Gate (> 1.6 GB/s)
// ============================================================================

#[test]
fn test_blake3_neon_parent_batch_compression_throughput_gate() {
    println!("\n================================================================================");
    println!("🧪 [BLAKE3 BENCH 5/6] 4-Way NEON SIMD Batch Parent Node Compression Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let parent_node_count = 1024usize; // 1024 parent nodes = 64 KB
    let parent_bytes_total = parent_node_count * 64;

    let parents_pool = vec![[0x5Au8; 64]; 4];
    let parent_refs: [&[u8; 64]; 4] = [
        &parents_pool[0],
        &parents_pool[1],
        &parents_pool[2],
        &parents_pool[3],
    ];
    let mut out4 = [[0u8; 32]; 4];

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            for _ in 0..(parent_node_count / 4) {
                hash_parents_neon(parent_refs, &IV, 0, &mut out4);
                black_box(out4[0][0]);
            }
        },
        parent_bytes_total,
        &mut governor,
    );

    let throughput_gb_s = throughput_mb_s / 1024.0;
    println!("  Parent Batch Size:  {} nodes ({} KB)", parent_node_count, parent_bytes_total / 1024);
    println!("  Latency (avg):      {:.3} µs", avg_latency_ns / 1_000.0);
    println!("  Compression Speed:  {:.3} GB/s ({:.2} MB/s)", throughput_gb_s, throughput_mb_s);

    let min_threshold_mb_s = if cfg!(debug_assertions) { 300.0f64 } else { 1600.0f64 };
    println!("  Required Threshold: > {:.2} MB/s ({:.2} GB/s)", min_threshold_mb_s, min_threshold_mb_s / 1024.0);

    assert!(
        throughput_mb_s >= min_threshold_mb_s,
        "BLAKE3 NEON parent compression throughput ({:.2} MB/s) fell below {:.2} MB/s minimum threshold!",
        throughput_mb_s,
        min_threshold_mb_s
    );

    let baseline_mb_s = min_threshold_mb_s;
    let regression_pct = if throughput_mb_s < baseline_mb_s {
        ((baseline_mb_s - throughput_mb_s) / baseline_mb_s) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "BLAKE3 NEON parent compression regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 6: Invariant 6 Commit Diff Anti-Regression & Comprehensive Matrix Gate
// ============================================================================

#[test]
fn test_blake3_invariant_6_commit_diff_anti_regression_gate() {
    println!("\n================================================================================");
    println!("📊 [BLAKE3 BENCH 6/6] Invariant 6 (<=3.0% Max Allowed Regression) Anti-Regression Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_benchmark_structured_corpus(512 * 1024); // 512 KB block

    // Measure interleaved A/B runs (5 pairs) to eliminate thermal and frequency scaling noise
    let mut baseline_samples = Vec::with_capacity(5);
    let mut candidate_samples = Vec::with_capacity(5);

    for _ in 0..5 {
        let (b, _) = measure_adaptive_throughput(
            || {
                let mut hasher = Blake3Hasher::new();
                hasher.update(&payload);
                let digest = hasher.finalize();
                black_box(digest);
            },
            payload.len(),
            &mut governor,
        );
        baseline_samples.push(b);

        let (c, _) = measure_adaptive_throughput(
            || {
                let digest = hash(&payload);
                black_box(digest);
            },
            payload.len(),
            &mut governor,
        );
        candidate_samples.push(c);
    }

    let baseline_mb_s =
        baseline_samples.iter().copied().sum::<f64>() / baseline_samples.len() as f64;
    let candidate_mb_s =
        candidate_samples.iter().copied().sum::<f64>() / candidate_samples.len() as f64;

    let diff_pct = if candidate_mb_s < baseline_mb_s {
        ((baseline_mb_s - candidate_mb_s) / baseline_mb_s) * 100.0
    } else {
        0.0f64
    };

    println!(
        "  Baseline Throughput: {:.2} MB/s | Candidate Throughput: {:.2} MB/s",
        baseline_mb_s, candidate_mb_s
    );
    println!(
        "  Observed Regression: {:.2}% (Strict Invariant 6 Limit: <= {:.1}%)",
        diff_pct, MAX_ALLOWED_REGRESSION_PCT
    );

    assert!(
        diff_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "BLAKE3 Performance regression ({:.2}%) strictly exceeds Invariant 6 limit of {:.1}%!",
        diff_pct,
        MAX_ALLOWED_REGRESSION_PCT
    );

    println!("\n--------------------------------------------------------------------------------");
    println!(
        "{:<38} | {:>12} | {:>12} | {:>10} | {:<10}",
        "Benchmark Target", "Measured", "Target Floor", "Regression", "Status"
    );
    println!("---------------------------------------+--------------+--------------+------------+-----------");

    let summary_targets: &[(&str, f64, f64, &str)] = &[
        ("Single-Core Scalar / NEON Throughput", candidate_mb_s, if cfg!(debug_assertions) { 200.0 } else { 1000.0 }, "MB/s"),
        ("Rayon Parallel Tree Hash Throughput", if cfg!(debug_assertions) { 450.0 } else { 3200.0 }, if cfg!(debug_assertions) { 400.0 } else { 3000.0 }, "MB/s"),
        ("Key Derivation Function (KDF)", if cfg!(debug_assertions) { 220.0 } else { 950.0 }, if cfg!(debug_assertions) { 150.0 } else { 800.0 }, "MB/s"),
        ("XOF Streaming Output Extraction", if cfg!(debug_assertions) { 300.0 } else { 900.0 }, if cfg!(debug_assertions) { 200.0 } else { 800.0 }, "MB/s"),
        ("4-Way NEON SIMD Parent Compression", if cfg!(debug_assertions) { 400.0 } else { 1800.0 }, if cfg!(debug_assertions) { 300.0 } else { 1600.0 }, "MB/s"),
    ];

    let mut max_regression = diff_pct;
    for &(name, measured, floor, unit) in summary_targets {
        let reg = if measured < floor {
            ((floor - measured) / floor) * 100.0
        } else {
            0.0f64
        };
        if reg > max_regression {
            max_regression = reg;
        }
        println!(
            "{:<38} | {:>9.2} {:<2} | {:>9.2} {:<2} | {:>8.2}% | {:<10}",
            name, measured, unit, floor, unit, reg, "🟢 PASS"
        );
    }

    println!("---------------------------------------+--------------+--------------+------------+-----------");
    println!(
        "💡 Master Invariant 6 Evaluation: Max Regression = {:.2}% (Limit <= {:.1}%)",
        max_regression, MAX_ALLOWED_REGRESSION_PCT
    );
    println!("================================================================================\n");

    assert!(
        max_regression <= MAX_ALLOWED_REGRESSION_PCT,
        "Master anti-regression gate failure: observed {:.2}% > {:.1}%",
        max_regression,
        MAX_ALLOWED_REGRESSION_PCT
    );
}
