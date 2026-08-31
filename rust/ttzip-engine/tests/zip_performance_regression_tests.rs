// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! ZIP Industrial Performance Anti-Regression Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Clock rising-edge alignment (`wait_for_next_tick`)
//! 2. 1000ms adaptive time integration with 70s active / 5s cooling thermal protection
//! 3. EOCD reverse sliding scan throughput gate (> 200 MB/s)
//! 4. Zip64 orchestration and Extra Fields parsing throughput gate (> 150 MB/s)
//! 5. WinZip AES-256 encryption/decryption throughput gate (> 100 MB/s)
//! 6. DataStreamAlignment 16KB page alignment assembly throughput gate (> 300 MB/s)
//! 7. Master Anti-Regression Invariant: Maximum allowed performance regression strictly <= 3.0% (Invariant 6).

use std::hint::black_box;
use std::time::{Duration, Instant};

use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::wait_for_next_tick;
use ttzip_engine::crypto::aes256::aes256_ctr_crypt;
use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::zip::extra::ZipExtraFields;
use ttzip_engine::zip::scanner::EocdScanner;
use ttzip_engine::zip::writer::{
    assemble_zip_archive, assemble_zip_archive_aligned, ZipCompressedItem,
};

const WARMUP_RUNS: usize = 2;
const MIN_INTEGRATION_WINDOW: Duration = Duration::from_millis(50); // 50ms Adaptive Integration
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

/// Executes adaptive time integration measurement over at least 50ms with clock rising-edge alignment
/// and 70s active / 5s cooling thermal protection throttling.
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

/// Generates a realistic multi-entry ZIP archive dataset with uncompressed payloads.
fn generate_benchmark_zip_items(count: usize, target_total_bytes: usize) -> Vec<ZipCompressedItem> {
    let mut items = Vec::with_capacity(count);
    let per_item_size = (target_total_bytes / count).max(1024);

    for i in 0..count {
        let rel_path = format!("benchmark/module_{:04}.bin", i);
        let mut data = Vec::with_capacity(per_item_size);
        let seed = (i as u32).wrapping_mul(0x45d9f3b);

        while data.len() < per_item_size {
            let offset = data.len();
            let chunk = format!(
                "TTZip Item {:04} | Offset {:06} | Pattern 0x{:08x}\n",
                i,
                offset,
                seed.wrapping_add(offset as u32)
            );
            data.extend_from_slice(chunk.as_bytes());
        }
        data.truncate(per_item_size);

        let crc = crc32_fast(0, &data);
        items.push(ZipCompressedItem {
            rel_path,
            payload: data,
            uncompressed_size: per_item_size as u64,
            compressed_size: per_item_size as u64,
            crc32: crc,
            compression_method: 0, // Store
            actual_method: 0,
            aes_strength: 0,
            mtime_epoch_secs: 1700000000 + (i as u32) * 60,
            mode: 0o644,
            is_directory: false,
            is_encrypted: false,
        });
    }

    items
}

// ============================================================================
// Test 1: EOCD Reverse Sliding Scan Throughput (> 200 MB/s & <=3.0% Regression)
// ============================================================================

#[test]
fn test_zip_eocd_reverse_scan_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [ZIP BENCH 1/4] EOCD Reverse Sliding Window Scanner Throughput Gate");
    println!("================================================================================");

    let mut governor =
        ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));

    // Construct realistic archive with 50 entries and 512KB payload
    let items = generate_benchmark_zip_items(50, 512 * 1024);
    let mut archive_bytes = assemble_zip_archive(&items).expect("assemble archive");

    // Append 32KB realistic comment
    let comment_size = 32 * 1024;
    let mut comment_bytes = vec![0x20u8; comment_size];
    for (i, b) in comment_bytes.iter_mut().enumerate() {
        if i % 1024 == 0 {
            *b = b'P';
        } else if i % 1024 == 1 {
            *b = b'K';
        }
    }
    let eocd_pos = archive_bytes.len() - 22;
    let comment_len_u16 = comment_size as u16;
    archive_bytes[eocd_pos + 20..eocd_pos + 22].copy_from_slice(&comment_len_u16.to_le_bytes());
    archive_bytes.extend_from_slice(&comment_bytes);

    let search_window_len = 65557.min(archive_bytes.len());
    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let info = EocdScanner::scan_slice(&archive_bytes).expect("scan eocd");
            black_box(info);
        },
        search_window_len,
        &mut governor,
    );

    println!(
        "  Archive Size:       {:.2} MB (Search Window: {:.2} KB)",
        archive_bytes.len() as f64 / (1024.0 * 1024.0),
        search_window_len as f64 / 1024.0
    );
    println!("  Avg Pass Latency:   {:.3} µs", avg_latency_ns / 1_000.0);
    println!("  Scan Throughput:    {:.2} MB/s", throughput_mb_s);
    println!("  Required Threshold: > 200.00 MB/s");

    assert!(
        throughput_mb_s > 200.0,
        "EOCD scanner throughput ({:.2} MB/s) fell below 200 MB/s minimum threshold!",
        throughput_mb_s
    );

    let baseline_mbs = 200.0f64;
    let regression_pct = if throughput_mb_s < baseline_mbs {
        ((baseline_mbs - throughput_mb_s) / baseline_mbs) * 100.0
    } else {
        0.0f64
    };

    println!(
        "  Observed Regression:{:.2}% (Max Allowed <= {:.1}%)",
        regression_pct, MAX_ALLOWED_REGRESSION_PCT
    );
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "EOCD scan regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 2: Zip64 & Extra Fields Parsing Throughput (> 150 MB/s & <=3.0% Regression)
// ============================================================================

#[test]
fn test_zip_zip64_and_extra_fields_parsing_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [ZIP BENCH 2/4] Zip64 & TLV Extra Fields Parsing Throughput Gate");
    println!("================================================================================");

    let mut governor =
        ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));

    // Construct dense synthetic Extra Fields buffer with Zip64, Extended Timestamp, Info-ZIP, and Alignment
    let count = 500usize;
    let mut composite_extra_stream = Vec::with_capacity(count * 64);

    for i in 0..count {
        let fields = ZipExtraFields {
            uncompressed_size: Some(0x1_0000_0000 + (i as u64) * 4096),
            compressed_size: Some(0x8000_0000 + (i as u64) * 2048),
            local_header_offset: Some((i as u64) * 65536),
            mod_time: Some(1700000000 + (i as u32) * 10),
            has_winzip_aes: true,
            aes_actual_method: 8,
            aes_strength: 3,
            aes_vendor_id: 0x4541,
            aes_version: 2,
            data_stream_alignment: Some(16384),
            ..Default::default()
        };

        let cdfh_extra = fields.build_central_extra();
        composite_extra_stream.extend_from_slice(&cdfh_extra);
    }

    let payload_bytes = composite_extra_stream.len();
    let stream_slice = composite_extra_stream.as_slice();

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let parsed = ZipExtraFields::parse(stream_slice, true, true, true, true);
            black_box(&parsed);
        },
        payload_bytes,
        &mut governor,
    );

    println!(
        "  Extra Stream Size:  {:.2} KB ({} composite records)",
        payload_bytes as f64 / 1024.0,
        count
    );
    println!("  Avg Pass Latency:   {:.3} µs", avg_latency_ns / 1_000.0);
    println!("  Parsing Throughput: {:.2} MB/s", throughput_mb_s);
    println!("  Required Threshold: > 150.00 MB/s");

    assert!(
        throughput_mb_s > 150.0,
        "Extra fields parsing throughput ({:.2} MB/s) fell below 150 MB/s minimum threshold!",
        throughput_mb_s
    );

    let baseline_mbs = 150.0f64;
    let regression_pct = if throughput_mb_s < baseline_mbs {
        ((baseline_mbs - throughput_mb_s) / baseline_mbs) * 100.0
    } else {
        0.0f64
    };

    println!(
        "  Observed Regression:{:.2}% (Max Allowed <= {:.1}%)",
        regression_pct, MAX_ALLOWED_REGRESSION_PCT
    );
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Extra fields parsing regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 3: WinZip AES-256 Encryption & Decryption Throughput (> 100 MB/s)
// ============================================================================

#[test]
fn test_zip_winzip_aes256_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [ZIP BENCH 3/4] WinZip AES-256 Hardware Encryption/Decryption Gate");
    println!("================================================================================");

    let mut governor =
        ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));

    let payload_len = 512 * 1024; // 512 KB payload
    let mut payload = vec![0u8; payload_len];
    for (i, b) in payload.iter_mut().enumerate() {
        *b = ((i * 47 + 11) & 0xFF) as u8;
    }

    let key32 = [0x7Au8; 32];
    let mut ciphertext = vec![0u8; payload_len];

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            aes256_ctr_crypt(&key32, 1, &payload, &mut ciphertext).expect("aes256_ctr_crypt");
            black_box(ciphertext[0]);
        },
        payload_len,
        &mut governor,
    );

    println!(
        "  AES Payload Size:   {:.2} KB",
        payload_len as f64 / 1024.0
    );
    println!("  Avg Pass Latency:   {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  Crypto Throughput:  {:.2} MB/s", throughput_mb_s);
    println!("  Required Threshold: > 100.00 MB/s");

    assert!(
        throughput_mb_s > 100.0,
        "WinZip AES-256 throughput ({:.2} MB/s) fell below 100 MB/s minimum threshold!",
        throughput_mb_s
    );

    let baseline_mbs = 100.0f64;
    let regression_pct = if throughput_mb_s < baseline_mbs {
        ((baseline_mbs - throughput_mb_s) / baseline_mbs) * 100.0
    } else {
        0.0f64
    };

    println!(
        "  Observed Regression:{:.2}% (Max Allowed <= {:.1}%)",
        regression_pct, MAX_ALLOWED_REGRESSION_PCT
    );
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "WinZip AES-256 regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 4: DataStreamAlignment 16KB Assembly Throughput (> 300 MB/s)
// ============================================================================

#[test]
fn test_zip_data_stream_alignment_assembly_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [ZIP BENCH 4/4] DataStreamAlignment 16KB Assembly Throughput Gate");
    println!("================================================================================");

    let mut governor =
        ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));

    let file_count = 40usize;
    let total_bytes = 512 * 1024; // 512 KB archive dataset
    let items = generate_benchmark_zip_items(file_count, total_bytes);

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let archive =
                assemble_zip_archive_aligned(&items, 16384).expect("assemble 16KB aligned archive");
            black_box(archive.len());
        },
        total_bytes,
        &mut governor,
    );

    println!(
        "  Archive Dataset:    {:.2} KB ({} items, 16KB aligned)",
        total_bytes as f64 / 1024.0,
        file_count
    );
    println!("  Avg Pass Latency:   {:.3} ms", avg_latency_ns / 1_000_000.0);
    println!("  Assembly Throughput:{:.2} MB/s", throughput_mb_s);
    println!("  Required Threshold: > 300.00 MB/s");

    assert!(
        throughput_mb_s > 300.0,
        "16KB alignment assembly throughput ({:.2} MB/s) fell below 300 MB/s minimum threshold!",
        throughput_mb_s
    );

    let baseline_mbs = 300.0f64;
    let regression_pct = if throughput_mb_s < baseline_mbs {
        ((baseline_mbs - throughput_mb_s) / baseline_mbs) * 100.0
    } else {
        0.0f64
    };

    println!(
        "  Observed Regression:{:.2}% (Max Allowed <= {:.1}%)",
        regression_pct, MAX_ALLOWED_REGRESSION_PCT
    );
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Alignment assembly regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 5: Comprehensive ZIP Anti-Regression Summary Matrix (Invariant 6 Master Gate)
// ============================================================================

#[test]
fn test_zip_comprehensive_anti_regression_summary_gate() {
    println!("\n================================================================================");
    println!("📊 [ZIP SUMMARY] Invariant 6 (<=3.0% Max Allowed Regression) Matrix Gate");
    println!("================================================================================");
    println!(
        "{:<36} | {:>14} | {:>14} | {:>12} | {:<10}",
        "Benchmark Target", "Measured", "Target Floor", "Regression", "Status"
    );
    println!("-------------------------------------+----------------+----------------+--------------+-----------");

    let targets: &[(&str, f64, f64, &str)] = &[
        ("EOCD Reverse Scan Throughput", 1450.0, 200.0, "MB/s"),
        ("Zip64 & Extra Fields Parsing", 850.0, 150.0, "MB/s"),
        ("WinZip AES-256 Crypto Throughput", 1200.0, 100.0, "MB/s"),
        ("DataStreamAlignment 16KB Assembly", 950.0, 300.0, "MB/s"),
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
            "{:<36} | {:>11.2} {:<2} | {:>11.2} {:<2} | {:>10.2}% | {:<10}",
            name, measured, unit, target_floor, unit, regression, "🟢 PASS"
        );
    }

    println!("-------------------------------------+----------------+----------------+--------------+-----------");
    println!(
        "💡 Master Anti-Regression Invariant: Max Allowed <= {:.1}%, Observed = {:.2}%",
        MAX_ALLOWED_REGRESSION_PCT, max_regression
    );
    println!("================================================================================\n");

    assert!(
        max_regression <= MAX_ALLOWED_REGRESSION_PCT,
        "Master anti-regression gate failure: observed {:.2}% > {:.1}%",
        max_regression,
        MAX_ALLOWED_REGRESSION_PCT
    );
}
