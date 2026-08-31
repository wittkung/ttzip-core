// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! RFC 7932 120KB Precompiled Static Dictionary and O(1) Constant-Time Lookup Engine.
//!
//! Brotli compression format specifies a 122,784-byte static dictionary partitioned
//! into 21 word-length buckets ranging from 4 to 24 bytes (RFC 7932 Appendix A).
//! This module embeds the precompiled static dictionary data as a static slice and provides
//! zero-allocation, branchless-indexed $O(1)$ constant-time lookup operations.

/// Minimum word length present in the RFC 7932 static dictionary (4 bytes).
pub const MIN_DICTIONARY_WORD_LENGTH: usize = 4;

/// Maximum word length present in the RFC 7932 static dictionary (24 bytes).
pub const MAX_DICTIONARY_WORD_LENGTH: usize = 24;

/// Total byte size of the RFC 7932 static dictionary (122,784 bytes, exactly 120 KiB).
pub const DICTIONARY_DATA_SIZE: usize = 122_784;

/// Embedded raw RFC 7932 static dictionary bytes.
pub const RFC7932_DICTIONARY_DATA: &[u8; DICTIONARY_DATA_SIZE] =
    include_bytes!("data/rfc7932_dictionary.bin");

/// Number of index bits for each word length bucket [0..31].
///
/// A non-zero entry `b` indicates that the dictionary contains `1 << b` words of that length.
/// Lengths `0..=3` and `25..=31` have 0 bits (no words).
pub const SIZE_BITS_BY_LENGTH: [u8; 32] = [
    0, 0, 0, 0, 10, 10, 11, 11,
    10, 10, 10, 10, 10, 9, 9, 8,
    7, 7, 8, 7, 7, 6, 6, 5,
    5, 0, 0, 0, 0, 0, 0, 0,
];

/// Absolute byte offsets into [`RFC7932_DICTIONARY_DATA`] for each word length bucket [0..31].
///
/// Invariant: `offset[i + 1] == offset[i] + (bits[i] != 0 ? (i << bits[i]) : 0)`.
pub const OFFSETS_BY_LENGTH: [u32; 32] = [
    0, 0, 0, 0, 0, 4096, 9216, 21504,
    35840, 44032, 53248, 63488, 74752, 87040, 93696, 100864,
    104704, 106752, 108928, 113536, 115968, 118528, 119872, 121280,
    122016, 122784, 122784, 122784, 122784, 122784, 122784, 122784,
];

/// Retrieves an immutable slice of a static dictionary word in $O(1)$ constant time.
///
/// # Arguments
/// * `len` - Word length in bytes (must satisfy `4 <= len <= 24`).
/// * `word_idx` - Zero-based index of the word within the length bucket
///   (must satisfy `word_idx < (1 << SIZE_BITS_BY_LENGTH[len])`).
///
/// # Returns
/// * `Some(&'static [u8])` containing the dictionary word if parameters are valid.
/// * `None` if `len` or `word_idx` is out of bounds (guaranteed 0 panic).
#[inline]
pub fn get_dictionary_word(len: usize, word_idx: usize) -> Option<&'static [u8]> {
    if !(MIN_DICTIONARY_WORD_LENGTH..=MAX_DICTIONARY_WORD_LENGTH).contains(&len) {
        return None;
    }

    let size_bits = SIZE_BITS_BY_LENGTH[len];
    if size_bits == 0 {
        return None;
    }

    let max_words = 1usize << size_bits;
    if word_idx >= max_words {
        return None;
    }

    let offset = OFFSETS_BY_LENGTH[len] as usize + word_idx * len;
    Some(&RFC7932_DICTIONARY_DATA[offset..offset + len])
}

/// Returns a reference to the entire 122,784-byte precompiled RFC 7932 dictionary.
#[inline]
pub fn rfc7932_dictionary_data() -> &'static [u8] {
    RFC7932_DICTIONARY_DATA
}

/// Returns the number of words in the static dictionary for the specified length.
#[inline]
pub fn dictionary_word_count(len: usize) -> usize {
    if len >= SIZE_BITS_BY_LENGTH.len() {
        return 0;
    }
    let bits = SIZE_BITS_BY_LENGTH[len];
    if bits == 0 {
        0
    } else {
        1usize << bits
    }
}
