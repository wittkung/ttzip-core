// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive RFC 7932 Brotli Compliance Test Matrix & Official Testdata Suite.
//!
//! Validates:
//! 1. 19 official `empty.compressed.*` edge-case streams (WBITS 10..=24, metadata blocks, padding).
//! 2. `x.compressed.*` single-byte family (Simple Huffman, Multi-Symbol, Uncompressed, Complex).
//! 3. Classic long-distance and repeated corpora (`zeros`, `zerosukkanooa`, `backward65536`).
//! 4. Full official Google Brotli test corpus bit-exact decompressed fidelity.
//! 5. Streaming reader chunking and file-level decompression pipelines.

use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use ttzip_engine::codecs::brotli::{
    brotli_decompress, brotli_decompress_file, brotli_decompress_stream_pipe,
    brotli_decompress_to_vec, BrotliDecompressorReader,
};
use ttzip_engine::crypto::HardwareSha256;

/// Resolves path to fixture in vendor testdata repository.
fn testdata_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../vendor/brotli/tests/testdata")
        .join(name)
}

/// Reads raw bytes of a fixture file from disk.
fn read_testdata(name: &str) -> Vec<u8> {
    let p = testdata_path(name);
    fs::read(&p).unwrap_or_else(|e| panic!("Failed to read test fixture {:?}: {:?}", p, e))
}

// ============================================================================
// 1. 19 Official `empty.compressed.*` Variants Matrix
// ============================================================================

#[test]
fn test_empty_compressed_base_and_00() {
    // 1-byte 0x06 stream: WBITS 16, ISLAST 1, ISLASTEMPTY 1
    for fixture in ["empty.compressed", "empty.compressed.00"] {
        let compressed = read_testdata(fixture);
        assert_eq!(compressed.len(), 1, "Fixture {} must be 1 byte", fixture);
        assert_eq!(compressed[0], 0x06, "Fixture {} must be 0x06", fixture);

        let mut out = [0u8; 64];
        let written = brotli_decompress(&compressed, &mut out)
            .unwrap_or_else(|e| panic!("Decompress failed for {}: {:?}", fixture, e));
        assert_eq!(written, 0, "Decompressed length must be 0 for {}", fixture);

        let vec_out = brotli_decompress_to_vec(&compressed, 1024)
            .unwrap_or_else(|e| panic!("Decompress to vec failed for {}: {:?}", fixture, e));
        assert!(vec_out.is_empty(), "Vec output must be empty for {}", fixture);
    }
}

#[test]
fn test_empty_compressed_01_to_07_all_window_sizes() {
    // 2-byte empty streams testing sliding window bits (WBITS 10..=24)
    let variants = [
        "empty.compressed.01",
        "empty.compressed.02",
        "empty.compressed.03",
        "empty.compressed.04",
        "empty.compressed.05",
        "empty.compressed.06",
        "empty.compressed.07",
    ];

    for name in variants {
        let compressed = read_testdata(name);
        assert_eq!(compressed.len(), 2, "Fixture {} must be 2 bytes", name);

        let mut out = [0u8; 64];
        let written = brotli_decompress(&compressed, &mut out)
            .unwrap_or_else(|e| panic!("Decompress failed for {}: {:?}", name, e));
        assert_eq!(written, 0, "Decompressed length must be 0 for {}", name);

        let vec_out = brotli_decompress_to_vec(&compressed, 1024)
            .unwrap_or_else(|e| panic!("Decompress to vec failed for {}: {:?}", name, e));
        assert!(vec_out.is_empty(), "Vec output must be empty for {}", name);
    }
}

#[test]
fn test_empty_compressed_08_to_15_metadata_blocks() {
    // 1-byte empty streams containing metadata and skip blocks
    let variants = [
        "empty.compressed.08",
        "empty.compressed.09",
        "empty.compressed.10",
        "empty.compressed.11",
        "empty.compressed.12",
        "empty.compressed.13",
        "empty.compressed.14",
        "empty.compressed.15",
    ];

    for name in variants {
        let compressed = read_testdata(name);
        assert_eq!(compressed.len(), 1, "Fixture {} must be 1 byte", name);

        let mut out = [0u8; 64];
        let written = brotli_decompress(&compressed, &mut out)
            .unwrap_or_else(|e| panic!("Decompress failed for {}: {:?}", name, e));
        assert_eq!(written, 0, "Decompressed length must be 0 for {}", name);

        let vec_out = brotli_decompress_to_vec(&compressed, 1024)
            .unwrap_or_else(|e| panic!("Decompress to vec failed for {}: {:?}", name, e));
        assert!(vec_out.is_empty(), "Vec output must be empty for {}", name);
    }
}

#[test]
fn test_empty_compressed_16_uncompressed_padded_empty() {
    // 4-byte stream: uncompressed empty block with byte padding
    let compressed = read_testdata("empty.compressed.16");
    assert_eq!(compressed.len(), 4, "empty.compressed.16 must be 4 bytes");

    let mut out = [0u8; 64];
    let written = brotli_decompress(&compressed, &mut out).expect("decompress empty.compressed.16");
    assert_eq!(written, 0);

    let vec_out = brotli_decompress_to_vec(&compressed, 1024).expect("to vec empty.compressed.16");
    assert!(vec_out.is_empty());
}

#[test]
fn test_empty_compressed_17_long_metadata_sequence() {
    // 65538-byte stream: long sequence of empty metadata blocks
    let compressed = read_testdata("empty.compressed.17");
    assert_eq!(compressed.len(), 65538);

    let vec_out = brotli_decompress_to_vec(&compressed, 1024).expect("decompress empty.compressed.17");
    assert!(vec_out.is_empty());

    let mut cursor = Cursor::new(&compressed);
    let mut pipe_out = Vec::new();
    let (read_bytes, written_bytes) = brotli_decompress_stream_pipe(&mut cursor, &mut pipe_out, None)
        .expect("pipe decompress empty.compressed.17");
    assert_eq!(read_bytes, 65538);
    assert_eq!(written_bytes, 0);
    assert!(pipe_out.is_empty());
}

#[test]
fn test_empty_compressed_18_ultra_long_stream() {
    // 196610-byte stream: zero-running ultra long metadata stream
    let compressed = read_testdata("empty.compressed.18");
    assert_eq!(compressed.len(), 196610);

    let vec_out = brotli_decompress_to_vec(&compressed, 1024).expect("decompress empty.compressed.18");
    assert!(vec_out.is_empty());

    let mut cursor = Cursor::new(&compressed);
    let mut pipe_out = Vec::new();
    let (read_bytes, written_bytes) = brotli_decompress_stream_pipe(&mut cursor, &mut pipe_out, None)
        .expect("pipe decompress empty.compressed.18");
    assert_eq!(read_bytes, 196610);
    assert_eq!(written_bytes, 0);
    assert!(pipe_out.is_empty());
}

#[test]
fn test_empty_compressed_all_variants_exhaustive_decompress() {
    let all_empty_variants = [
        "empty.compressed",
        "empty.compressed.00",
        "empty.compressed.01",
        "empty.compressed.02",
        "empty.compressed.03",
        "empty.compressed.04",
        "empty.compressed.05",
        "empty.compressed.06",
        "empty.compressed.07",
        "empty.compressed.08",
        "empty.compressed.09",
        "empty.compressed.10",
        "empty.compressed.11",
        "empty.compressed.12",
        "empty.compressed.13",
        "empty.compressed.14",
        "empty.compressed.15",
        "empty.compressed.16",
        "empty.compressed.17",
        "empty.compressed.18",
    ];

    for name in all_empty_variants {
        let compressed = read_testdata(name);
        let decomp = brotli_decompress_to_vec(&compressed, 4096)
            .unwrap_or_else(|e| panic!("Decompress failed for exhaustive {}: {:?}", name, e));
        assert!(
            decomp.is_empty(),
            "Exhaustive variant {} must decompress to empty slice",
            name
        );
    }
}

// ============================================================================
// 2. `x.compressed.*` Single-Byte Family Tests (00..=03)
// ============================================================================

#[test]
fn test_x_compressed_base() {
    let compressed = read_testdata("x.compressed");
    let expected = read_testdata("x");
    let decomp = brotli_decompress_to_vec(&compressed, 64).expect("decompress x.compressed");
    assert_eq!(decomp.as_slice(), expected.as_slice());
    assert_eq!(decomp.as_slice(), b"X");
}

#[test]
fn test_x_compressed_00_single_symbol_simple_huffman() {
    let compressed = read_testdata("x.compressed.00");
    let expected = read_testdata("x");
    let decomp = brotli_decompress_to_vec(&compressed, 64).expect("decompress x.compressed.00");
    assert_eq!(decomp.as_slice(), expected.as_slice());
    assert_eq!(decomp.as_slice(), b"X");
}

#[test]
fn test_x_compressed_01_multi_symbol_simple_huffman() {
    let compressed = read_testdata("x.compressed.01");
    let expected = read_testdata("x");
    let decomp = brotli_decompress_to_vec(&compressed, 64).expect("decompress x.compressed.01");
    assert_eq!(decomp.as_slice(), expected.as_slice());
    assert_eq!(decomp.as_slice(), b"X");
}

#[test]
fn test_x_compressed_02_uncompressed_meta_block() {
    let compressed = read_testdata("x.compressed.02");
    let expected = read_testdata("x");
    let decomp = brotli_decompress_to_vec(&compressed, 64).expect("decompress x.compressed.02");
    assert_eq!(decomp.as_slice(), expected.as_slice());
    assert_eq!(decomp.as_slice(), b"X");
}

#[test]
fn test_x_compressed_03_complex_huffman() {
    let compressed = read_testdata("x.compressed.03");
    let expected = read_testdata("x");
    let decomp = brotli_decompress_to_vec(&compressed, 64).expect("decompress x.compressed.03");
    assert_eq!(decomp.as_slice(), expected.as_slice());
    assert_eq!(decomp.as_slice(), b"X");
}

#[test]
fn test_x_compressed_all_variants_stream_pipe() {
    let x_variants = [
        "x.compressed",
        "x.compressed.00",
        "x.compressed.01",
        "x.compressed.02",
        "x.compressed.03",
    ];
    let expected = read_testdata("x");

    for name in x_variants {
        let compressed = read_testdata(name);
        let mut cursor = Cursor::new(&compressed);
        let mut out = Vec::new();
        let (read_bytes, written_bytes) = brotli_decompress_stream_pipe(&mut cursor, &mut out, None)
            .unwrap_or_else(|e| panic!("Pipe decompress failed for {}: {:?}", name, e));

        assert_eq!(read_bytes, compressed.len() as u64);
        assert_eq!(written_bytes, 1);
        assert_eq!(out.as_slice(), expected.as_slice());
        assert_eq!(out.as_slice(), b"X");
    }
}

// ============================================================================
// 3. Classic Long-Distance & Repetitive Corpora Tests
// ============================================================================

#[test]
fn test_corpus_zeros() {
    let compressed = read_testdata("zeros.compressed");
    let expected_raw = read_testdata("zeros");
    assert_eq!(expected_raw.len(), 262144); // 256 KiB
    assert!(expected_raw.iter().all(|&b| b == 0));

    let decomp = brotli_decompress_to_vec(&compressed, 1024 * 1024).expect("decompress zeros");
    assert_eq!(decomp.len(), expected_raw.len());
    assert_eq!(decomp, expected_raw);

    let expected_sha = HardwareSha256::digest(&expected_raw);
    let actual_sha = HardwareSha256::digest(&decomp);
    assert_eq!(actual_sha, expected_sha);
}

#[test]
fn test_corpus_zerosukkanooa() {
    let compressed = read_testdata("zerosukkanooa.compressed");
    let expected_raw = read_testdata("zerosukkanooa");
    assert_eq!(expected_raw.len(), 262263);

    let decomp = brotli_decompress_to_vec(&compressed, 1024 * 1024).expect("decompress zerosukkanooa");
    assert_eq!(decomp.len(), expected_raw.len());
    assert_eq!(decomp, expected_raw);

    let expected_sha = HardwareSha256::digest(&expected_raw);
    let actual_sha = HardwareSha256::digest(&decomp);
    assert_eq!(actual_sha, expected_sha);
}

#[test]
fn test_corpus_backward65536() {
    let compressed = read_testdata("backward65536.compressed");
    let expected_raw = read_testdata("backward65536");
    assert_eq!(expected_raw.len(), 65792);

    let decomp = brotli_decompress_to_vec(&compressed, 1024 * 1024).expect("decompress backward65536");
    assert_eq!(decomp.len(), expected_raw.len());
    assert_eq!(decomp, expected_raw);

    let expected_sha = HardwareSha256::digest(&expected_raw);
    let actual_sha = HardwareSha256::digest(&decomp);
    assert_eq!(actual_sha, expected_sha);
}

// ============================================================================
// 4. Full Official Testdata Corpus Bit-Exact Verification Suite
// ============================================================================

#[test]
fn test_official_corpus_full_matrix_fidelity() {
    let corpora = [
        "10x10y",
        "64x",
        "alice29.txt",
        "asyoulik.txt",
        "compressed_file",
        "compressed_repeated",
        "cp1251-utf16le",
        "cp852-utf8",
        "lcet10.txt",
        "mapsdatazrh",
        "monkey",
        "plrabn12.txt",
        "quickfox",
        "quickfox_repeated",
        "random_org_10k.bin",
        "ukkonooa",
        "xyzzy",
    ];

    for name in corpora {
        let compressed_name = format!("{}.compressed", name);
        let compressed = read_testdata(&compressed_name);
        let expected_raw = read_testdata(name);

        let decomp = brotli_decompress_to_vec(&compressed, expected_raw.len() + 65536)
            .unwrap_or_else(|e| panic!("Decompress failed for corpus {}: {:?}", name, e));

        assert_eq!(
            decomp.len(),
            expected_raw.len(),
            "Length mismatch for corpus {}",
            name
        );
        assert_eq!(
            decomp, expected_raw,
            "Byte mismatch for corpus {}",
            name
        );

        let expected_sha = HardwareSha256::digest(&expected_raw);
        let actual_sha = HardwareSha256::digest(&decomp);
        assert_eq!(
            actual_sha, expected_sha,
            "SHA-256 mismatch for corpus {}",
            name
        );
    }
}

// ============================================================================
// 5. Streaming Reader & File Pipeline Verification
// ============================================================================

#[test]
fn test_streaming_reader_incremental_chunks() {
    let compressed = read_testdata("alice29.txt.compressed");
    let expected_raw = read_testdata("alice29.txt");

    for chunk_size in [1, 7, 13, 1024, 65536] {
        let mut reader = BrotliDecompressorReader::new(Cursor::new(&compressed), 4096);
        let mut result = Vec::new();
        let mut buf = vec![0u8; chunk_size];

        loop {
            use std::io::Read;
            let n = reader.read(&mut buf).expect("read chunk");
            if n == 0 {
                break;
            }
            result.extend_from_slice(&buf[..n]);
        }

        assert_eq!(
            result.len(),
            expected_raw.len(),
            "Chunked read length mismatch for chunk_size {}",
            chunk_size
        );
        assert_eq!(
            result, expected_raw,
            "Chunked read content mismatch for chunk_size {}",
            chunk_size
        );
    }
}

#[test]
fn test_file_level_decompression_roundtrip() {
    let compressed = read_testdata("quickfox.compressed");
    let expected_raw = read_testdata("quickfox");

    let temp_dir = std::env::temp_dir().join(format!("ttzip_brotli_test_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);

    let comp_file = temp_dir.join("quickfox.br");
    let decomp_file = temp_dir.join("quickfox.out");

    fs::write(&comp_file, &compressed).expect("write comp file");

    let (read_len, written_len) =
        brotli_decompress_file(&comp_file, &decomp_file, None).expect("decompress file");

    assert_eq!(read_len, compressed.len() as u64);
    assert_eq!(written_len, expected_raw.len() as u64);

    let decomp_content = fs::read(&decomp_file).expect("read decomp file");
    assert_eq!(decomp_content, expected_raw);

    let _ = fs::remove_dir_all(temp_dir);
}
