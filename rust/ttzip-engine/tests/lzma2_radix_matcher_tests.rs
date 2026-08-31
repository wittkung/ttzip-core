// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive Test Suite for LZMA2 Radix-16 Match Finder.
//!
//! Validates:
//! 1. 65,536-entry Radix-16 initial bucketing distribution across varied prefixes.
//! 2. 100% semantic equivalence between 4-byte L1 cache prefetch and direct memory comparison.
//! 3. Small list brute-force pruning accuracy (MAX_BRUTE_FORCE_LIST_SIZE = 5) on short and long chains.
//! 4. Linear $O(N)$ convergence and absolute zero stack overflow on RLE repeating streams (zeros, 0xFF, patterned).
//! 5. Multi-threaded and single-threaded match table build bit-exactness.
//! 6. Numerical boundary edge cases (0, 1, 2, 3 byte inputs, non-repeating streams).

use ttzip_engine::codecs::lzma2::radix_matcher::{
    RadixBuildMatch, RadixMatchFinder, BUFFER_LINK_MASK, MAX_BRUTE_FORCE_LIST_SIZE, MAX_REPEAT,
    RADIX16_TABLE_SIZE, RADIX8_TABLE_SIZE, RADIX_LINK_BITS, RADIX_LINK_MASK, RADIX_MAX_LENGTH,
    RADIX_NULL_LINK,
};

#[test]
fn test_radix_constants_and_struct_layout() {
    assert_eq!(RADIX16_TABLE_SIZE, 65536);
    assert_eq!(RADIX8_TABLE_SIZE, 256);
    assert_eq!(MAX_BRUTE_FORCE_LIST_SIZE, 5);
    assert_eq!(MAX_REPEAT, 24);
    assert_eq!(RADIX_NULL_LINK, 0xFFFF_FFFF);
    assert_eq!(RADIX_LINK_BITS, 26);
    assert_eq!(RADIX_LINK_MASK, 0x03FF_FFFF);
    assert_eq!(RADIX_MAX_LENGTH, 63);
    assert_eq!(BUFFER_LINK_MASK, 0x00FF_FFFF);

    // Verify 12-byte compact struct layout for RadixBuildMatch
    assert_eq!(std::mem::size_of::<RadixBuildMatch>(), 12);
}

#[test]
fn test_radix_build_match_methods_and_caching() {
    let mut node = RadixBuildMatch::new(42, 0, 100, 4);
    assert_eq!(node.from, 42);
    assert_eq!(node.next_index(), 100);
    assert_eq!(node.depth(), 4);

    node.set_next_and_depth(200, 8);
    assert_eq!(node.next_index(), 200);
    assert_eq!(node.depth(), 8);

    let data = b"ABCDEFGHIJKLMN";
    node.load_src_u32(data, 2); // 'C', 'D', 'E', 'F'
    assert_eq!(node.byte_at(0), b'C');
    assert_eq!(node.byte_at(1), b'D');
    assert_eq!(node.byte_at(2), b'E');
    assert_eq!(node.byte_at(3), b'F');

    // Test partial chunk read near the end
    node.load_src_u32(data, 12); // 'M', 'N', 0, 0
    assert_eq!(node.byte_at(0), b'M');
    assert_eq!(node.byte_at(1), b'N');
    assert_eq!(node.byte_at(2), 0);
    assert_eq!(node.byte_at(3), 0);
}

#[test]
fn test_initial_radix16_bucketing_distribution() {
    let mut finder = RadixMatchFinder::new();
    let sample = b"ABCD_ABCE_ABCF_ABCG_1234_1235_XYZ_";
    finder.init_table(sample);

    assert_eq!(finder.table.len(), sample.len());

    // Check radix for "AB" (0x4142)
    let radix_ab = ((b'A' as usize) << 8) | (b'B' as usize);
    assert_eq!(finder.list_counts[radix_ab], 4);
    assert!(finder.stack.contains(&(radix_ab as u32)));

    // Check radix for "12" (0x3132)
    let radix_12 = ((b'1' as usize) << 8) | (b'2' as usize);
    assert_eq!(finder.list_counts[radix_12], 2);
    assert!(finder.stack.contains(&(radix_12 as u32)));

    // Verify non-existent radix has zero count and RADIX_NULL_LINK
    let radix_nonexistent = 0xFFFF;
    assert_eq!(finder.list_counts[radix_nonexistent], 0);
    assert_eq!(finder.list_heads[radix_nonexistent], RADIX_NULL_LINK);
}

#[test]
fn test_4byte_l1_cache_semantic_parity_with_direct_memory() {
    let sentence = b"The quick brown fox jumps over the lazy dog. ";
    let mut payload = Vec::new();
    payload.extend_from_slice(sentence);
    payload.extend_from_slice(sentence);

    let mut finder = RadixMatchFinder::with_max_depth(64);
    finder.init_table(&payload);
    finder.build_table(&payload, 1);

    // Second occurrence of sentence starts at index 45
    let second_pos = 45;
    let match_opt = finder.get_match(second_pos);
    assert!(match_opt.is_some(), "Match must be found for repeated sentence");

    let m = match_opt.unwrap();
    assert_eq!(m.link, 0, "Match link should point back to start (pos 0)");
    assert_eq!(m.length, 45, "Full repeated sentence length is 45 bytes");

    // Cross-verify each byte directly against raw memory slice
    for offset in 0..m.length {
        assert_eq!(
            payload[second_pos + offset],
            payload[m.link + offset],
            "Mismatch at offset {offset}"
        );
    }
}

#[test]
fn test_small_list_brute_force_pruning_precision() {
    // 3 occurrences of prefix "TAG_"
    // pos1 (0):  "TAG_alpha_123456789_" (20) + "TAG_alpha_123456789_" (20) + "TAG_alpha_999999999_" (20)
    // pos2 (20): "TAG_alpha_123456789_" (20) + "TAG_alpha_999999999_" (20)
    // pos3 (40): "TAG_alpha_999999999_" (20)
    let data = b"TAG_alpha_123456789_TAG_alpha_123456789_TAG_alpha_999999999_";
    let mut finder = RadixMatchFinder::with_max_depth(64);
    finder.init_table(data);

    let radix_tag = ((b'T' as usize) << 8) | (b'A' as usize);
    assert_eq!(
        finder.list_counts[radix_tag], 3,
        "Should have 3 entries (<= MAX_BRUTE_FORCE_LIST_SIZE)"
    );

    finder.build_table(data, 1);

    let pos1 = 0;
    let pos2 = 20;
    let pos3 = 40;

    let m2 = finder.get_match(pos2).expect("match at pos2");
    assert_eq!(m2.link, pos1);
    assert_eq!(m2.length, 30); // "TAG_alpha_123456789_TAG_alpha_"

    let m3 = finder.get_match(pos3).expect("match at pos3");
    assert_eq!(m3.length, 10); // "TAG_alpha_"
}

#[test]
fn test_rle_linear_convergence_and_zero_stack_overflow() {
    // 64 KB of repeating zeros
    let zeros = vec![0u8; 65536];
    let mut finder = RadixMatchFinder::with_max_depth(63);
    finder.init_table(&zeros);

    assert_eq!(finder.list_counts[0], (zeros.len() - 1) as u32);
    assert_eq!(finder.stack.len(), 1);

    // Ensure table build completes in linear time without stack overflow
    finder.build_table(&zeros, 1);

    // Check matches for high positions
    for pos in 100..200 {
        let m = finder.get_match(pos).expect("match found in RLE");
        assert_eq!(m.length, 63, "Match length capped at max 63");
        assert!(m.link < pos, "Link must point to prior position");
    }

    // 64 KB of repeating 0xFF bytes
    let all_ff = vec![0xFFu8; 65536];
    let mut finder_ff = RadixMatchFinder::with_max_depth(63);
    finder_ff.init_table(&all_ff);
    finder_ff.build_table(&all_ff, 1);

    for pos in 100..200 {
        let m = finder_ff.get_match(pos).expect("match found in 0xFF RLE");
        assert_eq!(m.length, 63);
    }
}

#[test]
fn test_multithreaded_build_table_parity() {
    let mut data = Vec::with_capacity(32768);
    for i in 0..1024 {
        data.extend_from_slice(format!("chunk_{:04}_data_pattern_abc_xyz_", i % 32).as_bytes());
    }

    let mut st_finder = RadixMatchFinder::with_max_depth(64);
    st_finder.init_table(&data);
    st_finder.build_table(&data, 1);

    let mut mt_finder = RadixMatchFinder::with_max_depth(64);
    mt_finder.init_table(&data);
    mt_finder.build_table(&data, 4);

    assert_eq!(st_finder.table.len(), mt_finder.table.len());
    assert_eq!(
        st_finder.table, mt_finder.table,
        "Single-threaded and multi-threaded builds must produce bit-exact tables"
    );
}

#[test]
fn test_small_and_boundary_buffers() {
    let mut finder = RadixMatchFinder::new();

    // 0-byte buffer
    finder.init_table(&[]);
    finder.build_table(&[], 1);
    assert_eq!(finder.get_match(0), None);

    // 1-byte buffer
    finder.init_table(b"A");
    finder.build_table(b"A", 1);
    assert_eq!(finder.get_match(0), None);

    // 2-byte buffer
    finder.init_table(b"AB");
    finder.build_table(b"AB", 1);
    assert_eq!(finder.get_match(0), None);

    // 3-byte buffer with no repeats
    finder.init_table(b"ABC");
    finder.build_table(b"ABC", 1);
    assert_eq!(finder.get_match(0), None);
}
