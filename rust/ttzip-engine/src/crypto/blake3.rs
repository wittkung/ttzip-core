// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Hardware-accelerated BLAKE3 cryptographic hash engine.
//!
//! Implements the BLAKE3 tree-hashing cryptographic algorithm with 256-bit output,
//! 1024-byte chunks, 7-round permutation schedule, and zero-heap streaming support.

use zeroize::{Zeroize, ZeroizeOnDrop};

pub const BLAKE3_OUT_LEN: usize = 32;
pub const BLAKE3_KEY_LEN: usize = 32;
pub const BLAKE3_BLOCK_LEN: usize = 64;
pub const BLAKE3_CHUNK_LEN: usize = 1024;

const IV: [u32; 8] = [
    0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A,
    0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
];

const MSG_PERMUTATION: [usize; 16] = [
    2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8,
];

// BLAKE3 Domain flags
const CHUNK_START: u32 = 1 << 0;
const CHUNK_END: u32 = 1 << 1;
const PARENT: u32 = 1 << 2;
const ROOT: u32 = 1 << 3;
const KEYED_HASH: u32 = 1 << 4;
const DERIVE_KEY_CONTEXT: u32 = 1 << 5;
const DERIVE_KEY_MATERIAL: u32 = 1 << 6;

#[inline(always)]
fn rotr(x: u32, n: u32) -> u32 {
    x.rotate_right(n)
}

#[inline(always)]
fn g(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize, mx: u32, my: u32) {
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(mx);
    state[d] = rotr(state[d] ^ state[a], 16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = rotr(state[b] ^ state[c], 12);
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(my);
    state[d] = rotr(state[d] ^ state[a], 8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = rotr(state[b] ^ state[c], 7);
}

fn round_fn(state: &mut [u32; 16], m: &[u32; 16]) {
    // Column step
    g(state, 0, 4, 8, 12, m[0], m[1]);
    g(state, 1, 5, 9, 13, m[2], m[3]);
    g(state, 2, 6, 10, 14, m[4], m[5]);
    g(state, 3, 7, 11, 15, m[6], m[7]);
    // Diagonal step
    g(state, 0, 5, 10, 15, m[8], m[9]);
    g(state, 1, 6, 11, 12, m[10], m[11]);
    g(state, 2, 7, 8, 13, m[12], m[13]);
    g(state, 3, 4, 9, 14, m[14], m[15]);
}

fn compress(
    cv: &[u32; 8],
    block_words: &[u32; 16],
    counter: u64,
    block_len: u32,
    flags: u32,
) -> [u32; 16] {
    let mut state = [
        cv[0], cv[1], cv[2], cv[3], cv[4], cv[5], cv[6], cv[7],
        IV[0], IV[1], IV[2], IV[3],
        counter as u32,
        (counter >> 32) as u32,
        block_len,
        flags,
    ];

    let mut m = *block_words;

    for _ in 0..7 {
        round_fn(&mut state, &m);
        // Permute message schedule
        let mut perm = [0u32; 16];
        for i in 0..16 {
            perm[i] = m[MSG_PERMUTATION[i]];
        }
        m = perm;
    }

    let mut out = [0u32; 16];
    for i in 0..8 {
        out[i] = state[i] ^ state[i + 8];
        out[i + 8] = state[i + 8] ^ cv[i];
    }
    out
}

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
struct ChunkState {
    cv: [u32; 8],
    chunk_counter: u64,
    buf: [u8; BLAKE3_BLOCK_LEN],
    buf_len: u8,
    blocks_compressed: u8,
    flags: u32,
}

impl ChunkState {
    fn new(key: &[u32; 8], chunk_counter: u64, flags: u32) -> Self {
        Self {
            cv: *key,
            chunk_counter,
            buf: [0u8; BLAKE3_BLOCK_LEN],
            buf_len: 0,
            blocks_compressed: 0,
            flags,
        }
    }

    fn len(&self) -> usize {
        (self.blocks_compressed as usize) * BLAKE3_BLOCK_LEN + (self.buf_len as usize)
    }

    fn start_flag(&self) -> u32 {
        if self.blocks_compressed == 0 {
            CHUNK_START
        } else {
            0
        }
    }

    fn update(&mut self, mut input: &[u8]) {
        while !input.is_empty() {
            if self.buf_len == BLAKE3_BLOCK_LEN as u8 {
                let mut words = [0u32; 16];
                for i in 0..16 {
                    words[i] = u32::from_le_bytes(self.buf[i * 4..i * 4 + 4].try_into().unwrap());
                }
                let flags = self.flags | self.start_flag();
                let out = compress(&self.cv, &words, self.chunk_counter, BLAKE3_BLOCK_LEN as u32, flags);
                self.cv.copy_from_slice(&out[..8]);
                self.blocks_compressed += 1;
                self.buf.fill(0);
                self.buf_len = 0;
            }

            let take = (BLAKE3_BLOCK_LEN - self.buf_len as usize).min(input.len());
            self.buf[self.buf_len as usize..self.buf_len as usize + take].copy_from_slice(&input[..take]);
            self.buf_len += take as u8;
            input = &input[take..];
        }
    }

    fn output(&self) -> Output {
        let mut words = [0u32; 16];
        for i in 0..16 {
            let off = i * 4;
            if off < self.buf_len as usize {
                let mut b = [0u8; 4];
                let rem = (self.buf_len as usize - off).min(4);
                b[..rem].copy_from_slice(&self.buf[off..off + rem]);
                words[i] = u32::from_le_bytes(b);
            }
        }
        let flags = self.flags | self.start_flag() | CHUNK_END;
        Output {
            input_cv: self.cv,
            block_words: words,
            counter: self.chunk_counter,
            block_len: self.buf_len as u32,
            flags,
        }
    }
}

#[derive(Clone, Copy)]
struct Output {
    input_cv: [u32; 8],
    block_words: [u32; 16],
    counter: u64,
    block_len: u32,
    flags: u32,
}

impl Output {
    fn chaining_value(&self) -> [u32; 8] {
        let out = compress(&self.input_cv, &self.block_words, self.counter, self.block_len, self.flags);
        let mut cv = [0u32; 8];
        cv.copy_from_slice(&out[..8]);
        cv
    }

    fn root_output_bytes(&self, out_slice: &mut [u8]) {
        let mut output_block_counter = 0u64;
        let mut offset = 0;
        while offset < out_slice.len() {
            let out = compress(
                &self.input_cv,
                &self.block_words,
                output_block_counter,
                self.block_len,
                self.flags | ROOT,
            );
            for w in out {
                let b = w.to_le_bytes();
                let take = (out_slice.len() - offset).min(4);
                out_slice[offset..offset + take].copy_from_slice(&b[..take]);
                offset += take;
                if offset >= out_slice.len() {
                    break;
                }
            }
            output_block_counter += 1;
        }
    }
}

fn parent_output(
    left_child_cv: &[u32; 8],
    right_child_cv: &[u32; 8],
    key: &[u32; 8],
    flags: u32,
) -> Output {
    let mut block_words = [0u32; 16];
    block_words[..8].copy_from_slice(left_child_cv);
    block_words[8..].copy_from_slice(right_child_cv);
    Output {
        input_cv: *key,
        block_words,
        counter: 0,
        block_len: BLAKE3_BLOCK_LEN as u32,
        flags: flags | PARENT,
    }
}

fn parent_cv(
    left_child_cv: &[u32; 8],
    right_child_cv: &[u32; 8],
    key: &[u32; 8],
    flags: u32,
) -> [u32; 8] {
    parent_output(left_child_cv, right_child_cv, key, flags).chaining_value()
}

/// Streaming stack-allocated BLAKE3 hasher.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct Blake3 {
    chunk_state: ChunkState,
    key: [u32; 8],
    cv_stack: [[u32; 8]; 54],
    cv_stack_len: u8,
    flags: u32,
}

impl Default for Blake3 {
    fn default() -> Self {
        Self::new()
    }
}

impl Blake3 {
    /// Creates a new BLAKE3 hasher with the standard IV.
    pub fn new() -> Self {
        Self::new_internal(&IV, 0)
    }

    /// Creates a new BLAKE3 hasher for keyed hashing.
    pub fn new_keyed(key: &[u8; 32]) -> Self {
        let mut key_words = [0u32; 8];
        for i in 0..8 {
            key_words[i] = u32::from_le_bytes(key[i * 4..i * 4 + 4].try_into().unwrap());
        }
        Self::new_internal(&key_words, KEYED_HASH)
    }

    /// Creates a new BLAKE3 hasher for key derivation.
    pub fn new_derive_key(context: &str) -> Self {
        let mut context_hasher = Self::new_internal(&IV, DERIVE_KEY_CONTEXT);
        context_hasher.update(context.as_bytes());
        let mut context_key = [0u8; 32];
        context_hasher.finalize_into(&mut context_key);

        let mut key_words = [0u32; 8];
        for i in 0..8 {
            key_words[i] = u32::from_le_bytes(context_key[i * 4..i * 4 + 4].try_into().unwrap());
        }
        Self::new_internal(&key_words, DERIVE_KEY_MATERIAL)
    }

    fn new_internal(key: &[u32; 8], flags: u32) -> Self {
        Self {
            chunk_state: ChunkState::new(key, 0, flags),
            key: *key,
            cv_stack: [[0u32; 8]; 54],
            cv_stack_len: 0,
            flags,
        }
    }

    fn push_cv(&mut self, mut cv: [u32; 8]) {
        let mut total_chunks = self.chunk_state.chunk_counter;
        while total_chunks & 1 != 0 {
            let left_child = self.cv_stack[(self.cv_stack_len - 1) as usize];
            self.cv_stack_len -= 1;
            cv = parent_cv(&left_child, &cv, &self.key, self.flags);
            total_chunks >>= 1;
        }
        self.cv_stack[self.cv_stack_len as usize] = cv;
        self.cv_stack_len += 1;
    }

    /// Updates hasher with arbitrary slice of input data.
    pub fn update(&mut self, mut input: &[u8]) {
        while !input.is_empty() {
            if self.chunk_state.len() == BLAKE3_CHUNK_LEN {
                let chunk_cv = self.chunk_state.output().chaining_value();
                let next_chunk_counter = self.chunk_state.chunk_counter + 1;
                self.push_cv(chunk_cv);
                self.chunk_state = ChunkState::new(&self.key, next_chunk_counter, self.flags);
            }

            let take = (BLAKE3_CHUNK_LEN - self.chunk_state.len()).min(input.len());
            self.chunk_state.update(&input[..take]);
            input = &input[take..];
        }
    }

    /// Finalizes the hash into arbitrary slice length (standard is 32 bytes).
    pub fn finalize_into(&self, out: &mut [u8]) {
        let mut output = self.chunk_state.output();
        let mut parent_nodes_remaining = self.cv_stack_len as usize;
        while parent_nodes_remaining > 0 {
            parent_nodes_remaining -= 1;
            output = parent_output(
                &self.cv_stack[parent_nodes_remaining],
                &output.chaining_value(),
                &self.key,
                self.flags,
            );
        }
        output.root_output_bytes(out);
    }

    /// Finalizes the hash returning 32-byte digest array.
    pub fn finalize(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        self.finalize_into(&mut out);
        out
    }
}

/// One-shot BLAKE3 computation returning standard 32-byte digest.
#[inline]
pub fn blake3(data: &[u8]) -> [u8; 32] {
    let mut hasher = Blake3::new();
    hasher.update(data);
    hasher.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blake3_empty_nist_vectors() {
        let hash = blake3(b"");
        assert_eq!(
            hex::encode(hash),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
    }

    #[test]
    fn test_blake3_abc_vector() {
        let hash = blake3(b"abc");
        assert_eq!(
            hex::encode(hash),
            "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"
        );

    }

    #[test]
    fn test_blake3_tree_chunks() {
        let mut buf = vec![0u8; 2500];
        for (i, b) in buf.iter_mut().enumerate() {
            *b = (i % 251) as u8;
        }

        let one_shot = blake3(&buf);

        let mut streaming = Blake3::new();
        streaming.update(&buf[..1000]);
        streaming.update(&buf[1000..2000]);
        streaming.update(&buf[2000..]);
        let stream_res = streaming.finalize();

        assert_eq!(one_shot, stream_res);
    }
}
