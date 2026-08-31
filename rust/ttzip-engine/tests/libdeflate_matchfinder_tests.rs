// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Exhaustive verification suite for Libdeflate `HtMatchfinder` and `HcMatchfinder`,
//! multiplicative hash functions, SWAR word match extension, and 32KB window rebasing.

use ttzip_engine::codecs::libdeflate::matchfinder::{
    lz_extend, lz_hash, rebase_pos, HcMatchfinder, HtMatchfinder, HC_HASH3_ORDER, HC_HASH3_SIZE,
    HC_HASH4_ORDER, HC_HASH4_SIZE, HC_MIN_MATCH_LEN, HT_BUCKET_SIZE, HT_HASH_ORDER,
    HT_HASH_SIZE, HT_MIN_MATCH_LEN, LZ_HASH_MULTIPLIER, MATCHFINDER_INITVAL,
    MATCHFINDER_WINDOW_SIZE, MAX_MATCH_LEN, MIN_MATCH_LEN, WINDOW_SIZE,
};

// MARK: - Helper Functions

/// Generates deterministic pseudo-random sequence for testing.
fn generate_pseudo_random(len: usize, seed: u32) -> Vec<u8> {
    let mut state = seed;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        out.push((state >> 24) as u8);
    }
    out
}

/// Generates repeating pattern buffer of specified length.
fn generate_repeated_pattern(pattern: &[u8], total_len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(total_len);
    while out.len() < total_len {
        let take = pattern.len().min(total_len - out.len());
        out.extend_from_slice(&pattern[..take]);
    }
    out
}

// MARK: - 1. Constant Invariants & Multiplicative Hash Tests

#[test]
fn test_matchfinder_constants() {
    assert_eq!(WINDOW_SIZE, 32768);
    assert_eq!(MATCHFINDER_WINDOW_SIZE, 32768);
    assert_eq!(HT_HASH_ORDER, 15);
    assert_eq!(HT_HASH_SIZE, 32768);
    assert_eq!(HT_BUCKET_SIZE, 2);
    assert_eq!(HC_HASH3_ORDER, 15);
    assert_eq!(HC_HASH3_SIZE, 32768);
    assert_eq!(HC_HASH4_ORDER, 16);
    assert_eq!(HC_HASH4_SIZE, 65536);
    assert_eq!(MATCHFINDER_INITVAL, -32768);
    assert_eq!(MIN_MATCH_LEN, 3);
    assert_eq!(MAX_MATCH_LEN, 258);
    assert_eq!(HT_MIN_MATCH_LEN, 4);
    assert_eq!(HC_MIN_MATCH_LEN, 3);
    assert_eq!(LZ_HASH_MULTIPLIER, 0x1E35_A7BD);
}

#[test]
fn test_multiplicative_lz_hash() {
    // Verify hash bounds for order 15 (0..32767) and order 16 (0..65535)
    for seq in [
        0u32,
        0x12345678,
        0xFFFFFFFF,
        0x00000001,
        0xDEADBEEF,
        0xCAFEBABE,
    ] {
        let h15 = lz_hash(seq, 15);
        assert!(h15 < 32768, "h15 ({h15}) must be < 32768");

        let h16 = lz_hash(seq, 16);
        assert!(h16 < 65536, "h16 ({h16}) must be < 65536");
    }

    // Verify deterministic mapping
    let h1 = lz_hash(0x11223344, 15);
    let h2 = lz_hash(0x11223344, 15);
    assert_eq!(h1, h2);
}

#[test]
fn test_rebase_pos_arithmetic() {
    assert_eq!(rebase_pos(0), -32768);
    assert_eq!(rebase_pos(100), (100i32 - 32768) as i16);
    assert_eq!(rebase_pos(32000), (32000i32 - 32768) as i16);
    assert_eq!(rebase_pos(32767), (32767i32 - 32768) as i16);
    assert_eq!(rebase_pos(-1), -32768);
    assert_eq!(rebase_pos(-768), -32768);
    assert_eq!(rebase_pos(-32768), -32768);
}

// MARK: - 2. SWAR Match Extension Tests

#[test]
fn test_swar_lz_extend_identical() {
    let data = b"abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let cur_pos = 0;
    let match_pos = 0;
    let ext = lz_extend(data, cur_pos, match_pos, 0, data.len());
    assert_eq!(ext, data.len());
}

#[test]
fn test_swar_lz_extend_partial_match() {
    let mut data = Vec::new();
    data.extend_from_slice(b"prefix_1234567890_same_end_A_remainder");
    let match_pos = 0;
    let cur_pos = data.len();
    data.extend_from_slice(b"prefix_1234567890_same_end_B_remainder");

    // Starts matching at 0 and cur_pos
    let ext = lz_extend(&data, cur_pos, match_pos, 0, 100);
    // "prefix_1234567890_same_end_" is 27 bytes long
    assert_eq!(ext, 27);
}

#[test]
fn test_swar_lz_extend_various_misalignments() {
    for offset in 0..16 {
        let mut data = vec![0u8; 100];
        for i in 0..30 {
            data[offset + i] = b'a' + (i % 26) as u8;
            data[offset + 40 + i] = b'a' + (i % 26) as u8;
        }
        // Differentiate at byte 25
        data[offset + 25] = b'X';
        data[offset + 40 + 25] = b'Y';

        let ext = lz_extend(&data, offset + 40, offset, 0, 100);
        assert_eq!(ext, 25);
    }
}

// MARK: - 3. HtMatchfinder Tests

#[test]
fn test_ht_matchfinder_init_and_reset() {
    let mut mf = HtMatchfinder::new();
    for bucket in mf.hash_tab.iter() {
        assert_eq!(bucket[0], MATCHFINDER_INITVAL);
        assert_eq!(bucket[1], MATCHFINDER_INITVAL);
    }

    // Mutate and reset
    mf.hash_tab[0][0] = 42;
    mf.hash_tab[0][1] = 43;
    mf.reset();
    assert_eq!(mf.hash_tab[0][0], MATCHFINDER_INITVAL);
    assert_eq!(mf.hash_tab[0][1], MATCHFINDER_INITVAL);
}

#[test]
fn test_ht_matchfinder_all_repeat_string() {
    let mut mf = HtMatchfinder::new();
    let data = vec![b'A'; 1000];

    // At position 0, no previous match
    let (len0, off0) = mf.longest_match(&data, 0, MAX_MATCH_LEN, MAX_MATCH_LEN);
    assert_eq!(len0, 0);
    assert_eq!(off0, 0);

    // At position 1..100, we should find matches with offset = 1 and max_len up to 258
    for pos in 1..100 {
        let (len, off) = mf.longest_match(&data, pos, MAX_MATCH_LEN, MAX_MATCH_LEN);
        let expected_max = (data.len() - pos).min(MAX_MATCH_LEN);
        assert!(len >= 4, "pos {pos} expected match >= 4, got {len}");
        assert_eq!(len, expected_max);
        assert_eq!(off, 1, "pos {pos} expected offset 1, got {off}");
    }
}

#[test]
fn test_ht_matchfinder_periodic_string() {
    let mut mf = HtMatchfinder::new();
    let pattern = b"ABCDEFGHIJKL";
    let data = generate_repeated_pattern(pattern, 600);

    // Scan through data
    for pos in 0..pattern.len() {
        mf.longest_match(&data, pos, MAX_MATCH_LEN, MAX_MATCH_LEN);
    }

    // At pos = 12 (second period start), it should match previous period
    let (len, off) = mf.longest_match(&data, 12, MAX_MATCH_LEN, MAX_MATCH_LEN);
    assert_eq!(off, 12);
    let expected_len = (data.len() - 12).min(MAX_MATCH_LEN);
    assert_eq!(len, expected_len);
}

#[test]
fn test_ht_matchfinder_dual_slot_history() {
    let mut mf = HtMatchfinder::new();
    // Construct three distinct occurrences with the same 4-byte prefix but different tails
    let mut data = Vec::new();
    data.extend_from_slice(b"TEST_OCCURRENCE_ONE_ALPHA_12345"); // 0
    data.extend_from_slice(b"ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ"); // 31
    data.extend_from_slice(b"TEST_OCCURRENCE_TWO_BETA__12345"); // 62
    data.extend_from_slice(b"YYYYYYYYYYYYYYYYYYYYYYYYYYYYYYY"); // 93
    let cur_pos = data.len();
    data.extend_from_slice(b"TEST_OCCURRENCE_TWO_BETA__12345"); // 124

    // Register occurrence 1 and occurrence 2
    mf.longest_match(&data, 0, MAX_MATCH_LEN, MAX_MATCH_LEN);
    mf.longest_match(&data, 62, MAX_MATCH_LEN, MAX_MATCH_LEN);

    // Match at cur_pos: should match occurrence 2 (pos 62, offset 62) perfectly
    let (len, off) = mf.longest_match(&data, cur_pos, MAX_MATCH_LEN, MAX_MATCH_LEN);
    assert_eq!(off, cur_pos - 62);
    assert_eq!(len, b"TEST_OCCURRENCE_TWO_BETA__12345".len());
}

#[test]
fn test_ht_matchfinder_skip_bytes() {
    let mut mf1 = HtMatchfinder::new();
    let mut mf2 = HtMatchfinder::new();
    let data = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

    // mf1 advances via skip_bytes
    mf1.skip_bytes(data, 0, 10);

    // mf2 advances via individual longest_match
    for i in 0..10 {
        mf2.longest_match(data, i, MAX_MATCH_LEN, MAX_MATCH_LEN);
    }

    // Verify hash tables match for relevant entries
    for i in 0..10 {
        let seq = u32::from_le_bytes(data[i..i + 4].try_into().unwrap());
        let hash = lz_hash(seq, HT_HASH_ORDER as u32);
        assert_eq!(mf1.hash_tab[hash][0], mf2.hash_tab[hash][0]);
    }
}

// MARK: - 4. HcMatchfinder Tests

#[test]
fn test_hc_matchfinder_init_and_reset() {
    let mut mf = HcMatchfinder::new();
    for v in mf.hash3_tab.iter() {
        assert_eq!(*v, MATCHFINDER_INITVAL);
    }
    for v in mf.hash4_tab.iter() {
        assert_eq!(*v, MATCHFINDER_INITVAL);
    }
    for v in mf.next_tab.iter() {
        assert_eq!(*v, MATCHFINDER_INITVAL);
    }

    mf.hash3_tab[10] = 5;
    mf.hash4_tab[20] = 6;
    mf.next_tab[30] = 7;
    mf.reset();
    assert_eq!(mf.hash3_tab[10], MATCHFINDER_INITVAL);
    assert_eq!(mf.hash4_tab[20], MATCHFINDER_INITVAL);
    assert_eq!(mf.next_tab[30], MATCHFINDER_INITVAL);
}

#[test]
fn test_hc_matchfinder_length3_match() {
    let mut mf = HcMatchfinder::new();
    // Data has two identical 3-byte sequences "XYZ" followed by distinct 4th byte
    let data = b"XYZ1_padding_middle_bytes_XYZ2_tail";
    let (len0, off0) = mf.longest_match(data, 0, MAX_MATCH_LEN, MAX_MATCH_LEN, 16);
    assert_eq!(len0, 0);
    assert_eq!(off0, 0);

    // Position of second "XYZ2" is 26
    let pos2 = 26;
    let (len, off) = mf.longest_match(data, pos2, MAX_MATCH_LEN, MAX_MATCH_LEN, 16);
    assert_eq!(len, 3, "expected length-3 match for XYZ");
    assert_eq!(off, 26);
}

#[test]
fn test_hc_matchfinder_depth_truncation() {
    let mut mf = HcMatchfinder::new();
    // Construct multiple occurrences of same 4-byte prefix
    let mut data = Vec::new();
    data.extend_from_slice(b"KEY_LONGEST_MATCH_TARGET_12345"); // 0
    data.extend_from_slice(b"KEY_SHORT_1"); // 30
    data.extend_from_slice(b"KEY_SHORT_2"); // 41
    data.extend_from_slice(b"KEY_SHORT_3"); // 52
    let cur_pos = data.len();
    data.extend_from_slice(b"KEY_LONGEST_MATCH_TARGET_12345");

    mf.longest_match(&data, 0, MAX_MATCH_LEN, MAX_MATCH_LEN, 32);
    mf.longest_match(&data, 30, MAX_MATCH_LEN, MAX_MATCH_LEN, 32);
    mf.longest_match(&data, 41, MAX_MATCH_LEN, MAX_MATCH_LEN, 32);
    mf.longest_match(&data, 52, MAX_MATCH_LEN, MAX_MATCH_LEN, 32);

    // With depth = 1, it only sees the most recent occurrence (pos 52, prefix "KEY_")
    let mut mf_depth1 = mf.clone();
    let (len1, off1) = mf_depth1.longest_match(&data, cur_pos, MAX_MATCH_LEN, MAX_MATCH_LEN, 1);
    assert_eq!(off1, cur_pos - 52);
    assert_eq!(len1, b"KEY_".len());

    // With depth = 10, it traverses back to pos 0 and finds full longest match
    let mut mf_depth10 = mf.clone();
    let (len10, off10) = mf_depth10.longest_match(&data, cur_pos, MAX_MATCH_LEN, MAX_MATCH_LEN, 10);
    assert_eq!(off10, cur_pos);
    assert_eq!(len10, b"KEY_LONGEST_MATCH_TARGET_12345".len());
}

#[test]
fn test_hc_matchfinder_all_repeat_string() {
    let mut mf = HcMatchfinder::new();
    let data = vec![b'B'; 1000];

    // Seed pos 0
    let (len0, off0) = mf.longest_match(&data, 0, MAX_MATCH_LEN, MAX_MATCH_LEN, 32);
    assert_eq!(len0, 0);
    assert_eq!(off0, 0);

    for pos in 1..100 {
        let (len, off) = mf.longest_match(&data, pos, MAX_MATCH_LEN, MAX_MATCH_LEN, 32);
        let expected_max = (data.len() - pos).min(MAX_MATCH_LEN);
        assert!(len >= 3);
        assert_eq!(len, expected_max);
        assert_eq!(off, 1);
    }
}

// MARK: - 5. Window Sliding & Rebase Tests

#[test]
fn test_ht_matchfinder_rebase_continuity() {
    let mut mf = HtMatchfinder::new();
    let total_size = 35000;
    let mut data = generate_pseudo_random(total_size, 42);

    // Embed known recurring pattern at pos 32000 and pos 33000 (across 32KB boundary)
    let pattern = b"REBASE_MATCH_ACROSS_32K_WINDOW";
    data[32000..32000 + pattern.len()].copy_from_slice(pattern);
    data[33000..33000 + pattern.len()].copy_from_slice(pattern);

    // Explicitly register pos 32000
    mf.longest_match(&data, 32000, MAX_MATCH_LEN, MAX_MATCH_LEN);

    // Window boundary reached: rebase matchfinder
    mf.rebase();

    // Match at pos 33000 (in new window slice)
    let (len, off) = mf.longest_match(&data, 33000, MAX_MATCH_LEN, MAX_MATCH_LEN);
    assert_eq!(off, 1000, "offset must correctly span rebased window");
    assert_eq!(len, pattern.len());
}

#[test]
fn test_hc_matchfinder_rebase_continuity() {
    let mut mf = HcMatchfinder::new();
    let total_size = 35000;
    let mut data = generate_pseudo_random(total_size, 100);

    let pattern = b"HC_REBASE_CONTINUITY_VERIFICATION";
    data[31500..31500 + pattern.len()].copy_from_slice(pattern);
    data[33500..33500 + pattern.len()].copy_from_slice(pattern);

    // Explicitly register pos 31500
    mf.longest_match(&data, 31500, MAX_MATCH_LEN, MAX_MATCH_LEN, 16);

    // Rebase
    mf.rebase();

    // Match at pos 33500
    let (len, off) = mf.longest_match(&data, 33500, MAX_MATCH_LEN, MAX_MATCH_LEN, 16);
    assert_eq!(off, 2000);
    assert_eq!(len, pattern.len());
}

// MARK: - 6. Boundary & Defensive Edge Case Tests

#[test]
fn test_defensive_bounds_and_short_inputs() {
    let mut ht = HtMatchfinder::new();
    let mut hc = HcMatchfinder::new();
    let data = b"abc";

    // Inputs shorter than 4 bytes
    assert_eq!(ht.longest_match(data, 0, MAX_MATCH_LEN, MAX_MATCH_LEN), (0, 0));
    assert_eq!(hc.longest_match(data, 0, MAX_MATCH_LEN, MAX_MATCH_LEN, 16), (0, 0));

    // Position at or beyond buffer end
    assert_eq!(ht.longest_match(data, 3, MAX_MATCH_LEN, MAX_MATCH_LEN), (0, 0));
    assert_eq!(hc.longest_match(data, 3, MAX_MATCH_LEN, MAX_MATCH_LEN, 16), (0, 0));
    assert_eq!(ht.longest_match(data, 10, MAX_MATCH_LEN, MAX_MATCH_LEN), (0, 0));
    assert_eq!(hc.longest_match(data, 10, MAX_MATCH_LEN, MAX_MATCH_LEN, 16), (0, 0));

    // max_len == 0
    let long_data = b"01234567890123456789";
    assert_eq!(ht.longest_match(long_data, 10, 0, 0), (0, 0));
    assert_eq!(hc.longest_match(long_data, 10, 0, 0, 16), (0, 0));
}
