// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration and property test suite for `BtMatchfinder` and `OptParser`.

use ttzip_engine::codecs::libdeflate::bt_matchfinder::{
    BtMatchfinder, BT_REQUIRED_NBYTES, DEFLATE_MAX_MATCH_LEN, DEFLATE_MIN_MATCH_LEN,
    MATCHFINDER_INITVAL, MATCHFINDER_WINDOW_SIZE,
};
use ttzip_engine::codecs::libdeflate::opt_parser::{
    build_matches_cache, compute_huffman_lengths, find_min_cost_path, optimize_parse_em,
    CostModel, OptimumNode, SequenceItem, DEFLATE_END_OF_BLOCK, DEFLATE_NUM_LITLEN_SYMS,
    MAX_HUFFMAN_CODE_LEN,
};

// MARK: - 1. BT Matchfinder Tests

#[test]
fn test_bt_matchfinder_init_and_reset() {
    let mut mf = BtMatchfinder::new();
    assert_eq!(mf.hash3_tab[0][0], MATCHFINDER_INITVAL);
    assert_eq!(mf.hash3_tab[65535][1], MATCHFINDER_INITVAL);
    assert_eq!(mf.hash4_tab[0], MATCHFINDER_INITVAL);
    assert_eq!(mf.hash4_tab[65535], MATCHFINDER_INITVAL);
    assert_eq!(mf.child_tab[0], MATCHFINDER_INITVAL);
    assert_eq!(mf.child_tab[65535], MATCHFINDER_INITVAL);

    // Dirty some state
    mf.hash3_tab[10][0] = 42;
    mf.hash4_tab[20] = 100;
    mf.child_tab[30] = 50;

    mf.reset();
    assert_eq!(mf.hash3_tab[10][0], MATCHFINDER_INITVAL);
    assert_eq!(mf.hash4_tab[20], MATCHFINDER_INITVAL);
    assert_eq!(mf.child_tab[30], MATCHFINDER_INITVAL);
}

#[test]
fn test_bt_matchfinder_monotonic_match_stream() {
    let mut mf = BtMatchfinder::new();
    let text = b"The quick brown fox jumps over the lazy dog. The quick brown fox jumps over the lazy dog.";
    let mut matches = Vec::new();

    for pos in 0..text.len() {
        mf.get_matches(text, pos, DEFLATE_MAX_MATCH_LEN, 128, 32, &mut matches);

        if !matches.is_empty() {
            // Verify strictly increasing match lengths
            for i in 0..matches.len() - 1 {
                assert!(
                    matches[i].length < matches[i + 1].length,
                    "Matches must be strictly monotonically increasing in length: {:?} at pos {}",
                    matches,
                    pos
                );
            }

            // Verify each match is accurate against source buffer
            for m in &matches {
                assert!(
                    (m.length as usize) >= DEFLATE_MIN_MATCH_LEN,
                    "Match length too short: {}",
                    m.length
                );
                assert!(
                    (m.offset as usize) <= MATCHFINDER_WINDOW_SIZE,
                    "Offset exceeds window: {}",
                    m.offset
                );
                assert!(
                    pos >= (m.offset as usize),
                    "Offset points ahead of current position"
                );

                let match_src = pos - (m.offset as usize);
                let len = m.length as usize;
                assert_eq!(
                    &text[match_src..match_src + len],
                    &text[pos..pos + len],
                    "Match mismatch at pos {} offset {} len {}",
                    pos,
                    m.offset,
                    len
                );
            }
        }
    }
}

#[test]
fn test_bt_matchfinder_repetitive_patterns() {
    let mut mf = BtMatchfinder::new();
    let data = vec![b'A'; 1024];
    let mut matches = Vec::new();

    for pos in 0..data.len() {
        mf.get_matches(&data, pos, DEFLATE_MAX_MATCH_LEN, 258, 64, &mut matches);
        if pos >= 3 && pos + BT_REQUIRED_NBYTES <= data.len() {
            assert!(
                !matches.is_empty(),
                "Expected matches in repetitive stream at pos {}",
                pos
            );
            let last_match = matches.last().unwrap();
            let expected_len = (data.len() - pos).min(DEFLATE_MAX_MATCH_LEN);
            assert_eq!(
                last_match.length as usize, expected_len,
                "Expected longest match of length {} at pos {}",
                expected_len, pos
            );
        }
    }
}

#[test]
fn test_bt_matchfinder_sliding_window() {
    let mut mf = BtMatchfinder::new();
    mf.hash3_tab[0][0] = 1000;
    mf.hash4_tab[0] = 2000;
    mf.child_tab[0] = 500;

    mf.slide_window();
    assert_eq!(
        mf.hash3_tab[0][0],
        (1000 - MATCHFINDER_WINDOW_SIZE as i32) as i16
    );
    assert_eq!(
        mf.hash4_tab[0],
        (2000 - MATCHFINDER_WINDOW_SIZE as i32) as i16
    );
    assert_eq!(
        mf.child_tab[0],
        (500 - MATCHFINDER_WINDOW_SIZE as i32) as i16
    );

    // Negative values clamp to MATCHFINDER_INITVAL
    mf.slide_window();
    assert_eq!(mf.hash3_tab[0][0], MATCHFINDER_INITVAL);
    assert_eq!(mf.hash4_tab[0], MATCHFINDER_INITVAL);
    assert_eq!(mf.child_tab[0], MATCHFINDER_INITVAL);
}

// MARK: - 2. Dynamic Programming Optimal Parser Tests

#[test]
fn test_opt_parser_node_packing() {
    let node_lit = OptimumNode {
        cost_to_end: 120,
        item: OptimumNode::pack_literal(b'Z'),
    };
    assert!(node_lit.is_literal());
    assert_eq!(node_lit.unpack(), SequenceItem::Literal(b'Z'));

    let node_match = OptimumNode {
        cost_to_end: 450,
        item: OptimumNode::pack_match(15, 250),
    };
    assert!(!node_match.is_literal());
    assert_eq!(
        node_match.unpack(),
        SequenceItem::Match {
            length: 15,
            offset: 250
        }
    );
}

#[test]
fn test_offset_slot_mapping_consistency() {
    // Check known offset bounds
    assert_eq!(CostModel::offset_slot(1), 0);
    assert_eq!(CostModel::offset_slot(2), 1);
    assert_eq!(CostModel::offset_slot(3), 2);
    assert_eq!(CostModel::offset_slot(4), 3);
    assert_eq!(CostModel::offset_slot(5), 4);
    assert_eq!(CostModel::offset_slot(6), 4);
    assert_eq!(CostModel::offset_slot(7), 5);
    assert_eq!(CostModel::offset_slot(8), 5);
    assert_eq!(CostModel::offset_slot(9), 6);
    assert_eq!(CostModel::offset_slot(32768), 29);
}

#[test]
fn test_dp_backward_path_optimality() {
    let mut mf = BtMatchfinder::new();
    let text = b"abcdefg_abcdefg_abcdefg_abcdefg";
    let cache = build_matches_cache(&mut mf, text, 128, 32);
    let costs = CostModel::default_uniform();

    let seq = find_min_cost_path(text, &cache, &costs);
    assert!(!seq.is_empty());

    // Verify reconstruction produces identical uncompressed stream
    let mut reconstructed = Vec::new();
    for item in &seq {
        match *item {
            SequenceItem::Literal(b) => reconstructed.push(b),
            SequenceItem::Match { length, offset } => {
                let start = reconstructed.len() - (offset as usize);
                for i in 0..length as usize {
                    reconstructed.push(reconstructed[start + i]);
                }
            }
        }
    }
    assert_eq!(&reconstructed, text);
}

#[test]
fn test_em_optimization_improves_or_maintains_bit_cost() {
    let mut mf = BtMatchfinder::new();
    let text = b"The quick brown fox jumps over the lazy dog. Pack my box with five dozen liquor jugs. The quick brown fox jumps over the lazy dog.";
    let cache = build_matches_cache(&mut mf, text, 128, 32);

    let (seq_1_pass, cost_1) = optimize_parse_em(text, &cache, 1);
    let (seq_4_pass, cost_4) = optimize_parse_em(text, &cache, 4);

    assert!(!seq_1_pass.is_empty());
    assert!(!seq_4_pass.is_empty());
    assert!(
        cost_4 <= cost_1,
        "EM multi-pass refinement must not regress bit cost: pass1={}, pass4={}",
        cost_1,
        cost_4
    );

    // Verify lossless reconstruction for both passes
    for (name, seq) in [("pass1", &seq_1_pass), ("pass4", &seq_4_pass)] {
        let mut reconstructed = Vec::new();
        for item in seq {
            match *item {
                SequenceItem::Literal(b) => reconstructed.push(b),
                SequenceItem::Match { length, offset } => {
                    let start = reconstructed.len() - (offset as usize);
                    for i in 0..length as usize {
                        reconstructed.push(reconstructed[start + i]);
                    }
                }
            }
        }
        assert_eq!(&reconstructed, text, "Lossless reconstruction failed for {}", name);
    }
}

// MARK: - 3. Compression Ratio Advantage over Greedy Parsing

#[test]
fn test_compression_ratio_superiority_over_greedy() {
    // Construct classic "lazy/optimal parse advantage" payload:
    // "ABXYZ...ABX" where matching "AB" early is suboptimal compared to matching "ABXYZ" later.
    let pattern = b"abcdefgh_123456_abcdefgh_abcdefgh_123456_abcdefgh_xyz";
    let mut payload = Vec::new();
    for _ in 0..16 {
        payload.extend_from_slice(pattern);
    }

    let mut mf = BtMatchfinder::new();
    let cache = build_matches_cache(&mut mf, &payload, 128, 32);

    // 1. Compute Greedy parsing token count
    let mut greedy_items = 0usize;
    let mut pos = 0;
    while pos < payload.len() {
        if pos < cache.len() && !cache[pos].is_empty() {
            let best_match = cache[pos].last().unwrap();
            pos += best_match.length as usize;
        } else {
            pos += 1;
        }
        greedy_items += 1;
    }

    // 2. Compute DP Optimal parsing
    let (opt_seq, opt_bits) = optimize_parse_em(&payload, &cache, 4);

    assert!(
        opt_seq.len() <= greedy_items,
        "Optimal DP parse items ({}) should be <= greedy items ({})",
        opt_seq.len(),
        greedy_items
    );
    assert!(opt_bits > 0);
}

// MARK: - 4. Robustness on Edge Cases and Tiny Data Blocks

#[test]
fn test_tiny_and_boundary_blocks() {
    let mut mf = BtMatchfinder::new();

    // 0 Bytes
    let empty: &[u8] = b"";
    let cache_0 = build_matches_cache(&mut mf, empty, 32, 16);
    assert!(cache_0.is_empty());
    let (seq_0, cost_0) = optimize_parse_em(empty, &cache_0, 4);
    assert!(seq_0.is_empty());
    assert_eq!(cost_0, 0);

    // 1 Byte
    let one_byte: &[u8] = b"Q";
    let cache_1 = build_matches_cache(&mut mf, one_byte, 32, 16);
    assert_eq!(cache_1.len(), 1);
    let (seq_1, cost_1) = optimize_parse_em(one_byte, &cache_1, 4);
    assert_eq!(seq_1.len(), 1);
    assert_eq!(seq_1[0], SequenceItem::Literal(b'Q'));
    assert!(cost_1 > 0);

    // 10 Bytes
    let ten_bytes: &[u8] = b"0123456789";
    let cache_10 = build_matches_cache(&mut mf, ten_bytes, 32, 16);
    assert_eq!(cache_10.len(), 10);
    let (seq_10, cost_10) = optimize_parse_em(ten_bytes, &cache_10, 4);
    assert_eq!(seq_10.len(), 10);
    for (i, item) in seq_10.iter().enumerate() {
        assert_eq!(*item, SequenceItem::Literal(ten_bytes[i]));
    }
    assert!(cost_10 > 0);
}

#[test]
fn test_package_merge_huffman_lengths() {
    let mut freqs = [0u32; DEFLATE_NUM_LITLEN_SYMS];
    freqs[b'a' as usize] = 100;
    freqs[b'b' as usize] = 50;
    freqs[b'c' as usize] = 25;
    freqs[b'd' as usize] = 12;
    freqs[DEFLATE_END_OF_BLOCK] = 1;

    let lens = compute_huffman_lengths(&freqs, MAX_HUFFMAN_CODE_LEN);

    // Most frequent symbol ('a') must have shortest or equal codeword length
    assert!(lens[b'a' as usize] > 0);
    assert!(lens[b'b' as usize] >= lens[b'a' as usize]);
    assert!(lens[b'c' as usize] >= lens[b'b' as usize]);
    assert!(lens[b'd' as usize] >= lens[b'c' as usize]);
    assert!(lens[DEFLATE_END_OF_BLOCK] >= lens[b'd' as usize]);

    // Invariant: all code lengths must be <= 15
    for &l in &lens {
        assert!(l as usize <= MAX_HUFFMAN_CODE_LEN);
    }
}
