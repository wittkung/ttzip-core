// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive test suite and throughput gate for pure-Rust Google Snappy raw encoder.

use std::time::Instant;
use ttzip_engine::codecs::snappy::*;

/// Simple deterministic pseudo-random number generator (SplitMix64) for high-entropy corpus generation.
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        let mut chunks = dest.chunks_exact_mut(8);
        for chunk in &mut chunks {
            let val = self.next_u64();
            chunk.copy_from_slice(&val.to_le_bytes());
        }
        let rem = chunks.into_remainder();
        if !rem.is_empty() {
            let val = self.next_u64();
            let bytes = val.to_le_bytes();
            rem.copy_from_slice(&bytes[..rem.len()]);
        }
    }
}

#[test]
fn test_raw_encoder_empty_buffer() {
    let empty = b"";
    let bound = max_compressed_len(empty.len());
    let mut comp = vec![0u8; bound];
    let written = raw_compress(empty, &mut comp).expect("compress empty buffer");
    assert_eq!(written, 1);
    assert_eq!(&comp[..written], &[0x00]);

    let mut decomp = Vec::new();
    let dec_len = snappy_decompress(&comp[..written], &mut decomp).expect("decompress empty");
    assert_eq!(dec_len, 0);
    assert!(decomp.is_empty());
}

#[test]
fn test_raw_encoder_small_strings_matrix() {
    for len in 1..=100 {
        // 1. Monotonic ASCII sequence
        let mut data = Vec::with_capacity(len);
        for i in 0..len {
            data.push(b'a' + (i % 26) as u8);
        }

        let bound = max_compressed_len(data.len());
        let mut comp = vec![0u8; bound];
        let written = raw_compress(&data, &mut comp).expect("compress small string");
        assert!(written > 0);
        assert!(written <= bound);

        let mut decomp = vec![0u8; data.len()];
        let dec_len = snappy_decompress(&comp[..written], &mut decomp).expect("decompress small string");
        assert_eq!(dec_len, data.len());
        assert_eq!(&decomp[..dec_len], &data[..]);

        // 2. Uniform repeated byte
        let rep_data = vec![0x5Au8; len];
        let mut rep_comp = vec![0u8; max_compressed_len(len)];
        let rep_written = raw_compress(&rep_data, &mut rep_comp).expect("compress uniform string");
        assert!(rep_written <= max_compressed_len(len));

        let mut rep_decomp = vec![0u8; len];
        let rep_dec_len = snappy_decompress(&rep_comp[..rep_written], &mut rep_decomp).expect("decompress uniform");
        assert_eq!(rep_dec_len, len);
        assert_eq!(&rep_decomp[..rep_dec_len], &rep_data[..]);
    }
}

#[test]
fn test_raw_encoder_medium_64kb_block() {
    let mut rng = SplitMix64::new(0xCAFEBABE12345678);
    let mut payload = vec![0u8; SNAPPY_BLOCK_SIZE];

    // Interleave pattern data with pseudo-random noise to exercise both literals and copies
    for i in 0..payload.len() {
        if i % 16 < 8 {
            payload[i] = (i % 256) as u8;
        } else {
            payload[i] = (rng.next_u64() & 0xFF) as u8;
        }
    }

    let bound = max_compressed_len(payload.len());
    let mut comp = vec![0u8; bound];
    let written = raw_compress(&payload, &mut comp).expect("compress 64KB block");
    assert!(written > 0);
    assert!(written <= bound);

    let mut decomp = vec![0u8; payload.len()];
    let dec_len = snappy_decompress(&comp[..written], &mut decomp).expect("decompress 64KB block");
    assert_eq!(dec_len, payload.len());
    assert_eq!(&decomp[..dec_len], &payload[..]);
}

#[test]
fn test_raw_encoder_large_512kb_multi_block() {
    let size = 512 * 1024; // 512KB (8x 64KB blocks)
    let mut payload = vec![0u8; size];
    let pattern = b"The quick brown fox jumps over the lazy dog. 2026 TTZip High Performance Microkernel.";

    for (i, byte) in payload.iter_mut().enumerate() {
        *byte = pattern[i % pattern.len()];
    }

    let bound = max_compressed_len(payload.len());
    let mut comp = vec![0u8; bound];
    let written = raw_compress(&payload, &mut comp).expect("compress 512KB buffer");
    assert!(written > 0);
    assert!(written <= bound);

    // Highly repetitive pattern should compress substantially
    assert!(
        written < size / 4,
        "Repetitive 512KB text should compress to < 128KB, got {} bytes",
        written
    );

    let mut decomp = vec![0u8; payload.len()];
    let dec_len = snappy_decompress(&comp[..written], &mut decomp).expect("decompress 512KB buffer");
    assert_eq!(dec_len, payload.len());
    assert_eq!(&decomp[..dec_len], &payload[..]);
}

#[test]
fn test_raw_encoder_high_entropy_random_data_fast_skipping() {
    let size = 64 * 1024;
    let mut random_data = vec![0u8; size];
    let mut rng = SplitMix64::new(0x9876543210FEDCBA);
    rng.fill_bytes(&mut random_data);

    let bound = max_compressed_len(random_data.len());
    let mut comp = vec![0u8; bound];
    let written = raw_compress(&random_data, &mut comp).expect("compress high entropy");

    assert!(written > 0);
    assert!(
        written <= bound,
        "Compressed output {} must be <= max_compressed_len {}",
        written,
        bound
    );

    // Verify worst-case expansion ratio <= (32 + N + N / 6) / N
    let expansion_ratio = (written as f64) / (size as f64);
    assert!(
        expansion_ratio < 1.20,
        "Expansion ratio for high-entropy random data must be < 1.20, got {:.4}",
        expansion_ratio
    );

    let mut decomp = vec![0u8; size];
    let dec_len = snappy_decompress(&comp[..written], &mut decomp).expect("decompress high entropy");
    assert_eq!(dec_len, size);
    assert_eq!(&decomp[..dec_len], &random_data[..]);
}

#[test]
fn test_raw_encoder_buffer_too_small_rejection() {
    let data = b"Testing buffer size validation failure in raw_compress";
    let bound = max_compressed_len(data.len());
    let mut small_buf = vec![0u8; bound - 1];

    let res = raw_compress(data, &mut small_buf);
    assert!(matches!(res, Err(SnappyError::BufferTooSmall { .. })));
}

#[test]
fn test_raw_encoder_throughput_gate_over_500mbs() {
    let size = 512 * 1024; // 512KB payload
    let mut corpus = vec![0u8; size];
    let sample_text = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 1234567890.";

    for (i, byte) in corpus.iter_mut().enumerate() {
        *byte = sample_text[i % sample_text.len()];
    }

    let bound = max_compressed_len(corpus.len());
    let mut comp = vec![0u8; bound];

    // Warm-up run
    let _ = raw_compress(&corpus, &mut comp).expect("warmup compress");

    // Timed benchmark over 10 iterations
    let iterations = 10;
    let start = Instant::now();
    for _ in 0..iterations {
        let written = raw_compress(&corpus, &mut comp).expect("benchmark compress");
        assert!(written > 0);
    }
    let elapsed = start.elapsed();

    let total_bytes = (size * iterations) as f64;
    let total_seconds = elapsed.as_secs_f64().max(1e-9);
    let throughput_mbs = (total_bytes / (1024.0 * 1024.0)) / total_seconds;

    println!(
        "[Snappy Raw Encoder] Processed {:.2} MB in {:.4} s => Throughput: {:.2} MB/s",
        total_bytes / (1024.0 * 1024.0),
        total_seconds,
        throughput_mbs
    );

    assert!(
        throughput_mbs > 500.0,
        "Snappy raw compressor single-core throughput must exceed 500 MB/s, got {:.2} MB/s",
        throughput_mbs
    );
}

#[test]
fn test_raw_encoder_swar_edge_cases() {
    let s1 = [0xAA; 32];
    let mut s2 = [0xAA; 32];
    assert_eq!(find_match_length(&s1, &s2), 32);

    s2[0] = 0xBB;
    assert_eq!(find_match_length(&s1, &s2), 0);

    s2[0] = 0xAA;
    s2[7] = 0xBB;
    assert_eq!(find_match_length(&s1, &s2), 7);

    s2[7] = 0xAA;
    s2[8] = 0xBB;
    assert_eq!(find_match_length(&s1, &s2), 8);

    s2[8] = 0xAA;
    s2[15] = 0xBB;
    assert_eq!(find_match_length(&s1, &s2), 15);

    s2[15] = 0xAA;
    s2[31] = 0xBB;
    assert_eq!(find_match_length(&s1, &s2), 31);
}
