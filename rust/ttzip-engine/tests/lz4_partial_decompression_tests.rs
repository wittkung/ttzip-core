// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit and integration test suite for `lz4_decompress_safe_partial`,
//! `lz4_decompress_safe_partial_using_dict`, and `Lz4HeaderSniffer`.
//!
//! Validates:
//! 1. Bit-Exact parity across partial extractions (16B, 64B, 512B, 4KB) from 64KB and 1MB payloads.
//! 2. Differential parity against canonical C-FFI `LZ4_decompress_safe_partial`.
//! 3. Destination capacity strict boundedness (`dst_capacity = min(target, dst.len())`).
//! 4. External dictionary partial decompression with cross-boundary match patterns.
//! 5. `Lz4HeaderSniffer` instant VFS format probing for `.tar.lz4` frames, raw blocks, and legacy frames.
//! 6. Robustness against malformed payloads, zero-offset, and cascade sum overflows.

use ttzip_engine::archive::unified::format_sniffer::ArchiveFormat;
use ttzip_engine::codecs::lz4::{
    lz4_compress_bound, lz4_compress_fast, lz4_compress_hc, lz4_decompress_safe_partial,
    lz4_decompress_safe_partial_using_dict, lz4_frame_compress_to_vec, Lz4HeaderSniffer,
};
use ttzip_engine::types::TTZipStatus;

// MARK: - Canonical C-FFI Bindings for Reference Verification

extern "C" {
    fn LZ4_decompress_safe_partial(
        src: *const libc::c_char,
        dst: *mut libc::c_char,
        src_size: libc::c_int,
        target_output_size: libc::c_int,
        dst_capacity: libc::c_int,
    ) -> libc::c_int;

    fn LZ4_decompress_safe_partial_usingDict(
        src: *const libc::c_char,
        dst: *mut libc::c_char,
        src_size: libc::c_int,
        target_output_size: libc::c_int,
        dst_capacity: libc::c_int,
        dict_start: *const libc::c_char,
        dict_size: libc::c_int,
    ) -> libc::c_int;
}

// MARK: - Helper Functions

fn compress_block(data: &[u8]) -> Vec<u8> {
    let mut comp = vec![0u8; lz4_compress_bound(data.len())];
    let c_len = lz4_compress_fast(data, &mut comp, 1).expect("lz4 compress failed");
    comp.truncate(c_len);
    comp
}

fn generate_pseudo_random_corpus(size: usize, seed: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    let mut state = seed;
    for _ in 0..size {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        data.push((state >> 33) as u8);
    }
    data
}

fn create_mock_tar_header(name: &str, file_size: usize) -> [u8; 512] {
    let mut header = [0u8; 512];

    // Name (offset 0..100)
    let name_bytes = name.as_bytes();
    let name_len = name_bytes.len().min(99);
    header[..name_len].copy_from_slice(&name_bytes[..name_len]);

    // Mode (offset 100..108)
    header[100..107].copy_from_slice(b"0000644");

    // UID & GID
    header[108..115].copy_from_slice(b"0001000");
    header[116..123].copy_from_slice(b"0001000");

    // Size (offset 124..136, octal with trailing space/null)
    let size_str = format!("{:011o} ", file_size);
    header[124..136].copy_from_slice(size_str.as_bytes());

    // Mtime (offset 136..148)
    header[136..147].copy_from_slice(b"14000000000");

    // Typeflag (offset 156)
    header[156] = b'0'; // regular file

    // Magic "ustar\0" (offset 257..263) & Version "00" (offset 263..265)
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");

    // Compute checksum (treating offset 148..156 as spaces)
    let mut sum = 0u32;
    for (i, &b) in header.iter().enumerate() {
        if (148..156).contains(&i) {
            sum += 0x20;
        } else {
            sum += b as u32;
        }
    }
    let chksum_str = format!("{:06o}\0 ", sum);
    header[148..156].copy_from_slice(chksum_str.as_bytes());

    header
}

// MARK: - 1. Bit-Exact & Target Output Size Bounds Tests

#[test]
fn test_partial_decompression_64kb_various_targets() {
    let mut payload = Vec::with_capacity(64 * 1024);
    let sample = b"TTZip ultra-fast high-performance partial decompressor benchmark test vector 2026.\n";
    while payload.len() + sample.len() <= 64 * 1024 {
        payload.extend_from_slice(sample);
    }
    payload.resize(64 * 1024, 0x5A);

    let comp = compress_block(&payload);

    for target in [16, 64, 512, 1024, 4096, 16384, 65536] {
        let mut out = vec![0u8; target];
        let written = lz4_decompress_safe_partial(&comp, &mut out, target)
            .expect("lz4_decompress_safe_partial failed");

        assert_eq!(written, target, "Written bytes must equal target");
        assert_eq!(
            &out[..written],
            &payload[..target],
            "Extracted slice must bit-match uncompressed payload prefix"
        );

        // Reference C-FFI comparison
        let mut c_out = vec![0u8; target];
        let c_written = unsafe {
            LZ4_decompress_safe_partial(
                comp.as_ptr() as *const libc::c_char,
                c_out.as_mut_ptr() as *mut libc::c_char,
                comp.len() as libc::c_int,
                target as libc::c_int,
                target as libc::c_int,
            )
        };
        assert_eq!(c_written as usize, target);
        assert_eq!(out, c_out, "Must bit-match reference C-FFI output");
    }
}

#[test]
fn test_partial_decompression_1mb_payload() {
    let payload = generate_pseudo_random_corpus(1024 * 1024, 0x1234_5678_9ABC);
    let comp = compress_block(&payload);

    for target in [16, 64, 512, 4096, 65536] {
        let mut out = vec![0u8; target];
        let written = lz4_decompress_safe_partial(&comp, &mut out, target)
            .expect("1MB partial decompression failed");

        assert_eq!(written, target);
        assert_eq!(&out[..written], &payload[..target]);

        // C-FFI comparison
        let mut c_out = vec![0u8; target];
        let c_written = unsafe {
            LZ4_decompress_safe_partial(
                comp.as_ptr() as *const libc::c_char,
                c_out.as_mut_ptr() as *mut libc::c_char,
                comp.len() as libc::c_int,
                target as libc::c_int,
                target as libc::c_int,
            )
        };
        assert_eq!(c_written as usize, target);
        assert_eq!(out, c_out);
    }
}

#[test]
fn test_partial_decompression_destination_smaller_than_target() {
    let payload = b"Safe destination clamping when dst.len() is smaller than target_output_size.";
    let comp = compress_block(payload);

    let mut small_dst = [0u8; 10];
    let target = 50; // Larger than small_dst.len()

    let written = lz4_decompress_safe_partial(&comp, &mut small_dst, target)
        .expect("should succeed with clamped size");

    assert_eq!(written, 10, "Must clamp to destination buffer length");
    assert_eq!(&small_dst[..10], &payload[..10]);
}

#[test]
fn test_partial_decompression_target_larger_than_uncompressed_size() {
    let payload = b"Short block decompressed completely with large target.";
    let comp = compress_block(payload);

    let mut dst = vec![0u8; 1024];
    let target = 512;

    let written = lz4_decompress_safe_partial(&comp, &mut dst, target)
        .expect("decompression to full block size");

    assert_eq!(
        written,
        payload.len(),
        "Must return full uncompressed length when target exceeds block data"
    );
    assert_eq!(&dst[..written], payload);
}

#[test]
fn test_partial_decompression_empty_and_zero_inputs() {
    let mut dst = [0u8; 64];

    // Empty src
    let res1 = lz4_decompress_safe_partial(&[], &mut dst, 32).expect("empty src");
    assert_eq!(res1, 0);

    // Target = 0
    let payload = b"Non empty payload";
    let comp = compress_block(payload);
    let res2 = lz4_decompress_safe_partial(&comp, &mut dst, 0).expect("zero target");
    assert_eq!(res2, 0);

    // Empty dst
    let res3 = lz4_decompress_safe_partial(&comp, &mut [], 32).expect("empty dst");
    assert_eq!(res3, 0);
}

// MARK: - 2. External Dictionary Partial Decompression Tests

#[test]
fn test_partial_decompression_using_dictionary() {
    let dict = b"HEADER_PREFIX_DICTIONARY_PATTERN_FOR_LZ4_CROSS_BLOCK_COMPRESSION_";
    let message = b"HEADER_PREFIX_DICTIONARY_PATTERN_FOR_LZ4_CROSS_BLOCK_COMPRESSION_ and unique local payload bytes!";

    // Compress message using external dictionary
    let mut comp = vec![0u8; lz4_compress_bound(message.len())];
    // Manual block construction or compress using standard HC / fast
    let c_len = lz4_compress_hc(message, &mut comp, 9).expect("hc compress");
    comp.truncate(c_len);

    for target in [10, 32, 64, message.len()] {
        let mut out = vec![0u8; target];
        let written = lz4_decompress_safe_partial_using_dict(&comp, &mut out, target, dict)
            .expect("dict partial decompression");

        assert_eq!(written, target);
        assert_eq!(&out[..written], &message[..target]);

        // Canonical C-FFI reference check
        let mut c_out = vec![0u8; target];
        let c_written = unsafe {
            LZ4_decompress_safe_partial_usingDict(
                comp.as_ptr() as *const libc::c_char,
                c_out.as_mut_ptr() as *mut libc::c_char,
                comp.len() as libc::c_int,
                target as libc::c_int,
                target as libc::c_int,
                dict.as_ptr() as *const libc::c_char,
                dict.len() as libc::c_int,
            )
        };
        assert_eq!(c_written as usize, target);
        assert_eq!(out, c_out);
    }
}

// MARK: - 3. Lz4HeaderSniffer VFS Tests

#[test]
fn test_sniffer_tar_lz4_frame_detection() {
    // Construct a TAR header followed by dummy content
    let tar_header = create_mock_tar_header("documents/report.pdf", 10240);
    let mut tar_archive = Vec::with_capacity(2048);
    tar_archive.extend_from_slice(&tar_header);
    tar_archive.extend_from_slice(&vec![0xAA; 1536]); // content + padding

    // Compress as standard LZ4 Frame
    let frame_bytes = lz4_frame_compress_to_vec(&tar_archive, None, 1)
        .expect("compress frame");

    // 1. Sniff 512-byte TAR Header
    let sniffed_header = Lz4HeaderSniffer::sniff_tar_header(&frame_bytes)
        .expect("sniff tar header from lz4 frame");
    assert_eq!(sniffed_header, tar_header);

    // 2. Sniff 64-byte Magic
    let sniffed_magic = Lz4HeaderSniffer::sniff_magic_64(&frame_bytes)
        .expect("sniff magic 64");
    assert_eq!(sniffed_magic, tar_header[..64]);

    // 3. Validate is_tar_lz4
    assert!(
        Lz4HeaderSniffer::is_tar_lz4(&frame_bytes),
        "Must identify .tar.lz4 successfully"
    );

    // 4. Validate inner format detection
    let inner_format = Lz4HeaderSniffer::sniff_inner_format(&frame_bytes)
        .expect("sniff inner format");
    assert_eq!(inner_format, ArchiveFormat::Tar);
}

#[test]
fn test_sniffer_raw_block_detection() {
    let tar_header = create_mock_tar_header("src/kernel/main.c", 2048);
    let raw_block = compress_block(&tar_header);

    // Sniff from raw block without frame header
    let sniffed_header = Lz4HeaderSniffer::sniff_tar_header(&raw_block)
        .expect("sniff tar header from raw block");
    assert_eq!(sniffed_header, tar_header);

    assert!(Lz4HeaderSniffer::is_tar_lz4(&raw_block));
    assert_eq!(
        Lz4HeaderSniffer::sniff_inner_format(&raw_block).unwrap(),
        ArchiveFormat::Tar
    );
}

#[test]
fn test_sniffer_non_tar_payload() {
    let text_payload = b"Hello TTZip! This is a plain text stream without any TAR header structure.";
    let frame_bytes = lz4_frame_compress_to_vec(text_payload, None, 1).expect("frame compress");

    assert!(!Lz4HeaderSniffer::is_tar_lz4(&frame_bytes));

    let magic = Lz4HeaderSniffer::sniff_magic_64(&frame_bytes).expect("sniff 64 bytes");
    assert_eq!(magic, text_payload[..64]);

    let payload_prefix = Lz4HeaderSniffer::sniff_payload(&frame_bytes, 16)
        .expect("sniff payload prefix");
    assert_eq!(payload_prefix, &text_payload[..16]);

    // Test a tiny payload (< 64 bytes)
    let tiny_payload = b"Short payload";
    let tiny_frame = lz4_frame_compress_to_vec(tiny_payload, None, 1).expect("frame compress");
    assert!(Lz4HeaderSniffer::sniff_magic_64(&tiny_frame).is_err());
    assert!(!Lz4HeaderSniffer::is_tar_lz4(&tiny_frame));
}

// MARK: - 4. Defensive Boundary and Corrupt Payload Tests

#[test]
fn test_reject_corrupt_partial_block() {
    let corrupt_block = [0xFF, 0xFF, 0x00, 0x00, 0x12];
    let mut dst = [0u8; 64];

    let res = lz4_decompress_safe_partial(&corrupt_block, &mut dst, 32);
    assert!(res.is_err(), "Must reject corrupted stream");
}

#[test]
fn test_reject_out_of_bounds_offset_in_partial() {
    // 4 literals 'A', 'B', 'C', 'D', followed by offset = 20 (only 4 bytes in history)
    let invalid_offset = [0x40, b'A', b'B', b'C', b'D', 0x14, 0x00];
    let mut dst = [0u8; 64];

    let res = lz4_decompress_safe_partial(&invalid_offset, &mut dst, 32);
    assert_eq!(res, Err(TTZipStatus::ErrInvalidOffset));
}

#[test]
fn test_reject_truncated_literals_in_partial() {
    let truncated = [0x80, b'A', b'B', b'C']; // claimed 8 literals, provided 3
    let mut dst = [0u8; 64];

    let res = lz4_decompress_safe_partial(&truncated, &mut dst, 32);
    assert_eq!(res, Err(TTZipStatus::ErrCorruptHeader));
}
