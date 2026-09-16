// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration tests for Apple LZFSE and LZVN production facade APIs.
//!
//! Verifies all 8 canonical facade functions, multi-block streaming pipe,
//! dual-mode format isolation, C-ABI FFI exports, and strict 0-panic error handling.

use std::io::{Cursor, Read, Write};
use ttzip_engine::codecs::lzfse::{
    lzfse_compress_raw, lzfse_compress_stream, lzfse_decompress_raw, lzfse_decompress_stream,
    lzfse_validate, lzvn_compress_raw, lzvn_decompress_raw, lzvn_validate, LzfseReader,
    LzfseWriter,
};
use ttzip_engine::ffi::codecs_ffi::{
    ttzip_rust_lzfse_compress_bound, ttzip_rust_lzfse_compress_raw,
    ttzip_rust_lzfse_compress_stream, ttzip_rust_lzfse_decompress_raw,
    ttzip_rust_lzfse_decompress_stream, ttzip_rust_lzfse_validate,
    ttzip_rust_lzvn_compress_bound, ttzip_rust_lzvn_compress_raw,
    ttzip_rust_lzvn_decompress_raw, ttzip_rust_lzvn_validate,
};
use ttzip_engine::types::TTZipStatus;

// MARK: - Helper Data Generators

fn generate_repetitive_text(count: usize) -> Vec<u8> {
    let sentence = b"Apple Silicon hardware-accelerated LZFSE and LZVN codecs in pure Safe Rust. ";
    let mut out = Vec::with_capacity(count * sentence.len());
    for _ in 0..count {
        out.extend_from_slice(sentence);
    }
    out
}

fn generate_mixed_source_code(target_bytes: usize) -> Vec<u8> {
    let code = b"pub fn execute_stream_transform<R: Read, W: Write>(reader: &mut R, writer: &mut W) -> Result<u64, TTZipStatus> {\n    let mut buffer = [0u8; 65536];\n    let mut total = 0u64;\n    while let Ok(n) = reader.read(&mut buffer) {\n        if n == 0 { break; }\n        writer.write_all(&buffer[..n]).map_err(|_| TTZipStatus::ErrIo)?;\n        total += n as u64;\n    }\n    Ok(total)\n}\n";
    let mut out = Vec::with_capacity(target_bytes);
    while out.len() < target_bytes {
        out.extend_from_slice(code);
    }
    out.truncate(target_bytes);
    out
}

// MARK: - 1. LZFSE Raw Block Facade Tests

#[test]
fn test_lzfse_raw_roundtrip_various_payloads() {
    let test_cases: Vec<Vec<u8>> = vec![
        // Empty buffer
        Vec::new(),
        // Small 32-byte string
        b"Small Apple LZFSE test payload!".to_vec(),
        // Repetitive 4KB text
        generate_repetitive_text(60),
        // 64KB mixed code
        generate_mixed_source_code(64 * 1024),
        // Alternating bytes
        (0..8192)
            .map(|i| if i % 2 == 0 { b'X' } else { b'Y' })
            .collect(),
    ];

    for (idx, original) in test_cases.iter().enumerate() {
        let compressed = lzfse_compress_raw(original)
            .unwrap_or_else(|e| panic!("case {idx} lzfse_compress_raw failed: {e:?}"));

        if original.is_empty() {
            assert!(compressed.is_empty());
            let decompressed = lzfse_decompress_raw(&compressed, 0)
                .expect("empty decompress");
            assert!(decompressed.is_empty());
            continue;
        }

        assert!(!compressed.is_empty());
        assert!(lzfse_validate(&compressed), "case {idx} lzfse_validate should succeed");

        let decompressed = lzfse_decompress_raw(&compressed, original.len())
            .unwrap_or_else(|e| panic!("case {idx} lzfse_decompress_raw failed: {e:?}"));

        assert_eq!(&decompressed[..], &original[..], "Mismatch in case {idx}");
    }
}

// MARK: - 2. LZFSE Multi-Block Streaming Facade Tests

#[test]
fn test_lzfse_stream_roundtrip_multi_blocks() {
    // 600KB payload spanning three 256KB chunks
    let original = generate_mixed_source_code(600 * 1024);

    let compressed = lzfse_compress_stream(&original)
        .expect("lzfse_compress_stream 600KB");
    assert!(!compressed.is_empty());
    assert!(compressed.len() < original.len() / 2, "Should compress well");

    assert!(lzfse_validate(&compressed), "lzfse_validate on 600KB stream");

    // Decompress stream without knowing uncompressed size ahead of time
    let decompressed = lzfse_decompress_stream(&compressed)
        .expect("lzfse_decompress_stream 600KB");

    assert_eq!(decompressed.len(), original.len());
    assert_eq!(&decompressed[..], &original[..]);
}

// MARK: - 3. LZVN Raw Block Facade Tests

#[test]
fn test_lzvn_raw_roundtrip_various_payloads() {
    let test_cases: Vec<Vec<u8>> = vec![
        Vec::new(),
        b"Apple LZVN high speed kernel compression 2026.".to_vec(),
        generate_repetitive_text(40),
        generate_mixed_source_code(32 * 1024),
    ];

    for (idx, original) in test_cases.iter().enumerate() {
        let compressed = lzvn_compress_raw(original)
            .unwrap_or_else(|e| panic!("case {idx} lzvn_compress_raw failed: {e:?}"));

        if original.is_empty() {
            assert!(compressed.is_empty());
            let decompressed = lzvn_decompress_raw(&compressed, 0).expect("empty lzvn decompress");
            assert!(decompressed.is_empty());
            continue;
        }

        assert!(!compressed.is_empty());
        assert!(lzvn_validate(&compressed), "case {idx} lzvn_validate should succeed");

        let decompressed = lzvn_decompress_raw(&compressed, original.len())
            .unwrap_or_else(|e| panic!("case {idx} lzvn_decompress_raw failed: {e:?}"));

        assert_eq!(&decompressed[..], &original[..], "Mismatch in LZVN case {idx}");
    }
}

// MARK: - 4. LZFSE Streaming Reader & Writer Adapter Tests

#[test]
fn test_lzfse_reader_writer_streaming_pipe() {
    let original = generate_mixed_source_code(300 * 1024);

    // 1. Write stream using LzfseWriter
    let mut compressed_buf = Vec::new();
    {
        let mut writer = LzfseWriter::new(&mut compressed_buf);
        // Write in irregular chunk sizes
        for chunk in original.chunks(1337) {
            writer.write_all(chunk).expect("writer write_all");
        }
        writer.finish().expect("writer finish");
    }

    assert!(lzfse_validate(&compressed_buf), "Stream written by LzfseWriter must be valid");

    // 2. Read stream back using LzfseReader
    let cursor = Cursor::new(compressed_buf);
    let mut reader = LzfseReader::new(cursor);
    let mut decompressed = Vec::new();
    let mut read_chunk = [0u8; 4096];

    loop {
        let n = reader.read(&mut read_chunk).expect("reader read");
        if n == 0 {
            break;
        }
        decompressed.extend_from_slice(&read_chunk[..n]);
    }

    assert_eq!(decompressed.len(), original.len());
    assert_eq!(&decompressed[..], &original[..]);
}

// MARK: - 5. Dual-Mode Format Isolation & Cross-Rejection Tests

#[test]
fn test_dual_mode_isolation_and_cross_rejection() {
    let payload = generate_repetitive_text(50);

    let lzfse_comp = lzfse_compress_raw(&payload).expect("lzfse compress");
    let lzvn_comp = lzvn_compress_raw(&payload).expect("lzvn compress");

    // 1. Cross-format validation must reject foreign formats
    assert!(lzfse_validate(&lzfse_comp));
    assert!(lzvn_validate(&lzvn_comp));

    // LZVN payload passed to LZFSE stream validator must return false
    assert!(!lzfse_validate(&lzvn_comp));

    // 2. Cross-format decompression must fail gracefully without panic
    let lzvn_on_lzfse_res = lzvn_decompress_raw(&lzfse_comp, payload.len());
    assert!(lzvn_on_lzfse_res.is_err());

    let lzfse_on_lzvn_res = lzfse_decompress_stream(&lzvn_comp);
    assert!(lzfse_on_lzvn_res.is_err());
}

// MARK: - 6. Invalid Inputs & Zero-Panic Robustness Tests

#[test]
fn test_invalid_inputs_zero_panic() {
    // 1. Random noise / fuzz input
    let garbage = vec![0xDE, 0xAD, 0xBE, 0xEF, 0xAA, 0x55, 0x12, 0x34];
    assert!(!lzfse_validate(&garbage));
    assert!(!lzvn_validate(&garbage));
    assert!(lzfse_decompress_raw(&garbage, 100).is_err());
    assert!(lzfse_decompress_stream(&garbage).is_err());
    assert!(lzvn_decompress_raw(&garbage, 100).is_err());

    // 2. Truncated header (3 bytes)
    let short_header = [0x62, 0x76, 0x78]; // "bvx"
    assert!(!lzfse_validate(&short_header));
    assert!(lzfse_decompress_stream(&short_header).is_err());

    // 3. Length mismatch on raw decompress
    let original = b"Sample string for testing length bound mismatch.".to_vec();
    let comp = lzfse_compress_raw(&original).expect("lzfse compress");
    // Requesting smaller length than actual payload produces extraction error
    let mismatch_err = lzfse_decompress_raw(&comp, 5);
    assert!(mismatch_err.is_err());
}

// MARK: - 7. C-ABI FFI Bridge Direct Integration Tests

#[test]
fn test_c_abi_ffi_bridge_exports() {
    let data = b"Testing CTTZipBridge C-ABI LZFSE and LZVN symbols in Rust integration test.";

    // 1. LZFSE C-ABI
    let lzfse_bound = ttzip_rust_lzfse_compress_bound(data.len());
    assert!(lzfse_bound >= data.len());

    let mut lzfse_out = vec![0u8; lzfse_bound];
    let mut lzfse_written = 0usize;
    let st = ttzip_rust_lzfse_compress_raw(
        data.as_ptr(),
        data.len(),
        lzfse_out.as_mut_ptr(),
        lzfse_out.len(),
        &mut lzfse_written,
    );
    assert_eq!(st, TTZipStatus::Ok);
    assert!(lzfse_written > 0);

    assert!(ttzip_rust_lzfse_validate(lzfse_out.as_ptr(), lzfse_written));

    let mut lzfse_decomp = vec![0u8; data.len()];
    let mut lzfse_decomp_len = 0usize;
    let st_dec = ttzip_rust_lzfse_decompress_raw(
        lzfse_out.as_ptr(),
        lzfse_written,
        lzfse_decomp.as_mut_ptr(),
        lzfse_decomp.len(),
        data.len(),
        &mut lzfse_decomp_len,
    );
    assert_eq!(st_dec, TTZipStatus::Ok);
    assert_eq!(lzfse_decomp_len, data.len());
    assert_eq!(&lzfse_decomp[..], &data[..]);

    // 2. LZFSE Stream C-ABI
    let mut stream_out = vec![0u8; lzfse_bound + 1024];
    let mut stream_written = 0usize;
    let st_stream = ttzip_rust_lzfse_compress_stream(
        data.as_ptr(),
        data.len(),
        stream_out.as_mut_ptr(),
        stream_out.len(),
        &mut stream_written,
    );
    assert_eq!(st_stream, TTZipStatus::Ok);
    assert!(stream_written > 0);

    let mut stream_decomp = vec![0u8; data.len() + 128];
    let mut stream_decomp_len = 0usize;
    let st_stream_dec = ttzip_rust_lzfse_decompress_stream(
        stream_out.as_ptr(),
        stream_written,
        stream_decomp.as_mut_ptr(),
        stream_decomp.len(),
        &mut stream_decomp_len,
    );
    assert_eq!(st_stream_dec, TTZipStatus::Ok);
    assert_eq!(stream_decomp_len, data.len());
    assert_eq!(&stream_decomp[..stream_decomp_len], &data[..]);

    // 3. LZVN C-ABI
    let lzvn_bound = ttzip_rust_lzvn_compress_bound(data.len());
    let mut lzvn_out = vec![0u8; lzvn_bound];
    let mut lzvn_written = 0usize;
    let st_lzvn = ttzip_rust_lzvn_compress_raw(
        data.as_ptr(),
        data.len(),
        lzvn_out.as_mut_ptr(),
        lzvn_out.len(),
        &mut lzvn_written,
    );
    assert_eq!(st_lzvn, TTZipStatus::Ok);
    assert!(lzvn_written > 0);

    assert!(ttzip_rust_lzvn_validate(lzvn_out.as_ptr(), lzvn_written));

    let mut lzvn_decomp = vec![0u8; data.len()];
    let mut lzvn_decomp_len = 0usize;
    let st_lzvn_dec = ttzip_rust_lzvn_decompress_raw(
        lzvn_out.as_ptr(),
        lzvn_written,
        lzvn_decomp.as_mut_ptr(),
        lzvn_decomp.len(),
        data.len(),
        &mut lzvn_decomp_len,
    );
    assert_eq!(st_lzvn_dec, TTZipStatus::Ok);
    assert_eq!(lzvn_decomp_len, data.len());
    assert_eq!(&lzvn_decomp[..], &data[..]);

    // 4. Null pointer safety check
    let null_res = ttzip_rust_lzfse_compress_raw(
        data.as_ptr(),
        data.len(),
        lzfse_out.as_mut_ptr(),
        lzfse_out.len(),
        std::ptr::null_mut(),
    );
    assert_eq!(null_res, TTZipStatus::ErrInvalidParam);
}
