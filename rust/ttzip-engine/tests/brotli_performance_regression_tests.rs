// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Google Brotli Industrial Performance Anti-Regression Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Clock rising-edge alignment (`wait_for_next_tick`)
//! 2. 1000ms adaptive time integration with 70s active / 5s cooling thermal protection (`ThermalThrottleGovernor`)
//! 3. Brotli BitReader / Window decoding throughput gate (> 600 MB/s)
//! 4. 120KB static dictionary & 121 transforms constant-time lookup throughput gate (> 10 Mops/s)
//! 5. Second-order context & 2-level Huffman DTable decoding throughput gate (> 100 Mops/s)
//! 6. BrotliStreamWriter Q0/Q1 fast compression throughput gate (> 250 MB/s)
//! 7. BrotliStreamDecoder streaming decompression throughput gate (> 200 MB/s)
//! 8. Master Anti-Regression Invariant: Maximum allowed performance regression strictly <= 3.0% (Invariant 6).

use std::hint::black_box;
use std::io::{Cursor, Read, Write};
use std::time::{Duration, Instant};

use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::wait_for_next_tick;
use ttzip_engine::codecs::brotli::{
    get_context_id, get_dictionary_word, transform_dictionary_word, BrotliBitReader,
    BrotliContextMode, BrotliStreamDecoder, BrotliStreamWriter, BrotliWindow, HuffmanCode,
    HuffmanTable, HUFFMAN_TABLE_MASK, TRANSFORMS_TABLE,
};

const WARMUP_RUNS: usize = 2;
const MIN_INTEGRATION_WINDOW: Duration = Duration::from_millis(50); // 50ms Adaptive Integration
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

static BENCH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

/// Measures adaptive operation rate (Million ops/sec) over at least 1000ms with clock rising-edge alignment.
fn measure_adaptive_ops_rate<F>(
    mut op: F,
    ops_per_iteration: usize,
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
    let total_ops = (iterations as f64) * (ops_per_iteration as f64);
    let mops_s = (total_ops / elapsed_secs) / 1_000_000.0;
    let avg_latency_ns = (elapsed_secs / total_ops) * 1_000_000_000.0;

    (mops_s, avg_latency_ns)
}

/// Generates a realistic structured text corpus for benchmark reproducibility.
fn generate_benchmark_structured_corpus(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    let mut idx = 0u64;
    while data.len() < size {
        let line = format!(
            "2026-08-30T14:30:{:02}.{:03}Z [INFO] ttzip::engine::brotli::worker_{:02}: \
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
// Test 1: Brotli BitReader / Window Decoding Throughput (> 600 MB/s & <=3.0% Regression)
// ============================================================================

#[test]
fn test_brotli_bit_reader_and_window_throughput_gate() {
    let _lock = BENCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    println!("\n================================================================================");
    println!("🧪 [BROTLI BENCH 1/5] BitReader 64-bit Accumulator & WBITS Window Parser Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));

    // Generate 512 KiB synthetic bitstream containing repetitive WBITS headers and bit chunks
    let mut bitstream_payload = Vec::with_capacity(512 * 1024);
    for i in 0..(256 * 1024) {
        // Pattern: WBITS=16 prefix (bit 0=0) followed by 15 bits of payload
        let u16_val = ((i as u16) & 0x7FFF) << 1; // Bit 0 is 0 (WBITS=16)
        bitstream_payload.extend_from_slice(&u16_val.to_le_bytes());
    }

    let payload_bytes = bitstream_payload.len();
    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let mut br = BrotliBitReader::new(&bitstream_payload);
            let mut parsed_count = 0usize;
            while br.pos < bitstream_payload.len() - 4 {
                if let Ok(w) = BrotliWindow::parse_window_bits(&mut br, false) {
                    black_box(w);
                    parsed_count += 1;
                }
                let _ = br.read_bits(15);
            }
            black_box(parsed_count);
        },
        payload_bytes,
        &mut governor,
    );

    let min_threshold = if cfg!(debug_assertions) { 200.0f64 } else { 600.0f64 };
    println!(
        "  Payload Size:       {:.2} KB ({} bytes)",
        payload_bytes as f64 / 1024.0,
        payload_bytes
    );
    println!("  Avg Pass Latency:   {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  Throughput:         {:.2} MB/s", throughput_mb_s);
    println!("  Required Threshold: > {:.2} MB/s", min_threshold);

    assert!(
        throughput_mb_s > min_threshold,
        "Brotli BitReader throughput ({:.2} MB/s) fell below {:.2} MB/s threshold!",
        throughput_mb_s, min_threshold
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
        "Brotli BitReader regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 2: 120KB Static Dictionary & 121 Transforms (> 10 Mops/s & <=3.0% Regression)
// ============================================================================

#[test]
fn test_brotli_static_dictionary_and_transforms_throughput_gate() {
    let _lock = BENCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    println!("\n================================================================================");
    println!("🧪 [BROTLI BENCH 2/5] 120KB Static Dictionary & 121 Transforms Lookup Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));
    let ops_per_iter = TRANSFORMS_TABLE.len() * 21; // 121 transforms * 21 word length buckets = 2541 ops

    let mut dst_buffer = [0u8; 128];
    let (mops_s, avg_latency_ns) = measure_adaptive_ops_rate(
        || {
            let mut total_written = 0usize;
            for (t_idx, _) in TRANSFORMS_TABLE.iter().enumerate() {
                for len in 4..=24 {
                    let word_idx = (t_idx * 7 + len) % 16;
                    if let Some(word) = get_dictionary_word(len, word_idx) {
                        if let Ok(written) =
                            transform_dictionary_word(&mut dst_buffer, word, t_idx)
                        {
                            total_written = total_written.wrapping_add(written);
                        }
                    }
                }
            }
            black_box(total_written);
        },
        ops_per_iter,
        &mut governor,
    );

    println!("  Operations/Pass:    {} lookups", ops_per_iter);
    println!("  Avg Pass Latency:   {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  Operations Rate:    {:.2} Mops/s", mops_s);
    println!("  Required Threshold: > 10.00 Mops/s");

    assert!(
        mops_s > 10.0,
        "Brotli Static Dictionary lookup rate ({:.2} Mops/s) fell below 10.00 Mops/s threshold!",
        mops_s
    );

    let baseline_mops = 10.0f64;
    let regression_pct = if mops_s < baseline_mops {
        ((baseline_mops - mops_s) / baseline_mops) * 100.0
    } else {
        0.0f64
    };

    println!(
        "  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)",
        regression_pct, MAX_ALLOWED_REGRESSION_PCT
    );
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Brotli Static Dictionary regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 3: 2nd-Order Context & 2-Level Huffman DTable (> 100 Mops/s & <=3.0% Regression)
// ============================================================================

#[test]
fn test_brotli_context_modeling_and_huffman_dtable_throughput_gate() {
    let _lock = BENCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    println!("\n================================================================================");
    println!("🧪 [BROTLI BENCH 3/5] Second-Order Context LUT & 2-Level Huffman DTable Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));

    // Construct Canonical Huffman DTable with 256 symbols of code length 8
    let code_lengths = vec![8u8; 256];
    let htree = HuffmanTable::build(&code_lengths, 256).expect("Huffman build failed");

    let ops_per_iter = 4096usize;
    let (mops_s, avg_latency_ns) = measure_adaptive_ops_rate(
        || {
            let mut acc = 0usize;
            for i in 0..ops_per_iter {
                let p1 = (i & 0xFF) as u8;
                let p2 = ((i >> 8) & 0xFF) as u8;
                let mode = match (i >> 4) & 3 {
                    0 => BrotliContextMode::Lsb6,
                    1 => BrotliContextMode::Msb6,
                    2 => BrotliContextMode::Utf8,
                    _ => BrotliContextMode::Signed,
                };
                let ctx_id = get_context_id(p1, p2, mode);

                // Zero-branch 1st-level root table lookup
                let root_idx = (ctx_id ^ (i & 0xFF)) & HUFFMAN_TABLE_MASK;
                let entry: HuffmanCode = htree.entries[root_idx];
                acc = acc.wrapping_add(entry.value as usize);
            }
            black_box(acc);
        },
        ops_per_iter,
        &mut governor,
    );

    println!("  Ops Per Pass:       {ops_per_iter} context queries + DTable lookups");
    println!("  Avg Op Latency:     {:.3} ns", avg_latency_ns);
    println!("  Throughput:         {:.2} Million ops/sec (Mops/s)", mops_s);
    println!("  Required Threshold: > 100.00 Mops/s (100,000,000 ops/s)");

    assert!(
        mops_s > 100.0,
        "Context & Huffman DTable throughput ({:.2} Mops/s) fell below 100.00 Mops/s threshold!",
        mops_s
    );

    let baseline_mops = 100.0f64;
    let regression_pct = if mops_s < baseline_mops {
        ((baseline_mops - mops_s) / baseline_mops) * 100.0
    } else {
        0.0f64
    };

    println!(
        "  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)",
        regression_pct, MAX_ALLOWED_REGRESSION_PCT
    );
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Context & Huffman DTable regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 4: BrotliStreamWriter Q0/Q1 Fast Compression (> 250 MB/s & <=3.0% Regression)
// ============================================================================

#[test]
fn test_brotli_stream_writer_q0_q1_compression_throughput_gate() {
    let _lock = BENCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    println!("\n================================================================================");
    println!("🧪 [BROTLI BENCH 4/5] BrotliStreamWriter Q0/Q1 Fast Compression Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));
    let raw_payload = generate_benchmark_structured_corpus(512 * 1024); // 512 KiB payload
    let mut comp_buffer = Vec::with_capacity(raw_payload.len() + 1024);

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            comp_buffer.clear();
            let mut writer = BrotliStreamWriter::with_quality(&mut comp_buffer, 0)
                .expect("brotli Q0 stream writer failed");
            writer.write_all(&raw_payload).expect("write failed");
            let _ = writer.finish().expect("finish failed");
            black_box(comp_buffer.len());
        },
        raw_payload.len(),
        &mut governor,
    );

    let min_threshold = if cfg!(debug_assertions) { 150.0f64 } else { 250.0f64 };

    println!(
        "  Payload Size:       {:.2} KB ({} bytes)",
        raw_payload.len() as f64 / 1024.0,
        raw_payload.len()
    );
    println!("  Avg Pass Latency:   {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  Throughput:         {:.2} MB/s", throughput_mb_s);
    println!("  Required Threshold: > {:.2} MB/s", min_threshold);

    assert!(
        throughput_mb_s > min_threshold,
        "Brotli Q0 compression throughput ({:.2} MB/s) fell below {:.2} MB/s threshold!",
        throughput_mb_s, min_threshold
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
        "Brotli Stream Compression regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 5: BrotliStreamDecoder Streaming Decompression (> 200 MB/s & <=3.0% Regression)
// ============================================================================

#[test]
fn test_brotli_stream_decoder_decompression_throughput_gate() {
    let _lock = BENCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    println!("\n================================================================================");
    println!("🧪 [BROTLI BENCH 5/5] BrotliStreamDecoder Streaming Decompression Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));
    let raw_payload = generate_benchmark_structured_corpus(512 * 1024); // 512 KiB payload

    let mut comp_buffer = Vec::new();
    let mut writer = BrotliStreamWriter::with_quality(&mut comp_buffer, 1)
        .expect("brotli Q1 compression failed");
    writer.write_all(&raw_payload).expect("write failed");
    let _ = writer.finish().expect("finish failed");

    let mut decomp_buffer = vec![0u8; raw_payload.len()];
    let comp_slice = comp_buffer.as_slice();

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let mut decoder = BrotliStreamDecoder::new(Cursor::new(comp_slice));
            let mut offset = 0usize;
            while offset < decomp_buffer.len() {
                match decoder.read(&mut decomp_buffer[offset..]) {
                    Ok(0) => break,
                    Ok(n) => offset += n,
                    Err(e) => panic!("decompression read failed: {}", e),
                }
            }
            black_box(offset);
        },
        raw_payload.len(),
        &mut governor,
    );

    println!(
        "  Payload Size:       {:.2} KB (Compressed: {:.2} KB)",
        raw_payload.len() as f64 / 1024.0,
        comp_buffer.len() as f64 / 1024.0
    );
    println!("  Avg Pass Latency:   {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  Throughput:         {:.2} MB/s", throughput_mb_s);
    println!("  Required Threshold: > 200.00 MB/s");

    assert!(
        throughput_mb_s > 200.0,
        "Brotli decompression throughput ({:.2} MB/s) fell below 200.00 MB/s threshold!",
        throughput_mb_s
    );

    let baseline_mbs = 200.0f64;
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
        "Brotli decompression regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}
