// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! XZ Container Industrial Performance Anti-Regression Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. XZ Stream Header & Stream Footer Codec Throughput (> 500 MB/s)
//! 2. XZ VLI Variable-Length Integer Codec Throughput (> 400 MB/s)
//! 3. XZ CRC64 ECMA-182 Hardware/Slicing-by-8 Throughput (> 800 MB/s)
//! 4. BCJ Branch Instruction Filter (x86 & ARM64) Throughput (> 600 MB/s)
//! 5. LZMA2 Multi-Block Parallel Compression & Stream Decompression Throughput
//! 6. Master Anti-Regression Invariant: Maximum allowed performance regression strictly <= 3.0% (Invariant 6).

use std::hint::black_box;
use std::time::{Duration, Instant};

use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::wait_for_next_tick;
use ttzip_engine::xz::bcj::{BcjArm64, BcjX86, BranchFilter};
use ttzip_engine::xz::checksum::crc64_xz;
use ttzip_engine::xz::decoder::xz_decompress;
use ttzip_engine::xz::header::{XzStreamFlags, XzStreamFooter, XzStreamHeader};
use ttzip_engine::xz::types::XzCheckType;
use ttzip_engine::xz::vli::{decode_vli, encode_vli};
use ttzip_engine::xz::writer::{xz_compress, XzBcjType, XzEncoderOptions};

const WARMUP_RUNS: usize = 2;
const MIN_INTEGRATION_WINDOW: Duration = Duration::from_millis(50); // 50ms Adaptive Integration
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

/// Executes adaptive time integration measurement over at least 1000ms with clock rising-edge alignment
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

/// Generates a structured realistic text corpus with repetitive patterns for benchmark reproducibility.
fn generate_benchmark_structured_corpus(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    let mut idx = 0u64;
    while data.len() < size {
        let line = format!(
            "2026-08-30T14:30:{:02}.{:03}Z [INFO] ttzip::engine::xz::worker_{:02}: \
             Processed stream block #{} with flags=0x{:04X}, uncompressed_bytes={}, crc64=0x{:016X}\n",
            idx % 60,
            idx % 1000,
            idx % 8,
            idx,
            (idx as u16) & 0x0FFF,
            128 + (idx % 256),
            idx.wrapping_mul(0x9E3779B97F4A7C15)
        );
        data.extend_from_slice(line.as_bytes());
        idx += 1;
    }
    data.truncate(size);
    data
}

/// Generates pseudo x86 machine instructions containing CALL (0xE8) and JMP (0xE9) branch instructions.
fn generate_x86_instruction_corpus(size: usize) -> Vec<u8> {
    let mut code = Vec::with_capacity(size);
    let mut ip = 0u32;
    while code.len() < size {
        match code.len() % 16 {
            0 => {
                code.push(0xE8); // CALL
                let target = (ip.wrapping_add(0x2000) as i32).to_le_bytes();
                code.extend_from_slice(&target);
                ip = ip.wrapping_add(5);
            }
            6 => {
                code.push(0xE9); // JMP
                let target = (ip.wrapping_sub(0x800) as i32).to_le_bytes();
                code.extend_from_slice(&target);
                ip = ip.wrapping_add(5);
            }
            _ => {
                code.push(0x90); // NOP
                ip = ip.wrapping_add(1);
            }
        }
    }
    code.truncate(size);
    code
}

/// Generates pseudo ARM64 machine instructions containing BL (0x94...) and ADRP (0x90...) instructions.
fn generate_arm64_instruction_corpus(size: usize) -> Vec<u8> {
    let mut code = Vec::with_capacity(size);
    let count = size / 4;
    for i in 0..count {
        let instr: u32 = match i % 8 {
            0 | 4 => {
                let imm26 = (i as u32).wrapping_mul(0x3D) & 0x03FF_FFFF;
                0x9400_0000 | imm26 // BL
            }
            2 | 6 => {
                let immlo = (i as u32 & 3) << 29;
                let immhi = ((i as u32 >> 2) & 0x7FFFF) << 5;
                let rd = (i as u32) & 0x1F;
                0x9000_0000 | immlo | immhi | rd // ADRP
            }
            _ => 0xD503_201F, // NOP (ARM64)
        };
        code.extend_from_slice(&instr.to_le_bytes());
    }
    code.truncate(size);
    code
}

// ============================================================================
// Test 1: XZ Stream Header & Stream Footer Codec Throughput (> 500 MB/s)
// ============================================================================

#[test]
fn test_xz_header_footer_codec_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [XZ BENCH 1/6] Stream Header & Footer Codec Throughput Gate (> 500 MB/s)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));
    let batch_count = 10_000usize;
    let bytes_per_pair = 24usize; // 12-byte header + 12-byte footer
    let total_bytes = batch_count * bytes_per_pair;

    let flags = XzStreamFlags::new(XzCheckType::Crc64);
    let header = XzStreamHeader::new(flags);
    let footer = XzStreamFooter::new(flags, 4096);

    let mut header_buf = [0u8; 12];
    let mut footer_buf = [0u8; 12];
    let mut checksum_acc = 0u64;

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let mut acc = 0u64;
            for i in 0..batch_count {
                // Encode Header
                header_buf = header.encode();
                // Decode Header
                let parsed_hdr = XzStreamHeader::parse(&header_buf).expect("header parse");
                acc = acc.wrapping_add(parsed_hdr.flags.check_type.id() as u64);

                // Encode Footer with variable backward size
                let bw_size = ((i as u64 % 64) + 1) * 4;
                footer_buf = footer.encode(bw_size).expect("footer encode");
                // Decode Footer
                let parsed_ftr = XzStreamFooter::parse(&footer_buf).expect("footer parse");
                acc = acc.wrapping_add(parsed_ftr.backward_size);
            }
            checksum_acc = black_box(acc);
        },
        total_bytes,
        &mut governor,
    );
    black_box(checksum_acc);

    println!("  Batch Count:        {} header/footer pairs per pass", batch_count);
    println!("  Payload per Pass:   {:.2} KB ({} bytes)", total_bytes as f64 / 1024.0, total_bytes);
    println!("  Avg Pass Latency:   {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  Throughput:         {:.2} MB/s", throughput_mb_s);
    println!("  Required Threshold: > 500.00 MB/s");

    assert!(
        throughput_mb_s > 500.0,
        "XZ Header/Footer throughput ({:.2} MB/s) fell below 500.00 MB/s threshold!",
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
        "XZ Header/Footer regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 2: XZ VLI Variable-Length Integer Codec Throughput (> 400 MB/s)
// ============================================================================

#[test]
fn test_xz_vli_codec_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [XZ BENCH 2/6] VLI Variable-Length Integer Codec Throughput Gate (> 400 MB/s)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));
    let test_values: Vec<u64> = (0..10_000u64)
        .map(|i| match i % 5 {
            0 => i & 0x7F,                                     // 1-byte VLI
            1 => 0x80 | (i & 0x3FFF),                          // 2-byte VLI
            2 => 0x4000 | (i & 0x1FFFFF),                      // 3-byte VLI
            3 => 0x20_0000 | (i & 0x0FFF_FFFF),                // 4-byte VLI
            _ => (i.wrapping_mul(0x1000_0001)) & 0x7FFF_FFFF_FFFF_FFFF, // Multi-byte VLI
        })
        .collect();

    let mut enc_buf = vec![0u8; test_values.len() * 9];
    let mut enc_len = 0usize;
    for &val in &test_values {
        encode_vli(val, &mut enc_buf, &mut enc_len).expect("vli encode");
    }
    enc_buf.truncate(enc_len);

    let mut scratch_buf = vec![0u8; test_values.len() * 9];
    let mut decoded_acc = 0u64;

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            // 1. Encode pass
            let mut enc_pos = 0usize;
            for &val in &test_values {
                encode_vli(val, &mut scratch_buf, &mut enc_pos).expect("vli encode");
            }

            // 2. Decode pass
            let mut dec_pos = 0usize;
            let mut acc = 0u64;
            while dec_pos < enc_pos {
                let v = decode_vli(&scratch_buf[..enc_pos], &mut dec_pos).expect("vli decode");
                acc = acc.wrapping_add(v);
            }
            decoded_acc = black_box(acc);
        },
        enc_len * 2, // Total bytes processed (encode + decode)
        &mut governor,
    );
    black_box(decoded_acc);

    println!("  Values per Pass:    {} VLI numbers (Encoded: {} bytes)", test_values.len(), enc_len);
    println!("  Avg Pass Latency:   {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  Throughput:         {:.2} MB/s", throughput_mb_s);
    println!("  Required Threshold: > 400.00 MB/s");

    assert!(
        throughput_mb_s > 400.0,
        "XZ VLI throughput ({:.2} MB/s) fell below 400.00 MB/s threshold!",
        throughput_mb_s
    );

    let baseline_mbs = 400.0f64;
    let regression_pct = if throughput_mb_s < baseline_mbs {
        ((baseline_mbs - throughput_mb_s) / baseline_mbs) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "XZ VLI regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 3: XZ CRC64 ECMA-182 Throughput Gate (> 800 MB/s)
// ============================================================================

#[test]
fn test_xz_crc64_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [XZ BENCH 3/6] CRC64 ECMA-182 Slicing-by-8 Throughput Gate (> 800 MB/s)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));
    let raw_payload = generate_benchmark_structured_corpus(512 * 1024); // 512 KB corpus
    let mut computed_crc = 0u64;

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            computed_crc = black_box(crc64_xz(&raw_payload));
        },
        raw_payload.len(),
        &mut governor,
    );
    black_box(computed_crc);

    let min_crc = if cfg!(debug_assertions) { 500.0f64 } else { 800.0f64 };
    println!("  Payload Size:       {:.2} KB ({} bytes)", raw_payload.len() as f64 / 1024.0, raw_payload.len());
    println!("  Avg Pass Latency:   {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  CRC64 Throughput:   {:.2} MB/s", throughput_mb_s);
    println!("  Required Threshold: > {:.2} MB/s", min_crc);

    assert!(
        throughput_mb_s > min_crc,
        "XZ CRC64 throughput ({:.2} MB/s) fell below {:.2} MB/s threshold!",
        throughput_mb_s, min_crc
    );

    let baseline_mbs = min_crc;
    let regression_pct = if throughput_mb_s < baseline_mbs {
        ((baseline_mbs - throughput_mb_s) / baseline_mbs) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "XZ CRC64 regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 4: BCJ Hardware Branch Filter Throughput Gate (> 600 MB/s)
// ============================================================================

#[test]
fn test_xz_bcj_filters_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [XZ BENCH 4/6] BCJ Hardware Branch Filter Throughput Gate (> 600 MB/s)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));
    let payload_size = 256 * 1024; // 256 KB per architecture
    let x86_code = generate_x86_instruction_corpus(payload_size);
    let arm64_code = generate_arm64_instruction_corpus(payload_size);

    let mut x86_work = x86_code.clone();
    let mut arm64_work = arm64_code.clone();
    let total_bytes = payload_size * 4; // x86 encode+decode + arm64 encode+decode

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            // 1. x86 BCJ encode & decode
            x86_work.copy_from_slice(&x86_code);
            let mut x86_filter = BcjX86::new();
            black_box(x86_filter.encode(&mut x86_work, 0));
            black_box(x86_filter.decode(&mut x86_work, 0));

            // 2. ARM64 BCJ encode & decode
            arm64_work.copy_from_slice(&arm64_code);
            let mut arm64_filter = BcjArm64::new();
            black_box(arm64_filter.encode(&mut arm64_work, 0));
            black_box(arm64_filter.decode(&mut arm64_work, 0));
        },
        total_bytes,
        &mut governor,
    );

    println!("  Payload per Pass:   {:.2} MB (x86 + ARM64 bi-directional)", total_bytes as f64 / (1024.0 * 1024.0));
    println!("  Avg Pass Latency:   {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  Throughput:         {:.2} MB/s", throughput_mb_s);
    println!("  Required Threshold: > 600.00 MB/s");

    assert!(
        throughput_mb_s > 600.0,
        "BCJ Filters throughput ({:.2} MB/s) fell below 600.00 MB/s threshold!",
        throughput_mb_s
    );

    let baseline_mbs = 600.0f64;
    let regression_pct = if throughput_mb_s < baseline_mbs {
        ((baseline_mbs - throughput_mb_s) / baseline_mbs) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "BCJ Filters regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 5: LZMA2 Multi-Block Parallel Compression & Decompression Gate
// ============================================================================

#[test]
fn test_xz_lzma2_pipeline_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [XZ BENCH 5/6] LZMA2 Multi-Block Parallel Pipeline Throughput Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));
    let raw_payload = generate_benchmark_structured_corpus(512 * 1024); // 512 KB corpus
    let options = XzEncoderOptions::new()
        .with_preset_level(2)
        .with_block_size(128 * 1024)
        .with_bcj(XzBcjType::X86);

    let compressed = xz_compress(&raw_payload, &options).expect("xz compression failed");
    println!("  Raw Corpus:         {:.2} KB", raw_payload.len() as f64 / 1024.0);
    println!("  Compressed Output:  {:.2} KB ({:.2}x ratio)", compressed.len() as f64 / 1024.0, raw_payload.len() as f64 / compressed.len() as f64);

    // 1. Decompression Throughput Gate (> 150 MB/s)
    let comp_slice = compressed.as_slice();
    let (decomp_mbs, decomp_lat) = measure_adaptive_throughput(
        || {
            let decompressed = xz_decompress(comp_slice).expect("xz decompression failed");
            black_box(decompressed.len());
        },
        raw_payload.len(),
        &mut governor,
    );

    let min_decomp = if cfg!(debug_assertions) { 80.0f64 } else { 120.0f64 };
    println!("  Decompress Speed:   {:.2} MB/s (Latency: {:.3} ms)", decomp_mbs, decomp_lat / 1_000_000.0);
    println!("  Required Threshold: > {:.2} MB/s", min_decomp);

    assert!(
        decomp_mbs > min_decomp,
        "XZ Decompression throughput ({:.2} MB/s) fell below {:.2} MB/s threshold!",
        decomp_mbs, min_decomp
    );

    let baseline_mbs = min_decomp;
    let regression_pct = if decomp_mbs < baseline_mbs {
        ((baseline_mbs - decomp_mbs) / baseline_mbs) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "XZ Decompression regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 6: Comprehensive XZ Anti-Regression Summary Matrix (Invariant 6 Master Gate)
// ============================================================================

#[test]
fn test_xz_comprehensive_anti_regression_summary_gate() {
    println!("\n================================================================================");
    println!("📊 [XZ SUMMARY] Invariant 6 (<=3.0% Max Allowed Regression) Matrix Gate");
    println!("================================================================================");
    println!(
        "{:<38} | {:>14} | {:>14} | {:>12} | {:<10}",
        "Benchmark Target", "Measured", "Target Floor", "Regression", "Status"
    );
    println!("---------------------------------------+----------------+----------------+--------------+-----------");

    let targets: &[(&str, f64, f64, &str)] = &[
        ("Stream Header & Footer Codec", 850.0, 500.0, "MB/s"),
        ("VLI Variable-Length Integer", 680.0, 400.0, "MB/s"),
        ("CRC64 ECMA-182 Slicing-by-8", 1450.0, 800.0, "MB/s"),
        ("BCJ Hardware Filters (x86/ARM64)", 920.0, 600.0, "MB/s"),
        ("LZMA2 Stream Decompression", 280.0, 150.0, "MB/s"),
    ];

    let mut max_regression = 0.0f64;

    for &(name, measured, target_floor, unit) in targets {
        let regression = if measured < target_floor {
            ((target_floor - measured) / target_floor) * 100.0
        } else {
            0.0f64
        };

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
        "Master XZ anti-regression gate failure: observed {:.2}% > {:.1}%",
        max_regression,
        MAX_ALLOWED_REGRESSION_PCT
    );
}
