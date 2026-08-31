// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit and property verification tests for BCJ2 11-Bit 258-Context Binary Range Coder.

use std::io::Cursor;
use ttzip_engine::codecs::branch::bcj2::{
    Bcj2RangeDecoder, Bcj2RangeDecoderProbs, Bcj2RangeEncoder, BIT_MODEL_TOTAL, NUM_BCJ2_PROBS,
    NUM_BIT_MODEL_TOTAL_BITS, NUM_MOVE_BITS, PROB_INIT_VAL, TOP_VALUE,
};

/// Deterministic 64-bit XorShift pseudo-random number generator for property testing.
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed.max(1),
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_bit(&mut self) -> u32 {
        (self.next_u64() & 1) as u32
    }

    fn next_range(&mut self, max: usize) -> usize {
        (self.next_u64() as usize) % max
    }
}

#[test]
fn test_bcj2_range_constants_and_probs_initialization() {
    assert_eq!(NUM_BIT_MODEL_TOTAL_BITS, 11);
    assert_eq!(BIT_MODEL_TOTAL, 2048);
    assert_eq!(NUM_MOVE_BITS, 5);
    assert_eq!(PROB_INIT_VAL, 1024);
    assert_eq!(NUM_BCJ2_PROBS, 258);
    assert_eq!(TOP_VALUE, 1 << 24);

    let default_probs = Bcj2RangeDecoderProbs::default();
    assert_eq!(default_probs.probs.len(), 258);
    for &p in default_probs.as_slice() {
        assert_eq!(p, 1024);
    }

    let mut new_probs = Bcj2RangeDecoderProbs::new();
    new_probs[0] = 500;
    new_probs[257] = 1800;
    assert_eq!(new_probs[0], 500);
    assert_eq!(new_probs[257], 1800);

    new_probs.reset();
    assert_eq!(new_probs[0], 1024);
    assert_eq!(new_probs[257], 1024);
}

#[test]
fn test_bcj2_probability_update_mathematical_invariants() {
    // Mathematical verification:
    // When bit = 0: P' = P + ((2048 - P) >> 5)
    // Starting with P = 1024: P' = 1024 + ((2048 - 1024) >> 5) = 1024 + 32 = 1056
    let mut prob_enc = PROB_INIT_VAL;
    let mut buffer = Vec::new();
    let mut encoder = Bcj2RangeEncoder::new(&mut buffer);

    encoder
        .encode_bit(&mut prob_enc, 0)
        .expect("encode_bit failed");
    assert_eq!(
        prob_enc, 1056,
        "Bit 0 probability adaptation must equal 1056"
    );

    // When bit = 1: P'' = P' - (P' >> 5)
    // Starting with P' = 1056: P'' = 1056 - (1056 >> 5) = 1056 - 33 = 1023
    encoder
        .encode_bit(&mut prob_enc, 1)
        .expect("encode_bit failed");
    assert_eq!(
        prob_enc, 1023,
        "Bit 1 probability adaptation must equal 1023"
    );

    encoder.flush().expect("flush failed");

    // Decode and verify exact matching probability updates
    let mut decoder = Bcj2RangeDecoder::new(Cursor::new(buffer)).expect("decoder init failed");
    let mut prob_dec = PROB_INIT_VAL;

    let bit0 = decoder
        .decode_bit(&mut prob_dec)
        .expect("decode_bit failed");
    assert_eq!(bit0, 0);
    assert_eq!(prob_dec, 1056);

    let bit1 = decoder
        .decode_bit(&mut prob_dec)
        .expect("decode_bit failed");
    assert_eq!(bit1, 1);
    assert_eq!(prob_dec, 1023);
}

#[test]
fn test_bcj2_range_coder_100k_random_bits_roundtrip() {
    const NUM_ITERATIONS: usize = 100_000;
    let mut rng = SimpleRng::new(0xDEAD_BEEF_CAFE_BABE);

    let mut stream_records = Vec::with_capacity(NUM_ITERATIONS);
    for _ in 0..NUM_ITERATIONS {
        let ctx = rng.next_range(NUM_BCJ2_PROBS);
        let bit = rng.next_bit();
        stream_records.push((ctx, bit));
    }

    let mut output_bytes = Vec::new();
    let mut enc_probs = Bcj2RangeDecoderProbs::new();
    let mut encoder = Bcj2RangeEncoder::new(&mut output_bytes);

    for &(ctx, bit) in &stream_records {
        encoder
            .encode_bit(&mut enc_probs[ctx], bit)
            .expect("encode_bit failed");
    }
    encoder.flush().expect("encoder flush failed");

    assert!(
        !output_bytes.is_empty(),
        "Encoded stream must not be empty"
    );

    let mut dec_probs = Bcj2RangeDecoderProbs::new();
    let mut decoder =
        Bcj2RangeDecoder::new(Cursor::new(output_bytes)).expect("decoder init failed");

    for (i, &(ctx, expected_bit)) in stream_records.iter().enumerate() {
        let bit = decoder
            .decode_bit(&mut dec_probs[ctx])
            .unwrap_or_else(|e| panic!("decode_bit failed at step {i}: {e}"));
        assert_eq!(
            bit, expected_bit,
            "Decoded bit mismatch at iteration {i} (context {ctx})"
        );
    }

    // Verify all 258 probability models converge to the exact same final states
    for ctx in 0..NUM_BCJ2_PROBS {
        assert_eq!(
            dec_probs[ctx], enc_probs[ctx],
            "Probability model divergence at context {ctx}"
        );
    }
}

#[test]
fn test_bcj2_range_coder_deterministic_patterns() {
    let patterns: Vec<(&str, Vec<(usize, u32)>)> = vec![
        (
            "Alternating 0-1 on single context",
            (0..10_000).map(|i| (0, (i % 2) as u32)).collect(),
        ),
        (
            "Context rotation with constant bit 0",
            (0..10_000).map(|i| (i % NUM_BCJ2_PROBS, 0)).collect(),
        ),
        (
            "Context rotation with constant bit 1",
            (0..10_000).map(|i| (i % NUM_BCJ2_PROBS, 1)).collect(),
        ),
        (
            "Long alternating blocks (256 zeros then 256 ones)",
            (0..10_000)
                .map(|i| ((i / 256) % NUM_BCJ2_PROBS, ((i / 256) % 2) as u32))
                .collect(),
        ),
    ];

    for (name, stream_data) in patterns {
        let mut encoded = Vec::new();
        let mut enc_probs = Bcj2RangeDecoderProbs::new();
        let mut encoder = Bcj2RangeEncoder::new(&mut encoded);

        for &(ctx, bit) in &stream_data {
            encoder
                .encode_bit(&mut enc_probs[ctx], bit)
                .expect("encode failed");
        }
        encoder.flush().expect("flush failed");

        let mut dec_probs = Bcj2RangeDecoderProbs::new();
        let mut decoder = Bcj2RangeDecoder::new(Cursor::new(encoded)).expect("decoder init failed");

        for (idx, &(ctx, expected)) in stream_data.iter().enumerate() {
            let bit = decoder
                .decode_bit(&mut dec_probs[ctx])
                .unwrap_or_else(|e| panic!("{name} decode failed at index {idx}: {e}"));
            assert_eq!(bit, expected, "Mismatch in pattern '{name}' at index {idx}");
        }
    }
}

#[test]
fn test_bcj2_range_coder_boundary_all_zeros() {
    const COUNT: usize = 20_000;
    let mut encoded = Vec::new();
    let mut enc_probs = Bcj2RangeDecoderProbs::new();
    let mut encoder = Bcj2RangeEncoder::new(&mut encoded);

    for i in 0..COUNT {
        let ctx = i % NUM_BCJ2_PROBS;
        encoder
            .encode_bit(&mut enc_probs[ctx], 0)
            .expect("encode_bit failed");
    }
    encoder.flush().expect("flush failed");

    let mut dec_probs = Bcj2RangeDecoderProbs::new();
    let mut decoder = Bcj2RangeDecoder::new(Cursor::new(encoded)).expect("decoder init failed");

    for i in 0..COUNT {
        let ctx = i % NUM_BCJ2_PROBS;
        let bit = decoder
            .decode_bit(&mut dec_probs[ctx])
            .expect("decode_bit failed");
        assert_eq!(bit, 0, "Expected bit 0 at index {i}");
    }
}

#[test]
fn test_bcj2_range_coder_boundary_all_ones() {
    const COUNT: usize = 20_000;
    let mut encoded = Vec::new();
    let mut enc_probs = Bcj2RangeDecoderProbs::new();
    let mut encoder = Bcj2RangeEncoder::new(&mut encoded);

    for i in 0..COUNT {
        let ctx = i % NUM_BCJ2_PROBS;
        encoder
            .encode_bit(&mut enc_probs[ctx], 1)
            .expect("encode_bit failed");
    }
    encoder.flush().expect("flush failed");

    let mut dec_probs = Bcj2RangeDecoderProbs::new();
    let mut decoder = Bcj2RangeDecoder::new(Cursor::new(encoded)).expect("decoder init failed");

    for i in 0..COUNT {
        let ctx = i % NUM_BCJ2_PROBS;
        let bit = decoder
            .decode_bit(&mut dec_probs[ctx])
            .expect("decode_bit failed");
        assert_eq!(bit, 1, "Expected bit 1 at index {i}");
    }
}

#[test]
fn test_bcj2_range_coder_carry_cascade_0xff_torture() {
    // Constructs an adversarial sequence designed to generate long runs of 0xFF bytes
    // in the range coder low register, heavily exercising carry cascade propagation.
    let mut rng = SimpleRng::new(0x1337_C0DE_CAFE_0001);
    let mut encoded = Vec::new();
    let mut enc_probs = Bcj2RangeDecoderProbs::new();
    let mut encoder = Bcj2RangeEncoder::new(&mut encoded);

    let mut expected_bits = Vec::with_capacity(30_000);
    for _ in 0..30_000 {
        let ctx = (rng.next_range(8)) * 32; // Spread across selected context strides
        let bit = if rng.next_range(100) < 95 { 1 } else { 0 };
        expected_bits.push((ctx, bit));
        encoder
            .encode_bit(&mut enc_probs[ctx], bit)
            .expect("encode failed");
    }
    encoder.flush().expect("flush failed");

    let mut dec_probs = Bcj2RangeDecoderProbs::new();
    let mut decoder = Bcj2RangeDecoder::new(Cursor::new(encoded)).expect("decoder init failed");

    for (idx, &(ctx, exp)) in expected_bits.iter().enumerate() {
        let bit = decoder
            .decode_bit(&mut dec_probs[ctx])
            .unwrap_or_else(|e| panic!("Carry cascade decode error at {idx}: {e}"));
        assert_eq!(bit, exp, "Mismatch at step {idx}");
    }
}

#[test]
fn test_bcj2_range_coder_direct_bits_and_mixed_stream() {
    let mut rng = SimpleRng::new(0x4242_9999_7777_1111);
    let mut encoded = Vec::new();
    let mut enc_probs = Bcj2RangeDecoderProbs::new();
    let mut encoder = Bcj2RangeEncoder::new(&mut encoded);

    enum BitKind {
        Modeled(usize, u32),
        Direct(u32),
    }

    let mut ops = Vec::with_capacity(20_000);
    for _ in 0..20_000 {
        if rng.next_bit() == 0 {
            let ctx = rng.next_range(NUM_BCJ2_PROBS);
            let bit = rng.next_bit();
            ops.push(BitKind::Modeled(ctx, bit));
            encoder
                .encode_bit(&mut enc_probs[ctx], bit)
                .expect("encode modeled failed");
        } else {
            let bit = rng.next_bit();
            ops.push(BitKind::Direct(bit));
            encoder
                .encode_direct_bit(bit)
                .expect("encode direct failed");
        }
    }
    encoder.flush().expect("flush failed");

    let mut dec_probs = Bcj2RangeDecoderProbs::new();
    let mut decoder = Bcj2RangeDecoder::new(Cursor::new(encoded)).expect("decoder init failed");

    for (idx, op) in ops.iter().enumerate() {
        match *op {
            BitKind::Modeled(ctx, exp) => {
                let bit = decoder
                    .decode_bit(&mut dec_probs[ctx])
                    .unwrap_or_else(|e| panic!("Modeled bit decode error at {idx}: {e}"));
                assert_eq!(bit, exp, "Modeled bit mismatch at {idx}");
            }
            BitKind::Direct(exp) => {
                let bit = decoder
                    .decode_direct_bit()
                    .unwrap_or_else(|e| panic!("Direct bit decode error at {idx}: {e}"));
                assert_eq!(bit, exp, "Direct bit mismatch at {idx}");
            }
        }
    }
}

#[test]
fn test_bcj2_range_decoder_defensive_errors() {
    // 1. Header with fewer than 5 bytes must return UnexpectedEof
    let short_data = [0x00, 0x11, 0x22, 0x33];
    let res = Bcj2RangeDecoder::new(Cursor::new(short_data));
    assert!(
        res.is_err(),
        "Decoder init with < 5 bytes must return an error"
    );

    // 2. Decoder hitting premature EOF during decode_bit renormalization
    let mut small_stream = Vec::new();
    let mut encoder = Bcj2RangeEncoder::new(&mut small_stream);
    let mut prob = PROB_INIT_VAL;
    encoder
        .encode_bit(&mut prob, 1)
        .expect("encode single bit");
    encoder.flush().expect("flush");

    // Truncate stream to 5 bytes
    let truncated = &small_stream[0..5.min(small_stream.len())];
    let mut decoder = Bcj2RangeDecoder::new(Cursor::new(truncated)).expect("init ok with 5 bytes");
    let mut dec_prob = PROB_INIT_VAL;

    // First decode might consume the pre-buffered bits, subsequent ones will hit EOF
    let mut eof_hit = false;
    for _ in 0..100 {
        if decoder.decode_bit(&mut dec_prob).is_err() {
            eof_hit = true;
            break;
        }
    }
    assert!(
        eof_hit,
        "Truncated stream must eventually trigger an EOF error upon renormalization"
    );
}
