// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive test suite for RFC 7932 Brotli 121 Static Transforms and UTF-8 Word Restructurer.

use ttzip_engine::codecs::brotli::{
    get_prefix_suffix, shift_utf8, to_uppercase_utf8, transform_dictionary_word, BrotliError,
    TransformTriplet, WordTransformOp, CUTOFF_TRANSFORMS, PREFIX_SUFFIX_MAP, PREFIX_SUFFIX_STORAGE,
    TRANSFORMS_TABLE,
};

// MARK: - 1. Storage Pool & Constants Invariants

#[test]
fn test_prefix_suffix_storage_and_map_invariants() {
    assert_eq!(PREFIX_SUFFIX_STORAGE.len(), 217);
    assert_eq!(PREFIX_SUFFIX_MAP.len(), 50);
    assert_eq!(TRANSFORMS_TABLE.len(), 121);
    assert_eq!(CUTOFF_TRANSFORMS.len(), 10);

    // Verify all 50 offsets in map are strictly within storage bounds
    for (i, &offset) in PREFIX_SUFFIX_MAP.iter().enumerate() {
        let off = offset as usize;
        assert!(
            off < PREFIX_SUFFIX_STORAGE.len(),
            "Offset at map index {} is out of bounds: {}",
            i,
            off
        );
        let len = PREFIX_SUFFIX_STORAGE[off] as usize;
        assert!(
            off + 1 + len <= PREFIX_SUFFIX_STORAGE.len(),
            "Length {} at offset {} exceeds storage pool at map index {}",
            len,
            off,
            i
        );
    }
}

#[test]
fn test_get_prefix_suffix_entries() {
    // ID 0 -> " "
    assert_eq!(get_prefix_suffix(0), b" ");
    // ID 1 -> ", "
    assert_eq!(get_prefix_suffix(1), b", ");
    // ID 2 -> " of the "
    assert_eq!(get_prefix_suffix(2), b" of the ");
    // ID 3 -> " of "
    assert_eq!(get_prefix_suffix(3), b" of ");
    // ID 4 -> "s "
    assert_eq!(get_prefix_suffix(4), b"s ");
    // ID 5 -> "."
    assert_eq!(get_prefix_suffix(5), b".");
    // ID 6 -> " and "
    assert_eq!(get_prefix_suffix(6), b" and ");
    // ID 7 -> " in "
    assert_eq!(get_prefix_suffix(7), b" in ");
    // ID 11 -> "\n"
    assert_eq!(get_prefix_suffix(11), b"\n");
    // ID 22 -> ". The "
    assert_eq!(get_prefix_suffix(22), b". The ");
    // ID 26 -> "ing "
    assert_eq!(get_prefix_suffix(26), b"ing ");
    // ID 30 -> "=\""
    assert_eq!(get_prefix_suffix(30), b"=\"");
    // ID 35 -> ".com/"
    assert_eq!(get_prefix_suffix(35), b".com/");
    // ID 45 -> UTF-8 non-breaking space (0xC2 0xA0)
    assert_eq!(get_prefix_suffix(45), b"\xc2\xa0");
    // ID 47 -> " the "
    assert_eq!(get_prefix_suffix(47), b" the ");
    // ID 48 -> "e "
    assert_eq!(get_prefix_suffix(48), b"e ");
    // ID 49 -> empty string ""
    assert_eq!(get_prefix_suffix(49), b"");
    // Invalid IDs -> empty slice
    assert_eq!(get_prefix_suffix(50), b"");
    assert_eq!(get_prefix_suffix(255), b"");
}

// MARK: - 2. All 121 RFC 7932 Transforms Parity

#[test]
fn test_all_121_transforms_table_structure() {
    assert_eq!(TRANSFORMS_TABLE.len(), 121);

    for (idx, triplet) in TRANSFORMS_TABLE.iter().enumerate() {
        assert!(
            triplet.prefix_id < 50,
            "Transform {} has invalid prefix_id: {}",
            idx,
            triplet.prefix_id
        );
        assert!(
            triplet.suffix_id < 50,
            "Transform {} has invalid suffix_id: {}",
            idx,
            triplet.suffix_id
        );
        assert!(
            (triplet.op.as_u8()) <= 22,
            "Transform {} has invalid op: {:?}",
            idx,
            triplet.op
        );
    }
}

#[test]
fn test_cutoff_transforms_table() {
    let expected_cutoffs = [0, 12, 27, 23, 42, 63, 56, 48, 59, 64];
    assert_eq!(CUTOFF_TRANSFORMS, expected_cutoffs);

    // Index 0 must be ["", Identity, ""]
    let t0 = TRANSFORMS_TABLE[CUTOFF_TRANSFORMS[0]];
    assert_eq!(t0, TransformTriplet::new(49, WordTransformOp::Identity, 49));

    // Index 1 must be ["", OmitLast1, ""]
    let t1 = TRANSFORMS_TABLE[CUTOFF_TRANSFORMS[1]];
    assert_eq!(t1, TransformTriplet::new(49, WordTransformOp::OmitLast1, 49));

    // Index 2 must be ["", OmitLast2, ""]
    let t2 = TRANSFORMS_TABLE[CUTOFF_TRANSFORMS[2]];
    assert_eq!(t2, TransformTriplet::new(49, WordTransformOp::OmitLast2, 49));

    // Index 3 must be ["", OmitLast3, ""]
    let t3 = TRANSFORMS_TABLE[CUTOFF_TRANSFORMS[3]];
    assert_eq!(t3, TransformTriplet::new(49, WordTransformOp::OmitLast3, 49));

    // Index 9 must be ["", OmitLast9, ""]
    let t9 = TRANSFORMS_TABLE[CUTOFF_TRANSFORMS[9]];
    assert_eq!(t9, TransformTriplet::new(49, WordTransformOp::OmitLast9, 49));
}

// MARK: - 3. Typical Word Transformations

#[test]
fn test_typical_transformations() {
    let mut dst = [0u8; 128];

    // Transform 0: ["", Identity, ""] -> "time" -> "time"
    let len = transform_dictionary_word(&mut dst, b"time", 0).expect("transform 0");
    assert_eq!(&dst[..len], b"time");

    // Transform 1: ["", Identity, " "] -> "time" -> "time "
    let len = transform_dictionary_word(&mut dst, b"time", 1).expect("transform 1");
    assert_eq!(&dst[..len], b"time ");

    // Transform 2: [" ", Identity, " "] -> "time" -> " time "
    let len = transform_dictionary_word(&mut dst, b"time", 2).expect("transform 2");
    assert_eq!(&dst[..len], b" time ");

    // Transform 3: ["", OmitFirst1, ""] -> "time" -> "ime"
    let len = transform_dictionary_word(&mut dst, b"time", 3).expect("transform 3");
    assert_eq!(&dst[..len], b"ime");

    // Transform 4: ["", UppercaseFirst, " "] -> "word" -> "Word "
    let len = transform_dictionary_word(&mut dst, b"word", 4).expect("transform 4");
    assert_eq!(&dst[..len], b"Word ");

    // Transform 44: ["", UppercaseAll, ""] -> "state" -> "STATE"
    let len = transform_dictionary_word(&mut dst, b"state", 44).expect("transform 44");
    assert_eq!(&dst[..len], b"STATE");

    // Transform 49: ["", OmitLast1, "ing "] -> "make" -> "making "
    let len = transform_dictionary_word(&mut dst, b"make", 49).expect("transform 49");
    assert_eq!(&dst[..len], b"making ");

    // Transform 72: [".com/", Identity, ""] -> "Google" -> ".com/Google"
    let len = transform_dictionary_word(&mut dst, b"Google", 72).expect("transform 72");
    assert_eq!(&dst[..len], b".com/Google");

    // Transform 43: ["", Identity, ". The "] -> "action" -> "action. The "
    let len = transform_dictionary_word(&mut dst, b"action", 43).expect("transform 43");
    assert_eq!(&dst[..len], b"action. The ");

    // Transform 41: [" the ", Identity, ""] -> "action" -> " the action"
    let len = transform_dictionary_word(&mut dst, b"action", 41).expect("transform 41");
    assert_eq!(&dst[..len], b" the action");

    // Transform 7: ["s ", Identity, " "] -> "apple" -> "s apple "
    let len = transform_dictionary_word(&mut dst, b"apple", 7).expect("transform 7");
    assert_eq!(&dst[..len], b"s apple ");

    // Transform 70: ["", Identity, "=\""] -> "href" -> "href=\""
    let len = transform_dictionary_word(&mut dst, b"href", 70).expect("transform 70");
    assert_eq!(&dst[..len], b"href=\"");
}

// MARK: - 4. Multi-byte UTF-8 Transformation Safety

#[test]
fn test_utf8_uppercase_transformations() {
    let mut buf_ascii = *b"hello";
    let step = to_uppercase_utf8(&mut buf_ascii);
    assert_eq!(step, 1);
    assert_eq!(&buf_ascii, b"Hello");

    // 2-byte UTF-8 Cyrillic small letter 'a' (U+0430: 0xD0 0xB0) -> (U+0410: 0xD0 0x90)
    let mut buf_cyrillic = [0xD0, 0xB0];
    let step = to_uppercase_utf8(&mut buf_cyrillic);
    assert_eq!(step, 2);
    assert_eq!(buf_cyrillic, [0xD0, 0x90]);

    // 3-byte UTF-8 Chinese character (U+4F60: 0xE4 0xBD 0xA0) -> 3rd byte XORed with 5 (0xA0 ^ 5 = 0xA5)
    let mut buf_cjk = [0xE4, 0xBD, 0xA0];
    let step = to_uppercase_utf8(&mut buf_cjk);
    assert_eq!(step, 3);
    assert_eq!(buf_cjk, [0xE4, 0xBD, 0xA5]);

    // Test UppercaseAll with mixed ASCII and multibyte UTF-8
    let mut dst = [0u8; 64];
    let mixed = [b'a', 0xD0, 0xB0, b'b', 0xE4, 0xBD, 0xA0];
    let len = transform_dictionary_word(&mut dst, &mixed, 44).expect("uppercase all");
    assert_eq!(len, 7);
    assert_eq!(dst[0], b'A');
    assert_eq!(dst[1..3], [0xD0, 0x90]);
    assert_eq!(dst[3], b'B');
    assert_eq!(dst[4..7], [0xE4, 0xBD, 0xA5]);
}

#[test]
fn test_utf8_shift_transformations() {
    let mut ascii = *b"a";
    let step = shift_utf8(&mut ascii, 1);
    assert_eq!(step, 1);
    assert_eq!(ascii[0], b'b');

    let mut cyrillic = [0xD0, 0xB0];
    let step = shift_utf8(&mut cyrillic, 1);
    assert_eq!(step, 2);

    let mut cjk = [0xE4, 0xBD, 0xA0];
    let step = shift_utf8(&mut cjk, 1);
    assert_eq!(step, 3);

    let mut emoji = [0xF0, 0x9F, 0x98, 0x80];
    let step = shift_utf8(&mut emoji, 1);
    assert_eq!(step, 4);
}

// MARK: - 5. Boundary Protection and 0-Panic Invariants

#[test]
fn test_invalid_transform_index_handling() {
    let mut dst = [0u8; 64];
    let word = b"test";

    // Valid boundary: 120
    assert!(transform_dictionary_word(&mut dst, word, 120).is_ok());

    // Invalid boundaries: >= 121
    assert_eq!(
        transform_dictionary_word(&mut dst, word, 121),
        Err(BrotliError::InvalidTransformIndex(121))
    );
    assert_eq!(
        transform_dictionary_word(&mut dst, word, 9999),
        Err(BrotliError::InvalidTransformIndex(9999))
    );
}

#[test]
fn test_destination_buffer_too_small() {
    let mut dst = [0u8; 3];
    // Transform 1 requires 4 + 1 = 5 bytes for "test" -> "test "
    let res = transform_dictionary_word(&mut dst, b"test", 1);
    assert_eq!(
        res,
        Err(BrotliError::BufferTooSmall {
            required: 5,
            available: 3,
        })
    );
}

#[test]
fn test_truncated_utf8_slices_zero_panic() {
    // Truncated 2-byte slice (only 1 byte provided)
    let mut truncated_2 = [0xD0];
    let step = to_uppercase_utf8(&mut truncated_2);
    assert_eq!(step, 1);

    // Truncated 3-byte slice (only 2 bytes provided)
    let mut truncated_3 = [0xE4, 0xBD];
    let step = to_uppercase_utf8(&mut truncated_3);
    assert_eq!(step, 2);

    // Empty slice
    let mut empty: [u8; 0] = [];
    assert_eq!(to_uppercase_utf8(&mut empty), 0);
    assert_eq!(shift_utf8(&mut empty, 10), 0);
}

#[test]
fn test_omit_lengths_exceeding_word_length() {
    let mut dst = [0u8; 32];
    // OmitLast3 on a 2-byte word
    // Transform 23 is ["", OmitLast3, ""]
    let len = transform_dictionary_word(&mut dst, b"ab", 23).expect("omit exceeds");
    assert_eq!(len, 0);

    // OmitFirst4 on a 2-byte word
    // Transform 34 is ["", OmitFirst4, ""]
    let len = transform_dictionary_word(&mut dst, b"ab", 34).expect("omit exceeds");
    assert_eq!(len, 0);
}

#[test]
fn test_word_transform_op_enum_conversions() {
    for val in 0..=22 {
        let op = WordTransformOp::from_u8(val).expect("valid op");
        assert_eq!(op.as_u8(), val);
    }
    assert!(WordTransformOp::from_u8(23).is_none());
    assert!(WordTransformOp::from_u8(255).is_none());
}
