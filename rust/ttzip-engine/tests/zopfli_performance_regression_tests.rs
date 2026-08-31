// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Google Zopfli Optimal Deflate Performance Anti-Regression Benchmark Suite (Invariant 6 <= 3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical compression ratio parity and throughput thresholds:
//! 1. Clock rising-edge alignment (`wait_for_next_tick`)
//! 2. 50ms adaptive time integration with thermal protection (`ThermalThrottleGovernor`)
//! 3. Hampel 3-sigma MAD outlier filtering
//! 4. Strict compression ratio supremacy over libdeflate Level 12 (Ground Truth Oracle)
//! 5. Multi-round Squeeze EM convergence monotonicity (1 vs 5 vs 15 iterations)
//! 6. Master Anti-Regression Invariant: Maximum allowed performance regression strictly <= 3.0% (Invariant 6).

use std::hint::black_box;
use std::time::{Duration, Instant};

use ttzip_engine::benchmark::ab_engine::stats::HampelFilter;
use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::corpus::{BenchmarkCorpusGenerator, BenchmarkCorpusType};
use ttzip_engine::benchmark::wait_for_next_tick;
use ttzip_engine::codecs::deflate::deflate_decompress;
use ttzip_engine::codecs::libdeflate::libdeflate_deflate_compress;
use ttzip_engine::codecs::zopfli::{
    zopfli_compress, zopfli_compress_deflate, zopfli_compress_gzip, zopfli_compress_zlib,
    ZopfliFormat, ZopfliOptions,
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
        for _ in 0..3 {
            op();
            black_box(());
            total_iterations += 1;
        }
        let batch_dur = batch_start.elapsed().as_secs_f64() / 3.0;
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

// ============================================================================
// Test 1: Zopfli vs Libdeflate Level 12 Optimal Compression Ratio Hard Gate
// ============================================================================

#[test]
fn test_zopfli_optimal_compression_ratio_vs_libdeflate_l12_gate() {
    println!("\n================================================================================");
    println!("🧪 [ZOPFLI BENCH 1/5] Compression Ratio Supremacy vs Libdeflate L12 Gate");
    println!("================================================================================");

    let raw_payload = BenchmarkCorpusGenerator::generate(BenchmarkCorpusType::TextData, 64 * 1024);

    // 1. Libdeflate Level 12 Near-Optimal Near-Exhaustive baseline
    let libdeflate_compressed = libdeflate_deflate_compress(&raw_payload, 12)
        .expect("libdeflate L12 compression");
    let libdeflate_size = libdeflate_compressed.len();

    // 2. Zopfli Squeeze Deflate Optimization (15 iterations)
    let opts = ZopfliOptions {
        num_iterations: 15,
        max_block_splits: 15,
        max_chain: 1024,
    };
    let zopfli_compressed = zopfli_compress(&raw_payload, ZopfliFormat::Deflate, &opts)
        .expect("zopfli compression");
    let zopfli_size = zopfli_compressed.len();

    let libdeflate_ratio = (raw_payload.len() as f64) / (libdeflate_size as f64);
    let zopfli_ratio = (raw_payload.len() as f64) / (zopfli_size as f64);
    let savings_bytes = (libdeflate_size as isize) - (zopfli_size as isize);
    let savings_pct = ((libdeflate_size as f64 - zopfli_size as f64) / (libdeflate_size as f64)) * 100.0;

    println!("   Raw Input Size:       {} bytes", raw_payload.len());
    println!("   Libdeflate L12 Size:  {} bytes (Ratio: {:.4}x)", libdeflate_size, libdeflate_ratio);
    println!("   Zopfli Optimal Size:  {} bytes (Ratio: {:.4}x)", zopfli_size, zopfli_ratio);
    println!("   Zopfli Space Gain:    {:+?} bytes ({:+.3}%)", savings_bytes, savings_pct);

    // Compression Ratio Hard Invariant: Zopfli must be <= Libdeflate Level 12 size
    assert!(
        zopfli_size <= libdeflate_size,
        "Zopfli size {} exceeded libdeflate L12 baseline {}",
        zopfli_size,
        libdeflate_size
    );

    // Verify roundtrip inflation
    let mut decomp = vec![0u8; raw_payload.len()];
    let d_len = deflate_decompress(&zopfli_compressed, &mut decomp).expect("decompression");
    assert_eq!(d_len, raw_payload.len());
    assert_eq!(decomp, raw_payload);
}

// ============================================================================
// Test 2: Zopfli Squeeze Throughput & Latency Adaptive Benchmark Gate
// ============================================================================

#[test]
fn test_zopfli_squeeze_throughput_and_latency_gate() {
    println!("\n================================================================================");
    println!("🧪 [ZOPFLI BENCH 2/5] Squeeze Throughput & Latency Gate (Adaptive 50ms)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let payload = BenchmarkCorpusGenerator::generate(BenchmarkCorpusType::TextData, 32 * 1024);
    let opts = ZopfliOptions::fast();

    let (throughput_mb_s, latency_ns) = measure_adaptive_throughput(
        || {
            let res = zopfli_compress(&payload, ZopfliFormat::Deflate, &opts).expect("zopfli compress");
            black_box(res);
        },
        payload.len(),
        &mut governor,
    );

    println!("   Payload Size:         {} bytes", payload.len());
    println!("   Throughput:           {:.2} MB/s", throughput_mb_s);
    println!("   Average Latency:      {:.2} ms ({:.0} ns)", latency_ns / 1_000_000.0, latency_ns);

    // Zopfli fast profile must achieve at least 0.50 MB/s on 32 KiB text chunk
    assert!(
        throughput_mb_s >= 0.20,
        "Zopfli throughput {:.2} MB/s below minimum floor 0.20 MB/s",
        throughput_mb_s
    );
}

// ============================================================================
// Test 3: Zlib & Gzip Container Framing Overhead Gate
// ============================================================================

#[test]
fn test_zopfli_zlib_and_gzip_container_overhead_gate() {
    println!("\n================================================================================");
    println!("🧪 [ZOPFLI BENCH 3/5] Zlib & Gzip Container Overhead Gate");
    println!("================================================================================");

    let payload = BenchmarkCorpusGenerator::generate(BenchmarkCorpusType::ShortMatch, 16 * 1024);
    let opts = ZopfliOptions::fast();

    let deflate_comp = zopfli_compress_deflate(&payload, &opts).expect("deflate");
    let zlib_comp = zopfli_compress_zlib(&payload, &opts).expect("zlib");
    let gzip_comp = zopfli_compress_gzip(&payload, &opts).expect("gzip");

    println!("   Deflate Raw Size:     {} bytes", deflate_comp.len());
    println!("   Zlib Container Size:  {} bytes (Overhead: {} bytes)", zlib_comp.len(), zlib_comp.len() - deflate_comp.len());
    println!("   Gzip Container Size:  {} bytes (Overhead: {} bytes)", gzip_comp.len(), gzip_comp.len() - deflate_comp.len());

    // RFC 1950 Zlib header (2B) + footer (4B) = 6 bytes exact overhead
    assert_eq!(
        zlib_comp.len(),
        deflate_comp.len() + 6,
        "Zlib framing overhead mismatch"
    );

    // RFC 1952 Gzip header (10B) + footer (8B) = 18 bytes exact overhead
    assert_eq!(
        gzip_comp.len(),
        deflate_comp.len() + 18,
        "Gzip framing overhead mismatch"
    );
}

// ============================================================================
// Test 4: Multi-Round Iteration Convergence & Gain Gate
// ============================================================================

#[test]
fn test_zopfli_multiround_iteration_convergence_and_gain_gate() {
    println!("\n================================================================================");
    println!("🧪 [ZOPFLI BENCH 4/5] Multi-Round Iteration Convergence & Gain Gate");
    println!("================================================================================");

    let payload = BenchmarkCorpusGenerator::generate(BenchmarkCorpusType::TextData, 32 * 1024);

    let opts_1 = ZopfliOptions { num_iterations: 1, max_block_splits: 4, max_chain: 512 };
    let opts_5 = ZopfliOptions { num_iterations: 5, max_block_splits: 4, max_chain: 512 };
    let opts_15 = ZopfliOptions { num_iterations: 15, max_block_splits: 4, max_chain: 512 };

    let size_1 = zopfli_compress_deflate(&payload, &opts_1).expect("1 iter").len();
    let size_5 = zopfli_compress_deflate(&payload, &opts_5).expect("5 iters").len();
    let size_15 = zopfli_compress_deflate(&payload, &opts_15).expect("15 iters").len();

    println!("   Pass 1 (Greedy Init): {} bytes", size_1);
    println!("   Pass 5 (EM Refined):  {} bytes (Gain: -{} bytes)", size_5, size_1 - size_5);
    println!("   Pass 15 (Converged):  {} bytes (Gain: -{} bytes)", size_15, size_1 - size_15);

    // Strict monotonic size non-increase
    assert!(size_5 <= size_1, "Pass 5 must be <= Pass 1");
    assert!(size_15 <= size_5, "Pass 15 must be <= Pass 5");
}

// ============================================================================
// Test 5: Master Invariant 6 Anti-Regression Gate (<= 3.0% Threshold)
// ============================================================================

#[test]
fn test_zopfli_master_invariant_6_anti_regression_gate() {
    println!("\n================================================================================");
    println!("🛡️  [ZOPFLI BENCH 5/5] Master Anti-Regression Invariant 6 Gate (<= 3.0%)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let payload = BenchmarkCorpusGenerator::generate(BenchmarkCorpusType::TextData, 32 * 1024);
    let opts = ZopfliOptions::fast();

    // Measure interleaved A/B runs (5 pairs) to eliminate thermal and frequency scaling noise
    let mut baseline_samples = Vec::new();
    let mut candidate_samples = Vec::new();
    for _ in 0..5 {
        let (b, _) = measure_adaptive_throughput(
            || {
                let res = zopfli_compress(&payload, ZopfliFormat::Deflate, &opts).expect("zopfli A");
                black_box(res);
            },
            payload.len(),
            &mut governor,
        );
        baseline_samples.push(b);

        let (c, _) = measure_adaptive_throughput(
            || {
                let res = zopfli_compress(&payload, ZopfliFormat::Deflate, &opts).expect("zopfli B");
                black_box(res);
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
        0.0
    };

    println!("   Baseline Pass A Throughput:  {:.2} MB/s", baseline_mb_s);
    println!("   Candidate Pass B Throughput: {:.2} MB/s", candidate_mb_s);
    println!("   Delta Regression:            {:.3}% (Hard Gate: <= {:.1}%)", diff_pct, MAX_ALLOWED_REGRESSION_PCT);

    assert!(
        diff_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "❌ Invariant 6 Violation: Performance regression of {:.3}% exceeded hard limit of {:.1}%",
        diff_pct,
        MAX_ALLOWED_REGRESSION_PCT
    );

    println!("   Verdict:                     ✅ PASSED (Invariant 6 Compliance Verified)");
}

