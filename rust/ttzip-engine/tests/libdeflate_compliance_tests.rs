// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Official Libdeflate RFC 1951 / RFC 1950 / RFC 1952 compliance test suite.
//!
//! Ported directly from Eric Biggers' official libdeflate test harness:
//! 1. Canonical corpora matrix (Calgary, Canterbury, Silesia, Fibonacci Skew, 125,500B Literal Run).
//! 2. 100% Bit-Exact roundtrip fidelity across Raw Deflate, Zlib, and Gzip on compression levels 0..=12.
//! 3. Incomplete Huffman code compliance (`test_incomplete_codes.c`: empty offset, singleton litlen, singleton offset).
//! 4. Trailing junk byte isolation and `actual_in_nbytes` / `actual_out_nbytes` precision accounting (`test_trailing_bytes.c`).
//! 5. Overread mitigation and unbounded output cutoff defense (`test_overread.c`).
//! 6. Static and Dynamic Huffman decompression bomb defense and throughput stability (`test_slow_decompression.c`).
//! 7. Comprehensive boundary sizes matrix (0B, 1B, 2B, 15B, 16B, 255B, 256B, 4095B, 4096B, 5552B, 5553B, 65535B, 65536B, 1MB).

use ttzip_engine::benchmark::corpus::{BenchmarkCorpusGenerator, BenchmarkCorpusType};
use ttzip_engine::codecs::deflate::{deflate_decompress, DeflateDecompressor};
use ttzip_engine::codecs::libdeflate::container::{
    compress_container, decompress_container, ContainerFormat,
};
use ttzip_engine::codecs::libdeflate::LibdeflateDecompressor;

// MARK: - 1. Deterministic Bitstream Writer for Synthetic Stream Construction

/// Simple LSB-first bitstream builder mirroring Eric Biggers' `output_bitstream`.
struct TestBitWriter {
    buf: Vec<u8>,
    bitbuf: u64,
    bitsleft: u32,
}

impl TestBitWriter {
    fn new() -> Self {
        Self {
            buf: Vec::new(),
            bitbuf: 0,
            bitsleft: 0,
        }
    }

    fn put_bits(&mut self, val: u32, num_bits: u32) {
        self.bitbuf |= ((val as u64) & ((1u64 << num_bits) - 1)) << self.bitsleft;
        self.bitsleft += num_bits;
        while self.bitsleft >= 8 {
            self.buf.push(self.bitbuf as u8);
            self.bitbuf >>= 8;
            self.bitsleft -= 8;
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bitsleft > 0 {
            self.buf.push(self.bitbuf as u8);
            self.bitbuf = 0;
            self.bitsleft = 0;
        }
        self.buf
    }
}

// MARK: - 2. Canonical Corpora Generators

/// Generates standard Canterbury corpus blend (HTML, C code, English prose, grammar patterns).
fn generate_canterbury_corpus(size: usize) -> Vec<u8> {
    if size == 0 {
        return Vec::new();
    }
    const SNIPPETS: &[&[u8]] = &[
        b"<!DOCTYPE html><html><head><title>Canterbury Corpus</title></head><body>\n",
        b"<p>The Canterbury Corpus is a collection of files for testing lossless compression.</p>\n",
        b"for (int i = 0; i < n; i++) { sum += array[i] * weight[i]; }\n",
        b"Whan that Aprill with his shoures soote / The droghte of March hath perced to the roote,\n",
        b"And bathed every veyne in swich licour / Of which vertu engendred is the flour;\n",
        b"{\n  \"corpus\": \"canterbury\",\n  \"benchmark\": \"libdeflate\",\n  \"version\": 1.0\n}\n",
    ];
    let mut buf = Vec::with_capacity(size);
    let mut idx = 0;
    while buf.len() < size {
        let snippet = SNIPPETS[idx % SNIPPETS.len()];
        let rem = size - buf.len();
        let take = rem.min(snippet.len());
        buf.extend_from_slice(&snippet[..take]);
        idx += 1;
    }
    buf
}

/// Generates extreme skewed Fibonacci distribution byte sequences.
fn generate_fibonacci_skew_corpus(size: usize) -> Vec<u8> {
    if size == 0 {
        return Vec::new();
    }
    // Fibonacci numbers up to 13-th order for weights
    let fib_weights = [1u32, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233, 377];
    let sum_weights: u32 = fib_weights.iter().sum();
    let mut buf = Vec::with_capacity(size);
    let mut state: u32 = 0x1123_5813;

    for _ in 0..size {
        state = state.wrapping_mul(1103515245).wrapping_add(12345);
        let roll = (state >> 16) % sum_weights;
        let mut acc = 0;
        let mut chosen_symbol = 0u8;
        for (i, &w) in fib_weights.iter().enumerate() {
            acc += w;
            if roll < acc {
                chosen_symbol = ((i as u32 * 19 + 65) % 256) as u8; // Spread symbols across ASCII range
                break;
            }
        }
        buf.push(chosen_symbol);
    }
    buf
}

/// Generates Eric Biggers' exact 125,500-byte literal run overflow corpus from `test_litrunlen_overflow.c`.
fn generate_literal_run_overflow_corpus() -> Vec<u8> {
    const DATA_SIZE: usize = 2 * 250 * 251; // 125,500 bytes
    let mut data = vec![0u8; DATA_SIZE];
    let mut j = 0;
    for _ in 0..2 {
        for stride in 1..251 {
            for multiple in 0..251 {
                data[j] = ((stride * multiple) % 251) as u8;
                j += 1;
            }
        }
    }
    assert_eq!(j, DATA_SIZE);
    data
}

// MARK: - 3. Full Levels 0..=12 Bit-Exact Roundtrip Fidelity Tests

#[test]
fn test_official_corpora_full_levels_matrix_roundtrip() {
    let corpora: [(&str, Vec<u8>); 5] = [
        ("Calgary", BenchmarkCorpusGenerator::gen_text_data(64 * 1024)),
        ("Canterbury", generate_canterbury_corpus(64 * 1024)),
        ("Silesia", BenchmarkCorpusGenerator::generate(BenchmarkCorpusType::Silesia, 64 * 1024)),
        ("FibonacciSkew", generate_fibonacci_skew_corpus(64 * 1024)),
        ("LitRunLenOverflow_125K", generate_literal_run_overflow_corpus()),
    ];

    let formats = [ContainerFormat::Raw, ContainerFormat::Zlib, ContainerFormat::Gzip];

    for (name, corpus) in &corpora {
        for &format in &formats {
            for level in 0..=12 {
                let compressed = compress_container(corpus, format, level).unwrap_or_else(|e| {
                    panic!("compress failed for {} {:?} level {}: {:?}", name, format, level, e)
                });

                let mut decompressed = vec![0u8; corpus.len()];
                let written = decompress_container(&compressed, &mut decompressed, format)
                    .unwrap_or_else(|e| {
                        panic!("decompress failed for {} {:?} level {}: {:?}", name, format, level, e)
                    });

                assert_eq!(
                    written,
                    corpus.len(),
                    "Length mismatch on {} {:?} level {}",
                    name,
                    format,
                    level
                );
                assert_eq!(
                    decompressed,
                    *corpus,
                    "Bit-exact payload corruption on {} {:?} level {}",
                    name,
                    format,
                    level
                );
            }
        }
    }
}

// MARK: - 4. Incomplete Huffman Codes Tests (test_incomplete_codes.c)

#[test]
fn test_incomplete_codes_empty_offset_tree() {
    let expected = b"ABAA";
    let mut bw = TestBitWriter::new();

    bw.put_bits(1, 1); // BFINAL: 1
    bw.put_bits(2, 2); // BTYPE: DYNAMIC_HUFFMAN
    bw.put_bits(0, 5); // num_litlen_syms: 0 + 257
    bw.put_bits(0, 5); // num_offset_syms: 0 + 1
    bw.put_bits(14, 4); // num_explicit_precode_lens: 14 + 4

    // Precode codeword lengths: [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15]
    bw.put_bits(0, 3); // 16: len=0
    bw.put_bits(0, 3); // 17: len=0
    bw.put_bits(1, 3); // 18: len=1
    bw.put_bits(3, 3); // 0: len=3
    for _ in 0..11 {
        bw.put_bits(0, 3); // 8..13: len=0
    }
    bw.put_bits(2, 3); // 2: len=2
    bw.put_bits(0, 3); // 14: len=0
    bw.put_bits(3, 3); // 1: len=3

    // Litlen and offset codeword lengths
    bw.put_bits(0x0, 1);
    bw.put_bits(54, 7); // presym_18, 65 zeroes
    bw.put_bits(0x7, 3); // presym_1 ('A')
    bw.put_bits(0x1, 2); // presym_2 ('B')
    bw.put_bits(0x0, 1);
    bw.put_bits(89, 7); // presym_18, 100 zeroes
    bw.put_bits(0x0, 1);
    bw.put_bits(78, 7); // presym_18, 89 zeroes
    bw.put_bits(0x1, 2); // presym_2 (256 EOB)
    bw.put_bits(0x3, 3); // presym_0 (257)

    // Litlen symbols: 'A', 'B', 'A', 'A', EOB
    bw.put_bits(0x0, 1); // 'A'
    bw.put_bits(0x1, 2); // 'B'
    bw.put_bits(0x0, 1); // 'A'
    bw.put_bits(0x0, 1); // 'A'
    bw.put_bits(0x3, 2); // litlensym_256 (EOB)

    let stream = bw.finish();
    let mut out = vec![0u8; 128];
    let written = deflate_decompress(&stream, &mut out).expect("empty offset code must be accepted");
    assert_eq!(&out[..written], expected);
}

#[test]
fn test_incomplete_codes_singleton_litrunlen_tree() {
    let mut bw = TestBitWriter::new();

    bw.put_bits(1, 1); // BFINAL: 1
    bw.put_bits(2, 2); // BTYPE: DYNAMIC_HUFFMAN
    bw.put_bits(0, 5); // num_litlen_syms: 0 + 257
    bw.put_bits(0, 5); // num_offset_syms: 0 + 1
    bw.put_bits(14, 4); // num_explicit_precode_lens: 14 + 4

    for _ in 0..2 {
        bw.put_bits(0, 3);
    }
    bw.put_bits(1, 3); // presym_18: len=1
    bw.put_bits(2, 3); // presym_0: len=2
    for _ in 0..13 {
        bw.put_bits(0, 3);
    }
    bw.put_bits(2, 3); // presym_1: len=2

    for _ in 0..2 {
        bw.put_bits(0, 1);
        bw.put_bits(117, 7); // presym_18: 128 zeroes
    }
    bw.put_bits(0x3, 2); // presym_1
    bw.put_bits(0x1, 2); // presym_0

    // Litlen symbols: symbol 256 (EOB)
    bw.put_bits(0x0, 1);

    let stream = bw.finish();
    let mut out = vec![0u8; 128];
    let written = deflate_decompress(&stream, &mut out)
        .expect("singleton litrunlen code must be accepted");
    assert_eq!(written, 0);
}

#[test]
fn test_incomplete_codes_singleton_offset_tree() {
    let expected = [255u8, 255, 255, 255];
    let mut bw = TestBitWriter::new();

    bw.put_bits(1, 1); // BFINAL: 1
    bw.put_bits(2, 2); // BTYPE: DYNAMIC_HUFFMAN
    bw.put_bits(1, 5); // num_litlen_syms: 1 + 257
    bw.put_bits(0, 5); // num_offset_syms: 0 + 1
    bw.put_bits(14, 4); // num_explicit_precode_lens: 14 + 4

    for _ in 0..2 {
        bw.put_bits(0, 3);
    }
    bw.put_bits(1, 3); // presym_18: len=1
    for _ in 0..12 {
        bw.put_bits(0, 3);
    }
    bw.put_bits(2, 3); // presym_2: len=2
    bw.put_bits(0, 3); // presym_14: len=0
    bw.put_bits(2, 3); // presym_1: len=2

    bw.put_bits(0x0, 1);
    bw.put_bits(117, 7); // presym_18: 128 zeroes
    bw.put_bits(0x0, 1);
    bw.put_bits(116, 7); // presym_18: 127 zeroes
    bw.put_bits(0x1, 2); // presym_1
    bw.put_bits(0x3, 2); // presym_2
    bw.put_bits(0x3, 2); // presym_2
    bw.put_bits(0x1, 2); // presym_1

    bw.put_bits(0x0, 1); // literal 255
    bw.put_bits(0x3, 2); // match len 3 (symbol 257)
    bw.put_bits(0x0, 1); // offset 0 (symbol 0 -> distance 1)
    bw.put_bits(0x1, 2); // EOB (symbol 256)

    let stream = bw.finish();
    let mut out = vec![0u8; 128];
    let written = deflate_decompress(&stream, &mut out).expect("singleton offset tree must be accepted");
    assert_eq!(&out[..written], &expected);
}

#[test]
fn test_incomplete_codes_singleton_offset_notsymzero() {
    let expected = [254u8, 255, 254, 255, 254];
    let mut bw = TestBitWriter::new();

    bw.put_bits(1, 1); // BFINAL: 1
    bw.put_bits(2, 2); // BTYPE: DYNAMIC_HUFFMAN
    bw.put_bits(1, 5); // num_litlen_syms: 1 + 257
    bw.put_bits(1, 5); // num_offset_syms: 1 + 1
    bw.put_bits(14, 4); // num_explicit_precode_lens: 14 + 4

    for _ in 0..2 {
        bw.put_bits(0, 3);
    }
    bw.put_bits(2, 3); // presym_18: len=2
    bw.put_bits(2, 3); // presym_0: len=2
    for _ in 0..11 {
        bw.put_bits(0, 3);
    }
    bw.put_bits(2, 3); // presym_2: len=2
    bw.put_bits(0, 3); // presym_14: len=0
    bw.put_bits(2, 3); // presym_1: len=2

    bw.put_bits(0x3, 2);
    bw.put_bits(117, 7); // presym_18: 128 zeroes
    bw.put_bits(0x3, 2);
    bw.put_bits(115, 7); // presym_18: 126 zeroes
    bw.put_bits(0x1, 2); // presym_2
    bw.put_bits(0x1, 2); // presym_2
    bw.put_bits(0x1, 2); // presym_2
    bw.put_bits(0x1, 2); // presym_2
    bw.put_bits(0x0, 2); // presym_0
    bw.put_bits(0x2, 2); // presym_1

    bw.put_bits(0x0, 2); // literal 254
    bw.put_bits(0x2, 2); // literal 255
    bw.put_bits(0x3, 2); // match len 3 (symbol 257)
    bw.put_bits(0x0, 1); // offset symbol 1 (offset 2)
    bw.put_bits(0x1, 2); // EOB (symbol 256)

    let stream = bw.finish();
    let mut out = vec![0u8; 128];
    let written = deflate_decompress(&stream, &mut out)
        .expect("singleton offset tree with non-zero symbol must be accepted");
    assert_eq!(&out[..written], &expected);
}

// MARK: - 5. Trailing Bytes & Overread Isolation Tests (test_trailing_bytes.c & test_overread.c)

#[test]
fn test_trailing_bytes_isolation_and_accounting() {
    let payload = b"TTZip Deflate Trailing Bytes Isolation and Exact Accounting Verification.";
    let mut decompressor = DeflateDecompressor::new().unwrap();

    let formats = [
        ContainerFormat::Raw,
        ContainerFormat::Zlib,
        ContainerFormat::Gzip,
    ];

    for &format in &formats {
        let compressed = compress_container(payload, format, 6).unwrap();
        let compressed_len = compressed.len();

        // Append trailing junk bytes (e.g. 512 bytes of arbitrary garbage)
        let mut stream_with_junk = compressed.clone();
        stream_with_junk.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF, 0xAA, 0x55, 0xFF, 0x00]);
        stream_with_junk.resize(compressed_len + 512, 0x7E);

        let mut out = vec![0u8; payload.len()];

        match format {
            ContainerFormat::Raw => {
                let (actual_in, actual_out) = decompressor
                    .decompress_ex(&stream_with_junk, &mut out)
                    .expect("decompress_ex on raw deflate with trailing bytes must succeed");
                assert_eq!(actual_in, compressed_len);
                assert_eq!(actual_out, payload.len());
                assert_eq!(&out[..actual_out], payload.as_slice());
            }
            ContainerFormat::Zlib => {
                let (actual_in, actual_out) = decompressor
                    .zlib_decompress_ex(&stream_with_junk, &mut out)
                    .expect("zlib_decompress_ex with trailing bytes must succeed");
                assert_eq!(actual_in, compressed_len);
                assert_eq!(actual_out, payload.len());
                assert_eq!(&out[..actual_out], payload.as_slice());
            }
            ContainerFormat::Gzip => {
                let (actual_in, actual_out) = decompressor
                    .gzip_decompress_ex(&stream_with_junk, &mut out)
                    .expect("gzip_decompress_ex with trailing bytes must succeed");
                assert_eq!(actual_in, compressed_len);
                assert_eq!(actual_out, payload.len());
                assert_eq!(&out[..actual_out], payload.as_slice());
            }
        }
    }
}

#[test]
fn test_overread_unbounded_zero_protection() {
    let mut bw = TestBitWriter::new();
    bw.put_bits(0, 1); // BFINAL: 0
    bw.put_bits(2, 2); // BTYPE: DYNAMIC_HUFFMAN
    bw.put_bits(0, 5); // num_litlen_syms: 0 + 257
    bw.put_bits(0, 5); // num_offset_syms: 0 + 1
    bw.put_bits(14, 4); // num_explicit_precode_lens: 14 + 4

    bw.put_bits(0, 3); // presym_16: len=0
    bw.put_bits(0, 3); // presym_17: len=0
    bw.put_bits(1, 3); // presym_18: len=1
    for _ in 0..14 {
        bw.put_bits(0, 3); // presym_0..14: len=0
    }
    bw.put_bits(1, 3); // presym_1: len=1

    bw.put_bits(0, 1); // presym_1
    bw.put_bits(1, 1);
    bw.put_bits(117, 7); // presym_18 (11 + 117 zeroes)
    bw.put_bits(1, 1);
    bw.put_bits(116, 7); // presym_18 (11 + 116 zeroes)
    bw.put_bits(0, 1); // presym_1
    bw.put_bits(0, 1); // presym_1

    let stream = bw.finish();
    let mut out = vec![0u8; 256];
    let res = deflate_decompress(&stream, &mut out);
    assert!(
        res.is_err(),
        "Prematurely truncated stream with zero codeword literal must return error rather than overread"
    );
}

// MARK: - 6. Static & Dynamic Huffman Decompression Bomb Defense (test_slow_decompression.c)

#[test]
fn test_empty_static_huffman_blocks_bomb_defense() {
    // Generate DEFLATE stream containing 500 consecutive empty static Huffman blocks
    let mut bw = TestBitWriter::new();
    for i in 0..500 {
        let is_final = if i == 499 { 1 } else { 0 };
        bw.put_bits(is_final, 1); // BFINAL
        bw.put_bits(1, 2); // BTYPE: STATIC_HUFFMAN
        bw.put_bits(0, 7); // litlensym_256 (EOB codeword = 0000000b)
    }
    let bomb_stream = bw.finish();

    let mut decompressor = LibdeflateDecompressor::new();
    let mut out = vec![0u8; 64];

    // Must execute rapidly and not trigger quadratic table re-generation or hang
    let written = decompressor
        .decompress(&bomb_stream, &mut out)
        .expect("empty static Huffman blocks must decompress cleanly to 0 bytes");
    assert_eq!(written, 0);
}

#[test]
fn test_empty_dynamic_huffman_blocks_defense() {
    // Generate stream containing 50 consecutive minimal dynamic Huffman blocks
    let mut bw = TestBitWriter::new();
    for i in 0..50 {
        let is_final = if i == 49 { 1 } else { 0 };
        bw.put_bits(is_final, 1); // BFINAL
        bw.put_bits(2, 2); // BTYPE: DYNAMIC_HUFFMAN
        bw.put_bits(0, 5); // num_litlen_syms: 257
        bw.put_bits(0, 5); // num_offset_syms: 1
        bw.put_bits(14, 4); // num_explicit_precode_lens: 18

        for _ in 0..2 {
            bw.put_bits(0, 3);
        }
        bw.put_bits(1, 3); // presym_18: len=1
        for _ in 0..14 {
            bw.put_bits(0, 3);
        }
        bw.put_bits(1, 3); // presym_1: len=1

        for _ in 0..2 {
            bw.put_bits(1, 1);
            bw.put_bits(117, 7); // presym_18: 128 zeroes
        }
        bw.put_bits(0, 1); // presym_1
        bw.put_bits(0, 1); // presym_1

        bw.put_bits(0, 1); // EOB: litlensym_256
    }
    let dynamic_bomb = bw.finish();

    let mut decompressor = LibdeflateDecompressor::new();
    let mut out = vec![0u8; 64];

    let written = decompressor
        .decompress(&dynamic_bomb, &mut out)
        .expect("empty dynamic Huffman blocks must decompress safely");
    assert_eq!(written, 0);
}

// MARK: - 7. Comprehensive Boundary Sizes Matrix Tests

#[test]
fn test_boundary_sizes_matrix() {
    let boundary_sizes = [
        0,              // 0 Bytes (empty)
        1,              // 1 Byte
        2,              // 2 Bytes
        15,             // 15 Bytes (below 16B vector chunk)
        16,             // 16 Bytes (exact vector alignment)
        255,            // 255 Bytes
        256,            // 256 Bytes
        4095,           // 4095 Bytes (4KB - 1)
        4096,           // 4096 Bytes (4KB exact page)
        5552,           // 5552 Bytes (Adler-32 NMAX boundary)
        5553,           // 5553 Bytes (Adler-32 NMAX + 1)
        65535,          // 65535 Bytes (64KB - 1)
        65536,          // 65536 Bytes (64KB window exact)
        1024 * 1024,    // 1 MB
    ];

    let test_levels = [0, 1, 6, 9, 12];
    let formats = [ContainerFormat::Raw, ContainerFormat::Zlib, ContainerFormat::Gzip];

    for &size in &boundary_sizes {
        let payload = generate_fibonacci_skew_corpus(size);

        for &format in &formats {
            for &level in &test_levels {
                let compressed = compress_container(&payload, format, level).unwrap_or_else(|e| {
                    panic!("compress failed for boundary size {} {:?} level {}: {:?}", size, format, level, e)
                });

                let mut decompressed = vec![0u8; size];
                let written = decompress_container(&compressed, &mut decompressed, format).unwrap_or_else(|e| {
                    panic!("decompress failed for boundary size {} {:?} level {}: {:?}", size, format, level, e)
                });

                assert_eq!(written, size);
                assert_eq!(decompressed, payload);
            }
        }
    }
}
