// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Exhaustive Full-Spectrum A/B Regression & Performance Benchmark Suite.
//!
//! Evaluates speedup, throughput (GB/s, ns/op), and zero-regression invariants
//! directly comparing:
//! - [BASELINE] TTZip Previous Commit (HEAD~1: 738db4b2) Dependencies & Implementations:
//!     - CRC-32: `crc32fast::hash` (Rust Ecosystem Standard SIMD/Slice-by-16)
//!     - Adler-32: `adler2::adler32_slice` (Rust Ecosystem Standard)
//!     - DEFLATE: `flate2::write::DeflateEncoder` (miniz_oxide / standard flate2 engine)
//!     - Matchfinder: Standard byte-by-byte loop comparison
//!     - Bitstream: Standard single-byte push accumulator & reader
//! - [CURRENT CANDIDATE] TTZip Libdeflate Architectural Absorption & Hardening:
//!     - CRC-32: `ttzip_engine::checksum::crc32` (Direct Hardware ACLE + 12-Way PMULL)
//!     - Adler-32: `ttzip_engine::checksum::adler32` (Scalar Fastpath + NEON DotProd)
//!     - DEFLATE: `DeflateCompressor` / `DeflateDecompressor` (Store + 12-Level Vectorized Engine)
//!     - Matchfinder: `lz_extend` (SWAR 64-bit XOR + Hardware CTZ)
//!     - Bitstream: `BitWriter` / `BitReader` (64-bit Wide-Register Branchless)

use std::hint::black_box;
use std::io::Write;
use std::time::{Duration, Instant};
use rayon::prelude::*;

#[path = "ab_perf_suite_ext.rs"]
mod ab_perf_suite_ext;

// Current Commit Real Implementations
use ttzip_engine::checksum::{adler32 as current_adler32, crc32 as current_crc32};
use ttzip_engine::codecs::deflate::compressor::DeflateCompressor;
use ttzip_engine::codecs::deflate::decompressor::DeflateDecompressor;
use ttzip_engine::codecs::deflate::{with_thread_local_compressor, with_thread_local_decompressor};
use ttzip_engine::utils::{lz_extend, BitReader, BitWriter};

// Previous Commit (HEAD~1) Real Dependencies & Implementations
use adler2::adler32_slice as baseline_adler32;
use crc32fast::hash as baseline_crc32;
use flate2::write::{DeflateDecoder as Flate2Decoder, DeflateEncoder as Flate2Encoder};
use flate2::Compression as Flate2Compression;

const WARMUP_RUNS: usize = 3;
const MEASURE_RUNS: usize = 10;

// ============================================================================
// Previous Commit (HEAD~1) Baseline Algorithms for Matchfinder & Bitstream
// ============================================================================

fn previous_commit_lz_extend(src: &[u8], match_slice: &[u8], start_len: usize) -> usize {
    let max_len = src.len().min(match_slice.len());
    let mut len = start_len.min(max_len);
    while len < max_len && src[len] == match_slice[len] {
        len += 1;
    }
    len
}

struct PreviousCommitBitWriter {
    buf: Vec<u8>,
    bitbuf: u8,
    bitcount: u8,
}

impl PreviousCommitBitWriter {
    fn new() -> Self {
        Self {
            buf: Vec::new(),
            bitbuf: 0,
            bitcount: 0,
        }
    }
    fn write_bits(&mut self, mut val: u64, mut nbits: u32) {
        while nbits > 0 {
            let bit = (val & 1) as u8;
            self.bitbuf |= bit << self.bitcount;
            self.bitcount += 1;
            if self.bitcount == 8 {
                self.buf.push(self.bitbuf);
                self.bitbuf = 0;
                self.bitcount = 0;
            }
            val >>= 1;
            nbits -= 1;
        }
    }
    fn finish(mut self) -> Vec<u8> {
        if self.bitcount > 0 {
            self.buf.push(self.bitbuf);
        }
        self.buf
    }
}

struct PreviousCommitBitReader<'a> {
    data: &'a [u8],
    bit_pos: usize,
}

impl<'a> PreviousCommitBitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, bit_pos: 0 }
    }
    fn read_bits(&mut self, nbits: u32) -> Option<u64> {
        if self.bit_pos + nbits as usize > self.data.len() * 8 {
            return None;
        }
        let mut val = 0u64;
        for i in 0..nbits {
            let byte_idx = (self.bit_pos + i as usize) / 8;
            let bit_idx = (self.bit_pos + i as usize) % 8;
            let bit = ((self.data[byte_idx] >> bit_idx) & 1) as u64;
            val |= bit << i;
        }
        self.bit_pos += nbits as usize;
        Some(val)
    }
}

// ============================================================================
// Benchmarking Measurement Helpers
// ============================================================================

fn bench_min<F, R>(mut f: F) -> Duration
where
    F: FnMut() -> R,
{
    for _ in 0..WARMUP_RUNS {
        black_box(f());
    }
    let mut best = Duration::from_secs(999);
    for _ in 0..MEASURE_RUNS {
        let start = Instant::now();
        black_box(f());
        let elapsed = start.elapsed();
        if elapsed < best {
            best = elapsed;
        }
    }
    best
}

fn bench_min_dyn(f: &mut dyn FnMut()) -> Duration {
    for _ in 0..WARMUP_RUNS {
        black_box(f());
    }
    let mut best = Duration::from_secs(999);
    for _ in 0..MEASURE_RUNS {
        let start = Instant::now();
        black_box(f());
        let elapsed = start.elapsed();
        if elapsed < best {
            best = elapsed;
        }
    }
    best
}

fn format_throughput(bytes: usize, dur: Duration) -> String {
    let secs = dur.as_secs_f64();
    let gb = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    let gb_per_sec = gb / secs;
    if gb_per_sec >= 1.0 {
        format!("{:.2} GB/s", gb_per_sec)
    } else {
        let mb = bytes as f64 / (1024.0 * 1024.0);
        format!("{:.2} MB/s", mb / secs)
    }
}

// ============================================================================
// Main Full-Spectrum Runner
// ============================================================================

fn main() {
    println!("==================================================================================");
    println!("     TTZip Full-Spectrum A/B Performance & Regression Benchmark Suite             ");
    println!("==================================================================================");
    println!("Target Platform : {} / {}", std::env::consts::OS, std::env::consts::ARCH);
    println!("Baseline Target : HEAD~1 (Commit 738db4b2: crc32fast, adler2, flate2)");
    println!("Candidate Target: Current Commit (12-Way PMULL, libdeflate, SWAR LZ, Wide Bitstream)");
    println!("Measurement Runs: {} runs (minimum elapsed time)", MEASURE_RUNS);
    println!("----------------------------------------------------------------------------------\n");

    run_ab_checksum_benchmarks();
    run_ab_unaligned_checksum_benchmarks();
    run_ab_matchfinder_benchmarks();
    run_ab_bitstream_benchmarks();
    run_ab_multi_corpus_deflate_benchmarks();
    run_ab_multicore_parallel_benchmarks();
    ab_perf_suite_ext::run_ab_modern_block_codecs_benchmarks(bench_min_dyn, format_throughput);
    ab_perf_suite_ext::run_ab_zstd_advanced_and_dict_benchmarks(bench_min_dyn, format_throughput);
    ab_perf_suite_ext::run_ab_crypto_and_hash_matrix_benchmarks(bench_min_dyn, format_throughput);

    println!("\n==================================================================================");
    println!("     🏁 Full-Spectrum A/B Benchmark Execution Completed                           ");
    println!("==================================================================================");
}

fn run_ab_checksum_benchmarks() {
    println!("─── [1] Checksum A/B Matrix: Multi-Scale Payloads (0B to 10MB) ───────────────────");
    println!(
        "{:<14} | {:<22} | {:<22} | {:<12}",
        "Payload Size", "Baseline (HEAD~1)", "Current (TTZip HW)", "Speedup"
    );
    println!("---------------+------------------------+------------------------+-------------");

    let sizes = [
        ("0 B", 0),
        ("1 B", 1),
        ("7 B", 7),
        ("16 B", 16),
        ("64 B", 64),
        ("256 B", 256),
        ("1 KB", 1024),
        ("64 KB", 64 * 1024),
        ("1 MB", 1024 * 1024),
        ("10 MB", 10 * 1024 * 1024),
    ];

    for &(label, size) in &sizes {
        let payload = vec![0xABu8; size];
        let iters = if size == 0 { 1_000_000 } else { (10 * 1024 * 1024 / size).max(10) };
        let total_bytes = size * iters;

        // CRC-32 A/B
        let base_crc_dur = bench_min(|| {
            let mut acc = 0;
            for _ in 0..iters {
                acc = black_box(baseline_crc32(&payload));
            }
            acc
        });

        let cur_crc_dur = bench_min(|| {
            let mut acc = 0;
            for _ in 0..iters {
                acc = black_box(current_crc32(0, &payload));
            }
            acc
        });

        let crc_speedup = base_crc_dur.as_secs_f64() / cur_crc_dur.as_secs_f64();
        let speedup_str = if crc_speedup >= 1.0 {
            format!("\x1b[32m{:.2}x\x1b[0m", crc_speedup)
        } else {
            format!("\x1b[31m{:.2}x (Regression)\x1b[0m", crc_speedup)
        };
        let base_tp = if size == 0 { format!("{:.2} ns/op", base_crc_dur.as_nanos() as f64 / iters as f64) } else { format_throughput(total_bytes, base_crc_dur) };
        let cur_tp = if size == 0 { format!("{:.2} ns/op", cur_crc_dur.as_nanos() as f64 / iters as f64) } else { format_throughput(total_bytes, cur_crc_dur) };

        println!("CRC-32 {:<7} | {:<22} | {:<22} | {}", label, base_tp, cur_tp, speedup_str);

        // Adler-32 A/B
        let base_adler_dur = bench_min(|| {
            let mut acc = 1;
            for _ in 0..iters {
                acc = black_box(baseline_adler32(&payload));
            }
            acc
        });

        let cur_adler_dur = bench_min(|| {
            let mut acc = 1;
            for _ in 0..iters {
                acc = black_box(current_adler32(1, &payload));
            }
            acc
        });

        let adler_speedup = base_adler_dur.as_secs_f64() / cur_adler_dur.as_secs_f64();
        let adler_speedup_str = if adler_speedup >= 1.0 {
            format!("\x1b[32m{:.2}x\x1b[0m", adler_speedup)
        } else {
            format!("\x1b[31m{:.2}x (Regression)\x1b[0m", adler_speedup)
        };
        let base_adler_tp = if size == 0 { format!("{:.2} ns/op", base_adler_dur.as_nanos() as f64 / iters as f64) } else { format_throughput(total_bytes, base_adler_dur) };
        let cur_adler_tp = if size == 0 { format!("{:.2} ns/op", cur_adler_dur.as_nanos() as f64 / iters as f64) } else { format_throughput(total_bytes, cur_adler_dur) };

        println!("Adler-32 {:<5} | {:<22} | {:<22} | {}", label, base_adler_tp, cur_adler_tp, adler_speedup_str);
    }
    println!();
}

fn run_ab_unaligned_checksum_benchmarks() {
    println!("─── [2] Checksum A/B Matrix: Unaligned Memory Pointers (&buf[1..], &buf[3..]) ───");
    println!(
        "{:<18} | {:<22} | {:<22} | {:<12}",
        "Offset Scenario", "Baseline (HEAD~1)", "Current (TTZip HW)", "Speedup"
    );
    println!("-------------------+------------------------+------------------------+-------------");

    let raw_buf = vec![0xCCu8; 64 * 1024 + 16];
    let iters = 200;

    for &offset in &[1usize, 3, 7, 15] {
        let slice = &raw_buf[offset..offset + 64 * 1024];
        let total_bytes = 64 * 1024 * iters;

        let base_crc_dur = bench_min(|| {
            let mut acc = 0;
            for _ in 0..iters {
                acc = black_box(baseline_crc32(slice));
            }
            acc
        });

        let cur_crc_dur = bench_min(|| {
            let mut acc = 0;
            for _ in 0..iters {
                acc = black_box(current_crc32(0, slice));
            }
            acc
        });

        let speedup = base_crc_dur.as_secs_f64() / cur_crc_dur.as_secs_f64();
        println!(
            "CRC-32 Offset +{:<2} | {:<22} | {:<22} | \x1b[32m{:.2}x\x1b[0m",
            offset,
            format_throughput(total_bytes, base_crc_dur),
            format_throughput(total_bytes, cur_crc_dur),
            speedup
        );
    }
    println!();
}

fn run_ab_matchfinder_benchmarks() {
    println!("─── [3] Matchfinder A/B Matrix: Byte Loop vs SWAR 64-bit CTZ (All Match Lengths) ─");
    println!(
        "{:<26} | {:<18} | {:<18} | {:<12}",
        "Scenario", "Baseline (HEAD~1)", "Current (SWAR CTZ)", "Speedup"
    );
    println!("---------------------------+--------------------+--------------------+-------------");

    let iters = 1_000_000;

    // Scenarios: Len=0 (Mismatch at byte 0), Len=1, Len=3, Len=5, Len=32, Len=256
    let scenarios = [
        ("Mismatch at Byte 0 (Len=0)", 0),
        ("Mismatch at Byte 1 (Len=1)", 1),
        ("Mismatch at Byte 3 (Len=3)", 3),
        ("Mismatch at Byte 5 (Len=5)", 5),
        ("Mismatch at Byte 32 (Len=32)", 32),
        ("Full Match 256B (Len=256)", 256),
    ];

    for &(label, match_len) in &scenarios {
        let max_len = 256.max(match_len + 8);
        let mut src = vec![b'A'; max_len];
        let mut match_buf = vec![b'A'; max_len];
        if match_len < max_len {
            src[match_len] = b'X';
            match_buf[match_len] = b'Y';
        }

        let base_dur = bench_min(|| {
            let mut total = 0;
            for _ in 0..iters {
                total += black_box(previous_commit_lz_extend(&src, &match_buf, 0));
            }
            total
        });

        let cur_dur = bench_min(|| {
            let mut total = 0;
            for _ in 0..iters {
                total += black_box(lz_extend(&src, &match_buf, 0));
            }
            total
        });

        let speedup = base_dur.as_secs_f64() / cur_dur.as_secs_f64();
        let speedup_str = if speedup >= 1.0 {
            format!("\x1b[32m{:.2}x\x1b[0m", speedup)
        } else {
            format!("\x1b[31m{:.2}x (Regression)\x1b[0m", speedup)
        };

        println!(
            "{:<26} | {:<18} | {:<18} | {}",
            label,
            format!("{:.2} ns/op", base_dur.as_nanos() as f64 / iters as f64),
            format!("{:.2} ns/op", cur_dur.as_nanos() as f64 / iters as f64),
            speedup_str
        );
    }
    println!();
}

fn run_ab_bitstream_benchmarks() {
    println!("─── [4] Bitstream A/B Matrix: BitWriter & BitReader Throughput ───────────────────");
    println!(
        "{:<26} | {:<18} | {:<18} | {:<12}",
        "Operation", "Baseline (HEAD~1)", "Current (TTZip Bit)", "Speedup"
    );
    println!("---------------------------+--------------------+--------------------+-------------");

    let num_symbols = 500_000;
    let symbols: Vec<(u64, u32)> = (0..num_symbols)
        .map(|i| {
            let bits = ((i % 16) + 1) as u32;
            let val = (i * 37 + 13) as u64 & ((1 << bits) - 1);
            (val, bits)
        })
        .collect();

    // 1. BitWriter Benchmark
    let base_writer_dur = bench_min(|| {
        let mut writer = PreviousCommitBitWriter::new();
        for &(val, bits) in &symbols {
            writer.write_bits(val, bits);
        }
        black_box(writer.finish())
    });

    let cur_writer_dur = bench_min(|| {
        let mut writer = BitWriter::with_capacity(num_symbols);
        for &(val, bits) in &symbols {
            writer.write_bits(val, bits);
        }
        black_box(writer.finish())
    });

    let writer_speedup = base_writer_dur.as_secs_f64() / cur_writer_dur.as_secs_f64();
    println!(
        "{:<26} | {:<18} | {:<18} | \x1b[32m{:.2}x\x1b[0m",
        "BitWriter: 500k Var-Writes",
        format!("{:.2} ms", base_writer_dur.as_secs_f64() * 1000.0),
        format!("{:.2} ms", cur_writer_dur.as_secs_f64() * 1000.0),
        writer_speedup
    );

    // 2. BitReader Benchmark
    let mut writer = BitWriter::with_capacity(num_symbols);
    for &(val, bits) in &symbols {
        writer.write_bits(val, bits);
    }
    let encoded_bytes = writer.finish();

    let base_reader_dur = bench_min(|| {
        let mut reader = PreviousCommitBitReader::new(&encoded_bytes);
        let mut acc = 0u64;
        for &(_, bits) in &symbols {
            if let Some(v) = reader.read_bits(bits) {
                acc = acc.wrapping_add(v);
            }
        }
        black_box(acc)
    });

    let cur_reader_dur = bench_min(|| {
        let mut reader = BitReader::new(&encoded_bytes);
        let mut acc = 0u64;
        for &(_, bits) in &symbols {
            if let Some(v) = reader.read_bits(bits) {
                acc = acc.wrapping_add(v);
            }
        }
        black_box(acc)
    });

    let reader_speedup = base_reader_dur.as_secs_f64() / cur_reader_dur.as_secs_f64();
    println!(
        "{:<26} | {:<18} | {:<18} | \x1b[32m{:.2}x\x1b[0m",
        "BitReader: 500k Var-Reads",
        format!("{:.2} ms", base_reader_dur.as_secs_f64() * 1000.0),
        format!("{:.2} ms", cur_reader_dur.as_secs_f64() * 1000.0),
        reader_speedup
    );
    println!();
}

fn run_ab_multi_corpus_deflate_benchmarks() {
    println!("─── [5] End-to-End DEFLATE: Multi-Corpus (JSON, Mach-O Binary, High-Entropy) ────");
    println!(
        "{:<18} | {:<16} | {:<18} | {:<18} | {:<12}",
        "Corpus Type", "Algorithm / Mode", "Compress Speed", "Decompress Speed", "Compressed Size"
    );
    println!("-------------------+------------------+--------------------+--------------------+-------------");

    // Corpus 1: Structured JSON/Text (1MB)
    let mut json_corpus = Vec::with_capacity(1024 * 1024);
    let sample = b"{\"id\": 1024, \"name\": \"TTZip Benchmark Record\", \"payload\": \"Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.\"}\n";
    while json_corpus.len() + sample.len() <= 1024 * 1024 {
        json_corpus.extend_from_slice(sample);
    }

    // Corpus 2: Synthetic Mach-O Binary Code (1MB, repeated opcodes & jump offsets)
    let mut macho_corpus = Vec::with_capacity(1024 * 1024);
    let code_sample = [0x55, 0x48, 0x89, 0xe5, 0x48, 0x83, 0xec, 0x20, 0x89, 0x7d, 0xec, 0x8b, 0x45, 0xec, 0x83, 0xc0, 0x01, 0x48, 0x83, 0xc4, 0x20, 0x5d, 0xc3, 0x90];
    while macho_corpus.len() + code_sample.len() <= 1024 * 1024 {
        macho_corpus.extend_from_slice(&code_sample);
    }

    // Corpus 3: High-Entropy Pseudorandom Bytes (1MB, uncompressible)
    let mut random_corpus = vec![0u8; 1024 * 1024];
    let mut rng_state = 0x123456789ABCDEF0u64;
    for b in random_corpus.iter_mut() {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *b = (rng_state >> 33) as u8;
    }

    let corpora = [
        ("JSON Text (1MB)", json_corpus),
        ("Mach-O Bin (1MB)", macho_corpus),
        ("Random (1MB)", random_corpus),
    ];

    for (corpus_name, corpus) in corpora {
        // Baseline 1: flate2 fast
        let mut base_fast_size = 0;
        let base_fast_dur = bench_min(|| {
            let mut enc = Flate2Encoder::new(Vec::with_capacity(corpus.len()), Flate2Compression::fast());
            enc.write_all(&corpus).expect("write failed");
            let res = enc.finish().expect("finish failed");
            base_fast_size = res.len();
            black_box(res)
        });

        // Baseline 2: flate2 default
        let mut base_def_size = 0;
        let base_def_dur = bench_min(|| {
            let mut enc = Flate2Encoder::new(Vec::with_capacity(corpus.len()), Flate2Compression::default());
            enc.write_all(&corpus).expect("write failed");
            let res = enc.finish().expect("finish failed");
            base_def_size = res.len();
            black_box(res)
        });

        // TTZip Level 0 (Store / 仅存储模式)
        let mut cur_l0_comp = DeflateCompressor::new(0).unwrap();
        let bound0 = cur_l0_comp.compress_bound(corpus.len());
        let mut cur_l0_buf = vec![0u8; bound0];
        let mut cur_l0_size = 0;
        let cur_l0_dur = bench_min(|| {
            cur_l0_size = black_box(cur_l0_comp.compress(&corpus, &mut cur_l0_buf).unwrap());
        });

        // TTZip Fast Mode (Ultra-fast single-pass)
        let mut cur_fast_comp = DeflateCompressor::new_fast().unwrap();
        let bound_fast = cur_fast_comp.compress_bound(corpus.len());
        let mut cur_fast_buf = vec![0u8; bound_fast];
        let mut cur_fast_size = 0;
        let cur_fast_dur = bench_min(|| {
            cur_fast_size = black_box(cur_fast_comp.compress(&corpus, &mut cur_fast_buf).unwrap());
        });

        // TTZip Level 1 (Fast SIMD)
        let mut cur_l1_comp = DeflateCompressor::new(1).unwrap();
        let bound1 = cur_l1_comp.compress_bound(corpus.len());
        let mut cur_l1_buf = vec![0u8; bound1];
        let mut cur_l1_size = 0;
        let cur_l1_dur = bench_min(|| {
            cur_l1_size = black_box(cur_l1_comp.compress(&corpus, &mut cur_l1_buf).unwrap());
        });

        // TTZip Level 6 (Default Balanced)
        let mut cur_l6_comp = DeflateCompressor::new(6).unwrap();
        let bound6 = cur_l6_comp.compress_bound(corpus.len());
        let mut cur_l6_buf = vec![0u8; bound6];
        let mut cur_l6_size = 0;
        let cur_l6_dur = bench_min(|| {
            cur_l6_size = black_box(cur_l6_comp.compress(&corpus, &mut cur_l6_buf).unwrap());
        });

        // Decompress TTZip Level 6 payload
        let mut decompressor = DeflateDecompressor::new().unwrap();
        let mut decomp_buf = vec![0u8; corpus.len()];
        let decomp_dur = bench_min(|| {
            black_box(decompressor.decompress(&cur_l6_buf[..cur_l6_size], &mut decomp_buf).unwrap());
        });

        let flate2_decomp_dur = bench_min(|| {
            let mut dec = Flate2Decoder::new(Vec::with_capacity(corpus.len()));
            dec.write_all(&cur_l6_buf[..cur_l6_size]).unwrap();
            let res = dec.finish().unwrap();
            black_box(res)
        });

        println!(
            "{:<18} | {:<16} | {:<18} | {:<18} | {:<12}",
            corpus_name, "flate2 (fast)", format_throughput(corpus.len(), base_fast_dur), "-", format!("{} B", base_fast_size)
        );
        println!(
            "{:<18} | {:<16} | {:<18} | {:<18} | {:<12}",
            "", "flate2 (default)", format_throughput(corpus.len(), base_def_dur), format_throughput(corpus.len(), flate2_decomp_dur), format!("{} B", base_def_size)
        );
        println!(
            "{:<18} | {:<16} | {:<18} | {:<18} | {:<12}",
            "", "TTZip (Level 0/Store)", format_throughput(corpus.len(), cur_l0_dur), "-", format!("{} B", cur_l0_size)
        );
        println!(
            "{:<18} | {:<16} | {:<18} | {:<18} | {:<12}",
            "", "TTZip (Fast Mode)", format_throughput(corpus.len(), cur_fast_dur), "-", format!("{} B", cur_fast_size)
        );
        println!(
            "{:<18} | {:<16} | {:<18} | {:<18} | {:<12}",
            "", "TTZip (Level 1)", format_throughput(corpus.len(), cur_l1_dur), "-", format!("{} B", cur_l1_size)
        );
        println!(
            "{:<18} | {:<16} | {:<18} | {:<18} | {:<12}",
            "", "TTZip (Level 6)", format_throughput(corpus.len(), cur_l6_dur), format_throughput(corpus.len(), decomp_dur), format!("{} B", cur_l6_size)
        );
        println!("-------------------+------------------+--------------------+--------------------+-------------");
    }
    println!();
}

fn run_ab_multicore_parallel_benchmarks() {
    println!("─── [6] Multi-Core Rayon Parallel Benchmark (100 Files x 1MB = 100MB Total) ─────");
    println!(
        "{:<26} | {:<18} | {:<18} | {:<12}",
        "Operation Mode", "Baseline (HEAD~1)", "Current (TTZip HW)", "Speedup"
    );
    println!("---------------------------+--------------------+--------------------+-------------");

    let num_files = 100;
    let file_size = 1024 * 1024;
    let sample = b"TTZip High-Performance Parallel Archiving Engine Record with JSON Payload Data\n";
    let mut file_data = Vec::with_capacity(file_size);
    while file_data.len() + sample.len() <= file_size {
        file_data.extend_from_slice(sample);
    }

    let files: Vec<Vec<u8>> = (0..num_files).map(|_| file_data.clone()).collect();
    let total_bytes = num_files * file_size;

    // 1. Parallel CRC-32 (100MB)
    let base_par_crc_dur = bench_min(|| {
        let hashes: Vec<u32> = files.par_iter().map(|f| baseline_crc32(f)).collect();
        black_box(hashes)
    });

    let cur_par_crc_dur = bench_min(|| {
        let hashes: Vec<u32> = files.par_iter().map(|f| current_crc32(0, f)).collect();
        black_box(hashes)
    });

    let par_crc_speedup = base_par_crc_dur.as_secs_f64() / cur_par_crc_dur.as_secs_f64();
    println!(
        "{:<26} | {:<18} | {:<18} | \x1b[32m{:.2}x\x1b[0m",
        "Parallel CRC-32 (100MB)",
        format_throughput(total_bytes, base_par_crc_dur),
        format_throughput(total_bytes, cur_par_crc_dur),
        par_crc_speedup
    );

    // 2. Parallel Compression (100MB)
    let base_par_comp_dur = bench_min(|| {
        let compressed: Vec<Vec<u8>> = files
            .par_iter()
            .map(|f| {
                let mut enc = Flate2Encoder::new(Vec::with_capacity(f.len() / 2), Flate2Compression::default());
                enc.write_all(f).unwrap();
                enc.finish().unwrap()
            })
            .collect();
        black_box(compressed)
    });

    let cur_par_comp_dur = bench_min(|| {
        let compressed: Vec<Vec<u8>> = files
            .par_iter()
            .map(|f| {
                with_thread_local_compressor(1, |comp| {
                    let bound = comp.compress_bound(f.len());
                    let mut buf = vec![0u8; bound];
                    let sz = comp.compress(f, &mut buf)?;
                    buf.truncate(sz);
                    Ok(buf)
                }).unwrap()
            })
            .collect();
        black_box(compressed)
    });

    let par_comp_speedup = base_par_comp_dur.as_secs_f64() / cur_par_comp_dur.as_secs_f64();
    println!(
        "{:<26} | {:<18} | {:<18} | \x1b[32m{:.2}x\x1b[0m",
        "Parallel Compress (100MB)",
        format_throughput(total_bytes, base_par_comp_dur),
        format_throughput(total_bytes, cur_par_comp_dur),
        par_comp_speedup
    );

    // 3. Parallel Decompression (100MB)
    let compressed_files: Vec<Vec<u8>> = files
        .iter()
        .map(|f| {
            with_thread_local_compressor(1, |comp| {
                let bound = comp.compress_bound(f.len());
                let mut buf = vec![0u8; bound];
                let sz = comp.compress(f, &mut buf)?;
                buf.truncate(sz);
                Ok(buf)
            }).unwrap()
        })
        .collect();

    let base_par_decomp_dur = bench_min(|| {
        let decompressed: Vec<Vec<u8>> = compressed_files
            .par_iter()
            .map(|cf| {
                let mut dec = Flate2Decoder::new(Vec::with_capacity(file_size));
                dec.write_all(cf).unwrap();
                dec.finish().unwrap()
            })
            .collect();
        black_box(decompressed)
    });

    let cur_par_decomp_dur = bench_min(|| {
        let decompressed: Vec<Vec<u8>> = compressed_files
            .par_iter()
            .map(|cf| {
                with_thread_local_decompressor(|dec| {
                    let mut buf = vec![0u8; file_size];
                    let sz = dec.decompress(cf, &mut buf)?;
                    buf.truncate(sz);
                    Ok(buf)
                }).unwrap()
            })
            .collect();
        black_box(decompressed)
    });

    let par_decomp_speedup = base_par_decomp_dur.as_secs_f64() / cur_par_decomp_dur.as_secs_f64();
    println!(
        "{:<26} | {:<18} | {:<18} | \x1b[32m{:.2}x\x1b[0m",
        "Parallel Decompress (100MB)",
        format_throughput(total_bytes, base_par_decomp_dur),
        format_throughput(total_bytes, cur_par_decomp_dur),
        par_decomp_speedup
    );
    println!();
}

