// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit tests for Bzip2 Burrows-Wheeler Transform (BWT) block sorting.

use ttzip_engine::codecs::bzip2::blocksort::bwt_block_sort;
use ttzip_engine::codecs::bzip2::inverse_bwt::inverse_bwt_fast;

#[test]
fn test_bwt_empty_and_single() {
    let empty = b"";
    let (orig, l) = bwt_block_sort(empty, 30).unwrap();
    assert_eq!(orig, 0);
    assert!(l.is_empty());

    let single = b"Z";
    let (orig, l) = bwt_block_sort(single, 30).unwrap();
    assert_eq!(orig, 0);
    assert_eq!(l, b"Z");
}

#[test]
fn test_bwt_banana_roundtrip() {
    let input = b"banana";
    let (orig_ptr, l) = bwt_block_sort(input, 30).unwrap();
    assert_eq!(orig_ptr, 3);
    assert_eq!(l, b"nnbaaa");

    let mut restored = vec![0u8; input.len()];
    inverse_bwt_fast(&l, orig_ptr, &mut restored).unwrap();
    assert_eq!(&restored, input);
}

#[test]
fn test_bwt_repetitive_and_periodic() {
    let input = b"abcabcabcabcabcabcabcabcabc";
    let (orig_ptr, l) = bwt_block_sort(input, 30).unwrap();
    let mut restored = vec![0u8; input.len()];
    inverse_bwt_fast(&l, orig_ptr, &mut restored).unwrap();
    assert_eq!(&restored, input);

    let all_a = vec![b'A'; 2048];
    let (orig_ptr, l) = bwt_block_sort(&all_a, 30).unwrap();
    assert_eq!(orig_ptr, 0);
    let mut restored = vec![0u8; all_a.len()];
    inverse_bwt_fast(&l, orig_ptr, &mut restored).unwrap();
    assert_eq!(&restored, &all_a);
}

#[test]
fn test_bwt_random_pseudo_entropy() {
    let mut pseudo_rand = Vec::with_capacity(4096);
    let mut state: u32 = 0x12345678;
    for _ in 0..4096 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        pseudo_rand.push((state >> 16) as u8);
    }

    let (orig_ptr, l) = bwt_block_sort(&pseudo_rand, 30).unwrap();
    let mut restored = vec![0u8; pseudo_rand.len()];
    inverse_bwt_fast(&l, orig_ptr, &mut restored).unwrap();
    assert_eq!(&restored, &pseudo_rand);
}
