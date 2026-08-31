// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! BLAKE3 cryptographic constants and domain separation flags.
//!
//! Specifications conform strictly to the official BLAKE3 standard paper
//! and portable reference implementation.

/// Default output length in bytes (256-bit digest = 32 bytes).
pub const OUT_LEN: usize = 32;

/// Key length in bytes for keyed hashing and key derivation (32 bytes).
pub const KEY_LEN: usize = 32;

/// Number of bytes in a single input block (64 bytes = 16 words of 32 bits).
pub const BLOCK_LEN: usize = 64;

/// Number of bytes in a Merkle tree chunk (1024 bytes = 16 blocks).
pub const CHUNK_LEN: usize = 1024;

/// Alias constants for legacy backwards compatibility.
pub const BLAKE3_OUT_LEN: usize = OUT_LEN;
pub const BLAKE3_KEY_LEN: usize = KEY_LEN;
pub const BLAKE3_BLOCK_LEN: usize = BLOCK_LEN;
pub const BLAKE3_CHUNK_LEN: usize = CHUNK_LEN;

/// BLAKE3 Initialization Vector (same as SHA-256 standard IV).
pub const IV: [u32; 8] = [
    0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A,
    0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
];

/// 7-round message word permutation schedule (\sigma^0 through \sigma^6).
///
/// Each round permutes message word indices 0..15 according to the fixed permutation:
/// `\sigma = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8]`.
pub const MSG_SCHEDULE: [[usize; 16]; 7] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8],
    [3, 4, 10, 12, 13, 2, 7, 14, 6, 5, 9, 0, 11, 15, 8, 1],
    [10, 7, 12, 9, 14, 3, 13, 15, 4, 0, 11, 2, 5, 8, 1, 6],
    [12, 13, 9, 11, 15, 10, 14, 8, 7, 2, 5, 3, 0, 1, 6, 4],
    [9, 14, 11, 5, 8, 12, 15, 1, 13, 3, 0, 10, 2, 6, 4, 7],
    [11, 15, 5, 0, 1, 9, 8, 6, 14, 10, 2, 12, 3, 4, 7, 13],
];

/// Domain separation flag: First block of a chunk.
pub const CHUNK_START: u8 = 1 << 0;

/// Domain separation flag: Last block of a chunk.
pub const CHUNK_END: u8 = 1 << 1;

/// Domain separation flag: Internal Merkle tree parent node.
pub const PARENT: u8 = 1 << 2;

/// Domain separation flag: Root node of the entire tree / output node.
pub const ROOT: u8 = 1 << 3;

/// Domain separation flag: Keyed hashing mode active.
pub const KEYED_HASH: u8 = 1 << 4;

/// Domain separation flag: Key derivation mode - context string hashing.
pub const DERIVE_KEY_CONTEXT: u8 = 1 << 5;

/// Domain separation flag: Key derivation mode - key material hashing.
pub const DERIVE_KEY_MATERIAL: u8 = 1 << 6;
