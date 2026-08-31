// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive Verification Test Suite for LZMA2 Sliding Dictionary Overlap,
//! History Buffering, Boundary Compaction, and Invariant Memory Defense.
//!
//! Validates:
//! 1. Zero-allocation initialization, default capacities, and bounds.
//! 2. Byte and multi-byte slice history preservation with exact distance retrieval.
//! 3. Distance underflow/overflow bounds enforcement (`Lzma2DecodeError::InvalidDistance`).
//! 4. Sliding window automatic compaction when capacity exceeds `2 * max_size`.
//! 5. Overlapping match reference decoding (e.g. self-referential RLE where distance < match length).
//! 6. State reset lifecycle and total uncompressed byte tracking across chunk boundaries.

use ttzip_engine::codecs::lzma2::decoder::Lzma2DecodeError;
use ttzip_engine::codecs::lzma2::Lzma2Dict;

#[test]
fn test_dict_initialization_and_default_bounds() {
    let dict = Lzma2Dict::new(1024 * 1024);
    assert_eq!(dict.len(), 0);
    assert!(dict.is_empty());
    assert_eq!(dict.total_written(), 0);
    assert_eq!(dict.last_byte(), 0);
}

#[test]
fn test_dict_put_byte_and_put_slice_history() {
    let mut dict = Lzma2Dict::new(64 * 1024);

    dict.put_byte(0xAA);
    dict.put_byte(0xBB);
    dict.put_byte(0xCC);

    assert_eq!(dict.len(), 3);
    assert_eq!(dict.total_written(), 3);
    assert_eq!(dict.last_byte(), 0xCC);

    // Distance offset 0 -> 1 byte back (0xCC)
    assert_eq!(dict.get_byte_at_distance(0).expect("distance 0"), 0xCC);
    // Distance offset 1 -> 2 bytes back (0xBB)
    assert_eq!(dict.get_byte_at_distance(1).expect("distance 1"), 0xBB);
    // Distance offset 2 -> 3 bytes back (0xAA)
    assert_eq!(dict.get_byte_at_distance(2).expect("distance 2"), 0xAA);

    // Append slice
    let slice_data = b"DEF_GHI_JKL";
    dict.put_slice(slice_data);

    assert_eq!(dict.len(), 3 + slice_data.len());
    assert_eq!(dict.total_written(), 3 + slice_data.len());
    assert_eq!(dict.last_byte(), b'L');

    // Query across slice boundary
    assert_eq!(dict.get_byte_at_distance(0).expect("last byte"), b'L');
    assert_eq!(dict.get_byte_at_distance(slice_data.len() - 1).expect("first byte of slice"), b'D');
    assert_eq!(dict.get_byte_at_distance(slice_data.len()).expect("0xCC"), 0xCC);
}

#[test]
fn test_dict_out_of_bounds_distance_error() {
    let mut dict = Lzma2Dict::new(1024);

    // Empty dict lookup must fail
    match dict.get_byte_at_distance(0) {
        Err(Lzma2DecodeError::InvalidDistance { distance, dict_len }) => {
            assert_eq!(distance, 1);
            assert_eq!(dict_len, 0);
        }
        other => panic!("Expected InvalidDistance error, got: {:?}", other),
    }

    dict.put_slice(b"12345"); // len = 5
    assert_eq!(dict.len(), 5);

    // Distance offset 4 -> 5 bytes back (valid, '1')
    assert_eq!(dict.get_byte_at_distance(4).expect("valid distance"), b'1');

    // Distance offset 5 -> 6 bytes back (invalid, len = 5)
    match dict.get_byte_at_distance(5) {
        Err(Lzma2DecodeError::InvalidDistance { distance, dict_len }) => {
            assert_eq!(distance, 6);
            assert_eq!(dict_len, 5);
        }
        other => panic!("Expected InvalidDistance error, got: {:?}", other),
    }
}

#[test]
fn test_dict_sliding_window_compaction_past_2x_max_size() {
    let max_size = 1024usize; // 1 KB max dictionary window
    let mut dict = Lzma2Dict::new(max_size);

    // Fill buffer beyond 2 * max_size (2048 bytes) to trigger compaction
    let chunk = vec![0x42u8; 1500];
    dict.put_slice(&chunk);
    assert_eq!(dict.len(), 1500);
    assert_eq!(dict.total_written(), 1500);

    // Append second chunk of 1000 bytes -> total buffered would be 2500 (> 2048)
    let marker_chunk: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
    dict.put_slice(&marker_chunk);

    // After compaction, buffer len must be clamped down to max_size (1024)
    assert_eq!(dict.len(), max_size);
    assert_eq!(dict.total_written(), 2500);

    // Verify recent history is preserved accurately
    assert_eq!(dict.last_byte(), marker_chunk[999]);
    for i in 0..1000 {
        let dist = 999 - i;
        assert_eq!(
            dict.get_byte_at_distance(dist).expect("recent history byte"),
            marker_chunk[i],
            "History mismatch at index {i}"
        );
    }
}

#[test]
fn test_dict_rle_overlapping_match_pattern_continuity() {
    // Simulates an overlapping copy loop where distance = 1 (RLE single byte repeat)
    let mut dict = Lzma2Dict::new(64 * 1024);

    dict.put_byte(b'X');
    let repeat_count = 5000usize;
    for _ in 0..repeat_count {
        let b = dict.get_byte_at_distance(0).expect("read last byte");
        dict.put_byte(b);
    }

    assert_eq!(dict.len(), repeat_count + 1);
    assert_eq!(dict.total_written(), repeat_count + 1);
    assert_eq!(dict.last_byte(), b'X');

    // Verify all history elements are 'X'
    for dist in 0..dict.len() {
        assert_eq!(dict.get_byte_at_distance(dist).expect("byte at dist"), b'X');
    }
}

#[test]
fn test_dict_multi_byte_periodic_overlap() {
    // Simulates an overlapping copy loop where distance = 3 (e.g. "ABCABCABC...")
    let mut dict = Lzma2Dict::new(64 * 1024);

    dict.put_slice(b"ABC");
    let cycles = 1000usize;
    for _ in 0..cycles {
        for _ in 0..3 {
            let b = dict.get_byte_at_distance(2).expect("read 3 bytes back");
            dict.put_byte(b);
        }
    }

    assert_eq!(dict.len(), 3 + cycles * 3);
    for i in 0..dict.len() {
        let expected = match i % 3 {
            0 => b'A',
            1 => b'B',
            _ => b'C',
        };
        let dist = dict.len() - 1 - i;
        assert_eq!(dict.get_byte_at_distance(dist).expect("byte at dist"), expected);
    }
}

#[test]
fn test_dict_reset_clears_history_and_restarts_bounds() {
    let mut dict = Lzma2Dict::new(1024);
    dict.put_slice(b"OLD_CHUNK_HISTORY_PAYLOAD");
    assert_eq!(dict.len(), 25);
    assert_eq!(dict.total_written(), 25);

    dict.reset();
    assert_eq!(dict.len(), 0);
    assert_eq!(dict.total_written(), 0);
    assert!(dict.is_empty());
    assert_eq!(dict.last_byte(), 0);

    // Lookups must now fail
    assert!(dict.get_byte_at_distance(0).is_err());

    // New writes start from clean baseline
    dict.put_slice(b"NEW");
    assert_eq!(dict.len(), 3);
    assert_eq!(dict.total_written(), 3);
    assert_eq!(dict.get_byte_at_distance(0).expect("byte 0"), b'W');
    assert_eq!(dict.get_byte_at_distance(1).expect("byte 1"), b'E');
    assert_eq!(dict.get_byte_at_distance(2).expect("byte 2"), b'N');
    assert!(dict.get_byte_at_distance(3).is_err());
}
