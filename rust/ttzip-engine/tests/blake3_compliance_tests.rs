// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Official BLAKE3 cryptographic compliance and test vector verification suite.
//!
//! Validates bit-exact compliance with the official BLAKE3 specifications across:
//! 1. Standard Hash (default IV mode)
//! 2. Keyed Hash (MAC mode with 32-byte key)
//! 3. Key Derivation (KDF mode with context string)
//! 4. 131-byte extended XOF outputs and 32-byte prefix consistency theorem
//! 5. 37 critical physical boundary dimensions (micro-extremes, block/chunk boundaries, SIMD degree ladders, deep subtrees)

use serde::Deserialize;
use std::io::{Read, Seek, SeekFrom};
use ttzip_engine::crypto::blake3::{
    blake3, derive_key, derive_key_into, derive_key_parallel, derive_key_xof, hash, hash_parallel,
    hash_xof, keyed_hash, keyed_hash_parallel, new_derive_key, new_keyed, Blake3, Hasher,
    OutputReader,
};

/// The official 32-byte key for BLAKE3 keyed hashing test vectors.
const TEST_KEY: &[u8; 32] = b"whats the Elvish word for friend";

/// The official context string for BLAKE3 key derivation test vectors.
const DERIVE_KEY_CONTEXT: &str = "BLAKE3 2019-12-27 16:29:52 test vectors context";

/// Official matrix of 37 critical physical boundary dimensions.
const CRITICAL_BOUNDARY_SIZES: &[usize] = &[
    // Micro extremes (0B..=8B)
    0, 1, 2, 3, 4, 5, 6, 7, 8,
    // Single block boundary (BLOCK_LEN = 64)
    63, 64, 65,
    // Double block boundary (2 * BLOCK_LEN = 128)
    127, 128, 129,
    // Single chunk boundary (CHUNK_LEN = 1024)
    1023, 1024, 1025,
    // Multi-chunk tree merge boundaries
    2048, 2049, 3072, 3073, 4096, 4097,
    // SIMD degree step ladders (NEON / AVX2 / AVX512 lanes)
    5120, 5121, 6144, 6145, 7168, 7169, 8192, 8193,
    // Deep subtree crossing and power-of-two buffer boundaries
    16383, 16384, 31744, 65536, 102400,
];

/// Generates a deterministic test vector payload using prime modulus 251.
///
/// `buf[i] = (i % 251) as u8`
/// Modulus 251 is the largest prime less than 256, ensuring permutation
/// sensitivity across adjacent blocks and chunk boundaries.
fn paint_test_input(len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    for (i, b) in buf.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
    buf
}

/// JSON test case schema representing a single official test vector entry.
#[derive(Deserialize)]
struct TestCase {
    input_len: usize,
    hash: String,
    keyed_hash: String,
    derive_key: String,
}

/// Root JSON structure of the official BLAKE3 test vectors file.
#[derive(Deserialize)]
struct TestVectorsFile {
    key: String,
    context_string: String,
    cases: Vec<TestCase>,
}

// ============================================================================
// 1. Bit-Exact Conformance Against Official Test Vectors (All 3 Modes & XOF)
// ============================================================================

#[test]
fn test_official_test_vectors_35_cases_bit_exact() {
    let json_str = include_str!("../../../../vendor/BLAKE3/test_vectors/test_vectors.json");
    let test_file: TestVectorsFile =
        serde_json::from_str(json_str).expect("Valid official BLAKE3 test_vectors.json format");

    assert_eq!(
        test_file.key.as_bytes(),
        TEST_KEY,
        "JSON key string must match official test key"
    );
    assert_eq!(
        test_file.context_string, DERIVE_KEY_CONTEXT,
        "JSON context string must match official derive_key context"
    );

    let key_bytes: &[u8; 32] = test_file
        .key
        .as_bytes()
        .try_into()
        .expect("32-byte key slice");
    let context = &test_file.context_string;

    for case in &test_file.cases {
        let input = paint_test_input(case.input_len);
        let expected_hash_xof = hex::decode(&case.hash).expect("valid hex hash");
        let expected_keyed_xof = hex::decode(&case.keyed_hash).expect("valid hex keyed_hash");
        let expected_derive_xof = hex::decode(&case.derive_key).expect("valid hex derive_key");

        // Verify official extended output length is 131 bytes
        assert_eq!(
            expected_hash_xof.len(),
            131,
            "Expected hash XOF length is 131 bytes"
        );
        assert_eq!(
            expected_keyed_xof.len(),
            131,
            "Expected keyed_hash XOF length is 131 bytes"
        );
        assert_eq!(
            expected_derive_xof.len(),
            131,
            "Expected derive_key XOF length is 131 bytes"
        );

        // --------------------------------------------------------------------
        // Mode 1: Standard Hash
        // --------------------------------------------------------------------
        // A. One-shot standard hash (32-byte digest)
        let hash_oneshot = blake3(&input);
        assert_eq!(
            &hash_oneshot[..],
            &expected_hash_xof[..32],
            "Standard hash one-shot 32B digest mismatch for len {}",
            case.input_len
        );
        assert_eq!(
            hash(&input),
            hash_oneshot,
            "hash() facade must match blake3() for len {}",
            case.input_len
        );

        // B. Incremental Hasher (32-byte digest)
        let mut hasher_std = Hasher::new();
        hasher_std.update(&input);
        assert_eq!(
            hasher_std.finalize(),
            hash_oneshot,
            "Hasher::finalize() must match one-shot for len {}",
            case.input_len
        );

        // C. Full 131-byte XOF expansion via OutputReader
        let mut hasher_xof = Hasher::new();
        hasher_xof.update(&input);
        let mut std_xof_out = vec![0u8; 131];
        hasher_xof.finalize_xof().fill(&mut std_xof_out);
        assert_eq!(
            std_xof_out, expected_hash_xof,
            "Standard hash 131B XOF mismatch for len {}",
            case.input_len
        );

        // D. Facade hash_xof direct expansion
        let mut facade_xof_out = vec![0u8; 131];
        hash_xof(&input).fill(&mut facade_xof_out);
        assert_eq!(
            facade_xof_out, expected_hash_xof,
            "hash_xof facade 131B mismatch for len {}",
            case.input_len
        );

        // --------------------------------------------------------------------
        // Mode 2: Keyed Hash (MAC)
        // --------------------------------------------------------------------
        // A. One-shot keyed hash (32-byte digest)
        let keyed_oneshot = keyed_hash(key_bytes, &input);
        assert_eq!(
            &keyed_oneshot[..],
            &expected_keyed_xof[..32],
            "Keyed hash one-shot 32B digest mismatch for len {}",
            case.input_len
        );

        // B. Incremental Keyed Hasher
        let mut hasher_keyed = new_keyed(key_bytes);
        hasher_keyed.update(&input);
        assert_eq!(
            hasher_keyed.finalize(),
            keyed_oneshot,
            "new_keyed finalize must match one-shot for len {}",
            case.input_len
        );

        // C. Full 131-byte XOF expansion for Keyed Hash
        let mut keyed_xof_reader = new_keyed(key_bytes);
        keyed_xof_reader.update(&input);
        let mut keyed_xof_out = vec![0u8; 131];
        keyed_xof_reader.finalize_xof().fill(&mut keyed_xof_out);
        assert_eq!(
            keyed_xof_out, expected_keyed_xof,
            "Keyed hash 131B XOF mismatch for len {}",
            case.input_len
        );

        // --------------------------------------------------------------------
        // Mode 3: Key Derivation (KDF)
        // --------------------------------------------------------------------
        // A. One-shot derive_key (32-byte subkey)
        let derive_oneshot = derive_key(context, &input);
        assert_eq!(
            &derive_oneshot[..],
            &expected_derive_xof[..32],
            "derive_key one-shot 32B subkey mismatch for len {}",
            case.input_len
        );

        // B. Incremental KDF Hasher
        let mut hasher_derive = new_derive_key(context);
        hasher_derive.update(&input);
        assert_eq!(
            hasher_derive.finalize(),
            derive_oneshot,
            "new_derive_key finalize must match one-shot for len {}",
            case.input_len
        );

        // C. Full 131-byte XOF expansion via derive_key_xof
        let mut derive_xof_out = vec![0u8; 131];
        derive_key_xof(context, &input).fill(&mut derive_xof_out);
        assert_eq!(
            derive_xof_out, expected_derive_xof,
            "derive_key_xof 131B XOF mismatch for len {}",
            case.input_len
        );

        // D. Direct buffer extraction via derive_key_into
        let mut derive_into_out = vec![0u8; 131];
        derive_key_into(context, &input, &mut derive_into_out);
        assert_eq!(
            derive_into_out, expected_derive_xof,
            "derive_key_into 131B output mismatch for len {}",
            case.input_len
        );

        // --------------------------------------------------------------------
        // Prefix Consistency Theorem Verification (XOF[..32] == OneShot)
        // --------------------------------------------------------------------
        assert_eq!(
            &std_xof_out[..32],
            &hash_oneshot[..],
            "Standard hash prefix consistency failed for len {}",
            case.input_len
        );
        assert_eq!(
            &keyed_xof_out[..32],
            &keyed_oneshot[..],
            "Keyed hash prefix consistency failed for len {}",
            case.input_len
        );
        assert_eq!(
            &derive_xof_out[..32],
            &derive_oneshot[..],
            "Derive key prefix consistency failed for len {}",
            case.input_len
        );
    }
}

// ============================================================================
// 2. 37 Critical Physical Boundary Dimensions Matrix Verification
// ============================================================================

#[test]
fn test_37_critical_boundary_sizes_matrix_streaming_and_reset() {
    assert_eq!(
        CRITICAL_BOUNDARY_SIZES.len(),
        37,
        "Matrix must contain exactly 37 critical physical boundary dimensions"
    );

    for &len in CRITICAL_BOUNDARY_SIZES {
        let input = paint_test_input(len);

        // 1. Standard Hash Equivalence
        let std_oneshot = blake3(&input);
        let mut std_hasher = Blake3::new();
        std_hasher.update(&input);
        assert_eq!(
            std_hasher.finalize(),
            std_oneshot,
            "Blake3 incremental must match one-shot for len {}",
            len
        );

        // Hasher reset fidelity
        std_hasher.reset();
        std_hasher.update(&input);
        assert_eq!(
            std_hasher.finalize(),
            std_oneshot,
            "Blake3 reset must yield identical digest for len {}",
            len
        );

        // 2. Keyed Hash Equivalence
        let keyed_oneshot = keyed_hash(TEST_KEY, &input);
        let mut keyed_hasher = new_keyed(TEST_KEY);
        keyed_hasher.update(&input);
        assert_eq!(
            keyed_hasher.finalize(),
            keyed_oneshot,
            "Keyed hasher incremental must match one-shot for len {}",
            len
        );

        keyed_hasher.reset();
        keyed_hasher.update(&input);
        assert_eq!(
            keyed_hasher.finalize(),
            keyed_oneshot,
            "Keyed hasher reset must yield identical MAC for len {}",
            len
        );

        // 3. Key Derivation Equivalence
        let derive_oneshot = derive_key(DERIVE_KEY_CONTEXT, &input);
        let mut derive_hasher = new_derive_key(DERIVE_KEY_CONTEXT);
        derive_hasher.update(&input);
        assert_eq!(
            derive_hasher.finalize(),
            derive_oneshot,
            "Derive hasher incremental must match one-shot for len {}",
            len
        );

        derive_hasher.reset();
        derive_hasher.update(&input);
        assert_eq!(
            derive_hasher.finalize(),
            derive_oneshot,
            "Derive hasher reset must yield identical subkey for len {}",
            len
        );

        // 4. Varying Chunk Step-Size Streaming Invariance
        let step_sizes = [1, 7, 31, 64, 127, 251, 512, 1024, 2048];
        for &step in &step_sizes {
            if step > len && len > 0 {
                continue;
            }
            let mut chunked_hasher = Hasher::new();
            let mut offset = 0;
            while offset < len {
                let end = (offset + step).min(len);
                chunked_hasher.update(&input[offset..end]);
                offset = end;
            }
            assert_eq!(
                chunked_hasher.finalize(),
                std_oneshot,
                "Step size {} streaming failed for input len {}",
                step,
                len
            );
        }
    }
}

// ============================================================================
// 3. Extended Output XOF Seeking and Arbitrary Read Verification
// ============================================================================

#[test]
fn test_xof_arbitrary_seeking_and_std_io_read() {
    let test_lengths = [0, 64, 1024, 4096, 65536];

    for &len in &test_lengths {
        let input = paint_test_input(len);
        let mut reference_reader: OutputReader = hash_xof(&input);
        let mut ground_truth = [0u8; 1024];
        reference_reader.fill(&mut ground_truth);

        // A. Inherent seek method verification
        let seek_offsets = [0, 1, 31, 32, 63, 64, 65, 127, 128, 256, 512, 1000];
        for &offset in &seek_offsets {
            let mut reader: OutputReader = hash_xof(&input);
            reader.seek(offset as u64);
            assert_eq!(
                reader.position(),
                offset as u64,
                "Position tracking mismatch"
            );

            let read_len = 64.min(1024 - offset);
            let mut slice = vec![0u8; read_len];
            reader.fill(&mut slice);
            assert_eq!(
                slice,
                ground_truth[offset..offset + read_len],
                "Inherent seek slice mismatch at offset {} for len {}",
                offset,
                len
            );
        }

        // B. std::io::Seek and std::io::Read trait verification
        let mut io_reader: OutputReader = hash_xof(&input);
        let target_seek = 128u64;
        let new_pos = Seek::seek(&mut io_reader, SeekFrom::Start(target_seek))
            .expect("std::io::Seek from start");
        assert_eq!(new_pos, target_seek);

        let mut io_buf = [0u8; 64];
        io_reader
            .read_exact(&mut io_buf)
            .expect("std::io::Read exact");
        assert_eq!(
            io_buf,
            ground_truth[128..192],
            "std::io::Read data mismatch at offset 128 for len {}",
            len
        );

        // C. SeekFrom::Current verification
        let current_seek = Seek::seek(&mut io_reader, SeekFrom::Current(64))
            .expect("seek relative");
        assert_eq!(current_seek, 256);
        let mut rel_buf = [0u8; 32];
        io_reader
            .read_exact(&mut rel_buf)
            .expect("std::io::Read relative");
        assert_eq!(
            rel_buf,
            ground_truth[256..288],
            "std::io relative seek data mismatch for len {}",
            len
        );
    }
}

// ============================================================================
// 4. Parallel Engine Equivalence Across Critical Boundary Sizes
// ============================================================================

#[test]
fn test_parallel_tree_hashing_exact_equivalence() {
    for &len in CRITICAL_BOUNDARY_SIZES {
        let input = paint_test_input(len);

        // 1. Standard Hash parallel equivalence
        let serial_hash = blake3(&input);
        let parallel_hash = hash_parallel(&input);
        assert_eq!(
            serial_hash, parallel_hash,
            "hash_parallel mismatch for input len {}",
            len
        );

        // 2. Keyed Hash parallel equivalence
        let serial_keyed = keyed_hash(TEST_KEY, &input);
        let parallel_keyed = keyed_hash_parallel(TEST_KEY, &input);
        assert_eq!(
            serial_keyed, parallel_keyed,
            "keyed_hash_parallel mismatch for input len {}",
            len
        );

        // 3. Derive Key parallel equivalence
        let serial_derived = derive_key(DERIVE_KEY_CONTEXT, &input);
        let parallel_derived = derive_key_parallel(DERIVE_KEY_CONTEXT, &input);
        assert_eq!(
            serial_derived, parallel_derived,
            "derive_key_parallel mismatch for input len {}",
            len
        );
    }
}
