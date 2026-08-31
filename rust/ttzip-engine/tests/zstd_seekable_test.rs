// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Exhaustive integration and conformance tests for Zstandard Seekable Format (`ZstdSeekable`).

use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use ttzip_engine::codecs::zstd_seekable::{
    SeekTableDecoder, SeekTableEncoder, SeekableError, ZstdSeekableReader, ZstdSeekableWriter,
    SEEKABLE_FOOTER_SIZE, SEEKABLE_MAGIC_NUMBER, SEEK_TABLE_FLAG_CHECKSUM, SKIPPABLE_HEADER_SIZE,
    SKIPPABLE_MAGIC_NUMBER,
};

#[test]
fn test_seekable_constants_and_magic_numbers() {
    assert_eq!(SEEKABLE_MAGIC_NUMBER, 0x8F92EAB1);
    assert_eq!(SKIPPABLE_MAGIC_NUMBER, 0x184D2A5E);
    assert_eq!(SEEKABLE_FOOTER_SIZE, 9);
    assert_eq!(SKIPPABLE_HEADER_SIZE, 8);
    assert_eq!(SEEK_TABLE_FLAG_CHECKSUM, 0x80);
}

#[test]
fn test_seek_table_encoder_and_decoder_roundtrip_no_checksum() {
    let mut encoder = SeekTableEncoder::new(false);
    encoder.add_frame(100, 500, None).expect("add frame 0");
    encoder.add_frame(120, 600, None).expect("add frame 1");
    encoder.add_frame(80, 400, None).expect("add frame 2");

    assert_eq!(encoder.frame_count(), 3);
    assert_eq!(encoder.total_compressed_size(), 300);
    assert_eq!(encoder.total_decompressed_size(), 1500);

    let serialized = encoder.serialize_to_vec();
    // Header (8) + Entries (3 * 8 = 24) + Footer (9) = 41 bytes
    assert_eq!(serialized.len(), 8 + 24 + 9);

    // Dummy compressed payload prefix
    let mut full_archive = vec![0u8; 300];
    full_archive.extend_from_slice(&serialized);

    let decoder = SeekTableDecoder::parse_from_slice(&full_archive).expect("parse seek table");
    assert_eq!(decoder.frame_count(), 3);
    assert_eq!(decoder.total_compressed_size(), 300);
    assert_eq!(decoder.total_decompressed_size(), 1500);
    assert!(!decoder.has_checksums());

    let f0 = decoder.get_frame(0).unwrap();
    assert_eq!(f0.c_offset, 0);
    assert_eq!(f0.c_size, 100);
    assert_eq!(f0.d_offset, 0);
    assert_eq!(f0.d_size, 500);

    let f1 = decoder.get_frame(1).unwrap();
    assert_eq!(f1.c_offset, 100);
    assert_eq!(f1.c_size, 120);
    assert_eq!(f1.d_offset, 500);
    assert_eq!(f1.d_size, 600);

    let f2 = decoder.get_frame(2).unwrap();
    assert_eq!(f2.c_offset, 220);
    assert_eq!(f2.c_size, 80);
    assert_eq!(f2.d_offset, 1100);
    assert_eq!(f2.d_size, 400);

    assert!(decoder.get_frame(3).is_none());
}

#[test]
fn test_seek_table_encoder_and_decoder_roundtrip_with_checksum() {
    let mut encoder = SeekTableEncoder::new(true);
    encoder
        .add_frame(200, 1000, Some(0xAABBCCDD))
        .expect("add frame 0");
    encoder
        .add_frame(250, 1200, Some(0x11223344))
        .expect("add frame 1");

    assert_eq!(encoder.frame_count(), 2);
    let serialized = encoder.serialize_to_vec();
    // Header (8) + Entries (2 * 12 = 24) + Footer (9) = 41 bytes
    assert_eq!(serialized.len(), 8 + 24 + 9);

    let mut full_archive = vec![0u8; 450];
    full_archive.extend_from_slice(&serialized);

    let decoder = SeekTableDecoder::parse_from_slice(&full_archive).expect("parse seek table");
    assert!(decoder.has_checksums());
    assert_eq!(decoder.get_frame(0).unwrap().checksum, Some(0xAABBCCDD));
    assert_eq!(decoder.get_frame(1).unwrap().checksum, Some(0x11223344));
}

#[test]
fn test_binary_search_offset_to_frame_index() {
    let mut encoder = SeekTableEncoder::new(false);
    // Frame 0: [0, 100)
    encoder.add_frame(50, 100, None).unwrap();
    // Frame 1: [100, 300)
    encoder.add_frame(80, 200, None).unwrap();
    // Frame 2: [300, 600)
    encoder.add_frame(120, 300, None).unwrap();

    let mut archive = vec![0u8; 250];
    archive.extend_from_slice(&encoder.serialize_to_vec());

    let decoder = SeekTableDecoder::parse_from_slice(&archive).unwrap();
    assert_eq!(decoder.offset_to_frame_index(0), Some(0));
    assert_eq!(decoder.offset_to_frame_index(50), Some(0));
    assert_eq!(decoder.offset_to_frame_index(99), Some(0));
    assert_eq!(decoder.offset_to_frame_index(100), Some(1));
    assert_eq!(decoder.offset_to_frame_index(299), Some(1));
    assert_eq!(decoder.offset_to_frame_index(300), Some(2));
    assert_eq!(decoder.offset_to_frame_index(599), Some(2));
    assert_eq!(decoder.offset_to_frame_index(600), None);
    assert_eq!(decoder.offset_to_frame_index(1000), None);
}

#[test]
fn test_seek_table_corrupted_magic_and_size_validation() {
    let mut encoder = SeekTableEncoder::new(false);
    encoder.add_frame(10, 20, None).unwrap();
    let serialized = encoder.serialize_to_vec();

    // 1. Archive too small
    let err = SeekTableDecoder::parse_from_slice(&serialized[..10]).unwrap_err();
    assert!(matches!(err, SeekableError::ArchiveTooSmall(_)));

    // 2. Corrupt Seekable Magic in footer (last 4 bytes)
    let mut bad_magic = serialized.clone();
    let len = bad_magic.len();
    bad_magic[len - 4..].copy_from_slice(&0xDEADBEEFu32.to_le_bytes());
    let err = SeekTableDecoder::parse_from_slice(&bad_magic).unwrap_err();
    assert!(matches!(err, SeekableError::InvalidSeekableMagic { .. }));

    // 3. Corrupt Skippable Magic in header (first 4 bytes)
    let mut bad_skippable = serialized.clone();
    bad_skippable[0..4].copy_from_slice(&0x12345678u32.to_le_bytes());
    let err = SeekTableDecoder::parse_from_slice(&bad_skippable).unwrap_err();
    assert!(matches!(err, SeekableError::InvalidSkippableMagic { .. }));
}

#[test]
fn test_seekable_writer_and_reader_full_pipeline_roundtrip() {
    let mut original = Vec::new();
    for i in 0..5_000 {
        original.extend_from_slice(format!("Row {}: high-performance stream seekable.\n", i).as_bytes());
    }

    let mut compressed_archive = Vec::new();
    let mut writer = ZstdSeekableWriter::new(&mut compressed_archive, 16384, 3, true);
    writer.write_all(&original).expect("write original data");
    let (_, table_len) = writer.finish().expect("finish writer");
    assert!(table_len > 0);

    // Test ZstdSeekableReader random seeking
    let mut reader = ZstdSeekableReader::new(Cursor::new(compressed_archive)).expect("open reader");
    assert_eq!(reader.total_size(), original.len() as u64);

    // 1. Read first 128 bytes
    let mut chunk1 = [0u8; 128];
    reader.read_exact(&mut chunk1).expect("read head");
    assert_eq!(&chunk1[..], &original[..128]);
    assert_eq!(reader.position(), 128);

    // 2. Seek to middle (e.g. 50,000)
    let target_pos = 50_000u64;
    let seek_res = reader.seek(SeekFrom::Start(target_pos)).expect("seek to 50000");
    assert_eq!(seek_res, target_pos);

    let mut chunk2 = [0u8; 256];
    reader.read_exact(&mut chunk2).expect("read middle");
    assert_eq!(
        &chunk2[..],
        &original[target_pos as usize..target_pos as usize + 256]
    );

    // 3. Seek from current (+1000)
    reader.seek(SeekFrom::Current(1000)).expect("seek current");
    let new_expected_pos = target_pos + 256 + 1000;
    assert_eq!(reader.position(), new_expected_pos);

    let mut chunk3 = [0u8; 64];
    reader.read_exact(&mut chunk3).expect("read forward");
    assert_eq!(
        &chunk3[..],
        &original[new_expected_pos as usize..new_expected_pos as usize + 64]
    );

    // 4. Seek from end (-100)
    let end_seek = reader.seek(SeekFrom::End(-100)).expect("seek end");
    assert_eq!(end_seek, original.len() as u64 - 100);

    let mut chunk_tail = [0u8; 100];
    reader.read_exact(&mut chunk_tail).expect("read tail");
    assert_eq!(&chunk_tail[..], &original[original.len() - 100..]);
}
