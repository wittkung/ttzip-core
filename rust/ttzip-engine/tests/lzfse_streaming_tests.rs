// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive test suite for Apple LZFSE and LZVN Safe Rust streaming Reader and Writer.
//!
//! Validates:
//! 1. End-to-end streaming block compression and decompression roundtrip fidelity.
//! 2. Micro-step jitter chunk streaming (1-byte, 7-byte, 4096-byte stepping).
//! 3. Lossless multi-stream concatenation handling.
//! 4. 1MB+ large payload streaming pipeline across 256KB block boundaries.
//! 5. Automatic block container routing (Raw `bvx-`, LZVN `bvxn`, and LZFSE `bvx2`).
//! 6. Dual-oracle differential testing against Apple C reference implementation.
//! 7. Defensive bounds validation against truncated and corrupted container streams.

use std::io::{Cursor, Read, Write};
use ttzip_engine::codecs::lzfse::block::BvxMagic;
use ttzip_engine::codecs::lzfse::reader::{
    lzfse_decompress_stream, lzfse_validate, LzfseReader,
};
use ttzip_engine::codecs::lzfse::writer::{
    lzfse_compress_stream, LzfseWriter, DEFAULT_LZVN_THRESHOLD,
};
use ttzip_engine::codecs::lzfse::lzfse_decompress_to_vec;

// MARK: - Test Helpers

/// Linear congruential generator for reproducible deterministic test payload generation.
fn generate_deterministic_payload(size: usize, seed: u64) -> Vec<u8> {
    let mut state = seed;
    let mut buf = vec![0u8; size];
    for b in buf.iter_mut() {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *b = (state >> 33) as u8;
    }
    buf
}

/// Structured repetitive text generator with high compressibility.
fn generate_structured_text(repetitions: usize) -> Vec<u8> {
    let paragraph = b"Apple LZFSE (Lempel-Ziv Finite State Entropy) is a high-performance compression format. \
It achieves 3x decompression speed compared to zlib deflate while maintaining comparable compression ratios. \
The container format supports uncompressed (bvx-), LZVN (bvxn), and LZFSE (bvx1, bvx2) payload blocks.\n";
    let mut out = Vec::with_capacity(paragraph.len() * repetitions);
    for _ in 0..repetitions {
        out.extend_from_slice(paragraph);
    }
    out
}

// MARK: - Test Cases

#[test]
fn test_lzfse_streaming_roundtrip_empty_and_small() {
    // 1. Empty buffer roundtrip
    let empty_data = b"";
    let mut compressed_empty = Vec::new();
    {
        let mut writer = LzfseWriter::new(&mut compressed_empty);
        writer.write_all(empty_data).expect("write empty");
        writer.finish().expect("finish empty writer");
    }
    // Finished empty stream should have emitted at least the bvx$ terminal marker (4 bytes)
    assert_eq!(&compressed_empty[..], &BvxMagic::EndOfStream.as_bytes());

    let mut decompressed_empty = Vec::new();
    let mut reader = LzfseReader::new(Cursor::new(&compressed_empty));
    reader
        .read_to_end(&mut decompressed_empty)
        .expect("read empty");
    assert!(decompressed_empty.is_empty());

    // 2. Short 5-byte payload (less than 8 bytes, should route to RawUncompressed)
    let short_data = b"Hello";
    let mut compressed_short = Vec::new();
    {
        let mut writer = LzfseWriter::new(&mut compressed_short);
        writer.write_all(short_data).expect("write short");
        writer.finish().expect("finish short writer");
    }
    assert!(compressed_short.len() >= 8); // 8 bytes raw header + 5 bytes payload + 4 bytes EOS

    let mut decompressed_short = Vec::new();
    let mut reader_short = LzfseReader::new(Cursor::new(&compressed_short));
    reader_short
        .read_to_end(&mut decompressed_short)
        .expect("read short");
    assert_eq!(&decompressed_short[..], short_data);
}

#[test]
fn test_lzfse_streaming_roundtrip_medium_and_convenience_facades() {
    let data = generate_structured_text(20); // ~4.5KB (just over 4KB threshold)

    // Test convenience helper lzfse_compress_stream and lzfse_decompress_stream
    let compressed = lzfse_compress_stream(&data).expect("compress stream");
    assert!(lzfse_validate(&compressed));

    let decompressed = lzfse_decompress_stream(&compressed).expect("decompress stream");
    assert_eq!(decompressed, data);
}

#[test]
fn test_lzfse_streaming_microstep_jitter_1byte() {
    let data = generate_structured_text(10); // ~2.2KB

    // Compress by feeding exactly 1 byte at a time
    let mut compressed = Vec::new();
    {
        let mut writer = LzfseWriter::new(&mut compressed);
        for &byte in &data {
            let written = writer.write(&[byte]).expect("write single byte");
            assert_eq!(written, 1);
        }
        writer.finish().expect("finish single byte writer");
    }

    // Decompress by reading exactly 1 byte at a time
    let mut decompressed = Vec::with_capacity(data.len());
    let mut reader = LzfseReader::new(Cursor::new(&compressed));
    let mut single_byte_buf = [0u8; 1];

    loop {
        let n = reader.read(&mut single_byte_buf).expect("read single byte");
        if n == 0 {
            break;
        }
        decompressed.push(single_byte_buf[0]);
    }

    assert_eq!(decompressed, data);
}

#[test]
fn test_lzfse_streaming_microstep_jitter_7byte_and_4096byte() {
    let data = generate_structured_text(40); // ~9KB

    // Compress in 7-byte chunks
    let mut compressed = Vec::new();
    {
        let mut writer = LzfseWriter::new(&mut compressed);
        for chunk in data.chunks(7) {
            writer.write_all(chunk).expect("write 7-byte chunk");
        }
        writer.finish().expect("finish 7-byte chunk writer");
    }

    // Decompress in 4096-byte chunks
    let mut decompressed = Vec::new();
    let mut reader = LzfseReader::new(Cursor::new(&compressed));
    let mut chunk_buf = [0u8; 4096];

    loop {
        let n = reader.read(&mut chunk_buf).expect("read 4096-byte chunk");
        if n == 0 {
            break;
        }
        decompressed.extend_from_slice(&chunk_buf[..n]);
    }

    assert_eq!(decompressed, data);
}

#[test]
fn test_lzfse_streaming_multistream_concatenation() {
    let stream1_data = b"First stream payload for LZFSE multi-stream test.\n";
    let stream2_data = b"Second stream payload with some repetitive repetitive repetitive content!\n";
    let stream3_data = b"Third and final stream payload.";

    let mut stream1_compressed = Vec::new();
    let mut stream2_compressed = Vec::new();
    let mut stream3_compressed = Vec::new();

    {
        let mut w1 = LzfseWriter::new(&mut stream1_compressed);
        w1.write_all(stream1_data).expect("write s1");
        w1.finish().expect("finish s1");

        let mut w2 = LzfseWriter::new(&mut stream2_compressed);
        w2.write_all(stream2_data).expect("write s2");
        w2.finish().expect("finish s2");

        let mut w3 = LzfseWriter::new(&mut stream3_compressed);
        w3.write_all(stream3_data).expect("write s3");
        w3.finish().expect("finish s3");
    }

    // Concatenate all 3 streams into a single contiguous byte stream
    let mut concatenated = Vec::new();
    concatenated.extend_from_slice(&stream1_compressed);
    concatenated.extend_from_slice(&stream2_compressed);
    concatenated.extend_from_slice(&stream3_compressed);

    // Read concatenated stream sequentially
    let mut reader = LzfseReader::new(Cursor::new(&concatenated));
    let mut out1 = vec![0u8; stream1_data.len()];
    let mut out2 = vec![0u8; stream2_data.len()];
    let mut out3 = vec![0u8; stream3_data.len()];

    reader.read_exact(&mut out1).expect("read stream 1");
    assert_eq!(&out1[..], stream1_data);

    reader.read_exact(&mut out2).expect("read stream 2");
    assert_eq!(&out2[..], stream2_data);

    reader.read_exact(&mut out3).expect("read stream 3");
    assert_eq!(&out3[..], stream3_data);

    let mut extra = [0u8; 1];
    assert_eq!(reader.read(&mut extra).expect("read at eof"), 0);
}

#[test]
fn test_lzfse_streaming_large_payload_1mb_pipeline() {
    // 1MB payload exceeding multiple 256KB block boundaries
    let size_1mb = 1024 * 1024; // 1,048,576 bytes
    let data = generate_deterministic_payload(size_1mb, 0xCAFE_BABE_1234_5678);

    let mut compressed = Vec::new();
    {
        let mut writer = LzfseWriter::new(&mut compressed);
        // Write in 64KB increments
        for chunk in data.chunks(64 * 1024) {
            writer.write_all(chunk).expect("write 64KB chunk");
        }
        writer.finish().expect("finish 1MB writer");
    }

    // Decompress via streaming reader
    let mut decompressed = Vec::with_capacity(size_1mb);
    let mut reader = LzfseReader::new(Cursor::new(&compressed));
    let mut read_buf = vec![0u8; 32 * 1024];

    loop {
        let n = reader.read(&mut read_buf).expect("read 32KB chunk");
        if n == 0 {
            break;
        }
        decompressed.extend_from_slice(&read_buf[..n]);
    }

    assert_eq!(decompressed.len(), size_1mb);
    assert_eq!(decompressed, data);
}

#[test]
fn test_lzfse_streaming_block_routing_matrix() {
    // 1. Very small (3 bytes) -> RawUncompressed block
    let small_data = b"ABC";
    let mut small_compressed = Vec::new();
    {
        let mut w = LzfseWriter::new(&mut small_compressed);
        w.write_all(small_data).expect("write 3b");
        w.finish().expect("finish 3b");
    }
    let magic = u32::from_le_bytes(small_compressed[0..4].try_into().unwrap());
    assert_eq!(magic, BvxMagic::RawUncompressed.as_u32());

    // 2. Small payload (1KB structured) -> LZVN block
    let lzvn_data = generate_structured_text(4); // ~900 bytes < 4096 bytes threshold
    let mut lzvn_compressed = Vec::new();
    {
        let mut w = LzfseWriter::with_lzvn_threshold(&mut lzvn_compressed, DEFAULT_LZVN_THRESHOLD);
        w.write_all(&lzvn_data).expect("write 1kb");
        w.finish().expect("finish 1kb");
    }
    let lzvn_magic = u32::from_le_bytes(lzvn_compressed[0..4].try_into().unwrap());
    assert_eq!(lzvn_magic, BvxMagic::CompressedLZVN.as_u32());

    let mut lzvn_decompressed = Vec::new();
    let mut r = LzfseReader::new(Cursor::new(&lzvn_compressed));
    r.read_to_end(&mut lzvn_decompressed).expect("read lzvn");
    assert_eq!(lzvn_decompressed, lzvn_data);

    // 3. Large payload (10KB structured) -> LZFSE V2 block
    let lzfse_data = generate_structured_text(50); // ~11KB > 4096 bytes threshold
    let mut lzfse_compressed = Vec::new();
    {
        let mut w = LzfseWriter::new(&mut lzfse_compressed);
        w.write_all(&lzfse_data).expect("write 10kb");
        w.finish().expect("finish 10kb");
    }
    let lzfse_magic = u32::from_le_bytes(lzfse_compressed[0..4].try_into().unwrap());
    assert_eq!(lzfse_magic, BvxMagic::CompressedV2.as_u32());

    let mut lzfse_decompressed = Vec::new();
    let mut r2 = LzfseReader::new(Cursor::new(&lzfse_compressed));
    r2.read_to_end(&mut lzfse_decompressed).expect("read lzfse");
    assert_eq!(lzfse_decompressed, lzfse_data);
}

#[test]
fn test_lzfse_streaming_c_reference_cross_compatibility() {
    let data = generate_structured_text(30); // ~6.8KB

    // Compress using Rust LzfseWriter
    let mut rust_compressed = Vec::new();
    {
        let mut w = LzfseWriter::new(&mut rust_compressed);
        w.write_all(&data).expect("write");
        w.finish().expect("finish");
    }

    // Verify native C reference decoder can decompress Rust-compressed container
    let c_decompressed = lzfse_decompress_to_vec(&rust_compressed, data.len())
        .expect("native C decompress Rust container");
    assert_eq!(c_decompressed, data);

    // Verify Rust LzfseReader can decompress native C-compressed stream
    let mut c_compressed = vec![0u8; data.len() + 4096];
    let written = ttzip_engine::codecs::lzfse::lzfse_compress(&data, &mut c_compressed)
        .expect("native C compress");
    c_compressed.truncate(written);

    let mut rust_reader_decompressed = Vec::new();
    let mut reader = LzfseReader::new(Cursor::new(&c_compressed));
    reader
        .read_to_end(&mut rust_reader_decompressed)
        .expect("read C container with Rust reader");
    assert_eq!(rust_reader_decompressed, data);
}

#[test]
fn test_lzfse_streaming_corrupted_stream_defense() {
    // 1. Truncated container (magic only, missing EOS or payload)
    let invalid_magic = b"bvxx";
    let mut reader = LzfseReader::new(Cursor::new(invalid_magic));
    let mut buf = [0u8; 16];
    assert!(reader.read(&mut buf).is_err());

    // 2. Corrupted block header
    let mut corrupted = Vec::new();
    corrupted.extend_from_slice(&BvxMagic::CompressedV2.as_bytes());
    corrupted.extend_from_slice(&[0xFF; 20]); // Invalid packed fields
    let mut reader2 = LzfseReader::new(Cursor::new(&corrupted));
    assert!(reader2.read(&mut buf).is_err());
}
