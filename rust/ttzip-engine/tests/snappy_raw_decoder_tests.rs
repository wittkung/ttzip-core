// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive test suite and throughput gate for Snappy raw block decompression.

use std::time::Instant;
use ttzip_engine::codecs::snappy::*;

#[test]
fn test_raw_uncompressed_length_valid_and_invalid() {
    // 1. Valid single-byte varint
    let mut buf = [0u8; 10];
    encode_varint32(1024, &mut buf);
    assert_eq!(raw_uncompressed_length(&buf).expect("valid varint"), 1024);

    // 2. Empty slice
    assert_eq!(raw_uncompressed_length(&[]), Err(SnappyError::UnexpectedEof));

    // 3. Truncated varint
    assert_eq!(
        raw_uncompressed_length(&[0x80, 0x80]),
        Err(SnappyError::UnexpectedEof)
    );

    // 4. Varint overflow (> 32 bits)
    assert_eq!(
        raw_uncompressed_length(&[0xFF, 0xFF, 0xFF, 0xFF, 0x80]),
        Err(SnappyError::VarintOverflow)
    );
}

#[test]
fn test_raw_decompress_zero_length() {
    let empty_compressed = [0x00];
    let mut dst = [0u8; 16];
    let written = raw_decompress(&empty_compressed, &mut dst).expect("decompress zero length");
    assert_eq!(written, 0);

    // Extraneous byte in zero-length stream must fail
    let invalid_empty = [0x00, 0xAA];
    assert!(raw_decompress(&invalid_empty, &mut dst).is_err());
}

#[test]
fn test_raw_roundtrip_various_payload_sizes_and_patterns() {
    let payloads: Vec<Vec<u8>> = vec![
        // 1. Tiny literal
        b"A".to_vec(),
        b"Hello Snappy".to_vec(),
        b"Quick brown fox jumps over the lazy dog 1234567890".to_vec(),
        // 2. Repetitive RLE
        vec![0x42; 256],
        vec![0x7F; 4096],
        vec![0xAA; 65536],
        vec![0x55; 131072],
        // 3. Arithmetic ramp
        (0..10000).map(|i| (i % 256) as u8).collect(),
        // 4. Structured text
        b"The quick brown fox jumps over the lazy dog. ".repeat(200),
        // 5. Mixed pseudo-random / semi-structured
        {
            let mut data = Vec::with_capacity(100_000);
            let mut state: u32 = 0x12345678;
            for i in 0..100_000 {
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                if i % 10 < 7 {
                    data.push((state & 0x07) as u8); // Repetitive pattern
                } else {
                    data.push((state >> 24) as u8); // Random noise
                }
            }
            data
        },
    ];

    for (idx, original) in payloads.iter().enumerate() {
        let compressed = raw_compress_to_vec(original)
            .unwrap_or_else(|e| panic!("compression failed at index {idx}: {e:?}"));

        assert!(
            raw_validate(&compressed, original.len() + 1024),
            "Validation failed for payload {idx}"
        );

        let uncompressed_len = raw_uncompressed_length(&compressed)
            .unwrap_or_else(|e| panic!("uncompressed length failed at index {idx}: {e:?}"));
        assert_eq!(uncompressed_len, original.len());

        let mut decompressed = vec![0u8; original.len()];
        let written = raw_decompress(&compressed, &mut decompressed)
            .unwrap_or_else(|e| panic!("decompression failed at index {idx}: {e:?}"));
        assert_eq!(written, original.len());
        assert_eq!(&decompressed, original, "Payload mismatch at index {idx}");

        // Also test raw_decompress_to_vec
        let decomp_vec = raw_decompress_to_vec(&compressed)
            .unwrap_or_else(|e| panic!("decompress_to_vec failed at index {idx}: {e:?}"));
        assert_eq!(&decomp_vec, original);

        // Cross-validation with canonical snap::raw::Decoder
        let mut snap_dec = snap::raw::Decoder::new();
        let snap_decomp = snap_dec
            .decompress_vec(&compressed)
            .expect("snap::raw::Decoder cross-validation failed");
        assert_eq!(&snap_decomp, original);
    }
}

#[test]
fn test_raw_overlapping_match_patterns_offset_1_to_15() {
    // Construct exact repeating pattern blocks for offsets 1..=15 with various match lengths
    for offset in 1..=15usize {
        for match_len in [4, 7, 8, 11, 16, 23, 32, 47, 64] {
            let mut pattern = Vec::new();
            for i in 0..offset {
                pattern.push(b'A' + (i as u8));
            }

            // Expected decompressed data: seed pattern followed by match_len bytes replicated
            let mut expected = Vec::new();
            expected.extend_from_slice(&pattern);
            for _ in 0..match_len {
                let b = expected[expected.len() - offset];
                expected.push(b);
            }

            // Manually craft Snappy bitstream to force overlapping match copy:
            // 1. Varint uncompressed length
            let mut stream = Vec::new();
            let mut varint_buf = [0u8; 8];
            let v_len = encode_varint32(expected.len() as u32, &mut varint_buf);
            stream.extend_from_slice(&varint_buf[..v_len]);

            // 2. Literal tag + seed pattern
            let mut lit_tag_buf = [0u8; 8];
            let t_len = emit_literal_tag(pattern.len(), &mut lit_tag_buf).expect("emit literal");
            stream.extend_from_slice(&lit_tag_buf[..t_len]);
            stream.extend_from_slice(&pattern);

            // 3. Copy tag (use Copy1 if len in 4..11 and offset <= 2047, else Copy2)
            if (4..=11).contains(&match_len) && offset <= 2047 {
                let mut copy1_buf = [0u8; 4];
                let c_len = emit_copy1_tag(match_len, offset as u32, &mut copy1_buf).expect("emit copy1");
                stream.extend_from_slice(&copy1_buf[..c_len]);
            } else {
                let mut copy2_buf = [0u8; 4];
                let c_len = emit_copy2_tag(match_len, offset as u32, &mut copy2_buf).expect("emit copy2");
                stream.extend_from_slice(&copy2_buf[..c_len]);
            }

            // Validate stream
            assert!(
                raw_validate(&stream, expected.len() + 64),
                "Validation failed for offset={offset}, len={match_len}"
            );

            // Decompress and verify exact bit-for-bit pattern
            let mut decompressed = vec![0u8; expected.len()];
            let written = raw_decompress(&stream, &mut decompressed).unwrap_or_else(|e| {
                panic!("Decompression failed for offset={offset}, len={match_len}: {e:?}")
            });
            assert_eq!(written, expected.len());
            assert_eq!(
                decompressed, expected,
                "Content mismatch for offset={offset}, match_len={match_len}"
            );
        }
    }
}

#[test]
fn test_raw_corrupted_stream_and_boundary_attacks() {
    // 1. Truncated stream after varint
    let truncated_varint = [0x40]; // says 64 bytes uncompressed, but no tags
    let mut dst = [0u8; 64];
    assert!(raw_decompress(&truncated_varint, &mut dst).is_err());
    assert!(!raw_validate(&truncated_varint, 64));

    // 2. Buffer too small
    let valid_data = b"Some valid uncompressed stream data for testing buffer size checks";
    let compressed = raw_compress_to_vec(valid_data).expect("compress");
    let mut tiny_dst = [0u8; 10];
    assert!(matches!(
        raw_decompress(&compressed, &mut tiny_dst),
        Err(SnappyError::BufferTooSmall { required, available: 10 }) if required == valid_data.len()
    ));

    // 3. Zero offset copy attack
    // Header len: 10 (0x0A), Copy1 tag with offset 0
    let zero_offset_stream = [0x0A, 0x01, 0x00];
    assert!(raw_decompress(&zero_offset_stream, &mut dst).is_err());
    assert!(!raw_validate(&zero_offset_stream, 10));

    // 4. Out-of-bounds offset copy (offset > op)
    // Header len: 10 (0x0A), Tag: Copy1 len 4, offset 5 (before any literals)
    let oob_offset_stream = [0x0A, 0x05, 0x05];
    assert!(raw_decompress(&oob_offset_stream, &mut dst).is_err());
    assert!(!raw_validate(&oob_offset_stream, 10));

    // 5. Truncated literal payload
    // Header len: 10 (0x0A), Literal tag for 8 bytes, but only 3 bytes provided
    let truncated_lit = [0x0A, (7 << 2), 0x01, 0x02, 0x03];
    assert!(raw_decompress(&truncated_lit, &mut dst).is_err());
    assert!(!raw_validate(&truncated_lit, 10));

    // 6. Extraneous trailing compressed bytes when target length is already met
    let mut with_extra = raw_compress_to_vec(b"Complete block").expect("compress");
    with_extra.push(0xAA);
    assert!(raw_decompress(&with_extra, &mut dst).is_err());
    assert!(!raw_validate(&with_extra, 100));
}

#[test]
fn test_raw_decompression_throughput_gate() {
    // Generate 4MB structured compressible text & repeating blocks
    let base_chunk = b"TTZip High Performance Snappy Raw Decompressor SIMD Vectorization Benchmark 2026! ";
    let iterations = (4 * 1024 * 1024) / base_chunk.len();
    let mut original = Vec::with_capacity(iterations * base_chunk.len());
    for _ in 0..iterations {
        original.extend_from_slice(base_chunk);
    }

    let compressed = raw_compress_to_vec(&original).expect("compress 4MB payload");
    let mut dst = vec![0u8; original.len()];

    // Warm-up run
    let written = raw_decompress(&compressed, &mut dst).expect("warmup decompress");
    assert_eq!(written, original.len());
    assert_eq!(&dst, &original);

    // Benchmark loop: 50 iterations over 4MB (200MB processed)
    let bench_iters = 50;
    let start = Instant::now();
    for _ in 0..bench_iters {
        let w = raw_decompress(&compressed, &mut dst).expect("bench decompress");
        assert_eq!(w, original.len());
    }
    let elapsed = start.elapsed();

    let total_bytes = (original.len() * bench_iters) as f64;
    let elapsed_secs = elapsed.as_secs_f64();
    let throughput_gb_s = (total_bytes / (1024.0 * 1024.0 * 1024.0)) / elapsed_secs;
    let throughput_mb_s = (total_bytes / (1024.0 * 1024.0)) / elapsed_secs;

    println!(
        "[Snappy Raw Decompressor Benchmark] Processed {:.2} MB in {:.4} s -> Throughput: {:.2} GB/s ({:.2} MB/s)",
        (total_bytes / (1024.0 * 1024.0)),
        elapsed_secs,
        throughput_gb_s,
        throughput_mb_s
    );

    // Performance Gate: must exceed 1.5 GB/s (1500 MB/s)
    assert!(
        throughput_gb_s > 1.5,
        "Throughput gate failed: expected > 1.5 GB/s, got {:.2} GB/s ({:.2} MB/s)",
        throughput_gb_s,
        throughput_mb_s
    );
}
