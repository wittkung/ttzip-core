// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Extension benchmark suite for Modern Block Codecs, Zstandard Advanced/Dict, and Cryptography.

use std::hint::black_box;
use std::time::Duration;

use ttzip_engine::benchmark::codecs_driver::{
    BrotliBenchmarkDriver, Bzip2BenchmarkDriver, CodecBenchmarkDriver, Lz4BenchmarkDriver,
    LzfseBenchmarkDriver, PpmdBenchmarkDriver, SnappyBenchmarkDriver, ZstdBenchmarkDriver,
    ZstdDictBenchmarkDriver, ZstdLdmBenchmarkDriver,
};
use ttzip_engine::benchmark::corpus::{BenchmarkCorpusGenerator, BenchmarkCorpusType};
use ttzip_engine::benchmark::crypto_driver::MatrixCryptoDriver;

pub fn run_ab_modern_block_codecs_benchmarks<F, T>(mut bench_min: F, format_throughput: T)
where
    F: FnMut(&mut dyn FnMut()) -> Duration,
    T: Fn(usize, Duration) -> String,
{
    println!("─── [7] Modern Block Codecs: Throughput & Ratio (1MB JSON vs Mach-O Binary) ────");
    println!(
        "{:<18} | {:<16} | {:<18} | {:<18} | {:<12}",
        "Corpus Type", "Codec / Level", "Compress Speed", "Decompress Speed", "Compressed"
    );
    println!("-------------------+------------------+--------------------+--------------------+-------------");

    let corpora = [
        (BenchmarkCorpusType::TextData, "JSON Text (1MB)"),
        (BenchmarkCorpusType::MachOBinary, "Mach-O Bin (1MB)"),
    ];

    let lz4 = Lz4BenchmarkDriver;
    let snappy = SnappyBenchmarkDriver;
    let lzfse = LzfseBenchmarkDriver;
    let brotli = BrotliBenchmarkDriver;
    let bzip2 = Bzip2BenchmarkDriver;
    let ppmd = PpmdBenchmarkDriver;

    let test_drivers: Vec<(&str, &dyn CodecBenchmarkDriver, i32)> = vec![
        ("LZ4-Fast (Acc 1)", &lz4, 1),
        ("LZ4-Fast (Acc 10)", &lz4, 10),
        ("LZ4-HC (L9)", &lz4, 9),
        ("Snappy-Raw", &snappy, 1),
        ("Snappy-Framed", &snappy, 2),
        ("Apple LZFSE", &lzfse, 1),
        ("Apple LZVN", &lzfse, 2),
        ("Brotli (Q4)", &brotli, 4),
        ("Brotli (Q11)", &brotli, 11),
        ("Bzip2 (L9)", &bzip2, 9),
        ("PPMd (Order 6)", &ppmd, 6),
    ];

    for (corpus_type, corpus_name) in corpora {
        let corpus_data = BenchmarkCorpusGenerator::generate(corpus_type, 1024 * 1024);

        for (idx, &(codec_label, driver, level)) in test_drivers.iter().enumerate() {
            let mut compressed = Vec::new();
            let mut comp_fn = || {
                compressed = black_box(driver.bench_compress(&corpus_data, level).unwrap());
            };
            let comp_dur = bench_min(&mut comp_fn);

            let mut decomp_fn = || {
                let decompressed = driver.bench_decompress(&compressed, corpus_data.len()).unwrap();
                black_box(decompressed);
            };
            let decomp_dur = bench_min(&mut decomp_fn);

            let prefix = if idx == 0 { corpus_name } else { "" };
            println!(
                "{:<18} | {:<16} | {:<18} | {:<18} | {:<12}",
                prefix,
                codec_label,
                format_throughput(corpus_data.len(), comp_dur),
                format_throughput(corpus_data.len(), decomp_dur),
                format!("{} B", compressed.len())
            );
        }
        println!("-------------------+------------------+--------------------+--------------------+-------------");
    }
    println!();
}

pub fn run_ab_zstd_advanced_and_dict_benchmarks<F, T>(mut bench_min: F, format_throughput: T)
where
    F: FnMut(&mut dyn FnMut()) -> Duration,
    T: Fn(usize, Duration) -> String,
{
    println!("─── [8] Zstd Advanced: 112KB Shared Dictionary & LDM 64MB Deduplication ─────────");
    println!(
        "{:<32} | {:<16} | {:<18} | {:<12}",
        "Scenario / Strategy", "Mode", "Throughput", "Total Size / Gain"
    );
    println!("---------------------------------+------------------+--------------------+-------------");

    let num_small_files = 1000;
    let small_file_size = 1024;
    let small_corpus = BenchmarkCorpusGenerator::generate(BenchmarkCorpusType::TextData, small_file_size);
    let small_files: Vec<Vec<u8>> = (0..num_small_files)
        .map(|i: usize| {
            let mut d = small_corpus.clone();
            d[0..8].copy_from_slice(&(i as u64).to_le_bytes());
            d
        })
        .collect();
    let total_small_bytes = num_small_files * small_file_size;

    let zstd_std = ZstdBenchmarkDriver;
    let zstd_dict = ZstdDictBenchmarkDriver;

    let mut std_total_size = 0;
    let mut std_fn = || {
        std_total_size = 0;
        for f in &small_files {
            let c = zstd_std.bench_compress(f, 3).unwrap();
            std_total_size += c.len();
        }
    };
    let std_dur = bench_min(&mut std_fn);

    let mut dict_total_size = 0;
    let mut dict_fn = || {
        dict_total_size = 0;
        for f in &small_files {
            let c = zstd_dict.bench_compress(f, 3).unwrap();
            dict_total_size += c.len();
        }
    };
    let dict_dur = bench_min(&mut dict_fn);

    let dict_speedup = std_dur.as_secs_f64() / dict_dur.as_secs_f64();

    println!(
        "{:<32} | {:<16} | {:<18} | {:<12}",
        "1,000x 1KB JSON (No Dict)", "Standard Zstd L3", format_throughput(total_small_bytes, std_dur), format!("{} B", std_total_size)
    );
    println!(
        "{:<32} | {:<16} | {:<18} | \x1b[32m{} B\x1b[0m (\x1b[32m{:.2}x speed\x1b[0m)",
        "1,000x 1KB JSON (112KB Dict)", "Dict Zstd L3", format_throughput(total_small_bytes, dict_dur), dict_total_size, dict_speedup
    );
    println!("---------------------------------+------------------+--------------------+-------------");

    let zstd_ldm = ZstdLdmBenchmarkDriver;
    let sample_128k = BenchmarkCorpusGenerator::generate(BenchmarkCorpusType::MachOBinary, 128 * 1024);
    let mut ldm_corpus = Vec::with_capacity(10 * 1024 * 1024);
    while ldm_corpus.len() + sample_128k.len() <= 10 * 1024 * 1024 {
        ldm_corpus.extend_from_slice(&sample_128k);
    }

    let mut std_ldm_size = 0;
    let mut std_ldm_fn = || {
        let c = zstd_std.bench_compress(&ldm_corpus, 3).unwrap();
        std_ldm_size = c.len();
    };
    let std_ldm_dur = bench_min(&mut std_ldm_fn);

    let mut ldm_size = 0;
    let mut ldm_fn = || {
        let c = zstd_ldm.bench_compress(&ldm_corpus, 3).unwrap();
        ldm_size = c.len();
    };
    let ldm_dur = bench_min(&mut ldm_fn);

    println!(
        "{:<32} | {:<16} | {:<18} | {:<12}",
        "10MB Repeating Mach-O (No LDM)", "Standard Zstd L3", format_throughput(ldm_corpus.len(), std_ldm_dur), format!("{} B", std_ldm_size)
    );
    println!(
        "{:<32} | {:<16} | {:<18} | \x1b[32m{} B\x1b[0m",
        "10MB Repeating Mach-O (LDM)", "Zstd LDM (64MB)", format_throughput(ldm_corpus.len(), ldm_dur), ldm_size
    );
    println!();
}

pub fn run_ab_crypto_and_hash_matrix_benchmarks<F, T>(mut bench_min: F, format_throughput: T)
where
    F: FnMut(&mut dyn FnMut()) -> Duration,
    T: Fn(usize, Duration) -> String,
{
    println!("─── [9] Unified Cryptography & Multi-Scale Hashing Matrix (11 Algorithms) ────────");
    println!(
        "{:<28} | {:<16} | {:<18} | {:<18}",
        "Algorithm / Primitive", "Category", "10MB Continuous", "10,000x 1KB Chunks"
    );
    println!("-----------------------------+------------------+--------------------+--------------------");

    let buf_10mb = BenchmarkCorpusGenerator::generate(BenchmarkCorpusType::RealisticRgb, 10 * 1024 * 1024);

    let num_chunks = 10000;
    let chunk_size = 1024;
    let chunks: Vec<Vec<u8>> = (0..num_chunks).map(|_| vec![0x7Eu8; chunk_size]).collect();
    let total_chunk_bytes = num_chunks * chunk_size;

    let drivers = MatrixCryptoDriver::all_drivers();
    for driver in drivers {
        let name = driver.algorithm_id();
        let cat = driver.category().as_str();

        let mut dur_10mb_fn = || {
            black_box(driver.bench_process(&buf_10mb).unwrap());
        };
        let dur_10mb = bench_min(&mut dur_10mb_fn);

        let mut dur_chunks_fn = || {
            for c in &chunks {
                black_box(driver.bench_process(c).unwrap());
            }
        };
        let dur_chunks = bench_min(&mut dur_chunks_fn);

        println!(
            "{:<28} | {:<16} | {:<18} | {:<18}",
            name,
            cat,
            format_throughput(buf_10mb.len(), dur_10mb),
            format_throughput(total_chunk_bytes, dur_chunks)
        );
    }
    println!();
}