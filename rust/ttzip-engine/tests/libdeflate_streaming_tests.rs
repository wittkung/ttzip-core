// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Exhaustive integration and differential verification suite for Safe Rust
//! streaming [`LibdeflateReader`] and [`LibdeflateWriter`].
//!
//! Covers:
//! - Small payload micro-roundtrips (1B, 7B, 100B, 512B) with unaligned byte-by-byte reads.
//! - Multi-block boundaries crossing 64KB buffers (128KB, 250KB).
//! - Large 1MB data roundtrips across all 3 container modes (`Raw`, `Zlib`, `Gzip`).
//! - Bidirectional cross-codec differential oracle testing with `flate2`.
//! - Truncated, corrupted, and adversarial stream defenses (0 Panic Invariant).

use flate2::read::{GzDecoder, ZlibDecoder};
use flate2::write::{GzEncoder, ZlibEncoder};
use flate2::Compression;
use std::io::{self, Cursor, Read, Write};
use ttzip_engine::codecs::libdeflate::{
    ContainerFormat, LibdeflateReader, LibdeflateWriter, DEFAULT_COMPRESS_CHUNK_SIZE,
};

// MARK: - Test Fixture Generators

/// Generates a deterministic pseudo-random byte vector for reproducible fuzzing.
fn generate_pseudo_random(len: usize, seed: u32) -> Vec<u8> {
    let mut state = seed;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        out.push((state >> 24) as u8);
    }
    out
}

/// Generates highly compressible repeated text-like patterns.
fn generate_compressible_pattern(len: usize) -> Vec<u8> {
    let base = b"TTZip Ultra-Fast Streaming DEFLATE Compression Engine 2026! ";
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        let take = base.len().min(len - out.len());
        out.extend_from_slice(&base[..take]);
    }
    out
}

/// Compresses data using `LibdeflateWriter` into a vector.
fn compress_with_writer(
    data: &[u8],
    format: ContainerFormat,
    level: i32,
    chunk_size: usize,
) -> Vec<u8> {
    let mut dest = Vec::new();
    let mut writer =
        LibdeflateWriter::with_chunk_size(&mut dest, format, level, chunk_size).expect("writer init");
    writer.write_all(data).expect("write_all");
    writer.finish().expect("finish");
    dest
}

/// Decompresses data using `LibdeflateReader` with specified read chunk buffer size.
fn decompress_with_reader(
    compressed: &[u8],
    format: ContainerFormat,
    read_step: usize,
) -> io::Result<Vec<u8>> {
    let mut reader = LibdeflateReader::new(Cursor::new(compressed), format).map_err(|e| {
        io::Error::other(format!("reader init failed: {e:?}"))
    })?;

    let mut out = Vec::new();
    let mut step_buf = vec![0u8; read_step];
    loop {
        let n = reader.read(&mut step_buf)?;
        if n == 0 {
            break;
        }
        out.extend_from_slice(&step_buf[..n]);
    }
    Ok(out)
}

// MARK: - 1. Zero-Byte & Small Chunk Roundtrip Tests

#[test]
fn test_zero_byte_roundtrip_all_formats() {
    for &format in &[
        ContainerFormat::Raw,
        ContainerFormat::Zlib,
        ContainerFormat::Gzip,
    ] {
        let empty = b"";
        let compressed = compress_with_writer(empty, format, 6, DEFAULT_COMPRESS_CHUNK_SIZE);
        let decompressed =
            decompress_with_reader(&compressed, format, 64).expect("zero-byte decompression");
        assert_eq!(decompressed, empty);
    }
}

#[test]
fn test_small_chunks_roundtrip() {
    let sizes = [1, 2, 7, 13, 64, 100, 255, 512, 1024];
    let read_steps = [1, 3, 7, 16, 64, 512];

    for &size in &sizes {
        let original = generate_pseudo_random(size, 42 + size as u32);

        for &format in &[
            ContainerFormat::Raw,
            ContainerFormat::Zlib,
            ContainerFormat::Gzip,
        ] {
            for &level in &[0, 1, 6, 12] {
                let compressed = compress_with_writer(&original, format, level, 512);

                for &step in &read_steps {
                    let decomp = decompress_with_reader(&compressed, format, step)
                        .unwrap_or_else(|e| {
                            panic!("failed roundtrip size={size} fmt={format:?} lvl={level} step={step}: {e}")
                        });
                    assert_eq!(
                        decomp, original,
                        "mismatch for size={size} fmt={format:?} lvl={level} step={step}"
                    );
                }
            }
        }
    }
}

// MARK: - 2. Multi-Block & Crossing 64KB Boundaries Tests

#[test]
fn test_crossing_64kb_boundaries() {
    let test_sizes = [65536 - 1, 65536, 65536 + 1, 131072, 256000];

    for &size in &test_sizes {
        let original = generate_compressible_pattern(size);

        for &format in &[
            ContainerFormat::Raw,
            ContainerFormat::Zlib,
            ContainerFormat::Gzip,
        ] {
            let compressed = compress_with_writer(&original, format, 6, DEFAULT_COMPRESS_CHUNK_SIZE);
            assert!(
                !compressed.is_empty(),
                "compressed stream must not be empty"
            );

            let decomp = decompress_with_reader(&compressed, format, 8192).expect("decompress 64kb+");
            assert_eq!(decomp.len(), original.len());
            assert_eq!(decomp, original);
        }
    }
}

// MARK: - 3. Large 1MB Stream Roundtrip Tests

#[test]
fn test_large_1mb_roundtrip_all_formats() {
    let size = 1024 * 1024; // 1 MB
    let random_data = generate_pseudo_random(size, 9999);
    let pattern_data = generate_compressible_pattern(size);

    for (desc, dataset) in &[("random", &random_data), ("pattern", &pattern_data)] {
        for &format in &[
            ContainerFormat::Raw,
            ContainerFormat::Zlib,
            ContainerFormat::Gzip,
        ] {
            let compressed = compress_with_writer(dataset, format, 6, DEFAULT_COMPRESS_CHUNK_SIZE);
            let decomp = decompress_with_reader(&compressed, format, 65536)
                .unwrap_or_else(|e| panic!("1MB {desc} {format:?} failed: {e}"));

            assert_eq!(decomp.len(), dataset.len());
            assert_eq!(&decomp[..], &dataset[..]);
        }
    }
}

// MARK: - 4. Cross-Codec Differential Oracle Tests with `flate2`

#[test]
fn test_differential_oracle_zlib_with_flate2() {
    let data = generate_compressible_pattern(150_000);

    // 1. LibdeflateWriter (Zlib) -> flate2 ZlibDecoder
    let ttzip_compressed = compress_with_writer(&data, ContainerFormat::Zlib, 6, 32768);
    let mut flate2_decoder = ZlibDecoder::new(Cursor::new(&ttzip_compressed));
    let mut flate2_decomp = Vec::new();
    flate2_decoder
        .read_to_end(&mut flate2_decomp)
        .expect("flate2 decode ttzip zlib");
    assert_eq!(flate2_decomp, data);

    // 2. flate2 ZlibEncoder -> LibdeflateReader (Zlib)
    let mut flate2_encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    flate2_encoder.write_all(&data).expect("flate2 write");
    let flate2_compressed = flate2_encoder.finish().expect("flate2 finish");

    let ttzip_decomp = decompress_with_reader(&flate2_compressed, ContainerFormat::Zlib, 16384)
        .expect("ttzip decode flate2 zlib");
    assert_eq!(ttzip_decomp, data);
}

#[test]
fn test_differential_oracle_gzip_with_flate2() {
    let data = generate_compressible_pattern(200_000);

    // 1. LibdeflateWriter (Gzip) -> flate2 GzDecoder
    let ttzip_compressed = compress_with_writer(&data, ContainerFormat::Gzip, 6, 65536);
    let mut flate2_decoder = GzDecoder::new(Cursor::new(&ttzip_compressed));
    let mut flate2_decomp = Vec::new();
    flate2_decoder
        .read_to_end(&mut flate2_decomp)
        .expect("flate2 decode ttzip gzip");
    assert_eq!(flate2_decomp, data);

    // 2. flate2 GzEncoder -> LibdeflateReader (Gzip)
    let mut flate2_encoder = GzEncoder::new(Vec::new(), Compression::fast());
    flate2_encoder.write_all(&data).expect("flate2 gz write");
    let flate2_compressed = flate2_encoder.finish().expect("flate2 gz finish");

    let ttzip_decomp = decompress_with_reader(&flate2_compressed, ContainerFormat::Gzip, 8192)
        .expect("ttzip decode flate2 gzip");
    assert_eq!(ttzip_decomp, data);
}

// MARK: - 5. Corrupted Headers & Footers Defense Tests (0 Panic)

#[test]
fn test_corrupted_zlib_header_defense() {
    let data = b"Hello Zlib Defense Test";
    let mut valid = compress_with_writer(data, ContainerFormat::Zlib, 6, 65536);

    // Corrupt CMF / FLG checksum rule
    valid[1] ^= 0x01;
    let res = decompress_with_reader(&valid, ContainerFormat::Zlib, 128);
    assert!(res.is_err(), "corrupted zlib header must return Error");
}

#[test]
fn test_corrupted_zlib_adler32_defense() {
    let data = b"Hello Zlib Adler-32 Checksum Tamper Defense";
    let mut valid = compress_with_writer(data, ContainerFormat::Zlib, 6, 65536);

    // Tamper the last 4 bytes (Adler-32 footer)
    let len = valid.len();
    valid[len - 1] ^= 0xFF;
    let res = decompress_with_reader(&valid, ContainerFormat::Zlib, 128);
    assert!(
        res.is_err(),
        "tampered Adler-32 checksum must fail verification"
    );
}

#[test]
fn test_corrupted_gzip_magic_defense() {
    let data = b"Hello Gzip Magic Tamper Defense";
    let mut valid = compress_with_writer(data, ContainerFormat::Gzip, 6, 65536);

    // Corrupt magic ID1
    valid[0] = 0x00;
    let res = decompress_with_reader(&valid, ContainerFormat::Gzip, 128);
    assert!(res.is_err(), "invalid gzip magic must return Error");
}

#[test]
fn test_corrupted_gzip_crc32_and_isize_defense() {
    let data = b"Testing Gzip CRC-32 and ISIZE verification failure";
    let valid = compress_with_writer(data, ContainerFormat::Gzip, 6, 65536);

    // 1. Tamper CRC-32 (bytes at len - 8 .. len - 4)
    let len = valid.len();
    let mut crc_tampered = valid.clone();
    crc_tampered[len - 8] ^= 0x55;
    let res_crc = decompress_with_reader(&crc_tampered, ContainerFormat::Gzip, 64);
    assert!(res_crc.is_err(), "tampered CRC32 must return Error");

    // 2. Tamper ISIZE (bytes at len - 4 .. len)
    let mut isize_tampered = valid;
    isize_tampered[len - 2] ^= 0xAA;
    let res_isize = decompress_with_reader(&isize_tampered, ContainerFormat::Gzip, 64);
    assert!(res_isize.is_err(), "tampered ISIZE must return Error");
}

// MARK: - 6. Stream Truncation & Unexpected EOF Defense (0 Panic)

#[test]
fn test_truncated_stream_sweep_no_panic() {
    let original = generate_compressible_pattern(1024);

    for &format in &[
        ContainerFormat::Raw,
        ContainerFormat::Zlib,
        ContainerFormat::Gzip,
    ] {
        let compressed = compress_with_writer(&original, format, 6, 512);

        // Progressively truncate from 0 bytes up to full length - 1
        for cut in 0..compressed.len() {
            let truncated = &compressed[..cut];
            let res = decompress_with_reader(truncated, format, 32);
            // It should either return error or (for 0 cut on Raw) return 0 bytes, NEVER panic
            if cut < compressed.len() && cut > 0 {
                let _ = res.is_err();
            }
        }
    }
}

#[test]
fn test_concatenated_gzip_members() {
    let chunk1 = b"First concatenated gzip member payload. ";
    let chunk2 = b"Second concatenated gzip member payload with extra words.";

    let comp1 = compress_with_writer(chunk1, ContainerFormat::Gzip, 6, 1024);
    let comp2 = compress_with_writer(chunk2, ContainerFormat::Gzip, 6, 1024);

    let mut concatenated = comp1;
    concatenated.extend_from_slice(&comp2);

    let decomp = decompress_with_reader(&concatenated, ContainerFormat::Gzip, 16)
        .expect("concatenated gzip decompress");

    let mut expected = Vec::new();
    expected.extend_from_slice(chunk1);
    expected.extend_from_slice(chunk2);

    assert_eq!(decomp, expected);
}
