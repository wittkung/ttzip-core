// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration and property tests for XZ Stream Index
//! serialization, deserialization, jump table binary search, and adversarial corruption defense.

use std::io::Cursor;

use ttzip_engine::crypto::crc32_fast;
use ttzip_engine::xz::header::{XzStreamFlags, XzStreamFooter, XzStreamHeader};
use ttzip_engine::xz::index::{XzRecord, XzStreamIndex};
use ttzip_engine::xz::types::{XzCheckType, XzError};
use ttzip_engine::xz::vli::{XzVliError, XZ_VLI_MAX};

#[test]
fn test_empty_index_roundtrip() {
    let index = XzStreamIndex::new();
    assert!(index.is_empty());
    assert_eq!(index.len(), 0);
    assert_eq!(index.total_uncompressed_size, 0);
    assert_eq!(index.total_unpadded_size, 0);
    assert_eq!(index.total_compressed_size(), 0);

    let encoded = index.encode().expect("encode empty index");
    // 1 (indicator) + 1 (num_records = 0) + 2 (padding) + 4 (crc32) = 8 bytes
    assert_eq!(encoded.len(), 8);
    assert_eq!(encoded.len() % 4, 0);
    assert_eq!(encoded[0], 0x00);
    assert_eq!(encoded[1], 0x00);
    assert_eq!(encoded[2], 0x00);
    assert_eq!(encoded[3], 0x00);

    let parsed = XzStreamIndex::parse(&encoded).expect("parse empty index");
    assert!(parsed.is_empty());
    assert_eq!(parsed.len(), 0);
    assert_eq!(parsed.total_uncompressed_size, 0);
    assert_eq!(parsed.total_unpadded_size, 0);
    assert_eq!(parsed.records, index.records);
}

#[test]
fn test_single_record_index_roundtrip() {
    let mut index = XzStreamIndex::new();
    index.append(100, 500).expect("append record");

    assert_eq!(index.len(), 1);
    assert_eq!(index.total_unpadded_size, 100);
    assert_eq!(index.total_uncompressed_size, 500);
    assert_eq!(index.uncompressed_prefix_sums, vec![0]);
    assert_eq!(index.compressed_prefix_sums, vec![0]);
    assert_eq!(index.total_compressed_size(), 100);

    let encoded = index.encode().expect("encode single record index");
    assert_eq!(encoded.len() % 4, 0);
    assert_eq!(encoded[0], 0x00);

    let parsed = XzStreamIndex::parse(&encoded).expect("parse single record index");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed.records[0].unpadded_size, 100);
    assert_eq!(parsed.records[0].uncompressed_size, 500);
    assert_eq!(parsed.total_unpadded_size, 100);
    assert_eq!(parsed.total_uncompressed_size, 500);
    assert_eq!(parsed.uncompressed_prefix_sums, vec![0]);
    assert_eq!(parsed.compressed_prefix_sums, vec![0]);
    assert_eq!(parsed.records, index.records);
}

#[test]
fn test_multi_record_index_various_padding_roundtrip() {
    // Test with varying numbers of records to exhaust all 0, 1, 2, 3 padding byte alignments
    for count in 1..=20 {
        let mut index = XzStreamIndex::with_capacity(count);
        for i in 0..count {
            let unpadded = 50 + (i as u64) * 17;
            let uncompressed = 100 + (i as u64) * 33;
            index.append(unpadded, uncompressed).expect("append");
        }

        let encoded = index.encode().expect("encode multi-record index");
        assert_eq!(
            encoded.len() % 4,
            0,
            "Index encoded size must always be a multiple of 4 bytes (count = {count})"
        );

        let parsed = XzStreamIndex::parse(&encoded).expect("parse multi-record index");
        assert_eq!(parsed.len(), count);
        assert_eq!(parsed.records, index.records);
        assert_eq!(parsed.total_unpadded_size, index.total_unpadded_size);
        assert_eq!(parsed.total_uncompressed_size, index.total_uncompressed_size);
        assert_eq!(parsed.uncompressed_prefix_sums, index.uncompressed_prefix_sums);
        assert_eq!(parsed.compressed_prefix_sums, index.compressed_prefix_sums);
        assert_eq!(parsed.total_compressed_size(), index.total_compressed_size());
    }
}

#[test]
fn test_large_vli_values_roundtrip() {
    let mut index = XzStreamIndex::new();
    let large_unpadded = 0x000F_FFFF_FFFF;
    let large_uncompressed = 0x00FF_FFFF_FFFF;

    index
        .append(large_unpadded, large_uncompressed)
        .expect("append large vli");

    let encoded = index.encode().expect("encode large vli index");
    let parsed = XzStreamIndex::parse(&encoded).expect("parse large vli index");

    assert_eq!(parsed.records[0].unpadded_size, large_unpadded);
    assert_eq!(parsed.records[0].uncompressed_size, large_uncompressed);
}

#[test]
fn test_locate_block_precision_and_boundaries() {
    let mut index = XzStreamIndex::new();
    // Block 0: unpadded 100 (aligned: 100), uncompressed 500  -> uncompressed [0, 500),   compressed [0, 100)
    index.append(100, 500).expect("append 0");
    // Block 1: unpadded 201 (aligned: 204), uncompressed 300  -> uncompressed [500, 800), compressed [100, 304)
    index.append(201, 300).expect("append 1");
    // Block 2: unpadded 50  (aligned: 52),  uncompressed 200  -> uncompressed [800, 1000), compressed [304, 356)
    index.append(50, 200).expect("append 2");
    // Block 3: unpadded 303 (aligned: 304), uncompressed 1000 -> uncompressed [1000, 2000), compressed [356, 660)
    index.append(303, 1000).expect("append 3");

    assert_eq!(index.total_uncompressed_size, 2000);
    assert_eq!(index.total_unpadded_size, 654);
    assert_eq!(index.uncompressed_prefix_sums, vec![0, 500, 800, 1000]);
    assert_eq!(index.compressed_prefix_sums, vec![0, 100, 304, 356]);
    assert_eq!(index.total_compressed_size(), 660);

    // 1. Block 0 boundaries
    assert_eq!(index.locate_block(0), Some((0, 0, 0)));
    assert_eq!(index.locate_block(1), Some((0, 0, 0)));
    assert_eq!(index.locate_block(250), Some((0, 0, 0)));
    assert_eq!(index.locate_block(499), Some((0, 0, 0)));

    // 2. Block 1 boundaries
    assert_eq!(index.locate_block(500), Some((1, 100, 500)));
    assert_eq!(index.locate_block(501), Some((1, 100, 500)));
    assert_eq!(index.locate_block(650), Some((1, 100, 500)));
    assert_eq!(index.locate_block(799), Some((1, 100, 500)));

    // 3. Block 2 boundaries
    assert_eq!(index.locate_block(800), Some((2, 304, 800)));
    assert_eq!(index.locate_block(801), Some((2, 304, 800)));
    assert_eq!(index.locate_block(900), Some((2, 304, 800)));
    assert_eq!(index.locate_block(999), Some((2, 304, 800)));

    // 4. Block 3 boundaries
    assert_eq!(index.locate_block(1000), Some((3, 356, 1000)));
    assert_eq!(index.locate_block(1500), Some((3, 356, 1000)));
    assert_eq!(index.locate_block(1999), Some((3, 356, 1000)));

    // 5. Out of bounds queries
    assert_eq!(index.locate_block(2000), None);
    assert_eq!(index.locate_block(2001), None);
    assert_eq!(index.locate_block(u64::MAX), None);
}

#[test]
fn test_locate_block_empty_and_zero_size_blocks() {
    // Empty index
    let empty_index = XzStreamIndex::new();
    assert_eq!(empty_index.locate_block(0), None);
    assert_eq!(empty_index.locate_block(100), None);

    // Index with leading zero-size uncompressed block
    let mut index = XzStreamIndex::new();
    index.append(40, 0).expect("zero uncompressed size block");
    index.append(80, 500).expect("normal block");

    assert_eq!(index.locate_block(0), Some((1, 40, 0)));
    assert_eq!(index.locate_block(499), Some((1, 40, 0)));
    assert_eq!(index.locate_block(500), None);
}

#[test]
fn test_backward_size_reverse_seek_loading() {
    let header_flags = XzStreamFlags::new(XzCheckType::Crc32);
    let header = XzStreamHeader::new(header_flags);
    let header_bytes = header.encode();

    // Create synthetic blocks payload
    let mut stream_bytes = Vec::new();
    stream_bytes.extend_from_slice(&header_bytes);

    let mut index = XzStreamIndex::new();
    index.append(120, 1000).expect("append block 0");
    index.append(240, 2000).expect("append block 1");

    // Add dummy block bytes corresponding to total compressed size
    let dummy_blocks = vec![0xAA; index.total_compressed_size() as usize];
    stream_bytes.extend_from_slice(&dummy_blocks);

    let index_offset = stream_bytes.len() as u64;
    let index_bytes = index.encode().expect("encode index");
    stream_bytes.extend_from_slice(&index_bytes);

    let footer = XzStreamFooter::new(header_flags, index_bytes.len() as u64);
    let footer_bytes = footer.encode_self().expect("encode footer");
    stream_bytes.extend_from_slice(&footer_bytes);

    let total_stream_len = stream_bytes.len() as u64;
    let mut cursor = Cursor::new(stream_bytes);

    let (parsed_index, parsed_index_offset) =
        XzStreamIndex::parse_from_footer(&mut cursor, total_stream_len)
            .expect("parse_from_footer");

    assert_eq!(parsed_index_offset, index_offset);
    assert_eq!(parsed_index.records, index.records);
    assert_eq!(
        parsed_index.total_uncompressed_size,
        index.total_uncompressed_size
    );
    assert_eq!(parsed_index.total_unpadded_size, index.total_unpadded_size);
}

#[test]
fn test_adversarial_invalid_indicator_rejection() {
    let mut index = XzStreamIndex::new();
    index.append(100, 200).expect("append");
    let mut encoded = index.encode().expect("encode");

    // Mutate indicator byte to non-zero
    encoded[0] = 0x01;
    // Recompute CRC32 to isolate indicator validation
    let payload_len = encoded.len() - 4;
    let new_crc = crc32_fast(0, &encoded[..payload_len]);
    encoded[payload_len..].copy_from_slice(&new_crc.to_le_bytes());

    let err = XzStreamIndex::parse(&encoded).expect_err("must reject non-zero indicator");
    assert_eq!(err, XzError::InvalidIndexIndicator(0x01));
}

#[test]
fn test_adversarial_crc32_mismatch_rejection() {
    let mut index = XzStreamIndex::new();
    index.append(100, 200).expect("append");
    let mut encoded = index.encode().expect("encode");

    // Corrupt one bit in payload
    encoded[1] ^= 0x01;

    let err = XzStreamIndex::parse(&encoded).expect_err("must reject corrupted CRC32");
    assert!(matches!(err, XzError::IndexCrcMismatch { .. }));
}

#[test]
fn test_xz_record_properties() {
    let rec = XzRecord::new(101, 500);
    assert_eq!(rec.unpadded_size, 101);
    assert_eq!(rec.uncompressed_size, 500);
    assert_eq!(rec.total_block_size(), 104);
    assert_eq!(rec.block_padding_size(), 3);

    let rec2 = XzRecord::new(100, 500);
    assert_eq!(rec2.total_block_size(), 100);
    assert_eq!(rec2.block_padding_size(), 0);
}

#[test]
fn test_adversarial_nonzero_padding_rejection() {
    let index = XzStreamIndex::new();
    // 1 (indicator) + 1 (num_records = 1) + 2 (rec 0) = 4 bytes payload -> 0 padding bytes.
    // Let's create an index with padding.
    // 0 records has 2 padding bytes.
    let mut encoded = index.encode().expect("encode empty index");

    assert_eq!(encoded.len(), 8);
    // bytes 2 and 3 are padding
    encoded[2] = 0xFF; // corrupt padding byte

    // Recompute CRC32 to isolate padding validation
    let payload_len = encoded.len() - 4;
    let new_crc = crc32_fast(0, &encoded[..payload_len]);
    encoded[payload_len..].copy_from_slice(&new_crc.to_le_bytes());

    let err = XzStreamIndex::parse(&encoded).expect_err("must reject non-zero padding");
    assert_eq!(err, XzError::NonZeroIndexPadding);
}

#[test]
fn test_adversarial_truncated_and_misaligned_index() {
    // 1. Truncated (< 8 bytes)
    let short_buf = [0u8; 6];
    assert!(matches!(
        XzStreamIndex::parse(&short_buf),
        Err(XzError::TruncatedData { .. })
    ));

    // 2. Not a multiple of 4 bytes
    let misaligned_buf = [0u8; 9];
    assert!(matches!(
        XzStreamIndex::parse(&misaligned_buf),
        Err(XzError::InvalidBackwardSize(9))
    ));
}

#[test]
fn test_adversarial_invalid_unpadded_size_zero_and_vli_overflow() {
    let mut index = XzStreamIndex::new();

    // 1. unpadded_size == 0
    assert_eq!(index.append(0, 100), Err(XzError::InvalidUnpaddedSize(0)));

    // 2. unpadded_size > XZ_VLI_MAX
    assert_eq!(
        index.append(XZ_VLI_MAX + 1, 100),
        Err(XzError::InvalidUnpaddedSize(XZ_VLI_MAX + 1))
    );

    // 3. uncompressed_size > XZ_VLI_MAX
    assert_eq!(
        index.append(100, XZ_VLI_MAX + 1),
        Err(XzError::InvalidVli(XzVliError::ValueTooLarge {
            val: XZ_VLI_MAX + 1
        }))
    );
}

#[test]
fn test_adversarial_backward_size_mismatch_in_footer() {
    let header_flags = XzStreamFlags::new(XzCheckType::Crc32);
    let header = XzStreamHeader::new(header_flags);

    let mut stream_bytes = Vec::new();
    stream_bytes.extend_from_slice(&header.encode());

    let mut index = XzStreamIndex::new();
    index.append(100, 500).expect("append");
    let index_bytes = index.encode().expect("encode index");
    stream_bytes.extend_from_slice(&index_bytes);

    // Corrupt footer's backward size (say index_bytes.len() + 4)
    let corrupted_backward_size = (index_bytes.len() as u64) + 4;
    let footer = XzStreamFooter::new(header_flags, corrupted_backward_size);
    stream_bytes.extend_from_slice(&footer.encode_self().expect("encode footer"));

    let total_stream_len = stream_bytes.len() as u64;
    let mut cursor = Cursor::new(stream_bytes);

    let err = XzStreamIndex::parse_from_footer(&mut cursor, total_stream_len)
        .expect_err("must detect backward size mismatch");

    assert!(
        matches!(err, XzError::BackwardSizeMismatch { expected, actual } if expected == corrupted_backward_size && actual == index_bytes.len() as u64)
            || matches!(err, XzError::TruncatedData { .. })
            || matches!(err, XzError::IndexCrcMismatch { .. })
    );
}
