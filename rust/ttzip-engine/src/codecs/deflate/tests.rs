// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Deflate / zlib / gzip codec unit tests.

use super::*;

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

