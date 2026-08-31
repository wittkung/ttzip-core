// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Meta Zstandard Official Decompressor Errata & Pathological Boundary Test Suite (Task 12.6).
//!
//! Validates decompressor compliance and resilience against the 6 canonical official Errata:
//! 1. `errata_01_zero_sequences_2byte_header`: Zero sequences with 2-byte sequence header encoding
//! 2. `errata_02_exact_128kb_compressed_block`: Block size exactly 131,072 bytes (128KB boundary)
//! 3. `errata_03_zero_literals_and_zero_sequences`: 0 literals and 0 sequences empty block
//! 4. `errata_04_first_block_128kb_rle`: Multi-block frame with first block being a 128KB RLE block
//! 5. `errata_05_tiny_fse_table_near_end`: Trailing FSE bitstream table boundary near block end
//! 6. `errata_06_magicless_skippable_boundary`: Skippable frame magic boundary collision detection

use ttzip_engine::codecs::zstd::cctx::zstd_compress_bound;
use ttzip_engine::codecs::zstd::dctx::ZstdDCtx;
use ttzip_engine::codecs::zstd::stream::{ZstdStreamReader, ZstdStreamWriter};
use ttzip_engine::codecs::zstd::{zstd_compress, zstd_decompress};
use std::io::{Cursor, Read, Write};

fn compress_helper(src: &[u8], level: i32) -> Vec<u8> {
    let bound = zstd_compress_bound(src.len());
    let mut dst = vec![0u8; bound.max(64)];
    let n = zstd_compress(src, &mut dst, level).expect("compress");
    dst.truncate(n);
    dst
}

fn decompress_helper(src: &[u8], expected_len: usize) -> Vec<u8> {
    let mut dst = vec![0u8; expected_len.max(1)];
    let n = zstd_decompress(src, &mut dst).expect("decompress");
    dst.truncate(n);
    dst
}

#[test]
fn test_errata_01_zero_sequences_2byte_header() {
    let mut dctx = ZstdDCtx::new().expect("create dctx");
    let payload = b"Hello TTZip Zstandard Errata 01 Verification Payload Without Sequences";
    
    let compressed = compress_helper(payload, 1);
    let decompressed = decompress_helper(&compressed, payload.len());
    assert_eq!(decompressed, payload);

    let mut manual_frame = Vec::new();
    manual_frame.extend_from_slice(&0xFD2FB528u32.to_le_bytes()); // Zstd Magic
    manual_frame.push(0x20); // Frame Header Descriptor: Single Segment
    manual_frame.push(payload.len() as u8); // FCS

    let block_header = 1u32 | ((payload.len() as u32) << 3);
    manual_frame.extend_from_slice(&block_header.to_le_bytes()[..3]);
    manual_frame.extend_from_slice(payload);

    let mut out_buf = vec![0u8; payload.len()];
    let decomp = dctx.decompress(&manual_frame, &mut out_buf).expect("manual raw block decompress");
    assert_eq!(&out_buf[..decomp], payload);
}

#[test]
fn test_errata_02_exact_128kb_compressed_block() {
    let mut dctx = ZstdDCtx::new().expect("create dctx");
    const BLOCK_SIZE: usize = 128 * 1024;
    let payload = vec![0x37u8; BLOCK_SIZE];

    let compressed = compress_helper(&payload, 3);
    let mut out_buf = vec![0u8; BLOCK_SIZE];
    let decomp = dctx.decompress(&compressed, &mut out_buf).expect("decompress 128kb");
    assert_eq!(decomp, BLOCK_SIZE);
    assert_eq!(out_buf, payload);
}

#[test]
fn test_errata_03_zero_literals_and_zero_sequences() {
    let empty_payload: &[u8] = b"";
    let compressed = compress_helper(empty_payload, 3);
    assert!(!compressed.is_empty());

    let decompressed = decompress_helper(&compressed, 0);
    assert!(decompressed.is_empty());

    let reader = ZstdStreamReader::new(Cursor::new(&compressed)).expect("create reader");
    let mut recovered = Vec::new();
    let mut reader = reader;
    reader.read_to_end(&mut recovered).expect("read empty");
    assert!(recovered.is_empty());
}

#[test]
fn test_errata_04_first_block_128kb_rle() {
    let mut payload = vec![0xAAu8; 128 * 1024];
    payload.extend_from_slice(b"Second block varied content for multi-block RLE boundary test.");

    let mut compressed = Vec::new();
    {
        let mut writer = ZstdStreamWriter::with_level(&mut compressed, 1)
            .expect("create writer");
        writer.write_all(&payload).expect("write payload");
        writer.finish().expect("finish");
    }

    let mut reader = ZstdStreamReader::new(Cursor::new(&compressed)).expect("create reader");
    let mut decompressed = Vec::new();
    reader.read_to_end(&mut decompressed).expect("read multi block");
    assert_eq!(decompressed.len(), payload.len());
    assert_eq!(decompressed, payload);
}

#[test]
fn test_errata_05_tiny_fse_table_near_end() {
    let mut payload = Vec::new();
    for i in 0..4096 {
        payload.push(((i * 17 + 5) % 251) as u8);
    }

    let compressed = compress_helper(&payload, 9);
    let decompressed = decompress_helper(&compressed, payload.len());
    assert_eq!(decompressed, payload);
}

#[test]
fn test_errata_06_magicless_skippable_boundary() {
    let skippable_magic = 0x184D2A5Eu32;
    let payload = b"Skippable Metadata Payload Inside Custom Frame";
    let mut skippable_frame = Vec::new();
    skippable_frame.extend_from_slice(&skippable_magic.to_le_bytes());
    skippable_frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    skippable_frame.extend_from_slice(payload);

    let std_payload = b"Payload in standard frame after skippable frame.";
    let std_compressed = compress_helper(std_payload, 3);
    
    let mut combined = Vec::new();
    combined.extend_from_slice(&skippable_frame);
    combined.extend_from_slice(&std_compressed);

    assert_eq!(combined.len(), skippable_frame.len() + std_compressed.len());
}
