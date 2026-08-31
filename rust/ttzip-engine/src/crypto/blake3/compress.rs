// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! BLAKE3 7-round Quarter-Round compression function and mixing core.

use super::constants::{IV, MSG_SCHEDULE};

/// Converts 64 little-endian bytes into 16 32-bit words without allocations.
#[inline(always)]
pub fn words_from_le_bytes_64(bytes: &[u8; 64]) -> [u32; 16] {
    let mut out = [0u32; 16];
    for i in 0..16 {
        let off = i * 4;
        out[i] = u32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]]);
    }
    out
}

/// Converts 32 little-endian bytes into 8 32-bit words without allocations.
#[inline(always)]
pub fn words_from_le_bytes_32(bytes: &[u8; 32]) -> [u32; 8] {
    let mut out = [0u32; 8];
    for i in 0..8 {
        let off = i * 4;
        out[i] = u32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]]);
    }
    out
}

/// Converts 8 32-bit words into 32 little-endian bytes.
#[inline(always)]
pub fn le_bytes_from_words_32(words: &[u32; 8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for i in 0..8 {
        let b = words[i].to_le_bytes();
        out[i * 4..i * 4 + 4].copy_from_slice(&b);
    }
    out
}

/// Converts 16 32-bit words into 64 little-endian bytes.
#[inline(always)]
pub fn le_bytes_from_words_64(words: &[u32; 16]) -> [u8; 64] {
    let mut out = [0u8; 64];
    for i in 0..16 {
        let b = words[i].to_le_bytes();
        out[i * 4..i * 4 + 4].copy_from_slice(&b);
    }
    out
}

/// Helper function to extract low 32 bits of a 64-bit counter.
#[inline(always)]
pub const fn counter_low(counter: u64) -> u32 {
    counter as u32
}

/// Helper function to extract high 32 bits of a 64-bit counter.
#[inline(always)]
pub const fn counter_high(counter: u64) -> u32 {
    (counter >> 32) as u32
}

/// BLAKE3 Quarter-Round mixing function $G$.
///
/// Operates on 4 state words (`a`, `b`, `c`, `d`) with two message words (`mx`, `my`)
/// using 4 rotation constants: 16, 12, 8, 7.
#[inline(always)]
pub fn g(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize, mx: u32, my: u32) {
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(mx);
    state[d] = (state[d] ^ state[a]).rotate_right(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(12);
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(my);
    state[d] = (state[d] ^ state[a]).rotate_right(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(7);
}

/// Executes one round of the BLAKE3 compression function.
///
/// Applies column mixing followed by diagonal mixing according to the permutation schedule.
#[inline(always)]
pub fn round_fn(state: &mut [u32; 16], msg: &[u32; 16], round: usize) {
    let schedule = &MSG_SCHEDULE[round];

    // Mix columns
    g(state, 0, 4, 8, 12, msg[schedule[0]], msg[schedule[1]]);
    g(state, 1, 5, 9, 13, msg[schedule[2]], msg[schedule[3]]);
    g(state, 2, 6, 10, 14, msg[schedule[4]], msg[schedule[5]]);
    g(state, 3, 7, 11, 15, msg[schedule[6]], msg[schedule[7]]);

    // Mix diagonals
    g(state, 0, 5, 10, 15, msg[schedule[8]], msg[schedule[9]]);
    g(state, 1, 6, 11, 12, msg[schedule[10]], msg[schedule[11]]);
    g(state, 2, 7, 8, 13, msg[schedule[12]], msg[schedule[13]]);
    g(state, 3, 4, 9, 14, msg[schedule[14]], msg[schedule[15]]);
}

/// Pre-compression helper: initializes 16-word state and runs 7 permutation rounds.
#[inline(always)]
pub fn compress_pre(
    cv: &[u32; 8],
    block: &[u8; 64],
    block_len: u8,
    counter: u64,
    flags: u8,
) -> [u32; 16] {
    let block_words = words_from_le_bytes_64(block);
    compress_pre_words(cv, &block_words, block_len, counter, flags)
}

/// Pre-compression helper accepting pre-decoded message words.
#[inline(always)]
pub fn compress_pre_words(
    cv: &[u32; 8],
    block_words: &[u32; 16],
    block_len: u8,
    counter: u64,
    flags: u8,
) -> [u32; 16] {
    let mut state = [
        cv[0],
        cv[1],
        cv[2],
        cv[3],
        cv[4],
        cv[5],
        cv[6],
        cv[7],
        IV[0],
        IV[1],
        IV[2],
        IV[3],
        counter_low(counter),
        counter_high(counter),
        block_len as u32,
        flags as u32,
    ];

    round_fn(&mut state, block_words, 0);
    round_fn(&mut state, block_words, 1);
    round_fn(&mut state, block_words, 2);
    round_fn(&mut state, block_words, 3);
    round_fn(&mut state, block_words, 4);
    round_fn(&mut state, block_words, 5);
    round_fn(&mut state, block_words, 6);

    state
}

/// Standard in-place single block compression function returning the updated 8-word chaining value.
///
/// Applies feed-forward XOR: `cv[i] = state[i] ^ state[i + 8]`.
#[inline(always)]
pub fn compress_in_place(
    cv: &[u32; 8],
    block: &[u8; 64],
    block_len: u8,
    counter: u64,
    flags: u8,
) -> [u32; 8] {
    let state = compress_pre(cv, block, block_len, counter, flags);
    [
        state[0] ^ state[8],
        state[1] ^ state[9],
        state[2] ^ state[10],
        state[3] ^ state[11],
        state[4] ^ state[12],
        state[5] ^ state[13],
        state[6] ^ state[14],
        state[7] ^ state[15],
    ]
}

/// In-place block compression mutating the given chaining value in place.
#[inline(always)]
pub fn compress_in_place_mut(
    cv: &mut [u32; 8],
    block: &[u8; 64],
    block_len: u8,
    counter: u64,
    flags: u8,
) {
    let state = compress_pre(cv, block, block_len, counter, flags);
    cv[0] = state[0] ^ state[8];
    cv[1] = state[1] ^ state[9];
    cv[2] = state[2] ^ state[10];
    cv[3] = state[3] ^ state[11];
    cv[4] = state[4] ^ state[12];
    cv[5] = state[5] ^ state[13];
    cv[6] = state[6] ^ state[14];
    cv[7] = state[7] ^ state[15];
}

/// Extended Output Function (XOF) single block compression producing 64 bytes of output.
///
/// Applies full feed-forward XOR:
/// `out[0..8] = state[0..8] ^ state[8..16]`
/// `out[8..16] = state[8..16] ^ cv[0..8]`
#[inline(always)]
pub fn compress_xof(
    cv: &[u32; 8],
    block: &[u8; 64],
    block_len: u8,
    counter: u64,
    flags: u8,
) -> [u8; 64] {
    let mut state = compress_pre(cv, block, block_len, counter, flags);
    state[0] ^= state[8];
    state[1] ^= state[9];
    state[2] ^= state[10];
    state[3] ^= state[11];
    state[4] ^= state[12];
    state[5] ^= state[13];
    state[6] ^= state[14];
    state[7] ^= state[15];
    state[8] ^= cv[0];
    state[9] ^= cv[1];
    state[10] ^= cv[2];
    state[11] ^= cv[3];
    state[12] ^= cv[4];
    state[13] ^= cv[5];
    state[14] ^= cv[6];
    state[15] ^= cv[7];
    le_bytes_from_words_64(&state)
}

/// Extended Output Function (XOF) accepting pre-decoded message words, returning 16 32-bit words.
#[inline(always)]
pub fn compress_xof_words(
    cv: &[u32; 8],
    block_words: &[u32; 16],
    block_len: u8,
    counter: u64,
    flags: u8,
) -> [u32; 16] {
    let mut state = compress_pre_words(cv, block_words, block_len, counter, flags);
    state[0] ^= state[8];
    state[1] ^= state[9];
    state[2] ^= state[10];
    state[3] ^= state[11];
    state[4] ^= state[12];
    state[5] ^= state[13];
    state[6] ^= state[14];
    state[7] ^= state[15];
    state[8] ^= cv[0];
    state[9] ^= cv[1];
    state[10] ^= cv[2];
    state[11] ^= cv[3];
    state[12] ^= cv[4];
    state[13] ^= cv[5];
    state[14] ^= cv[6];
    state[15] ^= cv[7];
    state
}
