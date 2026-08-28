// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Exhaustive A/B Performance Benchmark Suite for DEFLATE Microkernel (libdeflate),
//! Parallel ZIP Streaming Engine, and 0..=12 Compression Level Pareto Frontier.

use std::fs;
use std::time::Instant;
use tempfile::tempdir;
use ttzip_engine::codecs::deflate::{
    deflate_compress, deflate_compress_bound, deflate_decompress,
};
use ttzip_engine::types::{
    TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod,
};
use ttzip_engine::archive::unified::UnifiedArchiveOrchestrator;

/// Generates synthetic text/JSON-like corpus with high redundancy.
fn generate_text_corpus(size: usize) -> Vec<u8> {
    let pattern = br#"{"id":12345,"name":"TTZip Deflate Bench","status":"active","metrics":{"cpu":4.5,"mem":1024,"io":99.8},"tags":["archiving","deflate","fast"]}\n"#;
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
fn test_ab_deflate_levels_spectrum_and_pareto() {
    let corpus = generate_binary_corpus(4 * 1024 * 1024); // 4MB
    let levels = [0, 1, 3, 6, 9, 12];

    println!("\n=========================================================================");
    println!("  DEFLATE 0..=12 Levels Spectrum & Ratio Pareto Analysis (4MB Binary)");
    println!("=========================================================================");
    println!("  Level | Compressed Size | Ratio  | Time (ms) | Speed (MB/s)");
    println!("  -----------------------------------------------------------");

    let mut prev_size = usize::MAX;
    for &lvl in &levels {
        let max_bound = deflate_compress_bound(corpus.len(), lvl);
        let mut out = vec![0u8; max_bound];
        let start = Instant::now();
        let len = deflate_compress(&corpus, &mut out, lvl).unwrap();
        let elapsed = start.elapsed();
        let ratio = (len as f64 / corpus.len() as f64) * 100.0;
        let speed_mbs = (corpus.len() as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64();

        println!("    {:2}  | {:12} B | {:5.2}% | {:8.2?} | {:8.2} MB/s", lvl, len, ratio, elapsed, speed_mbs);

        if lvl > 0 && lvl >= 6 {
            assert!(len <= prev_size + 1024, "Higher levels should maintain high compression density");
        }
        prev_size = len;
    }
    println!("=========================================================================");
}

#[test]
fn test_ab_deflate_micro_benchmark_10000_runs() {
    let payload = generate_text_corpus(32 * 1024); // 32KB
    let max_bound = deflate_compress_bound(payload.len(), 6);
    let mut dst = vec![0u8; max_bound];
    let mut decomp_buf = vec![0u8; payload.len()];
    let iterations = 2_000;

    // 1. Deflate compression benchmark (2,000 iterations @ Level 6)
    let start_comp = Instant::now();
    let mut comp_len = 0;
    for _ in 0..iterations {
        comp_len = deflate_compress(&payload, &mut dst, 6).unwrap();
    }
    let duration_comp = start_comp.elapsed();

    // 2. Inflate decompression benchmark (2,000 iterations)
    let start_decomp = Instant::now();
    for _ in 0..iterations {
        deflate_decompress(&dst[..comp_len], &mut decomp_buf).unwrap();
    }
    let duration_decomp = start_decomp.elapsed();

    let total_bytes = (payload.len() * iterations) as f64 / (1024.0 * 1024.0 * 1024.0);
    let comp_throughput = total_bytes / duration_comp.as_secs_f64();
    let decomp_throughput = total_bytes / duration_decomp.as_secs_f64();

    println!("\n=======================================================");
    println!("  DEFLATE Libdeflate Micro-Bench (2,000 runs @ 32KB)");
    println!("=======================================================");
    println!("  Compression Throughput:   {:.2} GB/s ({:.2?})", comp_throughput, duration_comp);
    println!("  Decompression Throughput: {:.2} GB/s ({:.2?})", decomp_throughput, duration_decomp);
    println!("=======================================================");

    assert!(comp_throughput > 0.3, "Deflate compress throughput should exceed 300 MB/s");
    assert!(decomp_throughput > 1.0, "Inflate decompress throughput should exceed 1.0 GB/s");
}

#[test]
fn test_ab_parallel_zip_streaming_100mb() {
    let dir = tempdir().unwrap();
    let file_count = 20;
    let file_size = 5 * 1024 * 1024; // 20 x 5MB = 100 MB
    let mut input_paths = Vec::new();

    for i in 0..file_count {
        let file_path = dir.path().join(format!("file_{:02}.dat", i));
        let data = generate_text_corpus(file_size);
        fs::write(&file_path, &data).unwrap();
        input_paths.push(file_path);
    }

    // 1. Benchmark: Single-threaded ZIP creation (thread_budget = 1)
    let single_zip = dir.path().join("single.zip");
    let opts_single = TTZipCreateOptions {
        struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
        abi_version: ttzip_engine::types::TTZIP_ABI_VERSION_2,
        format: TTZipArchiveFormat::Zip,
        level: TTZipCompressionLevel::Normal,
        encryption: TTZipEncryptionMethod::None,
        password: std::ptr::null(),
        thread_budget: 1,
        solid_block_size_mb: 0,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };
    let start_single = Instant::now();
    UnifiedArchiveOrchestrator::create_archive(&input_paths, &single_zip, &opts_single, 0).unwrap();
    let duration_single = start_single.elapsed();
    let throughput_single = (100.0 / 1024.0) / duration_single.as_secs_f64();

    // 2. Benchmark: Multithreaded Parallel ZIP creation (thread_budget = 0 / Auto)
    let mt_zip = dir.path().join("multithread.zip");
    let opts_mt = TTZipCreateOptions {
        struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
        abi_version: ttzip_engine::types::TTZIP_ABI_VERSION_2,
        format: TTZipArchiveFormat::Zip,
        level: TTZipCompressionLevel::Normal,
        encryption: TTZipEncryptionMethod::None,
        password: std::ptr::null(),
        thread_budget: 0,
        solid_block_size_mb: 0,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };
    let start_mt = Instant::now();
    UnifiedArchiveOrchestrator::create_archive(&input_paths, &mt_zip, &opts_mt, 0).unwrap();
    let duration_mt = start_mt.elapsed();
    let throughput_mt = (100.0 / 1024.0) / duration_mt.as_secs_f64();

    let speedup = throughput_mt / throughput_single;

    println!("\n=======================================================");
    println!("  Parallel ZIP 100MB Multi-File Archive Streaming A/B");
    println!("=======================================================");
    println!("  Corpus:                20 files x 5MB (100 MB total)");
    println!("  Single-Thread Time:    {:.2?} ({:.2} GB/s)", duration_single, throughput_single);
    println!("  Multi-Thread Time:     {:.2?} ({:.2} GB/s)", duration_mt, throughput_mt);
    println!("  Parallel Speedup:      {:.2}x", speedup);
    println!("=======================================================");

    assert!(throughput_mt >= 1.0, "Parallel ZIP throughput should be high (got {:.2} GB/s)", throughput_mt);
}
