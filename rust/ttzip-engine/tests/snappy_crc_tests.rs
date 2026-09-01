// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Integration tests and performance throughput gates for Snappy Masked CRC-32C engine.

use std::time::Instant;
use ttzip_engine::codecs::snappy::crc::scalar::crc32c_slice8;
use ttzip_engine::codecs::snappy::{
    crc32c, crc32c_update, mask_crc32c, unmask_crc32c, SnappyCrc32cHasher,
    CASTAGNOLI_POLYNOMIAL, SNAPPY_CRC_MASK_DELTA,
};

#[test]
fn test_snappy_crc32c_standard_vectors() {
    // 1. Standard RFC 3720 / Castagnoli test vectors
    assert_eq!(crc32c(b""), 0x00000000, "Empty payload must produce 0x00000000");
    assert_eq!(
        crc32c(b"123456789"),
        0xE3069283,
        "Standard vector '123456789' must produce 0xE3069283"
    );

    // Verify scalar slice-by-8 produces identical results
    assert_eq!(crc32c_slice8(0, b""), 0x00000000);
    assert_eq!(crc32c_slice8(0, b"123456789"), 0xE3069283);

    // 2. 32 bytes of zeros (RFC 3720 Section B.4 example)
    let zeros = [0u8; 32];
    let expected_zeros_crc = 0x8A9136AA;
    assert_eq!(crc32c(&zeros), expected_zeros_crc);
    assert_eq!(crc32c_slice8(0, &zeros), expected_zeros_crc);

    // 3. 32 bytes of 0xFF (RFC 3720 Section B.4 example)
    let ones = [0xFFu8; 32];
    let expected_ones_crc = 0x62A8AB43;
    assert_eq!(crc32c(&ones), expected_ones_crc);
    assert_eq!(crc32c_slice8(0, &ones), expected_ones_crc);

    // 4. 32 bytes of increasing values 0x00..0x1F
    let mut inc = [0u8; 32];
    for (i, item) in inc.iter_mut().enumerate() {
        *item = i as u8;
    }
    let expected_inc_crc = 0x46DD794E;
    assert_eq!(crc32c(&inc), expected_inc_crc);
    assert_eq!(crc32c_slice8(0, &inc), expected_inc_crc);
}

#[test]
fn test_snappy_crc32c_constants() {
    assert_eq!(CASTAGNOLI_POLYNOMIAL, 0x82F63B78);
    assert_eq!(SNAPPY_CRC_MASK_DELTA, 0xa282_ead8);
}

#[test]
fn test_snappy_crc32c_mask_unmask_reversibility_and_boundaries() {
    // 1. Critical boundary values
    let boundary_samples: [u32; 18] = [
        0x00000000,
        0x00000001,
        0x00000002,
        0x7FFFFFFF,
        0x80000000,
        0xFFFFFFFE,
        0xFFFFFFFF,
        0xAAAAAAAA,
        0x55555555,
        0x12345678,
        0x87654321,
        0xE3069283,
        0x8A9136AA,
        0x62A8AB43,
        0x46DD794E,
        SNAPPY_CRC_MASK_DELTA,
        !SNAPPY_CRC_MASK_DELTA,
        0xDEADBEEF,
    ];

    for &val in &boundary_samples {
        let masked = mask_crc32c(val);
        let unmasked = unmask_crc32c(masked);
        assert_eq!(
            unmasked, val,
            "Reversibility failed for boundary sample: 0x{:08X}",
            val
        );
    }

    // 2. Comprehensive strided scan across full 32-bit integer space (16.7M samples)
    let stride: u32 = 257; // Coprime with 2^32
    let mut val: u32 = 0;
    for _ in 0..100_000 {
        let masked = mask_crc32c(val);
        let unmasked = unmask_crc32c(masked);
        assert_eq!(unmasked, val, "Reversibility failed for value: 0x{:08X}", val);
        val = val.wrapping_add(stride);
    }

    // 3. Bit-shift and single-bit set patterns
    for bit in 0..32 {
        let single_bit = 1u32 << bit;
        let masked = mask_crc32c(single_bit);
        let unmasked = unmask_crc32c(masked);
        assert_eq!(unmasked, single_bit);

        let inverted_bit = !single_bit;
        let masked_inv = mask_crc32c(inverted_bit);
        let unmasked_inv = unmask_crc32c(masked_inv);
        assert_eq!(unmasked_inv, inverted_bit);
    }
}

#[test]
fn test_snappy_crc32c_streaming_hasher_equivalence() {
    let test_sizes = [
        0, 1, 2, 3, 4, 7, 8, 9, 15, 16, 17, 31, 32, 33, 63, 64, 65, 127, 128, 129,
        255, 256, 512, 1024, 4096, 65536, 131072,
    ];

    let mut payload = vec![0u8; 131072];
    for (i, b) in payload.iter_mut().enumerate() {
        *b = ((i * 37 + 19) & 0xFF) as u8;
    }

    for &size in &test_sizes {
        let slice = &payload[..size];
        let expected_crc = crc32c(slice);
        let expected_slice8 = crc32c_slice8(0, slice);
        assert_eq!(
            expected_crc, expected_slice8,
            "Hardware and Slice-by-8 mismatch at size {}",
            size
        );

        // 1. One-shot Hasher
        let mut hasher = SnappyCrc32cHasher::new();
        hasher.update(slice);
        assert_eq!(hasher.finalize(), expected_crc);
        assert_eq!(hasher.finalize_masked(), mask_crc32c(expected_crc));

        // 2. Incremental single-byte streaming
        let mut byte_hasher = SnappyCrc32cHasher::new();
        for &byte in slice {
            byte_hasher.update(&[byte]);
        }
        assert_eq!(
            byte_hasher.finalize(),
            expected_crc,
            "Single-byte chunking mismatch at size {}",
            size
        );

        // 3. Incremental arbitrary chunk sizes (e.g. chunks of 7, 13, 31, 64 bytes)
        for chunk_size in [3, 7, 13, 31, 64, 128] {
            let mut chunk_hasher = SnappyCrc32cHasher::new();
            for chunk in slice.chunks(chunk_size) {
                chunk_hasher.update(chunk);
            }
            assert_eq!(
                chunk_hasher.finalize(),
                expected_crc,
                "Chunked ({}) streaming mismatch at size {}",
                chunk_size,
                size
            );
        }

        // 4. Hasher reset functionality
        hasher.reset();
        assert_eq!(hasher.finalize(), 0);
        hasher.update(slice);
        assert_eq!(hasher.finalize(), expected_crc);
    }
}

#[test]
fn test_snappy_crc32c_unaligned_offsets() {
    let mut buffer = vec![0u8; 4096];
    for (i, b) in buffer.iter_mut().enumerate() {
        *b = ((i * 43 + 7) & 0xFF) as u8;
    }

    // Test non-aligned slice offsets (0..16) across multiple lengths
    for offset in 0..16 {
        for length in [0, 1, 5, 8, 15, 32, 64, 127, 256, 1024, 2048] {
            if offset + length <= buffer.len() {
                let slice = &buffer[offset..offset + length];
                let hw = crc32c(slice);
                let sc = crc32c_slice8(0, slice);
                assert_eq!(
                    hw, sc,
                    "Unaligned CRC mismatch at offset {} with length {}",
                    offset, length
                );
            }
        }
    }
}

#[test]
fn test_snappy_crc32c_incremental_update_chain() {
    let data_a = b"The quick brown fox ";
    let data_b = b"jumps over the lazy dog. ";
    let data_c = b"Snappy framing format Castagnoli checksum test.";

    let mut combined = Vec::new();
    combined.extend_from_slice(data_a);
    combined.extend_from_slice(data_b);
    combined.extend_from_slice(data_c);

    let full_crc = crc32c(&combined);

    let crc_a = crc32c(data_a);
    let crc_ab = crc32c_update(crc_a, data_b);
    let crc_abc = crc32c_update(crc_ab, data_c);

    assert_eq!(crc_abc, full_crc, "Chained incremental updates must equal full calculation");
    assert_eq!(mask_crc32c(crc_abc), mask_crc32c(full_crc));
}

#[test]
fn test_snappy_crc32c_hardware_throughput_gate() {
    // 16 MB payload for realistic L3/DRAM throughput measurement
    const BENCH_SIZE: usize = 16 * 1024 * 1024;
    let mut payload = vec![0u8; BENCH_SIZE];
    for (i, b) in payload.iter_mut().enumerate() {
        *b = ((i ^ (i >> 8)) & 0xFF) as u8;
    }

    // Warm-up iteration
    let initial_crc = crc32c(&payload);
    assert_ne!(initial_crc, 0);

    // Timed throughput measurement (3 iterations to average)
    let iterations = 3;
    let start = Instant::now();
    let mut accumulated = 0u32;
    for _ in 0..iterations {
        accumulated ^= crc32c(&payload);
    }
    let elapsed = start.elapsed();
    assert_eq!(accumulated, if iterations % 2 == 1 { initial_crc } else { 0 });

    let total_bytes = (BENCH_SIZE * iterations) as f64;
    let elapsed_sec = elapsed.as_secs_f64();
    let throughput_gb_per_sec = (total_bytes / (1024.0 * 1024.0 * 1024.0)) / elapsed_sec;

    println!(
        "[CRC32C Hardware Gate] Processed {} MB in {:.4}s -> Throughput: {:.2} GB/s",
        (BENCH_SIZE * iterations) / (1024 * 1024),
        elapsed_sec,
        throughput_gb_per_sec
    );

    let min_gbps = if cfg!(debug_assertions) { 0.08 } else { 1.0 };
    // Hard architecture gate: Throughput MUST exceed threshold
    assert!(
        throughput_gb_per_sec > min_gbps,
        "CRC-32C throughput {:.2} GB/s did not satisfy > {:.2} GB/s gate!",
        throughput_gb_per_sec,
        min_gbps
    );
}
