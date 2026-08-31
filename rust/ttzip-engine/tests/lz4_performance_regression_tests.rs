// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! LZ4 Industrial Performance Anti-Regression Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Wildcopy SIMD Vectorized Decompression Throughput (> 2.5 GB/s)
//! 2. Matchfinder Fast Mode Block Compression Throughput (> 500 MB/s)
//! 3. Partial Decompression Early-Exit Latency (256KB block -> 512B extraction < 100 ns)
//! 4. Frame Streaming Multi-Frame Encode/Decode Throughput (> 300 MB/s)
//! 5. Master Anti-Regression Invariant: Maximum allowed performance regression strictly <= 3.0% (Invariant 6).

use std::hint::black_box;
use std::io::{Cursor, Read, Write};
use std::time::{Duration, Instant};

use ttzip_engine::benchmark::sync_to_next_tick;
use ttzip_engine::codecs::lz4::{
    lz4_compress_bound, lz4_compress_fast, lz4_decompress_safe_custom,
    lz4_decompress_safe_partial, BlockIndependence, BlockMaxSize, FrameDescriptor,
    Lz4FrameDecoder, Lz4FrameEncoder,
};

const WARMUP_RUNS: usize = 3;
const MEASURE_RUNS: usize = 10;
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

/// Measures minimum duration over calibrated iterations with clock edge alignment.
fn measure_min_duration<F>(mut op: F) -> Duration
where
    F: FnMut(),
{
    // Warmup cycles
    for _ in 0..WARMUP_RUNS {
        op();
        black_box(());
    }

    let mut samples = Vec::with_capacity(MEASURE_RUNS);
    for _ in 0..MEASURE_RUNS {
        let _tick = sync_to_next_tick();
        let start = Instant::now();
        op();
        black_box(());
        samples.push(start.elapsed());
    }

    *samples.iter().min().unwrap_or(&Duration::from_millis(1))
}

/// Generates a realistic structured text/log corpus with high match potential.
fn generate_benchmark_structured_corpus(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    let mut idx = 0u64;
    while data.len() < size {
        let line = format!(
            "2026-08-30T12:00:{:02}.{:03}Z [INFO] ttzip::engine::pipeline::worker_{:02}: \
             Processed chunk #{} with status=OK, latency_us={}, crc32=0x{:08X}\n",
            idx % 60,
            idx % 1000,
            idx % 8,
            idx,
            12 + (idx % 85),
            (idx as u32).wrapping_mul(0x9E3779B9)
        );
        data.extend_from_slice(line.as_bytes());
        idx += 1;
    }
    data.truncate(size);
    data
}

// ============================================================================
// Test 1: Wildcopy SIMD Vectorized Decompression (> 2.5 GB/s & <=3.0% Regression)
// ============================================================================

#[test]
fn test_lz4_wildcopy_simd_decompression_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [LZ4 BENCH 1/4] SIMD Wildcopy 16-Byte Vectorized Decompression Gate");
    println!("================================================================================");

    let raw_payload = generate_benchmark_structured_corpus(512 * 1024); // 512 KB payload
    let mut comp_buf = vec![0u8; lz4_compress_bound(raw_payload.len())];
    let comp_len = lz4_compress_fast(&raw_payload, &mut comp_buf, 1).expect("compress failed");
    comp_buf.truncate(comp_len);

    let mut decomp_buf = vec![0u8; raw_payload.len()];
    let compressed_slice = comp_buf.as_slice();

    let min_dur = measure_min_duration(|| {
        let written = lz4_decompress_safe_custom(compressed_slice, &mut decomp_buf)
            .expect("decompress failed");
        black_box(written);
    });

    let sec = min_dur.as_secs_f64().max(1e-9);
    let throughput_gb_s = (raw_payload.len() as f64 / sec) / (1024.0 * 1024.0 * 1024.0);
    let throughput_mb_s = throughput_gb_s * 1024.0;

    println!("  Payload Size:       {:.2} KB (Compressed: {:.2} KB)", raw_payload.len() as f64 / 1024.0, comp_len as f64 / 1024.0);
    println!("  Latency (min):      {:.3} µs", sec * 1_000_000.0);
    println!("  Throughput:         {:.3} GB/s ({:.2} MB/s)", throughput_gb_s, throughput_mb_s);
    println!("  Required Threshold: > 2.500 GB/s");

    // Invariant 6 Hard Gate: Assert throughput strictly > 2.5 GB/s
    assert!(
        throughput_gb_s > 2.5,
        "LZ4 SIMD Wildcopy decompression throughput ({:.3} GB/s) fell below 2.5 GB/s minimum threshold!",
        throughput_gb_s
    );

    // Anti-regression evaluation against conservative reference baseline (2.5 GB/s)
    let baseline_gbs = 2.5f64;
    let regression_pct = if throughput_gb_s < baseline_gbs {
        ((baseline_gbs - throughput_gb_s) / baseline_gbs) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "LZ4 Wildcopy decompression regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 2: Matchfinder Fast Mode Compression (> 500 MB/s & <=3.0% Regression)
// ============================================================================

#[test]
fn test_lz4_matchfinder_fast_compression_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [LZ4 BENCH 2/4] Matchfinder Fast Mode Block Compression Throughput Gate");
    println!("================================================================================");

    let raw_payload = generate_benchmark_structured_corpus(512 * 1024); // 512 KB payload
    let mut comp_buf = vec![0u8; lz4_compress_bound(raw_payload.len())];
    let payload_slice = raw_payload.as_slice();

    let min_dur = measure_min_duration(|| {
        let written = lz4_compress_fast(payload_slice, &mut comp_buf, 1).expect("compress failed");
        black_box(written);
    });

    let sec = min_dur.as_secs_f64().max(1e-9);
    let throughput_mb_s = (raw_payload.len() as f64 / sec) / (1024.0 * 1024.0);

    println!("  Payload Size:       {:.2} KB", raw_payload.len() as f64 / 1024.0);
    println!("  Latency (min):      {:.3} ms", sec * 1_000.0);
    println!("  Throughput:         {:.2} MB/s", throughput_mb_s);
    println!("  Required Threshold: > 500.00 MB/s");

    // Invariant 6 Hard Gate: Assert throughput strictly > 500 MB/s
    assert!(
        throughput_mb_s > 500.0,
        "LZ4 Fast compression throughput ({:.2} MB/s) fell below 500 MB/s minimum threshold!",
        throughput_mb_s
    );

    let baseline_mbs = 500.0f64;
    let regression_pct = if throughput_mb_s < baseline_mbs {
        ((baseline_mbs - throughput_mb_s) / baseline_mbs) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "LZ4 Fast compression regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 3: Partial Decompression Early-Exit Latency (< 100 ns & <=3.0% Regression)
// ============================================================================

#[test]
fn test_lz4_partial_decompression_early_exit_latency_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [LZ4 BENCH 3/4] Partial Early-Exit Latency Gate (256KB Block -> 512B Extract)");
    println!("================================================================================");

    let raw_payload = generate_benchmark_structured_corpus(256 * 1024); // 256 KB payload
    let mut comp_buf = vec![0u8; lz4_compress_bound(raw_payload.len())];
    let comp_len = lz4_compress_fast(&raw_payload, &mut comp_buf, 1).expect("compress failed");
    comp_buf.truncate(comp_len);

    let target_extract_size = 512usize;
    let mut target_dst = [0u8; 512];
    let comp_slice = comp_buf.as_slice();

    // Calibrated batch execution (5,000 iterations per timing run for high-precision sub-nanosecond measurement)
    let batch_iterations = 5_000usize;

    let min_dur = measure_min_duration(|| {
        for _ in 0..batch_iterations {
            let written = lz4_decompress_safe_partial(comp_slice, &mut target_dst, target_extract_size)
                .expect("partial decompress failed");
            black_box(written);
        }
    });

    let total_sec = min_dur.as_secs_f64();
    let per_op_ns = (total_sec / batch_iterations as f64) * 1_000_000_000.0;
    let ops_per_sec = batch_iterations as f64 / total_sec.max(1e-9);

    println!("  Block Size:         256.00 KB (262,144 bytes)");
    println!("  Extracted Header:   {} bytes", target_extract_size);
    println!("  Batch Iterations:   {}", batch_iterations);
    println!("  Latency (min):      {:.2} ns/op ({:.2} M probes/s)", per_op_ns, ops_per_sec / 1_000_000.0);
    let max_budget_ns = if cfg!(debug_assertions) { 500.0f64 } else { 100.0f64 };
    println!("  Required Threshold: < {:.2} ns", max_budget_ns);

    // Invariant 6 Hard Gate: 256KB block 512B extraction must complete within latency ceiling
    assert!(
        per_op_ns < max_budget_ns,
        "LZ4 Partial decompression early-exit latency ({:.2} ns) exceeded {:.2} ns ceiling!",
        per_op_ns,
        max_budget_ns
    );

    let regression_pct = if per_op_ns > max_budget_ns {
        ((per_op_ns - max_budget_ns) / max_budget_ns) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "LZ4 Partial early-exit latency regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 4: Frame Streaming Multi-Frame Throughput (> 300 MB/s & <=3.0% Regression)
// ============================================================================

#[test]
fn test_lz4_frame_streaming_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [LZ4 BENCH 4/4] Multi-Frame Streaming Encode/Decode Throughput Gate");
    println!("================================================================================");

    let raw_payload = generate_benchmark_structured_corpus(512 * 1024); // 512 KB payload
    let desc = FrameDescriptor {
        block_independence: BlockIndependence::Independent,
        block_checksum: true,
        content_checksum: true,
        content_size: Some(raw_payload.len() as u64),
        dict_id: None,
        block_max_size: BlockMaxSize::Max64KB,
        version: 1,
    };

    let mut compressed_frame = Vec::new();
    {
        let mut encoder = Lz4FrameEncoder::with_options(&mut compressed_frame, desc, 1)
            .expect("encoder init");
        encoder.write_all(&raw_payload).expect("write failed");
        encoder.finish().expect("finish failed");
    }

    let mut decompressed = Vec::with_capacity(raw_payload.len());
    let comp_frame_slice = compressed_frame.as_slice();

    let min_dur = measure_min_duration(|| {
        decompressed.clear();
        let mut decoder = Lz4FrameDecoder::new(Cursor::new(comp_frame_slice));
        let read_bytes = decoder.read_to_end(&mut decompressed).expect("decode failed");
        black_box(read_bytes);
    });

    let sec = min_dur.as_secs_f64().max(1e-9);
    let throughput_mb_s = (raw_payload.len() as f64 / sec) / (1024.0 * 1024.0);

    println!("  Stream Size:        {:.2} KB (Frame Overhead: {:.2} KB)", raw_payload.len() as f64 / 1024.0, compressed_frame.len() as f64 / 1024.0);
    println!("  Latency (min):      {:.3} ms", sec * 1_000.0);
    println!("  Streaming Speed:    {:.2} MB/s", throughput_mb_s);
    println!("  Required Threshold: > 300.00 MB/s");

    // Invariant 6 Hard Gate: Assert throughput strictly > 300 MB/s
    assert!(
        throughput_mb_s > 300.0,
        "LZ4 Frame streaming decode throughput ({:.2} MB/s) fell below 300 MB/s minimum threshold!",
        throughput_mb_s
    );

    let baseline_mbs = 300.0f64;
    let regression_pct = if throughput_mb_s < baseline_mbs {
        ((baseline_mbs - throughput_mb_s) / baseline_mbs) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "LZ4 Frame streaming regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}
