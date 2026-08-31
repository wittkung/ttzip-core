// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive Verification Test Suite for LZMA2 64-bit Single-Branch Range Encoder.
//!
//! Validates:
//! 1. Adaptive bit encoding/decoding roundtrip with probability model tracking.
//! 2. High-speed Direct Bits encoding/decoding with branchless register shifts.
//! 3. Binary tree, reverse binary tree, literal, and matched literal byte codecs.
//! 4. Constant-time carry resolution and consecutive 0xFF byte rollover cascades.
//! 5. 1,000,000 random bit stream roundtrip verifying absolute bit fidelity.
//! 6. Compile-time 2048-entry bit price table (`PROB_PRICES`) monotonicity, symmetry, and math specs.
//! 7. State machine lifecycle resets, sink buffer adapters, and memory boundaries.

use ttzip_engine::codecs::lzma::{
    RangeDecoder, BIT_MODEL_TOTAL, NUM_BIT_MODEL_TOTAL_BITS, NUM_MOVE_BITS, PROB_INIT_VAL,
    TOP_VALUE,
};
use ttzip_engine::codecs::lzma2::{
    get_bit_tree_price, get_direct_bits_price, get_price, get_price_0, get_price_1,
    get_reverse_bit_tree_price, Lzma2RangeEncoder, BIT_PRICE_UNIT, NUM_BIT_PRICE_SHIFT_BITS,
    PROB_PRICES, PROB_TABLE_SIZE,
};

#[test]
fn test_lzma2_range_encoder_constants_and_initialization() {
    assert_eq!(NUM_BIT_MODEL_TOTAL_BITS, 11);
    assert_eq!(BIT_MODEL_TOTAL, 2048);
    assert_eq!(NUM_MOVE_BITS, 5);
    assert_eq!(TOP_VALUE, 0x0100_0000);
    assert_eq!(PROB_INIT_VAL, 1024);

    assert_eq!(NUM_BIT_PRICE_SHIFT_BITS, 4);
    assert_eq!(BIT_PRICE_UNIT, 16);
    assert_eq!(PROB_TABLE_SIZE, 2048);
    assert_eq!(PROB_PRICES.len(), 2048);

    let enc = Lzma2RangeEncoder::new();
    assert_eq!(enc.range(), 0xFFFF_FFFF);
    assert_eq!(enc.low(), 0);
    assert_eq!(enc.cache(), 0);
    assert_eq!(enc.cache_size(), 1);
    assert!(enc.buffer().is_empty());
    assert_eq!(enc.processed_size(), 1);

    let mut enc_cap = Lzma2RangeEncoder::with_capacity(1024);
    assert_eq!(enc_cap.range(), 0xFFFF_FFFF);
    assert_eq!(enc_cap.low(), 0);

    let mut prob = PROB_INIT_VAL;
    enc_cap.encode_bit(&mut prob, 1);
    assert!(enc_cap.low() > 0);

    enc_cap.reset();
    assert_eq!(enc_cap.range(), 0xFFFF_FFFF);
    assert_eq!(enc_cap.low(), 0);
    assert_eq!(enc_cap.cache(), 0);
    assert_eq!(enc_cap.cache_size(), 1);
    assert!(enc_cap.buffer().is_empty());
}

#[test]
fn test_lzma2_range_encoder_adaptive_bit_roundtrip() {
    let test_bits: Vec<(usize, u32)> = vec![
        (0, 0),
        (0, 1),
        (1, 1),
        (1, 1),
        (2, 0),
        (2, 0),
        (0, 0),
        (3, 1),
        (3, 0),
        (1, 0),
        (2, 1),
        (4, 0),
        (4, 1),
        (5, 1),
        (6, 0),
        (7, 1),
        (8, 0),
        (9, 1),
        (10, 0),
        (11, 1),
        (12, 0),
        (13, 1),
        (14, 0),
        (15, 1),
    ];

    let mut encoder = Lzma2RangeEncoder::new();
    let mut enc_probs = [PROB_INIT_VAL; 16];
    let mut expected_prob_snapshots = Vec::new();

    for &(ctx, bit) in &test_bits {
        encoder.encode_bit(&mut enc_probs[ctx], bit);
        expected_prob_snapshots.push(enc_probs[ctx]);
    }
    let stream = encoder.into_vec();
    assert!(stream.len() >= 5);

    let mut decoder = RangeDecoder::new(&stream).expect("range decoder init");
    let mut dec_probs = [PROB_INIT_VAL; 16];

    for (idx, &(ctx, expected_bit)) in test_bits.iter().enumerate() {
        let decoded_bit = decoder
            .decode_bit(&mut dec_probs[ctx])
            .unwrap_or_else(|_| panic!("failed decoding bit at index {idx}"));
        assert_eq!(
            decoded_bit, expected_bit,
            "Bit mismatch at step {idx} (expected {expected_bit}, got {decoded_bit})"
        );
        assert_eq!(
            dec_probs[ctx], expected_prob_snapshots[idx],
            "Probability mismatch at step {idx}"
        );
    }
    assert_eq!(enc_probs, dec_probs);
}

#[test]
fn test_lzma2_range_encoder_direct_bits_roundtrip() {
    let test_values: Vec<(u32, u32)> = vec![
        (0, 1),
        (1, 1),
        (0b10, 2),
        (0b01, 2),
        (0b11, 2),
        (0b00, 2),
        (0x55, 8),
        (0xAA, 8),
        (0x00, 8),
        (0xFF, 8),
        (0x1234, 16),
        (0xDEAD_BEEF, 32),
        (0x0000_0000, 32),
        (0xFFFF_FFFF, 32),
        (0x5555_5555, 32),
        (0xAAAA_AAAA, 32),
        (12345, 14),
        (7, 3),
        (63, 6),
    ];

    let mut encoder = Lzma2RangeEncoder::new();
    for &(val, num_bits) in &test_values {
        encoder.encode_direct_bits(val, num_bits);
    }
    let stream = encoder.into_vec();

    let mut decoder = RangeDecoder::new(&stream).expect("decoder init");
    for (idx, &(expected_val, num_bits)) in test_values.iter().enumerate() {
        let decoded = decoder
            .decode_direct_bits(num_bits)
            .unwrap_or_else(|_| panic!("failed decoding direct bits at index {idx}"));
        assert_eq!(
            decoded, expected_val,
            "Direct bit mismatch at index {idx}, bits: {num_bits}"
        );
    }
}

#[test]
fn test_lzma2_range_encoder_tree_and_literal_roundtrip() {
    let mut encoder = Lzma2RangeEncoder::new();
    let mut tree_probs = [PROB_INIT_VAL; 64];
    let mut rev_tree_probs = [PROB_INIT_VAL; 64];
    let mut lit_probs = [PROB_INIT_VAL; 0x300];

    // Encode bit trees
    for sym in 0..8u32 {
        encoder.encode_bit_tree(&mut tree_probs, sym, 3);
        encoder.encode_reverse_bit_tree(&mut rev_tree_probs, sym, 3);
    }

    // Encode literals and matched bytes
    let test_bytes = [b'T', b'T', b'Z', b'i', b'p', 0x00, 0xFF, 0x55, 0xAA];
    let match_bytes = [b'T', b'A', b'Z', b'o', b'p', 0x01, 0xFE, 0x55, 0x55];

    for (&byte, &match_byte) in test_bytes.iter().zip(match_bytes.iter()) {
        encoder.encode_literal_byte(&mut lit_probs, byte);
        encoder.encode_matched_byte(&mut lit_probs, byte, match_byte);
    }

    let stream = encoder.into_vec();
    let mut decoder = RangeDecoder::new(&stream).expect("decoder init");

    let mut dec_tree_probs = [PROB_INIT_VAL; 64];
    let mut dec_rev_tree_probs = [PROB_INIT_VAL; 64];
    let mut dec_lit_probs = [PROB_INIT_VAL; 0x300];

    for sym in 0..8u32 {
        let dec_sym = decoder.decode_bit_tree(&mut dec_tree_probs, 3).unwrap();
        assert_eq!(dec_sym, sym, "Bit tree symbol mismatch");

        let dec_rev_sym = decoder
            .decode_reverse_bit_tree(&mut dec_rev_tree_probs, 3)
            .unwrap();
        assert_eq!(dec_rev_sym, sym, "Reverse bit tree symbol mismatch");
    }

    for (&expected_byte, &match_byte) in test_bytes.iter().zip(match_bytes.iter()) {
        let dec_lit = decoder.decode_literal_byte(&mut dec_lit_probs).unwrap();
        assert_eq!(dec_lit, expected_byte, "Literal byte mismatch");

        let dec_matched = decoder
            .decode_matched_byte(&mut dec_lit_probs, match_byte)
            .unwrap();
        assert_eq!(dec_matched, expected_byte, "Matched byte mismatch");
    }
}

#[test]
fn test_lzma2_carry_cascade_consecutive_0xff_rollover() {
    // Test 1: Carry cascade roundtrip with adversarial bits inducing multiple carries into sink
    let mut enc_carry = Lzma2RangeEncoder::new();
    let mut enc_sink: Vec<u8> = Vec::new();

    let mut prob1 = 2040u16;
    let mut prob2 = 8u16;

    for _ in 0..100 {
        enc_carry.encode_bit_with_sink(&mut prob1, 1, &mut enc_sink);
        enc_carry.encode_bit_with_sink(&mut prob2, 0, &mut enc_sink);
        enc_carry.encode_direct_bits_with_sink(0xFFFF, 16, &mut enc_sink);
    }
    enc_carry.flush(&mut enc_sink);

    assert!(enc_sink.len() >= 5);
    let mut dec = RangeDecoder::new(&enc_sink).expect("decoder init on carry stream");
    let mut dec_prob1 = 2040u16;
    let mut dec_prob2 = 8u16;

    for _ in 0..100 {
        let b1 = dec.decode_bit(&mut dec_prob1).unwrap();
        assert_eq!(b1, 1);
        let b2 = dec.decode_bit(&mut dec_prob2).unwrap();
        assert_eq!(b2, 0);
        let direct = dec.decode_direct_bits(16).unwrap();
        assert_eq!(direct, 0xFFFF);
    }

    // Test 2: Internal buffer carry cascade roundtrip
    let mut enc_internal = Lzma2RangeEncoder::new();
    let mut prob_a = 2000u16;
    let mut prob_b = 48u16;

    for _ in 0..200 {
        enc_internal.encode_bit(&mut prob_a, 1);
        enc_internal.encode_bit(&mut prob_b, 0);
        enc_internal.encode_direct_bits(0xAAAA, 16);
    }
    let stream_internal = enc_internal.into_vec();

    let mut dec_internal = RangeDecoder::new(&stream_internal).expect("decoder init");
    let mut dec_prob_a = 2000u16;
    let mut dec_prob_b = 48u16;

    for _ in 0..200 {
        let ba = dec_internal.decode_bit(&mut dec_prob_a).unwrap();
        assert_eq!(ba, 1);
        let bb = dec_internal.decode_bit(&mut dec_prob_b).unwrap();
        assert_eq!(bb, 0);
        let direct = dec_internal.decode_direct_bits(16).unwrap();
        assert_eq!(direct, 0xAAAA);
    }
}

#[test]
fn test_lzma2_range_coder_one_million_random_bits_roundtrip() {
    // Deterministic 64-bit LCG pseudo-random generator
    let mut state: u64 = 0xA1B2_C3D4_E5F6_0718;
    let mut lcg = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 32) as u32
    };

    const TOTAL_BITS: usize = 1_000_000;
    const NUM_CONTEXTS: usize = 32;

    let mut generated_bits = Vec::with_capacity(TOTAL_BITS);
    let mut generated_contexts = Vec::with_capacity(TOTAL_BITS);

    for _ in 0..TOTAL_BITS {
        let ctx = (lcg() as usize) % NUM_CONTEXTS;
        let bit = lcg() & 1;
        generated_contexts.push(ctx);
        generated_bits.push(bit);
    }

    let mut encoder = Lzma2RangeEncoder::with_capacity(TOTAL_BITS / 8 + 64);
    let mut enc_probs = [PROB_INIT_VAL; NUM_CONTEXTS];

    for i in 0..TOTAL_BITS {
        let ctx = generated_contexts[i];
        let bit = generated_bits[i];
        encoder.encode_bit(&mut enc_probs[ctx], bit);
    }
    let stream = encoder.into_vec();

    assert!(stream.len() >= 5);
    let mut decoder = RangeDecoder::new(&stream).expect("decoder init for 1M bits");
    let mut dec_probs = [PROB_INIT_VAL; NUM_CONTEXTS];

    for i in 0..TOTAL_BITS {
        let ctx = generated_contexts[i];
        let expected_bit = generated_bits[i];
        let decoded_bit = decoder
            .decode_bit(&mut dec_probs[ctx])
            .unwrap_or_else(|_| panic!("failed at bit {i}"));
        assert_eq!(
            decoded_bit, expected_bit,
            "Mismatch at random bit index {i}"
        );
    }
    assert_eq!(enc_probs, dec_probs);
}

#[test]
fn test_lzma2_prob_prices_table_properties_and_math() {
    // 1. Table properties
    assert_eq!(PROB_PRICES.len(), 2048);

    // 2. Exact theoretical benchmarks (4-bit fractional units: 1 bit = 16 units)
    // 50% probability (prob = 1024) -> 1.0 bit = 16 units
    assert_eq!(
        PROB_PRICES[1024], 16,
        "PROB_PRICES[1024] must equal 16 (1.0 bit)"
    );
    // 25% probability (prob = 512) -> 2.0 bits = 32 units
    assert_eq!(
        PROB_PRICES[512], 32,
        "PROB_PRICES[512] must equal 32 (2.0 bits)"
    );
    // 12.5% probability (prob = 256) -> 3.0 bits = 48 units
    assert_eq!(
        PROB_PRICES[256], 48,
        "PROB_PRICES[256] must equal 48 (3.0 bits)"
    );
    // 6.25% probability (prob = 128) -> 4.0 bits = 64 units
    assert_eq!(
        PROB_PRICES[128], 64,
        "PROB_PRICES[128] must equal 64 (4.0 bits)"
    );
    // 3.125% probability (prob = 64) -> 5.0 bits = 80 units
    assert_eq!(
        PROB_PRICES[64], 80,
        "PROB_PRICES[64] must equal 80 (5.0 bits)"
    );
    // 1.5625% probability (prob = 32) -> 6.0 bits = 96 units
    assert_eq!(
        PROB_PRICES[32], 96,
        "PROB_PRICES[32] must equal 96 (6.0 bits)"
    );

    // 3. Monotonicity check: as probability increases, price strictly decreases or stays equal
    for i in 1..2047 {
        assert!(
            PROB_PRICES[i] >= PROB_PRICES[i + 1],
            "Monotonicity violated at i={i}: PROB_PRICES[{i}] = {}, PROB_PRICES[{}] = {}",
            PROB_PRICES[i],
            i + 1,
            PROB_PRICES[i + 1]
        );
    }

    // 4. Symmetry check: price of bit 0 at prob p == price of bit 1 at prob (2048 - p)
    for p in 1..2048u16 {
        let price_0 = get_price_0(p);
        let price_1 = get_price_1(2048 - p);
        assert_eq!(
            price_0, price_1,
            "Symmetry mismatch at prob {p}: get_price_0({p})={price_0}, get_price_1({})={price_1}",
            2048 - p
        );

        assert_eq!(get_price(p, 0), price_0);
        assert_eq!(get_price(p, 1), get_price_1(p));
    }

    // 5. Direct bits price calculation
    for num_bits in 0..32u32 {
        assert_eq!(get_direct_bits_price(num_bits), num_bits * 16);
    }

    // 6. Tree price consistency
    let tree_probs = [PROB_INIT_VAL; 16];
    for sym in 0..8u32 {
        let tree_p = get_bit_tree_price(&tree_probs, sym, 3);
        let rev_tree_p = get_reverse_bit_tree_price(&tree_probs, sym, 3);
        // With uniform 50% probabilities, 3 bits must cost exactly 3 * 16 = 48 units
        assert_eq!(tree_p, 48);
        assert_eq!(rev_tree_p, 48);
    }
}

#[test]
fn test_lzma2_range_encoder_explicit_sink_adapters() {
    let mut enc = Lzma2RangeEncoder::new();
    let mut sink: Vec<u8> = Vec::new();
    let mut prob = PROB_INIT_VAL;

    enc.encode_bit_with_sink(&mut prob, 0, &mut sink);
    enc.encode_bit_with_sink(&mut prob, 1, &mut sink);
    enc.encode_direct_bits_with_sink(0xABC, 12, &mut sink);
    enc.flush(&mut sink);

    assert!(sink.len() >= 5);
    let mut dec = RangeDecoder::new(&sink).expect("decoder init");
    let mut dec_prob = PROB_INIT_VAL;

    let b0 = dec.decode_bit(&mut dec_prob).unwrap();
    assert_eq!(b0, 0);
    let b1 = dec.decode_bit(&mut dec_prob).unwrap();
    assert_eq!(b1, 1);
    let direct = dec.decode_direct_bits(12).unwrap();
    assert_eq!(direct, 0xABC);
}
