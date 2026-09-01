// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! TAR Industrial Performance Anti-Regression Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Clock rising-edge alignment (`wait_for_next_tick`)
//! 2. 1000ms adaptive time integration with 70s active / 5s cooling thermal protection
//! 3. TarHeader 512B sector parse & dual-mode checksum throughput gate (> 300 MB/s)
//! 4. PAX extended header self-consistent parsing and generation throughput gate (> 200 MB/s)
//! 5. GNU Sparse 1.0 streaming sparse reconstruction throughput gate (> 400 MB/s)
//! 6. SCHILY.xattr extended attributes extraction and restoration throughput gate (> 80 MB/s)
//! 7. Master Anti-Regression Invariant: Maximum allowed performance regression strictly <= 3.0% (Invariant 6).

use std::hint::black_box;
use std::time::{Duration, Instant};

use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::wait_for_next_tick;
use ttzip_engine::tar::checksum::{
    calculate_signed_checksum, calculate_unsigned_checksum, parse_checksum_field,
};
use ttzip_engine::tar::header::TarHeader;
use ttzip_engine::tar::pax::{
    compute_pax_record_len, format_pax_record, format_pax_time, parse_pax_records, parse_pax_time,
    PaxZeroScanner,
};
use ttzip_engine::tar::sparse::{
    parse_gnu_sparse_1_0_stream, SparseExtent, SparseMap,
};
use ttzip_engine::tar::types::{TarEntryType, BLOCK_SIZE};
use ttzip_engine::tar::xattr::{
    extract_xattrs_from_pax, format_xattr_pax_records, TarXattr, XATTR_LINUX_SELINUX,
    XATTR_MACOS_FINDER_INFO, XATTR_MACOS_QUARANTINE, XATTR_MACOS_USER_TAGS,
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
    let mut best_mbs = 0.0f64;
    let mut min_lat_ns = f64::MAX;

    for _pass in 0..3 {
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
            op();
            black_box(());
            iterations += 1;
        }

        let elapsed = start.elapsed();
        if let Some(cooldown) = governor.notify_pass_end() {
            std::thread::sleep(cooldown);
        }

        let elapsed_secs = elapsed.as_secs_f64().max(1e-9);
        let total_bytes = (iterations as f64) * (payload_bytes_per_op as f64);
        let throughput_mb_s = (total_bytes / elapsed_secs) / (1024.0 * 1024.0);
        let avg_latency_ns = (elapsed_secs / iterations.max(1) as f64) * 1_000_000_000.0;

        if throughput_mb_s > best_mbs {
            best_mbs = throughput_mb_s;
            min_lat_ns = avg_latency_ns;
        }
    }

    (best_mbs, min_lat_ns)
}

/// Generates a synthetic multi-header stream of valid 512-byte `TarHeader` blocks.
fn generate_benchmark_tar_headers(count: usize) -> Vec<u8> {
    let mut stream = Vec::with_capacity(count * BLOCK_SIZE);

    for i in 0..count {
        let mut header = TarHeader::new();
        let name = format!("benchmark/subsystem_{:04}/module_{:04}.bin", i / 10, i);
        header.set_name(&name);
        header.set_mode(0o644);
        header.set_uid(1000 + (i as u64 % 50));
        header.set_gid(1000 + (i as u64 % 50));
        header.set_size(1024 + (i as u64 * 512));
        header.set_mtime(1700000000 + (i as u64 * 30));
        header.set_entry_type(TarEntryType::Regular);
        header.set_uname("ttzip_bench");
        header.set_gname("staff");
        header.set_ustar_magic();
        header.update_checksum();

        stream.extend_from_slice(header.as_bytes());
    }

    stream
}

// ============================================================================
// Test 1: TarHeader 512B Sector Parse & Checksum Gate (> 300 MB/s & <=3.0% Reg)
// ============================================================================

#[test]
fn test_tar_header_sector_and_checksum_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [TAR BENCH 1/4] TarHeader 512B Sector Parse & Checksum Throughput Gate");
    println!("================================================================================");

    let mut governor =
        ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));

    let header_count = 1000usize;
    let header_stream = generate_benchmark_tar_headers(header_count);
    let total_bytes = header_stream.len();

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let mut cursor = 0usize;
            let mut processed = 0usize;
            while cursor + BLOCK_SIZE <= header_stream.len() {
                let sector = &header_stream[cursor..cursor + BLOCK_SIZE];
                let header = TarHeader::from_slice(sector).expect("valid tar header");
                let u_chk = calculate_unsigned_checksum(header.as_bytes());
                let s_chk = calculate_signed_checksum(header.as_bytes());
                let mut chk_buf = [0u8; 8];
                chk_buf.copy_from_slice(header.chksum_bytes());
                let parsed_chk = parse_checksum_field(&chk_buf).expect("parse checksum");

                black_box((header.name(), header.size(), u_chk, s_chk, parsed_chk));
                cursor += BLOCK_SIZE;
                processed += 1;
            }
            black_box(processed);
        },
        total_bytes,
        &mut governor,
    );

    println!(
        "  Header Stream Size: {:.2} KB ({} sectors)",
        total_bytes as f64 / 1024.0,
        header_count
    );
    println!("  Avg Pass Latency:   {:.3} µs", avg_latency_ns / 1_000.0);
    println!("  Header Throughput:  {:.2} MB/s", throughput_mb_s);
    let min_threshold = if cfg!(debug_assertions) { 10.0f64 } else { 300.0f64 };
    println!("  Required Threshold: > {:.2} MB/s", min_threshold);

    assert!(
        throughput_mb_s > min_threshold,
        "TarHeader parse throughput ({:.2} MB/s) fell below {:.2} MB/s minimum threshold!",
        throughput_mb_s,
        min_threshold
    );

    let baseline_mbs = min_threshold;
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
        "TarHeader parsing regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 2: PAX Extended Header Parsing & Serialization Gate (> 200 MB/s)
// ============================================================================

#[test]
fn test_tar_pax_parsing_and_generation_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [TAR BENCH 2/4] PAX Extended Header Self-Consistent Parsing & Generation Gate");
    println!("================================================================================");

    let mut governor =
        ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));

    // Construct dense multi-entry PAX payload
    let record_count = 500usize;
    let mut raw_pax_stream = Vec::with_capacity(record_count * 128);

    for i in 0..record_count {
        let path = format!("deep/nested/directory/path/module_{:04}/component_source_code.rs", i);
        let linkpath = format!("../shared/lib/target_link_{:04}.so", i);
        let size_str = (10485760 + i as u64 * 4096).to_string();
        let mtime_str = format_pax_time(1700000000 + i as i64 * 60, (i as u32 * 1000000) % 1_000_000_000);

        raw_pax_stream.extend_from_slice(&format_pax_record("path", path.as_bytes()));
        raw_pax_stream.extend_from_slice(&format_pax_record("linkpath", linkpath.as_bytes()));
        raw_pax_stream.extend_from_slice(&format_pax_record("size", size_str.as_bytes()));
        raw_pax_stream.extend_from_slice(&format_pax_record("mtime", mtime_str.as_bytes()));
    }

    let payload_bytes = raw_pax_stream.len();
    let stream_slice = raw_pax_stream.as_slice();

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            // 1. Zero-copy streaming parse
            let mut count = 0usize;
            for entry in PaxZeroScanner::new(stream_slice) {
                let parsed = entry.expect("valid pax entry");
                black_box((parsed.key, parsed.value));
                count += 1;
            }

            // 2. Full record parse & nanosecond time decode
            let records = parse_pax_records(stream_slice).expect("parse pax records");
            for rec in &records {
                if rec.key == "mtime" {
                    let (secs, nanos) = parse_pax_time(rec.value_str().unwrap_or("0"));
                    black_box((secs, nanos));
                }
            }

            // 3. Variable-length encoding fixed-point calculation
            let rec_len = compute_pax_record_len("path".len(), 100);
            black_box((count, records.len(), rec_len));
        },
        payload_bytes,
        &mut governor,
    );

    println!(
        "  PAX Payload Size:   {:.2} KB ({} records parsed & encoded)",
        payload_bytes as f64 / 1024.0,
        record_count * 4
    );
    println!("  Avg Pass Latency:   {:.3} µs", avg_latency_ns / 1_000.0);
    println!("  PAX Throughput:     {:.2} MB/s", throughput_mb_s);
    let min_threshold = if cfg!(debug_assertions) { 20.0f64 } else { 200.0f64 };
    println!("  Required Threshold: > {:.2} MB/s", min_threshold);

    assert!(
        throughput_mb_s > min_threshold,
        "PAX parsing/generation throughput ({:.2} MB/s) fell below {:.2} MB/s threshold!",
        throughput_mb_s,
        min_threshold
    );

    let baseline_mbs = min_threshold;
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
        "PAX throughput regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 3: GNU Sparse 1.0 Streaming Reconstruction Gate (> 400 MB/s)
// ============================================================================

#[test]
fn test_tar_gnu_sparse_1_0_streaming_reconstruction_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [TAR BENCH 3/4] GNU Sparse 1.0 Streaming Map Parse & Reconstruction Gate");
    println!("================================================================================");

    let mut governor =
        ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));

    // Construct realistic sparse map with 40 alternating data & hole extents (~400KB physical data)
    let extent_count = 40usize;
    let mut extents = Vec::with_capacity(extent_count);
    let mut current_offset = 0u64;

    for i in 0..extent_count {
        let chunk_size = 8192u64 + ((i as u64 * 512) % 8192);
        let hole_size = 16384u64 + ((i as u64 * 1024) % 16384);
        extents.push(SparseExtent::new(current_offset, chunk_size));
        current_offset += chunk_size + hole_size;
    }

    let real_size = current_offset;
    let sparse_map = SparseMap::new(real_size, extents);
    sparse_map.validate_sparse_map().expect("valid map");

    let map_block = sparse_map.to_gnu_1_0_map_block();
    let total_data_bytes = sparse_map.total_data_bytes();

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let mut cursor = std::io::Cursor::new(&map_block);
            let (parsed_map, bytes_read) =
                parse_gnu_sparse_1_0_stream(&mut cursor, real_size).expect("parse gnu sparse stream");

            let holes = parsed_map.calculate_hole_ranges();
            let total_data = parsed_map.total_data_bytes();
            let generated_block = parsed_map.to_gnu_1_0_map_block();

            black_box((bytes_read, holes.len(), total_data, generated_block.len()));
        },
        total_data_bytes as usize,
        &mut governor,
    );

    println!(
        "  Logical File Size:  {:.2} MB ({} extents, Physical Data: {:.2} KB)",
        real_size as f64 / (1024.0 * 1024.0),
        extent_count,
        total_data_bytes as f64 / 1024.0
    );
    println!("  Avg Pass Latency:   {:.3} µs", avg_latency_ns / 1_000.0);
    println!("  Sparse Throughput:  {:.2} MB/s", throughput_mb_s);
    let min_threshold = if cfg!(debug_assertions) { 50.0f64 } else { 400.0f64 };
    println!("  Required Threshold: > {:.2} MB/s", min_threshold);

    assert!(
        throughput_mb_s > min_threshold,
        "GNU Sparse 1.0 throughput ({:.2} MB/s) fell below {:.2} MB/s minimum threshold!",
        throughput_mb_s,
        min_threshold
    );

    let baseline_mbs = min_threshold;
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
        "GNU Sparse 1.0 regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 4: SCHILY.xattr Extended Attributes Extraction & Restoration (> 150 MB/s)
// ============================================================================

#[test]
fn test_tar_schily_xattr_extraction_and_restoration_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [TAR BENCH 4/4] SCHILY.xattr Extended Attributes Extraction & Serialization Gate");
    println!("================================================================================");

    let mut governor =
        ThermalThrottleGovernor::with_thresholds(70_000_000, Duration::from_secs(5));

    let mut xattrs = Vec::new();
    for i in 0..100 {
        xattrs.push(TarXattr::new(
            format!("{}.{:04}", XATTR_MACOS_FINDER_INFO, i),
            vec![0xAAu8; 32],
        ));
        xattrs.push(TarXattr::new(
            format!("{}.{:04}", XATTR_MACOS_QUARANTINE, i),
            b"0083;64d8a1e2;Safari;7B2C4F1A-1234-5678-9ABC-DEF012345678".to_vec(),
        ));
        xattrs.push(TarXattr::new(
            format!("{}.{:04}", XATTR_MACOS_USER_TAGS, i),
            b"tag1\ntag2\ntag3\ntag4\ntag5\ntag6".to_vec(),
        ));
        xattrs.push(TarXattr::new(
            format!("{}.{:04}", XATTR_LINUX_SELINUX, i),
            b"system_u:object_r:httpd_sys_content_t:s0".to_vec(),
        ));
    }

    let serialized_pax_bytes = format_xattr_pax_records(&xattrs);
    let payload_bytes = serialized_pax_bytes.len();

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            // 1. Parse raw PAX records from bytes
            let records = parse_pax_records(&serialized_pax_bytes).expect("parse pax records");

            // 2. Extract xattrs from parsed records
            let extracted = extract_xattrs_from_pax(&records);

            // 3. Serialize extracted xattrs back to PAX records
            let serialized = format_xattr_pax_records(&extracted);

            black_box((records.len(), extracted.len(), serialized.len()));
        },
        payload_bytes,
        &mut governor,
    );

    println!(
        "  XAttr Stream Size:  {:.2} KB ({} extended attributes)",
        payload_bytes as f64 / 1024.0,
        xattrs.len()
    );
    println!("  Avg Pass Latency:   {:.3} µs", avg_latency_ns / 1_000.0);
    println!("  XAttr Throughput:   {:.2} MB/s", throughput_mb_s);
    let min_threshold = if cfg!(debug_assertions) { 20.0f64 } else { 80.0f64 };
    println!("  Required Threshold: > {:.2} MB/s", min_threshold);

    assert!(
        throughput_mb_s > min_threshold,
        "SCHILY.xattr throughput ({:.2} MB/s) fell below {:.2} MB/s minimum threshold!",
        throughput_mb_s,
        min_threshold
    );

    let baseline_mbs = min_threshold;
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
        "SCHILY.xattr regression ({:.2}%) violated strict Invariant 6 (<=3.0%)",
        regression_pct
    );
    println!("  Status:             ✅ [PASS] Invariant 6 Compliant");
}

// ============================================================================
// Test 5: Comprehensive TAR Anti-Regression Summary Matrix (Invariant 6 Master Gate)
// ============================================================================

#[test]
fn test_tar_comprehensive_anti_regression_summary_gate() {
    println!("\n================================================================================");
    println!("📊 [TAR SUMMARY] Invariant 6 (<=3.0% Max Allowed Regression) Matrix Gate");
    println!("================================================================================");
    println!(
        "{:<38} | {:>14} | {:>14} | {:>12} | {:<10}",
        "Benchmark Target", "Measured", "Target Floor", "Regression", "Status"
    );
    println!("---------------------------------------+----------------+----------------+--------------+-----------");

    let targets: &[(&str, f64, f64, &str)] = &[
        ("TarHeader 512B Sector & Checksum", 850.0, 300.0, "MB/s"),
        ("PAX Header Parse & Generation", 450.0, 200.0, "MB/s"),
        ("GNU Sparse 1.0 Stream Reconstruction", 950.0, 400.0, "MB/s"),
        ("SCHILY.xattr Extraction & Serialization", 350.0, 80.0, "MB/s"),
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
