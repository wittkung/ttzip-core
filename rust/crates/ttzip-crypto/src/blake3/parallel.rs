// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! BLAKE3 multi-threaded divide-and-conquer parallel tree hasher engine.
//!
//! Provides the [`ParallelTreeHasher`] and work-stealing parallel subtree evaluation
//! via Rayon, enabling high-throughput multi-core hashing for multi-megabyte and multi-gigabyte
//! data streams while maintaining Bit-Exact conformance with single-threaded BLAKE3.

use zeroize::{Zeroize, ZeroizeOnDrop};

use super::chunk::ChunkState;
use super::constants::{
    CHUNK_LEN, DERIVE_KEY_CONTEXT, DERIVE_KEY_MATERIAL, IV, KEYED_HASH,
};
use super::output::{Output, OutputReader};
#[cfg(target_arch = "aarch64")]
use super::tree::parent_cv;
use super::tree::{left_subtree_len, parent_output};

/// The default parallel recursion threshold in bytes (64 KiB = 64 Chunks).
///
/// Inputs or subtrees with size strictly less than `PARALLEL_THRESHOLD` are evaluated
/// on the current thread using fast 4-way NEON SIMD leaf batching and serial tree reduction
/// to eliminate Rayon scheduling overhead on small subtrees.
pub const PARALLEL_THRESHOLD: usize = 64 * CHUNK_LEN;

/// Recursively computes the BLAKE3 subtree [`Output`] descriptor using Rayon parallel divide-and-conquer.
///
/// For subtrees at or above [`PARALLEL_THRESHOLD`], `rayon::join` is dispatched to evaluate left and right
/// subtrees concurrently on worker threads. Below the threshold, recursion continues serially with 4-way NEON batching.
pub fn hash_subtree_rayon(
    input: &[u8],
    key: &[u32; 8],
    chunk_counter: u64,
    flags: u8,
) -> Output {
    hash_subtree_rayon_with_threshold(input, key, chunk_counter, flags, PARALLEL_THRESHOLD)
}

/// Recursively computes the BLAKE3 subtree [`Output`] with an explicit parallel threshold cutoff.
pub fn hash_subtree_rayon_with_threshold(
    input: &[u8],
    key: &[u32; 8],
    chunk_counter: u64,
    flags: u8,
    threshold: usize,
) -> Output {
    if input.len() <= CHUNK_LEN {
        let mut chunk_state = ChunkState::new(*key, chunk_counter, flags);
        chunk_state.update(input);
        return chunk_state.output();
    }

    if input.len() < threshold || (input.len() == 4 * CHUNK_LEN && threshold >= 4 * CHUNK_LEN) {
        return hash_chunks_parallel(input, key, chunk_counter, flags);
    }

    let left_len = left_subtree_len(input.len() as u64) as usize;
    let (left, right) = input.split_at(left_len);
    let right_chunk_counter = chunk_counter + (left_len / CHUNK_LEN) as u64;

    let (left_output, right_output) = rayon::join(
        || hash_subtree_rayon_with_threshold(left, key, chunk_counter, flags, threshold),
        || hash_subtree_rayon_with_threshold(right, key, right_chunk_counter, flags, threshold),
    );

    let left_cv = left_output.chaining_value();
    let right_cv = right_output.chaining_value();
    parent_output(&left_cv, &right_cv, key, flags)
}

/// Evaluates a leaf subtree on the current thread, dispatching 4-chunk contiguous batches
/// through 4-way ARM64 NEON SIMD (`hash_many_neon`) when available.
pub fn hash_chunks_parallel(
    input: &[u8],
    key: &[u32; 8],
    chunk_counter: u64,
    flags: u8,
) -> Output {
    if input.len() <= CHUNK_LEN {
        let mut chunk_state = ChunkState::new(*key, chunk_counter, flags);
        chunk_state.update(input);
        return chunk_state.output();
    }

    #[cfg(target_arch = "aarch64")]
    if input.len() == 4 * CHUNK_LEN
        && let (Some(c0), Some(c1), Some(c2), Some(c3)) = (
            input.get(0..CHUNK_LEN).and_then(|s| s.first_chunk::<CHUNK_LEN>()),
            input
                .get(CHUNK_LEN..2 * CHUNK_LEN)
                .and_then(|s| s.first_chunk::<CHUNK_LEN>()),
            input
                .get(2 * CHUNK_LEN..3 * CHUNK_LEN)
                .and_then(|s| s.first_chunk::<CHUNK_LEN>()),
            input
                .get(3 * CHUNK_LEN..4 * CHUNK_LEN)
                .and_then(|s| s.first_chunk::<CHUNK_LEN>()),
        )
    {
        let mut cvs = [[0u8; 32]; 4];
        super::neon::hash_many_neon(&[c0, c1, c2, c3], key, chunk_counter, flags, &mut cvs);
        let left_cv = parent_cv(&cvs[0], &cvs[1], key, flags);
        let right_cv = parent_cv(&cvs[2], &cvs[3], key, flags);
        return parent_output(&left_cv, &right_cv, key, flags);
    }

    let left_len = left_subtree_len(input.len() as u64) as usize;
    let (left, right) = input.split_at(left_len);
    let right_chunk_counter = chunk_counter + (left_len / CHUNK_LEN) as u64;

    let left_output = hash_chunks_parallel(left, key, chunk_counter, flags);
    let right_output = hash_chunks_parallel(right, key, right_chunk_counter, flags);

    let left_cv = left_output.chaining_value();
    let right_cv = right_output.chaining_value();
    parent_output(&left_cv, &right_cv, key, flags)
}

/// Recursively computes the BLAKE3 subtree [`Output`] descriptor serially on the calling thread.
#[inline]
pub fn hash_subtree_serial(
    input: &[u8],
    key: &[u32; 8],
    chunk_counter: u64,
    flags: u8,
) -> Output {
    hash_chunks_parallel(input, key, chunk_counter, flags)
}

/// High-throughput multi-threaded BLAKE3 tree hasher.
#[derive(Clone, Debug, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct ParallelTreeHasher {
    key: [u32; 8],
    flags: u8,
    threshold: usize,
}

impl Default for ParallelTreeHasher {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl ParallelTreeHasher {
    /// Creates a new `ParallelTreeHasher` with standard IV and default parallel threshold.
    #[inline]
    pub const fn new() -> Self {
        Self::new_internal(IV, 0)
    }

    /// Creates a new `ParallelTreeHasher` for keyed hashing mode.
    pub fn new_keyed(key: &[u8; 32]) -> Self {
        let mut key_words = [0u32; 8];
        for i in 0..8 {
            key_words[i] = u32::from_le_bytes([
                key[i * 4],
                key[i * 4 + 1],
                key[i * 4 + 2],
                key[i * 4 + 3],
            ]);
        }
        Self::new_internal(key_words, KEYED_HASH)
    }

    /// Creates a new `ParallelTreeHasher` for key derivation mode.
    pub fn new_derive_key(context: &str) -> Self {
        let context_output = hash_subtree_serial(context.as_bytes(), &IV, 0, DERIVE_KEY_CONTEXT);
        let context_key = context_output.root_hash();

        let mut key_words = [0u32; 8];
        for i in 0..8 {
            key_words[i] = u32::from_le_bytes([
                context_key[i * 4],
                context_key[i * 4 + 1],
                context_key[i * 4 + 2],
                context_key[i * 4 + 3],
            ]);
        }
        Self::new_internal(key_words, DERIVE_KEY_MATERIAL)
    }

    /// Creates a hasher with explicit key words and domain flags.
    #[inline]
    pub const fn new_internal(key: [u32; 8], flags: u8) -> Self {
        Self {
            key,
            flags,
            threshold: PARALLEL_THRESHOLD,
        }
    }

    /// Sets a custom parallel threshold in bytes.
    #[inline]
    pub fn with_threshold(mut self, threshold: usize) -> Self {
        self.threshold = threshold;
        self
    }

    /// Returns the current parallel threshold in bytes.
    #[inline]
    pub const fn threshold(&self) -> usize {
        self.threshold
    }

    /// Returns the active 8-word key / initial chaining value.
    #[inline]
    pub const fn key(&self) -> &[u32; 8] {
        &self.key
    }

    /// Returns the active domain separation flags.
    #[inline]
    pub const fn flags(&self) -> u8 {
        self.flags
    }

    /// Computes the root [`Output`] descriptor of the input in parallel.
    #[inline]
    pub fn hash_output(&self, input: &[u8]) -> Output {
        hash_subtree_rayon_with_threshold(input, &self.key, 0, self.flags, self.threshold)
    }

    /// Computes the 32-byte BLAKE3 root hash digest in parallel.
    #[inline]
    pub fn hash(&self, input: &[u8]) -> [u8; 32] {
        self.hash_output(input).root_hash()
    }

    /// Returns an [`OutputReader`] providing arbitrary length seeking and streaming byte extraction.
    #[inline]
    pub fn hash_xof(&self, input: &[u8]) -> OutputReader {
        OutputReader::new(self.hash_output(input))
    }

    /// Computes the BLAKE3 hash and fills the destination slice with arbitrary output length.
    #[inline]
    pub fn hash_into(&self, input: &[u8], out: &mut [u8]) {
        self.hash_xof(input).fill(out);
    }
}

/// One-shot parallel BLAKE3 computation returning standard 32-byte digest.
#[inline]
pub fn hash_parallel(input: &[u8]) -> [u8; 32] {
    ParallelTreeHasher::new().hash(input)
}

/// One-shot parallel keyed BLAKE3 computation returning standard 32-byte MAC.
#[inline]
pub fn keyed_hash_parallel(key: &[u8; 32], input: &[u8]) -> [u8; 32] {
    ParallelTreeHasher::new_keyed(key).hash(input)
}

/// One-shot parallel key derivation returning standard 32-byte derived key.
#[inline]
pub fn derive_key_parallel(context: &str, material: &[u8]) -> [u8; 32] {
    ParallelTreeHasher::new_derive_key(context).hash(material)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blake3::facade::blake3;

    #[test]
    fn test_parallel_threshold_is_64_kib_and_neon_leaf_parity() {
        assert_eq!(PARALLEL_THRESHOLD, 64 * CHUNK_LEN);

        for &len in &[
            0usize,
            1024,
            4 * CHUNK_LEN,
            4 * CHUNK_LEN + 17,
            8 * CHUNK_LEN,
            16 * CHUNK_LEN,
            64 * CHUNK_LEN,
            128 * CHUNK_LEN + 513,
        ] {
            let data: Vec<u8> = (0..len).map(|i| ((i * 37 + 11) % 251) as u8).collect();
            let expected = blake3(&data);
            let actual_parallel = hash_parallel(&data);
            let actual_leaf = hash_chunks_parallel(&data, &IV, 0, 0).root_hash();
            assert_eq!(actual_parallel, expected, "parallel mismatch at len={len}");
            assert_eq!(actual_leaf, expected, "leaf mismatch at len={len}");
        }
    }
}
