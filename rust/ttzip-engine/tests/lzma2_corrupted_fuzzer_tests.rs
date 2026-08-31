// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! LZMA2 Corrupted Fuzzer, 6-Dimensional Fault Injection & Mutation Stress Test Suite.
//!
//! Ported and adapted from canonical `fast-lzma2` (FL2) and LZMA reference `fuzzer.c`:
//! 1. Truncated Data Stream (Header 1~5B and exhaustive arbitrary offset truncation).
//! 2. Corrupted Range State (Malicious `code >= range`, `range == 0`, and probability extremes).
//! 3. Dictionary Underflow (`distance > output_offset` cross-boundary match attacks).
//! 4. 32/64-bit Integer Overflow Bomb (0xFF UnpackSize/Chunk lengths and `SSIZE_MAX` clamping).
//! 5. 1B/1B Jitter Streaming (Byte-by-byte step pumping and asymmetric chunking).
//! 6. Cancel & Timeout Resilience (Mid-stream abort, timeout triggers, and RAII leak-free cleanup).
//! 7. 1,000-iteration pseudorandom 1% bit-flip and byte mutation fuzz stress loop.

use std::panic::{catch_unwind, AssertUnwindSafe};
use ttzip_engine::codecs::lzma::{
    RangeDecoder, BIT_MODEL_TOTAL, PROB_INIT_VAL,
};
use ttzip_engine::codecs::lzma2::{
    fl2_compress, fl2_compress_bound, fl2_decompress, fl2_find_decompressed_size,
    Fl2CStream, Fl2DStream, Fl2InBuffer, Fl2OutBuffer,
};
use ttzip_engine::types::TTZipStatus;

// MARK: - 1. Deterministic Knuth Multiplicative PRNG (FUZ_rand)

/// Deterministic Knuth multiplicative hash PRNG from canonical LZMA2 `fuzzer.c`.
///
/// Formula: `state = (state * 2654435761U) + 2246822519U; return state >> 13;`
#[derive(Debug, Clone)]
pub struct FuzRand {
    seed: u32,
}

impl FuzRand {
    /// Creates a new `FuzRand` instance initialized with `seed`.
    #[inline]
    pub const fn new(seed: u32) -> Self {
        Self { seed }
    }

    /// Computes the next 32-bit pseudo-random value matching `FUZ_rand()`.
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        self.seed = self
            .seed
            .wrapping_mul(2_654_435_761)
            .wrapping_add(2_246_822_519);
        self.seed >> 13
    }

    /// Returns a pseudo-random integer in the closed interval `[min, max]`.
    #[inline]
    pub fn rand_range(&mut self, min: u32, max: u32) -> u32 {
        if min >= max {
            return min;
        }
        let span = max - min + 1;
        min + (self.next_u32() % span)
    }

    /// Returns a pseudo-random `usize` in half-open interval `[0, bound)`.
    #[inline]
    pub fn rand_usize(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next_u32() as usize) % bound
        }
    }

    /// Returns a pseudo-random byte `u8`.
    #[inline]
    pub fn rand_u8(&mut self) -> u8 {
        (self.next_u32() & 0xFF) as u8
    }

    /// Generates pseudo-random buffer with target compressibility.
    pub fn gen_buffer(&mut self, size: usize, compressibility_pct: u32) -> Vec<u8> {
        let mut buf = vec![0u8; size];
        if compressibility_pct >= 100 {
            // Sparse / all-zero
            return buf;
        }
        let alphabet_len = match compressibility_pct {
            0..=10 => 256,
            11..=30 => 128,
            31..=60 => 32,
            61..=80 => 8,
            81..=95 => 3,
            _ => 1,
        };
        for b in buf.iter_mut() {
            *b = (self.next_u32() % alphabet_len) as u8;
        }
        buf
    }
}

// MARK: - Dimension 1: Truncated Data Stream (Header & Payload)

#[test]
fn test_lzma2_dim1_truncated_headers_and_property_byte_rejection() {
    // LZMA2 stream requires at least a 1-byte dictionary property header + chunk headers.
    for len in 0..=5 {
        let truncated = vec![0x1Fu8; len];
        let mut dst = vec![0u8; 1024];

        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            fl2_decompress(&truncated, &mut dst, 1)
        }));
        assert!(
            unwind_res.is_ok(),
            "Decompressing truncated {len}-byte header panicked!"
        );
        let res = unwind_res.unwrap();
        assert!(
            res.is_err(),
            "Truncated {len}-byte header must fail with typed error"
        );
        let err = res.unwrap_err();
        assert!(
            err == TTZipStatus::ErrCorruptHeader || err == TTZipStatus::ErrExtractionFailed,
            "Expected ErrCorruptHeader or ErrExtractionFailed, got: {:?}",
            err
        );
    }
}

#[test]
fn test_lzma2_dim1_exhaustive_byte_by_byte_truncation_sweep() {
    // Generate valid LZMA2 compressed payloads
    let mut rng = FuzRand::new(0x2026_0830);
    let sample = rng.gen_buffer(4096, 60);

    let mut comp_buf = vec![0u8; fl2_compress_bound(sample.len()) + 1024];
    let comp_len = fl2_compress(&sample, &mut comp_buf, 3, 1).expect("compression failed");
    let valid_stream = &comp_buf[..comp_len];

    // Sweep all truncation prefixes from 0 to comp_len - 1
    let mut dst = vec![0u8; sample.len() + 256];
    for trunc_len in 0..comp_len {
        let slice = &valid_stream[..trunc_len];
        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            fl2_decompress(slice, &mut dst, 1)
        }));
        assert!(
            unwind_res.is_ok(),
            "Truncation at offset {trunc_len}/{comp_len} panicked!"
        );
        let res = unwind_res.unwrap();
        assert!(
            res.is_err(),
            "Truncated slice at offset {trunc_len}/{comp_len} must return error"
        );
    }
}

#[test]
fn test_lzma2_dim1_streaming_dstream_truncated_input_safety() {
    let payload = b"TTZip High-Performance LZMA2 streaming truncation resilience test data 2026.";
    let mut comp_buf = vec![0u8; fl2_compress_bound(payload.len()) + 1024];
    let comp_len = fl2_compress(payload, &mut comp_buf, 3, 1).expect("compress");

    // Feed truncated input into Fl2DStream
    let truncated_len = comp_len / 2;
    let mut dstream = Fl2DStream::new(1).expect("create dstream");
    dstream.init(None).expect("init dstream");

    let mut in_buf = Fl2InBuffer {
        src: comp_buf.as_ptr() as *const libc::c_void,
        size: truncated_len,
        pos: 0,
    };
    let mut out_data = vec![0u8; payload.len() * 2];
    let mut out_buf = Fl2OutBuffer {
        dst: out_data.as_mut_ptr() as *mut libc::c_void,
        size: out_data.len(),
        pos: 0,
    };

    let unwind_res = catch_unwind(AssertUnwindSafe(|| {
        dstream.decompress_stream(&mut in_buf, &mut out_buf)
    }));
    assert!(unwind_res.is_ok(), "Streaming truncated feed panicked!");
    // Streaming decoder should either consume available input (waiting for more) or report typed error.
    assert!(in_buf.pos <= in_buf.size);
    assert!(out_buf.pos <= out_buf.size);
}

// MARK: - Dimension 2: Corrupted Range State Injection

#[test]
fn test_lzma2_dim2_corrupted_range_coder_state_injection() {
    // 1. Degenerate range code inputs (code >= range, all 0xFF, all 0x00)
    let malformed_streams = [
        vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        vec![0x00, 0x00, 0x00, 0x00, 0x00],
        vec![0x00, 0xFF, 0x00, 0xFF, 0x00],
        vec![0x80, 0x00, 0x00, 0x00, 0x00],
    ];

    for (idx, stream) in malformed_streams.iter().enumerate() {
        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            if let Ok(mut decoder) = RangeDecoder::new(stream) {
                let mut prob = PROB_INIT_VAL;
                for _ in 0..50 {
                    if decoder.decode_bit(&mut prob).is_err() {
                        break;
                    }
                    if decoder.decode_direct_bits(4).is_err() {
                        break;
                    }
                }
            }
        }));
        assert!(
            unwind_res.is_ok(),
            "Malformed stream #{idx} caused range coder panic!"
        );
    }
}

#[test]
fn test_lzma2_dim2_degenerate_probability_table_extremes() {
    let stream = vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
    let mut decoder = RangeDecoder::new(&stream).expect("range decoder init");

    // Test extreme probability values: 0, 1, 2047, 2048 (BIT_MODEL_TOTAL)
    let extreme_probs = [0u16, 1, 2047, BIT_MODEL_TOTAL as u16, 4096];
    for &prob_val in &extreme_probs {
        let mut prob = prob_val;
        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            let _ = decoder.decode_bit(&mut prob);
        }));
        assert!(
            unwind_res.is_ok(),
            "Probability value {prob_val} caused panic in decode_bit!"
        );
    }
}

#[test]
fn test_lzma2_dim2_corrupted_range_payload_in_stream_decoder() {
    // LZMA2 chunk starting with 0x80 (uncompressed 2MB, pack size 64KB, range payload)
    let mut malformed_lzma2 = vec![
        0x1F, // Dict property
        0x80, // Control byte: LZMA state reset
        0x00, 0x10, // Unpack size high/low
        0x00, 0x08, // Pack size
    ];
    malformed_lzma2.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
    malformed_lzma2.push(0x00); // End marker

    let mut dst = vec![0u8; 4096];
    let unwind_res = catch_unwind(AssertUnwindSafe(|| {
        fl2_decompress(&malformed_lzma2, &mut dst, 1)
    }));
    assert!(
        unwind_res.is_ok(),
        "Corrupted range payload in LZMA2 chunk panicked!"
    );
    let res = unwind_res.unwrap();
    assert!(
        res.is_err(),
        "Corrupted range payload must return typed error"
    );
}

// MARK: - Dimension 3: Dictionary Underflow (Match Offset vs History)

#[test]
fn test_lzma2_dim3_dictionary_underflow_distance_greater_than_offset() {
    // Construct LZMA2 chunk attempting match copy with distance > current decoded offset.
    let mut underflow_chunk = vec![
        0x14, // Dict property (1MB dict)
        0xE0, // Control: Reset state + reset dict + new props
        0x00, 0x20, // Unpack size = 33
        0x00, 0x08, // Pack size = 9
        0x00, // Props (lc=0, lp=0, pb=0)
    ];
    // Range coder bytes encoding a match with distance 0x1000 before any literal is decoded
    underflow_chunk.extend_from_slice(&[0x00, 0x80, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0x00]);
    underflow_chunk.push(0x00); // End marker

    let mut dst = vec![0u8; 1024];
    let unwind_res = catch_unwind(AssertUnwindSafe(|| {
        fl2_decompress(&underflow_chunk, &mut dst, 1)
    }));
    assert!(
        unwind_res.is_ok(),
        "Dictionary underflow chunk caused illegal memory read or panic!"
    );
    let res = unwind_res.unwrap();
    assert!(
        res.is_err(),
        "Dictionary underflow chunk must be safely intercepted"
    );
}

#[test]
fn test_lzma2_dim3_uninitialized_dictionary_match_cross_boundary_defense() {
    // Chunk without dictionary reset pointing back to non-existent previous block
    let mut no_dict_chunk = vec![
        0x14, // Dict prop
        0x80, // Control: No dict reset, unpack 64KB, pack 8B
        0x00, 0x40,
        0x00, 0x06,
    ];
    no_dict_chunk.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
    no_dict_chunk.push(0x00);

    let mut dst = vec![0u8; 1024];
    let res = fl2_decompress(&no_dict_chunk, &mut dst, 1);
    assert!(
        res.is_err(),
        "Uninitialized dictionary match must return typed error"
    );
}

// MARK: - Dimension 4: 32/64-Bit Integer Overflow Bomb (SSIZE_MAX Clamping)

#[test]
fn test_lzma2_dim4_integer_overflow_bomb_ssize_max_clamp_and_guard() {
    // Attack: Stream with consecutive 0xFF chunk headers and declared sizes > SSIZE_MAX
    let max_len = 128;
    let malicious_ff = vec![0xFFu8; max_len];

    for count in [1, 2, 5, 16, 64, 128] {
        let slice = &malicious_ff[..count];
        let mut dst = vec![0u8; 1024];

        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            fl2_decompress(slice, &mut dst, 1)
        }));
        assert!(
            unwind_res.is_ok(),
            "All-0xFF payload of size {count} caused panic!"
        );
        let res = unwind_res.unwrap();
        assert!(
            res.is_err(),
            "All-0xFF payload of size {count} must return error"
        );
    }
}

#[test]
fn test_lzma2_dim4_malicious_uncompressed_chunk_length_overflow() {
    // Control byte 0x01 = Uncompressed chunk (reset state = false)
    // Declared size = (0xFF << 8) | 0xFF + 1 = 65,536 bytes, but payload only contains 4 bytes
    let mut uncompressed_overflow = vec![
        0x14, // Dict prop
        0x01, // Uncompressed chunk
        0xFF, 0xFF, // Declared size = 65536
        0x41, 0x42, 0x43, 0x44, // Truncated 4 bytes
    ];
    uncompressed_overflow.push(0x00); // End marker

    let mut dst = vec![0u8; 65536];
    let res = fl2_decompress(&uncompressed_overflow, &mut dst, 1);
    assert!(
        res.is_err(),
        "Uncompressed chunk size mismatch must return error"
    );
}

#[test]
fn test_lzma2_dim4_find_decompressed_size_malformed_input_rejection() {
    // Malformed inputs to fl2_find_decompressed_size must return None (FL2_CONTENTSIZE_ERROR)
    assert_eq!(fl2_find_decompressed_size(&[]), None);
    assert_eq!(fl2_find_decompressed_size(&[0xFF]), None);
    assert_eq!(fl2_find_decompressed_size(&[0x14, 0xFF, 0xFF, 0xFF]), None);
    assert_eq!(fl2_find_decompressed_size(&[0x00]), None);
    assert_eq!(fl2_find_decompressed_size(&[0x14, 0x00]), Some(0)); // Valid empty stream (prop + end marker)
}

// MARK: - Dimension 5: 1B/1B Jitter Streaming Stress

#[test]
fn test_lzma2_dim5_1b_1b_extreme_jitter_streaming_roundtrip() {
    let test_payloads: [&[u8]; 4] = [
        b"TTZip Extreme 1-Byte In / 1-Byte Out Jitter Streaming Decompression Roundtrip.",
        b"A",
        b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        &[0x00, 0x01, 0x02, 0x03, 0xFE, 0xFF, 0x7F, 0x80, 0x12, 0x34, 0x56, 0x78],
    ];

    for (idx, &payload) in test_payloads.iter().enumerate() {
        // 1. Compress with streaming Fl2CStream using 1B step
        let mut cstream = Fl2CStream::new().expect("create cstream");
        cstream.init(3).expect("init cstream");

        let mut comp_data = Vec::new();
        let mut in_pos = 0;

        while in_pos < payload.len() {
            let mut in_buf = Fl2InBuffer {
                src: payload.as_ptr() as *const libc::c_void,
                size: (in_pos + 1).min(payload.len()),
                pos: in_pos,
            };
            let mut out_chunk = [0u8; 1];
            let mut out_buf = Fl2OutBuffer {
                dst: out_chunk.as_mut_ptr() as *mut libc::c_void,
                size: 1,
                pos: 0,
            };
            let _ = cstream.compress_chunk(&mut in_buf, &mut out_buf).expect("compress chunk");
            in_pos = in_buf.pos;
            if out_buf.pos > 0 {
                comp_data.extend_from_slice(&out_chunk[..out_buf.pos]);
            }
        }

        // Flush and finalize stream with 1B chunks
        loop {
            let mut out_chunk = [0u8; 1];
            let mut out_buf = Fl2OutBuffer {
                dst: out_chunk.as_mut_ptr() as *mut libc::c_void,
                size: 1,
                pos: 0,
            };
            let rem = cstream.end_stream(&mut out_buf).expect("end stream");
            if out_buf.pos > 0 {
                comp_data.extend_from_slice(&out_chunk[..out_buf.pos]);
            }
            if rem == 0 {
                break;
            }
        }

        assert!(!comp_data.is_empty(), "Compressed data must not be empty for payload #{idx}");

        // 2. Decompress with streaming Fl2DStream using 1B step
        let mut dstream = Fl2DStream::new(1).expect("create dstream");
        dstream.init(None).expect("init dstream");

        let mut decomp_data = Vec::new();
        let mut c_pos = 0;

        while c_pos < comp_data.len() {
            let mut in_buf = Fl2InBuffer {
                src: comp_data.as_ptr() as *const libc::c_void,
                size: (c_pos + 1).min(comp_data.len()),
                pos: c_pos,
            };
            let mut out_chunk = [0u8; 1];
            let mut out_buf = Fl2OutBuffer {
                dst: out_chunk.as_mut_ptr() as *mut libc::c_void,
                size: 1,
                pos: 0,
            };
            let rem = dstream.decompress_stream(&mut in_buf, &mut out_buf).expect("decompress stream");
            c_pos = in_buf.pos;
            if out_buf.pos > 0 {
                decomp_data.extend_from_slice(&out_chunk[..out_buf.pos]);
            }
            if rem == 0 && c_pos >= comp_data.len() {
                break;
            }
        }

        assert_eq!(
            decomp_data.as_slice(),
            payload,
            "1B/1B Jitter decompression output mismatch for payload #{idx}"
        );
    }
}

#[test]
fn test_lzma2_dim5_asymmetric_and_randomized_jitter_streaming() {
    let mut rng = FuzRand::new(0x1337_BEEF);
    let payload = rng.gen_buffer(2048, 50);

    let mut comp_buf = vec![0u8; fl2_compress_bound(payload.len()) + 1024];
    let comp_len = fl2_compress(&payload, &mut comp_buf, 3, 1).expect("fl2_compress");
    let compressed = &comp_buf[..comp_len];

    // Asymmetric tests: variable in_chunk (1..=7), variable out_chunk (1..=13)
    let mut dstream = Fl2DStream::new(1).expect("create dstream");
    dstream.init(None).expect("init dstream");

    let mut decomp_data = Vec::new();
    let mut c_pos = 0;

    while c_pos < compressed.len() {
        let in_step = rng.rand_range(1, 7) as usize;
        let out_step = rng.rand_range(1, 13) as usize;

        let mut in_buf = Fl2InBuffer {
            src: compressed.as_ptr() as *const libc::c_void,
            size: (c_pos + in_step).min(compressed.len()),
            pos: c_pos,
        };
        let mut out_chunk = vec![0u8; out_step];
        let mut out_buf = Fl2OutBuffer {
            dst: out_chunk.as_mut_ptr() as *mut libc::c_void,
            size: out_chunk.len(),
            pos: 0,
        };

        let rem = dstream.decompress_stream(&mut in_buf, &mut out_buf).expect("decompress stream");
        c_pos = in_buf.pos;
        if out_buf.pos > 0 {
            decomp_data.extend_from_slice(&out_chunk[..out_buf.pos]);
        }
        if rem == 0 && c_pos >= compressed.len() {
            break;
        }
    }

    assert_eq!(
        decomp_data.as_slice(),
        payload.as_slice(),
        "Asymmetric jitter decompression mismatch"
    );
}

// MARK: - Dimension 6: Cancel & Timeout Resilience

#[test]
fn test_lzma2_dim6_cancel_and_timeout_resilience_multi_stream_lifecycle() {
    let payload = vec![0x42u8; 65536];

    // Run 50 cancel & mid-stream abort iterations across multi-threaded compressor
    for _ in 0..50 {
        let mut cstream = Fl2CStream::new_mt(2).expect("create mt cstream");
        cstream.init(3).expect("init cstream");
        cstream.set_timeout(100).expect("set timeout");

        // Feed half of the payload
        let mut in_buf = Fl2InBuffer {
            src: payload.as_ptr() as *const libc::c_void,
            size: payload.len() / 2,
            pos: 0,
        };
        let mut out_data = vec![0u8; 1024];
        let mut out_buf = Fl2OutBuffer {
            dst: out_data.as_mut_ptr() as *mut libc::c_void,
            size: out_data.len(),
            pos: 0,
        };

        let _ = cstream.compress_chunk(&mut in_buf, &mut out_buf);
        // Explicitly cancel stream mid-flight
        cstream.cancel();
    }

    // Verify subsequent clean compression & decompression roundtrip succeeds 100%
    let mut normal_dst = vec![0u8; fl2_compress_bound(payload.len()) + 1024];
    let comp_len = fl2_compress(&payload, &mut normal_dst, 2, 1).expect("clean compress after cancel");
    let mut decomp_dst = vec![0u8; payload.len()];
    let decomp_len = fl2_decompress(&normal_dst[..comp_len], &mut decomp_dst, 1).expect("clean decompress");

    assert_eq!(decomp_len, payload.len());
    assert_eq!(&decomp_dst, &payload);
}

#[test]
fn test_lzma2_dim6_mid_stream_drop_and_reinitialization_no_leak() {
    let payload = vec![0x55u8; 32768];

    // Verify dropping streaming decoder mid-flight causes zero leak or deadlock
    for _ in 0..50 {
        let mut dstream = Fl2DStream::new(1).expect("create dstream");
        dstream.init(None).expect("init");

        let mut in_buf = Fl2InBuffer {
            src: payload.as_ptr() as *const libc::c_void,
            size: 16,
            pos: 0,
        };
        let mut out_data = vec![0u8; 32];
        let mut out_buf = Fl2OutBuffer {
            dst: out_data.as_mut_ptr() as *mut libc::c_void,
            size: out_data.len(),
            pos: 0,
        };

        let _ = dstream.decompress_stream(&mut in_buf, &mut out_buf);
        dstream.cancel();
        // Drop occurs here cleanly
    }
}

// MARK: - Dimension 7: 1% Pseudorandom Mutation Fuzzing Stress (1,000 Iterations)

#[test]
fn test_lzma2_dim7_1pct_pseudorandom_mutation_fuzzing_stress_1000_iterations() {
    let mut rng = FuzRand::new(0x2026_DEAD);

    // Prepare 2 baseline compressed archives
    let text_payload = rng.gen_buffer(2048, 70);
    let rand_payload = rng.gen_buffer(1024, 10);

    let mut comp_text = vec![0u8; fl2_compress_bound(text_payload.len()) + 1024];
    let c_text_len = fl2_compress(&text_payload, &mut comp_text, 1, 1).expect("compress text");
    comp_text.truncate(c_text_len);

    let mut comp_rand = vec![0u8; fl2_compress_bound(rand_payload.len()) + 1024];
    let c_rand_len = fl2_compress(&rand_payload, &mut comp_rand, 3, 1).expect("compress rand");
    comp_rand.truncate(c_rand_len);

    let baselines = [&comp_text, &comp_rand];
    let mut dst_buf = vec![0u8; 65536];

    for iteration in 0..1000 {
        let base = baselines[rng.rand_usize(baselines.len())];
        let mut mutated = base.clone();

        let mutation_type = rng.rand_range(0, 4);
        match mutation_type {
            0 => {
                // 1% Bit-flip mutation
                let num_flips = ((mutated.len() as f64) * 0.01).ceil() as usize;
                for _ in 0..num_flips.max(1) {
                    let byte_idx = rng.rand_usize(mutated.len());
                    let bit_idx = rng.rand_range(0, 7);
                    mutated[byte_idx] ^= 1 << bit_idx;
                }
            }
            1 => {
                // Random byte overwrite
                let num_overwrites = rng.rand_range(1, 5) as usize;
                for _ in 0..num_overwrites {
                    let idx = rng.rand_usize(mutated.len());
                    mutated[idx] = rng.rand_u8();
                }
            }
            2 => {
                // Arbitrary truncation
                let trunc_len = rng.rand_usize(mutated.len());
                mutated.truncate(trunc_len);
            }
            3 => {
                // Inject 0xFF run
                let run_len = rng.rand_range(1, 16) as usize;
                let insert_pos = rng.rand_usize(mutated.len());
                mutated.splice(insert_pos..insert_pos, std::iter::repeat_n(0xFF, run_len));
            }
            _ => {
                // Zero-out segment
                let zero_len = rng.rand_range(1, 8) as usize;
                let start_pos = rng.rand_usize(mutated.len());
                let end_pos = (start_pos + zero_len).min(mutated.len());
                for b in mutated[start_pos..end_pos].iter_mut() {
                    *b = 0;
                }
            }
        }

        // Execute decompression under catch_unwind
        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            fl2_decompress(&mutated, &mut dst_buf, 1)
        }));

        assert!(
            unwind_res.is_ok(),
            "Decompressor panicked on iteration {iteration} (mutation_type {mutation_type})!"
        );

        let res = unwind_res.unwrap();
        if let Ok(written) = res {
            assert!(
                written <= dst_buf.len(),
                "Output buffer write exceeded capacity on iteration {iteration}"
            );
        } else {
            let err = res.unwrap_err();
            assert!(
                err == TTZipStatus::ErrCorruptHeader || err == TTZipStatus::ErrExtractionFailed,
                "Unexpected error status on iteration {iteration}: {:?}",
                err
            );
        }
    }
}
