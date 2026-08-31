// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive test suite for LZ4 16-byte Wildcopy SIMD vectorization and dual-layer safe decode loop.
//!
//! Validates:
//! 1. 100% Bit-Exact parity against canonical C-FFI `LZ4_decompress_safe` on arbitrary blocks.
//! 2. Extreme payload sizes (0B, 1B, 15B, 16B, 64B, 128B, 64KB, 1MB).
//! 3. Fast Loop and Safe Boundary Convergence Loop boundary transition dynamics.
//! 4. Small offset replication patterns (offsets 1..=8, RLE runs, short period waves).
//! 5. Direct SIMD primitive verification (`wild_copy_16`, `wild_copy_32`, `wild_copy_8`, `copy_small_offset_ptr`).
//! 6. Strict defensive boundary gates: truncated streams, zero offset, out-of-bounds offset, cascade overflows,
//!    and insufficient output capacity.

use ttzip_engine::codecs::lz4::{
    copy_small_offset_ptr, lz4_compress_bound, lz4_compress_fast, lz4_compress_hc, lz4_decompress,
    lz4_decompress_custom_to_vec, lz4_decompress_safe_custom, wild_copy_16, wild_copy_32,
    wild_copy_8, LZ4_FAST_LOOP_MARGIN, LZ4_MIN_MATCH,
};
use ttzip_engine::types::TTZipStatus;

// MARK: - 1. Wildcopy SIMD Vector Primitives Direct Verification

#[test]
fn test_wild_copy_16_aligned_and_unaligned() {
    let mut src = [0u8; 64];
    for i in 0..64 {
        src[i] = (i as u8).wrapping_mul(7).wrapping_add(3);
    }

    let mut dst = [0u8; 64];
    unsafe {
        // Copy 32 bytes using 16-byte chunks
        wild_copy_16(dst.as_mut_ptr(), src.as_ptr(), dst.as_mut_ptr().add(32));
    }
    assert_eq!(&dst[..32], &src[..32]);

    // Unaligned test: offset by 3 bytes
    let mut dst_unaligned = [0u8; 64];
    unsafe {
        wild_copy_16(
            dst_unaligned.as_mut_ptr().add(3),
            src.as_ptr().add(1),
            dst_unaligned.as_mut_ptr().add(35),
        );
    }
    assert_eq!(&dst_unaligned[3..35], &src[1..33]);
}

#[test]
fn test_wild_copy_32_and_8() {
    let mut src = [0u8; 128];
    for i in 0..128 {
        src[i] = (i as u8).wrapping_mul(13);
    }

    let mut dst32 = [0u8; 128];
    unsafe {
        wild_copy_32(dst32.as_mut_ptr(), src.as_ptr(), dst32.as_mut_ptr().add(64));
    }
    assert_eq!(&dst32[..64], &src[..64]);

    let mut dst8 = [0u8; 128];
    unsafe {
        wild_copy_8(dst8.as_mut_ptr(), src.as_ptr(), dst8.as_mut_ptr().add(40));
    }
    assert_eq!(&dst8[..40], &src[..40]);
}

#[test]
fn test_copy_small_offset_ptr_patterns() {
    // Offset 1 (RLE byte broadcast)
    let mut buf1 = [0u8; 32];
    buf1[0] = 0x7E;
    unsafe {
        copy_small_offset_ptr(buf1.as_mut_ptr().add(1), buf1.as_ptr(), 31, 1);
    }
    assert_eq!(buf1, [0x7E; 32]);

    // Offset 2 (2-byte periodic pattern)
    let mut buf2 = [0u8; 18];
    buf2[0] = 0xAA;
    buf2[1] = 0x55;
    unsafe {
        copy_small_offset_ptr(buf2.as_mut_ptr().add(2), buf2.as_ptr(), 16, 2);
    }
    for i in 0..18 {
        let expected = if (i % 2) == 0 { 0xAA } else { 0x55 };
        assert_eq!(buf2[i], expected, "Mismatch at index {i}");
    }

    // Offset 4 (4-byte periodic pattern)
    let mut buf4 = [0u8; 20];
    buf4[0..4].copy_from_slice(&[10, 20, 30, 40]);
    unsafe {
        copy_small_offset_ptr(buf4.as_mut_ptr().add(4), buf4.as_ptr(), 16, 4);
    }
    for i in 0..20 {
        let expected = match i % 4 {
            0 => 10,
            1 => 20,
            2 => 30,
            _ => 40,
        };
        assert_eq!(buf4[i], expected, "Mismatch at index {i}");
    }

    // Offset 3 (odd periodic pattern)
    let mut buf3 = [0u8; 15];
    buf3[0..3].copy_from_slice(&[0x11, 0x22, 0x33]);
    unsafe {
        copy_small_offset_ptr(buf3.as_mut_ptr().add(3), buf3.as_ptr(), 12, 3);
    }
    for i in 0..15 {
        let expected = match i % 3 {
            0 => 0x11,
            1 => 0x22,
            _ => 0x33,
        };
        assert_eq!(buf3[i], expected, "Mismatch at index {i}");
    }
}

// MARK: - 2. Differential Parity Tests vs Canonical C-FFI LZ4_decompress_safe

fn verify_differential_roundtrip(data: &[u8], acceleration: i32) {
    if data.is_empty() {
        let mut dst_custom = [0u8; 16];
        let res = lz4_decompress_safe_custom(&[], &mut dst_custom).expect("empty custom decompress");
        assert_eq!(res, 0);
        return;
    }

    let mut comp = vec![0u8; lz4_compress_bound(data.len())];
    let comp_len = lz4_compress_fast(data, &mut comp, acceleration).expect("lz4 compress fast");
    let comp_slice = &comp[..comp_len];

    // Reference C-FFI decompression
    let mut c_decomp = vec![0u8; data.len()];
    let c_len = lz4_decompress(comp_slice, &mut c_decomp).expect("c-ffi lz4 decompress");
    assert_eq!(c_len, data.len());
    assert_eq!(&c_decomp, data);

    // Custom SIMD Wildcopy decompression
    let mut custom_decomp = vec![0u8; data.len()];
    let custom_len = lz4_decompress_safe_custom(comp_slice, &mut custom_decomp)
        .expect("custom simd lz4 decompress");
    assert_eq!(custom_len, data.len());
    assert_eq!(
        &custom_decomp, &c_decomp,
        "Bit-exact parity violation against C-FFI LZ4"
    );

    // Vec helper verification
    let vec_decomp = lz4_decompress_custom_to_vec(comp_slice, data.len())
        .expect("custom to_vec decompress");
    assert_eq!(vec_decomp, data);
}

#[test]
fn test_extreme_sizes_differential_matrix() {
    // 0 Bytes
    verify_differential_roundtrip(&[], 1);

    // 1 Byte
    verify_differential_roundtrip(b"X", 1);

    // 15 Bytes (Token literal threshold)
    verify_differential_roundtrip(b"123456789012345", 1);

    // 16 Bytes (SIMD vector width boundary)
    verify_differential_roundtrip(b"1234567890123456", 1);

    // 64 Bytes (Fast Loop margin boundary)
    let payload_64 = (0..64).map(|i| (i * 3 + 1) as u8).collect::<Vec<_>>();
    verify_differential_roundtrip(&payload_64, 1);

    // 128 Bytes (Dual-layer transition boundary)
    let payload_128 = (0..128).map(|i| (i * 5 + 7) as u8).collect::<Vec<_>>();
    verify_differential_roundtrip(&payload_128, 1);

    // 64 KB (Standard LZ4 window size)
    let mut payload_64k = Vec::with_capacity(64 * 1024);
    let pattern = b"High-speed LZ4 SIMD 16B Wildcopy verification vector across 64KB block.";
    while payload_64k.len() + pattern.len() <= 64 * 1024 {
        payload_64k.extend_from_slice(pattern);
    }
    verify_differential_roundtrip(&payload_64k, 1);

    // 1 MB (Large payload stress)
    let mut payload_1mb = vec![0u8; 1024 * 1024];
    for (idx, byte) in payload_1mb.iter_mut().enumerate() {
        *byte = ((idx ^ (idx >> 8)) & 0xFF) as u8;
    }
    verify_differential_roundtrip(&payload_1mb, 1);
    verify_differential_roundtrip(&payload_1mb, 10);
}

#[test]
fn test_hc_compressed_differential_matrix() {
    let mut corpus = Vec::new();
    for i in 0..500 {
        corpus.extend_from_slice(
            format!("LZ4 HC compression stream token test iteration {i}: ABCDEFGHIJKLMNOPQRSTUVWXYZ\n").as_bytes(),
        );
    }

    for level in [1, 3, 6, 9, 12] {
        let mut comp = vec![0u8; lz4_compress_bound(corpus.len())];
        let c_len = lz4_compress_hc(&corpus, &mut comp, level).expect("hc compress");
        let comp_slice = &comp[..c_len];

        let mut out = vec![0u8; corpus.len()];
        let written = lz4_decompress_safe_custom(comp_slice, &mut out)
            .expect("custom decompress hc stream");
        assert_eq!(written, corpus.len());
        assert_eq!(out, corpus);
    }
}

// MARK: - 3. Small Offset Repetition and RLE Tests

#[test]
fn test_small_offsets_all_values() {
    for offset in 1..=8 {
        let mut payload = Vec::new();
        // Seed pattern of length `offset`
        for i in 0..offset {
            payload.push((0xA0 + i) as u8);
        }
        // Repeat for 2,000 bytes to trigger match cascades
        for _ in 0..2000 {
            let b = payload[payload.len() - offset];
            payload.push(b);
        }

        verify_differential_roundtrip(&payload, 1);
    }
}

#[test]
fn test_pure_rle_single_byte() {
    let rle_payload = vec![0x3C; 50_000];
    verify_differential_roundtrip(&rle_payload, 1);
}

// MARK: - 4. Fast Loop / Safe Loop Transition Dynamics

#[test]
fn test_transition_zone_boundary_exact_sizes() {
    // Tests sizes right around FAST_LOOP_MARGIN (64)
    for size in [60, 61, 62, 63, 64, 65, 66, 67, 70, 80, 100, 127, 128, 129] {
        let mut data = Vec::with_capacity(size);
        for i in 0..size {
            data.push((i % 251) as u8);
        }
        verify_differential_roundtrip(&data, 1);
    }
}

// MARK: - 5. Defensive Error Handling & Corrupt Payload Rejection

#[test]
fn test_reject_zero_offset() {
    // Manually construct an invalid block with offset == 0:
    // Token: 0x00 (0 literals, 0 match), followed by 2-byte offset 0x0000
    let invalid_zero_offset = [0x00, 0x00, 0x00];
    let mut dst = [0u8; 64];
    let res = lz4_decompress_safe_custom(&invalid_zero_offset, &mut dst);
    assert_eq!(res, Err(TTZipStatus::ErrInvalidOffset));
}

#[test]
fn test_reject_offset_larger_than_history() {
    // Token: 0x40 (4 literals, 0 match), 4 literal bytes, followed by offset = 10 (history is only 4)
    let invalid_offset = [0x40, b'A', b'B', b'C', b'D', 0x0A, 0x00];
    let mut dst = [0u8; 64];
    let res = lz4_decompress_safe_custom(&invalid_offset, &mut dst);
    assert_eq!(res, Err(TTZipStatus::ErrInvalidOffset));
}

#[test]
fn test_reject_truncated_literal_payload() {
    // Token: 0x80 (8 literals claimed), but only 3 bytes provided
    let truncated_literals = [0x80, b'A', b'B', b'C'];
    let mut dst = [0u8; 64];
    let res = lz4_decompress_safe_custom(&truncated_literals, &mut dst);
    assert_eq!(res, Err(TTZipStatus::ErrCorruptHeader));
}

#[test]
fn test_reject_truncated_offset() {
    // Token: 0x00 (0 literals, match follows), but only 1 byte for offset
    let truncated_offset = [0x00, 0x05];
    let mut dst = [0u8; 64];
    let res = lz4_decompress_safe_custom(&truncated_offset, &mut dst);
    assert_eq!(res, Err(TTZipStatus::ErrCorruptHeader));
}

#[test]
fn test_reject_truncated_cascade_sum() {
    // Token: 0xF0 (15+ literals), but input ends prematurely with 255
    let truncated_cascade = [0xF0, 255, 255];
    let mut dst = [0u8; 64];
    let res = lz4_decompress_safe_custom(&truncated_cascade, &mut dst);
    assert_eq!(res, Err(TTZipStatus::ErrCorruptHeader));
}

#[test]
fn test_reject_insufficient_output_capacity() {
    let source = b"Quick brown fox jumps over lazy dog, producing enough decompressed text.";
    let mut comp = vec![0u8; lz4_compress_bound(source.len())];
    let c_len = lz4_compress_fast(source, &mut comp, 1).expect("compress");

    // Attempt to decompress into an undersized destination buffer
    let mut small_dst = vec![0u8; source.len() - 10];
    let res = lz4_decompress_safe_custom(&comp[..c_len], &mut small_dst);
    assert!(res.is_err(), "Must reject undersized destination buffer");
}

#[test]
fn test_constants_and_module_sanity() {
    assert_eq!(LZ4_FAST_LOOP_MARGIN, 64);
    assert_eq!(LZ4_MIN_MATCH, 4);
}
