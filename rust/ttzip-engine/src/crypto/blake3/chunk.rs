// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! BLAKE3 1024-byte chunk buffering, compression scheduling, and state machine.
//!
//! Provides the core [`ChunkState`] structure which buffers up to 1024 bytes
//! in 64-byte micro-blocks, manages the `CHUNK_START` / `CHUNK_END` flags, and
//! generates intermediate or root [`Output`] blocks with zero heap allocation.

use zeroize::{Zeroize, ZeroizeOnDrop};

use super::compress::compress_in_place_mut;
use super::constants::{BLOCK_LEN, CHUNK_END, CHUNK_LEN, CHUNK_START};
use super::output::Output;

/// BLAKE3 single chunk state machine buffering up to 1024 bytes.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct ChunkState {
    /// Running 8-word chaining value within the chunk.
    pub chaining_value: [u32; 8],
    /// Absolute chunk counter in the stream.
    pub chunk_counter: u64,
    /// Micro-block buffer of up to 64 bytes.
    pub block: [u8; BLOCK_LEN],
    /// Number of valid uncompressed bytes currently in `block` (0..=64).
    pub block_len: u8,
    /// Number of full 64-byte blocks compressed so far in this chunk (0..=15).
    pub blocks_compressed: u8,
    /// Base flags (e.g. `KEYED_HASH`, `DERIVE_KEY_MATERIAL`).
    pub flags: u8,
}

impl ChunkState {
    /// Constructs a new chunk state with the given initial key / chaining value.
    #[inline]
    pub const fn new(key: [u32; 8], chunk_counter: u64, flags: u8) -> Self {
        Self {
            chaining_value: key,
            chunk_counter,
            block: [0u8; BLOCK_LEN],
            block_len: 0,
            blocks_compressed: 0,
            flags,
        }
    }

    /// Resets the chunk state for a new chunk counter while preserving flags.
    #[inline]
    pub fn reset(&mut self, key: [u32; 8], chunk_counter: u64) {
        self.chaining_value = key;
        self.chunk_counter = chunk_counter;
        self.block.fill(0);
        self.block_len = 0;
        self.blocks_compressed = 0;
    }

    /// Returns the total number of bytes received in this chunk so far (0..=1024).
    #[inline]
    pub const fn len(&self) -> usize {
        (self.blocks_compressed as usize) * BLOCK_LEN + (self.block_len as usize)
    }

    /// Returns true if this chunk state has received zero bytes.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Computes the start flag for the next block compression.
    #[inline]
    pub const fn start_flag(&self) -> u8 {
        if self.blocks_compressed == 0 {
            CHUNK_START
        } else {
            0
        }
    }

    /// Ingests input bytes into this chunk, pausing when 1024 bytes are reached.
    ///
    /// Returns the number of bytes consumed from `input`.
    pub fn update(&mut self, mut input: &[u8]) -> usize {
        let available = CHUNK_LEN - self.len();
        let take_total = input.len().min(available);
        input = &input[..take_total];
        let mut remaining = input;

        // If there are existing buffered bytes, fill the current block first.
        if self.block_len > 0 {
            let want = BLOCK_LEN - (self.block_len as usize);
            let take = remaining.len().min(want);
            self.block[self.block_len as usize..self.block_len as usize + take]
                .copy_from_slice(&remaining[..take]);
            self.block_len += take as u8;
            remaining = &remaining[take..];

            // Only compress the 64-byte block if more bytes follow in this chunk.
            if self.block_len == (BLOCK_LEN as u8) && !remaining.is_empty() {
                let block_flags = self.flags | self.start_flag();
                compress_in_place_mut(
                    &mut self.chaining_value,
                    &self.block,
                    BLOCK_LEN as u8,
                    self.chunk_counter,
                    block_flags,
                );
                self.blocks_compressed += 1;
                self.block.fill(0);
                self.block_len = 0;
            }
        }

        // Compress full 64-byte blocks directly from the remaining input slice,
        // leaving at least 1 byte so that the chunk's trailing block is captured in `block`.
        while remaining.len() > BLOCK_LEN {
            let block_flags = self.flags | self.start_flag();
            let mut block_buf = [0u8; BLOCK_LEN];
            block_buf.copy_from_slice(&remaining[..BLOCK_LEN]);
            compress_in_place_mut(
                &mut self.chaining_value,
                &block_buf,
                BLOCK_LEN as u8,
                self.chunk_counter,
                block_flags,
            );
            self.blocks_compressed += 1;
            remaining = &remaining[BLOCK_LEN..];
        }

        // Buffer the final partial or full block into `self.block`.
        if !remaining.is_empty() {
            let take = remaining.len();
            self.block[self.block_len as usize..self.block_len as usize + take]
                .copy_from_slice(&remaining[..take]);
            self.block_len += take as u8;
        }

        take_total
    }

    /// Constructs the final [`Output`] structure representing this chunk.
    #[inline]
    pub const fn output(&self) -> Output {
        let block_flags = self.flags | self.start_flag() | CHUNK_END;
        Output {
            input_chaining_value: self.chaining_value,
            block: self.block,
            block_len: self.block_len,
            counter: self.chunk_counter,
            flags: block_flags,
        }
    }
}
