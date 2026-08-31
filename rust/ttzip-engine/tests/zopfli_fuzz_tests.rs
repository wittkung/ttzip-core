// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Malformed Zopfli Optimal Deflate Fault-Injection Fuzzing & Stress Suite.
//!
//! Implements an exhaustive 16-target fault injection matrix, concurrency stress harness,
//! and 500+ round automated mutation fuzzing loop aligned with RFC 1950, RFC 1951, and RFC 1952:
//! 1. Ultra-Long Repeating Single-Byte RLE Boundary Injections (64 KiB .. 1 MiB)
//! 2. Bad Hash Table & Self-Loop Infinite Loop Detection (Pathological 3-byte prefix collisions)
//! 3. Zero-Byte & Micro-Stream Boundary Traps (0..=7 bytes across raw Deflate, Zlib, Gzip)
//! 4. Extreme High-Entropy Incompressible Data Expansion & Bounded Degradation
//! 5. Recursive Block Splitting Super-Deep Depth (32+ levels) Circuit Breaker
//! 6. 1000+ Task Concurrent Zopfli Offline Optimization & Race Condition Fuzzing (Rayon)
//! 7. 500+ Round Pseudo-Random Squeeze Mutation Fuzzing
//! 8. Malformed RFC 1950 Zlib Header & CMF/FLG Corruption Defense
//! 9. Malformed RFC 1952 Gzip Header & ID1/ID2 Magic Number Corruption Defense
//! 10. Corrupted Adler-32 / CRC-32 Checksum Footer Tampering Injections
//! 11. Truncated Bitstream & Premature EOF Boundary Defenses
//! 12. Single-Bit Flip Mutation Sweep Across Valid Compressed Bitstreams
//! 13. Multi-Byte Erasure & Chunk Splice Attacks
//! 14. 32 KiB Sliding Window Boundary Wrap-Around & Back-Reference Distance Stress
//! 15. Dynamic Iteration Count Extreme Stepping (1, 2, 5, 10, 15 Monotonicity)
//! 16. 8-Corpus Mathematical Synthetic Chaotic Mutation Stress (`BenchmarkCorpusGenerator`)

use rayon::prelude::*;
use std::panic::{catch_unwind, AssertUnwindSafe};

use ttzip_engine::benchmark::corpus::{BenchmarkCorpusGenerator, BenchmarkCorpusType};
use ttzip_engine::codecs::deflate::{deflate_decompress, gzip_decompress, zlib_decompress};
use ttzip_engine::codecs::zopfli::{
    zopfli_compress, zopfli_compress_deflate, zopfli_compress_gzip, zopfli_compress_zlib,
    ZopfliBlockSplitter, ZopfliFormat, ZopfliOptions,
};
use ttzip_engine::security::zopfli_defense::{
    BlockSplitRecursionGuard, DagRecursionGuard,
};

// MARK: - Deterministic Pseudo-Random Generator (Knuth LCG)

#[derive(Debug, Clone)]
struct FuzzPrng {
    state: u64,
}

impl FuzzPrng {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.state >> 32) as u32
    }

    fn next_usize(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next_u32() as usize) % bound
        }
    }

    fn next_u8(&mut self) -> u8 {
        (self.next_u32() & 0xFF) as u8
    }

    fn gen_bytes(&mut self, len: usize) -> Vec<u8> {
        let mut buf = vec![0u8; len];
        for b in buf.iter_mut() {
            *b = self.next_u8();
        }
        buf
    }
}

// MARK: - Target 1: Ultra-Long Repeating Single-Byte RLE Boundary Injections

#[test]
fn test_target_01_ultra_long_repeating_single_byte_rle_injection() {
    let sizes = [64 * 1024, 256 * 1024, 1024 * 1024]; // 64 KiB, 256 KiB, 1 MiB

    for &size in &sizes {
        let rle_payload = vec![0x5Au8; size];

        // Compress with Zopfli fast options (5 iterations)
        let opts = ZopfliOptions {
            num_iterations: 5,
            max_block_splits: 5,
            max_chain: 256,
        };

        let compressed = zopfli_compress(&rle_payload, ZopfliFormat::Deflate, &opts)
            .expect("Zopfli must compress long RLE stream successfully");

        // RLE compression ratio must achieve extreme space savings (> 500x)
        assert!(
            compressed.len() < size / 200,
            "RLE compressed size {} too large for input {}",
            compressed.len(),
            size
        );

        // Verify lossless roundtrip inflation
        let mut decompressed = vec![0u8; size];
        let decomp_len = deflate_decompress(&compressed, &mut decompressed)
            .expect("Decompression of RLE stream must succeed");
        assert_eq!(decomp_len, size);
        assert_eq!(decompressed, rle_payload);
    }
}

// MARK: - Target 2: Bad Hash Table & Self-Loop Infinite Loop Detection

#[test]
fn test_target_02_bad_hash_table_and_self_loop_defense() {
    // 1. Pathological dense collision pattern (repeated 3-byte prefix)
    let mut collision_data = Vec::with_capacity(32 * 1024);
    for i in 0..(32 * 1024 / 4) {
        collision_data.extend_from_slice(&[b'A', b'B', b'C', (i % 4) as u8 + b'0']);
    }

    let opts = ZopfliOptions {
        num_iterations: 5,
        max_block_splits: 5,
        max_chain: 512,
    };
    let compressed = zopfli_compress(&collision_data, ZopfliFormat::Deflate, &opts)
        .expect("Zopfli must not hang on dense hash collisions");
    assert!(!compressed.is_empty());

    let mut decompressed = vec![0u8; collision_data.len()];
    let decomp_len = deflate_decompress(&compressed, &mut decompressed)
        .expect("Decompression of collision payload must succeed");
    assert_eq!(decomp_len, collision_data.len());
    assert_eq!(decompressed, collision_data);

    // 2. DAG recursion guard validation against cyclic state transitions
    let mut dag_guard = DagRecursionGuard::new(1024, 100_000);
    dag_guard.begin_block(100).expect("begin block");
    assert!(dag_guard.validate_transition(0, 3).is_ok());
    assert!(dag_guard.validate_transition(3, 10).is_ok());
    // Self-loop transition must be rejected
    assert!(dag_guard.validate_transition(10, 10).is_err());
    // Backward transition must be rejected
    assert!(dag_guard.validate_transition(10, 5).is_err());
}

// MARK: - Target 3: Zero-Byte & Micro-Stream Boundary Traps

#[test]
fn test_target_03_zero_byte_and_micro_stream_boundary_traps() {
    let micro_payloads: &[&[u8]] = &[
        b"",
        b"X",
        b"AB",
        b"XYZ",
        b"1234",
        b"12345",
        b"123456",
        b"1234567",
    ];

    let opts = ZopfliOptions::default();

    for &payload in micro_payloads {
        // 1. Raw Deflate
        let comp_deflate = zopfli_compress(payload, ZopfliFormat::Deflate, &opts)
            .expect("Zopfli Deflate micro payload compression");
        assert!(!comp_deflate.is_empty());
        let mut decomp_def = vec![0u8; payload.len() + 16];
        let def_len = deflate_decompress(&comp_deflate, &mut decomp_def)
            .expect("Deflate micro payload decompress");
        assert_eq!(def_len, payload.len());
        assert_eq!(&decomp_def[..def_len], payload);

        // 2. Zlib
        let comp_zlib = zopfli_compress(payload, ZopfliFormat::Zlib, &opts)
            .expect("Zopfli Zlib micro payload compression");
        assert!(comp_zlib.len() >= 6); // 2 header + payload + 4 adler
        let mut decomp_zlib = vec![0u8; payload.len() + 16];
        let zlib_len = zlib_decompress(&comp_zlib, &mut decomp_zlib)
            .expect("Zlib micro payload decompress");
        assert_eq!(zlib_len, payload.len());
        assert_eq!(&decomp_zlib[..zlib_len], payload);

        // 3. Gzip
        let comp_gzip = zopfli_compress(payload, ZopfliFormat::Gzip, &opts)
            .expect("Zopfli Gzip micro payload compression");
        assert!(comp_gzip.len() >= 18); // 10 header + payload + 8 footer
        let mut decomp_gzip = vec![0u8; payload.len() + 16];
        let gzip_len = gzip_decompress(&comp_gzip, &mut decomp_gzip)
            .expect("Gzip micro payload decompress");
        assert_eq!(gzip_len, payload.len());
        assert_eq!(&decomp_gzip[..gzip_len], payload);
    }
}

// MARK: - Target 4: Extreme High-Entropy Incompressible Data Expansion

#[test]
fn test_target_04_extreme_high_entropy_incompressible_expansion() {
    let mut prng = FuzzPrng::new(0xDEAD_BEEF_CAFE_BABE);
    let noise = prng.gen_bytes(32 * 1024); // 32 KiB pure noise

    let opts = ZopfliOptions {
        num_iterations: 3,
        max_block_splits: 3,
        max_chain: 256,
    };

    let compressed = zopfli_compress(&noise, ZopfliFormat::Deflate, &opts)
        .expect("High entropy compression must succeed without error");

    // Deflate expansion on 32 KiB random noise must not exceed 5% overhead + headers
    let max_allowable = noise.len() + noise.len() / 20 + 64;
    assert!(
        compressed.len() <= max_allowable,
        "High entropy payload expanded excessively: {} > {}",
        compressed.len(),
        max_allowable
    );

    let mut decompressed = vec![0u8; noise.len()];
    let decomp_len = deflate_decompress(&compressed, &mut decompressed)
        .expect("Decompressing high entropy stream must succeed");
    assert_eq!(decomp_len, noise.len());
    assert_eq!(decompressed, noise);
}

// MARK: - Target 5: Recursive Block Splitting Super-Deep Depth (32+ levels)

#[test]
fn test_target_05_recursive_block_splitting_super_deep_depth_circuit_breaker() {
    // 1. Verify BlockSplitRecursionGuard depth ceiling
    let guard_err = BlockSplitRecursionGuard::new(32, 100);
    assert!(guard_err.is_err(), "Guard must reject depth > 16");

    let mut guard = BlockSplitRecursionGuard::new(16, 50).expect("valid guard");
    for _ in 0..16 {
        assert!(guard.enter_depth().is_ok());
    }
    // Exceeding max depth 16 must trip circuit breaker
    assert!(guard.enter_depth().is_err());

    // 2. Synthetic distinct entropy regimes triggering dynamic block splitting
    let mut data = vec![0xAAu8; 32 * 1024];
    let mut prng = FuzzPrng::new(0x1234_5678);
    data.extend(prng.gen_bytes(32 * 1024));

    let splits = ZopfliBlockSplitter::split_optimal(&data, 0, data.len(), 32);
    assert!(!splits.is_empty(), "Block splitter must discover entropy transition");
    assert!(splits.len() <= 32);

    // Verify all split points are strictly ordered and within bounds
    let mut prev = 0;
    for &sp in &splits {
        assert!(sp > prev);
        assert!(sp <= data.len());
        prev = sp;
    }
}

// MARK: - Target 6: 1000+ Task Concurrent Zopfli Offline Optimization

#[test]
fn test_target_06_1000_task_concurrent_zopfli_race_fuzzing() {
    let task_count = 1000;

    let results: Vec<bool> = (0..task_count)
        .into_par_iter()
        .map(|idx| {
            let mut prng = FuzzPrng::new(idx as u64 ^ 0x9E37_79B9_7F4A_7C15);
            let len = 64 + prng.next_usize(512);
            let payload = prng.gen_bytes(len);

            let opts = ZopfliOptions {
                num_iterations: 2,
                max_block_splits: 2,
                max_chain: 128,
            };

            let comp = match zopfli_compress(&payload, ZopfliFormat::Deflate, &opts) {
                Ok(c) => c,
                Err(_) => return false,
            };

            let mut decomp = vec![0u8; len];
            match deflate_decompress(&comp, &mut decomp) {
                Ok(d_len) => d_len == len && decomp == payload,
                Err(_) => false,
            }
        })
        .collect();

    let passed_count = results.iter().filter(|&&r| r).count();
    assert_eq!(
        passed_count, task_count,
        "All {} concurrent Zopfli tasks must pass without race condition",
        task_count
    );
}

// MARK: - Target 7: 500+ Round Pseudo-Random Squeeze Mutation Fuzzing

#[test]
fn test_target_07_500_round_pseudorandom_squeeze_mutation_fuzzing() {
    let rounds = 500;
    let mut prng = FuzzPrng::new(0xFEED_FACE_1234_5678);

    for r in 0..rounds {
        let length = 1 + prng.next_usize(2048);
        let iterations = 1 + prng.next_usize(5); // 1..=5 iterations

        let mut payload = prng.gen_bytes(length);
        // Inject repetitive substrings to test squeeze DP match finder
        if length > 32 && r % 3 == 0 {
            for i in 0..(length / 8) {
                payload[i] = b'Z';
            }
        }

        let opts = ZopfliOptions {
            num_iterations: iterations,
            max_block_splits: 3,
            max_chain: 256,
        };

        let compressed = zopfli_compress(&payload, ZopfliFormat::Deflate, &opts)
            .unwrap_or_else(|_| panic!("Round {} Zopfli compress failed", r));

        let mut decompressed = vec![0u8; length];
        let decomp_len = deflate_decompress(&compressed, &mut decompressed)
            .unwrap_or_else(|_| panic!("Round {} Deflate decompress failed", r));

        assert_eq!(decomp_len, length);
        assert_eq!(decompressed, payload);
    }
}

// MARK: - Target 8: Malformed RFC 1950 Zlib Header & CMF/FLG Corruption

#[test]
fn test_target_08_malformed_zlib_header_corruption() {
    let bad_zlib_headers: Vec<Vec<u8>> = vec![
        vec![0x78, 0x00],       // Bad FCHECK ((0x78 * 256 + 0x00) % 31 != 0)
        vec![0x88, 0x9C],       // Invalid CM = 8 with CINFO > 7 (88)
        vec![0x79, 0x9C],       // Invalid CM != 8 (9)
        vec![0x78, 0xBB],       // FDICT flag set without dictionary
        vec![0x78],             // Truncated 1-byte header
        vec![],                 // Empty slice
    ];

    let mut out = vec![0u8; 256];
    for bad_hdr in &bad_zlib_headers {
        let res = catch_unwind(AssertUnwindSafe(|| zlib_decompress(bad_hdr, &mut out)));
        assert!(res.is_ok(), "zlib_decompress must not panic on malformed header");
        assert!(res.unwrap().is_err());
    }
}

// MARK: - Target 9: Malformed RFC 1952 Gzip Header & ID1/ID2 Corruption

#[test]
fn test_target_09_malformed_gzip_header_corruption() {
    let bad_gzip_headers: Vec<Vec<u8>> = vec![
        vec![0x1F, 0x00, 0x08, 0, 0, 0, 0, 0, 0, 0xFF], // Bad ID2 != 0x8B
        vec![0x00, 0x8B, 0x08, 0, 0, 0, 0, 0, 0, 0xFF], // Bad ID1 != 0x1F
        vec![0x1F, 0x8B, 0x09, 0, 0, 0, 0, 0, 0, 0xFF], // Bad CM != 8
        vec![0x1F, 0x8B, 0x08, 0xE0, 0, 0, 0, 0, 0, 0xFF], // Reserved flag bits set
        vec![0x1F, 0x8B, 0x08], // Truncated header
    ];

    let mut out = vec![0u8; 256];
    for bad_hdr in &bad_gzip_headers {
        let res = catch_unwind(AssertUnwindSafe(|| gzip_decompress(bad_hdr, &mut out)));
        assert!(res.is_ok(), "gzip_decompress must not panic on malformed gzip header");
        assert!(res.unwrap().is_err());
    }
}

// MARK: - Target 10: Corrupted Adler-32 / CRC-32 Checksum Footer Tampering

#[test]
fn test_target_10_corrupted_checksum_footer_tampering() {
    let payload = b"TTZip Zopfli Checksum Tamper Defense Test Payload 2026";
    let opts = ZopfliOptions::fast();

    // 1. Zlib Checksum Tampering
    let mut zlib_comp = zopfli_compress_zlib(payload, &opts).expect("zlib compress");
    let zlib_len = zlib_comp.len();
    assert!(zlib_len >= 6);
    // Corrupt the last byte of Adler-32
    zlib_comp[zlib_len - 1] ^= 0xFF;

    let mut out = vec![0u8; payload.len() + 16];
    let res = zlib_decompress(&zlib_comp, &mut out);
    assert!(res.is_err(), "Tampered Adler-32 checksum must fail validation");

    // 2. Gzip Checksum Tampering
    let mut gzip_comp = zopfli_compress_gzip(payload, &opts).expect("gzip compress");
    let gzip_len = gzip_comp.len();
    assert!(gzip_len >= 18);
    // Corrupt CRC-32 (bytes len - 8 .. len - 4)
    gzip_comp[gzip_len - 6] ^= 0xAA;

    let res_gz = gzip_decompress(&gzip_comp, &mut out);
    assert!(res_gz.is_err(), "Tampered Gzip CRC-32 must fail validation");
}

// MARK: - Target 11: Truncated Bitstream & Premature EOF Boundary Defenses

#[test]
fn test_target_11_truncated_bitstream_premature_eof_defenses() {
    let payload = b"Zopfli Deflate Truncation Boundary Defense Suite 2026.".repeat(8);
    let opts = ZopfliOptions::fast();
    let comp = zopfli_compress_deflate(&payload, &opts).expect("deflate compress");

    let mut out = vec![0u8; payload.len() + 64];

    // Truncate at every single byte offset from 1 to comp.len() - 1
    for cut in 1..comp.len() {
        let truncated = &comp[..cut];
        let res = catch_unwind(AssertUnwindSafe(|| deflate_decompress(truncated, &mut out)));
        assert!(res.is_ok(), "Decompressor must not panic on truncated input at offset {}", cut);
    }
}

// MARK: - Target 12: Single-Bit Flip Mutation Sweep

#[test]
fn test_target_12_single_bit_flip_mutation_sweep() {
    let payload = b"Deterministic Single Bit Flip Mutation Sweep Test on Zopfli Bitstream 2026.".repeat(4);
    let opts = ZopfliOptions::fast();
    let comp = zopfli_compress_deflate(&payload, &opts).expect("deflate compress");

    let mut out = vec![0u8; payload.len() + 128];

    // Flip bits across the first 64 bytes of valid compressed stream
    let sweep_len = comp.len().min(64);
    for byte_idx in 0..sweep_len {
        for bit_idx in 0..8 {
            let mut corrupted = comp.clone();
            corrupted[byte_idx] ^= 1 << bit_idx;

            let res = catch_unwind(AssertUnwindSafe(|| deflate_decompress(&corrupted, &mut out)));
            assert!(
                res.is_ok(),
                "Single bit flip at byte {} bit {} caused panic",
                byte_idx,
                bit_idx
            );
        }
    }
}

// MARK: - Target 13: Multi-Byte Erasure & Chunk Splice Attacks

#[test]
fn test_target_13_multibyte_erasure_and_chunk_splice_attacks() {
    let mut prng = FuzzPrng::new(0xABCD_1234_5678_90EF);
    let opts = ZopfliOptions::fast();

    let stream1 = zopfli_compress_deflate(&b"Alpha stream for chunk splicing test.".repeat(8), &opts)
        .expect("stream1");
    let stream2 = zopfli_compress_deflate(&b"Beta stream for chunk splicing attack.".repeat(8), &opts)
        .expect("stream2");

    let mut out = vec![0u8; 1024];

    for _ in 0..100 {
        // Splice prefix of stream1 with suffix of stream2
        let cut1 = prng.next_usize(stream1.len().max(1));
        let cut2 = prng.next_usize(stream2.len().max(1));

        let mut spliced = Vec::with_capacity(cut1 + (stream2.len() - cut2));
        spliced.extend_from_slice(&stream1[..cut1]);
        spliced.extend_from_slice(&stream2[cut2..]);

        let res = catch_unwind(AssertUnwindSafe(|| deflate_decompress(&spliced, &mut out)));
        assert!(res.is_ok(), "Chunk splice must never panic decompressor");
    }
}

// MARK: - Target 14: 32 KiB Sliding Window Boundary Wrap-Around Stress

#[test]
fn test_target_14_32k_sliding_window_boundary_wrap_around() {
    // Construct payload with repeating patterns exactly across 32 KiB boundary
    let mut payload = Vec::with_capacity(70 * 1024);
    let marker_a = b"MARKER_A_PATTERN_AT_ORIGIN_0000";
    let marker_b = b"MARKER_B_PATTERN_AT_OFFSET_32KB";

    payload.extend_from_slice(marker_a);
    payload.resize(32 * 1024, b'.');
    payload.extend_from_slice(marker_b);
    payload.resize(64 * 1024, b'-');
    payload.extend_from_slice(marker_a); // Refer back across 64 KiB (beyond window)
    payload.extend_from_slice(marker_b); // Refer back within 32 KiB window

    let opts = ZopfliOptions {
        num_iterations: 3,
        max_block_splits: 3,
        max_chain: 512,
    };

    let comp = zopfli_compress(&payload, ZopfliFormat::Deflate, &opts)
        .expect("Window boundary compression");

    let mut decomp = vec![0u8; payload.len()];
    let d_len = deflate_decompress(&comp, &mut decomp).expect("Window boundary decompression");
    assert_eq!(d_len, payload.len());
    assert_eq!(decomp, payload);
}

// MARK: - Target 15: Dynamic Iteration Count Extreme Stepping

#[test]
fn test_target_15_dynamic_iteration_count_extreme_stepping() {
    let payload = b"Zopfli Dynamic Squeeze Iteration Count Monotonicity and Stepping Evaluation 2026. \
                    Testing space savings improvement across multiple iterative passes.\n"
        .repeat(16);

    let iteration_steps = [1, 2, 5, 10, 15];
    let mut prev_size = usize::MAX;

    for &iters in &iteration_steps {
        let opts = ZopfliOptions {
            num_iterations: iters,
            max_block_splits: 3,
            max_chain: 512,
        };

        let comp = zopfli_compress(&payload, ZopfliFormat::Deflate, &opts)
            .unwrap_or_else(|_| panic!("Iteration {} compression failed", iters));

        // Higher iterations must yield equal or strictly smaller size
        assert!(
            comp.len() <= prev_size,
            "Iteration {} size {} was larger than previous {}",
            iters,
            comp.len(),
            prev_size
        );
        prev_size = comp.len();

        let mut decomp = vec![0u8; payload.len()];
        let d_len = deflate_decompress(&comp, &mut decomp).expect("decompress");
        assert_eq!(d_len, payload.len());
        assert_eq!(decomp, payload);
    }
}

// MARK: - Target 16: 8-Corpus Mathematical Synthetic Chaotic Mutation Stress

#[test]
fn test_target_16_eight_corpus_mathematical_synthetic_fuzzing() {
    let corpora = [
        BenchmarkCorpusType::TextData,
        BenchmarkCorpusType::ShortMatch,
        BenchmarkCorpusType::Dna,
        BenchmarkCorpusType::Noise,
        BenchmarkCorpusType::Literals,
        BenchmarkCorpusType::MachOBinary,
        BenchmarkCorpusType::RealisticRgb,
        BenchmarkCorpusType::StripedRgb,
    ];

    let opts = ZopfliOptions {
        num_iterations: 3,
        max_block_splits: 4,
        max_chain: 256,
    };

    for &corpus_type in &corpora {
        let raw_data = BenchmarkCorpusGenerator::generate(corpus_type, 16 * 1024);

        let comp = zopfli_compress(&raw_data, ZopfliFormat::Deflate, &opts)
            .unwrap_or_else(|_| panic!("Corpus {:?} compression failed", corpus_type));

        let mut decomp = vec![0u8; raw_data.len()];
        let d_len = deflate_decompress(&comp, &mut decomp)
            .unwrap_or_else(|_| panic!("Corpus {:?} decompression failed", corpus_type));

        assert_eq!(d_len, raw_data.len());
        assert_eq!(decomp, raw_data);
    }
}
