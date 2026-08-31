// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! LZ4 Corrupted Fuzzer, 32-bit Address Overflow Torture & Mutation Injection Test Suite.
//!
//! Ported and adapted from Yann Collet's canonical `fuzzer.c`:
//! 1. Deterministic Knuth Multiplicative PRNG (`FUZ_rand`) with reproducible seeding.
//! 2. 32-bit address space overflow and token/offset boundary wrap-around torture.
//! 3. All-0xFF malicious long sequence and byte-by-byte truncation attacks.
//! 4. Decoder ring buffer mathematical boundary stress ($2M + 65534$) over 1,000 cycles.
//! 5. 1,000-iteration pseudorandom 1% bit-flip and byte mutation fuzz stress loop.

use std::panic::{catch_unwind, AssertUnwindSafe};
use ttzip_engine::codecs::lz4::{
    lz4_compress_bound, lz4_compress_fast, lz4_compress_hc, lz4_decompress,
};
use ttzip_engine::types::TTZipStatus;

// MARK: - 1. Deterministic Knuth Multiplicative PRNG (FUZ_rand)

/// Deterministic Knuth multiplicative hash PRNG from canonical LZ4 `fuzzer.c`.
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

    /// Returns a boolean with `pct` percentage probability (0..=100).
    #[inline]
    pub fn rand_bool_pct(&mut self, pct: u32) -> bool {
        (self.next_u32() % 100) < pct
    }
}

// MARK: - 2. Test: Deterministic PRNG Reproducibility

#[test]
fn test_fuz_rand_knuth_multiplicative_prng_deterministic_reproducibility() {
    let mut rng1 = FuzRand::new(1337);
    let mut rng2 = FuzRand::new(1337);

    let seq1: Vec<u32> = (0..1000).map(|_| rng1.next_u32()).collect();
    let seq2: Vec<u32> = (0..1000).map(|_| rng2.next_u32()).collect();

    assert_eq!(seq1, seq2, "PRNG sequence must be 100% deterministic");

    // Verify statistical dispersion across 32-bit space
    let mut rng = FuzRand::new(0x2026_0830);
    let mut bit_counts = [0usize; 32];
    for _ in 0..10_000 {
        let val = rng.next_u32();
        for (bit_idx, count) in bit_counts.iter_mut().enumerate() {
            if (val & (1 << bit_idx)) != 0 {
                *count += 1;
            }
        }
    }

    // Every bit position should exhibit substantial entropy (between 25% and 75%)
    for (bit, &count) in bit_counts.iter().take(19).enumerate() {
        assert!(
            (2_500..=7_500).contains(&count),
            "Bit {} has unbalanced distribution: {}",
            bit,
            count
        );
    }
}

// MARK: - 3. Test: 32-bit Address Overflow & Pointer Bounds Torture

#[test]
fn test_lz4_32bit_address_overflow_f0_all_ff_accumulator_torture() {
    // Attack 1: Token 0xF0 (literal length 15 + extension bytes) followed by long 0xFF runs.
    // Tests potential integer overflow / wrap-around when accumulating literal lengths.
    for count in [1, 5, 20, 255, 1_000, 4_096, 65_536] {
        let mut malformed = Vec::with_capacity(1 + count);
        malformed.push(0xF0); // Token: Literal length 15, Match length 0
        malformed.extend(std::iter::repeat_n(0xFF, count));

        let mut dst = [0u8; 1024];
        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            lz4_decompress(&malformed, &mut dst)
        }));

        assert!(
            unwind_res.is_ok(),
            "0xF0 + 0xFF*{} caused a panic!",
            count
        );
        let res = unwind_res.unwrap();
        assert!(
            res.is_err(),
            "0xF0 + 0xFF*{} must be rejected as corrupt header",
            count
        );
        assert_eq!(res.err(), Some(TTZipStatus::ErrCorruptHeader));
    }
}

#[test]
fn test_lz4_boundary_pointer_out_of_bounds_1f_offset_torture() {
    // Attack 2: Token 0x1F (literal length 1, match length 15) with invalid offsets.
    // Offset 0x0000 is invalid in LZ4; offset 0x0001 with out-of-bounds match length.
    let payload = vec![
        0x1F, // Token: 1 literal byte, match length 15 (+extra)
        b'Z', // Literal byte
        0x01, // Offset low byte (offset = 1)
        0x00, // Offset high byte
        0xFF, // Match length extension (+255)
        0xFF, // Match length extension (+255)
        0x00, // End of match length
    ];

    for dst_size in [0, 1, 2, 16, 64, 1024, 65536] {
        let mut dst = vec![0u8; dst_size];
        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            lz4_decompress(&payload, &mut dst)
        }));

        assert!(
            unwind_res.is_ok(),
            "Token 0x1F + Offset 0x0001 caused panic on dst size {}",
            dst_size
        );
        let res = unwind_res.unwrap();
        assert!(
            res.is_err(),
            "Token 0x1F + Offset 0x0001 with invalid/truncated dst {} must return error",
            dst_size
        );
        assert_eq!(res.err(), Some(TTZipStatus::ErrCorruptHeader));
    }

    // Attack 3: Token 0x00 with invalid zero offset
    let invalid_zero_offset = [0x00, 0x00, 0x00];
    let mut dst = [0u8; 128];
    let res = lz4_decompress(&invalid_zero_offset, &mut dst);
    assert!(res.is_err(), "Zero offset must be rejected");
}

// MARK: - 4. Test: All-0xFF Malicious Long Sequences and Truncation Attacks

#[test]
fn test_lz4_all_0xff_long_sequences_and_exhaustive_truncation_attacks() {
    // Construct consecutive 31-byte and 64-byte All-0xFF malicious payloads.
    let max_len = 64;
    let malicious_ff = vec![0xFFu8; max_len];

    for trunc_len in 0..=max_len {
        let slice = &malicious_ff[..trunc_len];
        let mut dst = [0u8; 256];

        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            lz4_decompress(slice, &mut dst)
        }));

        assert!(
            unwind_res.is_ok(),
            "All-0xFF truncated at length {} caused a panic!",
            trunc_len
        );

        let res = unwind_res.unwrap();
        if trunc_len == 0 {
            assert_eq!(res, Ok(0), "Empty input must decode to 0 bytes");
        } else {
            assert!(
                res.is_err(),
                "All-0xFF slice at length {} must fail with typed error",
                trunc_len
            );
            assert_eq!(res.err(), Some(TTZipStatus::ErrCorruptHeader));
        }
    }
}

// MARK: - 5. Test: Decoder Ring Buffer Mathematical Bounds (2M + 65534)

#[test]
fn test_lz4_decoder_ring_buffer_mathematical_bounds_1000_rolling_writes() {
    // Mathematical Ring Buffer Formula: 2M + 65534 (where 65534 = LZ4_DISTANCE_MAX - 1).
    // Validates 1,000 cyclic block compressions and rolling window decompressions without
    // historical data corruption or wrap-around overwrites.
    const BLOCK_SIZE: usize = 4096;
    const RING_SIZE: usize = 2 * BLOCK_SIZE + 65534;

    let mut ring_buffer = vec![0u8; RING_SIZE];
    let mut rng = FuzRand::new(0xABCD_1234);

    let mut source_blocks = Vec::with_capacity(1000);
    let mut compressed_blocks = Vec::with_capacity(1000);

    // Pre-generate 1,000 blocks with overlapping patterns (compressible text + binary)
    for i in 0..1000 {
        let mut block = vec![0u8; BLOCK_SIZE];
        let prefix = format!("TTZip Ring Buffer Block #{i:04} - High-throughput LZ4 streaming: ");
        let prefix_bytes = prefix.as_bytes();
        block[..prefix_bytes.len()].copy_from_slice(prefix_bytes);

        for b in block[prefix_bytes.len()..].iter_mut() {
            *b = rng.rand_u8();
        }

        let bound = lz4_compress_bound(block.len());
        let mut comp = vec![0u8; bound];
        let c_len = lz4_compress_fast(&block, &mut comp, 1).expect("compress block");
        comp.truncate(c_len);

        source_blocks.push(block);
        compressed_blocks.push(comp);
    }

    // Execute 1,000 rolling-write cycles into ring buffer
    let mut current_offset = 0;
    for i in 0..1000 {
        let comp = &compressed_blocks[i];
        let expected = &source_blocks[i];

        // Ensure target buffer fits within ring buffer bounds
        if current_offset + BLOCK_SIZE > RING_SIZE {
            current_offset = 0; // Wrap around
        }

        let target_slice = &mut ring_buffer[current_offset..current_offset + BLOCK_SIZE];
        let written = lz4_decompress(comp, target_slice)
            .unwrap_or_else(|e| panic!("Ring buffer decompression failed at cycle {i}: {e:?}"));

        assert_eq!(written, BLOCK_SIZE, "Decoded size mismatch at cycle {i}");
        assert_eq!(
            target_slice,
            expected.as_slice(),
            "Data corruption detected in ring buffer at cycle {i}"
        );

        current_offset += BLOCK_SIZE;
    }
}

// MARK: - 6. Test: 1% Pseudorandom Mutation Fuzzing Stress (1,000 Iterations)

#[test]
fn test_lz4_1pct_pseudorandom_mutation_fuzzing_stress_1000_iterations() {
    // Generate valid baseline archives/blocks for mutation
    let mut rng = FuzRand::new(0x2026_DEAD);

    // Payload 1: Highly redundant pattern
    let mut text_payload = Vec::new();
    for _ in 0..40 {
        text_payload.extend_from_slice(b"TTZip native LZ4 block compression engine fuzzing test payload 2026.");
    }

    // Payload 2: Pseudo-random byte stream
    let mut rand_payload = vec![0u8; 1024];
    for b in rand_payload.iter_mut() {
        *b = rng.rand_u8();
    }

    let mut comp_text = vec![0u8; lz4_compress_bound(text_payload.len())];
    let c_text_len = lz4_compress_fast(&text_payload, &mut comp_text, 1).expect("compress text");
    comp_text.truncate(c_text_len);

    let mut comp_rand = vec![0u8; lz4_compress_bound(rand_payload.len())];
    let c_rand_len = lz4_compress_hc(&rand_payload, &mut comp_rand, 9).expect("compress rand");
    comp_rand.truncate(c_rand_len);

    let baselines = [&comp_text, &comp_rand];

    // Run 1,000 pseudo-random mutation stress iterations
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
            lz4_decompress(&mutated, &mut dst_buf)
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
                err == TTZipStatus::ErrCorruptHeader || err == TTZipStatus::ErrInvalidParam,
                "Unexpected error status on iteration {iteration}: {:?}",
                err
            );
        }
    }
}

// MARK: - 7. Test: Multi-Seed Matrix Stress Test

#[test]
fn test_lz4_random_fuzz_seeds_matrix_stress() {
    // Tests various edge-case seeds and random corrupted blocks
    let seeds = [0u32, 1, 42, 1337, 0xFFFF_FFFF, 0x8000_0000, 0x1234_5678];
    let mut dst = [0u8; 4096];

    for &seed in &seeds {
        let mut rng = FuzRand::new(seed);
        for _ in 0..100 {
            let len = rng.rand_range(1, 512) as usize;
            let mut corrupt_block = vec![0u8; len];
            for b in corrupt_block.iter_mut() {
                *b = rng.rand_u8();
            }

            let unwind_res = catch_unwind(AssertUnwindSafe(|| {
                lz4_decompress(&corrupt_block, &mut dst)
            }));

            assert!(
                unwind_res.is_ok(),
                "Random block with seed {seed:#X} caused panic"
            );
        }
    }
}
