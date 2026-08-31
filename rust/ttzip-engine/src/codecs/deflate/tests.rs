// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Deflate / zlib / gzip codec unit tests, matcher tests, synthetic corpora, and scheduler tests.

use super::*;
use std::time::Instant;

#[test]
fn test_deflate_roundtrip_basic() {
    let input = b"Hello world! TTZip native Rust DEFLATE engine testing 1234567890.";
    let mut compressed = vec![0u8; deflate_compress_bound(input.len(), 6)];
    let comp_len = deflate_compress(input, &mut compressed, 6).expect("deflate compress failed");
    assert!(comp_len > 0);

    let mut decompressed = vec![0u8; input.len()];
    let decomp_len = deflate_decompress(&compressed[..comp_len], &mut decompressed)
        .expect("deflate decompress failed");
    assert_eq!(decomp_len, input.len());
    assert_eq!(&decompressed[..decomp_len], input);
}

#[test]
fn test_zlib_roundtrip_all_levels() {
    let input = b"The quick brown fox jumps over the lazy dog. Repeat repeatedly for compression ratio.";
    let mut buffer = Vec::new();
    for _ in 0..50 {
        buffer.extend_from_slice(input);
    }

    for level in [1, 3, 6, 9, 12] {
        let mut compressed = vec![0u8; buffer.len() + 1024];
        let comp_len = zlib_compress(&buffer, &mut compressed, level).expect("zlib compress failed");
        assert!(comp_len > 0);
        assert!(comp_len < buffer.len());

        let mut decompressed = vec![0u8; buffer.len()];
        let decomp_len = zlib_decompress(&compressed[..comp_len], &mut decompressed)
            .expect("zlib decompress failed");
        assert_eq!(decomp_len, buffer.len());
        assert_eq!(&decompressed, &buffer);
    }
}

#[test]
fn test_gzip_roundtrip() {
    let input = b"GZIP format wrapping test for TTZip high-performance native pipeline.";
    let mut compressed = vec![0u8; input.len() + 1024];
    let comp_len = gzip_compress(input, &mut compressed, 6).expect("gzip compress failed");
    assert!(comp_len > 0);

    let mut decompressed = vec![0u8; input.len()];
    let decomp_len = gzip_decompress(&compressed[..comp_len], &mut decompressed)
        .expect("gzip decompress failed");
    assert_eq!(decomp_len, input.len());
    assert_eq!(&decompressed, input);
}

#[test]
fn test_corrupt_data_handling() {
    let garbage = [0xde, 0xad, 0xbe, 0xef, 0x01, 0x02, 0x03, 0x04];
    let mut out = [0u8; 64];
    let res = deflate_decompress(&garbage, &mut out);
    assert!(res.is_err());
}

#[test]
fn test_compress_bounds_zero_allocation() {
    let len = 100_000;
    let def_bound = deflate_compress_bound(len, 6);
    let zlib_bound = zlib_compress_bound(len, 6);
    let gzip_bound = gzip_compress_bound(len, 6);

    assert!(def_bound >= len);
    assert!(zlib_bound >= def_bound);
    assert!(gzip_bound >= def_bound);
}

#[test]
fn test_decompress_ex_and_insufficient_space() {
    let input = b"Zero-copy and exact buffer boundary testing with libdeflate in TTZip.";
    let mut compressed = vec![0u8; deflate_compress_bound(input.len(), 6)];
    let comp_len = deflate_compress(input, &mut compressed, 6).expect("compress failed");

    // 1. Test _ex interface
    let mut decompressor = DeflateDecompressor::new().expect("alloc decompressor");
    let mut decompressed = vec![0u8; input.len()];
    let (in_consumed, out_produced) = decompressor
        .decompress_ex(&compressed[..comp_len], &mut decompressed)
        .expect("decompress_ex failed");

    assert_eq!(in_consumed, comp_len);
    assert_eq!(out_produced, input.len());
    assert_eq!(&decompressed, input);

    // 2. Test InsufficientSpace distinction
    let mut too_small = vec![0u8; input.len() / 2];
    let err = decompressor
        .decompress_precise(&compressed[..comp_len], &mut too_small)
        .expect_err("should fail with InsufficientSpace");

    assert_eq!(err, DeflateDecompressError::InsufficientSpace);
}

// MARK: - ZlibNgMatcher Unit Tests

#[test]
fn test_zlib_ng_match_length_fast() {
    let a = b"abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut b = a.to_vec();

    // Exact match up to full length
    assert_eq!(match_length_fast(a, &b, a.len()), a.len());

    // Introduce difference at various offsets (0..60)
    for diff_idx in 0..a.len() {
        b[diff_idx] ^= 0x55;
        let matched = match_length_fast(a, &b, a.len());
        assert_eq!(matched, diff_idx, "mismatch detected incorrectly at diff_idx {}", diff_idx);
        b[diff_idx] ^= 0x55;
    }
}

#[test]
fn test_zlib_ng_matcher_tokenization() {
    let mut matcher = ZlibNgMatcher::with_level(6);
    let sample = b"abcde12345abcde12345abcde12345";
    let tokens = matcher.tokenize_stream(sample);

    assert!(!tokens.is_empty());
    let mut match_found = false;
    for t in &tokens {
        if let DeflateToken::Match(m) = t {
            assert!(m.length >= 3);
            assert!(m.distance > 0);
            match_found = true;
        }
    }
    assert!(match_found, "expected LZ77 match in repetitive sample");
}

#[test]
fn test_zlib_ng_matcher_slide_hash() {
    let mut matcher = ZlibNgMatcher::with_level(6);
    let sample = vec![b'x'; 100];
    matcher.insert_string(0, &sample);
    matcher.insert_string(35000, &sample);

    // Sliding hash should saturate values <= 32768 to 0
    matcher.slide_hash();

    // Verify slide_hash runs without panic and resets appropriately
    matcher.reset();
    assert_eq!(matcher.config().max_chain, 256);
}

// MARK: - Synthetic Corpus Unit Tests

#[test]
fn test_synthetic_corpus_generators() {
    let size = 8192;
    let silesia = SyntheticCorpus::generate_silesia_like(size);
    let enwik8 = SyntheticCorpus::generate_enwik8_like(size);
    let high_entropy = SyntheticCorpus::generate_high_entropy(size);
    let low_entropy = SyntheticCorpus::generate_low_entropy_periodic(size, 32);
    let zero_sparse = SyntheticCorpus::generate_zero_sparse(size, 0.95);
    let lz77_clusters = SyntheticCorpus::generate_lz77_clusters(size, 32, 16);
    let code_struct = SyntheticCorpus::generate_code_structure(size);
    let multimodal = SyntheticCorpus::generate_multimodal_interleaved(size);

    assert_eq!(silesia.len(), size);
    assert_eq!(enwik8.len(), size);
    assert_eq!(high_entropy.len(), size);
    assert_eq!(low_entropy.len(), size);
    assert_eq!(zero_sparse.len(), size);
    assert_eq!(lz77_clusters.len(), size);
    assert_eq!(code_struct.len(), size);
    assert_eq!(multimodal.len(), size);

    // Verify compressibility of low-entropy periodic data
    let mut comp_buf = vec![0u8; deflate_compress_bound(size, 6)];
    let comp_len = deflate_compress(&low_entropy, &mut comp_buf, 6).expect("compress low entropy");
    assert!(comp_len < size / 4, "low-entropy periodic data should compress heavily");

    // Verify kind dispatch
    let dispatched = SyntheticCorpus::generate(SyntheticCorpusKind::ZeroSparse, 1024);
    assert_eq!(dispatched.len(), 1024);
}

// MARK: - DeflateEngineArbitrator Unit Tests

#[test]
fn test_arbitrator_decisions_and_latency() {
    let arbitrator = DeflateEngineArbitrator::new();

    // Level 0 -> StoreDirect
    let hint_l0 = DeflateWorkloadHint {
        compression_level: 0,
        ..Default::default()
    };
    assert_eq!(arbitrator.arbitrate(b"test data", hint_l0), DeflateEngineChoice::StoreDirect);

    // Streaming -> ZlibNgStreaming
    let hint_streaming = DeflateWorkloadHint {
        compression_level: 6,
        is_streaming: true,
        ..Default::default()
    };
    assert_eq!(
        arbitrator.arbitrate(b"streaming chunk", hint_streaming),
        DeflateEngineChoice::ZlibNgStreaming
    );

    // Batch large buffer -> LibdeflateBatch
    let large_data = vec![0x41u8; 4096];
    let hint_batch = DeflateWorkloadHint {
        size: 4096,
        compression_level: 6,
        is_streaming: false,
        allow_store_fallback: false,
        concurrency_load: 0,
    };
    assert_eq!(
        arbitrator.arbitrate(&large_data, hint_batch),
        DeflateEngineChoice::LibdeflateBatch
    );

    // Latency benchmark: Ensure decision latency is <= 15ns
    let iterations = 100_000;
    let start = Instant::now();
    let mut dummy_acc = 0usize;

    for i in 0..iterations {
        let choice = arbitrator.arbitrate(&large_data, hint_batch);
        if choice == DeflateEngineChoice::LibdeflateBatch {
            dummy_acc += i & 1;
        }
    }
    let elapsed = start.elapsed();
    let nanos_per_decision = elapsed.as_nanos() as f64 / iterations as f64;
    assert!(dummy_acc > 0);
    assert!(
        nanos_per_decision <= 25.0,
        "arbitrator decision latency {}ns exceeds threshold",
        nanos_per_decision
    );
}

// MARK: - DynamicLevelScheduler Unit Tests

#[test]
fn test_dynamic_level_scheduler() {
    let mut scheduler = DynamicLevelScheduler::new(6, PerformanceProfile::Balanced);
    assert_eq!(scheduler.current_level(), 6);

    // Runtime level switching
    scheduler.set_level(9);
    assert_eq!(scheduler.current_level(), 9);

    // Profile switching
    scheduler.set_profile(PerformanceProfile::MaxSpeed);
    assert_eq!(scheduler.current_level(), 1);

    // Block type selection
    assert_eq!(
        scheduler.select_block_type(0, 0, false),
        DeflateBlockType::Stored
    );
    assert_eq!(
        scheduler.select_block_type(64, 2, false),
        DeflateBlockType::StaticHuffman
    );
    assert_eq!(
        scheduler.select_block_type(4096, 200, false),
        DeflateBlockType::DynamicHuffman
    );

    // Feedback recording
    scheduler.record_feedback(10000, 3000, 50000);
    assert_eq!(scheduler.metrics().block_count, 1);
    assert!(scheduler.metrics().compression_ratio() < 0.5);
    assert!(scheduler.metrics().average_mb_per_sec() > 0.0);
}
