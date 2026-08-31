// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration tests for 15-stage streaming `Lz4FrameDecoder` and `Lz4FrameEncoder`.
//!
//! Tests include:
//! 1. Encoder/Decoder streaming roundtrip across diverse buffer sizes and compression levels.
//! 2. Concatenated multi-frame auto-continuation (3 concatenated frames of varying block sizes).
//! 3. Skippable metadata frame filtering and payload bypass (`0x184D2A50..=0x184D2A5F`).
//! 4. 64KB sliding dictionary cross-block back-reference integrity in Linked mode.
//! 5. Micro-buffer (1-byte, 7-byte, 64KB) reading and re-entrancy safety.
//! 6. Defensive checksum validation (Block Checksum and Content Checksum mismatch rejection).
//! 7. In-memory helper functions (`lz4_frame_compress`, `lz4_frame_decompress`, `lz4_frame_validate`).

use std::io::{Cursor, Read, Write};
use ttzip_engine::codecs::lz4::{
    is_lz4_frame_magic, is_lz4_skippable_magic, lz4_frame_compress, lz4_frame_compress_to_vec,
    lz4_frame_decompress, lz4_frame_decompress_to_vec, lz4_frame_validate,
    lz4_frame_validate_reader, BlockIndependence, BlockMaxSize, DStage, FrameDescriptor,
    Lz4FrameDecoder, Lz4FrameEncoder, LZ4F_MAGIC_SKIPPABLE_START,
};

// MARK: - Test 1: Basic Streaming Roundtrip

#[test]
fn test_lz4_frame_encoder_decoder_basic_roundtrip() {
    let payload = b"The quick brown fox jumps over the lazy dog. TTZip high-performance LZ4 streaming.";
    let mut compressed = Vec::new();

    {
        let mut encoder = Lz4FrameEncoder::new(&mut compressed).expect("create encoder");
        encoder.write_all(payload).expect("write payload");
        encoder.finish().expect("finish encoder");
    }

    assert!(compressed.len() > 4);
    let magic = u32::from_le_bytes([compressed[0], compressed[1], compressed[2], compressed[3]]);
    assert!(is_lz4_frame_magic(magic));

    let mut decompressed = Vec::new();
    let mut decoder = Lz4FrameDecoder::new(Cursor::new(&compressed));
    decoder
        .read_to_end(&mut decompressed)
        .expect("read to end");

    assert_eq!(decompressed, payload);
    assert_eq!(decoder.frames_decoded(), 1);
    assert_eq!(decoder.stage(), DStage::GetFrameHeader);
}

// MARK: - Test 2: Multi-Block Streaming with Content & Block Checksums

#[test]
fn test_lz4_frame_multiblock_with_checksums() {
    // Generate ~300 KB repetitive and unique data to force multiple 64 KB blocks
    let mut payload = Vec::with_capacity(300 * 1024);
    for i in 0..3000 {
        payload.extend_from_slice(
            format!("Block segment row {:05}: Structured payload in LZ4 Frame.\n", i).as_bytes(),
        );
    }

    let desc = FrameDescriptor {
        block_independence: BlockIndependence::Independent,
        block_checksum: true,
        content_checksum: true,
        content_size: Some(payload.len() as u64),
        dict_id: None,
        block_max_size: BlockMaxSize::Max64KB,
        version: 1,
    };

    let mut compressed = Vec::new();
    {
        let mut encoder =
            Lz4FrameEncoder::with_options(&mut compressed, desc, 1).expect("create encoder");
        encoder.write_all(&payload).expect("write payload");
        encoder.finish().expect("finish encoder");
    }

    let mut decompressed = Vec::new();
    let mut decoder = Lz4FrameDecoder::new(Cursor::new(&compressed));
    decoder
        .read_to_end(&mut decompressed)
        .expect("decompress");

    assert_eq!(decompressed, payload);
    assert_eq!(decoder.frames_decoded(), 1);
}

// MARK: - Test 3: Concatenated Multi-Frame Auto-Continuation

#[test]
fn test_lz4_frame_concatenated_three_frames() {
    let payload1 = b"Frame 1: First independent stream segment.\n";
    let payload2 = b"Frame 2: Second segment with 256KB block configuration.\n";
    let payload3 = b"Frame 3: Third segment terminating the stream cleanly.\n";

    let desc1 = FrameDescriptor {
        block_max_size: BlockMaxSize::Max64KB,
        content_checksum: true,
        ..Default::default()
    };
    let desc2 = FrameDescriptor {
        block_max_size: BlockMaxSize::Max256KB,
        content_checksum: false,
        ..Default::default()
    };
    let desc3 = FrameDescriptor {
        block_max_size: BlockMaxSize::Max1MB,
        content_checksum: true,
        ..Default::default()
    };

    let mut multi_frame_stream = Vec::new();

    // Encode Frame 1
    {
        let mut enc1 = Lz4FrameEncoder::with_options(&mut multi_frame_stream, desc1, 1).unwrap();
        enc1.write_all(payload1).unwrap();
        enc1.finish().unwrap();
    }

    // Encode Frame 2
    {
        let mut enc2 = Lz4FrameEncoder::with_options(&mut multi_frame_stream, desc2, 1).unwrap();
        enc2.write_all(payload2).unwrap();
        enc2.finish().unwrap();
    }

    // Encode Frame 3
    {
        let mut enc3 = Lz4FrameEncoder::with_options(&mut multi_frame_stream, desc3, 1).unwrap();
        enc3.write_all(payload3).unwrap();
        enc3.finish().unwrap();
    }

    // Decode all 3 concatenated frames in a single Lz4FrameDecoder pass
    let mut decompressed = Vec::new();
    let mut decoder = Lz4FrameDecoder::new(Cursor::new(&multi_frame_stream));
    decoder.read_to_end(&mut decompressed).unwrap();

    let mut expected = Vec::new();
    expected.extend_from_slice(payload1);
    expected.extend_from_slice(payload2);
    expected.extend_from_slice(payload3);

    assert_eq!(decompressed, expected);
    assert_eq!(decoder.frames_decoded(), 3);
}

// MARK: - Test 4: Skippable Metadata Frame Filtering and Bypass

#[test]
fn test_lz4_frame_skippable_metadata_filtering() {
    let payload1 = b"Data payload before skippable metadata frame.\n";
    let payload2 = b"Data payload after skippable metadata frame.\n";

    let mut stream = Vec::new();

    // 1. Frame 1
    {
        let mut enc = Lz4FrameEncoder::new(&mut stream).unwrap();
        enc.write_all(payload1).unwrap();
        enc.finish().unwrap();
    }

    // 2. Skippable Frame 1 (Magic = 0x184D2A50, Size = 12, Data = "USER_METADATA")
    let skippable_magic = LZ4F_MAGIC_SKIPPABLE_START;
    assert!(is_lz4_skippable_magic(skippable_magic));
    stream.extend_from_slice(&skippable_magic.to_le_bytes());
    let meta_payload = b"USER_METADATA";
    stream.extend_from_slice(&(meta_payload.len() as u32).to_le_bytes());
    stream.extend_from_slice(meta_payload);

    // 3. Frame 2
    {
        let mut enc = Lz4FrameEncoder::new(&mut stream).unwrap();
        enc.write_all(payload2).unwrap();
        enc.finish().unwrap();
    }

    // 4. Skippable Frame 2 (Magic = 0x184D2A5F, Size = 0)
    let skippable_magic_end = 0x184D_2A5F;
    assert!(is_lz4_skippable_magic(skippable_magic_end));
    stream.extend_from_slice(&skippable_magic_end.to_le_bytes());
    stream.extend_from_slice(&0u32.to_le_bytes());

    // Decompress and verify that skippable frames are cleanly skipped
    let mut decompressed = Vec::new();
    let mut decoder = Lz4FrameDecoder::new(Cursor::new(&stream));
    decoder.read_to_end(&mut decompressed).unwrap();

    let mut expected = Vec::new();
    expected.extend_from_slice(payload1);
    expected.extend_from_slice(payload2);

    assert_eq!(decompressed, expected);
    assert_eq!(decoder.frames_decoded(), 2);
}

#[test]
fn test_pure_lz4_block_compress_pattern() {
    let pattern = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!@#$%^&*()_+-=[]{};:,.<>?";
    let mut block0 = Vec::new();
    while block0.len() < 65536 {
        block0.extend_from_slice(pattern);
    }
    block0.truncate(65536);

    let mut comp = vec![0u8; 100000];
    let c_len = ttzip_engine::codecs::lz4::lz4_compress_fast(&block0, &mut comp, 1).unwrap();
    let mut decomp = vec![0u8; 65536];
    let d_len = ttzip_engine::codecs::lz4::lz4_decompress(&comp[..c_len], &mut decomp).unwrap();
    assert_eq!(d_len, 65536);
    assert_eq!(decomp, block0);
}

// MARK: - Test 5: 64KB Sliding Dictionary in Linked Mode

#[test]
fn test_lz4_frame_linked_blocks_sliding_dictionary() {
    // Generate data with heavy cross-block repetition
    let pattern = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!@#$%^&*()_+-=[]{};:,.<>?";
    let mut payload = Vec::new();
    for _ in 0..3000 {
        payload.extend_from_slice(pattern);
    }

    let desc = FrameDescriptor {
        block_independence: BlockIndependence::Linked,
        block_checksum: true,
        content_checksum: true,
        block_max_size: BlockMaxSize::Max64KB,
        ..Default::default()
    };

    let mut compressed = Vec::new();
    {
        let mut encoder =
            Lz4FrameEncoder::with_options(&mut compressed, desc, 1).expect("create linked encoder");
        encoder.write_all(&payload).expect("write payload");
        encoder.finish().expect("finish linked encoder");
    }

    let mut decompressed = Vec::new();
    let mut decoder = Lz4FrameDecoder::new(Cursor::new(&compressed));
    let mut chunk = [0u8; 8192];
    loop {
        match decoder.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => decompressed.extend_from_slice(&chunk[..n]),
            Err(e) => panic!("decompress linked error: {:?}", e),
        }
    }

    assert_eq!(decompressed, payload);
    assert_eq!(decoder.frames_decoded(), 1);
}

// MARK: - Test 6: Micro-Buffer 1-Byte Streaming Decompression

#[test]
fn test_lz4_frame_microbuffer_single_byte_reads() {
    let payload = b"Micro-buffer single-byte streaming decompression invariant validation test 2026.";
    let compressed = lz4_frame_compress_to_vec(payload, None, 1).expect("compress");

    let mut decoder = Lz4FrameDecoder::new(Cursor::new(&compressed));
    let mut single_byte_buf = [0u8; 1];
    let mut reconstructed = Vec::new();

    loop {
        match decoder.read(&mut single_byte_buf) {
            Ok(0) => break,
            Ok(1) => reconstructed.push(single_byte_buf[0]),
            Ok(n) => panic!("unexpected read size: {}", n),
            Err(e) => panic!("read error: {:?}", e),
        }
    }

    assert_eq!(reconstructed, payload);
    assert_eq!(decoder.frames_decoded(), 1);
}

// MARK: - Test 7: Micro-Buffer 7-Byte Streaming Reads

#[test]
fn test_lz4_frame_arbitrary_chunk_size_reads() {
    let mut payload = Vec::new();
    for i in 0..1000 {
        payload.extend_from_slice(format!("Arbitrary chunk line {:04}\n", i).as_bytes());
    }
    let compressed = lz4_frame_compress_to_vec(&payload, None, 1).expect("compress");

    let mut decoder = Lz4FrameDecoder::new(Cursor::new(&compressed));
    let mut chunk = [0u8; 7];
    let mut reconstructed = Vec::new();

    loop {
        match decoder.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => reconstructed.extend_from_slice(&chunk[..n]),
            Err(e) => panic!("chunk read error: {:?}", e),
        }
    }

    assert_eq!(reconstructed, payload);
}

// MARK: - Test 8: Checksum Mismatch Detection

#[test]
fn test_lz4_frame_corrupt_checksum_rejection() {
    let payload = b"Payload for checksum tampering detection test.";
    let desc = FrameDescriptor {
        content_checksum: true,
        block_checksum: true,
        ..Default::default()
    };
    let mut compressed = lz4_frame_compress_to_vec(payload, Some(&desc), 1).expect("compress");

    // Corrupt the last byte (content checksum)
    let last_idx = compressed.len() - 1;
    compressed[last_idx] ^= 0xFF;

    let mut decoder = Lz4FrameDecoder::new(Cursor::new(&compressed));
    let mut out = Vec::new();
    let res = decoder.read_to_end(&mut out);
    assert!(res.is_err());
}

// MARK: - Test 9: In-Memory Convenience Helpers

#[test]
fn test_lz4_frame_convenience_helpers() {
    let payload = b"Convenience helpers verification for LZ4 frame codec.";
    let mut compressed = vec![0u8; payload.len() + 128];
    let written =
        lz4_frame_compress(payload, &mut compressed, None, 1).expect("lz4_frame_compress");
    assert!(written > 0);

    // Validate
    assert!(lz4_frame_validate(&compressed[..written]));
    assert!(lz4_frame_validate_reader(&mut Cursor::new(&compressed[..written])));

    // Decompress
    let mut decompressed = vec![0u8; payload.len()];
    let d_len = lz4_frame_decompress(&compressed[..written], &mut decompressed)
        .expect("lz4_frame_decompress");
    assert_eq!(d_len, payload.len());
    assert_eq!(&decompressed[..d_len], payload);

    // Decompress to vec
    let vec_out = lz4_frame_decompress_to_vec(&compressed[..written], 1024 * 1024)
        .expect("lz4_frame_decompress_to_vec");
    assert_eq!(vec_out, payload);
}

// MARK: - Test 10: Empty Stream Roundtrip

#[test]
fn test_lz4_frame_empty_payload() {
    let payload = b"";
    let compressed = lz4_frame_compress_to_vec(payload, None, 1).expect("compress empty");
    assert!(compressed.len() >= 4);

    let mut decoder = Lz4FrameDecoder::new(Cursor::new(&compressed));
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).expect("decompress empty");
    assert_eq!(out, payload);
    assert_eq!(decoder.frames_decoded(), 1);
}
