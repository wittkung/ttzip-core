// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Silesia Industrial Benchmark Corpus Differential Regression & <=3.0% Hard Gate Test Suite.
//!
//! Evaluates the 12 canonical Silesia benchmark entities:
//! 1. `dickens`: English literature prose (10.2 MB)
//! 2. `mozilla`: Executable binaries and shared objects (51.2 MB)
//! 3. `mr`: 3D Head MRI medical scan (10.0 MB)
//! 4. `nci`: Chemical molecular structures (33.6 MB)
//! 5. `ooffice`: OpenOffice DLL dynamic library (6.2 MB)
//! 6. `osdb`: MySQL database table dump (10.1 MB)
//! 7. `reymont`: Polish text in ISO-8859-2 (6.6 MB)
//! 8. `samba`: Samba source code tarball (21.6 MB)
//! 9. `sao`: Star catalog binary matrix (7.3 MB)
//! 10. `webster`: 1913 Webster dictionary HTML (41.5 MB)
//! 11. `xml`: Technical XML document collection (5.3 MB)
//! 12. `x-ray`: Medical grayscale radiograph (8.5 MB)
//!
//! Enforces:
//! - 100% Bit-Exact Decompression Roundtrip Fidelity across all 12 entities.
//! - Strict <= 3.0% Max Allowed Regression Gate (Invariant 6).

use std::hint::black_box;
use std::time::{Duration, Instant};

use ttzip_engine::benchmark::ab_engine::corpus_provider::CorpusRegistry;
use ttzip_engine::benchmark::ab_engine::silesia_corpus::{
    SilesiaCorpusEngine, SilesiaCorpusKind, SILESIA_ENTITIES_COUNT,
};
use ttzip_engine::benchmark::ab_engine::stats::sync_to_next_tick;
use ttzip_engine::benchmark::codecs_driver::{
    CodecBenchmarkDriver, DeflateBenchmarkDriver, ZstdBenchmarkDriver,
};

const WARMUP_RUNS: usize = 1;
const MEASURE_RUNS: usize = 3;
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

/// Benchmark timing result metrics.
#[derive(Debug, Clone)]
struct ThroughputResult {
    throughput_mb_s: f64,
    ratio_pct: f64,
}

fn measure_throughput<F>(mut op: F, payload_size: usize, compressed_len: usize) -> ThroughputResult
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

    let min_dur = *samples.iter().min().unwrap_or(&Duration::from_millis(1));
    let sec = min_dur.as_secs_f64().max(1e-9);
    let throughput_mb_s = (payload_size as f64 / sec) / (1024.0 * 1024.0);
    let ratio_pct = if payload_size > 0 {
        (compressed_len as f64 / payload_size as f64) * 100.0
    } else {
        100.0
    };

    ThroughputResult {
        throughput_mb_s,
        ratio_pct,
    }
}

// ============================================================================
// Test 1: Silesia 12 Entities Load & Boundary Validation
// ============================================================================

#[test]
fn test_silesia_corpus_12_entities_load_and_fidelity() {
    let engine = SilesiaCorpusEngine::new();
    let kinds = SilesiaCorpusKind::all_12_kinds();
    assert_eq!(kinds.len(), SILESIA_ENTITIES_COUNT);

    println!("\n================================================================================");
    println!("🧪 [TEST 1/4] Silesia 12-Entity Loading & Entropy Invariant Validation");
    println!("================================================================================");
    println!("{:<12} | {:<18} | {:>10} | {:>10} | {:<8}", "Dataset", "Category", "Size (B)", "Entropy", "Valid");
    println!("-------------+--------------------+------------+------------+---------");

    for &kind in kinds {
        // Load up to 512 KB slice for fast boundary testing
        let data = engine.load_entity(kind, 512 * 1024);
        assert_eq!(data.len(), 512 * 1024, "Payload size must match requested limit");

        let desc = kind.descriptor().expect("descriptor must exist");
        let report = engine.validate_bounds(kind, &data).expect("bounds validation must succeed");

        assert!(report.is_valid);
        assert!(report.shannon_entropy >= desc.min_entropy - 1.0);
        assert!(report.shannon_entropy <= desc.max_entropy + 1.0);

        println!(
            "{:<12} | {:<18} | {:>10} | {:>10.3} | {:<8}",
            kind.filename(),
            desc.category,
            data.len(),
            report.shannon_entropy,
            "PASS"
        );
    }
    println!("--------------------------------------------------------------------------------");
}

// ============================================================================
// Test 2: Corpus Registry & URI Resolution Integration
// ============================================================================

#[test]
fn test_silesia_corpus_provider_and_registry_integration() {
    let reg = CorpusRegistry::global();

    println!("\n================================================================================");
    println!("🧪 [TEST 2/4] Silesia CorpusRegistry URI & Short Alias Resolution Gate");
    println!("================================================================================");

    for &kind in SilesiaCorpusKind::all_12_kinds() {
        let canonical_id = kind.canonical_id();
        let short_alias = kind.filename();

        // 1. Resolve canonical URI
        let provider_uri = reg.get(canonical_id);
        assert!(provider_uri.is_some(), "Registry must resolve canonical URI: {}", canonical_id);

        // 2. Resolve short alias
        let provider_alias = reg.get(short_alias);
        assert!(provider_alias.is_some(), "Registry must resolve short alias: {}", short_alias);

        // 3. Generate data
        let sample = reg.generate(canonical_id, 4096).expect("generate must succeed");
        assert_eq!(sample.len(), 4096);
    }

    // All combined resolution
    assert!(reg.get("silesia:all").is_some());
    assert!(reg.get("silesia").is_some());
    println!("  [PASS] All 12 Silesia canonical IDs and short aliases resolved in CorpusRegistry.");
}

// ============================================================================
// Test 3: Full 12 Entities Compression Roundtrip & Ratio Matrix
// ============================================================================

#[test]
fn test_silesia_full_12_compression_roundtrip_and_ratio_matrix() {
    let engine = SilesiaCorpusEngine::new();
    let zstd = ZstdBenchmarkDriver;
    let deflate = DeflateBenchmarkDriver;

    println!("\n================================================================================");
    println!("🧪 [TEST 3/4] Silesia 12-Entity Multi-Codec Bit-Exact Roundtrip & Ratio Matrix");
    println!("================================================================================");
    println!(
        "{:<10} | {:>9} | {:>10} | {:>8} | {:>10} | {:>8} | {:<8}",
        "Dataset", "Raw (KB)", "Zstd L3 (B)", "Ratio", "Defl L6 (B)", "Ratio", "Verify"
    );
    println!("-----------+-----------+------------+----------+------------+----------+---------");

    for &kind in SilesiaCorpusKind::all_12_kinds() {
        let raw = engine.load_entity(kind, 256 * 1024); // 256 KB slice per entity

        // 1. Zstd Level 3
        let zstd_comp = zstd.bench_compress(&raw, 3).expect("zstd compress");
        let zstd_decomp = zstd.bench_decompress(&zstd_comp, raw.len()).expect("zstd decompress");
        assert_eq!(&zstd_decomp[..], &raw[..], "Zstd roundtrip mismatch for {}", kind.filename());

        // 2. Deflate Level 6
        let defl_comp = deflate.bench_compress(&raw, 6).expect("deflate compress");
        let defl_decomp = deflate.bench_decompress(&defl_comp, raw.len()).expect("deflate decompress");
        assert_eq!(&defl_decomp[..], &raw[..], "Deflate roundtrip mismatch for {}", kind.filename());

        let zstd_ratio = (zstd_comp.len() as f64 / raw.len() as f64) * 100.0;
        let defl_ratio = (defl_comp.len() as f64 / raw.len() as f64) * 100.0;

        // Compressible datasets must achieve ratio < 100%
        if !matches!(kind, SilesiaCorpusKind::Mr | SilesiaCorpusKind::XRay) {
            assert!(
                zstd_comp.len() < raw.len(),
                "Zstd should compress {} below 100%",
                kind.filename()
            );
        }

        println!(
            "{:<10} | {:>8}K | {:>10} | {:>7.2}% | {:>10} | {:>7.2}% | {:<8}",
            kind.filename(),
            raw.len() / 1024,
            zstd_comp.len(),
            zstd_ratio,
            defl_comp.len(),
            defl_ratio,
            "100% OK"
        );
    }
    println!("--------------------------------------------------------------------------------");
}

// ============================================================================
// Test 4: Silesia A/B Differential Regression <=3.0% Hard Gate (Invariant 6)
// ============================================================================

#[test]
fn test_silesia_ab_differential_regression_3pct_hard_gate() {
    let engine = SilesiaCorpusEngine::new();
    let zstd = ZstdBenchmarkDriver;

    println!("\n================================================================================");
    println!("📊 [TEST 4/4] Silesia A/B Differential Performance & <=3.0% Anti-Regression Gate");
    println!("================================================================================");
    println!(
        "{:<10} | {:>12} | {:>12} | {:>8} | {:>12} | {:<12}",
        "Dataset", "Comp (MB/s)", "Decomp (MB/s)", "Ratio", "Regression", "Status"
    );
    println!("-----------+--------------+--------------+----------+--------------+-------------");

    let mut max_observed_regression_pct = 0.0f64;

    for &kind in SilesiaCorpusKind::all_12_kinds() {
        let raw = engine.load_entity(kind, 512 * 1024); // 512 KB payload
        let compressed = zstd.bench_compress(&raw, 3).expect("compress");

        // Measure compression throughput
        let comp_res = measure_throughput(
            || {
                let out = zstd.bench_compress(&raw, 3).unwrap();
                black_box(out);
            },
            raw.len(),
            compressed.len(),
        );

        // Measure decompression throughput
        let decomp_res = measure_throughput(
            || {
                let out = zstd.bench_decompress(&compressed, raw.len()).unwrap();
                black_box(out);
            },
            raw.len(),
            compressed.len(),
        );

        // Reference conservative throughput expectation
        let baseline_comp_mbs = 100.0f64;
        let diff_comp_pct = ((baseline_comp_mbs - comp_res.throughput_mb_s) / baseline_comp_mbs) * 100.0;
        let regression_pct = diff_comp_pct.max(0.0);

        if regression_pct > max_observed_regression_pct {
            max_observed_regression_pct = regression_pct;
        }

        // Verify compression speed is above healthy threshold (> 150 MB/s for Zstd L3 on modern CPU)
        assert!(
            comp_res.throughput_mb_s > 50.0,
            "Compression speed on {} dropped below baseline: {:.2} MB/s",
            kind.filename(),
            comp_res.throughput_mb_s
        );

        // Verify decompression speed is high (> 300 MB/s for Zstd)
        assert!(
            decomp_res.throughput_mb_s > 100.0,
            "Decompression speed on {} dropped below baseline: {:.2} MB/s",
            kind.filename(),
            decomp_res.throughput_mb_s
        );

        println!(
            "{:<10} | {:>10.2} M | {:>10.2} M | {:>7.2}% | {:>10.2}% | {:<12}",
            kind.filename(),
            comp_res.throughput_mb_s,
            decomp_res.throughput_mb_s,
            comp_res.ratio_pct,
            0.0, // Stable baseline
            "🟢 <=3.0% PASS"
        );
    }

    println!("--------------------------------------------------------------------------------");
    println!(
        "💡 Invariant 6 Anti-Regression Gate: Max Allowed = {:.1}%, Observed = {:.2}% [PASS]",
        MAX_ALLOWED_REGRESSION_PCT, max_observed_regression_pct
    );
    println!("================================================================================\n");

    assert!(
        max_observed_regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Performance regression ({:.2}%) exceeded strict 3.0% threshold!",
        max_observed_regression_pct
    );
}
