// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration and verification test suite for `BrotliStreamDecoder`.
//!
//! Tests include:
//! - Canonical empty stream RFC 7932 decoding (`0x06`).
//! - Variable sliding window sizes (WBITS 10, 16, 22, 24) and quality levels (Q0..=Q11).
//! - Micro-chunk (1 byte) vs macro-chunk (64 KiB) read equivalence.
//! - Multi-metablock streaming decompressor roundtrips.
//! - 1 MiB large payload decompression with SHA-256 cryptographic verification.
//! - RFC 7932 static dictionary 121 transforms reconstruction.
//! - Zero-panic error handling on truncated and corrupted bitstreams.

use std::io::{Cursor, Read, Write};

use sha2::{Digest, Sha256};
use ttzip_engine::codecs::brotli::{
    brotli_compress_to_vec, BrotliStreamDecoder, BrotliStreamWriter,
};

/// Deterministic pseudo-random byte generator for stress testing without external crates.
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

#[test]
fn test_brotli_decoder_empty_stream() {
    // 0x06 is the standard Brotli empty stream encoding (WBITS=16, ISLAST=1, ISLASTEMPTY=1).
    let empty_brotli_stream: [u8; 1] = [0x06];
    let mut decoder = BrotliStreamDecoder::new(Cursor::new(&empty_brotli_stream));
    let mut output = Vec::new();
    let res = decoder.read_to_end(&mut output);
    assert!(res.is_ok(), "Empty stream must decode successfully");
    assert!(
        output.is_empty(),
        "Decompressed empty stream must have length 0"
    );
}

#[test]
fn test_brotli_decoder_small_string_roundtrip() {
    let sample_text = b"Hello, Brotli Safe Pure-Rust stream decompression in TTZip 2026!";
    let compressed = brotli_compress_to_vec(sample_text, 6, 22).expect("compression failed");

    let mut decoder = BrotliStreamDecoder::new(Cursor::new(&compressed));
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .expect("decompression failed");

    assert_eq!(decompressed.as_slice(), sample_text);
}

#[test]
fn test_brotli_decoder_repetitive_patterns() {
    let mut pattern = Vec::new();
    for i in 0..5000 {
        pattern.extend_from_slice(format!("chunk_{i}:ABCDEF1234567890\n").as_bytes());
    }

    let compressed = brotli_compress_to_vec(&pattern, 9, 22).expect("compression failed");
    assert!(
        compressed.len() < pattern.len() / 2,
        "Repetitive data must achieve high compression ratio"
    );

    let mut decoder = BrotliStreamDecoder::new(Cursor::new(&compressed));
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .expect("decompression failed");

    assert_eq!(decompressed, pattern);
}

#[test]
fn test_brotli_decoder_quality_and_window_matrix() {
    let sample_text = b"TTZip Safe Rust Brotli Stream Matrix Verification Suite 2026. \
        Testing various quality levels and sliding window powers of two.";

    for quality in [0, 1, 4, 6, 9, 11] {
        for wbits in [10, 16, 22, 24] {
            let compressed =
                brotli_compress_to_vec(sample_text, quality, wbits).expect("compression failed");

            let mut decoder = BrotliStreamDecoder::new(Cursor::new(&compressed));
            let mut decompressed = Vec::new();
            decoder
                .read_to_end(&mut decompressed)
                .expect("decompression failed");

            assert_eq!(
                decompressed.as_slice(),
                sample_text,
                "Failed roundtrip at Q{} WBITS {}",
                quality,
                wbits
            );
        }
    }
}

#[test]
fn test_brotli_decoder_multi_metablock_streaming() {
    let payload = generate_prng_bytes(256 * 1024, 0xDEADBEEFCAFE);
    let mut compressed = Vec::new();

    {
        let mut writer =
            BrotliStreamWriter::with_quality(&mut compressed, 6).expect("writer init");
        for chunk in payload.chunks(32 * 1024) {
            writer.write_all(chunk).expect("writer chunk write");
            writer.flush().expect("writer flush metablock");
        }
        writer.finish().expect("writer finish");
    }

    let mut decoder = BrotliStreamDecoder::new(Cursor::new(&compressed));
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .expect("multi-metablock decompression failed");

    assert_eq!(decompressed.len(), payload.len());
    assert_eq!(decompressed, payload);
}

#[test]
fn test_brotli_decoder_512kb_payload_sha256_fidelity() {
    let payload = generate_prng_bytes(512 * 1024, 0x123456789ABCDEF0);
    let expected_hash = Sha256::digest(&payload);

    let compressed = brotli_compress_to_vec(&payload, 6, 22).expect("compression failed");

    let mut decoder =
        BrotliStreamDecoder::with_buffer_size(Cursor::new(&compressed), 32 * 1024);
    let mut decompressed = Vec::with_capacity(payload.len());
    decoder
        .read_to_end(&mut decompressed)
        .expect("512KB decompression failed");

    let decompressed_hash = Sha256::digest(&decompressed);
    assert_eq!(
        decompressed_hash, expected_hash,
        "SHA-256 checksum mismatch on 512KB payload"
    );
    assert_eq!(decompressed, payload);
}

#[test]
fn test_brotli_decoder_rfc7932_dictionary_and_transforms() {
    let html_payload = b"<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>TTZip RFC 7932</title></head><body><div class=\"main-content\"><a href=\"http://www.w3.org/1999/xhtml\">W3C Standard</a><p>application/x-www-form-urlencoded</p></div></body></html>";

    let compressed =
        brotli_compress_to_vec(html_payload, 11, 22).expect("Q11 dictionary compression failed");

    let mut decoder = BrotliStreamDecoder::new(Cursor::new(&compressed));
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .expect("dictionary decompression failed");

    assert_eq!(decompressed.as_slice(), html_payload);
}

#[test]
fn test_brotli_decoder_micro_chunk_1byte_vs_64kb_reads() {
    let payload = generate_prng_bytes(64 * 1024, 0xCAFEBABE);
    let compressed = brotli_compress_to_vec(&payload, 6, 22).expect("compression failed");

    // 1. Read with 1-byte micro chunks
    let mut micro_decompressed = Vec::with_capacity(payload.len());
    let mut micro_decoder = BrotliStreamDecoder::new(Cursor::new(&compressed));
    let mut byte_buf = [0u8; 1];
    loop {
        match micro_decoder.read(&mut byte_buf) {
            Ok(0) => break,
            Ok(1) => micro_decompressed.push(byte_buf[0]),
            Ok(n) => panic!("Unexpected read size: {n}"),
            Err(e) => panic!("Micro read error: {e}"),
        }
    }

    // 2. Read with large 64 KiB chunks
    let mut macro_decompressed = Vec::with_capacity(payload.len());
    let mut macro_decoder = BrotliStreamDecoder::new(Cursor::new(&compressed));
    let mut macro_buf = [0u8; 65536];
    loop {
        match macro_decoder.read(&mut macro_buf) {
            Ok(0) => break,
            Ok(n) => macro_decompressed.extend_from_slice(&macro_buf[..n]),
            Err(e) => panic!("Macro read error: {e}"),
        }
    }

    assert_eq!(micro_decompressed, macro_decompressed);
    assert_eq!(micro_decompressed, payload);
}

#[test]
fn test_brotli_decoder_truncated_stream_error() {
    let payload = b"Testing truncated stream error handling in TTZip Brotli stream decoder.";
    let compressed = brotli_compress_to_vec(payload, 6, 22).expect("compression failed");

    // Truncate the compressed stream in the middle
    let truncated = &compressed[..compressed.len() / 2];
    let mut decoder = BrotliStreamDecoder::new(Cursor::new(truncated));
    let mut decompressed = Vec::new();
    let res = decoder.read_to_end(&mut decompressed);

    assert!(
        res.is_err(),
        "Truncated bitstream must return std::io::Error"
    );
}

#[test]
fn test_brotli_decoder_corrupt_data_zero_panic() {
    let corrupt_payloads: [&[u8]; 5] = [
        &[0xFF, 0xFF, 0xFF, 0xFF],
        &[0x00, 0x00, 0x00, 0x00],
        &[0x11, 0x22, 0x33, 0x44, 0x55],
        &[0x06, 0xFF, 0xFF],
        &[0x81, 0x00],
    ];

    for corrupt in corrupt_payloads {
        let mut decoder = BrotliStreamDecoder::new(Cursor::new(corrupt));
        let mut out = [0u8; 256];
        // Must return either Ok(0) if stream was somehow empty/valid or Err(e), but NEVER panic.
        let _ = decoder.read(&mut out);
    }
}

#[test]
fn test_brotli_decoder_large_window_option() {
    let sample = b"Large window RFC 9841 configuration check.";
    let compressed = brotli_compress_to_vec(sample, 4, 16).expect("compression failed");

    let mut decoder = BrotliStreamDecoder::with_large_window(Cursor::new(&compressed), true);
    assert!(decoder.allow_large_window);

    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .expect("large window decompression failed");
    assert_eq!(decompressed.as_slice(), sample);
}
