// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive test suite for `LibdeflateDecompressor` and SIMD Wild Copy Deflate engine.

use std::io::Write;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use ttzip_engine::codecs::libdeflate::{
    build_decode_table, deflate_decompress, FastBitWriterVec,
    LITLEN_ENOUGH, LITLEN_TABLEBITS, OFFSET_ENOUGH, OFFSET_TABLEBITS, PRECODE_ENOUGH,
    PRECODE_TABLEBITS,
};
use ttzip_engine::types::TTZipStatus;

// MARK: - Helper: Deflate Compression via flate2

fn compress_deflate(data: &[u8], compression: Compression) -> Vec<u8> {
    let mut encoder = DeflateEncoder::new(Vec::new(), compression);
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

// MARK: - 1. Uncompressed Block Tests

#[test]
fn test_uncompressed_block_exact() {
    let payload = b"Hello, TTZip Libdeflate Fast Uncompressed Stream!";
    let len = payload.len() as u16;
    let nlen = !len;

    let mut raw_deflate = Vec::new();
    // BFINAL=1, BTYPE=00 -> 0x01
    raw_deflate.push(0x01);
    raw_deflate.extend_from_slice(&len.to_le_bytes());
    raw_deflate.extend_from_slice(&nlen.to_le_bytes());
    raw_deflate.extend_from_slice(payload);

    let mut decompressed = vec![0u8; payload.len()];
    let written = deflate_decompress(&raw_deflate, &mut decompressed).unwrap();

    assert_eq!(written, payload.len());
    assert_eq!(&decompressed[..written], payload);
}

#[test]
fn test_uncompressed_block_multi_chunks() {
    let chunk1 = b"Chunk 1: 1234567890 ";
    let chunk2 = b"Chunk 2: ABCDEFGHIJKLMNOPQRSTUVWXYZ ";
    let chunk3 = b"Chunk 3: Complete!";

    let mut raw_deflate = Vec::new();
    // Block 1 (BFINAL=0, BTYPE=00 -> 0x00)
    let len1 = chunk1.len() as u16;
    raw_deflate.push(0x00);
    raw_deflate.extend_from_slice(&len1.to_le_bytes());
    raw_deflate.extend_from_slice(&(!len1).to_le_bytes());
    raw_deflate.extend_from_slice(chunk1);

    // Block 2 (BFINAL=0, BTYPE=00 -> 0x00)
    let len2 = chunk2.len() as u16;
    raw_deflate.push(0x00);
    raw_deflate.extend_from_slice(&len2.to_le_bytes());
    raw_deflate.extend_from_slice(&(!len2).to_le_bytes());
    raw_deflate.extend_from_slice(chunk2);

    // Block 3 (BFINAL=1, BTYPE=00 -> 0x01)
    let len3 = chunk3.len() as u16;
    raw_deflate.push(0x01);
    raw_deflate.extend_from_slice(&len3.to_le_bytes());
    raw_deflate.extend_from_slice(&(!len3).to_le_bytes());
    raw_deflate.extend_from_slice(chunk3);

    let expected = [chunk1.as_slice(), chunk2.as_slice(), chunk3.as_slice()].concat();
    let mut decompressed = vec![0u8; expected.len()];
    let written = deflate_decompress(&raw_deflate, &mut decompressed).unwrap();

    assert_eq!(written, expected.len());
    assert_eq!(&decompressed, &expected);
}

#[test]
fn test_uncompressed_block_nlen_mismatch_rejection() {
    let payload = b"Corrupted LEN/NLEN test";
    let len = payload.len() as u16;
    let bad_nlen = len; // Corrupt: nlen must be !len

    let mut raw_deflate = Vec::new();
    raw_deflate.push(0x01);
    raw_deflate.extend_from_slice(&len.to_le_bytes());
    raw_deflate.extend_from_slice(&bad_nlen.to_le_bytes());
    raw_deflate.extend_from_slice(payload);

    let mut decompressed = vec![0u8; payload.len()];
    let result = deflate_decompress(&raw_deflate, &mut decompressed);
    assert_eq!(result, Err(TTZipStatus::ErrCorruptHeader));
}

// MARK: - 2. Static Huffman Block Tests

#[test]
fn test_static_huffman_manual_emission() {
    let mut writer = FastBitWriterVec::new();
    // BFINAL=1 (1 bit), BTYPE=01 (2 bits, Static Huffman) -> 3 bits: 0b011 = 3
    writer.add_bits(3, 3);

    // In static Huffman code:
    // 'A' (65): 8 bits, code 0x30 + 65 - 0 = 0x71 = 0b01110001 -> reversed: 0b10001110
    // 'B' (66): 8 bits, code 0x30 + 66 - 0 = 0x72 = 0b01110010 -> reversed: 0b01001110
    // 'C' (67): 8 bits, code 0x30 + 67 - 0 = 0x73 = 0b01110011 -> reversed: 0b11001110
    // EOB (256): 7 bits, code 0x00 -> reversed: 0x00
    fn static_lit_codeword(lit: u8) -> (u32, u8) {
        let code = 0x30 + (lit as u32);
        let rev = code.reverse_bits() >> (32 - 8);
        (rev, 8)
    }

    for &b in b"ABCABCABC" {
        let (code, len) = static_lit_codeword(b);
        writer.add_bits(code as u64, len as u32);
        writer.flush_bits();
    }
    // Emit EOB (symbol 256, 7 bits, 0b0000000)
    writer.add_bits(0, 7);
    let stream = writer.finish();

    let mut dst = vec![0u8; 9];
    let written = deflate_decompress(&stream, &mut dst).unwrap();
    assert_eq!(written, 9);
    assert_eq!(&dst, b"ABCABCABC");
}

// MARK: - 3. Dynamic Huffman Block & Roundtrip Tests

#[test]
fn test_dynamic_huffman_roundtrip_all_levels() {
    let payload = b"The quick brown fox jumps over the lazy dog. \
                    Pack my box with five dozen liquor jugs. \
                    1234567890!@#$%^&*()_+~`|}{[]:;?><,./-=";

    for level in [Compression::none(), Compression::fast(), Compression::default(), Compression::best()] {
        let compressed = compress_deflate(payload, level);
        let mut decompressed = vec![0u8; payload.len()];
        let written = deflate_decompress(&compressed, &mut decompressed).unwrap();
        assert_eq!(written, payload.len());
        assert_eq!(&decompressed, payload);
    }
}

#[test]
fn test_dynamic_huffman_large_buffer_roundtrip() {
    let mut pattern = Vec::with_capacity(65536);
    for i in 0..65536 {
        pattern.push(((i * 37 + 13) % 251) as u8);
    }

    let compressed = compress_deflate(&pattern, Compression::default());
    let mut decompressed = vec![0u8; pattern.len()];
    let written = deflate_decompress(&compressed, &mut decompressed).unwrap();
    assert_eq!(written, pattern.len());
    assert_eq!(&decompressed, &pattern);
}

// MARK: - 4. 3-Tier SIMD Wild Copy Distance Tests

#[test]
fn test_simd_copy_distance_1_rle() {
    // 5000 identical bytes (tests D=1 SIMD 0x0101010101010101 broadcast)
    let payload = vec![b'Z'; 5000];
    let compressed = compress_deflate(&payload, Compression::best());

    let mut decompressed = vec![0u8; payload.len()];
    let written = deflate_decompress(&compressed, &mut decompressed).unwrap();
    assert_eq!(written, payload.len());
    assert_eq!(&decompressed, &payload);
}

#[test]
fn test_simd_copy_distance_2_alternating() {
    // "ABABABAB..." (tests D=2 step-slide expansion)
    let mut payload = Vec::new();
    for _ in 0..2500 {
        payload.extend_from_slice(b"AB");
    }
    let compressed = compress_deflate(&payload, Compression::best());

    let mut decompressed = vec![0u8; payload.len()];
    let written = deflate_decompress(&compressed, &mut decompressed).unwrap();
    assert_eq!(written, payload.len());
    assert_eq!(&decompressed, &payload);
}

#[test]
fn test_simd_copy_distance_3_triplet() {
    // "XYZXYZXYZ..." (tests D=3 step-slide expansion)
    let mut payload = Vec::new();
    for _ in 0..2000 {
        payload.extend_from_slice(b"XYZ");
    }
    let compressed = compress_deflate(&payload, Compression::best());

    let mut decompressed = vec![0u8; payload.len()];
    let written = deflate_decompress(&compressed, &mut decompressed).unwrap();
    assert_eq!(written, payload.len());
    assert_eq!(&decompressed, &payload);
}

#[test]
fn test_simd_copy_distance_7_step_slide() {
    // 7-byte repeating unit (tests D=7 step-slide expansion)
    let mut payload = Vec::new();
    for _ in 0..1000 {
        payload.extend_from_slice(b"1234567");
    }
    let compressed = compress_deflate(&payload, Compression::best());

    let mut decompressed = vec![0u8; payload.len()];
    let written = deflate_decompress(&compressed, &mut decompressed).unwrap();
    assert_eq!(written, payload.len());
    assert_eq!(&decompressed, &payload);
}

#[test]
fn test_simd_copy_distance_8_exact_boundary() {
    // 8-byte repeating unit (tests D=8 unrolled Wild Copy boundary)
    let mut payload = Vec::new();
    for _ in 0..1000 {
        payload.extend_from_slice(b"12345678");
    }
    let compressed = compress_deflate(&payload, Compression::best());

    let mut decompressed = vec![0u8; payload.len()];
    let written = deflate_decompress(&compressed, &mut decompressed).unwrap();
    assert_eq!(written, payload.len());
    assert_eq!(&decompressed, &payload);
}

#[test]
fn test_simd_copy_distance_100_long_backref() {
    // 100-byte repeating unit (tests D=100 long wild copy)
    let unit: Vec<u8> = (0..100).map(|i| (i % 256) as u8).collect();
    let mut payload = Vec::new();
    for _ in 0..100 {
        payload.extend_from_slice(&unit);
    }
    let compressed = compress_deflate(&payload, Compression::best());

    let mut decompressed = vec![0u8; payload.len()];
    let written = deflate_decompress(&compressed, &mut decompressed).unwrap();
    assert_eq!(written, payload.len());
    assert_eq!(&decompressed, &payload);
}

// MARK: - 5. Security, Bounds & Zero-Panic Invariants

#[test]
fn test_insufficient_output_buffer_returns_error() {
    let payload = b"Testing insufficient output buffer space error handling";
    let compressed = compress_deflate(payload, Compression::default());

    // Allocate smaller buffer
    let mut too_small = vec![0u8; payload.len() / 2];
    let result = deflate_decompress(&compressed, &mut too_small);
    assert_eq!(result, Err(TTZipStatus::ErrExtractionFailed));
}

#[test]
fn test_truncated_bitstream_returns_error() {
    let payload = b"Complete data stream payload for truncation test";
    let compressed = compress_deflate(payload, Compression::default());

    // Truncate compressed stream
    let truncated = &compressed[..compressed.len() / 2];
    let mut dst = vec![0u8; payload.len()];
    let result = deflate_decompress(truncated, &mut dst);
    assert_eq!(result, Err(TTZipStatus::ErrCorruptHeader));
}

#[test]
fn test_empty_input_returns_error() {
    let empty: &[u8] = &[];
    let mut dst = vec![0u8; 32];
    let result = deflate_decompress(empty, &mut dst);
    assert_eq!(result, Err(TTZipStatus::ErrCorruptHeader));
}

#[test]
fn test_random_fuzz_zero_panic_guarantee() {
    let mut seed = 0x12345678u64;
    let mut pseudo_rand = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 32) as u8
    };

    let mut fuzz_buf = vec![0u8; 512];
    let mut dst_buf = vec![0u8; 4096];

    for _ in 0..100 {
        for b in &mut fuzz_buf {
            *b = pseudo_rand();
        }
        let res = deflate_decompress(&fuzz_buf, &mut dst_buf);
        let _ = res; // Must not panic
    }
}

// MARK: - 6. Table Builder Unit Tests

#[test]
fn test_build_decode_table_constants_and_subtables() {
    let mut precode_table = [0u32; PRECODE_ENOUGH];
    let mut precode_lens = [0u8; 19];
    precode_lens[0] = 3;
    precode_lens[1] = 3;
    precode_lens[2] = 3;
    precode_lens[3] = 3;
    precode_lens[4] = 3;
    precode_lens[5] = 3;
    precode_lens[6] = 3;
    precode_lens[7] = 3;

    let res = build_decode_table(&precode_lens, 19, PRECODE_TABLEBITS, 7, &mut precode_table);
    assert!(res.is_ok());

    let mut litlen_table = [0u32; LITLEN_ENOUGH];
    let mut litlen_lens = [0u8; 288];
    for i in 0..144 { litlen_lens[i] = 8; }
    for i in 144..256 { litlen_lens[i] = 9; }
    for i in 256..280 { litlen_lens[i] = 7; }
    for i in 280..288 { litlen_lens[i] = 8; }
    let res = build_decode_table(&litlen_lens, 288, LITLEN_TABLEBITS, 15, &mut litlen_table);
    assert!(res.is_ok());

    let mut offset_table = [0u32; OFFSET_ENOUGH];
    let mut offset_lens = [0u8; 32];
    for i in 0..32 {
        offset_lens[i] = 5;
    }
    let res = build_decode_table(&offset_lens, 32, OFFSET_TABLEBITS, 15, &mut offset_table);
    assert!(res.is_ok());
}
