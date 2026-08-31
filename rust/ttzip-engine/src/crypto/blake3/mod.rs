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

pub mod chunk;
pub mod compress;
pub mod constants;
pub mod facade;
pub mod kdf;
pub mod neon;
pub mod output;
pub mod parallel;
pub mod tree;

pub use chunk::ChunkState;
pub use compress::*;
pub use constants::{
    BLAKE3_BLOCK_LEN, BLAKE3_CHUNK_LEN, BLAKE3_KEY_LEN, BLAKE3_OUT_LEN, BLOCK_LEN, CHUNK_END,
    CHUNK_LEN, CHUNK_START, DERIVE_KEY_CONTEXT, DERIVE_KEY_MATERIAL, IV, KEYED_HASH, KEY_LEN,
    MSG_SCHEDULE, OUT_LEN, PARENT, ROOT,
};
pub use facade::{
    blake3, derive_key, hash, hash_xof, keyed_hash, Blake3, Blake3Hasher, Hasher,
};
pub use kdf::{derive_key_into, derive_key_xof, new_derive_key, new_keyed};
pub use neon::{
    hash_many_neon, hash_many_parents_neon, hash_many_variable_chunks, hash_parents_neon,
    hash4_neon,
};
pub use output::{Output, OutputReader};
pub use parallel::{
    derive_key_parallel, hash_parallel, hash_subtree_rayon, hash_subtree_rayon_with_threshold,
    hash_subtree_serial, keyed_hash_parallel, ParallelTreeHasher, PARALLEL_THRESHOLD,
};
pub use tree::{
    left_subtree_len, parent_cv, parent_output, TreeStack, MAX_DEPTH, STACK_CAPACITY,
};

/// One-shot parallel BLAKE3 computation returning standard 32-byte digest.
#[inline]
pub fn blake3_parallel(data: &[u8]) -> [u8; 32] {
    parallel::hash_parallel(data)
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
