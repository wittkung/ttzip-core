// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive TAR 512-Byte Sector Alignment & 2x512B Zero EOF Detector Test Suite.
//!
//! Validates:
//! 1. Mathematical accuracy of `pad_to_512` and `aligned_size_512` across all critical boundaries
//!    (0, 1, 511, 512, 513, 1024, 4096, and near-u64::MAX saturation).
//! 2. High-speed 64-bit SIMD word zero-block detection (`is_all_zeros` and `is_slice_all_zeros`)
//!    against pure-zero blocks and single-bit perturbations at boundary positions.
//! 3. Streaming `EofBlockDetector` state machine transitions:
//!    - Standard two consecutive 512-byte zero blocks (1024B EOF).
//!    - Non-standard single 512-byte zero block stream truncation (`TarEofStatus::TruncatedZero`).
//!    - Multi-archive concatenation with `ignore_zeros` mode.
//!    - Stream reset and interleaving data/zero blocks.
//! 4. Binary output verification of `write_eof_blocks`.

use std::io::Cursor;
use ttzip_engine::tar::alignment::{
    aligned_size_512, is_all_zeros, is_slice_all_zeros, pad_to_512, write_eof_blocks,
    EofBlockDetector, TarEofStatus, TAR_EOF_SIZE, TAR_EOF_SIZE_U64, TAR_SECTOR_SIZE,
    TAR_SECTOR_SIZE_U64,
};

#[test]
fn test_tar_sector_and_eof_constants() {
    assert_eq!(TAR_SECTOR_SIZE, 512);
    assert_eq!(TAR_SECTOR_SIZE_U64, 512);
    assert_eq!(TAR_EOF_SIZE, 1024);
    assert_eq!(TAR_EOF_SIZE_U64, 1024);
    assert_eq!(TAR_EOF_SIZE, TAR_SECTOR_SIZE * 2);
}

#[test]
fn test_pad_to_512_mathematical_precision() {
    // 0-byte payload: already aligned -> 0 padding
    assert_eq!(pad_to_512(0), 0);

    // 1-byte payload: needs 511 bytes to reach 512
    assert_eq!(pad_to_512(1), 511);

    // 511-byte payload: needs 1 byte to reach 512
    assert_eq!(pad_to_512(511), 1);

    // 512-byte payload: perfectly aligned -> 0 padding
    assert_eq!(pad_to_512(512), 0);

    // 513-byte payload: needs 511 bytes to reach 1024
    assert_eq!(pad_to_512(513), 511);

    // 1023-byte payload: needs 1 byte to reach 1024
    assert_eq!(pad_to_512(1023), 1);

    // 1024-byte payload: perfectly aligned -> 0 padding
    assert_eq!(pad_to_512(1024), 0);

    // 4096-byte payload: 8 * 512 -> perfectly aligned -> 0 padding
    assert_eq!(pad_to_512(4096), 0);

    // 4097-byte payload: needs 511 bytes to reach 4608
    assert_eq!(pad_to_512(4097), 511);

    // Arbitrary size check
    for offset in 0..1024u64 {
        let pad = pad_to_512(offset);
        assert_eq!((offset + pad) % 512, 0);
        assert!(pad < 512);
    }
}

#[test]
fn test_aligned_size_512_mathematical_precision() {
    assert_eq!(aligned_size_512(0), 0);
    assert_eq!(aligned_size_512(1), 512);
    assert_eq!(aligned_size_512(511), 512);
    assert_eq!(aligned_size_512(512), 512);
    assert_eq!(aligned_size_512(513), 1024);
    assert_eq!(aligned_size_512(1023), 1024);
    assert_eq!(aligned_size_512(1024), 1024);
    assert_eq!(aligned_size_512(4096), 4096);
    assert_eq!(aligned_size_512(4097), 4608);

    // Saturation behavior on near-overflow
    let near_max = u64::MAX - 100;
    let aligned = aligned_size_512(near_max);
    assert!(aligned >= near_max);
}

#[test]
fn test_is_all_zeros_full_zero_block() {
    let block = [0u8; TAR_SECTOR_SIZE];
    assert!(is_all_zeros(&block));
    assert!(is_slice_all_zeros(&block));
}

#[test]
fn test_is_all_zeros_single_bit_perturbations() {
    let mut block = [0u8; TAR_SECTOR_SIZE];

    // Perturb first byte
    block[0] = 1;
    assert!(!is_all_zeros(&block));
    assert!(!is_slice_all_zeros(&block));
    block[0] = 0;

    // Perturb middle byte (index 255 and 256)
    block[255] = 0x80;
    assert!(!is_all_zeros(&block));
    assert!(!is_slice_all_zeros(&block));
    block[255] = 0;

    block[256] = 0x01;
    assert!(!is_all_zeros(&block));
    assert!(!is_slice_all_zeros(&block));
    block[256] = 0;

    // Perturb last byte
    block[511] = 0xFF;
    assert!(!is_all_zeros(&block));
    assert!(!is_slice_all_zeros(&block));
    block[511] = 0;

    // Verify back to zero
    assert!(is_all_zeros(&block));

    // Exhaustive test: every single byte position perturbed individually
    for i in 0..TAR_SECTOR_SIZE {
        block[i] = (i % 255 + 1) as u8;
        assert!(!is_all_zeros(&block), "Failed detection at byte index {}", i);
        assert!(!is_slice_all_zeros(&block), "Failed slice detection at byte index {}", i);
        block[i] = 0;
    }
}

#[test]
fn test_is_slice_all_zeros_arbitrary_lengths() {
    // Empty slice
    assert!(is_slice_all_zeros(&[]));

    // Lengths 1 to 32 with all zeros
    for len in 1..=32 {
        let zeroes = vec![0u8; len];
        assert!(is_slice_all_zeros(&zeroes));

        // Perturb each position
        for pos in 0..len {
            let mut modified = zeroes.clone();
            modified[pos] = 1;
            assert!(!is_slice_all_zeros(&modified));
        }
    }

    // Unaligned slices
    let large = vec![0u8; 1024];
    assert!(is_slice_all_zeros(&large[3..17]));
    assert!(is_slice_all_zeros(&large[7..513]));

    let mut large_dirty = vec![0u8; 1024];
    large_dirty[500] = 42;
    assert!(!is_slice_all_zeros(&large_dirty[7..513]));
    assert!(is_slice_all_zeros(&large_dirty[0..499]));
}

#[test]
fn test_eof_block_detector_standard_two_zero_blocks() {
    let mut detector = EofBlockDetector::default();
    let zero_block = [0u8; TAR_SECTOR_SIZE];
    let mut data_block = [0u8; TAR_SECTOR_SIZE];
    data_block[0] = b'u';
    data_block[1] = b's';
    data_block[2] = b't';
    data_block[3] = b'a';
    data_block[4] = b'r';

    // 1. Data block -> Continue
    let st = detector.feed_block(&data_block);
    assert_eq!(st, TarEofStatus::Continue);
    assert_eq!(detector.consecutive_zero_blocks(), 0);
    assert!(!detector.is_eof());

    // 2. Another data block -> Continue
    let st = detector.feed_block(&data_block);
    assert_eq!(st, TarEofStatus::Continue);
    assert_eq!(detector.consecutive_zero_blocks(), 0);
    assert!(!detector.is_eof());

    // 3. First zero block -> Continue (awaiting 2nd block)
    let st = detector.feed_block(&zero_block);
    assert_eq!(st, TarEofStatus::Continue);
    assert_eq!(detector.consecutive_zero_blocks(), 1);
    assert!(!detector.is_eof());

    // 4. Second zero block -> EndOfArchive!
    let st = detector.feed_block(&zero_block);
    assert_eq!(st, TarEofStatus::EndOfArchive);
    assert_eq!(detector.consecutive_zero_blocks(), 2);
    assert!(detector.is_eof());

    // 5. Subsequent blocks after EOF continue to indicate EOF
    let st = detector.feed_block(&zero_block);
    assert_eq!(st, TarEofStatus::EndOfArchive);
    assert_eq!(detector.consecutive_zero_blocks(), 3);
    assert_eq!(detector.on_stream_end(), TarEofStatus::EndOfArchive);
}

#[test]
fn test_eof_block_detector_reset_on_data() {
    let mut detector = EofBlockDetector::new(false);
    let zero_block = [0u8; TAR_SECTOR_SIZE];
    let mut data_block = [0u8; TAR_SECTOR_SIZE];
    data_block[10] = 0xAA;

    // 1. First zero block
    assert_eq!(detector.feed_block(&zero_block), TarEofStatus::Continue);
    assert_eq!(detector.consecutive_zero_blocks(), 1);

    // 2. Interrupted by data block -> count resets to 0
    assert_eq!(detector.feed_block(&data_block), TarEofStatus::Continue);
    assert_eq!(detector.consecutive_zero_blocks(), 0);
    assert!(!detector.is_eof());

    // 3. First zero block again
    assert_eq!(detector.feed_block(&zero_block), TarEofStatus::Continue);
    assert_eq!(detector.consecutive_zero_blocks(), 1);

    // 4. Explicit reset
    detector.reset();
    assert_eq!(detector.consecutive_zero_blocks(), 0);
    assert!(!detector.is_eof());
}

#[test]
fn test_eof_block_detector_truncated_single_zero() {
    let mut detector = EofBlockDetector::default();
    let zero_block = [0u8; TAR_SECTOR_SIZE];
    let mut data_block = [0u8; TAR_SECTOR_SIZE];
    data_block[0] = 0x55;

    // Feed payload
    assert_eq!(detector.feed_block(&data_block), TarEofStatus::Continue);

    // Feed single zero block
    assert_eq!(detector.feed_block(&zero_block), TarEofStatus::Continue);
    assert_eq!(detector.consecutive_zero_blocks(), 1);

    // Stream abruptly ends here (e.g. non-standard 1-block tar)
    let end_status = detector.on_stream_end();
    assert_eq!(end_status, TarEofStatus::TruncatedZero);
}

#[test]
fn test_eof_block_detector_ignore_zeros_concatenation() {
    let mut detector = EofBlockDetector::new(true);
    assert!(detector.ignore_zeros());

    let zero_block = [0u8; TAR_SECTOR_SIZE];
    let mut data_block = [0u8; TAR_SECTOR_SIZE];
    data_block[0] = 0x12;

    // In ignore_zeros mode, every zero block returns IgnoredZero
    assert_eq!(detector.feed_block(&zero_block), TarEofStatus::IgnoredZero);
    assert_eq!(detector.consecutive_zero_blocks(), 1);

    assert_eq!(detector.feed_block(&zero_block), TarEofStatus::IgnoredZero);
    assert_eq!(detector.consecutive_zero_blocks(), 2);

    assert_eq!(detector.feed_block(&zero_block), TarEofStatus::IgnoredZero);
    assert_eq!(detector.consecutive_zero_blocks(), 3);

    // Followed by another archive entry payload
    assert_eq!(detector.feed_block(&data_block), TarEofStatus::Continue);
    assert_eq!(detector.consecutive_zero_blocks(), 0);

    // Builder method
    let detector2 = EofBlockDetector::default().with_ignore_zeros(true);
    assert!(detector2.ignore_zeros());
}

#[test]
fn test_write_eof_blocks() {
    let mut buffer = Vec::new();
    let written = write_eof_blocks(&mut buffer).expect("write_eof_blocks should succeed");

    assert_eq!(written, 1024);
    assert_eq!(buffer.len(), 1024);
    assert!(buffer.iter().all(|&b| b == 0));

    // Verify reading back with EofBlockDetector
    let mut detector = EofBlockDetector::default();
    let mut cursor = Cursor::new(&buffer);
    let mut block1 = [0u8; TAR_SECTOR_SIZE];
    let mut block2 = [0u8; TAR_SECTOR_SIZE];

    use std::io::Read;
    cursor.read_exact(&mut block1).unwrap();
    assert_eq!(detector.feed_block(&block1), TarEofStatus::Continue);

    cursor.read_exact(&mut block2).unwrap();
    assert_eq!(detector.feed_block(&block2), TarEofStatus::EndOfArchive);
    assert!(detector.is_eof());
}
