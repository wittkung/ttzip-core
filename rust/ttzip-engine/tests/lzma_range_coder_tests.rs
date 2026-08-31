// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit and property test suite for LZMA / LZMA2 Range Coder and 12-State FSM.

use ttzip_engine::codecs::lzma::{
    LenCoderProbs, LiteralProperties, LzmaProbTable, LzmaState, RangeCoderError, RangeDecoder,
    RangeEncoder, State0, State1, State10, State11, State2, State3, State4, State5, State6,
    State7, State8, State9, ALIGN_TABLE_SIZE, BIT_MODEL_TOTAL, NUM_ALIGN_BITS,
    NUM_BIT_MODEL_TOTAL_BITS, NUM_LEN_TO_POS_STATES, NUM_MOVE_BITS, NUM_POS_DECODERS,
    NUM_POS_SLOTS, NUM_POS_STATES_MAX, NUM_STATES, PROB_INIT_VAL, TOP_VALUE,
};

#[test]
fn test_range_coder_constants() {
    assert_eq!(NUM_BIT_MODEL_TOTAL_BITS, 11);
    assert_eq!(BIT_MODEL_TOTAL, 2048);
    assert_eq!(NUM_MOVE_BITS, 5);
    assert_eq!(TOP_VALUE, 0x0100_0000);
    assert_eq!(PROB_INIT_VAL, 1024);
    assert_eq!(NUM_STATES, 12);
    assert_eq!(NUM_POS_STATES_MAX, 16);
    assert_eq!(NUM_LEN_TO_POS_STATES, 4);
    assert_eq!(NUM_POS_SLOTS, 64);
    assert_eq!(NUM_ALIGN_BITS, 4);
    assert_eq!(ALIGN_TABLE_SIZE, 16);
    assert_eq!(NUM_POS_DECODERS, 114);
}

#[test]
fn test_lzma_state_12_fsm_transitions() {
    // 1. Initial State
    let mut state = LzmaState::default();
    assert_eq!(state, State0);
    assert_eq!(state.as_usize(), 0);
    assert_eq!(state.as_u8(), 0);

    // 2. is_literal check for all 12 states
    for i in 0..12u8 {
        let st = LzmaState::from_u8(i).expect("valid state");
        if i < 7 {
            assert!(st.is_literal(), "State {i} must be literal");
        } else {
            assert!(!st.is_literal(), "State {i} must not be literal");
        }
    }
    assert_eq!(LzmaState::from_u8(12), None);
    assert_eq!(LzmaState::from_u8(255), None);

    // 3. update_literal() transition matrix
    assert_eq!(State0.update_literal(), State0);
    assert_eq!(State1.update_literal(), State0);
    assert_eq!(State2.update_literal(), State0);
    assert_eq!(State3.update_literal(), State0);
    assert_eq!(State4.update_literal(), State1);
    assert_eq!(State5.update_literal(), State2);
    assert_eq!(State6.update_literal(), State3);
    assert_eq!(State7.update_literal(), State4);
    assert_eq!(State8.update_literal(), State5);
    assert_eq!(State9.update_literal(), State6);
    assert_eq!(State10.update_literal(), State4);
    assert_eq!(State11.update_literal(), State5);

    // 4. update_match() transition matrix
    for i in 0..7u8 {
        let st = LzmaState::from_u8(i).unwrap();
        assert_eq!(st.update_match(), State7);
    }
    for i in 7..12u8 {
        let st = LzmaState::from_u8(i).unwrap();
        assert_eq!(st.update_match(), State10);
    }

    // 5. update_rep() transition matrix
    for i in 0..7u8 {
        let st = LzmaState::from_u8(i).unwrap();
        assert_eq!(st.update_rep(), State8);
    }
    for i in 7..12u8 {
        let st = LzmaState::from_u8(i).unwrap();
        assert_eq!(st.update_rep(), State11);
    }

    // 6. update_short_rep() transition matrix
    for i in 0..7u8 {
        let st = LzmaState::from_u8(i).unwrap();
        assert_eq!(st.update_short_rep(), State9);
    }
    for i in 7..12u8 {
        let st = LzmaState::from_u8(i).unwrap();
        assert_eq!(st.update_short_rep(), State11);
    }

    // 7. Full lifecycle trajectory simulation
    state = State0; // start
    state = state.update_match(); // -> 7 (LitMatch)
    assert_eq!(state, State7);
    state = state.update_literal(); // -> 4 (MatchMatch)
    assert_eq!(state, State4);
    state = state.update_literal(); // -> 1 (MatchLit)
    assert_eq!(state, State1);
    state = state.update_literal(); // -> 0 (LitLit)
    assert_eq!(state, State0);
    state = state.update_rep(); // -> 8 (LitRep)
    assert_eq!(state, State8);
    state = state.update_short_rep(); // -> 11 (LitRepLit)
    assert_eq!(state, State11);
    state = state.update_literal(); // -> 5 (RepMatch)
    assert_eq!(state, State5);
    state = state.update_short_rep(); // -> 9 (LitShortRep)
    assert_eq!(state, State9);
}

#[test]
fn test_literal_properties_and_prob_table() {
    // 1. Valid property construction
    let props = LiteralProperties::new(3, 0, 2).expect("valid props");
    assert_eq!(props.lc, 3);
    assert_eq!(props.lp, 0);
    assert_eq!(props.pb, 2);
    assert_eq!(props.num_literal_contexts(), 8);
    assert_eq!(props.literal_probs_len(), 8 * 0x300);

    // 2. Packed property byte serialization
    let byte = props.to_byte();
    let unpacked = LiteralProperties::from_byte(byte).expect("unpack");
    assert_eq!(props, unpacked);

    // 3. Property boundary limits
    assert!(LiteralProperties::new(9, 0, 2).is_err()); // lc > 8
    assert!(LiteralProperties::new(3, 5, 2).is_err()); // lp > 4
    assert!(LiteralProperties::new(3, 0, 5).is_err()); // pb > 4
    assert!(LiteralProperties::new(8, 5, 2).is_err()); // lc + lp > 12
    assert!(LiteralProperties::from_byte(255).is_err());

    // 4. Pos state context calculation
    assert_eq!(props.pos_state(0), 0);
    assert_eq!(props.pos_state(1), 1);
    assert_eq!(props.pos_state(3), 3);
    assert_eq!(props.pos_state(4), 0); // pb=2 -> mask=3

    // 5. Probability table initialization and reset
    let mut table = LzmaProbTable::new(props);
    assert_eq!(table.literal_probs.len(), 8 * 0x300);
    assert!(table.literal_probs.iter().all(|&p| p == PROB_INIT_VAL));

    // Mutate table sub-contexts
    let sub1 = table.literal_sub_table_mut(0, b'A');
    sub1[0] = 500;
    assert_eq!(table.literal_sub_table(0, b'A')[0], 500);

    table.reset();
    assert_eq!(table.literal_sub_table(0, b'A')[0], PROB_INIT_VAL);
    assert!(table.literal_probs.iter().all(|&p| p == PROB_INIT_VAL));

    // 6. LenCoderProbs test
    let mut len_coder = LenCoderProbs::new();
    len_coder.choice1 = 123;
    len_coder.reset();
    assert_eq!(len_coder.choice1, PROB_INIT_VAL);
}

#[test]
fn test_range_coder_bit_roundtrip_deterministic() {
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
    ];

    let mut encoder = RangeEncoder::new();
    let mut enc_probs = [PROB_INIT_VAL; 16];
    let mut stream = Vec::new();
    let mut expected_prob_snapshots = Vec::new();

    for &(ctx, bit) in &test_bits {
        encoder.encode_bit(&mut enc_probs[ctx], bit, &mut stream);
        expected_prob_snapshots.push(enc_probs[ctx]);
    }
    encoder.finish(&mut stream);

    assert!(stream.len() >= 5);
    let mut decoder = RangeDecoder::new(&stream).expect("range decoder init");
    let mut dec_probs = [PROB_INIT_VAL; 16];

    for (idx, &(ctx, expected_bit)) in test_bits.iter().enumerate() {
        let decoded_bit = decoder
            .decode_bit(&mut dec_probs[ctx])
            .unwrap_or_else(|_| panic!("failed decoding bit at {idx}"));
        assert_eq!(decoded_bit, expected_bit, "Bit mismatch at step {idx}");
        assert_eq!(
            dec_probs[ctx], expected_prob_snapshots[idx],
            "Probability snapshot mismatch at step {idx}"
        );
    }
    assert_eq!(enc_probs, dec_probs);
}

#[test]
fn test_range_coder_direct_bits_branchless_roundtrip() {
    let test_values: Vec<(u32, u32)> = vec![
        (0b0, 1),
        (0b1, 1),
        (0b10, 2),
        (0b01, 2),
        (0b11, 2),
        (0b1010_1100, 8),
        (0xDEAD_BEEF, 32),
        (0x0000_0000, 32),
        (0xFFFF_FFFF, 32),
        (0x5555_5555, 32),
        (0xAAAA_AAAA, 32),
        (12345, 14),
    ];

    let mut encoder = RangeEncoder::new();
    let mut stream = Vec::new();

    for &(val, num_bits) in &test_values {
        encoder.encode_direct_bits(val, num_bits, &mut stream);
    }
    encoder.finish(&mut stream);

    let mut decoder = RangeDecoder::new(&stream).expect("decoder init");
    for (idx, &(expected_val, num_bits)) in test_values.iter().enumerate() {
        let decoded = decoder
            .decode_direct_bits(num_bits)
            .unwrap_or_else(|_| panic!("failed decoding direct bits at {idx}"));
        assert_eq!(
            decoded, expected_val,
            "Direct bit mismatch at index {idx}, bits: {num_bits}"
        );
    }
}

#[test]
fn test_range_coder_bit_tree_and_reverse_tree() {
    let mut encoder = RangeEncoder::new();
    let mut tree_probs = [PROB_INIT_VAL; 64];
    let mut rev_tree_probs = [PROB_INIT_VAL; 64];
    let mut stream = Vec::new();

    // Encode symbols 0..8 with 3 bits
    for sym in 0..8u32 {
        encoder.encode_bit_tree(&mut tree_probs, sym, 3, &mut stream);
        encoder.encode_reverse_bit_tree(&mut rev_tree_probs, sym, 3, &mut stream);
    }
    encoder.finish(&mut stream);

    let mut decoder = RangeDecoder::new(&stream).expect("decoder init");
    let mut dec_tree_probs = [PROB_INIT_VAL; 64];
    let mut dec_rev_tree_probs = [PROB_INIT_VAL; 64];

    for sym in 0..8u32 {
        let dec_sym = decoder.decode_bit_tree(&mut dec_tree_probs, 3).unwrap();
        assert_eq!(dec_sym, sym, "Bit tree symbol mismatch");

        let dec_rev_sym = decoder
            .decode_reverse_bit_tree(&mut dec_rev_tree_probs, 3)
            .unwrap();
        assert_eq!(dec_rev_sym, sym, "Reverse bit tree symbol mismatch");
    }
}

#[test]
fn test_range_coder_matched_and_literal_byte() {
    let mut encoder = RangeEncoder::new();
    let mut lit_probs = [PROB_INIT_VAL; 0x300];
    let mut stream = Vec::new();

    let test_bytes = [b'A', b'B', b'Z', 0x00, 0xFF, 0x55, 0xAA];
    let match_bytes = [b'A', b'C', b'Z', 0x01, 0xFE, 0x55, 0x55];

    for (&byte, &match_byte) in test_bytes.iter().zip(match_bytes.iter()) {
        encoder.encode_matched_byte(&mut lit_probs, byte, match_byte, &mut stream);
    }
    encoder.finish(&mut stream);

    let mut decoder = RangeDecoder::new(&stream).expect("decoder init");
    let mut dec_lit_probs = [PROB_INIT_VAL; 0x300];

    for (&expected_byte, &match_byte) in test_bytes.iter().zip(match_bytes.iter()) {
        let dec_byte = decoder
            .decode_matched_byte(&mut dec_lit_probs, match_byte)
            .unwrap();
        assert_eq!(dec_byte, expected_byte, "Matched byte mismatch");
    }
}

#[test]
fn test_range_coder_defensive_error_handling() {
    // 1. Less than 5 bytes in new()
    let short_buf = [0u8; 4];
    assert_eq!(
        RangeDecoder::new(&short_buf).unwrap_err(),
        RangeCoderError::UnexpectedEof
    );

    // 2. Truncated stream during bit decoding
    let valid_stream = [0u8; 5];
    let mut decoder = RangeDecoder::new(&valid_stream).expect("init ok");
    let mut prob = PROB_INIT_VAL;
    // Prebuffering consumed all 5 bytes, next bit requiring normalization will hit EOF
    let mut eof_hit = false;
    for _ in 0..100 {
        if decoder.decode_bit(&mut prob).is_err() {
            eof_hit = true;
            break;
        }
    }
    assert!(eof_hit, "Expected UnexpectedEof on truncated bitstream");

    // 3. Truncated direct bit decoding
    let mut decoder2 = RangeDecoder::new(&valid_stream).expect("init ok");
    let mut eof_hit2 = false;
    for _ in 0..100 {
        if decoder2.decode_direct_bits(8).is_err() {
            eof_hit2 = true;
            break;
        }
    }
    assert!(eof_hit2, "Expected UnexpectedEof on direct bits");
}

#[test]
fn test_range_coder_fuzz_random_inputs_no_panic() {
    // Simple LCG pseudo-random generator
    let mut seed: u64 = 0x1234_5678_9ABC_DEF0;
    let mut rand = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 32) as u32
    };

    for _ in 0..100 {
        let len = 5 + (rand() % 256) as usize;
        let mut data = vec![0u8; len];
        for b in &mut data {
            *b = (rand() & 0xFF) as u8;
        }

        if let Ok(mut decoder) = RangeDecoder::new(&data) {
            let mut probs = [PROB_INIT_VAL; 16];
            for _ in 0..50 {
                let ctx = (rand() % 16) as usize;
                if decoder.decode_bit(&mut probs[ctx]).is_err() {
                    break;
                }
                if decoder.decode_direct_bits(3).is_err() {
                    break;
                }
            }
        }
    }
}
