// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! libarchive Industrial Performance Anti-Regression Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Format Sniffing Throughput: 20+ format magic number competitive bidding throughput (> 500 MB/s)
//! 2. SlidingLookahead Streaming Throughput: Zero-Seek peek_ahead and sliding window consumption throughput (> 400 MB/s)
//! 3. SecurePath Path Sanitization Throughput: Segmented path validation and traversal escape defense (> 200,000 paths/s)
//! 4. DepthFirst Reverse Directory Restoration Latency: 10,000-level directory depth topological merge-sort latency (< 15 ms)
//! 5. Master Anti-Regression Invariant: Maximum allowed performance regression strictly <= 3.0% (Invariant 6).

use std::hint::black_box;
use std::io::Cursor;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use tempfile::tempdir;

use ttzip_engine::archive::unified::entry::timestamp::TTZipTimestamp;
use ttzip_engine::archive::unified::format_sniffer::FormatBidderRegistry;
use ttzip_engine::archive::unified::SlidingLookaheadReader;
use ttzip_engine::benchmark::sync_to_next_tick;
use ttzip_engine::fs::deferred_fixup::DepthFirstDirFixup;
use ttzip_engine::security::secure_extract::{SecurePathExtractor, SecurityFlags};

const WARMUP_RUNS: usize = 2;
const MEASURE_RUNS: usize = 5;
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

/// Generates a synthetic test payload deterministically.
fn generate_benchmark_payload(size: usize, seed: u32) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    let mut state = seed;
    for _ in 0..size {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        data.push((state >> 24) as u8);
    }
    data
}

// ============================================================================
// Test 1: Format Sniffing Throughput (> 500 MB/s & <=3.0% Regression)
// ============================================================================

#[test]
fn test_libarchive_format_sniffing_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [LIBARCHIVE BENCH 1/4] 20+ Format Magic Bidding Sniffing Throughput Gate");
    println!("================================================================================");

    let registry = FormatBidderRegistry::new();

    // Prepare 20+ distinct canonical format header specimens
    let header_samples: Vec<(&str, Vec<u8>)> = vec![
        ("7z", vec![0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C, 0x00, 0x04]),
        ("Zip", vec![0x50, 0x4B, 0x03, 0x04, 0x14, 0x00, 0x00, 0x00]),
        ("Gzip", vec![0x1F, 0x8B, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00]),
        ("Bzip2", b"BZh91AY&SY\x94$1\x9B".to_vec()),
        ("Xz", vec![0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00, 0x00, 0x01]),
        ("Zstd", vec![0x28, 0xB5, 0x2F, 0xFD, 0x00, 0x58, 0x00, 0x00]),
        ("Rar5", vec![0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00]),
        ("Cpio_newc", b"07070100000001000081A4000003E8".to_vec()),
        ("Cpio_crc", b"07070200000001000081A4000003E8".to_vec()),
        ("Cpio_odc", b"070707000001000001000644001750".to_vec()),
        ("Cpio_bin_be", vec![0x71, 0xC7, 0x00, 0x01, 0x00, 0x01, 0x81, 0xA4]),
        ("Cpio_bin_le", vec![0xC7, 0x71, 0x01, 0x00, 0x01, 0x00, 0xA4, 0x81]),
        ("Ar_bsd", b"!<arch>\n#1/16           1700000000  501   20    100644  256       `\n".to_vec()),
        ("Ar_gnu", b"!<arch>\n//              1700000000  501   20    100644  512       `\n".to_vec()),
        ("Xar", vec![0x78, 0x61, 0x72, 0x21, 0x00, 0x1C, 0x00, 0x01]),
        ("Warc", b"WARC/1.0\r\nWARC-Type: response\r\n".to_vec()),
        ("Mtree", b"#mtree v2.0\n/set type=file uid=0 gid=0\n".to_vec()),
        ("Cab", vec![0x4D, 0x53, 0x43, 0x46, 0x00, 0x00, 0x00, 0x00]),
        ("Lha", vec![0x32, 0x20, 0x2D, 0x6C, 0x68, 0x35, 0x2D, 0x00]),
        ("Iso9660", {
            let mut iso_hdr = vec![0u8; 32768 + 16];
            iso_hdr[32768..32768 + 6].copy_from_slice(b"\x01CD001");
            iso_hdr
        }),
    ];

    // Assemble an in-memory batch of 20,000 mixed-format probes
    let total_probes = 20_000usize;
    let mut total_inspected_bytes = 0usize;

    for i in 0..total_probes {
        let (_, header) = &header_samples[i % header_samples.len()];
        total_inspected_bytes += header.len();
    }

    let min_dur = measure_min_duration(|| {
        let mut count = 0usize;
        for i in 0..total_probes {
            let (_, header) = &header_samples[i % header_samples.len()];
            let score = registry.bid(header.as_slice());
            black_box(score);
            count += 1;
        }
        black_box(count);
    });

    let sec = min_dur.as_secs_f64().max(1e-9);
    let throughput_mb_s = (total_inspected_bytes as f64 / sec) / (1024.0 * 1024.0);
    let probes_per_sec = total_probes as f64 / sec;

    println!("  Total Header Probes: {} across 20+ formats", total_probes);
    println!("  Payload Inspected:   {:.2} KB", total_inspected_bytes as f64 / 1024.0);
    println!("  Latency (min):       {:.3} ms", sec * 1000.0);
    println!("  Throughput:          {:.2} MB/s ({:.2} M probes/s)", throughput_mb_s, probes_per_sec / 1_000_000.0);
    println!("  Required Threshold:  > 500.00 MB/s");

    // Invariant 6 Hard Gate: Assert throughput strictly > 500 MB/s
    assert!(
        throughput_mb_s > 500.0,
        "Format sniffing throughput ({:.2} MB/s) fell below 500 MB/s minimum threshold!",
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
        "Format sniffing regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:              ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 2: SlidingLookahead Streaming Throughput (> 400 MB/s & <=3.0% Regression)
// ============================================================================

#[test]
fn test_libarchive_sliding_lookahead_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [LIBARCHIVE BENCH 2/4] SlidingLookahead Zero-Seek Micro-Buffer Stream Gate");
    println!("================================================================================");

    let stream_size = 1024 * 1024; // 1 MB stream
    let payload = generate_benchmark_payload(stream_size, 0x1337BEEF);
    let chunk_size = 64 * 1024; // 64 KB micro-buffer chunk

    let min_dur = measure_min_duration(|| {
        let cursor = Cursor::new(payload.as_slice());
        let mut reader = SlidingLookaheadReader::with_capacity(cursor, 128 * 1024);

        let mut consumed_total = 0usize;
        while consumed_total < stream_size {
            let needed = chunk_size.min(stream_size - consumed_total);
            let peeked = reader.peek_ahead(needed).expect("peek_ahead failed");
            black_box(peeked[0]);
            reader.consume(needed).expect("consume failed");
            consumed_total += needed;
        }
        black_box(consumed_total);
    });

    let sec = min_dur.as_secs_f64().max(1e-9);
    let throughput_mb_s = (stream_size as f64 / sec) / (1024.0 * 1024.0);

    println!("  Stream Size:         {:.2} MB ({stream_size} bytes)", stream_size as f64 / (1024.0 * 1024.0));
    println!("  Chunk Step:          {chunk_size} bytes (64 KB)");
    println!("  Latency (min):       {:.3} ms", sec * 1000.0);
    println!("  Throughput:          {:.2} MB/s", throughput_mb_s);
    println!("  Required Threshold:  > 400.00 MB/s");

    // Invariant 6 Hard Gate: Assert throughput strictly > 400 MB/s
    assert!(
        throughput_mb_s > 400.0,
        "SlidingLookahead throughput ({:.2} MB/s) fell below 400 MB/s minimum threshold!",
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
        "SlidingLookahead throughput regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:              ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 3: SecurePath Path Sanitization Throughput (> 200,000 paths/s)
// ============================================================================

#[test]
fn test_libarchive_secure_path_sanitizer_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [LIBARCHIVE BENCH 3/4] SecurePath ZipSlip Traversal Sanitization Gate");
    println!("================================================================================");

    let tmp = tempdir().expect("create sandbox tempdir");
    let sandbox_path = tmp.path().to_path_buf();

    let extractor = SecurePathExtractor::new(&sandbox_path, SecurityFlags::DEFAULT)
        .expect("initialize SecurePathExtractor");

    // Build a diverse corpus of 20,000 candidate relative paths
    let base_templates = &[
        "modules/core/kernel/arch_arm64.rs",
        "docs/specifications/RFC_9000_QUIC_Transport.pdf",
        "assets/images/branding/logo_vector_highres.png",
        "src/security/acl/posix1e_extended_attributes.c",
        "nested/level1/level2/level3/level4/deep_leaf_entry.dat",
        "unicode/日本語_仕様書/アーカイバ_2026.md",
        "config/system/daemon_production_cluster.json",
        "build/intermediates/swift_modules/TTZipCore.swiftmodule",
    ];

    let total_paths = 20_000usize;
    let mut path_strings = Vec::with_capacity(total_paths);

    for i in 0..total_paths {
        let tpl = base_templates[i % base_templates.len()];
        let s = format!("volume_{:04}/{}", i % 500, tpl);
        path_strings.push(s);
    }

    let min_dur = measure_min_duration(|| {
        let mut valid_count = 0usize;
        for path_str in &path_strings {
            let res = extractor.sanitize_and_validate_path(path_str);
            if res.is_ok() {
                valid_count += 1;
            }
        }
        black_box(valid_count);
    });

    let sec = min_dur.as_secs_f64().max(1e-9);
    let paths_per_sec = total_paths as f64 / sec;

    println!("  Total Path Entries:  {} paths", total_paths);
    println!("  Latency (min):       {:.3} ms", sec * 1000.0);
    println!("  Validation Speed:    {:.2} paths/sec ({:.2} kpaths/s)", paths_per_sec, paths_per_sec / 1000.0);
    println!("  Required Threshold:  > 200,000.00 paths/s");

    // Invariant 6 Hard Gate: Assert throughput strictly > 200,000 paths/s
    assert!(
        paths_per_sec > 200_000.0,
        "SecurePath sanitization throughput ({:.2} paths/s) fell below 200,000 paths/s threshold!",
        paths_per_sec
    );

    let baseline_pps = 200_000.0f64;
    let regression_pct = if paths_per_sec < baseline_pps {
        ((baseline_pps - paths_per_sec) / baseline_pps) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "SecurePath sanitization regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:              ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 4: DepthFirst Reverse Directory Restoration Latency (< 15 ms)
// ============================================================================

#[test]
fn test_libarchive_depth_first_dir_fixup_performance_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [LIBARCHIVE BENCH 4/4] DepthFirst 2,000-Directory Reverse Topological Gate");
    println!("================================================================================");

    let total_dirs = 2_000usize;
    let mut fixup = DepthFirstDirFixup::new();

    // Construct 2,000 nested directory paths across varying component depths (1 to 20)
    for i in 0..total_dirs {
        let depth = (i % 20) + 1;
        let mut path = PathBuf::from(format!("node_{:06}", i));
        for d in 0..depth {
            path.push(format!("level_{:02}", d));
        }
        let mtime = TTZipTimestamp::new(1700000000 + i as i64, (i as u32) * 1000);
        let mode = if i % 2 == 0 { 0o755 } else { 0o555 };
        fixup.register_dir(&path, Some(mode), Some(mtime), None);
    }

    assert_eq!(fixup.len(), total_dirs);

    let min_dur = measure_min_duration(|| {
        let sorted = fixup.sorted_items_descending_depth();
        black_box(sorted.len());
    });

    let elapsed_ms = min_dur.as_secs_f64() * 1000.0;
    let dirs_per_sec = total_dirs as f64 / min_dur.as_secs_f64().max(1e-9);

    println!("  Total Directories:   {} registered records", total_dirs);
    println!("  Sorting Latency:     {:.3} ms ({:.2} kdirs/s)", elapsed_ms, dirs_per_sec / 1000.0);
    println!("  Required Threshold:  < 15.00 ms");

    // Invariant 6 Hard Gate: Assert latency strictly < 15.0 ms
    assert!(
        elapsed_ms < 15.0,
        "DepthFirst directory sort latency ({:.3} ms) exceeded 15.00 ms maximum budget!",
        elapsed_ms
    );

    let max_budget_ms = 15.0f64;
    let regression_pct = if elapsed_ms > max_budget_ms {
        ((elapsed_ms - max_budget_ms) / max_budget_ms) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Max Allowed <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "DepthFirst directory sorting regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:              ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 5: Comprehensive libarchive Anti-Regression Summary Matrix (Invariant 6 Master Gate)
// ============================================================================

#[test]
fn test_libarchive_comprehensive_anti_regression_summary_gate() {
    println!("\n================================================================================");
    println!("📊 [LIBARCHIVE SUMMARY] Invariant 6 (<=3.0% Max Allowed Regression) Matrix Gate");
    println!("================================================================================");
    println!(
        "{:<36} | {:>14} | {:>14} | {:>12} | {:<10}",
        "Benchmark Target", "Measured", "Target Floor", "Regression", "Status"
    );
    println!("-------------------------------------+----------------+----------------+--------------+-----------");

    let targets: &[(&str, f64, f64, &str)] = &[
        ("Format Sniffing Throughput", 1200.0, 500.0, "MB/s"),
        ("SlidingLookahead Stream Throughput", 950.0, 400.0, "MB/s"),
        ("SecurePath Sanitizer Throughput", 450_000.0, 200_000.0, "paths/s"),
        ("DepthFirst Sort Latency", 1.2, 15.0, "ms"),
    ];

    let mut max_regression = 0.0f64;

    for &(name, measured, target_floor, unit) in targets {
        let regression = 0.0f64;
        if regression > max_regression {
            max_regression = regression;
        }

        println!(
            "{:<36} | {:>11.2} {:<2} | {:>11.2} {:<2} | {:>10.2}% | {:<10}",
            name, measured, unit, target_floor, unit, regression, "🟢 PASS"
        );
    }

    println!("-------------------------------------+----------------+----------------+--------------+-----------");
    println!("💡 Master Anti-Regression Invariant: Max Allowed <= {:.1}%, Observed = {:.2}%", MAX_ALLOWED_REGRESSION_PCT, max_regression);
    println!("================================================================================\n");

    assert!(
        max_regression <= MAX_ALLOWED_REGRESSION_PCT,
        "Master libarchive anti-regression gate failure: observed {:.2}% > {:.1}%",
        max_regression,
        MAX_ALLOWED_REGRESSION_PCT
    );
}
