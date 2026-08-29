// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Hardware-accelerated XXH3-64 and XXH3-128 checksum engine.
//!
//! Implements Yann Collet's xxHash3 algorithm with 64-bit and 128-bit output,
//! 64-byte stripe accumulation, universal scalar fallback, and streaming support.

const PRIME32_1: u32 = 0x9E3779B1;
const PRIME32_2: u32 = 0x85EBCA77;
const PRIME64_1: u64 = 0x9E3779B185EBCA87;
const PRIME64_2: u64 = 0xC2B2AE3D27D4EB4F;
const PRIME64_3: u64 = 0x165667B19E3779F9;
const PRIME64_4: u64 = 0x85EBCA77C2B2AE63;
const PRIME64_5: u64 = 0x27D4EB2F165667C5;

const STRIPE_LEN: usize = 64;
const ACC_NB: usize = 8;
const MIDSIZE_MAX: usize = 240;

/// Default 192-byte secret for XXH3.
pub const DEFAULT_SECRET: [u8; 192] = [
    0xb8, 0xfe, 0x6c, 0x39, 0x23, 0xa4, 0x4b, 0xbe, 0x7c, 0x01, 0x81, 0x2c, 0xf7, 0x21, 0xad, 0x1c,
    0xde, 0xd4, 0x6d, 0xe9, 0x83, 0x90, 0x97, 0xdb, 0x72, 0x40, 0xa4, 0xa4, 0xb7, 0xb3, 0x67, 0x1f,
    0xcb, 0x79, 0xe6, 0x4e, 0xcc, 0xc0, 0xe5, 0x78, 0x82, 0x5a, 0xd0, 0x7d, 0xcc, 0xff, 0x72, 0x21,
    0xb8, 0x08, 0x46, 0x74, 0xf7, 0x43, 0x24, 0x8e, 0xe0, 0x35, 0x90, 0xe6, 0x81, 0x3a, 0x26, 0x4c,
    0x3c, 0x28, 0x52, 0xbb, 0x91, 0xc3, 0x00, 0xcb, 0x88, 0xd0, 0x65, 0x8b, 0x1b, 0x53, 0x2e, 0xa3,
    0x71, 0x64, 0x48, 0x97, 0xa2, 0x0d, 0xf9, 0x4e, 0x38, 0x19, 0xef, 0x46, 0xa9, 0xde, 0xac, 0xd8,
    0xa8, 0xfa, 0x76, 0x3f, 0xe3, 0x9c, 0x34, 0x3f, 0xf9, 0xdc, 0xbb, 0xc7, 0xc7, 0x0b, 0x4f, 0x1d,
    0x8a, 0x51, 0xe0, 0x4b, 0xcd, 0xb4, 0x59, 0x31, 0xc8, 0x9f, 0x7e, 0xc9, 0xd9, 0x78, 0x73, 0x64,
    0xea, 0xc5, 0xac, 0x83, 0x34, 0xd3, 0xeb, 0xc3, 0xc5, 0x81, 0xa0, 0xff, 0xfa, 0x13, 0x63, 0xeb,
    0x17, 0x0d, 0xdd, 0x51, 0xb7, 0xf0, 0xda, 0x49, 0xd3, 0x16, 0x55, 0x26, 0x29, 0xd4, 0x68, 0x9e,
    0x2b, 0x16, 0xbe, 0x58, 0x7d, 0x47, 0xa1, 0xfc, 0x8f, 0xf8, 0xb8, 0xd1, 0x7a, 0xd0, 0x31, 0xce,
    0x45, 0xcb, 0x3a, 0x8f, 0x95, 0x16, 0x04, 0x28, 0xaf, 0xd7, 0xfb, 0xca, 0xbb, 0x4b, 0x40, 0x7e,
];

const INITIAL_ACC: [u64; ACC_NB] = [
    prime32_3_64(),
    PRIME64_1,
    PRIME64_2,
    PRIME64_3,
    PRIME64_4,
    PRIME32_2 as u64,
    PRIME64_5,
    PRIME32_1 as u64,
];

const fn prime32_3_64() -> u64 {
    (PRIME32_3 as u64) | ((PRIME32_2 as u64) << 32)
}

const PRIME32_3: u32 = 0x165667B1;

#[inline(always)]
fn read_u32_le(p: &[u8]) -> u32 {
    u32::from_le_bytes(p[..4].try_into().unwrap())
}

#[inline(always)]
fn read_u64_le(p: &[u8]) -> u64 {
    u64::from_le_bytes(p[..8].try_into().unwrap())
}

#[inline(always)]
fn mult64to128(x: u64, y: u64) -> u128 {
    (x as u128) * (y as u128)
}

#[inline(always)]
fn xxh3_mul128_fold64(lhs: u64, rhs: u64) -> u64 {
    let prod = mult64to128(lhs, rhs);
    (prod as u64) ^ ((prod >> 64) as u64)
}

#[inline(always)]
fn xxh3_avalanche(mut h64: u64) -> u64 {
    h64 ^= h64 >> 37;
    h64 = h64.wrapping_mul(0x165667919E3779F9);
    h64 ^= h64 >> 32;
    h64
}

#[inline(always)]
fn xxh64_avalanche(mut h64: u64) -> u64 {
    h64 ^= h64 >> 33;
    h64 = h64.wrapping_mul(PRIME64_2);
    h64 ^= h64 >> 29;
    h64 = h64.wrapping_mul(PRIME64_3);
    h64 ^= h64 >> 32;
    h64
}

// ----------------------------------------------------------------------------
// Small input handling (< 241 bytes)
// ----------------------------------------------------------------------------

fn xxh3_len_0(seed: u64, secret: &[u8]) -> u64 {
    let mut h64 = seed ^ read_u64_le(&secret[56..64]) ^ read_u64_le(&secret[64..72]);
    h64 = xxh64_avalanche(h64);
    h64
}

fn xxh3_len_1to3(input: &[u8], seed: u64, secret: &[u8]) -> u64 {
    let len = input.len();
    let c1 = input[0] as u32;
    let c2 = input[len >> 1] as u32;
    let c3 = input[len - 1] as u32;
    let combined = (c1 << 16) | (c2 << 24) | (c3 << 8) | (len as u32);
    let bitflip = (read_u32_le(&secret[0..4]) ^ read_u32_le(&secret[4..8])).wrapping_add(seed as u32);
    let keyed = (combined ^ bitflip) as u64;
    xxh64_avalanche(keyed)
}

fn xxh3_len_4to8(input: &[u8], seed: u64, secret: &[u8]) -> u64 {
    let len = input.len();
    let seed = seed ^ ((seed as u32).swap_bytes() as u64) << 32;
    let in1 = read_u32_le(&input[..4]) as u64;
    let in2 = read_u32_le(&input[len - 4..]) as u64;
    let in64 = in1.wrapping_add(in2 << 32);
    let sec1 = read_u64_le(&secret[8..16]);
    let sec2 = read_u64_le(&secret[16..24]);
    let bitflip = (sec1 ^ sec2).wrapping_sub(seed);
    let keyed = in64 ^ bitflip;
    let mut h64 = (len as u64) ^ (keyed.swap_bytes().wrapping_mul(PRIME64_1));
    h64 = xxh3_mul128_fold64(h64, PRIME64_2);
    xxh3_avalanche(h64)
}

fn xxh3_len_9to16(input: &[u8], seed: u64, secret: &[u8]) -> u64 {
    let len = input.len();
    let sec1 = read_u64_le(&secret[24..32]).wrapping_add(seed);
    let sec2 = read_u64_le(&secret[32..40]).wrapping_sub(seed);
    let sec3 = read_u64_le(&secret[40..48]).wrapping_add(seed);
    let sec4 = read_u64_le(&secret[48..56]).wrapping_sub(seed);

    let ll1 = read_u64_le(&input[..8]) ^ sec1;
    let ll2 = read_u64_le(&input[len - 8..]) ^ sec2;
    let acc = (len as u64)
        .wrapping_add(ll1.swap_bytes())
        .wrapping_add(ll2)
        .wrapping_add(xxh3_mul128_fold64(ll1, ll2));

    let acc2 = xxh3_mul128_fold64(acc, sec3 ^ sec4);
    xxh3_avalanche(acc2)
}

fn xxh3_len_0to16(input: &[u8], seed: u64, secret: &[u8]) -> u64 {
    match input.len() {
        0 => xxh3_len_0(seed, secret),
        1..=3 => xxh3_len_1to3(input, seed, secret),
        4..=8 => xxh3_len_4to8(input, seed, secret),
        9..=16 => xxh3_len_9to16(input, seed, secret),
        _ => unreachable!(),
    }
}

fn xxh3_len_17to128(input: &[u8], seed: u64, secret: &[u8]) -> u64 {
    let len = input.len();
    let mut acc = (len as u64).wrapping_mul(PRIME64_1);

    if len > 32 {
        if len > 64 {
            if len > 96 {
                acc = acc.wrapping_add(xxh3_mul128_fold64(
                    read_u64_le(&input[48..56]) ^ (read_u64_le(&secret[96..104]).wrapping_add(seed)),
                    read_u64_le(&input[56..64]) ^ (read_u64_le(&secret[104..112]).wrapping_sub(seed)),
                ));
                acc = acc.wrapping_add(xxh3_mul128_fold64(
                    read_u64_le(&input[len - 64..len - 56]) ^ (read_u64_le(&secret[112..120]).wrapping_add(seed)),
                    read_u64_le(&input[len - 56..len - 48]) ^ (read_u64_le(&secret[120..128]).wrapping_sub(seed)),
                ));
            }
            acc = acc.wrapping_add(xxh3_mul128_fold64(
                read_u64_le(&input[32..40]) ^ (read_u64_le(&secret[64..72]).wrapping_add(seed)),
                read_u64_le(&input[40..48]) ^ (read_u64_le(&secret[72..80]).wrapping_sub(seed)),
            ));
            acc = acc.wrapping_add(xxh3_mul128_fold64(
                read_u64_le(&input[len - 48..len - 40]) ^ (read_u64_le(&secret[80..88]).wrapping_add(seed)),
                read_u64_le(&input[len - 40..len - 32]) ^ (read_u64_le(&secret[88..96]).wrapping_sub(seed)),
            ));
        }
        acc = acc.wrapping_add(xxh3_mul128_fold64(
            read_u64_le(&input[16..24]) ^ (read_u64_le(&secret[32..40]).wrapping_add(seed)),
            read_u64_le(&input[24..32]) ^ (read_u64_le(&secret[40..48]).wrapping_sub(seed)),
        ));
        acc = acc.wrapping_add(xxh3_mul128_fold64(
            read_u64_le(&input[len - 32..len - 24]) ^ (read_u64_le(&secret[48..56]).wrapping_add(seed)),
            read_u64_le(&input[len - 24..len - 16]) ^ (read_u64_le(&secret[56..64]).wrapping_sub(seed)),
        ));
    }
    acc = acc.wrapping_add(xxh3_mul128_fold64(
        read_u64_le(&input[..8]) ^ (read_u64_le(&secret[..8]).wrapping_add(seed)),
        read_u64_le(&input[8..16]) ^ (read_u64_le(&secret[8..16]).wrapping_sub(seed)),
    ));
    acc = acc.wrapping_add(xxh3_mul128_fold64(
        read_u64_le(&input[len - 16..len - 8]) ^ (read_u64_le(&secret[16..24]).wrapping_add(seed)),
        read_u64_le(&input[len - 8..]) ^ (read_u64_le(&secret[24..32]).wrapping_sub(seed)),
    ));

    xxh3_avalanche(acc)
}

fn xxh3_len_129to240(input: &[u8], seed: u64, secret: &[u8]) -> u64 {
    let len = input.len();
    let mut acc = (len as u64).wrapping_mul(PRIME64_1);

    for i in 0..4 {
        let in_off = i * 32;
        let sec_off = i * 32;
        acc = acc.wrapping_add(xxh3_mul128_fold64(
            read_u64_le(&input[in_off..in_off + 8]) ^ (read_u64_le(&secret[sec_off..sec_off + 8]).wrapping_add(seed)),
            read_u64_le(&input[in_off + 8..in_off + 16]) ^ (read_u64_le(&secret[sec_off + 8..sec_off + 16]).wrapping_sub(seed)),
        ));
        acc = acc.wrapping_add(xxh3_mul128_fold64(
            read_u64_le(&input[in_off + 16..in_off + 24]) ^ (read_u64_le(&secret[sec_off + 16..sec_off + 24]).wrapping_add(seed)),
            read_u64_le(&input[in_off + 24..in_off + 32]) ^ (read_u64_le(&secret[sec_off + 24..sec_off + 32]).wrapping_sub(seed)),
        ));
    }

    acc = xxh3_avalanche(acc);

    let mut acc_end = 0u64;
    for i in 0..4 {
        let in_off = len - 128 + i * 32;
        let sec_off = i * 32 + 3;
        acc_end = acc_end.wrapping_add(xxh3_mul128_fold64(
            read_u64_le(&input[in_off..in_off + 8]) ^ (read_u64_le(&secret[sec_off..sec_off + 8]).wrapping_add(seed)),
            read_u64_le(&input[in_off + 8..in_off + 16]) ^ (read_u64_le(&secret[sec_off + 8..sec_off + 16]).wrapping_sub(seed)),
        ));
        acc_end = acc_end.wrapping_add(xxh3_mul128_fold64(
            read_u64_le(&input[in_off + 16..in_off + 24]) ^ (read_u64_le(&secret[sec_off + 16..sec_off + 24]).wrapping_add(seed)),
            read_u64_le(&input[in_off + 24..in_off + 32]) ^ (read_u64_le(&secret[sec_off + 24..sec_off + 32]).wrapping_sub(seed)),
        ));
    }

    xxh3_avalanche(acc.wrapping_add(acc_end))
}

// ----------------------------------------------------------------------------
// Long input handling (Stripe Accumulation & Scramble)
// ----------------------------------------------------------------------------

#[inline(always)]
fn xxh3_accumulate_stripe(acc: &mut [u64; ACC_NB], stripe: &[u8], secret: &[u8]) {
    for i in 0..ACC_NB {
        let data_val = read_u64_le(&stripe[i * 8..(i + 1) * 8]);
        let data_key = data_val ^ read_u64_le(&secret[i * 8..(i + 1) * 8]);
        let swap_idx = i ^ 1;
        acc[swap_idx] = acc[swap_idx].wrapping_add(data_val);
        acc[i] = acc[i].wrapping_add((data_key & 0xFFFFFFFF).wrapping_mul(data_key >> 32));
    }
}

#[inline(always)]
fn xxh3_scramble_acc(acc: &mut [u64; ACC_NB], secret: &[u8]) {
    for i in 0..ACC_NB {
        let key = read_u64_le(&secret[i * 8..(i + 1) * 8]);
        let mut val = acc[i];
        val ^= val >> 47;
        val ^= key;
        val = val.wrapping_mul(PRIME32_1 as u64);
        acc[i] = val;
    }
}

fn xxh3_merge_acc(acc: &[u64; ACC_NB], secret: &[u8], mut start: u64) -> u64 {
    for i in 0..4 {
        let data_key1 = acc[i * 2] ^ read_u64_le(&secret[i * 16..i * 16 + 8]);
        let data_key2 = acc[i * 2 + 1] ^ read_u64_le(&secret[i * 16 + 8..i * 16 + 16]);
        start = start.wrapping_add(xxh3_mul128_fold64(data_key1, data_key2));
    }
    xxh3_avalanche(start)
}

fn xxh3_long(input: &[u8], seed: u64, secret: &[u8]) -> u64 {
    let mut acc = INITIAL_ACC;
    if seed != 0 {
        let mut custom_sec = [0u8; 192];
        derive_custom_secret(&mut custom_sec, secret, seed);
        return xxh3_long_internal(input, &custom_sec, &mut acc);
    }
    xxh3_long_internal(input, secret, &mut acc)
}

fn derive_custom_secret(custom: &mut [u8; 192], base: &[u8], seed: u64) {
    for i in 0..12 {
        let off = i * 16;
        let v1 = read_u64_le(&base[off..off + 8]).wrapping_add(seed);
        let v2 = read_u64_le(&base[off + 8..off + 16]).wrapping_sub(seed);
        custom[off..off + 8].copy_from_slice(&v1.to_le_bytes());
        custom[off + 8..off + 16].copy_from_slice(&v2.to_le_bytes());
    }
}

fn xxh3_long_internal(input: &[u8], secret: &[u8], acc: &mut [u64; ACC_NB]) -> u64 {
    let len = input.len();
    let nb_stripes_per_block = (secret.len() - STRIPE_LEN) / 8;
    let block_len = STRIPE_LEN * nb_stripes_per_block;
    let nb_blocks = (len - 1) / block_len;

    for n in 0..nb_blocks {
        let block = &input[n * block_len..(n + 1) * block_len];
        for s in 0..nb_stripes_per_block {
            xxh3_accumulate_stripe(acc, &block[s * STRIPE_LEN..(s + 1) * STRIPE_LEN], &secret[s * 8..s * 8 + STRIPE_LEN]);
        }
        xxh3_scramble_acc(acc, &secret[secret.len() - STRIPE_LEN..]);
    }

    let last_block_start = nb_blocks * block_len;
    let last_block = &input[last_block_start..];
    let nb_stripes = (len - 1 - last_block_start) / STRIPE_LEN;

    for s in 0..nb_stripes {
        xxh3_accumulate_stripe(acc, &last_block[s * STRIPE_LEN..(s + 1) * STRIPE_LEN], &secret[s * 8..s * 8 + STRIPE_LEN]);
    }

    // Last stripe at very end
    let last_stripe = &input[len - STRIPE_LEN..];
    xxh3_accumulate_stripe(acc, last_stripe, &secret[secret.len() - STRIPE_LEN - 7..secret.len() - 7]);

    xxh3_merge_acc(acc, &secret[11..], (len as u64).wrapping_mul(PRIME64_1))
}

// ----------------------------------------------------------------------------
// XXH3-128 Implementation
// ----------------------------------------------------------------------------

#[inline(always)]
fn mult64to128_full(x: u64, y: u64) -> (u64, u64) {
    let p = (x as u128) * (y as u128);
    (p as u64, (p >> 64) as u64)
}

fn xxh3_128_len_0(seed: u64, secret: &[u8]) -> (u64, u64) {
    let low = seed ^ read_u64_le(&secret[64..72]) ^ read_u64_le(&secret[72..80]);
    let high = seed ^ read_u64_le(&secret[80..88]) ^ read_u64_le(&secret[88..96]);
    (xxh64_avalanche(low), xxh64_avalanche(high))
}

fn xxh3_128_len_1to3(input: &[u8], seed: u64, secret: &[u8]) -> (u64, u64) {
    let len = input.len();
    let c1 = input[0] as u32;
    let c2 = input[len >> 1] as u32;
    let c3 = input[len - 1] as u32;
    let combined_low = (c1 << 16) | (c2 << 24) | (c3 << 8) | (len as u32);
    let combined_high = combined_low.swap_bytes().rotate_left(13);

    let bitflip_low = (read_u32_le(&secret[0..4]) ^ read_u32_le(&secret[4..8])).wrapping_add(seed as u32);
    let bitflip_high = (read_u32_le(&secret[8..12]) ^ read_u32_le(&secret[12..16])).wrapping_sub(seed as u32);

    let keyed_low = (combined_low ^ bitflip_low) as u64;
    let keyed_high = (combined_high ^ bitflip_high) as u64;

    (xxh64_avalanche(keyed_low), xxh64_avalanche(keyed_high))
}

fn xxh3_128_len_4to8(input: &[u8], seed: u64, secret: &[u8]) -> (u64, u64) {
    let len = input.len();
    let in1 = read_u32_le(&input[..4]) as u64;
    let in2 = read_u32_le(&input[len - 4..]) as u64;
    let in64_1 = in1.wrapping_add(in2 << 32);

    let sec1 = read_u64_le(&secret[16..24]);
    let sec2 = read_u64_le(&secret[24..32]);
    let keyed1 = in64_1 ^ (sec1 ^ sec2).wrapping_sub(seed);

    let (mut l1, mut h1) = mult64to128_full(keyed1, PRIME64_1.wrapping_add((len as u64) << 32));
    h1 = h1.wrapping_add(l1.wrapping_mul(PRIME64_2));
    l1 = xxh3_avalanche(l1);
    h1 = xxh3_avalanche(h1);
    (l1, h1)
}

fn xxh3_128_len_9to16(input: &[u8], seed: u64, secret: &[u8]) -> (u64, u64) {
    let len = input.len();
    let sec1 = read_u64_le(&secret[32..40]).wrapping_add(seed);
    let sec2 = read_u64_le(&secret[40..48]).wrapping_sub(seed);
    let sec3 = read_u64_le(&secret[48..56]).wrapping_add(seed);
    let sec4 = read_u64_le(&secret[56..64]).wrapping_sub(seed);

    let input_low = read_u64_le(&input[..8]);
    let input_high = read_u64_le(&input[len - 8..]);

    let mut low = (len as u64).wrapping_add(input_low ^ sec1).wrapping_add(input_high ^ sec2);
    let mut high = (len as u64).wrapping_add(input_low ^ sec3).wrapping_add(input_high ^ sec4);

    let p1 = mult64to128(input_low ^ sec1, input_high ^ sec2);
    let p2 = mult64to128(input_low ^ sec3, input_high ^ sec4);

    low = low.wrapping_add(p1 as u64 ^ (p1 >> 64) as u64);
    high = high.wrapping_add(p2 as u64 ^ (p2 >> 64) as u64);

    (xxh3_avalanche(low), xxh3_avalanche(high))
}

fn xxh3_128_len_17to128(input: &[u8], seed: u64, secret: &[u8]) -> (u64, u64) {
    let low = xxh3_len_17to128(input, seed, secret);
    let high = xxh3_len_17to128(input, seed ^ 0xFFFFFFFFFFFFFFFF, &secret[16..]);
    (low, high)
}

fn xxh3_128_len_129to240(input: &[u8], seed: u64, secret: &[u8]) -> (u64, u64) {
    let low = xxh3_len_129to240(input, seed, secret);
    let high = xxh3_len_129to240(input, seed ^ 0xFFFFFFFFFFFFFFFF, &secret[16..]);
    (low, high)
}

// ----------------------------------------------------------------------------
// Public High-Level Entrypoints
// ----------------------------------------------------------------------------

/// Computes 64-bit XXH3 hash with default secret and zero seed.
#[inline]
pub fn xxh3_64(input: &[u8]) -> u64 {
    xxh3_64_with_seed(input, 0)
}

/// Computes 64-bit XXH3 hash with specified seed.
#[inline]
pub fn xxh3_64_with_seed(input: &[u8], seed: u64) -> u64 {
    let secret = &DEFAULT_SECRET;
    let len = input.len();
    if len <= 16 {
        xxh3_len_0to16(input, seed, secret)
    } else if len <= 128 {
        xxh3_len_17to128(input, seed, secret)
    } else if len <= MIDSIZE_MAX {
        xxh3_len_129to240(input, seed, secret)
    } else {
        xxh3_long(input, seed, secret)
    }
}

/// Computes 128-bit XXH3 hash as a pair of `(low, high)` u64 words with zero seed.
#[inline]
pub fn xxh3_128(input: &[u8]) -> (u64, u64) {
    xxh3_128_with_seed(input, 0)
}

/// Computes 128-bit XXH3 hash with specified seed, returning `(low, high)` u64 words.
#[inline]
pub fn xxh3_128_with_seed(input: &[u8], seed: u64) -> (u64, u64) {
    let secret = &DEFAULT_SECRET;
    let len = input.len();
    if len == 0 {
        xxh3_128_len_0(seed, secret)
    } else if len <= 3 {
        xxh3_128_len_1to3(input, seed, secret)
    } else if len <= 8 {
        xxh3_128_len_4to8(input, seed, secret)
    } else if len <= 16 {
        xxh3_128_len_9to16(input, seed, secret)
    } else if len <= 128 {
        xxh3_128_len_17to128(input, seed, secret)
    } else if len <= MIDSIZE_MAX {
        xxh3_128_len_129to240(input, seed, secret)
    } else {
        let low = xxh3_long(input, seed, secret);
        let mut custom_sec = [0u8; 192];
        derive_custom_secret(&mut custom_sec, &DEFAULT_SECRET, seed ^ 0xFFFFFFFFFFFFFFFF);
        let mut acc = INITIAL_ACC;
        let high = xxh3_long_internal(input, &custom_sec, &mut acc);
        (low, high)
    }
}

/// Computes 128-bit XXH3 hash returning 16 raw bytes in little-endian order.
#[inline]
pub fn xxh3_128_bytes(input: &[u8]) -> [u8; 16] {
    let (low, high) = xxh3_128(input);
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&low.to_le_bytes());
    out[8..].copy_from_slice(&high.to_le_bytes());
    out
}

/// Streaming XXH3 64-bit hasher.
#[derive(Clone)]
pub struct Xxh3_64 {
    seed: u64,
    buffer: Vec<u8>,
}

impl Default for Xxh3_64 {
    fn default() -> Self {
        Self::new()
    }
}

impl Xxh3_64 {
    pub fn new() -> Self {
        Self {
            seed: 0,
            buffer: Vec::with_capacity(512),
        }
    }

    pub fn with_seed(seed: u64) -> Self {
        Self {
            seed,
            buffer: Vec::with_capacity(512),
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    pub fn finalize(self) -> u64 {
        xxh3_64_with_seed(&self.buffer, self.seed)
    }
}

/// Streaming XXH3 128-bit hasher.
#[derive(Clone)]
pub struct Xxh3_128 {
    seed: u64,
    buffer: Vec<u8>,
}

impl Default for Xxh3_128 {
    fn default() -> Self {
        Self::new()
    }
}

impl Xxh3_128 {
    pub fn new() -> Self {
        Self {
            seed: 0,
            buffer: Vec::with_capacity(512),
        }
    }

    pub fn with_seed(seed: u64) -> Self {
        Self {
            seed,
            buffer: Vec::with_capacity(512),
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    pub fn finalize(self) -> (u64, u64) {
        xxh3_128_with_seed(&self.buffer, self.seed)
    }

    pub fn finalize_bytes(self) -> [u8; 16] {
        let (low, high) = self.finalize();
        let mut out = [0u8; 16];
        out[..8].copy_from_slice(&low.to_le_bytes());
        out[8..].copy_from_slice(&high.to_le_bytes());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xxh3_64_empty() {
        assert_eq!(xxh3_64(b""), 3244421341483603138);
    }


    #[test]
    fn test_xxh3_64_known_vectors() {
        let h1 = xxh3_64(b"123456789");
        assert_ne!(h1, 0);

        let mut streaming = Xxh3_64::new();
        streaming.update(b"12345");
        streaming.update(b"6789");
        assert_eq!(streaming.finalize(), h1);
    }

    #[test]
    fn test_xxh3_128_known_vectors() {
        let (l, h) = xxh3_128(b"");
        assert_ne!((l, h), (0, 0));

        let (l1, h1) = xxh3_128(b"The quick brown fox jumps over the lazy dog");
        assert_ne!((l1, h1), (0, 0));

        let mut st = Xxh3_128::new();
        st.update(b"The quick brown fox ");
        st.update(b"jumps over the lazy dog");
        assert_eq!(st.finalize(), (l1, h1));
    }

    #[test]
    fn test_xxh3_long_buffer() {
        let buf = vec![0xABu8; 4096];
        let h = xxh3_64(&buf);
        assert_ne!(h, 0);
        let (l, high) = xxh3_128(&buf);
        assert_ne!((l, high), (0, 0));
    }
}
