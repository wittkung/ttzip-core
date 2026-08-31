// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration and unit tests for BLAKE3 `ChunkState` 1024-byte chunk buffering,
//! micro-block state machine transitions, domain flag emissions, and oracle fidelity.

use ttzip_engine::crypto::blake3::{
    blake3, Blake3, ChunkState, BLAKE3_CHUNK_LEN, CHUNK_END, CHUNK_START, IV, KEYED_HASH,
};

/// Helper to generate deterministic test vectors as specified in the BLAKE3 specification
/// (repeating sequence of 251 bytes: 0, 1, 2, ..., 249, 250, 0, 1, ...).
fn generate_deterministic_input(len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    for (i, b) in buf.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
    buf
}

// ============================================================================
// 1. ChunkState Initial State and 0-Byte Input Invariants
// ============================================================================
#[test]
fn test_chunk_state_empty_0b() {
    let mut chunk = ChunkState::new(IV, 0, 0);

    assert_eq!(chunk.len(), 0);
    assert!(chunk.is_empty());
    assert_eq!(chunk.blocks_compressed, 0);
    assert_eq!(chunk.block_len, 0);
    assert_eq!(chunk.chunk_counter, 0);
    assert_eq!(chunk.start_flag(), CHUNK_START);

    // Consuming 0 bytes should return 0 and keep state intact
    let consumed = chunk.update(&[]);
    assert_eq!(consumed, 0);
    assert_eq!(chunk.len(), 0);
    assert!(chunk.is_empty());

    let output = chunk.output();
    assert_eq!(output.counter, 0);
    assert_eq!(output.block_len, 0);
    assert_eq!(output.flags, CHUNK_START | CHUNK_END);
    assert_eq!(output.input_chaining_value, IV);

    // Root hash of empty chunk must match official BLAKE3 empty NIST vector
    let root_hash = output.root_hash();
    assert_eq!(
        hex::encode(root_hash),
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
    );
}

// ============================================================================
// 2. Comprehensive Boundary Matrix: 0B, 1B, 63B, 64B, 65B, 512B, 1023B, 1024B
// ============================================================================
#[test]
fn test_chunk_state_boundary_matrix() {
    let test_cases = [
        (
            0usize,
            0u8,
            0u8,
            CHUNK_START | CHUNK_END,
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262",
        ),
        (
            1usize,
            0u8,
            1u8,
            CHUNK_START | CHUNK_END,
            "2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213",
        ),
        (
            2usize,
            0u8,
            2u8,
            CHUNK_START | CHUNK_END,
            "7b7015bb92cf0b318037702a6cdd81dee41224f734684c2c122cd6359cb1ee63",
        ),
        (
            63usize,
            0u8,
            63u8,
            CHUNK_START | CHUNK_END,
            "e9bc37a594daad83be9470df7f7b3798297c3d834ce80ba85d6e207627b7db7b",
        ),
        (
            64usize,
            0u8,
            64u8,
            CHUNK_START | CHUNK_END,
            "4eed7141ea4a5cd4b788606bd23f46e212af9cacebacdc7d1f4c6dc7f2511b98",
        ),
        (
            65usize,
            1u8,
            1u8,
            CHUNK_END,
            "de1e5fa0be70df6d2be8fffd0e99ceaa8eb6e8c93a63f2d8d1c30ecb6b263dee",
        ),
        (
            127usize,
            1u8,
            63u8,
            CHUNK_END,
            "d81293fda863f008c09e92fc382a81f5a0b4a1251cba1634016a0f86a6bd640d",
        ),
        (
            128usize,
            1u8,
            64u8,
            CHUNK_END,
            "f17e570564b26578c33bb7f44643f539624b05df1a76c81f30acd548c44b45ef",
        ),
        (
            129usize,
            2u8,
            1u8,
            CHUNK_END,
            "683aaae9f3c5ba37eaaf072aed0f9e30bac0865137bae68b1fde4ca2aebdcb12",
        ),
        (
            512usize,
            7u8,
            64u8,
            CHUNK_END,
            "",
        ),
        (
            1023usize,
            15u8,
            63u8,
            CHUNK_END,
            "10108970eeda3eb932baac1428c7a2163b0e924c9a9e25b35bba72b28f70bd11",
        ),
        (
            1024usize,
            15u8,
            64u8,
            CHUNK_END,
            "42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af7",
        ),
    ];

    for (size, expected_blocks_compressed, expected_block_len, expected_flags, expected_hex) in
        test_cases
    {
        let input = generate_deterministic_input(size);
        let mut chunk = ChunkState::new(IV, 0, 0);
        let consumed = chunk.update(&input);

        assert_eq!(consumed, size, "Failed to consume full buffer for size {size}");
        assert_eq!(chunk.len(), size, "Length mismatch for size {size}");
        assert_eq!(
            chunk.blocks_compressed, expected_blocks_compressed,
            "Blocks compressed mismatch for size {size}"
        );
        assert_eq!(
            chunk.block_len, expected_block_len,
            "Block len mismatch for size {size}"
        );

        let output = chunk.output();
        assert_eq!(
            output.flags, expected_flags,
            "Flag mismatch for size {size}: expected {:08b}, got {:08b}",
            expected_flags, output.flags
        );
        assert_eq!(
            output.block_len, expected_block_len,
            "Output block_len mismatch for size {size}"
        );

        let root_hash = output.root_hash();
        let reference_hash = blake3(&input);
        assert_eq!(
            root_hash, reference_hash,
            "Single chunk root hash must equal one-shot hash for size {size}"
        );

        if !expected_hex.is_empty() {
            assert_eq!(
                hex::encode(root_hash),
                expected_hex,
                "Hash mismatch against official test vector for size {size}"
            );
        }
    }
}

// ============================================================================
// 3. 1024-Byte Chunk Boundary Enforcement and Pause Invariants
// ============================================================================
#[test]
fn test_chunk_state_1024b_boundary_pause() {
    let large_input = generate_deterministic_input(2048);
    let mut chunk = ChunkState::new(IV, 0, 0);

    // Initial update with 2048 bytes must ingest exactly 1024 bytes and pause
    let consumed_1 = chunk.update(&large_input);
    assert_eq!(consumed_1, BLAKE3_CHUNK_LEN);
    assert_eq!(chunk.len(), BLAKE3_CHUNK_LEN);
    assert_eq!(chunk.blocks_compressed, 15);
    assert_eq!(chunk.block_len, 64);

    // Subsequent updates on a saturated chunk must consume 0 bytes
    let consumed_2 = chunk.update(&large_input[consumed_1..]);
    assert_eq!(consumed_2, 0);
    assert_eq!(chunk.len(), BLAKE3_CHUNK_LEN);

    let output = chunk.output();
    assert_eq!(output.flags, CHUNK_END);
    assert_eq!(output.counter, 0);
    assert_eq!(output.block_len, 64);

    let root_hash = output.root_hash();
    let expected_1024_hash = blake3(&large_input[..1024]);
    assert_eq!(root_hash, expected_1024_hash);
}

// ============================================================================
// 4. Incremental Update Fidelity and Arbitrary Step Sizes
// ============================================================================
#[test]
fn test_chunk_state_incremental_step_fidelity() {
    let target_sizes = [1usize, 63, 64, 65, 127, 128, 129, 255, 512, 1023, 1024];
    let step_sizes = [1usize, 2, 3, 7, 13, 17, 31, 32, 47, 63, 64, 128, 256];

    for &size in &target_sizes {
        let input = generate_deterministic_input(size);
        let expected_hash = blake3(&input);

        for &step in &step_sizes {
            let mut chunk = ChunkState::new(IV, 0, 0);
            let mut offset = 0;

            while offset < size {
                let chunk_slice = &input[offset..(offset + step).min(size)];
                let consumed = chunk.update(chunk_slice);
                assert_eq!(consumed, chunk_slice.len());
                offset += consumed;
            }

            assert_eq!(chunk.len(), size);
            let output = chunk.output();
            let root_hash = output.root_hash();

            assert_eq!(
                root_hash, expected_hash,
                "Incremental hash mismatch for size {size} with step {step}"
            );
        }
    }
}

// ============================================================================
// 5. Reset and State Reuse Across Chunks
// ============================================================================
#[test]
fn test_chunk_state_reset_and_reuse() {
    let mut chunk = ChunkState::new(IV, 0, 0);
    let chunk0_data = generate_deterministic_input(1024);

    let consumed0 = chunk.update(&chunk0_data);
    assert_eq!(consumed0, 1024);
    let cv0 = chunk.output().chaining_value();

    // Reset for chunk counter 1
    chunk.reset(IV, 1);
    assert_eq!(chunk.len(), 0);
    assert!(chunk.is_empty());
    assert_eq!(chunk.chunk_counter, 1);
    assert_eq!(chunk.blocks_compressed, 0);
    assert_eq!(chunk.block_len, 0);

    let chunk1_data = generate_deterministic_input(500);
    let consumed1 = chunk.update(&chunk1_data);
    assert_eq!(consumed1, 500);
    assert_eq!(chunk.len(), 500);
    assert_eq!(chunk.chunk_counter, 1);

    let output1 = chunk.output();
    assert_eq!(output1.counter, 1);
    assert_eq!(output1.block_len, (500 % 64) as u8);
    assert_eq!(chunk.blocks_compressed, (500 / 64) as u8);
    assert_ne!(output1.chaining_value(), cv0);
}

// ============================================================================
// 6. Domain Separation Flags Propagation: Keyed Hash & Derive Key
// ============================================================================
#[test]
fn test_chunk_state_keyed_hash_domain_flags() {
    let key_bytes: [u8; 32] = *b"whats the Elvish word for friend";
    let mut key_words = [0u32; 8];
    for i in 0..8 {
        key_words[i] = u32::from_le_bytes(key_bytes[i * 4..i * 4 + 4].try_into().unwrap());
    }

    // 0-byte keyed hash
    let chunk0 = ChunkState::new(key_words, 0, KEYED_HASH);
    let out0 = chunk0.output();
    assert_eq!(out0.flags, KEYED_HASH | CHUNK_START | CHUNK_END);
    assert_eq!(
        hex::encode(out0.root_hash()),
        "92b2b75604ed3c761f9d6f62392c8a9227ad0ea3f09573e783f1498a4ed60d26"
    );

    // 1-byte keyed hash
    let input1 = generate_deterministic_input(1);
    let mut chunk1 = ChunkState::new(key_words, 0, KEYED_HASH);
    chunk1.update(&input1);
    let out1 = chunk1.output();
    assert_eq!(out1.flags, KEYED_HASH | CHUNK_START | CHUNK_END);
    assert_eq!(
        hex::encode(out1.root_hash()),
        "6d7878dfff2f485635d39013278ae14f1454b8c0a3a2d34bc1ab38228a80c95b"
    );

    // 65-byte keyed hash (multi-block within chunk)
    let input65 = generate_deterministic_input(65);
    let mut chunk65 = ChunkState::new(key_words, 0, KEYED_HASH);
    chunk65.update(&input65);
    let out65 = chunk65.output();
    assert_eq!(out65.flags, KEYED_HASH | CHUNK_END);
    assert_eq!(
        hex::encode(out65.root_hash()),
        "c0a4edefa2d2accb9277c371ac12fcdbb52988a86edc54f0716e1591b4326e72"
    );
}

#[test]
fn test_chunk_state_derive_key_material_flags() {
    let mut derive_key_hasher =
        Blake3::new_derive_key("BLAKE3 2019-12-27 16:29:52 test vectors context");

    let input1024 = generate_deterministic_input(1024);
    derive_key_hasher.update(&input1024);
    let expected_derived = derive_key_hasher.finalize();

    assert_eq!(
        hex::encode(expected_derived),
        "7356cd7720d5b66b6d0697eb3177d9f8d73a4a5c5e968896eb6a689684302706"
    );
}

// ============================================================================
// 7. Micro-Block Cross Boundary Assertions
// ============================================================================
#[test]
fn test_chunk_state_microblock_crossing_exactness() {
    let mut chunk = ChunkState::new(IV, 0, 0);

    // Step 1: Ingest exactly 64 bytes
    let b64 = generate_deterministic_input(64);
    chunk.update(&b64);
    assert_eq!(chunk.blocks_compressed, 0);
    assert_eq!(chunk.block_len, 64);
    assert_eq!(chunk.len(), 64);

    // Step 2: Ingest 1 additional byte -> triggers compression of first block
    let b1 = [42u8];
    chunk.update(&b1);
    assert_eq!(chunk.blocks_compressed, 1);
    assert_eq!(chunk.block_len, 1);
    assert_eq!(chunk.len(), 65);

    // Step 3: Ingest 63 bytes to complete second block
    let b63 = generate_deterministic_input(63);
    chunk.update(&b63);
    assert_eq!(chunk.blocks_compressed, 1);
    assert_eq!(chunk.block_len, 64);
    assert_eq!(chunk.len(), 128);

    // Step 4: Ingest 1 additional byte -> triggers compression of second block
    chunk.update(&b1);
    assert_eq!(chunk.blocks_compressed, 2);
    assert_eq!(chunk.block_len, 1);
    assert_eq!(chunk.len(), 129);
}
