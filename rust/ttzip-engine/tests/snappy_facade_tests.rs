// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration and facade test suite for Google Snappy in TTZip Engine.
//!
//! Validates the 6 production facade APIs across diverse datasets, verifies
//! Raw vs Framed dual-mode isolation, and enforces zero-panic error handling.

use std::io::Read;
use ttzip_engine::codecs::snappy::*;
use ttzip_engine::types::TTZipStatus;

/// Simple deterministic pseudo-random number generator (SplitMix64) for test corpora.
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        let mut chunks = dest.chunks_exact_mut(8);
        for chunk in &mut chunks {
            let val = self.next_u64();
            chunk.copy_from_slice(&val.to_le_bytes());
        }
        let rem = chunks.into_remainder();
        if !rem.is_empty() {
            let val = self.next_u64();
            let bytes = val.to_le_bytes();
            rem.copy_from_slice(&bytes[..rem.len()]);
        }
    }
}

// ============================================================================
// 1. Comprehensive Facade Roundtrip Tests Across Varied Datasets
// ============================================================================

#[test]
fn test_facade_empty_payload() {
    let empty = b"";

    // 1. Raw Facade
    let raw_c = snappy_compress_raw(empty).expect("raw compress empty");
    assert!(!raw_c.is_empty(), "Raw compress must emit 1-byte 0x00 varint header");
    assert!(snappy_validate_raw(&raw_c, 1024));
    let raw_d = snappy_decompress_raw(&raw_c).expect("raw decompress empty");
    assert_eq!(raw_d.as_slice(), empty);

    // 2. Framed Facade
    let framed_c = snappy_compress_framed(empty).expect("framed compress empty");
    assert_eq!(framed_c.len(), SNAPPY_STREAM_IDENTIFIER.len());
    assert!(is_framed_snappy(&framed_c));
    assert!(snappy_validate_framed(&framed_c));
    let framed_d = snappy_decompress_framed(&framed_c).expect("framed decompress empty");
    assert_eq!(framed_d.as_slice(), empty);
}

#[test]
fn test_facade_single_byte_and_tiny_strings() {
    let tiny_cases: &[&[u8]] = &[
        b"X",
        b"ab",
        b"123",
        b"Hello world!",
        b"TTZip Snappy 2026 Microkernel Facade Verification.",
    ];

    for &data in tiny_cases {
        // Raw
        let raw_c = snappy_compress_raw(data).expect("raw compress tiny");
        assert!(snappy_validate_raw(&raw_c, data.len() + 64));
        let raw_d = snappy_decompress_raw(&raw_c).expect("raw decompress tiny");
        assert_eq!(raw_d.as_slice(), data);

        // Framed
        let framed_c = snappy_compress_framed(data).expect("framed compress tiny");
        assert!(is_framed_snappy(&framed_c));
        assert!(snappy_validate_framed(&framed_c));
        let framed_d = snappy_decompress_framed(&framed_c).expect("framed decompress tiny");
        assert_eq!(framed_d.as_slice(), data);
    }
}

#[test]
fn test_facade_large_and_multi_chunk_datasets() {
    let mut rng = SplitMix64::new(0x1122334455667788);

    let test_sizes = [
        1024,             // 1 KB
        64 * 1024,        // 64 KB (single chunk boundary)
        64 * 1024 + 1,    // 64 KB + 1 byte (multi-chunk transition)
        128 * 1024,       // 128 KB (exactly 2 chunks)
        256 * 1024 + 512, // 256.5 KB (multi-chunk)
        512 * 1024,       // 512 KB
    ];

    for &size in &test_sizes {
        // Generate semi-structured payload with high compressibility
        let mut payload = vec![0u8; size];
        for i in 0..size {
            if i % 16 < 12 {
                payload[i] = (i % 64) as u8;
            } else {
                payload[i] = (rng.next_u64() & 0xFF) as u8;
            }
        }

        // 1. Raw Block Roundtrip
        let raw_c = snappy_compress_raw(&payload).expect("raw compress");
        assert!(snappy_validate_raw(&raw_c, size + 1024));
        assert!(!snappy_validate_raw(&raw_c, size.saturating_sub(1))); // bound reject
        let raw_d = snappy_decompress_raw(&raw_c).expect("raw decompress");
        assert_eq!(raw_d.len(), size);
        assert_eq!(raw_d, payload);

        // 2. Framed Stream Roundtrip
        let framed_c = snappy_compress_framed(&payload).expect("framed compress");
        assert!(is_framed_snappy(&framed_c));
        assert!(snappy_validate_framed(&framed_c));
        let framed_d = snappy_decompress_framed(&framed_c).expect("framed decompress");
        assert_eq!(framed_d.len(), size);
        assert_eq!(framed_d, payload);
    }
}

#[test]
fn test_facade_rle_highly_compressible_payload() {
    let payload = vec![0x42u8; 512 * 1024]; // 512 KB uniform data

    // Raw
    let raw_c = snappy_compress_raw(&payload).expect("raw compress rle");
    assert!(raw_c.len() < payload.len() / 10, "Repetitive data must compress heavily");
    assert!(snappy_validate_raw(&raw_c, payload.len() + 1024));
    let raw_d = snappy_decompress_raw(&raw_c).expect("raw decompress rle");
    assert_eq!(raw_d, payload);

    // Framed
    let framed_c = snappy_compress_framed(&payload).expect("framed compress rle");
    assert!(framed_c.len() < payload.len() / 10);
    assert!(is_framed_snappy(&framed_c));
    let mut cursor = std::io::Cursor::new(&framed_c);
    let mut decoder = SnappyFramedReader::new(&mut cursor);
    let mut stack_buf = [0u8; SNAPPY_MAX_CHUNK_SIZE];
    let mut total_read = 0;
    loop {
        match decoder.read(&mut stack_buf) {
            Ok(0) => break,
            Ok(n) => total_read += n,
            Err(e) => panic!("Validation error at offset {}, total read so far {}: {:?}", cursor.position(), total_read, e),
        }
    }
    assert_eq!(total_read, payload.len());
    assert!(snappy_validate_framed(&framed_c));
    let framed_d = snappy_decompress_framed(&framed_c).expect("framed decompress rle");
    assert_eq!(framed_d, payload);
}

#[test]
fn test_facade_high_entropy_random_payload() {
    let mut rng = SplitMix64::new(0xDEADBEEFFEEDCAFE);
    let mut payload = vec![0u8; 100_000];
    rng.fill_bytes(&mut payload);

    // Raw
    let raw_c = snappy_compress_raw(&payload).expect("raw compress random");
    assert!(snappy_validate_raw(&raw_c, payload.len() + 1024));
    let raw_d = snappy_decompress_raw(&raw_c).expect("raw decompress random");
    assert_eq!(raw_d, payload);

    // Framed (will use uncompressed chunk 0x01 when uncompressible)
    let framed_c = snappy_compress_framed(&payload).expect("framed compress random");
    assert!(is_framed_snappy(&framed_c));
    assert!(snappy_validate_framed(&framed_c));
    let framed_d = snappy_decompress_framed(&framed_c).expect("framed decompress random");
    assert_eq!(framed_d, payload);
}

// ============================================================================
// 2. Raw vs Framed Dual-Mode Isolation & Non-Interference
// ============================================================================

#[test]
fn test_raw_and_framed_mutual_isolation() {
    let data = b"Dual-mode isolation verification payload between Raw Block and Framed stream formats.";

    let raw_compressed = snappy_compress_raw(data).expect("raw compress");
    let framed_compressed = snappy_compress_framed(data).expect("framed compress");

    // 1. Raw buffer fed to Framed APIs MUST be rejected
    assert!(
        !is_framed_snappy(&raw_compressed),
        "Raw compressed data must not be identified as framed"
    );
    assert!(
        !snappy_validate_framed(&raw_compressed),
        "Framed validation must reject raw block"
    );
    let framed_dec_err = snappy_decompress_framed(&raw_compressed);
    assert!(
        framed_dec_err.is_err(),
        "Framed decompressor must fail on raw block"
    );

    // 2. Framed buffer fed to Raw APIs MUST be rejected
    assert!(
        !snappy_validate_raw(&framed_compressed, data.len()),
        "Raw validation must reject framed stream"
    );
    let raw_dec_err = snappy_decompress_raw(&framed_compressed);
    assert!(
        raw_dec_err.is_err(),
        "Raw decompressor must fail on framed stream"
    );
}

// ============================================================================
// 3. Defensive 0-Panic Guarantees & Malformed Input Handling
// ============================================================================

#[test]
fn test_defensive_truncated_inputs_zero_panic() {
    let data = b"Testing truncation resilience across every prefix slice length in TTZip.";
    let raw_c = snappy_compress_raw(data).expect("raw compress");
    let framed_c = snappy_compress_framed(data).expect("framed compress");

    // 1. Truncated Raw Block prefixes
    for prefix_len in 0..raw_c.len() {
        let truncated = &raw_c[..prefix_len];
        let val_res = snappy_validate_raw(truncated, 1024 * 1024);
        if prefix_len < raw_c.len() {
            assert!(!val_res, "Truncated raw slice of len {prefix_len} must not validate");
        }
        let dec_res = snappy_decompress_raw(truncated);
        if prefix_len < raw_c.len() {
            assert!(dec_res.is_err(), "Truncated raw slice of len {prefix_len} must return Err");
        }
    }

    // 2. Truncated Framed Stream prefixes
    for prefix_len in 0..framed_c.len() {
        let truncated = &framed_c[..prefix_len];
        let val_res = snappy_validate_framed(truncated);
        let dec_res = snappy_decompress_framed(truncated);
        if prefix_len == 10 {
            // Exactly 10 bytes represents a canonical valid empty framed Snappy stream
            assert!(val_res, "10-byte stream identifier is a valid empty framed stream");
            assert_eq!(dec_res.expect("decode empty"), Vec::<u8>::new());
        } else {
            assert!(!val_res, "Truncated framed slice of len {prefix_len} must not validate");
            assert!(dec_res.is_err(), "Truncated framed slice of len {prefix_len} must return Err");
        }
    }
}

#[test]
fn test_defensive_corrupted_payload_and_checksum_tampering() {
    let data = b"Castagnoli CRC32-C corruption detection and tampering verification payload.";
    let framed_c = snappy_compress_framed(data).expect("framed compress");

    // 1. Corrupt magic identifier
    let mut bad_magic = framed_c.clone();
    bad_magic[0] = 0xFE;
    assert!(!snappy_validate_framed(&bad_magic));
    assert_eq!(
        snappy_decompress_framed(&bad_magic),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // 2. Corrupt CRC bytes (bytes 4..8 in the first chunk, which starts after 10-byte magic)
    if framed_c.len() > 14 {
        let mut bad_crc = framed_c.clone();
        bad_crc[14] ^= 0xFF;
        assert!(!snappy_validate_framed(&bad_crc));
        assert_eq!(
            snappy_decompress_framed(&bad_crc),
            Err(TTZipStatus::ErrCorruptHeader)
        );
    }

    // 3. Corrupt payload byte
    let mut bad_payload = framed_c.clone();
    let last_idx = bad_payload.len() - 1;
    bad_payload[last_idx] ^= 0xAA;
    assert!(!snappy_validate_framed(&bad_payload));
    assert!(snappy_decompress_framed(&bad_payload).is_err());
}

#[test]
fn test_defensive_malicious_varint_and_backreference_attacks() {
    // 1. 4GB malicious uncompressed length claim in varint preamble
    let malicious_4gb = [0xFF, 0xFF, 0xFF, 0xFF, 0x0F, 0x00, 0x01, 0x02];
    assert!(!snappy_validate_raw(&malicious_4gb, 1024 * 1024));
    let dec_res = snappy_decompress_raw(&malicious_4gb);
    assert!(dec_res.is_err());

    // 2. 64-bit varint overflow (> 5 bytes)
    let overflow_varint = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01];
    assert!(!snappy_validate_raw(&overflow_varint, 1024 * 1024));
    assert_eq!(
        snappy_decompress_raw(&overflow_varint),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // 3. Out of bounds copy backreference (offset > decompressed cursor)
    let invalid_copy = [0x0A, 0x05, 0x05];
    assert!(!snappy_validate_raw(&invalid_copy, 1024));
    assert!(snappy_decompress_raw(&invalid_copy).is_err());

    // 4. Zero backreference offset
    let zero_offset = [0x0A, 0x01, 0x00];
    assert!(!snappy_validate_raw(&zero_offset, 1024));
    assert!(snappy_decompress_raw(&zero_offset).is_err());
}

#[test]
fn test_defensive_bounded_validation_limits() {
    let payload = vec![0x77u8; 10_000];
    let compressed = snappy_compress_raw(&payload).expect("compress raw");

    // Exact bound -> true
    assert!(snappy_validate_raw(&compressed, 10_000));

    // Generous bound -> true
    assert!(snappy_validate_raw(&compressed, 50_000));

    // Bounded limit smaller than payload -> MUST return false without decompression
    assert!(!snappy_validate_raw(&compressed, 9_999));
    assert!(!snappy_validate_raw(&compressed, 100));
    assert!(!snappy_validate_raw(&compressed, 0));
}
