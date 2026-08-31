// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit tests for Bzip2 Inverse BWT matrix reconstruction.

use ttzip_engine::codecs::bzip2::blocksort::bwt_block_sort;
use ttzip_engine::codecs::bzip2::inverse_bwt::{inverse_bwt_fast, inverse_bwt_small};

#[test]
fn test_fast_and_small_inverse_bwt_equivalence() {
    let test_corpus = [
        b"The quick brown fox jumps over the lazy dog.".as_slice(),
        b"TO BE OR NOT TO BE THAT IS THE QUESTION".as_slice(),
        b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".as_slice(),
        b"1234567890123456789012345678901234567890".as_slice(),
    ];

    for &corpus in &test_corpus {
        let (orig_ptr, l) = bwt_block_sort(corpus, 30).unwrap();

        let mut restored_fast = vec![0u8; corpus.len()];
        inverse_bwt_fast(&l, orig_ptr, &mut restored_fast).unwrap();
        assert_eq!(&restored_fast, corpus);

        let mut restored_small = vec![0u8; corpus.len()];
        inverse_bwt_small(&l, orig_ptr, &mut restored_small).unwrap();
        assert_eq!(&restored_small, corpus);
    }
}
