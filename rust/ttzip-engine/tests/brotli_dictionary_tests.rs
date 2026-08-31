// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration tests and throughput benchmark for RFC 7932
//! precompiled static dictionary constant-time lookup engine.

use std::time::Instant;
use ttzip_engine::codecs::brotli::{
    dictionary_word_count, get_dictionary_word, rfc7932_dictionary_data,
    DICTIONARY_DATA_SIZE, MAX_DICTIONARY_WORD_LENGTH, MIN_DICTIONARY_WORD_LENGTH,
    OFFSETS_BY_LENGTH, RFC7932_DICTIONARY_DATA, SIZE_BITS_BY_LENGTH,
};

// MARK: - 1. Dictionary Size & Bounds Invariants

#[test]
fn test_dictionary_exact_size_and_constants() {
    assert_eq!(DICTIONARY_DATA_SIZE, 122_784);
    assert_eq!(RFC7932_DICTIONARY_DATA.len(), 122_784);
    assert_eq!(rfc7932_dictionary_data().len(), 122_784);

    assert_eq!(MIN_DICTIONARY_WORD_LENGTH, 4);
    assert_eq!(MAX_DICTIONARY_WORD_LENGTH, 24);

    assert_eq!(SIZE_BITS_BY_LENGTH.len(), 32);
    assert_eq!(OFFSETS_BY_LENGTH.len(), 32);
}

#[test]
fn test_offsets_and_bit_invariants() {
    // RFC 7932 invariant: offset[i + 1] == offset[i] + (bits[i] != 0 ? (i << bits[i]) : 0)
    for i in 0..31 {
        let bits = SIZE_BITS_BY_LENGTH[i] as usize;
        let expected_next_offset = if bits != 0 {
            OFFSETS_BY_LENGTH[i] + ((i * (1 << bits)) as u32)
        } else {
            OFFSETS_BY_LENGTH[i]
        };
        assert_eq!(
            OFFSETS_BY_LENGTH[i + 1],
            expected_next_offset,
            "Offset mismatch at index {}",
            i
        );
    }
    assert_eq!(OFFSETS_BY_LENGTH[31] as usize, DICTIONARY_DATA_SIZE);
}

// MARK: - 2. Exact First and Last Word Matching for 21 Buckets

#[test]
fn test_all_21_buckets_first_and_last_words() {
    let expected_words: &[(usize, usize, &[u8], &[u8])] = &[
        // (length, count, first_word, last_word)
        (4, 1024, &[116, 105, 109, 101], &[217, 136, 216, 180]),
        (5, 1024, &[102, 105, 114, 115, 116], &[97, 103, 117, 97, 115]),
        (6, 2048, &[38, 113, 117, 111, 116, 59], &[216, 183, 217, 132, 216, 168]),
        (7, 2048, &[112, 114, 111, 102, 105, 108, 101], &[109, 101, 106, 111, 114, 97, 114]),
        (8, 1024, &[112, 111, 115, 105, 116, 105, 111, 110], &[0, 4, 0, 5, 0, 6, 0, 7]),
        (9, 1024, &[114, 101, 115, 111, 117, 114, 99, 101, 115], &[224, 164, 178, 224, 164, 151, 224, 165, 128]),
        (10, 1024, &[99, 97, 116, 101, 103, 111, 114, 105, 101, 115], &[217, 138, 216, 185, 216, 183, 217, 138, 217, 131]),
        (11, 1024, &[115, 66, 121, 84, 97, 103, 78, 97, 109, 101, 40], &[99, 111, 110, 102, 111, 114, 109, 105, 100, 97, 100]),
        (12, 1024, &[108, 105, 110, 101, 45, 104, 101, 105, 103, 104, 116, 58], &[216, 167, 217, 132, 216, 185, 216, 184, 217, 138, 217, 133]),
        (13, 512, &[101, 110, 116, 101, 114, 116, 97, 105, 110, 109, 101, 110, 116], &[99, 111, 110, 115, 116, 114, 117, 99, 99, 105, 195, 179, 110]),
        (14, 512, &[34, 62, 60, 100, 105, 118, 32, 99, 108, 97, 115, 115, 61, 34], &[216, 167, 217, 132, 216, 167, 216, 186, 216, 167, 217, 134, 217, 138]),
        (15, 256, &[99, 117, 114, 115, 111, 114, 58, 112, 111, 105, 110, 116, 101, 114, 59], &[224, 164, 156, 224, 164, 191, 224, 164, 184, 224, 164, 149, 224, 165, 135]),
        (16, 128, &[114, 115, 115, 43, 120, 109, 108, 34, 32, 116, 105, 116, 108, 101, 61, 34], &[216, 167, 217, 132, 217, 133, 216, 177, 216, 166, 217, 138, 216, 167, 216, 170]),
        (17, 128, &[114, 111, 98, 111, 116, 115, 34, 32, 99, 111, 110, 116, 101, 110, 116, 61, 34], &[111, 99, 99, 97, 115, 105, 111, 110, 97, 108, 108, 121, 32, 117, 115, 101, 100]),
        (18, 256, &[112, 111, 115, 105, 116, 105, 111, 110, 58, 97, 98, 115, 111, 108, 117, 116, 101, 59], &[216, 167, 217, 132, 216, 167, 216, 170, 216, 181, 216, 167, 217, 132, 216, 167, 216, 170]),
        (19, 128, &[107, 101, 121, 119, 111, 114, 100, 115, 34, 32, 99, 111, 110, 116, 101, 110, 116, 61, 34], &[104, 97, 118, 101, 32, 99, 104, 105, 108, 100, 114, 101, 110, 32, 117, 110, 100, 101, 114]),
        (20, 128, &[37, 51, 69, 37, 51, 67, 47, 115, 99, 114, 105, 112, 116, 37, 51, 69, 34, 41, 41, 59], &[216, 167, 217, 132, 216, 167, 217, 130, 216, 170, 216, 177, 216, 167, 216, 173, 216, 167, 216, 170]),
        (21, 64, &[104, 116, 109, 108, 59, 32, 99, 104, 97, 114, 115, 101, 116, 61, 85, 84, 70, 45, 56, 34, 32], &[224, 164, 178, 224, 164, 191, 224, 164, 174, 224, 164, 191, 224, 164, 159, 224, 165, 135, 224, 164, 161]),
        (22, 64, &[100, 101, 115, 99, 114, 105, 112, 116, 105, 111, 110, 34, 32, 99, 111, 110, 116, 101, 110, 116, 61, 34], &[208, 190, 208, 177, 209, 143, 208, 183, 208, 176, 209, 130, 208, 181, 208, 187, 209, 140, 208, 189, 208, 176]),
        (23, 32, &[60, 33, 68, 79, 67, 84, 89, 80, 69, 32, 104, 116, 109, 108, 32, 80, 85, 66, 76, 73, 67, 32, 34], &[105, 110, 112, 117, 116, 32, 116, 121, 112, 101, 61, 34, 104, 105, 100, 100, 101, 110, 34, 32, 110, 97, 109]),
        (24, 32, &[60, 115, 99, 114, 105, 112, 116, 32, 116, 121, 112, 101, 61, 34, 116, 101, 120, 116, 47, 106, 97, 118, 97, 115], &[224, 164, 184, 224, 164, 149, 224, 165, 141, 224, 164, 176, 224, 164, 191, 224, 164, 175, 224, 164, 164, 224, 164, 190]),
    ];

    let mut total_words_count = 0;
    for &(len, expected_count, first_word, last_word) in expected_words {
        let count = dictionary_word_count(len);
        assert_eq!(count, expected_count, "Word count mismatch for len {}", len);
        total_words_count += count;

        let first = get_dictionary_word(len, 0).expect("First word must exist");
        assert_eq!(
            first, first_word,
            "First word mismatch for len {}: {:?} vs {:?}",
            len, first, first_word
        );

        let last = get_dictionary_word(len, count - 1).expect("Last word must exist");
        assert_eq!(
            last, last_word,
            "Last word mismatch for len {}: {:?} vs {:?}",
            len, last, last_word
        );
    }

    assert_eq!(total_words_count, 13_504);
}

// MARK: - 3. Complete Corpus Validation

#[test]
fn test_exhaustive_dictionary_iteration() {
    let mut total_bytes = 0usize;
    let mut word_count = 0usize;

    for len in MIN_DICTIONARY_WORD_LENGTH..=MAX_DICTIONARY_WORD_LENGTH {
        let count = dictionary_word_count(len);
        for idx in 0..count {
            let word = get_dictionary_word(len, idx).expect("Valid word must return Some");
            assert_eq!(word.len(), len);
            total_bytes += word.len();
            word_count += 1;
        }
    }

    assert_eq!(word_count, 13_504);
    assert_eq!(total_bytes, DICTIONARY_DATA_SIZE);
}

// MARK: - 4. Robust Boundary Checks & 0-Panic Invariants

#[test]
fn test_invalid_lengths_and_out_of_bounds_indices() {
    // Lengths below MIN_DICTIONARY_WORD_LENGTH (0..=3)
    for len in 0..4 {
        assert_eq!(get_dictionary_word(len, 0), None);
        assert_eq!(get_dictionary_word(len, 100), None);
        assert_eq!(dictionary_word_count(len), 0);
    }

    // Lengths above MAX_DICTIONARY_WORD_LENGTH (25..=31 and beyond)
    for len in 25..=32 {
        assert_eq!(get_dictionary_word(len, 0), None);
        assert_eq!(get_dictionary_word(len, 10), None);
        assert_eq!(dictionary_word_count(len), 0);
    }

    assert_eq!(get_dictionary_word(100, 0), None);
    assert_eq!(get_dictionary_word(usize::MAX, 0), None);
    assert_eq!(dictionary_word_count(100), 0);
    assert_eq!(dictionary_word_count(usize::MAX), 0);

    // Out of bounds word_idx within valid lengths
    for len in MIN_DICTIONARY_WORD_LENGTH..=MAX_DICTIONARY_WORD_LENGTH {
        let count = dictionary_word_count(len);
        assert_eq!(get_dictionary_word(len, count), None);
        assert_eq!(get_dictionary_word(len, count + 1), None);
        assert_eq!(get_dictionary_word(len, usize::MAX), None);
    }
}

// MARK: - 5. O(1) Constant-Time Lookup Throughput Gate (> 10M lookups/sec)

#[test]
fn test_constant_time_lookup_throughput_gate() {
    const NUM_ITERATIONS: usize = 20_000_000;

    let start = Instant::now();
    let mut sum: u64 = 0;

    // Linear Congruential Generator for deterministic lookup pattern
    let mut lcg_state: u64 = 0xDEAD_BEEF_CAFE_BABE;

    for _ in 0..NUM_ITERATIONS {
        // Fast pseudo-random number generator: x_n+1 = (a * x_n + c) mod 2^64
        lcg_state = lcg_state.wrapping_mul(6364136223846793005).wrapping_add(1);

        // Select length in 4..=24
        let len = 4 + ((lcg_state >> 32) as usize % 21);
        let max_words = dictionary_word_count(len);
        let word_idx = ((lcg_state >> 16) as usize) % max_words;

        if let Some(word) = get_dictionary_word(len, word_idx) {
            sum = sum.wrapping_add(word[0] as u64);
        }
    }

    let elapsed = start.elapsed();
    let elapsed_secs = elapsed.as_secs_f64();
    let lookups_per_sec = (NUM_ITERATIONS as f64) / elapsed_secs;

    println!(
        "[BrotliDictionary] Throughput: {:.2} million lookups/sec ({:?} for {} lookups, checksum={})",
        lookups_per_sec / 1_000_000.0,
        elapsed,
        NUM_ITERATIONS,
        sum
    );

    // Hard gate: must achieve at least 10 million lookups per second
    assert!(
        lookups_per_sec >= 10_000_000.0,
        "Lookup throughput {:.2} M/s is below gate threshold of 10.0 M/s",
        lookups_per_sec / 1_000_000.0
    );
    assert!(sum > 0, "Sum must be non-zero");
}
