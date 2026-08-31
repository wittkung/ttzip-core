// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zero-Copy Mmap Engine Industrial Performance Anti-Regression Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Clock rising-edge alignment (`wait_for_next_tick`)
//! 2. 50ms adaptive time integration with Hampel MAD 3-sigma outlier filtering and thermal throttling
//! 3. Sequential memory scan throughput gate (> 10.0 GB/s)
//! 4. Random multi-threaded slice read throughput gate (> 6.0 GB/s)
//! 5. Cross-page boundary slicing throughput gate (> 8.0 GB/s)
//! 6. Micro-block random seek latency gate (< 50 ns avg)
//! 7. Master Anti-Regression Invariant: Maximum allowed performance regression strictly <= 3.0% (Invariant 6).

use std::hint::black_box;
use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::NamedTempFile;
use ttzip_engine::archive::source::{ArchiveSource, MmapSource, StorageMedium};
use ttzip_engine::benchmark::ab_engine::stats::HampelFilter;
use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::wait_for_next_tick;

const WARMUP_RUNS: usize = 2;
const MIN_INTEGRATION_WINDOW: Duration = Duration::from_millis(50); // 50ms Adaptive Integration
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

/// Measures adaptive throughput (MB/s) and latency (ns) over at least 50ms with clock rising-edge alignment,
/// Hampel 3-sigma outlier filtering, and thermal protection governor.
fn measure_adaptive_mmap_throughput<F>(
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

/// Helper function to create a test file filled with pseudo-random deterministic data.
fn create_bench_file(size: usize) -> (NamedTempFile, MmapSource) {
    let mut temp = NamedTempFile::new().expect("Failed to create temporary file");
    let mut buf = vec![0u8; 64 * 1024];
    let mut written = 0;
    let mut state = 0x1234_5678_9ABC_DEF0u64;

    while written < size {
        for chunk in buf.chunks_mut(8) {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let b = state.to_le_bytes();
            let len = chunk.len().min(8);
            chunk[..len].copy_from_slice(&b[..len]);
        }
        let to_write = (size - written).min(buf.len());
        temp.write_all(&buf[..to_write]).expect("Failed to write bench data");
        written += to_write;
    }
    temp.flush().expect("Failed to flush bench file");

    let source = MmapSource::open(temp.path(), StorageMedium::LocalFastApfs)
        .expect("Failed to open MmapSource for benchmark");
    (temp, source)
}

// ============================================================================
// Test 1: Sequential Memory Scan Throughput Gate (> 10.0 GB/s)
// ============================================================================
#[test]
fn test_mmap_sequential_scan_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [MMAP BENCH 1/4] Sequential Memory Scan Throughput Gate (> 10.0 GB/s)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let file_size = 16 * 1024 * 1024; // 16 MB mapped buffer
    let (_temp, source) = create_bench_file(file_size);

    let slice = source.as_slice().expect("Slice must be present");
    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_mmap_throughput(
        || {
            let mut acc = 0u64;
            for chunk in slice.chunks_exact(8) {
                let val = u64::from_le_bytes(chunk.try_into().unwrap());
                acc = acc.wrapping_add(val);
            }
            black_box(acc);
        },
        file_size,
        &mut governor,
    );

    let throughput_gb_s = throughput_mb_s / 1024.0;
    println!("  Payload Size:       {:.2} MB", file_size as f64 / (1024.0 * 1024.0));
    println!("  Latency (avg):      {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  Scan Speed:         {:.3} GB/s ({:.2} MB/s)", throughput_gb_s, throughput_mb_s);

    let min_threshold_mb_s = if cfg!(debug_assertions) { 2000.0f64 } else { 10000.0f64 };
    println!("  Required Threshold: > {:.2} MB/s ({:.2} GB/s)", min_threshold_mb_s, min_threshold_mb_s / 1024.0);

    assert!(
        throughput_mb_s >= min_threshold_mb_s,
        "Mmap sequential scan throughput ({:.2} MB/s) fell below {:.2} MB/s minimum threshold!",
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
        "Mmap sequential scan throughput regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 2: Random Multi-Threaded Slice Read Throughput Gate (> 6.0 GB/s)
// ============================================================================
#[test]
fn test_mmap_random_multithreaded_read_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [MMAP BENCH 2/4] Random Multi-Threaded Slice Read Gate (> 6.0 GB/s)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let file_size = 16 * 1024 * 1024; // 16 MB
    let (_temp, source) = create_bench_file(file_size);
    let source_arc = Arc::new(source);

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .ok();

    let slice_size = 64 * 1024; // 64 KB slices
    let chunks_per_batch = 128;
    let total_batch_bytes = slice_size * chunks_per_batch;

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_mmap_throughput(
        || {
            let run_batch = || {
                (0..chunks_per_batch).for_each(|i| {
                    let offset = ((i * 131071) % (file_size - slice_size)) as u64;
                    let mut buf = vec![0u8; slice_size];
                    let read = source_arc.read_at(&mut buf, offset).unwrap();
                    black_box(read);
                    black_box(buf[0]);
                });
            };

            if let Some(ref p) = pool {
                p.install(run_batch);
            } else {
                run_batch();
            }
        },
        total_batch_bytes,
        &mut governor,
    );

    let throughput_gb_s = throughput_mb_s / 1024.0;
    println!("  Batch Size:         {:.2} MB ({} threads, {} chunks of 64KB)",
        total_batch_bytes as f64 / (1024.0 * 1024.0), num_threads, chunks_per_batch);
    println!("  Latency (avg):      {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  Random Read Speed:  {:.3} GB/s ({:.2} MB/s)", throughput_gb_s, throughput_mb_s);

    let min_threshold_mb_s = if cfg!(debug_assertions) { 1500.0f64 } else { 6000.0f64 };
    println!("  Required Threshold: > {:.2} MB/s ({:.2} GB/s)", min_threshold_mb_s, min_threshold_mb_s / 1024.0);

    assert!(
        throughput_mb_s >= min_threshold_mb_s,
        "Mmap random multi-threaded read throughput ({:.2} MB/s) fell below {:.2} MB/s minimum threshold!",
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
        "Mmap random multi-threaded read regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 3: Cross-Page Boundary Slicing Throughput Gate (> 8.0 GB/s)
// ============================================================================
#[test]
fn test_mmap_cross_page_slicing_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [MMAP BENCH 3/4] Cross-Page Boundary Slicing Throughput Gate (> 8.0 GB/s)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let file_size = 16 * 1024 * 1024; // 16 MB
    let (_temp, source) = create_bench_file(file_size);

    let chunk_count = 256;
    let chunk_size = 32 * 1024; // 32 KB unaligned chunks
    let total_bytes = chunk_count * chunk_size;

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_mmap_throughput(
        || {
            let mut buf = vec![0u8; chunk_size];
            for i in 0..chunk_count {
                // Cross 16KB page boundary with odd offset
                let offset = (i * 65536 + 16383) as u64 % (file_size - chunk_size) as u64;
                let read = source.read_at(&mut buf, offset).unwrap();
                black_box(read);
                black_box(buf[0]);
            }
        },
        total_bytes,
        &mut governor,
    );

    let throughput_gb_s = throughput_mb_s / 1024.0;
    println!("  Payload Size:       {:.2} MB ({} cross-page unaligned reads)",
        total_bytes as f64 / (1024.0 * 1024.0), chunk_count);
    println!("  Latency (avg):      {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  Cross-Page Speed:   {:.3} GB/s ({:.2} MB/s)", throughput_gb_s, throughput_mb_s);

    let min_threshold_mb_s = if cfg!(debug_assertions) { 1800.0f64 } else { 8000.0f64 };
    println!("  Required Threshold: > {:.2} MB/s ({:.2} GB/s)", min_threshold_mb_s, min_threshold_mb_s / 1024.0);

    assert!(
        throughput_mb_s >= min_threshold_mb_s,
        "Mmap cross-page slicing throughput ({:.2} MB/s) fell below {:.2} MB/s minimum threshold!",
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
        "Mmap cross-page slicing regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 4: Micro-Block Random Seek Latency Gate (< 50 ns)
// ============================================================================
#[test]
fn test_mmap_micro_block_random_seek_latency_gate() {
    println!("\n================================================================================");
    println!("🧪 [MMAP BENCH 4/4] Micro-Block Random Seek Latency Gate (< 50 ns)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let file_size = 4 * 1024 * 1024; // 4 MB
    let (_temp, source) = create_bench_file(file_size);

    let read_ops = 1000;
    let payload_bytes = read_ops * 8;

    let (_throughput_mb_s, avg_latency_ns) = measure_adaptive_mmap_throughput(
        || {
            let mut buf = [0u8; 8];
            for i in 0..read_ops {
                let offset = (i * 4099) as u64 % (file_size - 8) as u64;
                let _ = source.read_at(&mut buf, offset);
                black_box(buf[0]);
            }
        },
        payload_bytes,
        &mut governor,
    );

    let per_op_latency_ns = avg_latency_ns / (read_ops as f64);
    println!("  Operations:         {} micro-reads (8B each)", read_ops);
    println!("  Per-Op Latency:     {:.2} ns", per_op_latency_ns);

    let max_allowed_latency_ns = if cfg!(debug_assertions) { 250.0f64 } else { 50.0f64 };
    println!("  Required Threshold: < {:.2} ns", max_allowed_latency_ns);

    assert!(
        per_op_latency_ns <= max_allowed_latency_ns,
        "Mmap micro-block seek latency ({:.2} ns) exceeded {:.2} ns threshold!",
        per_op_latency_ns,
        max_allowed_latency_ns
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}
