// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Exhaustive 0..=12 All-Levels Matrix Benchmark Suite for DEFLATE Microkernel.
//! Covers all 13 individual compression levels (0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12)
//! across 4 representative industrial corpus types (Text/Log, JSON/Code, Mach-O Binary, High-Entropy).

use std::time::Instant;
use ttzip_engine::codecs::deflate::{
    deflate_compress, deflate_compress_bound, deflate_decompress,
};

/// 1. High-redundancy repetitive log/text corpus.
fn generate_text_log_corpus(size: usize) -> Vec<u8> {
    let pattern = br#"2026-08-28T15:40:00.123Z [INFO] ttzip::microkernel::deflate - Connection pool dispatched worker=18 status=OK latency=0.23ms\n"#;
    let mut data = Vec::with_capacity(size);
    while data.len() < size {
        let chunk = (size - data.len()).min(pattern.len());
        data.extend_from_slice(&pattern[..chunk]);
    }
    data
}

/// 2. Structured JSON / Source Code corpus (AST-like syntax).
fn generate_code_json_corpus(size: usize) -> Vec<u8> {
    let pattern = br#"{"event":"archive_create","version":"2.0.0","options":{"level":6,"threads":18,"algorithm":"libdeflate"},"items":[{"path":"src/main.rs","size":4096,"crc":3735928559}]}\n"#;
    let mut data = Vec::with_capacity(size);
    while data.len() < size {
        let chunk = (size - data.len()).min(pattern.len());
        data.extend_from_slice(&pattern[..chunk]);
    }
    data
}

/// 3. Real macOS Mach-O 64-bit Executable Binary machine code.
fn generate_macho_binary_corpus(size: usize) -> Vec<u8> {
    let candidate_paths = [
        "/bin/zsh",
        "/bin/bash",
        "/usr/bin/tar",
        "../../vendor/libdeflate/build_official/programs/benchmark",
    ];

    let mut base_data = Vec::new();
    for p in &candidate_paths {
        if let Ok(content) = std::fs::read(p) {
            base_data = content;
            break;
        }
    }

    if base_data.is_empty() {
        // Fallback: ARM64 instruction sequence pattern (STP, LDP, BL, ADRP, RET)
        let arm64_pattern: [u32; 8] = [
            0xA9BF7BFD, // stp x29, x30, [sp, #-16]!
            0x910003FD, // mov x29, sp
            0x90000000, // adrp x0, 0
            0x91000000, // add x0, x0, #0
            0x94000000, // bl 0
            0xAA0003E0, // mov x0, x0
            0xA8C17BFD, // ldp x29, x30, [sp], #16
            0xD65F03C0, // ret
        ];
        let bytes: Vec<u8> = arm64_pattern.iter().flat_map(|w| w.to_le_bytes()).collect();
        while base_data.len() < size {
            base_data.extend_from_slice(&bytes);
        }
    }

    let mut data = Vec::with_capacity(size);
    while data.len() < size {
        let chunk = (size - data.len()).min(base_data.len());
        data.extend_from_slice(&base_data[..chunk]);
    }
    data
}

/// 4. High-entropy non-compressible pseudo-random stream.
fn generate_high_entropy_corpus(size: usize) -> Vec<u8> {
    let mut data = vec![0u8; size];
    let mut seed = 0x85457F23079AA01Bu64;
    for (i, byte) in data.iter_mut().enumerate() {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(i as u64);
        *byte = (seed >> 32) as u8;
    }
    data
}

#[allow(dead_code)]
struct LevelBenchmarkResult {
    level: i32,
    orig_size: usize,
    comp_size: usize,
    ratio: f64,
    comp_time_us: u128,
    comp_speed_mbs: f64,
    decomp_time_us: u128,
    decomp_speed_mbs: f64,
}

fn benchmark_corpus_all_13_levels(corpus_name: &str, corpus: &[u8]) {
    println!("\n=========================================================================================================");
    println!("  DEFLATE 0..=12 All-Levels Comprehensive Matrix Analysis: [{}] (Size: {} B)", corpus_name, corpus.len());
    println!("=========================================================================================================");
    println!("  Lvl | Mode / Strategy      | Comp Size   | Ratio (%) | Comp Time    | Comp Speed   | Decomp Speed   ");
    println!("  -------------------------------------------------------------------------------------------------------");

    let mut results = Vec::new();

    for level in 0..=12 {
        let max_bound = deflate_compress_bound(corpus.len(), level);
        let mut out = vec![0u8; max_bound];
        let mut decomp_buf = vec![0u8; corpus.len()];

        // 1. Compression Benchmark (Median of 3 runs for stability)
        let mut comp_times = Vec::new();
        let mut comp_len = 0;
        for _ in 0..3 {
            let start = Instant::now();
            comp_len = deflate_compress(corpus, &mut out, level).expect("compress");
            comp_times.push(start.elapsed().as_micros());
        }
        comp_times.sort_unstable();
        let comp_time_us = comp_times[1]; // median
        let ratio = (comp_len as f64 / corpus.len() as f64) * 100.0;
        let comp_speed_mbs = (corpus.len() as f64 / (1024.0 * 1024.0)) / (comp_time_us as f64 / 1_000_000.0);

        // 2. Decompression Benchmark (Median of 3 runs)
        let mut decomp_times = Vec::new();
        for _ in 0..3 {
            let start = Instant::now();
            deflate_decompress(&out[..comp_len], &mut decomp_buf).expect("decompress");
            decomp_times.push(start.elapsed().as_micros());
        }
        decomp_times.sort_unstable();
        let decomp_time_us = decomp_times[1];
        let decomp_speed_mbs = (corpus.len() as f64 / (1024.0 * 1024.0)) / (decomp_time_us as f64 / 1_000_000.0);

        // Verify lossless roundtrip integrity
        assert_eq!(&decomp_buf[..], corpus, "Lossless roundtrip must match exactly at level {}", level);

        let strategy_desc = match level {
            0 => "Store / Bypass      ",
            1 => "Fast Greedy HC3     ",
            2 => "Fast Hash-Chain     ",
            3 => "Fast HC3/HC4        ",
            4 => "Balanced HC4        ",
            5 => "Balanced Lazy HC4   ",
            6 => "Standard Lazy HC4   ",
            7 => "Deep Lazy HC4       ",
            8 => "Deep Lazy HC4 Max   ",
            9 => "Maximum Lazy HC4    ",
            10 => "Ultra BT Matchfinder ",
            11 => "Ultra BT Deep Search ",
            12 => "Ultra BT Opt-Parser  ",
            _ => "Unknown             ",
        };

        println!(
            "  {:2}  | {} | {:11} | {:8.2}% | {:8} µs | {:9.2} MB/s | {:9.2} MB/s",
            level, strategy_desc, comp_len, ratio, comp_time_us, comp_speed_mbs, decomp_speed_mbs
        );

        results.push(LevelBenchmarkResult {
            level,
            orig_size: corpus.len(),
            comp_size: comp_len,
            ratio,
            comp_time_us,
            comp_speed_mbs,
            decomp_time_us,
            decomp_speed_mbs,
        });
    }

    println!("=========================================================================================================");

    // Verify monotonic Pareto properties for non-random data
    if corpus_name != "High-Entropy Random Stream" {
        for i in 1..results.len() {
            if results[i].level >= 6 {
                // Higher levels should yield smaller or equal compressed sizes
                assert!(
                    results[i].comp_size <= results[i - 1].comp_size + 64,
                    "Level {} should produce size <= Level {} (got {} vs {})",
                    results[i].level,
                    results[i - 1].level,
                    results[i].comp_size,
                    results[i - 1].comp_size
                );
            }
        }
    }
}

#[test]
fn test_exhaustive_all_13_levels_across_4_corpora() {
    let text_corpus = generate_text_log_corpus(2 * 1024 * 1024); // 2 MB
    benchmark_corpus_all_13_levels("High-Redundancy Text Logs", &text_corpus);

    let code_corpus = generate_code_json_corpus(2 * 1024 * 1024); // 2 MB
    benchmark_corpus_all_13_levels("Structured JSON & Source Code", &code_corpus);

    let binary_corpus = generate_macho_binary_corpus(2 * 1024 * 1024); // 2 MB
    benchmark_corpus_all_13_levels("Mach-O Binary Machine Code", &binary_corpus);

    let high_entropy = generate_high_entropy_corpus(1024 * 1024); // 1 MB
    benchmark_corpus_all_13_levels("High-Entropy Random Stream", &high_entropy);
}
