// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration and conformance test suite for ARM NEON / Apple Silicon
//! 4-Way hardware-vectorized BLAKE3 compression kernel (`ttzip-engine`).
//!
//! Validates bit-exact conformance between 4-way NEON SIMD and reference scalar implementations
//! across Quarter-Round rotations, matrix transpositions, 7-round permutation rounds, single-block
//! compression, 1024-byte chunk hashing, parent node reduction, and throughput scaling.

use std::time::Instant;

use ttzip_engine::crypto::blake3::constants::{
    BLOCK_LEN, CHUNK_END, CHUNK_LEN, CHUNK_START, IV, KEYED_HASH, PARENT,
};
use ttzip_engine::crypto::blake3::neon::{
    hash_many_neon, hash_many_parents_neon, hash_many_variable_chunks, hash_parents_neon,
    hash4_neon,
};
use ttzip_engine::crypto::blake3::tree::parent_cv;
use ttzip_engine::crypto::blake3::{compress_in_place, ChunkState};

#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::*;
#[cfg(target_arch = "aarch64")]
use ttzip_engine::crypto::blake3::neon::{
    rot12_128, rot16_128, rot7_128, rot8_128, round_fn4, transpose_vecs_128,
};

// ============================================================================
// 1. NEON Fast Rotation Intrinsics Bit-Exact Correctness Tests
// ============================================================================

#[test]
#[cfg(target_arch = "aarch64")]
fn test_neon_quarter_round_rotations_equivalence() {
    let test_words: [u32; 4] = [
        0x12345678,
        0xDEADBEEF,
        0x00000001,
        0x80000000,
    ];

    unsafe {
        let v = vld1q_u32(test_words.as_ptr());

        // Test rot16
        let r16 = rot16_128(v);
        let mut r16_out = [0u32; 4];
        vst1q_u32(r16_out.as_mut_ptr(), r16);
        for i in 0..4 {
            assert_eq!(
                r16_out[i],
                test_words[i].rotate_right(16),
                "rot16 mismatch at lane {}",
                i
            );
        }

        // Test rot12
        let r12 = rot12_128(v);
        let mut r12_out = [0u32; 4];
        vst1q_u32(r12_out.as_mut_ptr(), r12);
        for i in 0..4 {
            assert_eq!(
                r12_out[i],
                test_words[i].rotate_right(12),
                "rot12 mismatch at lane {}",
                i
            );
        }

        // Test rot8
        let r8 = rot8_128(v);
        let mut r8_out = [0u32; 4];
        vst1q_u32(r8_out.as_mut_ptr(), r8);
        for i in 0..4 {
            assert_eq!(
                r8_out[i],
                test_words[i].rotate_right(8),
                "rot8 mismatch at lane {}",
                i
            );
        }

        // Test rot7
        let r7 = rot7_128(v);
        let mut r7_out = [0u32; 4];
        vst1q_u32(r7_out.as_mut_ptr(), r7);
        for i in 0..4 {
            assert_eq!(
                r7_out[i],
                test_words[i].rotate_right(7),
                "rot7 mismatch at lane {}",
                i
            );
        }
    }
}

// ============================================================================
// 2. 2-Level Butterfly Network Matrix Transposition Mathematical Invariant
// ============================================================================

#[test]
#[cfg(target_arch = "aarch64")]
fn test_neon_matrix_transposition_and_involution() {
    let row0 = [1u32, 2, 3, 4];
    let row1 = [5u32, 6, 7, 8];
    let row2 = [9u32, 10, 11, 12];
    let row3 = [13u32, 14, 15, 16];

    unsafe {
        let mut matrix = [
            vld1q_u32(row0.as_ptr()),
            vld1q_u32(row1.as_ptr()),
            vld1q_u32(row2.as_ptr()),
            vld1q_u32(row3.as_ptr()),
        ];

        // First transpose: row <-> column swap
        transpose_vecs_128(&mut matrix);

        let mut out0 = [0u32; 4];
        let mut out1 = [0u32; 4];
        let mut out2 = [0u32; 4];
        let mut out3 = [0u32; 4];
        vst1q_u32(out0.as_mut_ptr(), matrix[0]);
        vst1q_u32(out1.as_mut_ptr(), matrix[1]);
        vst1q_u32(out2.as_mut_ptr(), matrix[2]);
        vst1q_u32(out3.as_mut_ptr(), matrix[3]);

        assert_eq!(out0, [1, 5, 9, 13]);
        assert_eq!(out1, [2, 6, 10, 14]);
        assert_eq!(out2, [3, 7, 11, 15]);
        assert_eq!(out3, [4, 8, 12, 16]);

        // Second transpose: must yield original matrix (Involution property T(T(M)) == M)
        transpose_vecs_128(&mut matrix);
        vst1q_u32(out0.as_mut_ptr(), matrix[0]);
        vst1q_u32(out1.as_mut_ptr(), matrix[1]);
        vst1q_u32(out2.as_mut_ptr(), matrix[2]);
        vst1q_u32(out3.as_mut_ptr(), matrix[3]);

        assert_eq!(out0, row0);
        assert_eq!(out1, row1);
        assert_eq!(out2, row2);
        assert_eq!(out3, row3);
    }
}

// ============================================================================
// 3. Permutation Round Function 4-Way vs Scalar Equivalence
// ============================================================================

#[test]
#[cfg(target_arch = "aarch64")]
fn test_round_fn4_exact_scalar_conformance() {
    use ttzip_engine::crypto::blake3::compress::round_fn;

    let mut scalar_states = [[0u32; 16]; 4];
    let mut scalar_msgs = [[0u32; 16]; 4];

    for lane in 0..4 {
        for word in 0..16 {
            let s_val = ((lane as u32 + 1).wrapping_mul(0x11111111)) ^ ((word as u32 + 1).wrapping_mul(0x01010101));
            let m_val = ((lane as u32 + 7).wrapping_mul(0x22222222)) ^ ((word as u32 + 3).wrapping_mul(0x03030303));
            scalar_states[lane][word] = s_val;
            scalar_msgs[lane][word] = m_val;
        }
    }

    unsafe {
        let mut v_vecs = [vdupq_n_u32(0); 16];
        let mut m_vecs = [vdupq_n_u32(0); 16];

        for word in 0..16 {
            let state_lane = [
                scalar_states[0][word],
                scalar_states[1][word],
                scalar_states[2][word],
                scalar_states[3][word],
            ];
            let msg_lane = [
                scalar_msgs[0][word],
                scalar_msgs[1][word],
                scalar_msgs[2][word],
                scalar_msgs[3][word],
            ];
            v_vecs[word] = vld1q_u32(state_lane.as_ptr());
            m_vecs[word] = vld1q_u32(msg_lane.as_ptr());
        }

        // Run round 0..7 across both
        for r in 0..7 {
            round_fn4(&mut v_vecs, &m_vecs, r);
            for lane in 0..4 {
                round_fn(&mut scalar_states[lane], &scalar_msgs[lane], r);
            }

            for word in 0..16 {
                let mut extracted = [0u32; 4];
                vst1q_u32(extracted.as_mut_ptr(), v_vecs[word]);
                for lane in 0..4 {
                    assert_eq!(
                        extracted[lane], scalar_states[lane][word],
                        "Round {} word {} mismatch on lane {}",
                        r, word, lane
                    );
                }
            }
        }
    }
}

// ============================================================================
// 4. hash4_neon Single Block vs Scalar compress_in_place Conformance
// ============================================================================

#[test]
fn test_hash4_neon_single_block_bit_exact() {
    let key = IV;
    let mut block0 = [0u8; BLOCK_LEN];
    let mut block1 = [0u8; BLOCK_LEN];
    let mut block2 = [0u8; BLOCK_LEN];
    let mut block3 = [0u8; BLOCK_LEN];

    for i in 0..BLOCK_LEN {
        block0[i] = (i * 3 + 1) as u8;
        block1[i] = (i * 7 + 13) as u8;
        block2[i] = (i * 11 + 29) as u8;
        block3[i] = (i * 17 + 43) as u8;
    }

    let inputs: [&[u8]; 4] = [&block0, &block1, &block2, &block3];
    let mut neon_out = [[0u8; 32]; 4];
    let counter = 100u64;
    let flags = CHUNK_START | CHUNK_END;

    hash4_neon(
        inputs,
        1,
        &key,
        counter,
        true,
        flags,
        0,
        0,
        &mut neon_out,
    );

    for lane in 0..4 {
        let expected_words = compress_in_place(
            &key,
            inputs[lane].try_into().unwrap(),
            BLOCK_LEN as u8,
            counter + (lane as u64),
            flags,
        );
        let mut expected_bytes = [0u8; 32];
        for i in 0..8 {
            expected_bytes[i * 4..(i + 1) * 4].copy_from_slice(&expected_words[i].to_le_bytes());
        }
        assert_eq!(
            neon_out[lane], expected_bytes,
            "hash4_neon single block output diverged at lane {}",
            lane
        );
    }
}

// ============================================================================
// 5. hash4_neon Full 1024-Byte Chunk vs ChunkState Conformance
// ============================================================================

#[test]
fn test_hash4_neon_full_1024b_chunks_bit_exact() {
    let key = IV;
    let mut chunk0 = [0u8; CHUNK_LEN];
    let mut chunk1 = [0u8; CHUNK_LEN];
    let mut chunk2 = [0u8; CHUNK_LEN];
    let mut chunk3 = [0u8; CHUNK_LEN];

    for i in 0..CHUNK_LEN {
        chunk0[i] = (i % 251) as u8;
        chunk1[i] = ((i * 3) % 251) as u8;
        chunk2[i] = ((i * 7) % 251) as u8;
        chunk3[i] = ((i * 13) % 251) as u8;
    }

    let input_chunks = [&chunk0, &chunk1, &chunk2, &chunk3];
    let mut neon_out = [[0u8; 32]; 4];
    let start_counter = 42u64;
    let flags = 0u8;

    hash_many_neon(&input_chunks, &key, start_counter, flags, &mut neon_out);

    for lane in 0..4 {
        let mut scalar_chunk = ChunkState::new(key, start_counter + (lane as u64), flags);
        scalar_chunk.update(input_chunks[lane]);
        let expected_cv = scalar_chunk.output().chaining_value();

        assert_eq!(
            neon_out[lane], expected_cv,
            "hash_many_neon 1024B chunk diverged at lane {}",
            lane
        );
    }
}

// ============================================================================
// 6. Parent Node Merging: hash_parents_neon & hash_many_parents_neon
// ============================================================================

#[test]
fn test_hash_parents_neon_exact_parent_cv_conformance() {
    let key = [
        0x01020304, 0x05060708, 0x090A0B0C, 0x0D0E0F10,
        0x11121314, 0x15161718, 0x191A1B1C, 0x1D1E1F20,
    ];
    let flags = KEYED_HASH;

    let mut parents = [[0u8; 64]; 4];
    for p in 0..4 {
        for i in 0..64 {
            parents[p][i] = ((p + 1) * 37 + i * 5) as u8;
        }
    }

    let parent_ptrs = [&parents[0], &parents[1], &parents[2], &parents[3]];
    let mut neon_parents_out = [[0u8; 32]; 4];
    hash_parents_neon(parent_ptrs, &key, flags, &mut neon_parents_out);

    for p in 0..4 {
        let left: &[u8; 32] = parents[p][..32].try_into().unwrap();
        let right: &[u8; 32] = parents[p][32..].try_into().unwrap();
        let expected_cv = parent_cv(left, right, &key, flags);

        assert_eq!(
            neon_parents_out[p], expected_cv,
            "hash_parents_neon parent {} diverged from scalar parent_cv",
            p
        );
    }
}

#[test]
fn test_hash_many_parents_neon_varying_batch_sizes() {
    let key = IV;
    let flags = PARENT;
    let counts = [1, 2, 3, 4, 5, 7, 8, 11, 16, 23, 32];

    for &count in &counts {
        let mut parent_list = Vec::with_capacity(count);
        for i in 0..count {
            let mut block = [0u8; 64];
            for b in 0..64 {
                block[b] = ((i * 17 + b * 3) % 256) as u8;
            }
            parent_list.push(block);
        }

        let mut out = vec![[0u8; 32]; count];
        hash_many_parents_neon(&parent_list, &key, flags, &mut out);

        for (i, p) in parent_list.iter().enumerate() {
            let left: &[u8; 32] = p[..32].try_into().unwrap();
            let right: &[u8; 32] = p[32..].try_into().unwrap();
            let expected = parent_cv(left, right, &key, flags);
            assert_eq!(
                out[i], expected,
                "Batch size {} parent {} mismatch",
                count, i
            );
        }
    }
}

// ============================================================================
// 7. 4 Independent Mixed Chunk Lengths (0B, 64B, 512B, 1024B)
// ============================================================================

#[test]
fn test_mixed_chunk_lengths_conformance() {
    let key = IV;
    let flags = 0u8;
    let start_counter = 0u64;

    let chunk_0b = vec![];
    let chunk_64b = vec![0x42u8; 64];
    let chunk_512b = vec![0x7Fu8; 512];
    let mut chunk_1024b = vec![0u8; 1024];
    for (i, b) in chunk_1024b.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }

    let mixed_inputs: Vec<&[u8]> = vec![
        &chunk_0b,
        &chunk_64b,
        &chunk_512b,
        &chunk_1024b,
        &chunk_1024b,
        &chunk_512b,
        &chunk_64b,
        &chunk_0b,
    ];

    let mut out = vec![[0u8; 32]; mixed_inputs.len()];
    hash_many_variable_chunks(&mixed_inputs, &key, start_counter, flags, &mut out);

    for (i, &input) in mixed_inputs.iter().enumerate() {
        let mut scalar_state = ChunkState::new(key, start_counter + (i as u64), flags);
        scalar_state.update(input);
        let expected = scalar_state.output().chaining_value();
        assert_eq!(
            out[i], expected,
            "Mixed chunk length test failed at index {} (len: {})",
            i,
            input.len()
        );
    }
}

// ============================================================================
// 8. Large Batch Scheduling (64 Chunks) Scaling & Bit-Exact Verification
// ============================================================================

#[test]
fn test_large_batch_64_chunks_bit_exact() {
    let key = IV;
    let flags = 0u8;
    let chunk_count = 64;
    let mut chunks = Vec::with_capacity(chunk_count);

    for i in 0..chunk_count {
        let mut chunk = [0u8; 1024];
        for (b_idx, b) in chunk.iter_mut().enumerate() {
            *b = ((i * 31 + b_idx * 7) % 251) as u8;
        }
        chunks.push(chunk);
    }

    let chunk_refs: Vec<&[u8; 1024]> = chunks.iter().collect();
    let mut out = vec![[0u8; 32]; chunk_count];
    let start_counter = 1000u64;

    hash_many_neon(&chunk_refs, &key, start_counter, flags, &mut out);

    for (i, chunk) in chunks.iter().enumerate() {
        let mut scalar = ChunkState::new(key, start_counter + (i as u64), flags);
        scalar.update(chunk);
        let expected = scalar.output().chaining_value();
        assert_eq!(
            out[i], expected,
            "64-chunk large batch mismatch at chunk {}",
            i
        );
    }
}

// ============================================================================
// 9. Throughput Benchmark: NEON 4-Way vs Scalar Acceleration Ratio
// ============================================================================

#[test]
fn test_neon_throughput_speedup_benchmark() {
    let key = IV;
    let flags = 0u8;
    let num_chunks = 128; // 128 KB of test payload
    let iterations = 600;

    let mut chunks = Vec::with_capacity(num_chunks);
    for i in 0..num_chunks {
        let mut chunk = [0u8; 1024];
        for (b_idx, b) in chunk.iter_mut().enumerate() {
            *b = ((i * 13 + b_idx * 17) % 251) as u8;
        }
        chunks.push(chunk);
    }
    let chunk_refs: Vec<&[u8; 1024]> = chunks.iter().collect();

    // 1. Symmetric Warm-up
    let mut out_neon = vec![[0u8; 32]; num_chunks];
    let mut out_scalar = vec![[0u8; 32]; num_chunks];
    for _ in 0..20 {
        hash_many_neon(&chunk_refs, &key, 0, flags, &mut out_neon);
        for (i, chunk) in chunks.iter().enumerate() {
            let mut state = ChunkState::new(key, i as u64, flags);
            state.update(chunk);
            out_scalar[i] = state.output().chaining_value();
        }
    }

    // 2. Measure NEON 4-Way SIMD throughput (best-of-3 runs)
    let mut min_neon_elapsed = std::time::Duration::from_secs(100);
    for _ in 0..3 {
        let start_neon = Instant::now();
        for iter in 0..iterations {
            hash_many_neon(
                &chunk_refs,
                &key,
                (iter * num_chunks) as u64,
                flags,
                &mut out_neon,
            );
        }
        let elapsed = start_neon.elapsed();
        if elapsed < min_neon_elapsed {
            min_neon_elapsed = elapsed;
        }
    }
    let elapsed_neon = min_neon_elapsed;

    // 3. Measure Scalar sequential throughput (best-of-3 runs)
    let mut min_scalar_elapsed = std::time::Duration::from_secs(100);
    for _ in 0..3 {
        let start_scalar = Instant::now();
        for iter in 0..iterations {
            let base_counter = (iter * num_chunks) as u64;
            for (i, chunk) in chunks.iter().enumerate() {
                let mut state = ChunkState::new(key, base_counter + (i as u64), flags);
                state.update(chunk);
                out_scalar[i] = state.output().chaining_value();
            }
        }
        let elapsed = start_scalar.elapsed();
        if elapsed < min_scalar_elapsed {
            min_scalar_elapsed = elapsed;
        }
    }
    let elapsed_scalar = min_scalar_elapsed;

    let total_bytes = (num_chunks * 1024 * iterations) as f64;
    let neon_mb_s = (total_bytes / 1_000_000.0) / elapsed_neon.as_secs_f64();
    let scalar_mb_s = (total_bytes / 1_000_000.0) / elapsed_scalar.as_secs_f64();
    let speedup = elapsed_scalar.as_secs_f64() / elapsed_neon.as_secs_f64();

    println!(
        "\n================ BLAKE3 4-Way NEON Benchmark ================\n\
         Payload: {} chunks ({} KB) x {} iterations = {:.2} MB\n\
         Scalar Throughput : {:.2} MB/s ({:?})\n\
         NEON 4-Way SIMD  : {:.2} MB/s ({:?})\n\
         Speedup Ratio    : {:.2}x\n\
         =============================================================",
        num_chunks,
        num_chunks,
        iterations,
        total_bytes / (1024.0 * 1024.0),
        scalar_mb_s,
        elapsed_scalar,
        neon_mb_s,
        elapsed_neon,
        speedup
    );

    // On AArch64 (Apple Silicon), 4-way NEON must demonstrate strong vector speedup
    #[cfg(target_arch = "aarch64")]
    {
        assert!(
            speedup >= 1.5,
            "NEON speedup ({:.2}x) fell below target threshold",
            speedup
        );
    }
}
