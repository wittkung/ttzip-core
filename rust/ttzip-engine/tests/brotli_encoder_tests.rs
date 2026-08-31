// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration tests for TTZip Google Brotli tiered streaming encoder (Q0..=Q11).

use std::io::{Read, Write};
use ttzip_engine::codecs::brotli::{
    brotli_decompress_to_vec, BrotliEncoderMode, BrotliEncoderParams, BrotliError, BrotliQuality,
    BrotliStreamWriter,
};

/// Simple deterministic pseudo-random byte generator for testing without external crates.
fn generate_prng_bytes(len: usize, seed: u64) -> Vec<u8> {
    let mut state = seed;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        out.push((state & 0xFF) as u8);
    }
    out
}

/// Helper function to decompress a Brotli stream using `brotli::Decompressor`.
fn decompress_brotli_stream(compressed: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut reader = brotli::Decompressor::new(compressed, 65536);
    let mut decompressed = Vec::new();
    reader.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

#[test]
fn test_brotli_quality_gradient_q0_to_q11_roundtrip() {
    let sample_text = b"TTZip Brotli High-Performance Microkernel Architecture 2026. \
        The quick brown fox jumps over the lazy dog repeatedly. \
        Stream compression across all 12 quality levels from Fast 1-Pass Q0 up to HQ Optimal Q11. \
        Zero allocation path sanitization and strict bounds-first invariants.";

    for quality in 0..=11 {
        let mut compressed_buf = Vec::new();
        let mut writer = BrotliStreamWriter::with_quality(&mut compressed_buf, quality)
            .unwrap_or_else(|_| panic!("failed to initialize BrotliStreamWriter for Q{}", quality));

        assert_eq!(writer.quality().value(), quality);
        writer.write_all(sample_text).expect("write failed");
        assert_eq!(writer.total_in(), sample_text.len() as u64);

        let _ = writer.finish().expect("finish failed");
        assert!(
            !compressed_buf.is_empty(),
            "Compressed output for Q{} must not be empty",
            quality
        );

        let decompressed = decompress_brotli_stream(&compressed_buf)
            .unwrap_or_else(|e| panic!("decompression failed for Q{}: {}", quality, e));

        assert_eq!(
            decompressed.as_slice(),
            sample_text,
            "100% roundtrip fidelity failure for Q{}",
            quality
        );
    }
}

#[test]
fn test_brotli_stream_writer_empty_input() {
    let mut compressed_buf = Vec::new();
    let writer = BrotliStreamWriter::with_quality(&mut compressed_buf, 6)
        .expect("initialize writer for empty input");

    assert_eq!(writer.total_in(), 0);
    let _ = writer.finish().expect("finish empty stream");

    let decompressed = decompress_brotli_stream(&compressed_buf).expect("decompress empty stream");
    assert!(
        decompressed.is_empty(),
        "Decompressed empty stream must yield 0 bytes"
    );
}

#[test]
fn test_brotli_stream_writer_small_inputs() {
    let sizes = [1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 100];
    let qualities = [0, 1, 4, 6, 9, 11];

    for &size in &sizes {
        let original_data: Vec<u8> = (0..size).map(|i| (i * 7 + 11) as u8).collect();

        for &quality in &qualities {
            let mut compressed_buf = Vec::new();
            let mut writer = BrotliStreamWriter::with_quality(&mut compressed_buf, quality)
                .expect("init small input writer");

            writer
                .write_all(&original_data)
                .expect("write small input");
            let _ = writer.finish().expect("finish small input");

            let decompressed = decompress_brotli_stream(&compressed_buf)
                .expect("decompress small input payload");

            assert_eq!(
                decompressed, original_data,
                "Small input roundtrip mismatch (size: {}, Q: {})",
                size, quality
            );
        }
    }
}

#[test]
fn test_brotli_stream_writer_medium_file_64kb() {
    let size = 64 * 1024;
    let pattern = b"BrotliMediumStream64KBChunkTest_AppleSilicon_M5Max_ZeroCopy_";
    let mut payload = Vec::with_capacity(size);
    while payload.len() < size {
        payload.extend_from_slice(pattern);
    }
    payload.truncate(size);

    let mut compressed_buf = Vec::new();
    let mut writer =
        BrotliStreamWriter::with_quality(&mut compressed_buf, 5).expect("init 64KB writer");

    // Write in chunks of 4096 bytes
    for chunk in payload.chunks(4096) {
        writer.write_all(chunk).expect("chunk write failed");
    }
    assert_eq!(writer.total_in(), size as u64);

    let _ = writer.finish().expect("finish 64KB stream");
    assert!(
        compressed_buf.len() < size,
        "Compressed size ({}) should be significantly less than 64KB ({})",
        compressed_buf.len(),
        size
    );

    let decompressed =
        decompress_brotli_stream(&compressed_buf).expect("decompress 64KB stream failed");
    assert_eq!(decompressed, payload);
}

#[test]
fn test_brotli_stream_writer_large_file_512kb() {
    let size = 512 * 1024; // 512KB payload
    let mut payload = Vec::with_capacity(size);
    let chunk_sample = b"JSON_REST_API_LOG_ENTRY_STREAM_2026_MICROKERNEL_HIGH_THROUGHPUT_BOUNDS_";
    while payload.len() < size {
        payload.extend_from_slice(chunk_sample);
    }
    payload.truncate(size);

    let mut compressed_buf = Vec::new();
    let params = BrotliEncoderParams::fast();
    let mut writer = BrotliStreamWriter::new(&mut compressed_buf, params);

    // Stream writes in varying buffer sizes (16KB, 32KB, 64KB)
    for (i, chunk) in payload.chunks(32 * 1024).enumerate() {
        writer
            .write_all(chunk)
            .unwrap_or_else(|e| panic!("write chunk {} failed: {}", i, e));
    }
    assert_eq!(writer.total_in(), size as u64);

    let _ = writer.finish().expect("finish 512KB stream");

    let ratio = (compressed_buf.len() as f64) / (size as f64);
    assert!(
        ratio < 0.05,
        "Highly repetitive 512KB payload must compress to < 5% ratio, got {}",
        ratio
    );

    let decompressed = brotli_decompress_to_vec(&compressed_buf, 1024 * 1024)
        .expect("decompress 512KB stream failed");
    assert_eq!(decompressed.len(), size);
    assert_eq!(decompressed, payload);
}

#[test]
fn test_brotli_stream_writer_flush_and_finish_integrity() {
    let part1 = b"Part 1: Initial header segment and stream introduction.";
    let part2 = b"Part 2: Middle payload with intermediate flush checkpoint.";
    let part3 = b"Part 3: Final tail records preceding stream finish.";

    let mut compressed_buf = Vec::new();
    let mut writer =
        BrotliStreamWriter::with_quality(&mut compressed_buf, 6).expect("init flush writer");

    writer.write_all(part1).expect("write part 1");
    writer.flush().expect("flush 1");

    writer.write_all(part2).expect("write part 2");
    writer.flush().expect("flush 2");

    writer.write_all(part3).expect("write part 3");
    let _ = writer.finish().expect("finish stream");

    let decompressed = decompress_brotli_stream(&compressed_buf).expect("decompress flushed stream");

    let mut expected = Vec::new();
    expected.extend_from_slice(part1);
    expected.extend_from_slice(part2);
    expected.extend_from_slice(part3);

    assert_eq!(decompressed, expected);
}

#[test]
fn test_brotli_stream_writer_high_entropy_random_data() {
    // 64KB of high-entropy pseudorandom bytes (entropy ~ 7.99 bits/byte)
    let random_data = generate_prng_bytes(64 * 1024, 0x9E3779B97F4A7C15);

    let mut compressed_buf = Vec::new();
    let mut writer =
        BrotliStreamWriter::with_quality(&mut compressed_buf, 2).expect("init random data writer");

    writer
        .write_all(&random_data)
        .expect("write random high entropy data");

    // Entropy analyzer should detect high-entropy noise
    assert!(
        writer.is_incompressible(),
        "High-entropy random payload must trigger incompressibility flag"
    );

    let _ = writer.finish().expect("finish random stream");

    let decompressed =
        decompress_brotli_stream(&compressed_buf).expect("decompress random data stream");
    assert_eq!(decompressed, random_data);
}

#[test]
fn test_brotli_encoder_params_validation() {
    // Quality > 11 must fail validation
    assert!(BrotliEncoderParams::with_quality(12).is_err());
    assert_eq!(
        BrotliQuality::new(12),
        Err(BrotliError::InvalidQuality(12))
    );

    // Valid qualities
    assert!(BrotliQuality::new(0).unwrap().is_fast());
    assert!(BrotliQuality::new(1).unwrap().is_fast());
    assert!(BrotliQuality::new(2).unwrap().is_balanced());
    assert!(BrotliQuality::new(9).unwrap().is_balanced());
    assert!(BrotliQuality::new(10).unwrap().is_optimal());
    assert!(BrotliQuality::new(11).unwrap().is_optimal());

    // Window validation
    let mut params = BrotliEncoderParams::balanced();
    assert!(params.validated().is_ok());

    params.lgwin = 9; // < 10
    assert!(params.validated().is_err());

    params.lgwin = 31; // > 30
    assert!(params.validated().is_err());
}

#[test]
fn test_brotli_encoder_modes() {
    let text_sample = b"<html><head><title>TTZip Brotli</title></head><body><h1>Text Mode Test</h1></body></html>";

    for mode in [
        BrotliEncoderMode::Generic,
        BrotliEncoderMode::Text,
        BrotliEncoderMode::Font,
    ] {
        let mut params = BrotliEncoderParams::balanced();
        params.mode = mode;

        let mut compressed = Vec::new();
        let mut writer = BrotliStreamWriter::new(&mut compressed, params);
        writer.write_all(text_sample).expect("write text sample");
        let _ = writer.finish().expect("finish text mode");

        let decomp = decompress_brotli_stream(&compressed).expect("decompress mode sample");
        assert_eq!(decomp.as_slice(), text_sample);
    }
}
