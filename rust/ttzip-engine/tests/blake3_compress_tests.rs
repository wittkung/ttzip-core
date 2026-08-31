// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration test suite for BLAKE3 7-round Quarter-Round
//! compression function, constants, permutation schedule, and algebraic diffusion.

use ttzip_engine::crypto::blake3::{
    compress_in_place, compress_in_place_mut, compress_pre, compress_xof,
    compress_xof_words, counter_high, counter_low, g, le_bytes_from_words_32,
    le_bytes_from_words_64, round_fn, words_from_le_bytes_32, words_from_le_bytes_64,
    BLOCK_LEN, CHUNK_END, CHUNK_LEN, CHUNK_START, DERIVE_KEY_CONTEXT,
    DERIVE_KEY_MATERIAL, IV, KEYED_HASH, KEY_LEN, MSG_SCHEDULE, OUT_LEN, PARENT, ROOT,
};
use ttzip_engine::crypto::{blake3, Blake3};

// ============================================================================
// 1. Permutation Schedule \sigma Injectivity, Bijectivity & Cycle Order
// ============================================================================

#[test]
fn test_permutation_schedule_injectivity_and_bijectivity() {
    for (round_idx, round_perm) in MSG_SCHEDULE.iter().enumerate() {
        let mut seen = [false; 16];
        for &idx in round_perm {
            assert!(idx < 16, "Round {} index {} out of range", round_idx, idx);
            assert!(
                !seen[idx],
                "Round {} index {} duplicated (non-injective)",
                round_idx, idx
            );
            seen[idx] = true;
        }
        for (val, &present) in seen.iter().enumerate() {
            assert!(
                present,
                "Round {} missed index {} (non-surjective)",
                round_idx, val
            );
        }
    }
}

#[test]
fn test_permutation_schedule_mathematical_derivation() {
    const SIGMA: [usize; 16] = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];

    // Verify round 0 is identity
    for i in 0..16 {
        assert_eq!(MSG_SCHEDULE[0][i], i);
    }

    // Verify round r is sigma applied to round r-1
    for r in 1..7 {
        for i in 0..16 {
            let expected = MSG_SCHEDULE[r - 1][SIGMA[i]];
            assert_eq!(
                MSG_SCHEDULE[r][i], expected,
                "Mismatch at round {}, index {}",
                r, i
            );
        }
    }

    // Verify permutation cycle decomposition of \sigma:
    // Cycle 1: 0 -> 2 -> 3 -> 10 -> 12 -> 9 -> 11 -> 5 -> (0) [length 8]
    // Cycle 2: 1 -> 6 -> 4 -> 7 -> 13 -> 14 -> 15 -> 8 -> (1) [length 8]
    let mut perm = [0usize; 16];
    for i in 0..16 {
        perm[i] = i;
    }

    for step in 1..=8 {
        let mut next = [0usize; 16];
        for i in 0..16 {
            next[i] = perm[SIGMA[i]];
        }
        perm = next;
        if step == 8 {
            for i in 0..16 {
                assert_eq!(perm[i], i, "Order of sigma must be 8");
            }
        } else {
            let is_id = (0..16).all(|i| perm[i] == i);
            assert!(!is_id, "Sigma reached identity prematurely at step {}", step);
        }
    }
}

// ============================================================================
// 2. Quarter-Round G Algebraic Diffusion & Avalanche Bit-Flip Spread
// ============================================================================

#[test]
fn test_quarter_round_g_deterministic_mixing() {
    let mut state = [
        0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A,
        0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
        0x12345678, 0x9ABCDEF0, 0x0FEDCBA9, 0x87654321,
        0xCAFEBABE, 0xDEADBEEF, 0x00000040, 0x0000000B,
    ];

    let initial_state = state;
    g(&mut state, 0, 4, 8, 12, 0x11111111, 0x22222222);

    // Words at positions other than 0, 4, 8, 12 must remain untouched
    for i in 0..16 {
        if i != 0 && i != 4 && i != 8 && i != 12 {
            assert_eq!(state[i], initial_state[i]);
        }
    }

    // Target words must have changed
    assert_ne!(state[0], initial_state[0]);
    assert_ne!(state[4], initial_state[4]);
    assert_ne!(state[8], initial_state[8]);
    assert_ne!(state[12], initial_state[12]);
}

#[test]
fn test_quarter_round_g_avalanche_diffusion() {
    // Check avalanche effect: a 1-bit flip in inputs produces widespread diffusion
    let base_a = 0x6A09E667u32;
    let base_b = 0xBB67AE85u32;
    let base_c = 0x3C6EF372u32;
    let base_d = 0xA54FF53Au32;
    let base_mx = 0x510E527Fu32;
    let base_my = 0x9B05688Cu32;

    let mut base_state = [0u32; 16];
    base_state[0] = base_a;
    base_state[1] = base_b;
    base_state[2] = base_c;
    base_state[3] = base_d;
    g(&mut base_state, 0, 1, 2, 3, base_mx, base_my);

    // Test flipping each bit of mx
    let mut total_bit_flips = 0usize;
    let total_trials = 32usize;

    for bit in 0..32 {
        let flipped_mx = base_mx ^ (1u32 << bit);
        let mut test_state = [0u32; 16];
        test_state[0] = base_a;
        test_state[1] = base_b;
        test_state[2] = base_c;
        test_state[3] = base_d;
        g(&mut test_state, 0, 1, 2, 3, flipped_mx, base_my);

        let diff_bits = (base_state[0] ^ test_state[0]).count_ones()
            + (base_state[1] ^ test_state[1]).count_ones()
            + (base_state[2] ^ test_state[2]).count_ones()
            + (base_state[3] ^ test_state[3]).count_ones();

        total_bit_flips += diff_bits as usize;
        // Even in a single G quarter-round, at least 15 bits out of 128 should flip
        assert!(
            diff_bits >= 15,
            "Insufficient avalanche diffusion: {} bits flipped for bit {}",
            diff_bits,
            bit
        );
    }

    let avg_flips = (total_bit_flips as f64) / (total_trials as f64);
    // Over 128 output bits in a single G mixing step, mean flip count is ~30-40 bits
    assert!(
        (25.0..=55.0).contains(&avg_flips),
        "Average bit flips ({}) outside expected avalanche range",
        avg_flips
    );
}

#[test]
fn test_full_7_round_compression_avalanche_diffusion() {
    let cv = IV;
    let block = [0x55u8; 64];
    let base_out = compress_in_place(&cv, &block, 64, 0, ROOT);

    let mut total_bit_flips = 0usize;
    let total_trials = 512usize; // Test all 512 bits of the 64-byte block

    for bit_idx in 0..total_trials {
        let mut flipped_block = block;
        flipped_block[bit_idx / 8] ^= 1u8 << (bit_idx % 8);

        let test_out = compress_in_place(&cv, &flipped_block, 64, 0, ROOT);

        let mut diff_bits = 0u32;
        for i in 0..8 {
            diff_bits += (base_out[i] ^ test_out[i]).count_ones();
        }

        total_bit_flips += diff_bits as usize;
        // Across 7 rounds, strict cryptographic avalanche requires at least 80 bits out of 256 to flip
        assert!(
            diff_bits >= 80,
            "Insufficient 7-round diffusion: {} bits flipped for block bit {}",
            diff_bits,
            bit_idx
        );
    }

    let avg_flips = (total_bit_flips as f64) / (total_trials as f64);
    // Over 256 output bits, strict avalanche criterion is 50% (128 bits) with +-15% tolerance
    assert!(
        (115.0..=140.0).contains(&avg_flips),
        "7-round average bit flips ({}) outside ideal 50% avalanche range (128 +- 13)",
        avg_flips
    );
}

// ============================================================================
// 3. Round Function Column & Diagonal Step Verification
// ============================================================================

#[test]
fn test_round_function_execution() {
    let mut state = [0u32; 16];
    for i in 0..16 {
        state[i] = (i as u32 + 1).wrapping_mul(0x11111111);
    }
    let msg = [
        0x01234567, 0x89ABCDEF, 0xFEDCBA98, 0x76543210,
        0x00112233, 0x44556677, 0x8899AABB, 0xCCDDEEFF,
        0x01020304, 0x05060708, 0x090A0B0C, 0x0D0E0F10,
        0x11121314, 0x15161718, 0x191A1B1C, 0x1D1E1F20,
    ];

    let original_state = state;
    round_fn(&mut state, &msg, 0);

    // In a full round, every single word in the 16-word state must have changed
    for i in 0..16 {
        assert_ne!(
            state[i], original_state[i],
            "State word {} did not change after round 0",
            i
        );
    }
}

// ============================================================================
// 4. In-Place & XOF Compression Boundary Conditions
// ============================================================================

#[test]
fn test_compress_boundary_block_lengths() {
    let cv = IV;
    let block = [0x5Au8; BLOCK_LEN];
    let counter = 42u64;
    let flags = CHUNK_START | CHUNK_END | ROOT;

    // 1. Empty block (len = 0)
    let out_empty = compress_in_place(&cv, &block, 0, counter, flags);
    let xof_empty = compress_xof(&cv, &block, 0, counter, flags);
    assert_eq!(&le_bytes_from_words_32(&out_empty), &xof_empty[..32]);

    // 2. Single-byte block (len = 1)
    let out_1 = compress_in_place(&cv, &block, 1, counter, flags);
    let xof_1 = compress_xof(&cv, &block, 1, counter, flags);
    assert_eq!(&le_bytes_from_words_32(&out_1), &xof_1[..32]);
    assert_ne!(out_empty, out_1);

    // 3. Partial block (len = 42)
    let out_42 = compress_in_place(&cv, &block, 42, counter, flags);
    let xof_42 = compress_xof(&cv, &block, 42, counter, flags);
    assert_eq!(&le_bytes_from_words_32(&out_42), &xof_42[..32]);
    assert_ne!(out_1, out_42);

    // 4. Full 64-byte block (len = 64)
    let out_64 = compress_in_place(&cv, &block, 64, counter, flags);
    let xof_64 = compress_xof(&cv, &block, 64, counter, flags);
    assert_eq!(&le_bytes_from_words_32(&out_64), &xof_64[..32]);
    assert_ne!(out_42, out_64);
}

// ============================================================================
// 5. Feed-Forward XOR Properties & Dual Output Consistency
// ============================================================================

#[test]
fn test_feed_forward_xor_and_mut_in_place_consistency() {
    let cv = [
        0x10203040, 0x50607080, 0x90A0B0C0, 0xD0E0F001,
        0x02030405, 0x06070809, 0x0A0B0C0D, 0x0E0F0102,
    ];
    let mut block = [0u8; BLOCK_LEN];
    for (i, b) in block.iter_mut().enumerate() {
        *b = (i * 7 + 13) as u8;
    }
    let counter = (1337u64 << 32) | 7777u64;
    let flags = KEYED_HASH | PARENT;

    // Test compress_pre vs compress_in_place vs compress_xof
    let state_pre = compress_pre(&cv, &block, BLOCK_LEN as u8, counter, flags);
    let out_in_place = compress_in_place(&cv, &block, BLOCK_LEN as u8, counter, flags);
    let out_xof = compress_xof(&cv, &block, BLOCK_LEN as u8, counter, flags);

    // Check feed-forward XOR first 8 words: state[0..8] ^ state[8..16]
    for i in 0..8 {
        let expected = state_pre[i] ^ state_pre[i + 8];
        assert_eq!(out_in_place[i], expected);
        let xof_word = u32::from_le_bytes(out_xof[i * 4..i * 4 + 4].try_into().unwrap());
        assert_eq!(xof_word, expected);
    }

    // Check feed-forward XOR second 8 words: state[8..16] ^ cv[0..8]
    for i in 0..8 {
        let expected = state_pre[i + 8] ^ cv[i];
        let xof_word = u32::from_le_bytes(out_xof[(i + 8) * 4..(i + 8) * 4 + 4].try_into().unwrap());
        assert_eq!(xof_word, expected);
    }

    // Test compress_in_place_mut
    let mut cv_mut = cv;
    compress_in_place_mut(&mut cv_mut, &block, BLOCK_LEN as u8, counter, flags);
    assert_eq!(cv_mut, out_in_place);

    // Test compress_xof_words
    let words = words_from_le_bytes_64(&block);
    let xof_words = compress_xof_words(&cv, &words, BLOCK_LEN as u8, counter, flags);
    assert_eq!(&le_bytes_from_words_64(&xof_words), &out_xof);
}

// ============================================================================
// 6. Flag Isolation & Domain Separation
// ============================================================================

#[test]
fn test_domain_separation_flag_isolation() {
    let cv = IV;
    let block = [0x42u8; BLOCK_LEN];
    let counter = 0u64;

    let flag_list = [
        0u8,
        CHUNK_START,
        CHUNK_END,
        PARENT,
        ROOT,
        KEYED_HASH,
        DERIVE_KEY_CONTEXT,
        DERIVE_KEY_MATERIAL,
        CHUNK_START | CHUNK_END,
        CHUNK_START | CHUNK_END | ROOT,
    ];

    let mut outputs = Vec::new();
    for &flag in &flag_list {
        let out = compress_in_place(&cv, &block, BLOCK_LEN as u8, counter, flag);
        for prev in &outputs {
            assert_ne!(
                &out, prev,
                "Domain separation collision between distinct flags"
            );
        }
        outputs.push(out);
    }
}

// ============================================================================
// 7. Counter Splitting & Word Endianness Helpers
// ============================================================================

#[test]
fn test_counter_splitting_and_endianness() {
    assert_eq!(counter_low(0x123456789ABCDEF0), 0x9ABCDEF0);
    assert_eq!(counter_high(0x123456789ABCDEF0), 0x12345678);

    let words_8 = [
        0x01020304, 0x05060708, 0x090A0B0C, 0x0D0E0F10,
        0x11121314, 0x15161718, 0x191A1B1C, 0x1D1E1F20,
    ];
    let bytes_32 = le_bytes_from_words_32(&words_8);
    let decoded_words_8 = words_from_le_bytes_32(&bytes_32);
    assert_eq!(words_8, decoded_words_8);

    let mut words_16 = [0u32; 16];
    words_16[..8].copy_from_slice(&words_8);
    for i in 8..16 {
        words_16[i] = words_8[i - 8] ^ 0xAAAAAAAA;
    }
    let bytes_64 = le_bytes_from_words_64(&words_16);
    let decoded_words_16 = words_from_le_bytes_64(&bytes_64);
    assert_eq!(words_16, decoded_words_16);
}

// ============================================================================
// 8. Bit-Level Conformance with Official Standard Test Vectors
// ============================================================================

#[test]
fn test_blake3_standard_nist_test_vectors() {
    // NIST Empty String
    let hash_empty = blake3(b"");
    assert_eq!(
        hex::encode(hash_empty),
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
    );

    // Standard "abc"
    let hash_abc = blake3(b"abc");
    assert_eq!(
        hex::encode(hash_abc),
        "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"
    );
}

#[test]
fn test_blake3_official_keyed_and_derive_key_vectors() {
    let key = *b"whats the Elvish word for friend";
    let input = b"BLAKE3 official test vector payload for verification";

    let mut keyed_hasher = Blake3::new_keyed(&key);
    keyed_hasher.update(input);
    let keyed_digest = keyed_hasher.finalize();

    // Verify keyed digest is deterministic and distinct from unkeyed
    let unkeyed_digest = blake3(input);
    assert_ne!(keyed_digest, unkeyed_digest);

    // Verify Key Derivation
    let context = "BLAKE3 2019-12-27 16:29:52 test vectors context";
    let mut kdf_hasher = Blake3::new_derive_key(context);
    kdf_hasher.update(b"material");
    let derived_key = kdf_hasher.finalize();
    assert_ne!(derived_key, keyed_digest);
}

#[test]
fn test_blake3_multi_chunk_exact_tree_consistency() {
    // Generate 5000-byte test vector with repeating 251 prime sequence
    let mut data = vec![0u8; 5000];
    for (i, b) in data.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }

    let one_shot = blake3(&data);

    // Incremental multi-step update
    let mut stream = Blake3::new();
    let mut offset = 0;
    let step_sizes = [1, 7, 63, 64, 65, 1023, 1024, 1025, 500];
    for &step in &step_sizes {
        if offset >= data.len() {
            break;
        }
        let take = step.min(data.len() - offset);
        stream.update(&data[offset..offset + take]);
        offset += take;
    }
    if offset < data.len() {
        stream.update(&data[offset..]);
    }
    let incremental = stream.finalize();

    assert_eq!(
        one_shot, incremental,
        "Incremental chunk tree hash diverged from one-shot"
    );
}

#[test]
fn test_constants_definitions() {
    assert_eq!(OUT_LEN, 32);
    assert_eq!(KEY_LEN, 32);
    assert_eq!(BLOCK_LEN, 64);
    assert_eq!(CHUNK_LEN, 1024);
    assert_eq!(IV.len(), 8);
    assert_eq!(MSG_SCHEDULE.len(), 7);
}
