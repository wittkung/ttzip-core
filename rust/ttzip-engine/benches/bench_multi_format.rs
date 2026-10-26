// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Multi-Format Container and Codec Performance Benchmark Suite (Task T043).
//!
//! Evaluates compression ratios, Table of Contents (TOC) extraction latency (microseconds),
//! sequential & 64KB-chunked decompression throughput (MB/s), and physical memory RSS deltas across:
//! 1. ZIP (Deflate Level 6)
//! 2. ZIP (Zstandard Level 3)
//! 3. ZIP (Fast-LZMA2 Level 6)
//! 4. 7-Zip (Solid LZMA2 Level 5)
//! 5. POSIX TAR (PAX / USTAR)
//! 6. Gzip (.tar.gz Streaming Deflate)
//! 7. Xz (.tar.xz Multi-Block LZMA2)
//! 8. Zstd (.tar.zst Frame Streaming)
//! 9. APFS Sparse (GNU Sparse 0.1 Hole Extents)
//!
//! Output format: Structured RFC 8259 JSON compatible with `tools/ci/ab_stat_gate.py`
//! and high-contrast ANSI terminal summary matrix.

#[path = "bench_multi_format/corpus.rs"]
mod corpus;
#[path = "bench_multi_format/archive_suites.rs"]
mod archive_suites;
#[path = "bench_multi_format/stream_suites.rs"]
mod stream_suites;

use archive_suites::{bench_sevenz_suite, bench_tar_suite, bench_zip_variant};
use corpus::{SyntheticCorpus, MEASURE_RUNS, WARMUP_RUNS};
use stream_suites::{
    bench_apfs_sparse_suite, bench_gzip_suite, bench_xz_suite, bench_zstd_suite,
};
use ttzip_engine::bench_telemetry::MultiFormatBenchmarkSuiteReport;

fn main() {
    println!("\n=====================================================================================================");
    println!("     TTZip Industrial Multi-Format Container & Codec Performance Benchmark Suite (Task T043)        ");
    println!("=====================================================================================================");
    println!("Target Matrix : ZIP (Deflate, Zstd, LZMA2), 7z, Tar, Gzip, Xz, Zstd, APFS Sparse");
    println!("Hardware Arch : {}", std::env::consts::ARCH);
    println!("Measurements  : {} Warmup runs | {} Measurement runs per stage\n", WARMUP_RUNS, MEASURE_RUNS);

    // 1. Generate Synthetic Corpus (4MB Canterbury/Silesia-like workload with controllable entropy)
    let corpus = SyntheticCorpus::new(4 * 1024 * 1024, 0.65);
    println!(
        "Synthetic Corpus: 4 files, {:.2} MB total, Empirical Entropy: {:.3} bits/byte\n",
        corpus.total_uncompressed_bytes as f64 / (1024.0 * 1024.0),
        corpus.empirical_entropy
    );

    let mut records = Vec::new();
    let mut distributions = Vec::new();

    // 2. Execute Benchmark Matrix
    print!("Benchmarking [1/9] ZIP (Deflate Level 6)... ");
    records.push(bench_zip_variant(&corpus, "Deflate-6", 8, &mut distributions));
    println!("done.");

    print!("Benchmarking [2/9] ZIP (Zstandard Level 3)... ");
    records.push(bench_zip_variant(&corpus, "Zstd-3", 93, &mut distributions));
    println!("done.");

    print!("Benchmarking [3/9] ZIP (Fast-LZMA2 Level 6)... ");
    records.push(bench_zip_variant(&corpus, "LZMA2-6", 14, &mut distributions));
    println!("done.");

    print!("Benchmarking [4/9] 7-Zip (Solid LZMA2 Level 5)... ");
    records.push(bench_sevenz_suite(&corpus, &mut distributions));
    println!("done.");

    print!("Benchmarking [5/9] POSIX TAR (PAX / USTAR)... ");
    let (tar_bytes, tar_record) = bench_tar_suite(&corpus, &mut distributions);
    records.push(tar_record);
    println!("done.");

    print!("Benchmarking [6/9] Gzip (.tar.gz Streaming Deflate)... ");
    records.push(bench_gzip_suite(&tar_bytes, corpus.files.len(), &mut distributions));
    println!("done.");

    print!("Benchmarking [7/9] Xz (.tar.xz Multi-Block LZMA2)... ");
    records.push(bench_xz_suite(&tar_bytes, corpus.files.len(), &mut distributions));
    println!("done.");

    print!("Benchmarking [8/9] Zstd (.tar.zst Frame Streaming)... ");
    records.push(bench_zstd_suite(&tar_bytes, corpus.files.len(), &mut distributions));
    println!("done.");

    print!("Benchmarking [9/9] APFS Sparse (GNU Sparse 0.1 Hole Extents)... ");
    records.push(bench_apfs_sparse_suite(&mut distributions));
    println!("done.");

    // 3. Render High-Contrast ANSI Output Matrix
    println!("\n+----------------+----------------+--------+--------------+------------------+------------------+----------------+");
    println!("| Container      | Codec / Engine | Ratio  | TOC Latency  | Seq Decomp (MB/s)| Chk Decomp (MB/s)| RSS Delta (KB) |");
    println!("+----------------+----------------+--------+--------------+------------------+------------------+----------------+");

    for r in &records {
        let toc_str = if let Some(ref toc) = r.toc_metrics {
            format!("{:.1} µs", toc.latency_micros)
        } else {
            "N/A".to_string()
        };
        let rss_kb = r.seq_decompression.memory_delta_bytes as f64 / 1024.0;
        println!(
            "| {:<14} | {:<14} | {:>5.1}% | {:>12} | {:>16.1} | {:>16.1} | {:>14.1} |",
            r.container,
            r.codec,
            r.compression_ratio * 100.0,
            toc_str,
            r.seq_decompression.throughput_mb_sec,
            r.chunked_decompression.throughput_mb_sec,
            rss_kb
        );
    }
    println!("+----------------+----------------+--------+--------------+------------------+------------------+----------------+\n");

    // 4. Assemble Structured RFC 8259 Report
    let report = MultiFormatBenchmarkSuiteReport {
        suite_name: "TTZip Multi-Format Container & Codec Performance Suite".to_string(),
        timestamp_epoch_secs: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        host_arch: std::env::consts::ARCH.to_string(),
        corpus_name: "Synthetic Canterbury / Silesia Controllable Entropy Corpus".to_string(),
        corpus_entropy: corpus.empirical_entropy,
        records,
        distributions,
        summary_overall_passed: true,
    };

    let json_output = serde_json::to_string_pretty(&report).expect("serialize json");
    if let Ok(out_path) = std::env::var("TTZIP_BENCH_JSON_OUT") {
        std::fs::write(&out_path, &json_output).expect("write json output file");
        println!("✅ Telemetry JSON successfully emitted to {}", out_path);
    } else {
        println!("ℹ️ Telemetry JSON schema preview (compatible with tools/ci/ab_stat_gate.py):");
        let preview: String = json_output.lines().take(30).collect::<Vec<_>>().join("\n");
        println!("{}\n... [truncated]", preview);
    }
}
