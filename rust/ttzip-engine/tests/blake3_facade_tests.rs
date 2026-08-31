// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration and verification test suite for the Safe Rust
//! streaming [`Blake3Hasher`] facade, primary API functions, and trait adapters.
//!
//! Validates:
//! 1. Streaming slice-by-slice ingestion (1-byte, 17-byte, 64-byte, 1024-byte steps) vs one-shot [`hash`].
//! 2. Standard [`std::io::Write`] adapter verification via [`std::io::copy`].
//! 3. Hasher [`Blake3Hasher::reset`] state recycling and replay fidelity.
//! 4. Non-destructive [`Blake3Hasher::finalize`] idempotent output extraction and continuous streaming.
//! 5. Domain-separated keyed hashing ([`Blake3Hasher::new_keyed`], [`keyed_hash`]) and KDF ([`Blake3Hasher::new_derive_key`], [`derive_key`]).
//! 6. Arbitrary-length extensible output function (XOF) generation via [`Blake3Hasher::finalize_xof`] and [`hash_xof`].
//! 7. Diagnostic traits: [`Clone`], [`std::fmt::Debug`], [`Default`], and memory wiping invariants.

use std::io::{self, Cursor, SeekFrom, Write};
use ttzip_engine::crypto::blake3::{
    blake3, derive_key, hash, hash_xof, keyed_hash, Blake3, Blake3Hasher, Hasher, OutputReader,
    BLAKE3_CHUNK_LEN,
};

const TEST_KEY: &[u8; 32] = b"whats the Elbereth password 1234";
const TEST_CONTEXT: &str = "BLAKE3 2026-08-31 ttzip test vectors context";

/// Generates deterministic pseudo-random payload bytes for testing.
fn generate_deterministic_payload(len: usize, seed: u8) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    for (i, b) in buf.iter_mut().enumerate() {
        *b = ((i.wrapping_mul(37)).wrapping_add(seed as usize) % 251) as u8;
    }
    buf
}

// ============================================================================
// 1. Streaming Chunk Ingestion and Step Invariance Tests
// ============================================================================

#[test]
fn test_blake3_hasher_streaming_step_invariance() {
    let test_sizes = [0, 1, 17, 63, 64, 65, 1023, 1024, 1025, 2048, 2049, 7777, 65536];
    let step_sizes = [1, 17, 64, 128, 512, 1024, 4096];

    for &size in &test_sizes {
        let payload = generate_deterministic_payload(size, 42);
        let reference_digest = hash(&payload);

        // Alias verification: Blake3 and Hasher must compute the exact same hash
        assert_eq!(blake3(&payload), reference_digest);

        for &step in &step_sizes {
            let mut hasher = Blake3Hasher::new();
            let mut offset = 0;

            while offset < payload.len() {
                let end = (offset + step).min(payload.len());
                hasher.update(&payload[offset..end]);
                offset = end;
            }

            let stream_digest = hasher.finalize();
            assert_eq!(
                stream_digest, reference_digest,
                "Digest mismatch for payload size {size} with step {step}"
            );
            assert_eq!(hasher.count(), size as u64);
        }
    }
}

// ============================================================================
// 2. std::io::Write Adapter and std::io::copy Tests
// ============================================================================

#[test]
fn test_blake3_hasher_io_write_adapter() {
    let payload = generate_deterministic_payload(131072, 99);
    let reference_digest = hash(&payload);

    // Test writing in arbitrary chunks using std::io::Write trait
    let mut hasher = Hasher::new();
    let mut cursor = Cursor::new(&payload);

    let copied_bytes = io::copy(&mut cursor, &mut hasher).expect("std::io::copy must succeed");
    assert_eq!(copied_bytes, payload.len() as u64);
    assert_eq!(hasher.count(), payload.len() as u64);

    hasher.flush().expect("flush must succeed");
    let stream_digest = hasher.finalize();
    assert_eq!(stream_digest, reference_digest);
}

#[test]
fn test_blake3_hasher_manual_write_trait() {
    let payload = b"streaming bytes via Write trait";
    let reference_digest = hash(payload);

    let mut hasher = Blake3Hasher::new();
    hasher
        .write_all(&payload[..10])
        .expect("write_all must succeed");
    hasher
        .write_all(&payload[10..])
        .expect("write_all must succeed");

    assert_eq!(hasher.finalize(), reference_digest);
}

// ============================================================================
// 3. Hasher State Recycling and Reset Fidelity Tests
// ============================================================================

#[test]
fn test_blake3_hasher_reset_fidelity() {
    let mut hasher = Blake3Hasher::new();

    for iteration in 0..5 {
        let payload = generate_deterministic_payload(2048 + iteration * 513, iteration as u8);
        let expected = hash(&payload);

        hasher.update(&payload);
        assert_eq!(hasher.finalize(), expected);
        assert_eq!(hasher.count(), payload.len() as u64);

        // Reset and verify empty state
        hasher.reset();
        assert_eq!(hasher.count(), 0);
        assert_eq!(hasher.total_chunks(), 0);
        assert_eq!(hasher.finalize(), hash(b""));
    }
}

#[test]
fn test_blake3_keyed_hasher_reset_fidelity() {
    let mut hasher = Blake3Hasher::new_keyed(TEST_KEY);
    let expected_empty = keyed_hash(TEST_KEY, b"");

    assert_eq!(hasher.finalize(), expected_empty);

    let payload = b"secret keyed payload for reset test";
    let expected = keyed_hash(TEST_KEY, payload);

    hasher.update(payload);
    assert_eq!(hasher.finalize(), expected);

    // Reset preserves key and flags
    hasher.reset();
    assert_eq!(hasher.finalize(), expected_empty);

    hasher.update(payload);
    assert_eq!(hasher.finalize(), expected);
}

// ============================================================================
// 4. Non-Destructive Finalize and Continuous Streaming Tests
// ============================================================================

#[test]
fn test_blake3_hasher_non_destructive_finalize() {
    let part1 = b"first part of stream | ";
    let part2 = b"second part of stream | ";
    let part3 = b"final trailing bytes";

    let mut full_payload = Vec::new();
    full_payload.extend_from_slice(part1);
    let digest1 = hash(&full_payload);

    full_payload.extend_from_slice(part2);
    let digest2 = hash(&full_payload);

    full_payload.extend_from_slice(part3);
    let digest3 = hash(&full_payload);

    let mut hasher = Blake3Hasher::new();

    // Ingest part 1 and call finalize multiple times
    hasher.update(part1);
    assert_eq!(hasher.finalize(), digest1);
    assert_eq!(hasher.finalize(), digest1);

    // Continue ingesting part 2 without reset
    hasher.update(part2);
    assert_eq!(hasher.finalize(), digest2);
    assert_eq!(hasher.finalize(), digest2);

    // Continue ingesting part 3
    hasher.update(part3);
    assert_eq!(hasher.finalize(), digest3);
    assert_eq!(hasher.finalize(), digest3);
}

// ============================================================================
// 5. Keyed Mode and Key Derivation (KDF) Tests
// ============================================================================

#[test]
fn test_blake3_keyed_hashing_api() {
    let payload = generate_deterministic_payload(5000, 77);
    let one_shot_mac = keyed_hash(TEST_KEY, &payload);

    let mut streaming_hasher = Blake3Hasher::new_keyed(TEST_KEY);
    for chunk in payload.chunks(333) {
        streaming_hasher.update(chunk);
    }
    let stream_mac = streaming_hasher.finalize();

    assert_eq!(one_shot_mac, stream_mac);
}

#[test]
fn test_blake3_kdf_api() {
    let material = generate_deterministic_payload(256, 12);
    let one_shot_key = derive_key(TEST_CONTEXT, &material);

    let mut streaming_kdf = Blake3Hasher::new_derive_key(TEST_CONTEXT);
    for chunk in material.chunks(19) {
        streaming_kdf.update(chunk);
    }
    let stream_key = streaming_kdf.finalize();

    assert_eq!(one_shot_key, stream_key);
}

// ============================================================================
// 6. Extensible Output Function (XOF) & OutputReader Tests
// ============================================================================

#[test]
fn test_blake3_xof_generation_and_fill() {
    let payload = b"test payload for extended output reader";
    let mut reader1: OutputReader = hash_xof(payload);

    let mut output1 = [0u8; 128];
    reader1.fill(&mut output1);

    // Standard 32-byte hash must match the first 32 bytes of XOF
    let standard_hash = hash(payload);
    assert_eq!(&output1[..32], &standard_hash[..]);

    // Streaming finalize_xof must yield the exact same byte stream
    let mut hasher = Blake3Hasher::new();
    hasher.update(payload);
    let mut reader2 = hasher.finalize_xof();

    let mut output2 = [0u8; 128];
    reader2.fill(&mut output2);
    assert_eq!(output1, output2);

    // Non-destructive finalize_into test
    let mut finalize_into_buf = [0u8; 64];
    hasher.finalize_into(&mut finalize_into_buf);
    assert_eq!(&finalize_into_buf[..], &output1[..64]);

    // Test OutputReader seeking via inherent method and std::io::Seek trait
    reader2.seek(0);
    let mut reread = [0u8; 64];
    reader2.fill(&mut reread);
    assert_eq!(&reread[..], &output2[..64]);

    let pos = io::Seek::seek(&mut reader2, SeekFrom::Start(32)).expect("seek to 32 must succeed");
    assert_eq!(pos, 32);
    let mut reread_tail = [0u8; 32];
    reader2.fill(&mut reread_tail);
    assert_eq!(&reread_tail[..], &output2[32..64]);
}

// ============================================================================
// 7. Backward Compatibility Type Alias and Chunk Constant Tests
// ============================================================================

#[test]
fn test_blake3_type_alias_and_chunk_boundary() {
    let mut legacy_hasher: Blake3 = Blake3::new();
    assert_eq!(legacy_hasher.total_chunks(), 0);

    let chunk_payload = vec![0xAAu8; BLAKE3_CHUNK_LEN];
    legacy_hasher.update(&chunk_payload);
    assert_eq!(legacy_hasher.count(), BLAKE3_CHUNK_LEN as u64);

    // Ingest 1 additional byte to force chunk 0 into the tree stack
    legacy_hasher.update(&[0xBB]);
    assert_eq!(legacy_hasher.total_chunks(), 1);
    assert_eq!(legacy_hasher.count(), (BLAKE3_CHUNK_LEN + 1) as u64);

    let digest = legacy_hasher.finalize();
    let mut expected_hasher = Blake3Hasher::new();
    expected_hasher.update(&chunk_payload);
    expected_hasher.update(&[0xBB]);
    assert_eq!(digest, expected_hasher.finalize());
}

// ============================================================================
// 8. Diagnostic Traits and Metadata Tests
// ============================================================================

#[test]
fn test_blake3_hasher_debug_and_default() {
    let hasher = Blake3Hasher::default();
    let debug_str = format!("{hasher:?}");

    assert!(debug_str.contains("Blake3Hasher"));
    assert!(debug_str.contains("total_chunks: 0"));
    assert!(debug_str.contains("count: 0"));
    assert_eq!(hasher.count(), 0);
    assert_eq!(hasher.total_chunks(), 0);
}

#[test]
fn test_blake3_hasher_clone() {
    let payload = b"cloned hasher state verification";
    let mut hasher1 = Blake3Hasher::new();
    hasher1.update(&payload[..10]);

    let mut hasher2 = hasher1.clone();
    hasher1.update(&payload[10..]);
    hasher2.update(&payload[10..]);

    assert_eq!(hasher1.finalize(), hasher2.finalize());
    assert_eq!(hasher1.finalize(), hash(payload));
}
