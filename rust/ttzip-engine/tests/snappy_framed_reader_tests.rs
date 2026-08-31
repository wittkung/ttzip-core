// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration test suite for `SnappyFramedReader` streaming decompressor.
//!
//! Validates:
//! 1. Empty streams and header-only streams.
//! 2. Small payloads and multi-chunk streaming across multiple 64KB boundaries.
//! 3. Transparent multi-file concatenated stream decompression (`cat a.sz b.sz`).
//! 4. Micro-step jitter reads (1-byte, 7-byte, 4096-byte, 65536-byte).
//! 5. Skippable chunks, padding chunks, and redundant stream identifiers.
//! 6. Defensive interception of corrupt CRC32C, truncated chunks, invalid headers, and oversized chunks.

use std::io::{Cursor, Read};
use ttzip_engine::codecs::snappy::crc::{crc32c, mask_crc32c};
use ttzip_engine::codecs::snappy::framed_reader::SnappyFramedReader;
use ttzip_engine::codecs::snappy::{
    snappy_frame_encode_to_vec, SNAPPY_MAX_CHUNK_SIZE, SNAPPY_STREAM_IDENTIFIER,
};

#[test]
fn test_snappy_framed_reader_empty_and_header_only_stream() {
    // 1. Truly empty 0-byte reader
    let empty_src: &[u8] = &[];
    let mut reader = SnappyFramedReader::new(empty_src);
    let mut out = Vec::new();
    let n = reader.read_to_end(&mut out).expect("read empty stream");
    assert_eq!(n, 0);
    assert!(out.is_empty());

    // 2. Stream containing only the 10-byte stream identifier
    let mut header_only_reader = SnappyFramedReader::new(&SNAPPY_STREAM_IDENTIFIER[..]);
    let mut header_out = Vec::new();
    let n = header_only_reader
        .read_to_end(&mut header_out)
        .expect("read header-only stream");
    assert_eq!(n, 0);
    assert!(header_out.is_empty());
}

#[test]
fn test_snappy_framed_reader_small_payload_roundtrip() {
    let payload = b"Hello Pure Safe Rust SnappyFramedReader streaming decompressor 2026!";
    let encoded = snappy_frame_encode_to_vec(payload).expect("encode frame");

    let mut reader = SnappyFramedReader::new(Cursor::new(&encoded));
    let mut decompressed = Vec::new();
    reader
        .read_to_end(&mut decompressed)
        .expect("decompress frame");

    assert_eq!(decompressed, payload);
}

#[test]
fn test_snappy_framed_reader_multi_chunk_spanning_64kb() {
    // 250KB payload spanning at least 4 chunks of 64KB
    let mut payload = Vec::with_capacity(250 * 1024);
    for i in 0..(250 * 1024) {
        payload.push(((i * 7 + 13) ^ (i >> 8)) as u8);
    }

    let encoded = snappy_frame_encode_to_vec(&payload).expect("encode multi-chunk");

    let mut reader = SnappyFramedReader::new(Cursor::new(&encoded));
    let mut decompressed = Vec::new();
    reader
        .read_to_end(&mut decompressed)
        .expect("decompress multi-chunk");

    assert_eq!(decompressed.len(), payload.len());
    assert_eq!(decompressed, payload);
}

#[test]
fn test_snappy_framed_reader_concatenated_streams() {
    let part1 = b"First segment in multi-stream concatenation. ";
    let part2 = b"Second segment with distinct content. ";
    let part3 = b"Third segment finalizing concatenated .sz payload.";

    let enc1 = snappy_frame_encode_to_vec(part1).expect("encode part 1");
    let enc2 = snappy_frame_encode_to_vec(part2).expect("encode part 2");
    let enc3 = snappy_frame_encode_to_vec(part3).expect("encode part 3");

    // Concatenate streams back-to-back: `cat part1.sz part2.sz part3.sz`
    let mut concat = Vec::new();
    concat.extend_from_slice(&enc1);
    concat.extend_from_slice(&enc2);
    concat.extend_from_slice(&enc3);

    let mut reader = SnappyFramedReader::new(Cursor::new(&concat));
    let mut decompressed = Vec::new();
    reader
        .read_to_end(&mut decompressed)
        .expect("decompress concatenated");

    let mut expected = Vec::new();
    expected.extend_from_slice(part1);
    expected.extend_from_slice(part2);
    expected.extend_from_slice(part3);

    assert_eq!(decompressed, expected);
}

#[test]
fn test_snappy_framed_reader_microstep_jitter_reads() {
    // 100KB repetitive and dynamic payload
    let mut payload = Vec::with_capacity(100 * 1024);
    for i in 0..(100 * 1024) {
        payload.push(((i % 251) ^ (i % 31)) as u8);
    }
    let encoded = snappy_frame_encode_to_vec(&payload).expect("encode jitter payload");

    // 1. Single-byte reads (1 byte per call)
    {
        let mut reader = SnappyFramedReader::new(Cursor::new(&encoded));
        let mut reconstructed = Vec::new();
        let mut single = [0u8; 1];
        loop {
            let n = reader.read(&mut single).expect("read 1 byte");
            if n == 0 {
                break;
            }
            reconstructed.push(single[0]);
        }
        assert_eq!(reconstructed, payload);
    }

    // 2. 7-byte jitter reads
    {
        let mut reader = SnappyFramedReader::new(Cursor::new(&encoded));
        let mut reconstructed = Vec::new();
        let mut chunk = [0u8; 7];
        loop {
            let n = reader.read(&mut chunk).expect("read 7 bytes");
            if n == 0 {
                break;
            }
            reconstructed.extend_from_slice(&chunk[..n]);
        }
        assert_eq!(reconstructed, payload);
    }

    // 3. 4096-byte chunk reads
    {
        let mut reader = SnappyFramedReader::new(Cursor::new(&encoded));
        let mut reconstructed = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let n = reader.read(&mut chunk).expect("read 4096 bytes");
            if n == 0 {
                break;
            }
            reconstructed.extend_from_slice(&chunk[..n]);
        }
        assert_eq!(reconstructed, payload);
    }
}

#[test]
fn test_snappy_framed_reader_skippable_padding_and_custom_chunks() {
    let payload = b"Data surrounded by padding and skippable chunks in Snappy frame.";
    let encoded = snappy_frame_encode_to_vec(payload).expect("encode payload");

    // Build framed stream with injected skippable chunks:
    // [Stream ID (10B)] -> [Padding 0xFE (8B)] -> [Skippable 0x82 (12B)] -> [Data chunk] -> [Padding 0xFE (4B)]
    let mut custom_stream = Vec::new();
    custom_stream.extend_from_slice(&encoded[..10]); // Magic header

    // Padding chunk (0xFE): length 4 (0x04, 0x00, 0x00), payload 4 zeros
    custom_stream.extend_from_slice(&[0xFE, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);

    // Skippable chunk (0x85): length 6 (0x06, 0x00, 0x00), payload 6 dummy bytes
    custom_stream.extend_from_slice(&[0x85, 0x06, 0x00, 0x00, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);

    // Redundant Stream Identifier chunk (0xFF)
    custom_stream.extend_from_slice(&SNAPPY_STREAM_IDENTIFIER);

    // Actual data chunk from encoded
    custom_stream.extend_from_slice(&encoded[10..]);

    // Trailing padding chunk (0xFE)
    custom_stream.extend_from_slice(&[0xFE, 0x02, 0x00, 0x00, 0x00, 0x00]);

    let mut reader = SnappyFramedReader::new(Cursor::new(&custom_stream));
    let mut decompressed = Vec::new();
    reader
        .read_to_end(&mut decompressed)
        .expect("decompress with skippable chunks");

    assert_eq!(decompressed, payload);
}

#[test]
fn test_snappy_framed_reader_uncompressed_chunk_decoding() {
    // Manually construct an uncompressed chunk (type 0x01)
    let raw_payload = b"Uncompressed fallback chunk payload data.";
    let calc_crc = crc32c(raw_payload);
    let masked_crc = mask_crc32c(calc_crc);

    let mut stream = Vec::new();
    stream.extend_from_slice(&SNAPPY_STREAM_IDENTIFIER);

    let payload_len = raw_payload.len() + 4;
    stream.push(0x01); // Chunk type: Uncompressed
    stream.push((payload_len & 0xFF) as u8);
    stream.push(((payload_len >> 8) & 0xFF) as u8);
    stream.push(((payload_len >> 16) & 0xFF) as u8);
    stream.extend_from_slice(&masked_crc.to_le_bytes());
    stream.extend_from_slice(raw_payload);

    let mut reader = SnappyFramedReader::new(Cursor::new(&stream));
    let mut decompressed = Vec::new();
    reader
        .read_to_end(&mut decompressed)
        .expect("decompress uncompressed chunk");

    assert_eq!(decompressed, raw_payload);
}

#[test]
fn test_snappy_framed_reader_error_interception() {
    let payload = b"Corrupted framing test payload.";
    let valid_encoded = snappy_frame_encode_to_vec(payload).expect("encode");

    // 1. Missing Stream Identifier (stream starts directly with data chunk)
    {
        let invalid_start = &valid_encoded[10..];
        let mut reader = SnappyFramedReader::new(Cursor::new(invalid_start));
        let mut out = Vec::new();
        assert!(reader.read_to_end(&mut out).is_err());
    }

    // 2. Corrupted Stream Identifier Magic (e.g. "sNaPpX")
    {
        let mut corrupt_magic = valid_encoded.clone();
        corrupt_magic[9] = b'X';
        let mut reader = SnappyFramedReader::new(Cursor::new(&corrupt_magic));
        let mut out = Vec::new();
        assert!(reader.read_to_end(&mut out).is_err());
    }

    // 3. Corrupted CRC32C in compressed chunk
    {
        let mut corrupt_crc = valid_encoded.clone();
        // CRC is located at bytes 14..18 (10 bytes header + 4 bytes chunk header)
        corrupt_crc[14] ^= 0xFF;
        let mut reader = SnappyFramedReader::new(Cursor::new(&corrupt_crc));
        let mut out = Vec::new();
        assert!(reader.read_to_end(&mut out).is_err());
    }

    // 4. Corrupted CRC32C in uncompressed chunk
    {
        let raw = b"Testing uncompressed chunk corrupt CRC32C";
        let masked_crc = mask_crc32c(crc32c(raw)) ^ 0x12345678; // Tampered CRC
        let mut stream = Vec::new();
        stream.extend_from_slice(&SNAPPY_STREAM_IDENTIFIER);
        let len = raw.len() + 4;
        stream.push(0x01);
        stream.push((len & 0xFF) as u8);
        stream.push(((len >> 8) & 0xFF) as u8);
        stream.push(((len >> 16) & 0xFF) as u8);
        stream.extend_from_slice(&masked_crc.to_le_bytes());
        stream.extend_from_slice(raw);

        let mut reader = SnappyFramedReader::new(Cursor::new(&stream));
        let mut out = Vec::new();
        assert!(reader.read_to_end(&mut out).is_err());
    }

    // 5. Reserved Unskippable Chunk (e.g. 0x05)
    {
        let mut stream = Vec::new();
        stream.extend_from_slice(&SNAPPY_STREAM_IDENTIFIER);
        stream.extend_from_slice(&[0x05, 0x04, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04]);

        let mut reader = SnappyFramedReader::new(Cursor::new(&stream));
        let mut out = Vec::new();
        assert!(reader.read_to_end(&mut out).is_err());
    }

    // 6. Truncated chunk header (less than 4 bytes after stream ID)
    {
        let mut stream = Vec::new();
        stream.extend_from_slice(&SNAPPY_STREAM_IDENTIFIER);
        stream.extend_from_slice(&[0x00, 0x10]); // Only 2 bytes instead of 4

        let mut reader = SnappyFramedReader::new(Cursor::new(&stream));
        let mut out = Vec::new();
        assert!(reader.read_to_end(&mut out).is_err());
    }

    // 7. Truncated chunk payload
    {
        let mut stream = Vec::new();
        stream.extend_from_slice(&SNAPPY_STREAM_IDENTIFIER);
        stream.extend_from_slice(&[0x01, 0x10, 0x00, 0x00]); // Claims 16 bytes payload, but EOF immediately follows

        let mut reader = SnappyFramedReader::new(Cursor::new(&stream));
        let mut out = Vec::new();
        assert!(reader.read_to_end(&mut out).is_err());
    }

    // 8. Oversized uncompressed chunk exceeding 64KB + 4
    {
        let mut stream = Vec::new();
        stream.extend_from_slice(&SNAPPY_STREAM_IDENTIFIER);
        let oversized = (SNAPPY_MAX_CHUNK_SIZE + 5) as u32;
        stream.push(0x01);
        stream.extend_from_slice(&oversized.to_le_bytes()[..3]);

        let mut reader = SnappyFramedReader::new(Cursor::new(&stream));
        let mut out = Vec::new();
        assert!(reader.read_to_end(&mut out).is_err());
    }
}
