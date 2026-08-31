// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive zlib-ng & Modern Deflate 16-Dimensional Fault-Injection Fuzzing Suite.
//!
//! Implements an exhaustive 16-target fault injection matrix and 8-corpus mutation fuzzing harness:
//! 1. Bad Hash Chain & Self-Loop Infinite Loop Injection (degenerate hash collision patterns)
//! 2. Sliding Window Pointer Out-of-Bounds & Negative Offset Injections ($D > dst\_pos$ / $D > 32KB$)
//! 3. Dynamic Level High-Frequency Mutation & Random Strategy Switching (0..=12 across 200+ chunks)
//! 4. Extreme High-Entropy Incompressible Data Expansion & Bound Defense (XorShift128+ noise)
//! 5. Zero-Byte & Single-Byte / Micro-Stream Boundary Traps (0..=7 bytes across raw, zlib, gzip)
//! 6. 1000+ Task Concurrent Dual-Engine Arbitration & Race Condition Fuzzing (Rayon multithreaded)
//! 7. 500+ Round 8-Corpus Mathematical Synthetic Mutation Fuzzing (`BenchmarkCorpusGenerator`)
//! 8. Corrupted Adler-32 & CRC-32 Checksum Injection (RFC 1950 / RFC 1952 footer tampering)
//! 9. Truncated Deflate / Zlib / Gzip Stream Premature EOF Injections (all byte offsets)
//! 10. Corrupted Block Type 11 (BTYPE = 3 reserved) Injection
//! 11. Corrupted Uncompressed Block Header Injections (NLEN != !LEN overruns)
//! 12. Incomplete / Corrupted Dynamic Huffman Codespace Injections (Kraft inequality violations)
//! 13. Out-of-Bounds Match Length & Excessive Distance Code Injections
//! 14. Single-Bit Flip Mutation Sweep Across Valid Compressed Bitstreams
//! 15. Random Multi-Byte Erasure & Disparate Stream Chunk Splice Attacks
//! 16. Oversized Malformed Header & Reserved Flag Bit Injections

use rayon::prelude::*;
use std::io::{Cursor, Read, Write};
use std::panic::catch_unwind;

use ttzip_engine::benchmark::corpus::{BenchmarkCorpusGenerator, BenchmarkCorpusType};
use ttzip_engine::codecs::deflate::{
    deflate_compress, deflate_compress_bound, deflate_decompress, gzip_compress,
    gzip_compress_bound, gzip_decompress, zlib_compress, zlib_compress_bound, zlib_decompress,
    DeflateCompressor, DeflateStrategy,
};

// MARK: - Deterministic Pseudo-Random Generator

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
}

// MARK: - Target 1: Bad Hash Chain & Self-Loop Infinite Loop Injection

#[test]
fn test_target_01_bad_hash_chain_and_self_loop_defense() {
    // 1. Pathological repetitive sequences causing hash chain collisions
    let mut collision_payload = Vec::with_capacity(64 * 1024);
    for i in 0..(64 * 1024 / 4) {
        // Cyclic pattern with 4-byte period creating identical 3-byte prefixes
        collision_payload.extend_from_slice(&[b'A', b'C', b'G', (i % 4) as u8 + b'0']);
    }

    let bound = deflate_compress_bound(collision_payload.len(), 6);
    let mut compressed = vec![0u8; bound];
    let comp_len = deflate_compress(&collision_payload, &mut compressed, 6)
        .expect("Deflate compression must not deadlock on dense collisions");
    assert!(comp_len > 0);

    let mut decompressed = vec![0u8; collision_payload.len()];
    let decomp_len = deflate_decompress(&compressed[..comp_len], &mut decompressed)
        .expect("Decompression of dense collision payload must succeed");
    assert_eq!(decomp_len, collision_payload.len());
    assert_eq!(decompressed, collision_payload);

    // 2. Crafted stream attempting self-loop decompression: Distance = 0 or cyclic match
    let malformed_self_loop = vec![0x78, 0x9c, 0x63, 0x60, 0x00, 0x02, 0x00, 0x00, 0x05, 0x00, 0x01];
    let mut out = vec![0u8; 1024];
    let _ = zlib_decompress(&malformed_self_loop, &mut out);
}

// MARK: - Target 2: Sliding Window Pointer Out-of-Bounds & Negative Offset

#[test]
fn test_target_02_sliding_window_out_of_bounds_and_negative_offset() {
    let payload = b"TTZip Safe Sliding Window Invariant Testing 2026";
    let bound = zlib_compress_bound(payload.len(), 6);
    let mut comp = vec![0u8; bound];
    let comp_len = zlib_compress(payload, &mut comp, 6).expect("compress");

    // Mutate internal bitstream to inject out-of-bounds backward distance (D > output position)
    for pos in 2..(comp_len.saturating_sub(4)) {
        let mut corrupted = comp[..comp_len].to_vec();
        corrupted[pos] ^= 0xFF;

        let mut out = vec![0u8; payload.len() * 2];
        let result = catch_unwind(std::panic::AssertUnwindSafe(|| {
            zlib_decompress(&corrupted, &mut out)
        }));

        assert!(
            result.is_ok(),
            "Decompressor must not panic on corrupted backward distances"
        );
    }
}

// MARK: - Target 3: Dynamic Level High-Frequency Mutation & Strategy Switching

#[test]
fn test_target_03_dynamic_level_high_frequency_mutation() {
    let mut prng = FuzzPrng::new(0xDEAD_BEEF_CAFE_0001);
    let levels = [0, 1, 3, 6, 9, 12];
    let strategies = [
        DeflateStrategy::Store,
        DeflateStrategy::Fast,
        DeflateStrategy::Level(1),
        DeflateStrategy::Level(6),
        DeflateStrategy::Level(12),
    ];

    for round in 0..200 {
        let chunk_size = 16 + prng.next_usize(4096);
        let raw = prng.state.to_le_bytes().repeat(chunk_size / 8 + 1);
        let src = &raw[..chunk_size];

        let chosen_lvl = levels[prng.next_usize(levels.len())];
        let chosen_strat = strategies[prng.next_usize(strategies.len())];

        // 1. Thread-local helper
        let bound = deflate_compress_bound(src.len(), chosen_lvl);
        let mut dst = vec![0u8; bound + 64];
        let comp_len = deflate_compress(src, &mut dst, chosen_lvl)
            .unwrap_or_else(|_| panic!("Failed compress round {round} lvl {chosen_lvl}"));

        let mut recovered = vec![0u8; src.len()];
        let decomp_len = deflate_decompress(&dst[..comp_len], &mut recovered)
            .unwrap_or_else(|_| panic!("Failed decompress round {round} lvl {chosen_lvl}"));
        assert_eq!(decomp_len, src.len());
        assert_eq!(&recovered[..], src);

        // 2. Explicit compressor strategy instance
        let mut comp_instance = DeflateCompressor::with_strategy(chosen_strat).expect("alloc");
        let inst_bound = comp_instance.compress_bound(src.len());
        let mut inst_dst = vec![0u8; inst_bound + 64];
        let inst_written = comp_instance
            .compress(src, &mut inst_dst)
            .expect("instance compress");

        let mut inst_recovered = vec![0u8; src.len()];
        let inst_decomp_len =
            deflate_decompress(&inst_dst[..inst_written], &mut inst_recovered)
                .expect("instance decompress");
        assert_eq!(inst_decomp_len, src.len());
        assert_eq!(&inst_recovered[..], src);
    }
}

// MARK: - Target 4: Extreme High-Entropy Incompressible Data Expansion & Bound Defense

#[test]
fn test_target_04_extreme_high_entropy_incompressible_defense() {
    let sizes = [1, 7, 15, 64, 255, 1024, 8192, 32768, 65536];

    for &sz in &sizes {
        let noise = BenchmarkCorpusGenerator::generate(BenchmarkCorpusType::Noise, sz);
        for &lvl in &[0, 1, 6, 12] {
            // Raw Deflate
            let bound_def = deflate_compress_bound(noise.len(), lvl);
            let mut out_def = vec![0u8; bound_def];
            let comp_def = deflate_compress(&noise, &mut out_def, lvl)
                .unwrap_or_else(|_| panic!("Failed raw noise compress sz {sz} lvl {lvl}"));
            assert!(
                comp_def <= bound_def,
                "Compressed output {comp_def} exceeded bound {bound_def}"
            );

            let mut dec_def = vec![0u8; noise.len()];
            let d_len = deflate_decompress(&out_def[..comp_def], &mut dec_def).expect("dec noise");
            assert_eq!(d_len, noise.len());
            assert_eq!(dec_def, noise);

            // Zlib
            let bound_zlib = zlib_compress_bound(noise.len(), lvl);
            let mut out_zlib = vec![0u8; bound_zlib];
            let comp_zlib = zlib_compress(&noise, &mut out_zlib, lvl).expect("zlib noise");
            assert!(comp_zlib <= bound_zlib);

            let mut dec_zlib = vec![0u8; noise.len()];
            let z_len = zlib_decompress(&out_zlib[..comp_zlib], &mut dec_zlib).expect("dec zlib");
            assert_eq!(z_len, noise.len());
            assert_eq!(dec_zlib, noise);
        }
    }
}

// MARK: - Target 5: Zero-Byte & Single-Byte Micro-Stream Boundaries

#[test]
fn test_target_05_zero_and_single_byte_micro_stream_boundary() {
    let micro_payloads: &[&[u8]] = &[
        b"",
        b"A",
        b"AB",
        b"XYZ",
        b"1234",
        b"1234567",
        b"0123456789ABCDEF",
    ];

    for &payload in micro_payloads {
        for &lvl in &[0, 1, 6, 9, 12] {
            // Raw Deflate
            let b_def = deflate_compress_bound(payload.len(), lvl);
            let mut out_def = vec![0u8; b_def];
            let w_def = deflate_compress(payload, &mut out_def, lvl).expect("micro def comp");
            let mut dec_def = vec![0u8; payload.len()];
            let r_def = deflate_decompress(&out_def[..w_def], &mut dec_def).expect("micro def dec");
            assert_eq!(r_def, payload.len());
            assert_eq!(&dec_def[..], payload);

            // Zlib
            let b_zlib = zlib_compress_bound(payload.len(), lvl);
            let mut out_zlib = vec![0u8; b_zlib];
            let w_zlib = zlib_compress(payload, &mut out_zlib, lvl).expect("micro zlib comp");
            let mut dec_zlib = vec![0u8; payload.len()];
            let r_zlib = zlib_decompress(&out_zlib[..w_zlib], &mut dec_zlib).expect("micro zlib dec");
            assert_eq!(r_zlib, payload.len());
            assert_eq!(&dec_zlib[..], payload);

            // Gzip
            let b_gz = gzip_compress_bound(payload.len(), lvl);
            let mut out_gz = vec![0u8; b_gz];
            let w_gz = gzip_compress(payload, &mut out_gz, lvl).expect("micro gz comp");
            let mut dec_gz = vec![0u8; payload.len()];
            let r_gz = gzip_decompress(&out_gz[..w_gz], &mut dec_gz).expect("micro gz dec");
            assert_eq!(r_gz, payload.len());
            assert_eq!(&dec_gz[..], payload);
        }
    }
}

// MARK: - Target 6: 1000+ Task Concurrent Dual-Engine Arbitration

#[test]
fn test_target_06_1000_task_concurrent_dual_engine_arbitration() {
    let task_count = 1000;

    (0..task_count).into_par_iter().for_each(|task_id| {
        let mut prng = FuzzPrng::new(0xA0B0_C0D0_0000_0000 + task_id as u64);
        let len = 32 + prng.next_usize(2048);
        let mut raw = vec![0u8; len];
        for b in &mut raw {
            *b = prng.next_u8();
        }

        let level = (task_id % 13) as i32;

        // 1. TTZip Compress -> flate2 Decompress (Zlib)
        let bound = zlib_compress_bound(raw.len(), level);
        let mut ttzip_comp = vec![0u8; bound];
        let written = zlib_compress(&raw, &mut ttzip_comp, level).expect("ttzip compress");

        let mut flate2_dec = flate2::read::ZlibDecoder::new(Cursor::new(&ttzip_comp[..written]));
        let mut flate2_out = Vec::with_capacity(raw.len());
        flate2_dec
            .read_to_end(&mut flate2_out)
            .expect("flate2 cross-decompress of ttzip stream");
        assert_eq!(flate2_out, raw, "Cross-decompression parity failure at task {task_id}");

        // 2. flate2 Compress -> TTZip Decompress (Zlib)
        let mut flate2_enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(6));
        flate2_enc.write_all(&raw).expect("flate2 write");
        let flate2_comp = flate2_enc.finish().expect("flate2 finish");

        let mut ttzip_out = vec![0u8; raw.len()];
        let ttzip_decomp_len =
            zlib_decompress(&flate2_comp, &mut ttzip_out).expect("ttzip decode of flate2 stream");
        assert_eq!(ttzip_decomp_len, raw.len());
        assert_eq!(ttzip_out, raw);
    });
}

// MARK: - Target 7: 500+ Round 8-Corpus Mathematical Synthetic Mutation Fuzzing

#[test]
fn test_target_07_500_round_8_corpus_mutation_fuzzing() {
    let corpus_types = [
        BenchmarkCorpusType::TextData,
        BenchmarkCorpusType::ShortMatch,
        BenchmarkCorpusType::Dna,
        BenchmarkCorpusType::Noise,
        BenchmarkCorpusType::Literals,
        BenchmarkCorpusType::MachOBinary,
        BenchmarkCorpusType::RealisticRgb,
        BenchmarkCorpusType::StripedRgb,
    ];

    let mut prng = FuzzPrng::new(0x1928_3746_55AA_BBCC);

    for (c_idx, &c_type) in corpus_types.iter().enumerate() {
        let corpus_data = BenchmarkCorpusGenerator::generate(c_type, 8192);
        let bound = zlib_compress_bound(corpus_data.len(), 6);
        let mut compressed = vec![0u8; bound];
        let comp_len = zlib_compress(&corpus_data, &mut compressed, 6).expect("compress corpus");
        let compressed_slice = &compressed[..comp_len];

        // 65 iterations per corpus = 520 total mutation iterations
        for iter in 0..65 {
            let mut mutated = compressed_slice.to_vec();
            let mutation_kind = prng.next_usize(4);

            match mutation_kind {
                0 => {
                    // Random bit flip
                    let byte_pos = prng.next_usize(mutated.len());
                    let bit_pos = prng.next_usize(8);
                    mutated[byte_pos] ^= 1 << bit_pos;
                }
                1 => {
                    // Random byte overwrite
                    let byte_pos = prng.next_usize(mutated.len());
                    mutated[byte_pos] = prng.next_u8();
                }
                2 => {
                    // Random byte erasure / truncation
                    let cut = 2 + prng.next_usize(mutated.len().saturating_sub(4));
                    mutated.truncate(cut);
                }
                _ => {
                    // Random byte insertion
                    let ins_pos = prng.next_usize(mutated.len());
                    mutated.insert(ins_pos, prng.next_u8());
                }
            }

            let mut out = vec![0u8; corpus_data.len() * 2];
            let result = catch_unwind(std::panic::AssertUnwindSafe(|| {
                zlib_decompress(&mutated, &mut out)
            }));

            assert!(
                result.is_ok(),
                "Panic during 8-corpus mutation fuzzing on corpus {c_idx} iter {iter}"
            );
        }
    }
}

// MARK: - Target 8: Corrupted Adler-32 & CRC-32 Checksum Injection

#[test]
fn test_target_08_corrupted_adler32_and_crc32_checksum_injection() {
    let payload = b"TTZip Checksum Integrity Defense & Verification 2026";

    // 1. Zlib Adler-32 Footer Tampering (Last 4 bytes)
    let bound_z = zlib_compress_bound(payload.len(), 6);
    let mut comp_z = vec![0u8; bound_z];
    let len_z = zlib_compress(payload, &mut comp_z, 6).expect("zlib comp");

    let mut bad_adler = comp_z[..len_z].to_vec();
    let adler_offset = len_z - 4;
    bad_adler[adler_offset] ^= 0x55;
    bad_adler[adler_offset + 3] ^= 0xAA;

    let mut out_z = vec![0u8; payload.len()];
    let res_z = zlib_decompress(&bad_adler, &mut out_z);
    assert!(
        res_z.is_err(),
        "Zlib decompressor must reject corrupted Adler-32 checksum"
    );

    // 2. Gzip CRC-32 Footer Tampering (Last 8 bytes: CRC32 + ISIZE)
    let bound_g = gzip_compress_bound(payload.len(), 6);
    let mut comp_g = vec![0u8; bound_g];
    let len_g = gzip_compress(payload, &mut comp_g, 6).expect("gz comp");

    let mut bad_crc = comp_g[..len_g].to_vec();
    let crc_offset = len_g - 8;
    bad_crc[crc_offset] ^= 0xFF;

    let mut out_g = vec![0u8; payload.len()];
    let res_g = gzip_decompress(&bad_crc, &mut out_g);
    assert!(
        res_g.is_err(),
        "Gzip decompressor must reject corrupted CRC-32 checksum"
    );
}

// MARK: - Target 9: Truncated Deflate / Zlib / Gzip Stream Premature EOF

#[test]
fn test_target_09_truncated_stream_premature_eof_injection() {
    let payload = b"Premature EOF Bitstream Underflow Defense Harness - TTZip High Speed Engine";
    let bound = zlib_compress_bound(payload.len(), 6);
    let mut comp = vec![0u8; bound];
    let comp_len = zlib_compress(payload, &mut comp, 6).expect("compress");

    // Truncate at every possible length from 0 to comp_len - 1
    for cut in 0..comp_len {
        let truncated = &comp[..cut];
        let mut out = vec![0u8; payload.len()];
        let res = zlib_decompress(truncated, &mut out);
        assert!(
            res.is_err(),
            "Decompressing truncated stream (len {cut}/{comp_len}) must safely fail"
        );
    }
}

// MARK: - Target 10: Corrupted Block Type 11 (BTYPE = 3 reserved) Injection

#[test]
fn test_target_10_illegal_block_type_btype3_injection() {
    // BTYPE = 11 (binary 3) is reserved and invalid in RFC 1951
    // Byte: BFINAL=1 (bit 0), BTYPE=11 (bits 1-2) -> 0b0000_0111 = 0x07
    let illegal_raw_deflate = [0x07u8, 0x00, 0x00, 0x00];
    let mut out = vec![0u8; 128];
    let res = deflate_decompress(&illegal_raw_deflate, &mut out);
    assert!(
        res.is_err(),
        "Deflate decompressor must reject reserved BTYPE = 3 (11b)"
    );
}

// MARK: - Target 11: Corrupted Uncompressed Block Header Injections (NLEN != !LEN)

#[test]
fn test_target_11_corrupted_uncompressed_nlen_header_injection() {
    // BTYPE = 00 (Uncompressed block), LEN = 4 (0x0004), NLEN should be 0xFFFB
    // Corrupted NLEN = 0x1234
    let corrupted_stored_block = [
        0x01, // BFINAL=1, BTYPE=00
        0x04, 0x00, // LEN = 4
        0x34, 0x12, // NLEN = 0x1234 (Mismatch! Expected 0xFB, 0xFF)
        0xDE, 0xAD, 0xBE, 0xEF,
    ];

    let mut out = vec![0u8; 64];
    let res = deflate_decompress(&corrupted_stored_block, &mut out);
    assert!(
        res.is_err(),
        "Deflate decompressor must reject uncompressed block with NLEN != ~LEN"
    );
}

// MARK: - Target 12: Incomplete / Corrupted Dynamic Huffman Codespace (Kraft Violation)

#[test]
fn test_target_12_incomplete_dynamic_huffman_codespace_violation() {
    // Dynamic Huffman block header (BTYPE=10) with corrupted HCLEN/HLIT counts
    let malformed_dynamic_header = [
        0x78, 0x9C, // Zlib CMF/FLG
        0x05, 0xC0, 0x81, 0x08, 0x00, 0x00, 0x00, 0x00, 0x20, 0x7F, 0xEB, 0x0F,
    ];

    let mut out = vec![0u8; 256];
    let res = zlib_decompress(&malformed_dynamic_header, &mut out);
    assert!(
        res.is_err(),
        "Zlib decompressor must reject corrupted dynamic Huffman codespace"
    );
}

// MARK: - Target 13: Out-of-Bounds Match Length & Excessive Distance Codes

#[test]
fn test_target_13_out_of_range_match_length_and_distance_injection() {
    // Malformed bitstream emitting invalid symbol (> 285) or distance (> 29)
    let crafted_invalid_symbol = [
        0x78, 0x9C, 0xED, 0xC1, 0x01, 0x01, 0x00, 0x00, 0x00, 0x80, 0xA0, 0xFA, 0xEF, 0xFF,
    ];

    let mut out = vec![0u8; 256];
    let res = zlib_decompress(&crafted_invalid_symbol, &mut out);
    assert!(
        res.is_err(),
        "Decompressor must reject invalid Huffman symbol / distance ranges"
    );
}

// MARK: - Target 14: Single-Bit Flip Mutation Sweep Across Valid Stream

#[test]
fn test_target_14_single_bit_flip_mutation_sweep() {
    let payload = b"TTZip Single Bit Flip Fuzzing Sweep Matrix 2026";
    let bound = deflate_compress_bound(payload.len(), 6);
    let mut comp = vec![0u8; bound];
    let comp_len = deflate_compress(payload, &mut comp, 6).expect("compress");

    // Flip every single bit in the compressed stream
    for byte_idx in 0..comp_len {
        for bit in 0..8 {
            let mut mutated = comp[..comp_len].to_vec();
            mutated[byte_idx] ^= 1 << bit;

            let mut out = vec![0u8; payload.len() * 2];
            let result = catch_unwind(std::panic::AssertUnwindSafe(|| {
                deflate_decompress(&mutated, &mut out)
            }));

            assert!(
                result.is_ok(),
                "Bit flip at byte {byte_idx} bit {bit} must not panic"
            );
        }
    }
}

// MARK: - Target 15: Random Multi-Byte Erasure & Disparate Stream Chunk Splice Attacks

#[test]
fn test_target_15_random_byte_erasure_and_chunk_splice_fuzzing() {
    let payload1 = b"Stream A: The quick brown fox jumps over the lazy dog repeatedly.";
    let payload2 = b"Stream B: High-performance zlib-ng modern microkernel Rust architecture.";

    let mut comp1 = vec![0u8; zlib_compress_bound(payload1.len(), 6)];
    let len1 = zlib_compress(payload1, &mut comp1, 6).expect("comp1");

    let mut comp2 = vec![0u8; zlib_compress_bound(payload2.len(), 6)];
    let len2 = zlib_compress(payload2, &mut comp2, 6).expect("comp2");

    let mut prng = FuzzPrng::new(0x55AA_1234_9876_FEDC);

    for trial in 0..100 {
        let split1 = prng.next_usize(len1);
        let split2 = prng.next_usize(len2);

        // Splice stream 1 prefix with stream 2 suffix
        let mut spliced = Vec::with_capacity(split1 + (len2 - split2));
        spliced.extend_from_slice(&comp1[..split1]);
        spliced.extend_from_slice(&comp2[split2..len2]);

        let mut out = vec![0u8; 1024];
        let result = catch_unwind(std::panic::AssertUnwindSafe(|| {
            zlib_decompress(&spliced, &mut out)
        }));

        assert!(
            result.is_ok(),
            "Stream splice attack must not panic on trial {trial}"
        );
    }
}

// MARK: - Target 16: Oversized Malformed Header & Reserved Flag Bit Injections

#[test]
fn test_target_16_oversized_header_and_reserved_flags_corruption() {
    // 1. Zlib invalid CMF (CM != 8)
    let bad_cmf_zlib = [0x18, 0x9C, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01];
    let mut out = vec![0u8; 128];
    assert!(
        zlib_decompress(&bad_cmf_zlib, &mut out).is_err(),
        "Zlib must reject CM != 8"
    );

    // 2. Zlib invalid FLG check bits ((CMF * 256 + FLG) % 31 != 0)
    let bad_flg_zlib = [0x78, 0x9D, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01];
    assert!(
        zlib_decompress(&bad_flg_zlib, &mut out).is_err(),
        "Zlib must reject invalid FLG check bits"
    );

    // 3. Gzip bad magic ID bytes (Not 0x1F, 0x8B)
    let bad_magic_gzip = [0x1F, 0x8C, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03];
    assert!(
        gzip_decompress(&bad_magic_gzip, &mut out).is_err(),
        "Gzip must reject invalid magic ID bytes"
    );

    // 4. Gzip reserved flags set (Bits 5, 6, 7 of FLG)
    let bad_flags_gzip = [0x1F, 0x8B, 0x08, 0xE0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03];
    assert!(
        gzip_decompress(&bad_flags_gzip, &mut out).is_err(),
        "Gzip must reject reserved header flags"
    );
}
