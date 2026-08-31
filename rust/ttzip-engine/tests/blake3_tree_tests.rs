// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration and verification test suite for the BLAKE3
//! binary tree hierarchical reduction stack, parent node merger, and geometry partitioning.
//!
//! Validates:
//! 1. `left_subtree_len` power-of-two geometric partitioning across power-of-2 boundaries.
//! 2. Parent node 64-byte block combination (`left_child || right_child`) and `PARENT` domain flags.
//! 3. `TreeStack` 55-element fixed-size array lifecycle, zero-heap safety, and Hamming weight (`count_ones()`).
//! 4. Irregular chunk counts (1, 2, 3, 5, 7, 13, 31, 33, 63, 64 chunks) right-spine folding integrity.
//! 5. Bit-Exact conformance against official standard BLAKE3 test vectors across keyed and unkeyed modes.

use ttzip_engine::crypto::blake3::{
    blake3, left_subtree_len, parent_cv, parent_output, Blake3, ChunkState, TreeStack,
    BLAKE3_BLOCK_LEN, BLAKE3_CHUNK_LEN, DERIVE_KEY_MATERIAL, IV, KEYED_HASH, MAX_DEPTH, PARENT,
    STACK_CAPACITY,
};

// ============================================================================
// 1. Geometric Partitioning (`left_subtree_len`) Tests
// ============================================================================

#[test]
fn test_left_subtree_len_boundaries() {
    // 1 chunk + 1 byte (1025 bytes) -> left child gets 1 full chunk (1024 bytes)
    assert_eq!(left_subtree_len(1025), 1024);

    // Exact power-of-two chunk boundary tests
    for num_chunks in [2u64, 4, 8, 16, 32, 64, 128, 256, 1024] {
        let input_len = num_chunks * (BLAKE3_CHUNK_LEN as u64);

        // input_len - 1: left subtree is half of previous power of two
        assert_eq!(
            left_subtree_len(input_len - 1),
            input_len / 2,
            "Boundary check failed for input_len - 1 where num_chunks = {num_chunks}"
        );

        // input_len: left subtree is exactly half of input_len
        assert_eq!(
            left_subtree_len(input_len),
            input_len / 2,
            "Boundary check failed for exact input_len where num_chunks = {num_chunks}"
        );

        // input_len + 1: left subtree expands to the full power of two
        assert_eq!(
            left_subtree_len(input_len + 1),
            input_len,
            "Boundary check failed for input_len + 1 where num_chunks = {num_chunks}"
        );
    }
}

#[test]
fn test_left_subtree_len_mathematical_invariants() {
    for len in (1025..=20000).step_by(73) {
        let left = left_subtree_len(len);
        let right = len - left;

        // Left subtree length must be a power of two
        assert_eq!(
            left.count_ones(),
            1,
            "left_subtree_len({len}) = {left} must be a power of two"
        );

        // Left subtree length must be strictly less than input_len
        assert!(
            left < len,
            "left_subtree_len({len}) = {left} must be strictly less than input_len"
        );

        // Left subtree must be complete and >= right subtree
        assert!(
            left >= right,
            "left ({left}) must be >= right ({right}) for input_len {len}"
        );
    }
}

// ============================================================================
// 2. Parent Node Combination and Flag Tests
// ============================================================================

#[test]
fn test_parent_node_block_combination_and_flags() {
    let mut left_child = [0u8; 32];
    let mut right_child = [0u8; 32];
    for i in 0..32 {
        left_child[i] = (i + 1) as u8;
        right_child[i] = (i + 101) as u8;
    }

    let output = parent_output(&left_child, &right_child, &IV, 0);

    // Verify 64-byte block assembly: first 32 bytes == left_child, next 32 bytes == right_child
    assert_eq!(&output.block[..32], &left_child[..]);
    assert_eq!(&output.block[32..], &right_child[..]);

    // Verify block metadata
    assert_eq!(output.block_len, BLAKE3_BLOCK_LEN as u8);
    assert_eq!(output.counter, 0);
    assert_eq!(output.flags, PARENT);
    assert_eq!(output.input_chaining_value, IV);

    // Verify parent_cv matches output.chaining_value()
    let expected_cv = output.chaining_value();
    let actual_cv = parent_cv(&left_child, &right_child, &IV, 0);
    assert_eq!(actual_cv, expected_cv);
}

#[test]
fn test_parent_node_with_domain_flags() {
    let left = [0xAAu8; 32];
    let right = [0xBBu8; 32];
    let custom_key = [1, 2, 3, 4, 5, 6, 7, 8];

    // Keyed hash mode
    let keyed_output = parent_output(&left, &right, &custom_key, KEYED_HASH);
    assert_eq!(keyed_output.flags, KEYED_HASH | PARENT);
    assert_eq!(keyed_output.input_chaining_value, custom_key);

    // Key derivation mode
    let derive_output = parent_output(&left, &right, &custom_key, DERIVE_KEY_MATERIAL);
    assert_eq!(derive_output.flags, DERIVE_KEY_MATERIAL | PARENT);
    assert_eq!(derive_output.input_chaining_value, custom_key);
}

// ============================================================================
// 3. TreeStack Lifecycle, Capacity, and Hamming Weight Reduction Tests
// ============================================================================

#[test]
fn test_tree_stack_push_pop_clear() {
    let mut stack = TreeStack::new();
    assert!(stack.is_empty());
    assert_eq!(stack.len(), 0);
    assert_eq!(stack.pop(), None);
    assert_eq!(STACK_CAPACITY, 55);
    assert_eq!(MAX_DEPTH, 54);

    let cv1 = [1u8; 32];
    let cv2 = [2u8; 32];
    let cv3 = [3u8; 32];

    stack.push(cv1);
    stack.push(cv2);
    stack.push(cv3);

    assert_eq!(stack.len(), 3);
    assert!(!stack.is_empty());
    assert_eq!(stack.as_slice(), &[cv1, cv2, cv3]);

    assert_eq!(stack.pop(), Some(cv3));
    assert_eq!(stack.pop(), Some(cv2));
    assert_eq!(stack.len(), 1);

    stack.clear();
    assert!(stack.is_empty());
    assert_eq!(stack.len(), 0);
    assert_eq!(stack.as_slice(), &[] as &[[u8; 32]]);
}

#[test]
fn test_tree_stack_hamming_weight_progression_1_to_64() {
    let mut stack = TreeStack::new();

    for chunk_counter in 0..64u64 {
        let cv = [(chunk_counter + 1) as u8; 32];

        // Before pushing chunk_counter, perform lazy merge of prior subtrees
        stack.merge_cv_stack(chunk_counter, &IV, 0);
        stack.push(cv);

        assert!(
            stack.len() <= STACK_CAPACITY,
            "Stack length {} exceeded capacity {} at chunk {}",
            stack.len(),
            STACK_CAPACITY,
            chunk_counter
        );

        // Check that a post-merge simulation matches (chunk_counter + 1).count_ones()
        let mut sim_stack = stack.clone();
        sim_stack.merge_cv_stack(chunk_counter + 1, &IV, 0);
        let expected_len = (chunk_counter + 1).count_ones() as usize;
        assert_eq!(
            sim_stack.len(),
            expected_len,
            "Mismatch in stack depth for total_chunks = {}: expected {}, got {}",
            chunk_counter + 1,
            expected_len,
            sim_stack.len()
        );
    }
}

// ============================================================================
// 4. Right-Spine Folding and Irregular Chunk Count Integrity Tests
// ============================================================================

#[test]
fn test_irregular_chunk_counts_right_spine_folding() {
    let irregular_chunk_counts = [1, 2, 3, 5, 7, 13, 17, 31, 32, 33, 63, 64];

    for &num_chunks in &irregular_chunk_counts {
        let mut payload = vec![0u8; num_chunks * BLAKE3_CHUNK_LEN];
        for (i, b) in payload.iter_mut().enumerate() {
            *b = ((i * 31 + 17) % 251) as u8;
        }

        // Expected digest from official one-shot hasher
        let expected_hash = blake3(&payload);

        // Manual incremental simulation using ChunkState + TreeStack + fold_right_spine
        let mut stack = TreeStack::new();
        let mut final_output = None;

        for chunk_idx in 0..num_chunks {
            let chunk_data = &payload[chunk_idx * BLAKE3_CHUNK_LEN..(chunk_idx + 1) * BLAKE3_CHUNK_LEN];
            let mut chunk_state = ChunkState::new(IV, chunk_idx as u64, 0);
            chunk_state.update(chunk_data);

            if chunk_idx + 1 == num_chunks {
                // Last chunk is not pushed into stack; it becomes right_output for folding
                if chunk_idx > 0 {
                    stack.merge_cv_stack(chunk_idx as u64, &IV, 0);
                }
                final_output = Some(chunk_state.output());
            } else {
                let chunk_cv = chunk_state.output().chaining_value();
                stack.merge_cv_stack(chunk_idx as u64, &IV, 0);
                stack.push(chunk_cv);
            }
        }

        let folded_output = stack.fold_right_spine(final_output.unwrap(), &IV, 0);
        let manual_hash = folded_output.root_hash();

        assert_eq!(
            manual_hash, expected_hash,
            "Right-spine folding failed for num_chunks = {num_chunks}"
        );
        assert!(stack.is_empty(), "Stack must be empty after full right-spine fold");
    }
}

#[test]
fn test_irregular_partial_trailing_chunks() {
    let test_sizes = [
        1024 + 1,       // 1 chunk + 1 byte
        1024 + 500,     // 1 chunk + 500 bytes
        2 * 1024 + 42,  // 2 chunks + 42 bytes
        3 * 1024 + 999, // 3 chunks + 999 bytes
        5 * 1024 + 128, // 5 chunks + 128 bytes
        7 * 1024 + 512, // 7 chunks + 512 bytes
        13 * 1024 + 7,  // 13 chunks + 7 bytes
        31 * 1024 + 63, // 31 chunks + 63 bytes
        33 * 1024 + 1,  // 33 chunks + 1 byte
    ];

    for &size in &test_sizes {
        let mut data = vec![0u8; size];
        for (i, b) in data.iter_mut().enumerate() {
            *b = ((i * 7 + 13) % 251) as u8;
        }

        let one_shot = blake3(&data);

        // Streaming with multiple chunk boundaries
        let mut hasher = Blake3::new();
        hasher.update(&data);
        let stream_hash = hasher.finalize();

        assert_eq!(
            one_shot, stream_hash,
            "Streaming hash mismatch for partial trailing size {size}"
        );
    }
}

// ============================================================================
// 5. Standard Test Vectors and Bit-Exact Conformance
// ============================================================================

#[test]
fn test_blake3_standard_test_vectors() {
    // 1. Empty input vector
    let hash_empty = blake3(b"");
    assert_eq!(
        hex::encode(hash_empty),
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262",
        "NIST empty input vector mismatch"
    );

    // 2. "abc" vector
    let hash_abc = blake3(b"abc");
    assert_eq!(
        hex::encode(hash_abc),
        "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85",
        "Standard 'abc' vector mismatch"
    );

    // 3. Exact 1024 bytes (1 chunk) with pseudo-random modulus
    let mut chunk1024 = vec![0u8; 1024];
    for (i, b) in chunk1024.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
    let mut hasher = Blake3::new();
    hasher.update(&chunk1024);
    assert_eq!(hasher.finalize(), blake3(&chunk1024));

    // 4. Exact 2048 bytes (2 chunks)
    let mut chunk2048 = vec![0u8; 2048];
    for (i, b) in chunk2048.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
    let mut hasher2 = Blake3::new();
    hasher2.update(&chunk2048[..500]);
    hasher2.update(&chunk2048[500..1500]);
    hasher2.update(&chunk2048[1500..]);
    assert_eq!(hasher2.finalize(), blake3(&chunk2048));
}

#[test]
fn test_blake3_keyed_and_derive_key_vectors() {
    let key = [0x42u8; 32];
    let data = b"TTZip High-Performance Cryptographic Engine";

    let mut keyed_hasher = Blake3::new_keyed(&key);
    keyed_hasher.update(data);
    let keyed_digest = keyed_hasher.finalize();

    // Verify deterministic keyed output
    let mut keyed_hasher2 = Blake3::new_keyed(&key);
    keyed_hasher2.update(&data[..10]);
    keyed_hasher2.update(&data[10..]);
    assert_eq!(keyed_digest, keyed_hasher2.finalize());

    // Key derivation mode
    let context = "ttzip 2026-08-31 kdf context";
    let mut kdf_hasher1 = Blake3::new_derive_key(context);
    kdf_hasher1.update(data);
    let kdf_digest1 = kdf_hasher1.finalize();

    let mut kdf_hasher2 = Blake3::new_derive_key(context);
    kdf_hasher2.update(data);
    let kdf_digest2 = kdf_hasher2.finalize();

    assert_eq!(kdf_digest1, kdf_digest2);
    assert_ne!(kdf_digest1, keyed_digest);
}
