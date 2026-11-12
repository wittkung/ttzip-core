// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Scalar block compression routine for SHA-1 (RFC 3174).

use zeroize::Zeroize;

pub const SHA1_INITIAL_H: [u32; 5] = [
    0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0,
];

#[inline(always)]
pub fn rol(val: u32, bits: u32) -> u32 {
    val.rotate_left(bits)
}

#[inline(always)]
fn expand_sha1_schedule(block: &[u8; 64]) -> [u32; 80] {
    let mut w = [0u32; 80];
    for i in 0..16 {
        w[i] = u32::from_be_bytes([
            block[i * 4],
            block[i * 4 + 1],
            block[i * 4 + 2],
            block[i * 4 + 3],
        ]);
    }
    for i in 16..80 {
        w[i] = rol(w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16], 1);
    }
    w
}

#[inline(always)]
fn sha1_round_step(
    [a, b, c, d, e]: [u32; 5],
    f: u32,
    k: u32,
    w_i: u32,
) -> [u32; 5] {
    let temp = rol(a, 5)
        .wrapping_add(f)
        .wrapping_add(e)
        .wrapping_add(k)
        .wrapping_add(w_i);
    [temp, a, rol(b, 30), c, d]
}

#[inline(always)]
pub fn sha1_compress_block(state: &mut [u32; 5], block: &[u8; 64]) {
    let mut w = expand_sha1_schedule(block);
    let mut v = *state;

    for i in 0..20 {
        v = sha1_round_step(v, (v[1] & v[2]) | ((!v[1]) & v[3]), 0x5A827999, w[i]);
    }
    for i in 20..40 {
        v = sha1_round_step(v, v[1] ^ v[2] ^ v[3], 0x6ED9EBA1, w[i]);
    }
    for i in 40..60 {
        v = sha1_round_step(v, (v[1] & v[2]) | (v[1] & v[3]) | (v[2] & v[3]), 0x8F1BBCDC, w[i]);
    }
    for i in 60..80 {
        v = sha1_round_step(v, v[1] ^ v[2] ^ v[3], 0xCA62C1D6, w[i]);
    }

    for i in 0..5 {
        state[i] = state[i].wrapping_add(v[i]);
    }

    w.zeroize();
}
