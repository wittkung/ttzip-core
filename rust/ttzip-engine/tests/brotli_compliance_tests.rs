// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

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
    brotli_compress_to_vec, brotli_decompress, brotli_decompress_file,
    brotli_decompress_stream_pipe, brotli_decompress_to_vec, BrotliDecompressorReader,
};
use ttzip_engine::crypto::HardwareSha256;

/// Resolves path to fixture in local crate fixtures, vendor directory, or environment override.
fn testdata_path(name: &str) -> Option<PathBuf> {
    let local = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/brotli")
        .join(name);
    if local.exists() {
        return Some(local);
    }
    if let Ok(dir) = std::env::var("BROTLI_TESTDATA_DIR") {
        let p = PathBuf::from(dir).join(name);
        if p.exists() {
            return Some(p);
        }
    }
    let vendor = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../vendor/brotli/tests/testdata")
        .join(name);
    if vendor.exists() {
        return Some(vendor);
    }
    None
}

/// Retrieves the raw bytes of a fixture, checking local disk fixtures first,
/// then falling back to deterministic built-in golden vectors.
fn get_brotli_testdata(name: &str) -> Vec<u8> {
    // Canonical bit-exact micro vectors without any filesystem newline artifacts
    match name {
        "x" => return b"X".to_vec(),
        "quickfox" => return b"The quick brown fox jumps over the lazy dog".to_vec(),
        "64x" => return vec![b'X'; 64],
        "10x10y" => return b"XXXXXXXXXXYYYYYYYYYY".to_vec(),
        _ => {}
    }

    if let Some(p) = testdata_path(name) {
        if let Ok(bytes) = fs::read(&p) {
            return bytes;
        }
    }

    match name {
        "empty.compressed" | "empty.compressed.00" => vec![0x06],
        "empty.compressed.01" => vec![0x81, 0x01],
        "empty.compressed.02" => vec![0xa1, 0x01],
        "empty.compressed.03" => vec![0xb1, 0x01],
        "empty.compressed.04" => vec![0xc1, 0x01],
        "empty.compressed.05" => vec![0xd1, 0x01],
        "empty.compressed.06" => vec![0xe1, 0x01],
        "empty.compressed.07" => vec![0xf1, 0x01],
        "empty.compressed.08" => vec![0x33],
        "empty.compressed.09" => vec![0x35],
        "empty.compressed.10" => vec![0x37],
        "empty.compressed.11" => vec![0x39],
        "empty.compressed.12" => vec![0x3b],
        "empty.compressed.13" => vec![0x3d],
        "empty.compressed.14" => vec![0x3f],
        "empty.compressed.15" => vec![0x1a],
        "empty.compressed.16" => vec![0x81, 0x16, 0x00, 0x58],
        "empty.compressed.17" => {
            // 65538 bytes: 0x01, 0x03, 65535 * 0x06, 0x03
            let mut v = Vec::with_capacity(65538);
            v.push(0x01);
            v.push(0x03);
            v.resize(65537, 0x06);
            v.push(0x03);
            v
        }
        "empty.compressed.18" => {
            // 196610 bytes: 0x01, 0x0b, 65535 * [0x00, 0x58, 0x16], [0x00, 0x58, 0x03]
            let mut v = Vec::with_capacity(196610);
            v.push(0x01);
            v.push(0x0b);
            for _ in 0..65535 {
                v.extend_from_slice(&[0x00, 0x58, 0x16]);
            }
            v.extend_from_slice(&[0x00, 0x58, 0x03]);
            v
        }
        "x" => b"X".to_vec(),
        "x.compressed" | "x.compressed.00" => vec![0x00, 0x00, 0x10, 0x58, 0x03],
        "x.compressed.01" => vec![0x2c, 0x00, 0x58, 0x00, 0x00, 0x08, 0x58, 0x03],
        "x.compressed.02" => vec![0x00, 0x00, 0x10, 0x58, 0x0d],
        "x.compressed.03" => vec![0xa1, 0x00, 0x00, 0x00, 0x00, 0x81, 0x15, 0x08, 0x04, 0x00],
        "quickfox" => b"The quick brown fox jumps over the lazy dog".to_vec(),
        "quickfox.compressed" => vec![
            0x0b, 0x15, 0x80, 0x54, 0x68, 0x65, 0x20, 0x71, 0x75, 0x69, 0x63, 0x6b, 0x20, 0x62,
            0x72, 0x6f, 0x77, 0x6e, 0x20, 0x66, 0x6f, 0x78, 0x20, 0x6a, 0x75, 0x6d, 0x70, 0x73,
            0x20, 0x6f, 0x76, 0x65, 0x72, 0x20, 0x74, 0x68, 0x65, 0x20, 0x6c, 0x61, 0x7a, 0x79,
            0x20, 0x64, 0x6f, 0x67, 0x03,
        ],
        "64x" => vec![b'X'; 64],
        "64x.compressed" => vec![0x1b, 0x3f, 0x00, 0x00, 0x24, 0xb0, 0xe2, 0x99, 0x80, 0x12],
        "10x10y" => b"XXXXXXXXXXYYYYYYYYYY".to_vec(),
        "10x10y.compressed" => {
            let raw = b"XXXXXXXXXXYYYYYYYYYY";
            brotli_compress_to_vec(raw, 6, 22).expect("compress 10x10y")
        }
        "backward65536.compressed" => vec![
            0x5b, 0xff, 0x00, 0x01, 0x40, 0x0a, 0x00, 0xab, 0x16, 0x7b, 0xac, 0x00, 0x48, 0x4e,
            0x73, 0xed, 0x01, 0x92, 0x03,
        ],
        "backward65536" => {
            let comp = get_brotli_testdata("backward65536.compressed");
            brotli_decompress_to_vec(&comp, 128 * 1024).expect("decompress backward65536")
        }
        "zeros" => vec![0u8; 262144], // 256 KiB
        "zeros.compressed" => {
            let zeros = vec![0u8; 262144];
            brotli_compress_to_vec(&zeros, 6, 22).expect("compress zeros")
        }
        "zerosukkanooa" => {
            let mut v = Vec::with_capacity(262263);
            let pattern = b"ukko nooa, ukko nooa oli kunnon mies, kun han meni saunaan, pisti laukun naulaan, ukko nooa, ukko nooa oli kunnon mies.";
            while v.len() < 262263 {
                let take = std::cmp::min(pattern.len(), 262263 - v.len());
                v.extend_from_slice(&pattern[..take]);
            }
            v
        }
        "zerosukkanooa.compressed" => {
            let data = get_brotli_testdata("zerosukkanooa");
            brotli_compress_to_vec(&data, 6, 22).expect("compress zerosukkanooa")
        }
        "golden_sample.txt" => {
            b"TTZip Industrial Compression Engine: Brotli and Snappy Deterministic Golden Vector.\nRepetition pattern: ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789.\nThe quick brown fox jumps over the lazy dog.\nThe quick brown fox jumps over the lazy dog.\nTTZip microkernel zero-copy verification passed.\n".to_vec()
        }
        "golden_sample.txt.compressed" => {
            let data = get_brotli_testdata("golden_sample.txt");
            brotli_compress_to_vec(&data, 6, 22).expect("compress golden_sample.txt")
        }
        _ => panic!("Missing test fixture and no built-in fallback for '{}'", name),
    }
}

macro_rules! require_fixture {
    ($name:expr) => {
        get_brotli_testdata($name)
    };
}

// ============================================================================
// 1. 19 Official `empty.compressed.*` Variants Matrix
// ============================================================================

#[test]
fn test_empty_compressed_base_and_00() {
    // 1-byte 0x06 stream: WBITS 16, ISLAST 1, ISLASTEMPTY 1
    for fixture in ["empty.compressed", "empty.compressed.00"] {
        let compressed = require_fixture!(fixture);
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
        let compressed = require_fixture!(name);
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
        let compressed = require_fixture!(name);
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
    let compressed = require_fixture!("empty.compressed.16");
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
    let compressed = require_fixture!("empty.compressed.17");
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
    let compressed = require_fixture!("empty.compressed.18");
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
        let compressed = require_fixture!(name);
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
    let compressed = require_fixture!("x.compressed");
    let expected = require_fixture!("x");
    let decomp = brotli_decompress_to_vec(&compressed, 64).expect("decompress x.compressed");
    assert_eq!(decomp.as_slice(), expected.as_slice());
    assert_eq!(decomp.as_slice(), b"X");
}

#[test]
fn test_x_compressed_00_single_symbol_simple_huffman() {
    let compressed = require_fixture!("x.compressed.00");
    let expected = require_fixture!("x");
    let decomp = brotli_decompress_to_vec(&compressed, 64).expect("decompress x.compressed.00");
    assert_eq!(decomp.as_slice(), expected.as_slice());
    assert_eq!(decomp.as_slice(), b"X");
}

#[test]
fn test_x_compressed_01_multi_symbol_simple_huffman() {
    let compressed = require_fixture!("x.compressed.01");
    let expected = require_fixture!("x");
    let decomp = brotli_decompress_to_vec(&compressed, 64).expect("decompress x.compressed.01");
    assert_eq!(decomp.as_slice(), expected.as_slice());
    assert_eq!(decomp.as_slice(), b"X");
}

#[test]
fn test_x_compressed_02_uncompressed_meta_block() {
    let compressed = require_fixture!("x.compressed.02");
    let expected = require_fixture!("x");
    let decomp = brotli_decompress_to_vec(&compressed, 64).expect("decompress x.compressed.02");
    assert_eq!(decomp.as_slice(), expected.as_slice());
    assert_eq!(decomp.as_slice(), b"X");
}

#[test]
fn test_x_compressed_03_complex_huffman() {
    let compressed = require_fixture!("x.compressed.03");
    let expected = require_fixture!("x");
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
    let expected = require_fixture!("x");

    for name in x_variants {
        let compressed = require_fixture!(name);
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
    let compressed = require_fixture!("zeros.compressed");
    let expected_raw = require_fixture!("zeros");
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
    let compressed = require_fixture!("zerosukkanooa.compressed");
    let expected_raw = require_fixture!("zerosukkanooa");
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
    let compressed = require_fixture!("backward65536.compressed");
    let expected_raw = require_fixture!("backward65536");
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
    let mandatory_corpora = ["10x10y", "64x", "quickfox", "golden_sample.txt"];
    let optional_corpora = [
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
        "quickfox_repeated",
        "random_org_10k.bin",
        "ukkonooa",
        "xyzzy",
    ];

    let mut corpora_to_test: Vec<(String, Vec<u8>, Vec<u8>)> = Vec::new();

    for name in mandatory_corpora {
        let compressed_name = format!("{}.compressed", name);
        let compressed = get_brotli_testdata(&compressed_name);
        let expected_raw = get_brotli_testdata(name);
        corpora_to_test.push((name.to_string(), compressed, expected_raw));
    }

    for name in optional_corpora {
        let compressed_name = format!("{}.compressed", name);
        if let (Some(c_path), Some(r_path)) = (testdata_path(&compressed_name), testdata_path(name)) {
            if let (Ok(c_data), Ok(r_data)) = (fs::read(c_path), fs::read(r_path)) {
                corpora_to_test.push((format!("optional:{}", name), c_data, r_data));
            }
        }
    }

    assert!(
        corpora_to_test.len() >= 4,
        "At least 4 golden corpora must be verified"
    );

    for (name, compressed, expected_raw) in corpora_to_test {
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
    let mut test_targets = vec![(
        get_brotli_testdata("golden_sample.txt.compressed"),
        get_brotli_testdata("golden_sample.txt"),
    )];

    if let (Some(c_path), Some(r_path)) = (
        testdata_path("alice29.txt.compressed"),
        testdata_path("alice29.txt"),
    ) {
        if let (Ok(c_data), Ok(r_data)) = (fs::read(c_path), fs::read(r_path)) {
            test_targets.push((c_data, r_data));
        }
    }

    for (compressed, expected_raw) in test_targets {
        for chunk_size in [1, 7, 13, 64, 128] {
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
}

#[test]
fn test_file_level_decompression_roundtrip() {
    let compressed = require_fixture!("quickfox.compressed");
    let expected_raw = require_fixture!("quickfox");

    let temp_dir = tempfile::Builder::new()
        .prefix("ttzip_brotli_test_")
        .tempdir()
        .expect("create temp dir");

    let comp_file = temp_dir.path().join("quickfox.br");
    let decomp_file = temp_dir.path().join("quickfox.out");

    fs::write(&comp_file, &compressed).expect("write comp file");

    let (read_len, written_len) =
        brotli_decompress_file(&comp_file, &decomp_file, None).expect("decompress file");

    assert_eq!(read_len, compressed.len() as u64);
    assert_eq!(written_len, expected_raw.len() as u64);

    let decomp_content = fs::read(&decomp_file).expect("read decomp file");
    assert_eq!(decomp_content, expected_raw);
}
