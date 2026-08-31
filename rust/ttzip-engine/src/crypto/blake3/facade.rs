// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe Rust streaming [`Blake3Hasher`] facade, high-level API functions, and trait adapters.
//!
//! Provides the primary, zero-heap streaming BLAKE3 hashing facade conforming to the
//! official BLAKE3 standard. Encapsulates [`ChunkState`] 1024-byte buffering, [`TreeStack`]
//! subtree reduction, domain flags, and non-destructive output extraction.

use std::fmt;
use std::io;
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::chunk::ChunkState;
use super::constants::{
    BLAKE3_CHUNK_LEN, DERIVE_KEY_CONTEXT, DERIVE_KEY_MATERIAL, IV, KEYED_HASH,
};
use super::output::{Output, OutputReader};
use super::tree::TreeStack;

/// Primary Safe Rust stack-allocated streaming BLAKE3 hasher.
///
/// Encapsulates the complete BLAKE3 state machine including 1024-byte chunk buffering,
/// 55-element fixed-capacity subtree reduction stack, and non-destructive output generation.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct Blake3Hasher {
    /// Active single-chunk state machine buffering up to 1024 bytes.
    pub(crate) chunk_state: ChunkState,
    /// Fixed-size stack-allocated subtree chaining value reduction stack.
    pub(crate) tree_stack: TreeStack,
    /// 256-bit base key or initial chaining value words.
    pub(crate) key: [u32; 8],
    /// Total number of full 1024-byte chunks completed and pushed to the tree stack.
    pub(crate) total_chunks: u64,
    /// Base domain separation flags (e.g. `KEYED_HASH`, `DERIVE_KEY_MATERIAL`).
    pub(crate) flags: u8,
}

impl Default for Blake3Hasher {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Blake3Hasher {
    /// Creates a new unkeyed [`Blake3Hasher`] initialized with the standard BLAKE3 IV.
    #[inline]
    pub fn new() -> Self {
        Self::new_internal(&IV, 0)
    }

    /// Creates a new keyed [`Blake3Hasher`] for MAC generation and keyed verification.
    pub fn new_keyed(key: &[u8; 32]) -> Self {
        let mut key_words = [0u32; 8];
        for i in 0..8 {
            key_words[i] = u32::from_le_bytes(
                key[i * 4..i * 4 + 4]
                    .try_into()
                    .expect("slice length matches 4 bytes"),
            );
        }
        Self::new_internal(&key_words, KEYED_HASH)
    }

    /// Creates a new [`Blake3Hasher`] for cryptographic key derivation (KDF).
    ///
    /// Derives an internal context key from `context` and configures the hasher
    /// with `DERIVE_KEY_MATERIAL` domain separation.
    pub fn new_derive_key(context: &str) -> Self {
        let mut context_hasher = Self::new_internal(&IV, DERIVE_KEY_CONTEXT);
        context_hasher.update(context.as_bytes());
        let mut context_key = [0u8; 32];
        context_hasher.finalize_into(&mut context_key);

        let mut key_words = [0u32; 8];
        for i in 0..8 {
            key_words[i] = u32::from_le_bytes(
                context_key[i * 4..i * 4 + 4]
                    .try_into()
                    .expect("slice length matches 4 bytes"),
            );
        }
        Self::new_internal(&key_words, DERIVE_KEY_MATERIAL)
    }

    #[inline]
    fn new_internal(key: &[u32; 8], flags: u8) -> Self {
        Self {
            chunk_state: ChunkState::new(*key, 0, flags),
            tree_stack: TreeStack::new(),
            key: *key,
            total_chunks: 0,
            flags,
        }
    }

    /// Resets the hasher to its initial state using the original key and domain flags.
    pub fn reset(&mut self) {
        self.chunk_state.reset(self.key, 0);
        self.tree_stack.clear();
        self.total_chunks = 0;
    }

    /// Adds input bytes to the running hash state.
    ///
    /// Buffers input in 1024-byte chunks, advancing the chunk counter and lazily
    /// merging subtree chaining values into the internal reduction stack.
    pub fn update(&mut self, mut input: &[u8]) -> &mut Self {
        while !input.is_empty() {
            if self.chunk_state.len() == BLAKE3_CHUNK_LEN {
                let chunk_cv = self.chunk_state.output().chaining_value();
                self.total_chunks += 1;
                let next_chunk_counter = self.total_chunks;
                self.tree_stack.merge_cv_stack(
                    self.chunk_state.chunk_counter,
                    &self.key,
                    self.flags,
                );
                self.tree_stack.push(chunk_cv);
                self.chunk_state.reset(self.key, next_chunk_counter);
            }

            let consumed = self.chunk_state.update(input);
            input = &input[consumed..];
        }
        self
    }

    /// Returns the total number of bytes ingested into this hasher so far.
    #[inline]
    pub fn count(&self) -> u64 {
        self.chunk_state
            .chunk_counter
            .wrapping_mul(BLAKE3_CHUNK_LEN as u64)
            .wrapping_add(self.chunk_state.len() as u64)
    }

    /// Returns the total number of full 1024-byte chunks completed and pushed to the tree stack.
    #[inline]
    pub fn total_chunks(&self) -> u64 {
        self.total_chunks
    }

    /// Finalizes the hash tree and returns the root [`Output`] descriptor without mutating self.
    pub fn finalize_output(&self) -> Output {
        let mut stack = self.tree_stack.clone();
        if !self.chunk_state.is_empty() && self.chunk_state.chunk_counter > 0 {
            stack.merge_cv_stack(self.chunk_state.chunk_counter, &self.key, self.flags);
        }
        stack.fold_right_spine(self.chunk_state.output(), &self.key, self.flags)
    }

    /// Finalizes the hash state and returns the standard 32-byte (256-bit) digest.
    ///
    /// This method is non-destructive and idempotent; subsequent calls or further
    /// `update` calls remain completely valid.
    #[inline]
    pub fn finalize(&self) -> [u8; 32] {
        self.finalize_output().root_hash()
    }

    /// Finalizes the hash into an arbitrary destination slice length.
    #[inline]
    pub fn finalize_into(&self, out: &mut [u8]) {
        self.finalize_xof().fill(out);
    }

    /// Finalizes the hash state and returns an [`OutputReader`] for arbitrary-length output (XOF).
    ///
    /// The returned reader supports $O(1)$ random seeking, buffering, and standard [`io::Read`].
    #[inline]
    pub fn finalize_xof(&self) -> OutputReader {
        OutputReader::new(self.finalize_output())
    }

    /// Computes BLAKE3 hash in parallel using Rayon divide-and-conquer tree engine.
    #[inline]
    pub fn hash_parallel(data: &[u8]) -> [u8; 32] {
        super::parallel::hash_parallel(data)
    }
}

impl io::Write for Blake3Hasher {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.update(buf);
        Ok(buf.len())
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl fmt::Debug for Blake3Hasher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Blake3Hasher")
            .field("total_chunks", &self.total_chunks)
            .field("count", &self.count())
            .field("flags", &self.flags)
            .field("tree_stack_len", &self.tree_stack.len())
            .field("chunk_buffered_len", &self.chunk_state.len())
            .finish()
    }
}

/// Standard type alias conforming to cryptography crate conventions.
pub type Hasher = Blake3Hasher;

/// Backward-compatible alias for [`Blake3Hasher`].
pub type Blake3 = Blake3Hasher;

// ============================================================================
// Top-Level Convenience Functions
// ============================================================================

/// Computes the default 32-byte BLAKE3 hash of an input slice in a single call.
#[inline]
pub fn hash(input: &[u8]) -> [u8; 32] {
    let mut hasher = Blake3Hasher::new();
    hasher.update(input);
    hasher.finalize()
}

/// Computes an extensible output function (XOF) stream reader from the given input data.
#[inline]
pub fn hash_xof(input: &[u8]) -> OutputReader {
    let mut hasher = Blake3Hasher::new();
    hasher.update(input);
    hasher.finalize_xof()
}

/// Computes a 32-byte keyed BLAKE3 MAC using the specified 256-bit key.
#[inline]
pub fn keyed_hash(key: &[u8; 32], input: &[u8]) -> [u8; 32] {
    let mut hasher = Blake3Hasher::new_keyed(key);
    hasher.update(input);
    hasher.finalize()
}

/// Derives a 32-byte subkey from the provided key material and application context string.
#[inline]
pub fn derive_key(context: &str, material: &[u8]) -> [u8; 32] {
    let mut hasher = Blake3Hasher::new_derive_key(context);
    hasher.update(material);
    hasher.finalize()
}

/// Backward-compatible one-shot BLAKE3 computation returning a 32-byte digest array.
#[inline]
pub fn blake3(input: &[u8]) -> [u8; 32] {
    hash(input)
}
