// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration tests for XzStreamDecoder, XzSeekableReader,
//! MultiBlock decompression state machine, and adversarial corruption defense.

use sha2::{Digest, Sha256};
use std::io::{Cursor, Read, Seek, SeekFrom};

use ttzip_engine::xz::decoder::{xz_decompress, XzStreamDecoder};
use ttzip_engine::xz::seekable::XzSeekableReader;
use ttzip_engine::xz::types::XzCheckType;
use ttzip_engine::xz::writer::{
    xz_compress, XzBcjType, XzEncoderOptions, XzParallelStreamWriter,
};

/// Helper to compute standard SHA-256 hexadecimal string of slice.
fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Helper to generate deterministic synthetic payload.
fn generate_deterministic_payload(size: usize, seed: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(size);
    let mut state = seed;
    for i in 0..size {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        let byte = ((state >> 16) ^ (i as u32)) as u8;
        out.push(byte);
    }
    out
}

#[test]
fn test_single_block_roundtrip_all_checksum_types() {
    let check_types = [
        XzCheckType::None,
        XzCheckType::Crc32,
        XzCheckType::Crc64,
        XzCheckType::Sha256,
    ];

    let original = generate_deterministic_payload(128 * 1024, 0x1337BEEF);
    let expected_hash = compute_sha256(&original);

    for &check_type in &check_types {
        let options = XzEncoderOptions::new()
            .with_check_type(check_type)
            .with_dict_size(4 * 1024 * 1024)
            .with_preset_level(3);

        let compressed = xz_compress(&original, &options).expect("xz_compress failed");
        assert!(compressed.len() >= 24);

        // 1. Decompress via streaming XzStreamDecoder
        let mut decoder = XzStreamDecoder::new(Cursor::new(&compressed));
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .expect("streaming decode failed");

        assert_eq!(decompressed.len(), original.len());
        assert_eq!(compute_sha256(&decompressed), expected_hash);
        assert_eq!(decoder.cumulative_records().len(), 1);

        // 2. Decompress via convenience function xz_decompress
        let quick_decomp = xz_decompress(&compressed).expect("xz_decompress failed");
        assert_eq!(quick_decomp, original);
    }
}

#[test]
fn test_multi_block_parallel_stream_decoding() {
    // 512 KB payload split into four 128 KB blocks
    let original = generate_deterministic_payload(512 * 1024, 0xCAFEBABE);
    let expected_hash = compute_sha256(&original);

    let options = XzEncoderOptions::new()
        .with_check_type(XzCheckType::Crc64)
        .with_dict_size(1024 * 1024)
        .with_block_size(128 * 1024)
        .with_preset_level(2);

    let mut compressed_sink = Vec::new();
    let mut writer = XzParallelStreamWriter::new(&mut compressed_sink, options)
        .expect("create parallel writer");
    writer.write_parallel(&original).expect("write parallel");
    writer.finish().expect("finish parallel writer");

    assert!(compressed_sink.len() > 100);

    let mut decoder = XzStreamDecoder::new(Cursor::new(&compressed_sink));
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .expect("multi block stream decode failed");

    assert_eq!(decompressed.len(), original.len());
    assert_eq!(compute_sha256(&decompressed), expected_hash);
    assert_eq!(decoder.cumulative_records().len(), 4);
}

#[test]
fn test_multi_block_with_bcj_x86_filter() {
    let mut code_data = generate_deterministic_payload(256 * 1024, 0xDEADC0DE);
    // Inject x86 CALL instructions
    for i in (0..code_data.len().saturating_sub(5)).step_by(64) {
        code_data[i] = 0xE8;
        code_data[i + 1] = 0x10;
        code_data[i + 2] = 0x20;
        code_data[i + 3] = 0x00;
        code_data[i + 4] = 0x00;
    }
    let expected_hash = compute_sha256(&code_data);

    let options = XzEncoderOptions::new()
        .with_check_type(XzCheckType::Crc32)
        .with_dict_size(2 * 1024 * 1024)
        .with_bcj(XzBcjType::X86)
        .with_block_size(64 * 1024);

    let compressed = xz_compress(&code_data, &options).expect("compress with BCJ x86");
    let decompressed = xz_decompress(&compressed).expect("decompress with BCJ x86");

    assert_eq!(decompressed.len(), code_data.len());
    assert_eq!(compute_sha256(&decompressed), expected_hash);
}

#[test]
fn test_stream_padding_and_concatenated_multi_stream() {
    let stream1_payload = b"First XZ Stream content payload representing file A.".to_vec();
    let stream2_payload = b"Second XZ Stream content payload representing file B.".to_vec();

    let opt1 = XzEncoderOptions::new().with_check_type(XzCheckType::Crc32);
    let opt2 = XzEncoderOptions::new().with_check_type(XzCheckType::Crc64);

    let comp1 = xz_compress(&stream1_payload, &opt1).expect("compress stream 1");
    let comp2 = xz_compress(&stream2_payload, &opt2).expect("compress stream 2");
    println!("comp1 len={}: {:02x?}", comp1.len(), comp1);

    let d1 = xz_decompress(&comp1).expect("decompress stream 1 standalone");
    assert_eq!(d1, stream1_payload);
    let d2 = xz_decompress(&comp2).expect("decompress stream 2 standalone");
    assert_eq!(d2, stream2_payload);

    // Concatenate: Stream 1 + 8 bytes of 0x00 Stream Padding + Stream 2 + 12 bytes of Stream Padding
    let mut chained = Vec::new();
    chained.extend_from_slice(&comp1);
    chained.extend_from_slice(&[0x00; 8]); // 2 x 4-byte padding units
    chained.extend_from_slice(&comp2);
    chained.extend_from_slice(&[0x00; 12]); // 3 x 4-byte padding units

    let mut decoder = XzStreamDecoder::new(Cursor::new(&chained));
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .expect("multi stream chained decode");

    let mut expected = Vec::new();
    expected.extend_from_slice(&stream1_payload);
    expected.extend_from_slice(&stream2_payload);

    assert_eq!(decompressed, expected);
}

#[test]
fn test_seekable_reader_random_access_and_reversals() {
    // 12 MB total data across 12 blocks of 1 MB each
    let block_count = 12;
    let block_size = 1024 * 1024;
    let total_size = block_count * block_size;
    let original = generate_deterministic_payload(total_size, 0x5EE1AB1E);

    let options = XzEncoderOptions::new()
        .with_check_type(XzCheckType::Crc32)
        .with_dict_size(1024 * 1024)
        .with_block_size(block_size);

    let mut comp_sink = Vec::new();
    let mut writer = XzParallelStreamWriter::new(&mut comp_sink, options).expect("writer");
    writer.write_parallel(&original).expect("write parallel");
    writer.finish().expect("finish");

    let cursor = Cursor::new(comp_sink);
    let mut seekable = XzSeekableReader::new(cursor).expect("create seekable reader");

    assert_eq!(seekable.total_uncompressed_size(), total_size as u64);
    assert_eq!(seekable.index().records.len(), block_count);

    // 1. Sequential read from start (first 100 bytes)
    let mut head = vec![0u8; 100];
    seekable.read_exact(&mut head).expect("read head");
    assert_eq!(&head, &original[0..100]);
    assert_eq!(seekable.current_position(), 100);

    // 2. Skip to 10 MB (offset 10 * 1024 * 1024)
    let offset_10mb = 10 * 1024 * 1024;
    let seek_res = seekable
        .seek(SeekFrom::Start(offset_10mb as u64))
        .expect("seek to 10MB");
    assert_eq!(seek_res, offset_10mb as u64);

    let mut buf_4k = vec![0u8; 4096];
    seekable.read_exact(&mut buf_4k).expect("read 4KB at 10MB");
    assert_eq!(
        &buf_4k,
        &original[offset_10mb..offset_10mb + 4096],
        "Fidelity failure reading at 10MB"
    );

    // 3. Reverse seek: step back 2048 bytes
    let rev_res = seekable
        .seek(SeekFrom::Current(-2048))
        .expect("reverse seek 2KB");
    assert_eq!(rev_res, (offset_10mb + 4096 - 2048) as u64);

    let mut buf_2k = vec![0u8; 2048];
    seekable.read_exact(&mut buf_2k).expect("read 2KB");
    assert_eq!(
        &buf_2k,
        &original[offset_10mb + 2048..offset_10mb + 4096],
        "Fidelity failure on reverse seek read"
    );

    // 4. Seek relative to End: read last 1024 bytes
    let end_res = seekable
        .seek(SeekFrom::End(-1024))
        .expect("seek from end");
    assert_eq!(end_res, (total_size - 1024) as u64);

    let mut tail = vec![0u8; 1024];
    seekable.read_exact(&mut tail).expect("read tail 1KB");
    assert_eq!(&tail, &original[total_size - 1024..total_size]);

    // 5. Seek beyond EOF returns Ok(0) on read
    seekable
        .seek(SeekFrom::Start(total_size as u64 + 500))
        .expect("seek beyond EOF");
    let mut eof_buf = [0u8; 16];
    let n = seekable.read(&mut eof_buf).expect("read at EOF");
    assert_eq!(n, 0);
}

#[test]
fn test_corrupted_checksum_immediate_rejection() {
    let original = b"TTZip Integrity Verification Checksum Hard-Gate Payload".to_vec();
    let options = XzEncoderOptions::new().with_check_type(XzCheckType::Crc32);
    let mut compressed = xz_compress(&original, &options).expect("compress");

    // The stream header is 12 bytes. Block header is 8 bytes.
    // Flip a byte in the compressed block data or check
    let mid_idx = compressed.len() - 20;
    compressed[mid_idx] ^= 0xFF;

    let res = xz_decompress(&compressed);
    assert!(res.is_err(), "Corrupted payload must return an Err, not panic");
}

#[test]
fn test_corrupted_header_magic_interception() {
    let original = b"Magic bytes corruption test".to_vec();
    let options = XzEncoderOptions::new();
    let mut compressed = xz_compress(&original, &options).expect("compress");

    // Corrupt magic header byte
    compressed[0] = 0x00;

    let mut decoder = XzStreamDecoder::new(Cursor::new(&compressed));
    let mut out = Vec::new();
    let res = decoder.read_to_end(&mut out);

    assert!(res.is_err(), "Invalid header magic must be intercepted");
}

#[test]
fn test_corrupted_footer_backward_size_interception() {
    let original = generate_deterministic_payload(64 * 1024, 0x11223344);
    let options = XzEncoderOptions::new().with_check_type(XzCheckType::Crc32);
    let mut compressed = xz_compress(&original, &options).expect("compress");

    // Corrupt backward size in Stream Footer (stored at footer offset 4..8)
    let footer_start = compressed.len() - 12;
    compressed[footer_start + 4] ^= 0x55;

    // CRC of footer will mismatch
    let mut decoder = XzStreamDecoder::new(Cursor::new(&compressed));
    let mut out = Vec::new();
    let res = decoder.read_to_end(&mut out);

    assert!(res.is_err(), "Corrupted footer CRC / backward size must be rejected");
}

#[test]
fn test_truncated_stream_zero_panic_defense() {
    let original = generate_deterministic_payload(32 * 1024, 0xAABBCCDD);
    let options = XzEncoderOptions::new();
    let compressed = xz_compress(&original, &options).expect("compress");

    // Truncate at various arbitrary byte boundaries
    let cutoffs = [0, 4, 11, 12, 20, compressed.len() / 2, compressed.len() - 5];
    for &cutoff in &cutoffs {
        let truncated = &compressed[..cutoff];
        let res = xz_decompress(truncated);
        assert!(res.is_err(), "Truncated stream at cutoff {cutoff} must return Err without panic");

        // Also test seekable reader with truncated buffer
        let seek_res = XzSeekableReader::new(Cursor::new(truncated));
        assert!(seek_res.is_err(), "Seekable reader must reject truncated stream at cutoff {cutoff}");
    }
}
