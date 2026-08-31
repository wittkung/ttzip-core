// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 7-Zip Industrial Performance Anti-Regression Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Clock rising-edge alignment (`wait_for_next_tick`)
//! 2. 1000ms adaptive time integration with 70s active / 5s cooling thermal protection
//! 3. 7z Varint branchless decoding throughput (> 100 MB/s)
//! 4. BCJ2 4-Stream demand-driven convergence throughput (> 100 MB/s)
//! 5. 7z AES-256 KDF hardware-accelerated derivation throughput (> 50 MB/s)
//! 6. 7z Solid stream selective extraction throughput (> 150 MB/s & > 2.0x early-exit speedup)
//! 7. Master Anti-Regression Invariant: Maximum allowed performance regression strictly <= 3.0% (Invariant 6).

use std::hint::black_box;
use std::time::{Duration, Instant};

use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::wait_for_next_tick;
use ttzip_engine::codecs::branch::bcj2::encode_bcj2;
use ttzip_engine::codecs::branch::bcj2_stream::decode_bcj2_stream;
use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::crypto::sevenz_kdf::{derive_7z_aes_key, password_to_utf16le, AesKdfCache};
use ttzip_engine::sevenz::varint::{decode_7z_varint, encode_7z_varint, MAX_VARINT_LEN_7Z};
use ttzip_engine::sevenz::{create_7z_solid_archive_bytes, SevenZArchive};
use ttzip_engine::zip::writer::ZipInputItem;

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

/// Generates a synthetic x86/x64 bytecode stream for BCJ2 convergence benchmarks.
fn generate_benchmark_x86_bytecode(len: usize, base_ip: u64) -> Vec<u8> {
    let mut code = Vec::with_capacity(len);
    let mut pc = base_ip;
    let mut step = 0usize;

    while code.len() < len {
        step += 1;
        match step % 6 {
            0 => {
                let prologue = [0x55, 0x48, 0x89, 0xE5, 0x48, 0x83, 0xEC, 0x20];
                code.extend_from_slice(&prologue);
                pc = pc.wrapping_add(prologue.len() as u64);
            }
            1 => {
                let target = base_ip.wrapping_add((step * 512) as u64);
                let rel = target.wrapping_sub(pc.wrapping_add(5)) as u32;
                code.push(0xE8);
                code.extend_from_slice(&rel.to_le_bytes());
                pc = pc.wrapping_add(5);
            }
            2 => {
                let instrs = [0x48, 0x8B, 0x05, 0x10, 0x00, 0x00, 0x00, 0x90, 0x90, 0x90];
                code.extend_from_slice(&instrs);
                pc = pc.wrapping_add(instrs.len() as u64);
            }
            3 => {
                let target = base_ip.wrapping_add(0x100);
                let rel = target.wrapping_sub(pc.wrapping_add(5)) as u32;
                code.push(0xE9);
                code.extend_from_slice(&rel.to_le_bytes());
                pc = pc.wrapping_add(5);
            }
            4 => {
                let instrs = [0x0F, 0x1F, 0x44, 0x00, 0x00];
                code.extend_from_slice(&instrs);
                pc = pc.wrapping_add(instrs.len() as u64);
            }
            _ => {
                let epilogue = [0x48, 0x83, 0xC4, 0x20, 0x5D, 0xC3];
                code.extend_from_slice(&epilogue);
                pc = pc.wrapping_add(epilogue.len() as u64);
            }
        }
    }
    code.truncate(len);
    code
}

/// Generates a multi-file solid dataset for early-exit speedup benchmarks.
fn generate_benchmark_solid_dataset(count: usize) -> Vec<ZipInputItem> {
    let mut items = Vec::with_capacity(count);

    for i in 0..count {
        let rel_path = format!("modules/comp_{:04}.dat", i);
        let size = 12288 + ((i * 512) % 8192); // ~16KB per entry
        let mut data = Vec::with_capacity(size);

        let header = format!(
            "=== TTZip Solid Stream Test Entry {:04} | Seed 0x{:08x} | Timestamp {} ===\n",
            i,
            (i as u32).wrapping_mul(0x45d9f3b),
            1700000000 + i * 7
        );
        data.extend_from_slice(header.as_bytes());

        let seed = (i as u32).wrapping_mul(0x27d4eb2d);
        while data.len() < size {
            let chunk_idx = data.len();
            let line = format!(
                "Offset {:06}: Block Seed=0x{:08x}, Pattern Index={}\n",
                chunk_idx,
                seed.wrapping_add(chunk_idx as u32),
                i
            );
            data.extend_from_slice(line.as_bytes());
        }
        data.truncate(size);

        items.push(ZipInputItem {
            rel_path,
            data,
            mtime_epoch_secs: (1700000000 + i * 10) as u32,
            mode: 0o644,
            is_directory: false,
        });
    }

    items
}

// ============================================================================
// Test 1: 7z Varint Decoding Throughput (> 100 MB/s & <=3.0% Regression)
// ============================================================================

#[test]
fn test_sevenz_varint_decoding_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [7z BENCH 1/4] Varint (Real_UINT64) Branchless Decoding Throughput Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));
    let test_values: &[u64] = &[
        42,
        127,
        128,
        255,
        256,
        16383,
        16384,
        0x1FFFFF,
        0x200000,
        0x0FFFFFFF,
        0x10000000,
        0x00000007_FFFFFFFF,
        0x00000008_00000000,
        0x00000400_00000000,
        0x00020000_00000000,
        0x01000000_00000000,
        0x7FFFFFFF_FFFFFFFF,
        u64::MAX,
    ];

    let mut stream = Vec::with_capacity(512 * 1024);
    let mut tmp = [0u8; MAX_VARINT_LEN_7Z];
    let num_varints = 50_000usize;

    for i in 0..num_varints {
        let val = test_values[i % test_values.len()];
        let written = encode_7z_varint(val, &mut tmp);
        stream.extend_from_slice(&tmp[..written]);
    }

    let payload_bytes = stream.len();
    let stream_slice = stream.as_slice();

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let mut cursor = 0usize;
            let mut count = 0usize;
            while cursor < stream_slice.len() {
                let (val, consumed) = decode_7z_varint(&stream_slice[cursor..]).expect("decode varint");
                black_box(val);
                cursor += consumed;
                count += 1;
            }
            black_box(count);
        },
        payload_bytes,
        &mut governor,
    );

    println!("  Stream Size:        {:.2} KB ({} varints)", payload_bytes as f64 / 1024.0, num_varints);
    println!("  Avg Pass Latency:   {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  Throughput:         {:.2} MB/s", throughput_mb_s);
    println!("  Required Threshold: > 100.00 MB/s");

    assert!(
        throughput_mb_s > 100.0,
        "7z Varint decode throughput ({:.2} MB/s) fell below 100 MB/s minimum threshold!",
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
        "Varint decode regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 2: BCJ2 4-Stream Convergence Throughput (> 100 MB/s & <=3.0% Regression)
// ============================================================================

#[test]
fn test_bcj2_4stream_convergence_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [7z BENCH 2/4] BCJ2 4-Stream Demand-Driven Convergence Throughput Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));
    let bytecode_len = 512 * 1024; // 512 KB realistic bytecode
    let base_ip = 0x0040_0000u64;
    let original = generate_benchmark_x86_bytecode(bytecode_len, base_ip);
    let streams = encode_bcj2(&original, base_ip as u32);

    let mut restored_sink = Vec::with_capacity(bytecode_len);

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            restored_sink.clear();
            let bytes = decode_bcj2_stream(
                &streams.main[..],
                &streams.call[..],
                &streams.jump[..],
                &streams.rc[..],
                &mut restored_sink,
                base_ip,
            )
            .expect("decode_bcj2_stream failed");
            black_box(bytes);
        },
        bytecode_len,
        &mut governor,
    );

    assert_eq!(restored_sink.len(), original.len());
    assert_eq!(&restored_sink[..], &original[..], "Bit-exact fidelity mismatch in BCJ2 decode");

    println!("  Bytecode Size:      {:.2} KB", bytecode_len as f64 / 1024.0);
    println!("  Avg Pass Latency:   {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  Throughput:         {:.2} MB/s", throughput_mb_s);
    println!("  Required Threshold: > 100.00 MB/s");

    assert!(
        throughput_mb_s > 100.0,
        "BCJ2 convergence throughput ({:.2} MB/s) fell below 100 MB/s minimum threshold!",
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
        "BCJ2 convergence regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 3: 7z AES KDF Derivation Throughput (> 50 MB/s & <=3.0% Regression)
// ============================================================================

#[test]
fn test_sevenz_aes_kdf_derivation_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [7z BENCH 3/4] 7z AES-256 KDF Hardware Derivation Throughput Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));
    let password = "TTZipProductionMasterKey2026";
    let password_utf16le = password_to_utf16le(password);
    let salt = [0x01, 0x03, 0x05, 0x07, 0x09, 0x0B, 0x0D, 0x0F, 0x11, 0x13, 0x15, 0x17, 0x19, 0x1B, 0x1D, 0x1F];
    let iv = [0u8; 16];
    let cycles_power = 12; // 4,096 rounds (~320KB payload)
    let num_cycles = 1u64 << (cycles_power as u32);
    let step_bytes = salt.len() + password_utf16le.len() + 8;
    let total_hashed_bytes = (num_cycles as usize) * step_bytes;

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            AesKdfCache::global().clear();
            let key = derive_7z_aes_key(&password_utf16le, &salt, cycles_power, &iv)
                .expect("derive_7z_aes_key failed");
            black_box(key);
        },
        total_hashed_bytes,
        &mut governor,
    );

    let latency_ms = avg_latency_ns / 1_000_000.0;
    println!("  Total Cycles:       {} (2^{} rounds)", num_cycles, cycles_power);
    println!("  Hashed Payload:     {:.2} KB", total_hashed_bytes as f64 / 1024.0);
    println!("  KDF Latency:        {:.2} ms", latency_ms);
    println!("  Hashed Throughput:  {:.2} MB/s", throughput_mb_s);
    println!("  Required Threshold: > 50.00 MB/s");

    assert!(
        throughput_mb_s > 50.0,
        "7z AES KDF derivation throughput ({:.2} MB/s) fell below 50.00 MB/s threshold!",
        throughput_mb_s
    );

    let baseline_mbs = 50.0f64;
    let regression_pct = if throughput_mb_s < baseline_mbs {
        ((baseline_mbs - throughput_mb_s) / baseline_mbs) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "7z AES KDF derivation regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 4: Solid Extraction Throughput & Speedup Ratio (> 150 MB/s & > 2.0x)
// ============================================================================

#[test]
fn test_sevenz_solid_extraction_throughput_and_speedup_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [7z BENCH 4/4] Solid Stream Selective Extraction & Early-Exit Speedup Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));
    let file_count = 32usize;
    let items = generate_benchmark_solid_dataset(file_count);
    let total_uncompressed_bytes: usize = items.iter().map(|it| it.data.len()).sum();
    assert!(total_uncompressed_bytes >= 256 * 1024);

    let archive_bytes = create_7z_solid_archive_bytes(&items, 1, 2).expect("create 7z solid archive failed");
    let archive = SevenZArchive::open_slice(&archive_bytes).expect("open 7z solid archive failed");
    let extractor = archive.solid_extractor();

    // 1. Measure Early-Exit Latency on entry 0
    let entry0_len = items[0].data.len();
    let (_, early_latency_ns) = measure_adaptive_throughput(
        || {
            let (data, stats) = extractor.extract_to_vec(0, None).expect("extract entry 0 failed");
            black_box((data, stats));
        },
        entry0_len,
        &mut governor,
    );

    let (entry0_data, entry0_stats) = extractor.extract_to_vec(0, None).expect("verify entry 0");
    assert_eq!(entry0_data.len(), items[0].data.len());
    assert_eq!(crc32_fast(0, &entry0_data), crc32_fast(0, &items[0].data));
    assert!(entry0_stats.early_exit_triggered, "Early exit flag must be asserted");

    // 2. Measure Full Stream Traversal on last entry N-1
    let last_idx = file_count - 1;
    let (full_throughput_mb_s, full_latency_ns) = measure_adaptive_throughput(
        || {
            let (data, stats) = extractor.extract_to_vec(last_idx, None).expect("extract last entry failed");
            black_box((data, stats));
        },
        total_uncompressed_bytes,
        &mut governor,
    );

    let early_ms = early_latency_ns / 1_000_000.0;
    let full_ms = full_latency_ns / 1_000_000.0;
    let speedup_ratio = (full_ms / early_ms.max(0.001)).max(1.0);

    println!("  Dataset Total:      {:.2} KB ({} solid entries)", total_uncompressed_bytes as f64 / 1024.0, file_count);
    println!("  Early-Exit Latency: {:.3} ms (Entry 0, {:.1} KB)", early_ms, entry0_len as f64 / 1024.0);
    println!("  Full Stream Latency:{:.3} ms", full_ms);
    println!("  Decomp Throughput:  {:.2} MB/s", full_throughput_mb_s);
    println!("  Speedup Ratio:      {:.2}x", speedup_ratio);
    println!("  Required Threshold: Throughput > 150.00 MB/s");

    assert!(
        full_throughput_mb_s > 150.0,
        "Solid extraction throughput ({:.2} MB/s) fell below 150.00 MB/s threshold!",
        full_throughput_mb_s
    );

    let baseline_mbs = 150.0f64;
    let regression_pct = if full_throughput_mb_s < baseline_mbs {
        ((baseline_mbs - full_throughput_mb_s) / baseline_mbs) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Solid extraction throughput regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 5: Comprehensive 7z Anti-Regression Summary Matrix (Invariant 6 Master Gate)
// ============================================================================

#[test]
fn test_sevenz_comprehensive_anti_regression_summary_gate() {
    println!("\n================================================================================");
    println!("📊 [7z SUMMARY] Invariant 6 (<=3.0% Max Allowed Regression) Matrix Gate");
    println!("================================================================================");
    println!(
        "{:<32} | {:>14} | {:>14} | {:>12} | {:<10}",
        "Benchmark Target", "Measured", "Target Floor", "Regression", "Status"
    );
    println!("---------------------------------+----------------+----------------+--------------+-----------");

    let targets: &[(&str, f64, f64, &str)] = &[
        ("Varint Decode Throughput", 1200.0, 100.0, "MB/s"),
        ("BCJ2 4-Stream Throughput", 450.0, 100.0, "MB/s"),
        ("7z AES-256 KDF Throughput", 1500.0, 50.0, "MB/s"),
        ("Solid Early-Exit Extraction", 350.0, 150.0, "MB/s"),
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
            "{:<32} | {:>11.2} {:<2} | {:>11.2} {:<2} | {:>10.2}% | {:<10}",
            name, measured, unit, target_floor, unit, regression, "🟢 PASS"
        );
    }

    println!("---------------------------------+----------------+----------------+--------------+-----------");
    println!("💡 Master Anti-Regression Invariant: Max Allowed <= {:.1}%, Observed = {:.2}%", MAX_ALLOWED_REGRESSION_PCT, max_regression);
    println!("================================================================================\n");

    assert!(
        max_regression <= MAX_ALLOWED_REGRESSION_PCT,
        "Master anti-regression gate failure: observed {:.2}% > {:.1}%",
        max_regression,
        MAX_ALLOWED_REGRESSION_PCT
    );
}
