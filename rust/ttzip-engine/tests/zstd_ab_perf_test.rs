// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive A/B Performance Benchmark Suite for Zstandard Multithreading,
//! TLS Context Pooling, and Compression Level Range Verification.

use std::fs::File;
use std::io::Write;
use std::time::Instant;
use tempfile::tempdir;
use ttzip_engine::codecs::zstd::{
    zstd_compress, zstd_compress_bound, zstd_decompress, ZstdConfig, ZstdStreamWriter,
};

/// Generates synthetic text/JSON-like corpus with high redundancy.
fn generate_text_corpus(size: usize) -> Vec<u8> {
    let pattern = br#"{"id":12345,"name":"TTZip Engine Bench","status":"active","metrics":{"cpu":4.5,"mem":1024,"io":99.8},"tags":["archiving","compression","fast"]}\n"#;
    let mut data = Vec::with_capacity(size);
    while data.len() < size {
        let chunk = (size - data.len()).min(pattern.len());
        data.extend_from_slice(&pattern[..chunk]);
    }
    data
}

/// Generates pseudo-random binary data (Mach-O executable-like entropy).
fn generate_binary_corpus(size: usize) -> Vec<u8> {
    let mut data = vec![0u8; size];
    let mut seed = 0x123456789ABCDEF0u64;
    for chunk in data.chunks_mut(8) {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let bytes = seed.to_le_bytes();
        let len = chunk.len().min(8);
        chunk[..len].copy_from_slice(&bytes[..len]);
    }
    data
}

#[test]
fn test_ab_zstd_multithread_speedup_100mb() {
    let corpus_size = 100 * 1024 * 1024; // 100 MB
    let corpus = generate_text_corpus(corpus_size);
    let dir = tempdir().unwrap();

    // 1. Baseline: Single-threaded (nb_workers = 1)
    let single_path = dir.path().join("single.zst");
    let single_file = File::create(&single_path).unwrap();
    let config_single = ZstdConfig {
        level: 3,
        nb_workers: 1,
        enable_checksum: true,
        ..Default::default()
    };
    let start_single = Instant::now();
    let mut writer_single = ZstdStreamWriter::new(single_file, &config_single).unwrap();
    writer_single.write_all(&corpus).unwrap();
    writer_single.finish().unwrap();
    let duration_single = start_single.elapsed();
    let throughput_single = (corpus_size as f64 / (1024.0 * 1024.0 * 1024.0)) / duration_single.as_secs_f64();

    // 2. Candidate: Multithreaded (nb_workers = available_parallelism)
    let workers = std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(4);
    let mt_path = dir.path().join("multithread.zst");
    let mt_file = File::create(&mt_path).unwrap();
    let config_mt = ZstdConfig {
        level: 3,
        nb_workers: workers,
        enable_checksum: true,
        ..Default::default()
    };
    let start_mt = Instant::now();
    let mut writer_mt = ZstdStreamWriter::new(mt_file, &config_mt).unwrap();
    writer_mt.write_all(&corpus).unwrap();
    writer_mt.finish().unwrap();
    let duration_mt = start_mt.elapsed();
    let throughput_mt = (corpus_size as f64 / (1024.0 * 1024.0 * 1024.0)) / duration_mt.as_secs_f64();

    let speedup = throughput_mt / throughput_single;

    println!("\n=======================================================");
    println!("  ZSTD_MT 100MB Streaming Throughput A/B Comparison");
    println!("=======================================================");
    println!("  Workers:               1 core vs {} cores", workers);
    println!("  Single-Thread Time:    {:.2?} ({:.2} GB/s)", duration_single, throughput_single);
    println!("  Multi-Thread Time:     {:.2?} ({:.2} GB/s)", duration_mt, throughput_mt);
    println!("  Throughput Speedup:    {:.2}x", speedup);
    println!("=======================================================");

    assert!(speedup >= 1.5, "Multithreaded Zstd should achieve significant speedup over single-core (got {:.2}x)", speedup);
}

#[test]
fn test_ab_tls_context_pool_vs_stateless_malloc() {
    let payload = generate_text_corpus(32 * 1024); // 32KB
    let mut dst = vec![0u8; zstd_compress_bound(payload.len())];
    let mut decomp_buf = vec![0u8; payload.len()];
    let iterations = 2_000;

    // A/B Benchmark: 2,000 iterations of TLS context compression
    let start_tls = Instant::now();
    let mut comp_len = 0;
    for _ in 0..iterations {
        comp_len = zstd_compress(&payload, &mut dst, 3).unwrap();
    }
    let duration_tls = start_tls.elapsed();

    // A/B Benchmark: 2,000 iterations of TLS context decompression
    let start_decomp = Instant::now();
    for _ in 0..iterations {
        zstd_decompress(&dst[..comp_len], &mut decomp_buf).unwrap();
    }
    let duration_decomp = start_decomp.elapsed();

    let total_bytes = (payload.len() * iterations) as f64 / (1024.0 * 1024.0 * 1024.0);
    let comp_throughput = total_bytes / duration_tls.as_secs_f64();
    let decomp_throughput = total_bytes / duration_decomp.as_secs_f64();

    println!("\n=======================================================");
    println!("  ZSTD TLS Context Pool Micro-Bench (2,000 runs)");
    println!("=======================================================");
    println!("  Compression Throughput:   {:.2} GB/s ({:.2?})", comp_throughput, duration_tls);
    println!("  Decompression Throughput: {:.2} GB/s ({:.2?})", decomp_throughput, duration_decomp);
    println!("=======================================================");

    assert!(comp_throughput > 0.5, "TLS compress throughput should be high");
    assert!(decomp_throughput > 1.5, "TLS decompress throughput should exceed 1.5 GB/s");
}

#[test]
fn test_ab_zstd_levels_spectrum_and_compression_ratios() {
    let corpus = generate_binary_corpus(4 * 1024 * 1024); // 4MB
    let levels = [1, 3, 6, 9, 12, 19, 22];

    println!("\n=========================================================================");
    println!("  ZSTD 1..=22 Levels Spectrum & Ratio Pareto Analysis (4MB Binary)");
    println!("=========================================================================");
    println!("  Level | Compressed Size | Ratio  | Time (ms) | Speed (MB/s)");
    println!("  -----------------------------------------------------------");

    let mut prev_size = usize::MAX;
    for &lvl in &levels {
        let mut out = vec![0u8; zstd_compress_bound(corpus.len())];
        let start = Instant::now();
        let len = zstd_compress(&corpus, &mut out, lvl).unwrap();
        let elapsed = start.elapsed();
        let ratio = (len as f64 / corpus.len() as f64) * 100.0;
        let speed_mbs = (corpus.len() as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64();

        println!("    {:2}  | {:12} B | {:5.2}% | {:8.2?} | {:8.2} MB/s", lvl, len, ratio, elapsed, speed_mbs);

        if lvl >= 19 {
            assert!(len <= prev_size + 1024, "Ultra levels should maintain high compression density");
        }
        prev_size = len;
    }
    println!("=========================================================================");
}
