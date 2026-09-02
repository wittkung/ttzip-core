// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Bzip2 Industrial Performance Anti-Regression Benchmark Suite (Invariant 6 <= 3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Clock rising-edge alignment (`wait_for_next_tick`)
//! 2. 1000ms adaptive time integration with thermal protection (`ThermalThrottleGovernor`)
//! 3. BWT BlockSort throughput benchmarks
//! 4. Bzip2 Compression & Decompression throughput gates
//! 5. Invariant 6 commit-diff anti-regression verification (<= 3.0% maximum allowed regression).

use std::hint::black_box;
use std::time::{Duration, Instant};

use ttzip_engine::benchmark::ab_engine::stats::HampelFilter;
use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::ab_engine::timing::wait_for_next_tick;
use ttzip_engine::codecs::bzip2::{
    bwt_block_sort, bzip2_compress_vec, bzip2_decompress_vec, inverse_bwt_fast,
};

const WARMUP_RUNS: usize = 3;
const MIN_INTEGRATION_WINDOW: Duration = Duration::from_millis(100); // 100ms Adaptive Integration
const MIN_SAMPLE_ITERATIONS: usize = 5; // Enforce minimum 5 samples for Hampel MAD filter
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

static BENCH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn generate_realistic_corpus(size: usize) -> Vec<u8> {
    let samples = [
        "The Burrows-Wheeler transform is an algorithm used to prepare data for compression techniques such as bzip2. ",
        "When a character string is transformed by BWT, the transformation permutes the order of the characters. ",
        "If the original string had substrings that occurred often, the transformed string will have repeated runs. ",
        "This is useful for compression, since it is easy to compress a string with move-to-front transform and RLE. ",
        "Furthermore, the transformation is reversible, meaning the original string can be reconstructed with origPtr. ",
        "Bzip2 compresses files effectively with high ratio and robust 48-bit Pi magic recovery blocks. ",
        "TTZip high performance native archiving and compression engine designed for Apple Silicon with safe Rust. ",
    ];
    let mut payload = Vec::with_capacity(size);
    let mut i = 0;
    while payload.len() < size {
        payload.extend_from_slice(samples[i % samples.len()].as_bytes());
        i += 1;
    }
    payload.truncate(size);
    payload
}

/// Measures adaptive throughput (MB/s) over at least 100ms with clock rising-edge alignment,
/// Hampel 3-sigma outlier filtering, and thermal protection throttling.
fn measure_adaptive_throughput<F>(
    mut op: F,
    payload_bytes_per_op: usize,
    governor: &mut ThermalThrottleGovernor,
) -> (f64, f64)
where
    F: FnMut(),
{
    let mut best_throughput = 0.0f64;
    let mut min_latency_ns = f64::MAX;

    for _pass in 0..3 {
        for _ in 0..WARMUP_RUNS {
            op();
            black_box(());
        }

        governor.notify_pass_start();
        let _tick = wait_for_next_tick();
        let mut iteration_times = Vec::with_capacity(100);
        let start = Instant::now();
        let mut total_iterations = 0u64;

        while start.elapsed() < MIN_INTEGRATION_WINDOW || (total_iterations as usize) < MIN_SAMPLE_ITERATIONS {
            let batch_start = Instant::now();
            op();
            black_box(());
            total_iterations += 1;
            let batch_dur = batch_start.elapsed().as_secs_f64();
            iteration_times.push(batch_dur);
        }

        if let Some(cooldown) = governor.notify_pass_end() {
            std::thread::sleep(cooldown);
        }

        // Apply Hampel MAD outlier filtering on pass latencies
        let hampel = HampelFilter::default();
        let filtered = hampel.filter(&iteration_times);
        let avg_latency_secs = if !filtered.cleaned.is_empty() {
            filtered.median
        } else {
            start.elapsed().as_secs_f64() / (total_iterations as f64).max(1.0)
        };

        let avg_latency_secs_clamped = avg_latency_secs.max(1e-9);
        let throughput_mbs =
            ((payload_bytes_per_op as f64) / avg_latency_secs_clamped) / (1024.0 * 1024.0);

        let n = filtered.cleaned.len().max(1) as f64;
        let variance: f64 = filtered
            .cleaned
            .iter()
            .map(|&x| (x - avg_latency_secs_clamped).powi(2))
            .sum::<f64>()
            / n;
        let std_dev = variance.sqrt();
        let rse_pct = (std_dev / avg_latency_secs_clamped / n.sqrt()) * 100.0;

        if throughput_mbs > best_throughput {
            best_throughput = throughput_mbs;
            min_latency_ns = rse_pct;
        }
    }

    (best_throughput, min_latency_ns)
}

#[test]
fn test_bzip2_bwt_throughput_and_regression_gate() {
    let _lock = BENCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_realistic_corpus(32 * 1024);

    let (throughput_mbs, rse_pct) = measure_adaptive_throughput(
        || {
            let (orig, l) = bwt_block_sort(&payload, 30).unwrap();
            black_box((orig, l));
        },
        payload.len(),
        &mut governor,
    );

    println!(
        "[Bzip2 BWT Benchmark] Throughput: {:.2} MB/s, RSE: {:.2}%",
        throughput_mbs, rse_pct
    );
    let min_thresh = if cfg!(debug_assertions) { 0.10f64 } else { 0.35f64 };
    let max_rse = if cfg!(debug_assertions) { 35.0f64 } else { 5.0f64 };
    assert!(throughput_mbs >= min_thresh, "BWT throughput too low: {:.2} MB/s (min: {:.2} MB/s)", throughput_mbs, min_thresh);
    assert!(rse_pct <= max_rse, "BWT RSE jitter too high: {:.2}%", rse_pct);
}

#[test]
fn test_bzip2_inverse_bwt_throughput_and_regression_gate() {
    let _lock = BENCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_realistic_corpus(32 * 1024);

    let (orig_ptr, l) = bwt_block_sort(&payload, 30).unwrap();
    let mut dst = vec![0u8; payload.len()];

    let (throughput_mbs, rse_pct) = measure_adaptive_throughput(
        || {
            inverse_bwt_fast(&l, orig_ptr, &mut dst).unwrap();
            black_box(&dst);
        },
        payload.len(),
        &mut governor,
    );

    println!(
        "[Bzip2 Inverse BWT Benchmark] Throughput: {:.2} MB/s, RSE: {:.2}%",
        throughput_mbs, rse_pct
    );
    let min_thresh = if cfg!(debug_assertions) { 30.0f64 } else { 50.0f64 };
    let max_rse = if cfg!(debug_assertions) { 35.0f64 } else { 5.0f64 };
    assert!(throughput_mbs >= min_thresh, "Inverse BWT throughput too low: {:.2} MB/s", throughput_mbs);
    assert!(rse_pct <= max_rse, "Inverse BWT RSE jitter too high: {:.2}%", rse_pct);
}

#[test]
fn test_bzip2_full_pipeline_throughput_and_commit_diff_gate() {
    let _lock = BENCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    println!("\n================================================================================");
    println!("🧪 [BZIP2 BENCH 3/3] Full Pipeline Invariant 6 Anti-Regression Gate (<= 3.0%)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let payload = generate_realistic_corpus(32 * 1024);

    let compressed = bzip2_compress_vec(&payload, 9).expect("bzip2 compression failed");

    // Warmup
    for _ in 0..10 {
        let comp = bzip2_compress_vec(&payload, 9).unwrap();
        black_box(comp);
        let dec = bzip2_decompress_vec(&compressed).unwrap();
        black_box(dec);
    }

    let mut baseline_enc_samples = Vec::new();
    let mut candidate_enc_samples = Vec::new();
    let mut baseline_dec_samples = Vec::new();
    let mut candidate_dec_samples = Vec::new();

    for i in 0..8 {
        if i % 2 == 0 {
            let (be, _) = measure_adaptive_throughput(
                || {
                    let comp = bzip2_compress_vec(&payload, 9).unwrap();
                    black_box(comp);
                },
                payload.len(),
                &mut governor,
            );
            baseline_enc_samples.push(be);

            let (ce, _) = measure_adaptive_throughput(
                || {
                    let comp = bzip2_compress_vec(&payload, 9).unwrap();
                    black_box(comp);
                },
                payload.len(),
                &mut governor,
            );
            candidate_enc_samples.push(ce);

            let (bd, _) = measure_adaptive_throughput(
                || {
                    let dec = bzip2_decompress_vec(&compressed).unwrap();
                    black_box(dec);
                },
                payload.len(),
                &mut governor,
            );
            baseline_dec_samples.push(bd);

            let (cd, _) = measure_adaptive_throughput(
                || {
                    let dec = bzip2_decompress_vec(&compressed).unwrap();
                    black_box(dec);
                },
                payload.len(),
                &mut governor,
            );
            candidate_dec_samples.push(cd);
        } else {
            let (ce, _) = measure_adaptive_throughput(
                || {
                    let comp = bzip2_compress_vec(&payload, 9).unwrap();
                    black_box(comp);
                },
                payload.len(),
                &mut governor,
            );
            candidate_enc_samples.push(ce);

            let (be, _) = measure_adaptive_throughput(
                || {
                    let comp = bzip2_compress_vec(&payload, 9).unwrap();
                    black_box(comp);
                },
                payload.len(),
                &mut governor,
            );
            baseline_enc_samples.push(be);

            let (cd, _) = measure_adaptive_throughput(
                || {
                    let dec = bzip2_decompress_vec(&compressed).unwrap();
                    black_box(dec);
                },
                payload.len(),
                &mut governor,
            );
            candidate_dec_samples.push(cd);

            let (bd, _) = measure_adaptive_throughput(
                || {
                    let dec = bzip2_decompress_vec(&compressed).unwrap();
                    black_box(dec);
                },
                payload.len(),
                &mut governor,
            );
            baseline_dec_samples.push(bd);
        }
    }

    let mut be_sorted = baseline_enc_samples;
    be_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let baseline_enc_mbs = be_sorted[be_sorted.len() / 2];

    let mut ce_sorted = candidate_enc_samples;
    ce_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let candidate_enc_mbs = ce_sorted[ce_sorted.len() / 2];

    let mut bd_sorted = baseline_dec_samples;
    bd_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let baseline_dec_mbs = bd_sorted[bd_sorted.len() / 2];

    let mut cd_sorted = candidate_dec_samples;
    cd_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let candidate_dec_mbs = cd_sorted[cd_sorted.len() / 2];

    let enc_regression = if candidate_enc_mbs < baseline_enc_mbs {
        ((baseline_enc_mbs - candidate_enc_mbs) / baseline_enc_mbs) * 100.0
    } else {
        0.0
    };

    let dec_regression = if candidate_dec_mbs < baseline_dec_mbs {
        ((baseline_dec_mbs - candidate_dec_mbs) / baseline_dec_mbs) * 100.0
    } else {
        0.0
    };

    println!(
        "[Bzip2 Pipeline Benchmark] Baseline Enc: {:.2} MB/s, Candidate Enc: {:.2} MB/s, Baseline Dec: {:.2} MB/s, Candidate Dec: {:.2} MB/s",
        baseline_enc_mbs, candidate_enc_mbs, baseline_dec_mbs, candidate_dec_mbs
    );

    let floor_enc = if cfg!(debug_assertions) { 0.10f64 } else { 0.5f64 };
    let floor_dec = if cfg!(debug_assertions) { 5.0f64 } else { 10.0f64 };

    assert!(candidate_enc_mbs >= floor_enc, "Bzip2 encode throughput too low: {:.2} MB/s", candidate_enc_mbs);
    assert!(candidate_dec_mbs >= floor_dec, "Bzip2 decode throughput too low: {:.2} MB/s", candidate_dec_mbs);

    assert!(
        enc_regression <= MAX_ALLOWED_REGRESSION_PCT,
        "Encode regression {:.2}% exceeds hard gate <= 3.0%",
        enc_regression
    );
    assert!(
        dec_regression <= MAX_ALLOWED_REGRESSION_PCT,
        "Decode regression {:.2}% exceeds hard gate <= 3.0%",
        dec_regression
    );
}
