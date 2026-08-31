// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! ARM NEON / Apple Silicon 4-Way hardware-vectorized BLAKE3 compression kernel.
//!
//! Provides SIMD-accelerated 4-way parallel chunk and parent node compression for ARMv8-A /
//! Apple Silicon architectures utilizing 16 128-bit vector registers (`uint32x4_t`), fast
//! in-register Quarter-Round rotations (`vrev32q_u16`, `vsriq_n_u32`), 2-level butterfly network
//! matrix transposition (`vtrnq_u32` + `vcombine_u32`), and safe portable fallback for non-AArch64 targets.

#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::*;

use super::chunk::ChunkState;
#[cfg(not(target_arch = "aarch64"))]
use super::compress::compress_in_place_mut;
use super::compress::{counter_high, counter_low};
use super::constants::{BLOCK_LEN, CHUNK_END, CHUNK_LEN, CHUNK_START, IV, MSG_SCHEDULE, PARENT};
use super::tree::parent_cv;

// ============================================================================
// 1. Native AArch64 ARM NEON 4-Way SIMD Implementation
// ============================================================================

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn loadu_128(src: *const u8) -> uint32x4_t {
    vreinterpretq_u32_u8(vld1q_u8(src))
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn storeu_128(src: uint32x4_t, dest: *mut u8) {
    vst1q_u8(dest, vreinterpretq_u8_u32(src));
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn add_128(a: uint32x4_t, b: uint32x4_t) -> uint32x4_t {
    vaddq_u32(a, b)
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn xor_128(a: uint32x4_t, b: uint32x4_t) -> uint32x4_t {
    veorq_u32(a, b)
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn set1_128(x: u32) -> uint32x4_t {
    vdupq_n_u32(x)
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn set4(a: u32, b: u32, c: u32, d: u32) -> uint32x4_t {
    let array = [a, b, c, d];
    vld1q_u32(array.as_ptr())
}

/// Rotates each 32-bit lane right by 16 bits using 16-bit lane reversal (`vrev32q_u16`).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub unsafe fn rot16_128(x: uint32x4_t) -> uint32x4_t {
    vreinterpretq_u32_u16(vrev32q_u16(vreinterpretq_u16_u32(x)))
}

/// Rotates each 32-bit lane right by 12 bits using shift-right-and-insert (`vsriq_n_u32`).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub unsafe fn rot12_128(x: uint32x4_t) -> uint32x4_t {
    vsriq_n_u32(vshlq_n_u32(x, 20), x, 12)
}

/// Rotates each 32-bit lane right by 8 bits using shift-right-and-insert (`vsriq_n_u32`).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub unsafe fn rot8_128(x: uint32x4_t) -> uint32x4_t {
    vsriq_n_u32(vshlq_n_u32(x, 24), x, 8)
}

/// Rotates each 32-bit lane right by 7 bits using shift-right-and-insert (`vsriq_n_u32`).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub unsafe fn rot7_128(x: uint32x4_t) -> uint32x4_t {
    vsriq_n_u32(vshlq_n_u32(x, 25), x, 7)
}

/// Executes one round of 4-way parallel BLAKE3 permutation across 16 NEON vector registers.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub unsafe fn round_fn4(v: &mut [uint32x4_t; 16], m: &[uint32x4_t; 16], r: usize) {
    let schedule = &MSG_SCHEDULE[r];

    // Column mixing step
    v[0] = add_128(v[0], m[schedule[0]]);
    v[1] = add_128(v[1], m[schedule[2]]);
    v[2] = add_128(v[2], m[schedule[4]]);
    v[3] = add_128(v[3], m[schedule[6]]);
    v[0] = add_128(v[0], v[4]);
    v[1] = add_128(v[1], v[5]);
    v[2] = add_128(v[2], v[6]);
    v[3] = add_128(v[3], v[7]);
    v[12] = xor_128(v[12], v[0]);
    v[13] = xor_128(v[13], v[1]);
    v[14] = xor_128(v[14], v[2]);
    v[15] = xor_128(v[15], v[3]);
    v[12] = rot16_128(v[12]);
    v[13] = rot16_128(v[13]);
    v[14] = rot16_128(v[14]);
    v[15] = rot16_128(v[15]);
    v[8] = add_128(v[8], v[12]);
    v[9] = add_128(v[9], v[13]);
    v[10] = add_128(v[10], v[14]);
    v[11] = add_128(v[11], v[15]);
    v[4] = xor_128(v[4], v[8]);
    v[5] = xor_128(v[5], v[9]);
    v[6] = xor_128(v[6], v[10]);
    v[7] = xor_128(v[7], v[11]);
    v[4] = rot12_128(v[4]);
    v[5] = rot12_128(v[5]);
    v[6] = rot12_128(v[6]);
    v[7] = rot12_128(v[7]);
    v[0] = add_128(v[0], m[schedule[1]]);
    v[1] = add_128(v[1], m[schedule[3]]);
    v[2] = add_128(v[2], m[schedule[5]]);
    v[3] = add_128(v[3], m[schedule[7]]);
    v[0] = add_128(v[0], v[4]);
    v[1] = add_128(v[1], v[5]);
    v[2] = add_128(v[2], v[6]);
    v[3] = add_128(v[3], v[7]);
    v[12] = xor_128(v[12], v[0]);
    v[13] = xor_128(v[13], v[1]);
    v[14] = xor_128(v[14], v[2]);
    v[15] = xor_128(v[15], v[3]);
    v[12] = rot8_128(v[12]);
    v[13] = rot8_128(v[13]);
    v[14] = rot8_128(v[14]);
    v[15] = rot8_128(v[15]);
    v[8] = add_128(v[8], v[12]);
    v[9] = add_128(v[9], v[13]);
    v[10] = add_128(v[10], v[14]);
    v[11] = add_128(v[11], v[15]);
    v[4] = xor_128(v[4], v[8]);
    v[5] = xor_128(v[5], v[9]);
    v[6] = xor_128(v[6], v[10]);
    v[7] = xor_128(v[7], v[11]);
    v[4] = rot7_128(v[4]);
    v[5] = rot7_128(v[5]);
    v[6] = rot7_128(v[6]);
    v[7] = rot7_128(v[7]);

    // Diagonal mixing step
    v[0] = add_128(v[0], m[schedule[8]]);
    v[1] = add_128(v[1], m[schedule[10]]);
    v[2] = add_128(v[2], m[schedule[12]]);
    v[3] = add_128(v[3], m[schedule[14]]);
    v[0] = add_128(v[0], v[5]);
    v[1] = add_128(v[1], v[6]);
    v[2] = add_128(v[2], v[7]);
    v[3] = add_128(v[3], v[4]);
    v[15] = xor_128(v[15], v[0]);
    v[12] = xor_128(v[12], v[1]);
    v[13] = xor_128(v[13], v[2]);
    v[14] = xor_128(v[14], v[3]);
    v[15] = rot16_128(v[15]);
    v[12] = rot16_128(v[12]);
    v[13] = rot16_128(v[13]);
    v[14] = rot16_128(v[14]);
    v[10] = add_128(v[10], v[15]);
    v[11] = add_128(v[11], v[12]);
    v[8] = add_128(v[8], v[13]);
    v[9] = add_128(v[9], v[14]);
    v[5] = xor_128(v[5], v[10]);
    v[6] = xor_128(v[6], v[11]);
    v[7] = xor_128(v[7], v[8]);
    v[4] = xor_128(v[4], v[9]);
    v[5] = rot12_128(v[5]);
    v[6] = rot12_128(v[6]);
    v[7] = rot12_128(v[7]);
    v[4] = rot12_128(v[4]);
    v[0] = add_128(v[0], m[schedule[9]]);
    v[1] = add_128(v[1], m[schedule[11]]);
    v[2] = add_128(v[2], m[schedule[13]]);
    v[3] = add_128(v[3], m[schedule[15]]);
    v[0] = add_128(v[0], v[5]);
    v[1] = add_128(v[1], v[6]);
    v[2] = add_128(v[2], v[7]);
    v[3] = add_128(v[3], v[4]);
    v[15] = xor_128(v[15], v[0]);
    v[12] = xor_128(v[12], v[1]);
    v[13] = xor_128(v[13], v[2]);
    v[14] = xor_128(v[14], v[3]);
    v[15] = rot8_128(v[15]);
    v[12] = rot8_128(v[12]);
    v[13] = rot8_128(v[13]);
    v[14] = rot8_128(v[14]);
    v[10] = add_128(v[10], v[15]);
    v[11] = add_128(v[11], v[12]);
    v[8] = add_128(v[8], v[13]);
    v[9] = add_128(v[9], v[14]);
    v[5] = xor_128(v[5], v[10]);
    v[6] = xor_128(v[6], v[11]);
    v[7] = xor_128(v[7], v[8]);
    v[4] = xor_128(v[4], v[9]);
    v[5] = rot7_128(v[5]);
    v[6] = rot7_128(v[6]);
    v[7] = rot7_128(v[7]);
    v[4] = rot7_128(v[4]);
}

/// 2-level 4x4 32-bit butterfly network matrix transposition (`vtrnq_u32` + `vcombine_u32`).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub unsafe fn transpose_vecs_128(vecs: &mut [uint32x4_t; 4]) {
    let rows01 = vtrnq_u32(vecs[0], vecs[1]);
    let rows23 = vtrnq_u32(vecs[2], vecs[3]);

    vecs[0] = vcombine_u32(vget_low_u32(rows01.0), vget_low_u32(rows23.0));
    vecs[1] = vcombine_u32(vget_low_u32(rows01.1), vget_low_u32(rows23.1));
    vecs[2] = vcombine_u32(vget_high_u32(rows01.0), vget_high_u32(rows23.0));
    vecs[3] = vcombine_u32(vget_high_u32(rows01.1), vget_high_u32(rows23.1));
}

/// Transposes 4 64-byte input blocks into 16 `uint32x4_t` message vector registers.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn transpose_msg_vecs4(inputs: [&[u8]; 4], block_offset: usize, out: &mut [uint32x4_t; 16]) {
    for i in 0..4 {
        let base_idx = i * 4;
        let offset = block_offset + i * 16;
        for lane in 0..4 {
            out[base_idx + lane] = loadu_128(inputs[lane].as_ptr().add(offset));
        }
        let mut group = [
            out[base_idx],
            out[base_idx + 1],
            out[base_idx + 2],
            out[base_idx + 3],
        ];
        transpose_vecs_128(&mut group);
        out[base_idx..base_idx + 4].copy_from_slice(&group);
    }
}

/// Loads 64-bit counters into low and high 32-bit SIMD registers for 4 streams.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn load_counters4(counter: u64, increment_counter: bool) -> (uint32x4_t, uint32x4_t) {
    let inc = if increment_counter { 1 } else { 0 };
    let c0 = counter;
    let c1 = counter.wrapping_add(inc);
    let c2 = counter.wrapping_add(inc * 2);
    let c3 = counter.wrapping_add(inc * 3);

    let out_low = set4(
        counter_low(c0),
        counter_low(c1),
        counter_low(c2),
        counter_low(c3),
    );
    let out_high = set4(
        counter_high(c0),
        counter_high(c1),
        counter_high(c2),
        counter_high(c3),
    );
    (out_low, out_high)
}

/// Concurrently processes 4 input streams of `blocks` 64-byte blocks using ARM NEON SIMD.
#[cfg(target_arch = "aarch64")]
#[allow(clippy::too_many_arguments)]
pub fn hash4_neon(
    inputs: [&[u8]; 4],
    blocks: usize,
    key: &[u32; 8],
    counter: u64,
    increment_counter: bool,
    flags: u8,
    flags_start: u8,
    flags_end: u8,
    out: &mut [[u8; 32]; 4],
) {
    for input in &inputs {
        debug_assert!(
            input.len() >= blocks * BLOCK_LEN,
            "Input slice length must be at least blocks * 64"
        );
    }

    unsafe {
        let mut h_vecs = [
            set1_128(key[0]),
            set1_128(key[1]),
            set1_128(key[2]),
            set1_128(key[3]),
            set1_128(key[4]),
            set1_128(key[5]),
            set1_128(key[6]),
            set1_128(key[7]),
        ];

        let (counter_low_vec, counter_high_vec) = load_counters4(counter, increment_counter);
        let mut block_flags = flags | flags_start;

        for block in 0..blocks {
            if block + 1 == blocks {
                block_flags |= flags_end;
            }

            let block_len_vec = set1_128(BLOCK_LEN as u32);
            let block_flags_vec = set1_128(block_flags as u32);
            let mut msg_vecs = [set1_128(0); 16];
            transpose_msg_vecs4(inputs, block * BLOCK_LEN, &mut msg_vecs);

            let mut v = [
                h_vecs[0],
                h_vecs[1],
                h_vecs[2],
                h_vecs[3],
                h_vecs[4],
                h_vecs[5],
                h_vecs[6],
                h_vecs[7],
                set1_128(IV[0]),
                set1_128(IV[1]),
                set1_128(IV[2]),
                set1_128(IV[3]),
                counter_low_vec,
                counter_high_vec,
                block_len_vec,
                block_flags_vec,
            ];

            round_fn4(&mut v, &msg_vecs, 0);
            round_fn4(&mut v, &msg_vecs, 1);
            round_fn4(&mut v, &msg_vecs, 2);
            round_fn4(&mut v, &msg_vecs, 3);
            round_fn4(&mut v, &msg_vecs, 4);
            round_fn4(&mut v, &msg_vecs, 5);
            round_fn4(&mut v, &msg_vecs, 6);

            h_vecs[0] = xor_128(v[0], v[8]);
            h_vecs[1] = xor_128(v[1], v[9]);
            h_vecs[2] = xor_128(v[2], v[10]);
            h_vecs[3] = xor_128(v[3], v[11]);
            h_vecs[4] = xor_128(v[4], v[12]);
            h_vecs[5] = xor_128(v[5], v[13]);
            h_vecs[6] = xor_128(v[6], v[14]);
            h_vecs[7] = xor_128(v[7], v[15]);

            block_flags = flags;
        }

        let mut h_low = [h_vecs[0], h_vecs[1], h_vecs[2], h_vecs[3]];
        let mut h_high = [h_vecs[4], h_vecs[5], h_vecs[6], h_vecs[7]];
        transpose_vecs_128(&mut h_low);
        transpose_vecs_128(&mut h_high);

        for lane in 0..4 {
            storeu_128(h_low[lane], out[lane].as_mut_ptr());
            storeu_128(h_high[lane], out[lane].as_mut_ptr().add(16));
        }
    }
}

// ============================================================================
// 2. Portable Fallback for Non-AArch64 Architectures
// ============================================================================

#[cfg(not(target_arch = "aarch64"))]
pub fn hash4_neon(
    inputs: [&[u8]; 4],
    blocks: usize,
    key: &[u32; 8],
    mut counter: u64,
    increment_counter: bool,
    flags: u8,
    flags_start: u8,
    flags_end: u8,
    out: &mut [[u8; 32]; 4],
) {
    for lane in 0..4 {
        let mut cv = *key;
        let mut block_flags = flags | flags_start;
        for block_idx in 0..blocks {
            if block_idx + 1 == blocks {
                block_flags |= flags_end;
            }
            let block_slice: &[u8; 64] = inputs[lane]
                [block_idx * BLOCK_LEN..(block_idx + 1) * BLOCK_LEN]
                .try_into()
                .expect("64-byte block slice");
            compress_in_place_mut(
                &mut cv,
                block_slice,
                BLOCK_LEN as u8,
                counter,
                block_flags,
            );
            block_flags = flags;
        }
        for i in 0..8 {
            out[lane][i * 4..(i + 1) * 4].copy_from_slice(&cv[i].to_le_bytes());
        }
        if increment_counter {
            counter += 1;
        }
    }
}

// ============================================================================
// 3. High-Level 4-Way Vectorized Tree & Chunk Hashing Facades
// ============================================================================

/// Vectorized 4-way concurrent compression of 4 parent nodes.
///
/// Each parent block contains 64 bytes (`left_child[32]` || `right_child[32]`).
#[inline]
pub fn hash_parents_neon(
    parents: [&[u8; 64]; 4],
    key: &[u32; 8],
    flags: u8,
    out: &mut [[u8; 32]; 4],
) {
    let inputs: [&[u8]; 4] = [
        parents[0].as_slice(),
        parents[1].as_slice(),
        parents[2].as_slice(),
        parents[3].as_slice(),
    ];
    hash4_neon(
        inputs,
        1,
        key,
        0,
        false,
        flags | PARENT,
        0,
        0,
        out,
    );
}

/// Batch vector compression for an arbitrary number of parent nodes.
///
/// Groups parent nodes into 4-way SIMD batches, falling back to scalar `parent_cv`
/// for remaining trailing nodes (< 4).
pub fn hash_many_parents_neon(
    parents: &[[u8; 64]],
    key: &[u32; 8],
    flags: u8,
    out: &mut [[u8; 32]],
) {
    assert!(
        out.len() >= parents.len(),
        "Output slice must be at least as large as input parents slice"
    );

    let mut parent_idx = 0;
    while parent_idx + 4 <= parents.len() {
        let parent_ptrs: [&[u8; 64]; 4] = [
            &parents[parent_idx],
            &parents[parent_idx + 1],
            &parents[parent_idx + 2],
            &parents[parent_idx + 3],
        ];
        let mut out4 = [[0u8; 32]; 4];
        hash_parents_neon(parent_ptrs, key, flags, &mut out4);
        out[parent_idx..parent_idx + 4].copy_from_slice(&out4);
        parent_idx += 4;
    }

    while parent_idx < parents.len() {
        let left: &[u8; 32] = parents[parent_idx][..32]
            .try_into()
            .expect("32-byte left child");
        let right: &[u8; 32] = parents[parent_idx][32..]
            .try_into()
            .expect("32-byte right child");
        out[parent_idx] = parent_cv(left, right, key, flags);
        parent_idx += 1;
    }
}

/// High-throughput vector scheduling facade for batch hashing 1024-byte chunks.
///
/// Processes full 1024-byte chunks in 4-way SIMD groups (4 KB per vector pass),
/// automatically handling sequential chunk counters and applying fallback for trailing chunks.
pub fn hash_many_neon(
    inputs: &[&[u8; 1024]],
    key: &[u32; 8],
    mut start_counter: u64,
    flags: u8,
    out: &mut [[u8; 32]],
) {
    assert!(
        out.len() >= inputs.len(),
        "Output slice must be at least as large as inputs slice"
    );

    let blocks_per_chunk = CHUNK_LEN / BLOCK_LEN; // 16 blocks per 1024B chunk
    let mut chunk_idx = 0;

    while chunk_idx + 4 <= inputs.len() {
        let input_slices: [&[u8]; 4] = [
            inputs[chunk_idx].as_slice(),
            inputs[chunk_idx + 1].as_slice(),
            inputs[chunk_idx + 2].as_slice(),
            inputs[chunk_idx + 3].as_slice(),
        ];
        let mut out4 = [[0u8; 32]; 4];
        hash4_neon(
            input_slices,
            blocks_per_chunk,
            key,
            start_counter,
            true,
            flags,
            CHUNK_START,
            CHUNK_END,
            &mut out4,
        );
        out[chunk_idx..chunk_idx + 4].copy_from_slice(&out4);

        start_counter += 4;
        chunk_idx += 4;
    }

    while chunk_idx < inputs.len() {
        let mut chunk_state = ChunkState::new(*key, start_counter, flags);
        chunk_state.update(inputs[chunk_idx].as_slice());
        out[chunk_idx] = chunk_state.output().chaining_value();

        start_counter += 1;
        chunk_idx += 1;
    }
}

/// Convenience helper for hashing an arbitrary array of chunks with varying byte lengths.
pub fn hash_many_variable_chunks(
    inputs: &[&[u8]],
    key: &[u32; 8],
    mut start_counter: u64,
    flags: u8,
    out: &mut [[u8; 32]],
) {
    assert!(
        out.len() >= inputs.len(),
        "Output slice must be at least as large as inputs slice"
    );

    let mut chunk_idx = 0;
    while chunk_idx + 4 <= inputs.len() {
        let all_1024 = inputs[chunk_idx].len() == CHUNK_LEN
            && inputs[chunk_idx + 1].len() == CHUNK_LEN
            && inputs[chunk_idx + 2].len() == CHUNK_LEN
            && inputs[chunk_idx + 3].len() == CHUNK_LEN;

        if all_1024 {
            let input_slices: [&[u8]; 4] = [
                inputs[chunk_idx],
                inputs[chunk_idx + 1],
                inputs[chunk_idx + 2],
                inputs[chunk_idx + 3],
            ];
            let mut out4 = [[0u8; 32]; 4];
            hash4_neon(
                input_slices,
                16,
                key,
                start_counter,
                true,
                flags,
                CHUNK_START,
                CHUNK_END,
                &mut out4,
            );
            out[chunk_idx..chunk_idx + 4].copy_from_slice(&out4);
            start_counter += 4;
            chunk_idx += 4;
            continue;
        }

        // Process single chunk with ChunkState
        let mut chunk_state = ChunkState::new(*key, start_counter, flags);
        chunk_state.update(inputs[chunk_idx]);
        out[chunk_idx] = chunk_state.output().chaining_value();
        start_counter += 1;
        chunk_idx += 1;
    }

    while chunk_idx < inputs.len() {
        let mut chunk_state = ChunkState::new(*key, start_counter, flags);
        chunk_state.update(inputs[chunk_idx]);
        out[chunk_idx] = chunk_state.output().chaining_value();
        start_counter += 1;
        chunk_idx += 1;
    }
}
