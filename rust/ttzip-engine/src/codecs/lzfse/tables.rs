// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Apple LZFSE static tables, base values, extra bits, and Huffman prefix codes for header serialization.

// MARK: - Constants

pub const LZFSE_ENCODE_HASH_BITS: usize = 14;
pub const LZFSE_ENCODE_HASH_WIDTH: usize = 4;
pub const LZFSE_ENCODE_HASH_VALUES: usize = 1 << LZFSE_ENCODE_HASH_BITS; // 16,384

pub const LZFSE_ENCODE_L_SYMBOLS: usize = 20;
pub const LZFSE_ENCODE_M_SYMBOLS: usize = 20;
pub const LZFSE_ENCODE_D_SYMBOLS: usize = 64;
pub const LZFSE_ENCODE_LITERAL_SYMBOLS: usize = 256;

pub const LZFSE_ENCODE_L_STATES: usize = 64;
pub const LZFSE_ENCODE_M_STATES: usize = 64;
pub const LZFSE_ENCODE_D_STATES: usize = 256;
pub const LZFSE_ENCODE_LITERAL_STATES: usize = 1024;

pub const LZFSE_MATCHES_PER_BLOCK: usize = 10000;
pub const LZFSE_LITERALS_PER_BLOCK: usize = 4 * LZFSE_MATCHES_PER_BLOCK; // 40,000

pub const LZFSE_ENCODE_MAX_L_VALUE: usize = 315;
pub const LZFSE_ENCODE_MAX_M_VALUE: usize = 2359;
pub const LZFSE_ENCODE_MAX_D_VALUE: usize = 262139;
pub const LZFSE_ENCODE_MAX_MATCH_LENGTH: usize = 100 * LZFSE_ENCODE_MAX_M_VALUE;
pub const LZFSE_ENCODE_GOOD_MATCH: usize = 40;

pub const LZFSE_NO_BLOCK_MAGIC: u32 = 0x0000_0000;
pub const LZFSE_ENDOFSTREAM_BLOCK_MAGIC: u32 = 0x2478_7662; // "bvx$"
pub const LZFSE_UNCOMPRESSED_BLOCK_MAGIC: u32 = 0x2d78_7662; // "bvx-"
pub const LZFSE_COMPRESSEDV1_BLOCK_MAGIC: u32 = 0x3178_7662; // "bvx1"
pub const LZFSE_COMPRESSEDV2_BLOCK_MAGIC: u32 = 0x3278_7662; // "bvx2"
pub const LZFSE_COMPRESSEDLZVN_BLOCK_MAGIC: u32 = 0x6e78_7662; // "bvxn"

// MARK: - Extra Bits and Base Values

pub static L_EXTRA_BITS: [u8; LZFSE_ENCODE_L_SYMBOLS] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 3, 5, 8,
];

pub static L_BASE_VALUE: [i32; LZFSE_ENCODE_L_SYMBOLS] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 20, 28, 60,
];

pub static M_EXTRA_BITS: [u8; LZFSE_ENCODE_M_SYMBOLS] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 5, 8, 11,
];

pub static M_BASE_VALUE: [i32; LZFSE_ENCODE_M_SYMBOLS] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 24, 56, 312,
];

pub static D_EXTRA_BITS: [u8; LZFSE_ENCODE_D_SYMBOLS] = [
    0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 7, 7, 7,
    7, 8, 8, 8, 8, 9, 9, 9, 9, 10, 10, 10, 10, 11, 11, 11, 11, 12, 12, 12, 12, 13, 13, 13, 13,
    14, 14, 14, 14, 15, 15, 15, 15,
];

pub static D_BASE_VALUE: [i32; LZFSE_ENCODE_D_SYMBOLS] = [
    0, 1, 2, 3, 4, 6, 8, 10, 12, 16, 20, 24, 28, 36, 44, 52, 60, 76, 92, 108, 124, 156, 188, 220,
    252, 316, 380, 444, 508, 636, 764, 892, 1020, 1276, 1532, 1788, 2044, 2556, 3068, 3580, 4092,
    5116, 6140, 7164, 8188, 10236, 12284, 14332, 16380, 20476, 24572, 28668, 32764, 40956, 49148,
    57340, 65532, 81916, 98300, 114684, 131068, 163836, 196604, 229372,
];

// MARK: - Value to Symbol Mapping Functions

/// Maps literal length value L (0..=315) to its FSE symbol index (0..20).
#[inline]
pub fn l_base_from_value(value: i32) -> u8 {
    let v = value.clamp(0, LZFSE_ENCODE_MAX_L_VALUE as i32) as usize;
    L_SYM_TABLE[v]
}

/// Maps match length value M (0..=2359) to its FSE symbol index (0..20).
#[inline]
pub fn m_base_from_value(value: i32) -> u8 {
    let v = value.clamp(0, LZFSE_ENCODE_MAX_M_VALUE as i32) as usize;
    M_SYM_TABLE[v]
}

/// Maps match distance value D (0..=262139) to its FSE symbol index (0..64).
#[inline]
pub fn d_base_from_value(value: i32) -> u8 {
    static SYM: [u8; 256] = [
        0, 1, 2, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 8, 8, 9, 9, 9, 9, 10, 10, 10, 10, 11, 11, 11,
        11, 12, 12, 12, 12, 12, 12, 12, 12, 13, 13, 13, 13, 13, 13, 13, 13, 14, 14, 14, 14, 14,
        14, 14, 14, 15, 15, 15, 15, 15, 15, 15, 15, 16, 16, 16, 16, 16, 17, 18, 19, 20, 20, 21,
        21, 22, 22, 23, 23, 24, 24, 24, 24, 25, 25, 25, 25, 26, 26, 26, 26, 27, 27, 27, 27, 28,
        28, 28, 28, 28, 28, 28, 28, 29, 29, 29, 29, 29, 29, 29, 29, 30, 30, 30, 30, 30, 30, 30,
        30, 31, 31, 31, 31, 31, 31, 31, 31, 32, 32, 32, 32, 32, 33, 34, 35, 36, 36, 37, 37, 38,
        38, 39, 39, 40, 40, 40, 40, 41, 41, 41, 41, 42, 42, 42, 42, 43, 43, 43, 43, 44, 44, 44,
        44, 44, 44, 44, 44, 45, 45, 45, 45, 45, 45, 45, 45, 46, 46, 46, 46, 46, 46, 46, 46, 47,
        47, 47, 47, 47, 47, 47, 47, 48, 48, 48, 48, 48, 49, 50, 51, 52, 52, 53, 53, 54, 54, 55,
        55, 56, 56, 56, 56, 57, 57, 57, 57, 58, 58, 58, 58, 59, 59, 59, 59, 60, 60, 60, 60, 60,
        60, 60, 60, 61, 61, 61, 61, 61, 61, 61, 61, 62, 62, 62, 62, 62, 62, 62, 62, 63, 63, 63,
        63, 63, 63, 63, 63, 0, 0, 0, 0,
    ];

    let mut index: i32 = 0;
    if (0..60).contains(&value) {
        index = value;
    } else if (60..1020).contains(&value) {
        index = ((value - 60) >> 4) + 64;
    } else if (1020..16380).contains(&value) {
        index = ((value - 1020) >> 8) + 128;
    } else if (16380..262140).contains(&value) {
        index = ((value - 16380) >> 12) + 192;
    }
    SYM[(index & 255) as usize]
}

// MARK: - Frequency Value Huffman Encoder

/// Encodes normalized frequency value into prefix Huffman bits.
#[inline]
pub fn lzfse_encode_v1_freq_value(value: i32) -> (u32, i32) {
    match value {
        0 => (0, 2),
        1 => (2, 2),
        2 => (1, 3),
        3 => (5, 3),
        4 => (3, 5),
        5 => (11, 5),
        6 => (19, 5),
        7 => (27, 5),
        8..=23 => (7 + (((value - 8) as u32) << 4), 8),
        _ => ((((value - 24) as u32) << 4) + 15, 14),
    }
}

// MARK: - Precomputed Static Symbol Tables

const L_SYM_TABLE: [u8; LZFSE_ENCODE_MAX_L_VALUE + 1] = init_l_sym_table();
const M_SYM_TABLE: [u8; LZFSE_ENCODE_MAX_M_VALUE + 1] = init_m_sym_table();

const fn init_l_sym_table() -> [u8; LZFSE_ENCODE_MAX_L_VALUE + 1] {
    let mut table = [0u8; LZFSE_ENCODE_MAX_L_VALUE + 1];
    let mut i = 0;
    while i <= LZFSE_ENCODE_MAX_L_VALUE {
        table[i] = if i < 16 {
            i as u8
        } else if i < 20 {
            16
        } else if i < 28 {
            17
        } else if i < 60 {
            18
        } else {
            19
        };
        i += 1;
    }
    table
}

const fn init_m_sym_table() -> [u8; LZFSE_ENCODE_MAX_M_VALUE + 1] {
    let mut table = [0u8; LZFSE_ENCODE_MAX_M_VALUE + 1];
    let mut i = 0;
    while i <= LZFSE_ENCODE_MAX_M_VALUE {
        table[i] = if i < 16 {
            i as u8
        } else if i < 24 {
            16
        } else if i < 56 {
            17
        } else if i < 312 {
            18
        } else {
            19
        };
        i += 1;
    }
    table
}
