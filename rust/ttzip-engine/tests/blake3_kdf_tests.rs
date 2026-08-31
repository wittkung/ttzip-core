// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration and conformance tests for BLAKE3 Keyed Hash (MAC)
//! and Key Derivation Function (KDF) implementations.

use serde::Deserialize;
use std::io::{Read, Seek, SeekFrom};
use ttzip_engine::crypto::blake3::{
    blake3, derive_key, derive_key_into, derive_key_xof, keyed_hash, new_derive_key, new_keyed,
};

const TEST_KEY: &[u8; 32] = b"whats the Elvish word for friend";
const DERIVE_KEY_CONTEXT: &str = "BLAKE3 2019-12-27 16:29:52 test vectors context";

/// Generates a deterministic test vector payload with repeating 251-byte cycle (0..=250).
fn generate_deterministic_input(len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    for (i, b) in buf.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
    buf
}

// ============================================================================
// 1. Keyed Hash Input Length and Streaming Correctness Tests (0B..=100KB)
// ============================================================================
#[test]
fn test_keyed_hash_various_input_lengths_and_streaming() {
    let test_lengths = [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 31, 32, 63, 64, 65, 127, 128, 129,
        255, 256, 511, 512, 1023, 1024, 1025, 2048, 4096, 8192, 16384,
        31744, 65536, 102400,
    ];

    for &len in &test_lengths {
        let input = generate_deterministic_input(len);

        // One-shot calculation
        let mac_oneshot = keyed_hash(TEST_KEY, &input);

        // Incremental full update
        let mut hasher_full = new_keyed(TEST_KEY);
        hasher_full.update(&input);
        let mac_incremental = hasher_full.finalize();
        assert_eq!(
            mac_oneshot, mac_incremental,
            "One-shot keyed_hash must match incremental for length {}",
            len
        );

        // Incremental chunked updates (varying step sizes)
        let chunk_step = if len > 100 { len / 7 } else { 1 };
        let mut hasher_chunked = new_keyed(TEST_KEY);
        let mut offset = 0;
        while offset < len {
            let next_offset = (offset + chunk_step).min(len);
            hasher_chunked.update(&input[offset..next_offset]);
            offset = next_offset;
        }
        let mac_chunked = hasher_chunked.finalize();
        assert_eq!(
            mac_oneshot, mac_chunked,
            "Chunked incremental keyed_hash must match one-shot for length {}",
            len
        );

        // Hasher reset verification
        hasher_chunked.reset();
        hasher_chunked.update(&input);
        let mac_after_reset = hasher_chunked.finalize();
        assert_eq!(
            mac_oneshot, mac_after_reset,
            "Hasher reset must produce identical MAC for length {}",
            len
        );
    }
}

// ============================================================================
// 2. KDF Domain Separation and Context Isolation Tests
// ============================================================================
#[test]
fn test_kdf_domain_isolation_and_context_separation() {
    let material = b"cryptographic key material that is not a password 2026";

    let ctx1 = "ttzip 2026-08-31 archive payload encryption v1";
    let ctx2 = "ttzip 2026-08-31 archive payload encryption v2";
    let ctx3 = "ttzip 2026-08-31 archive metadata authentication v1";
    let ctx_empty = "";

    let key1 = derive_key(ctx1, material);
    let key2 = derive_key(ctx2, material);
    let key3 = derive_key(ctx3, material);
    let key_empty = derive_key(ctx_empty, material);

    // Assert all distinct contexts produce orthogonal subkeys
    assert_ne!(key1, key2, "Different version contexts must produce distinct keys");
    assert_ne!(key1, key3, "Different purpose contexts must produce distinct keys");
    assert_ne!(key2, key3, "Different purpose contexts must produce distinct keys");
    assert_ne!(key1, key_empty, "Empty context must produce distinct key");

    // Orthogonality between hash modes with identical byte sequence
    let regular_hash = blake3(material);
    let keyed_mac = keyed_hash(TEST_KEY, material);
    assert_ne!(
        regular_hash, key1,
        "Regular hash and derived key must be cryptographically isolated"
    );
    assert_ne!(
        keyed_mac, key1,
        "Keyed hash and derived key must be cryptographically isolated"
    );
    assert_ne!(
        regular_hash, keyed_mac,
        "Regular hash and keyed hash must be cryptographically isolated"
    );

    // Avalanche effect / high Hamming distance between close contexts
    let mut diff_bits = 0;
    for i in 0..32 {
        diff_bits += (key1[i] ^ key2[i]).count_ones();
    }
    // Expected ~128 bits flipped for 256-bit hash (allow reasonable bounds [90..166])
    assert!(
        (90..=166).contains(&diff_bits),
        "Avalanche effect violated: only {} bits flipped between close contexts",
        diff_bits
    );
}

// ============================================================================
// 3. KDF XOF Arbitrary Length Expansion and Seeking Tests
// ============================================================================
#[test]
fn test_kdf_xof_arbitrary_length_expansion_and_seeking() {
    let material = generate_deterministic_input(2048);
    let context = "ttzip 2026-08-31 xof subkey expansion test";

    // 1. Prefix Consistency Theorem (XOF[..32] == derive_key())
    let default_key = derive_key(context, &material);
    let mut reader = derive_key_xof(context, &material);
    let mut xof_32 = [0u8; 32];
    reader.fill(&mut xof_32);
    assert_eq!(
        default_key, xof_32,
        "KDF XOF first 32 bytes must strictly match derive_key()"
    );

    // 2. Arbitrary length derivation into buffers (64B, 131B, 1024B, 4096B)
    let lengths = [64, 131, 256, 1024, 4096];
    for &target_len in &lengths {
        let mut direct_buf = vec![0u8; target_len];
        derive_key_into(context, &material, &mut direct_buf);

        let mut xof_stream_reader = derive_key_xof(context, &material);
        let mut stream_buf = vec![0u8; target_len];
        xof_stream_reader.fill(&mut stream_buf);

        assert_eq!(
            direct_buf, stream_buf,
            "derive_key_into must equal derive_key_xof for length {}",
            target_len
        );
        assert_eq!(
            &direct_buf[..32],
            &default_key[..],
            "Buffer head must equal default 32-byte key for length {}",
            target_len
        );
    }

    // 3. Seeking on KDF OutputReader
    let mut seek_reader = derive_key_xof(context, &material);
    let mut ground_truth = [0u8; 512];
    seek_reader.fill(&mut ground_truth);

    let mut seeking_reader = derive_key_xof(context, &material);
    let pos = Seek::seek(&mut seeking_reader, SeekFrom::Start(100)).expect("seek to 100");
    assert_eq!(pos, 100);
    let mut slice = [0u8; 64];
    seeking_reader.read_exact(&mut slice).expect("read 64 bytes at offset 100");
    assert_eq!(
        slice,
        ground_truth[100..164],
        "Seek and read must match ground truth subkey bytes"
    );

    // Inherent seek method verification
    let mut inherent_reader = derive_key_xof(context, &material);
    inherent_reader.seek(100);
    assert_eq!(inherent_reader.position(), 100);
    let mut slice2 = [0u8; 64];
    inherent_reader.fill(&mut slice2);
    assert_eq!(
        slice2,
        ground_truth[100..164],
        "Inherent seek must match ground truth subkey bytes"
    );
}

// ============================================================================
// 4. Official 100% Bit-Exact Conformance Vectors from test_vectors.json
// ============================================================================
#[derive(Deserialize)]
struct TestCase {
    input_len: usize,
    hash: String,
    keyed_hash: String,
    derive_key: String,
}

#[derive(Deserialize)]
struct TestVectorsFile {
    key: String,
    context_string: String,
    cases: Vec<TestCase>,
}

#[test]
fn test_official_test_vectors_keyed_hash_and_derive_key_100_percent_bit_exact() {
    let json_str = include_str!("../../../../vendor/BLAKE3/test_vectors/test_vectors.json");
    let test_file: TestVectorsFile =
        serde_json::from_str(json_str).expect("Valid test_vectors.json format");

    assert_eq!(
        test_file.key.as_bytes(),
        TEST_KEY,
        "JSON key must match test constant"
    );
    assert_eq!(
        test_file.context_string, DERIVE_KEY_CONTEXT,
        "JSON context string must match test constant"
    );

    let key_bytes: &[u8; 32] = test_file
        .key
        .as_bytes()
        .try_into()
        .expect("32-byte key string");
    let context = &test_file.context_string;

    assert!(
        !test_file.cases.is_empty(),
        "test_vectors.json must contain test cases"
    );

    for case in &test_file.cases {
        let input = generate_deterministic_input(case.input_len);
        let expected_hash_full = hex::decode(&case.hash).expect("valid hex hash");
        let expected_keyed_full = hex::decode(&case.keyed_hash).expect("valid hex keyed_hash");
        let expected_derive_full = hex::decode(&case.derive_key).expect("valid hex derive_key");

        // 0. Standard Hash One-Shot (first 32 bytes)
        let hash_oneshot = blake3(&input);
        assert_eq!(
            &hash_oneshot[..],
            &expected_hash_full[..32],
            "Standard hash 32-byte digest mismatch for input_len {}",
            case.input_len
        );

        // 1. Keyed Hash One-Shot (first 32 bytes)
        let mac_oneshot = keyed_hash(key_bytes, &input);
        assert_eq!(
            &mac_oneshot[..],
            &expected_keyed_full[..32],
            "Keyed hash 32-byte digest mismatch for input_len {}",
            case.input_len
        );

        // 2. Keyed Hash Extended Output (XOF stream 131 bytes)
        let mut keyed_hasher = new_keyed(key_bytes);
        keyed_hasher.update(&input);
        let mut keyed_xof_out = vec![0u8; expected_keyed_full.len()];
        keyed_hasher.finalize_xof().fill(&mut keyed_xof_out);
        assert_eq!(
            keyed_xof_out, expected_keyed_full,
            "Keyed hash full extended XOF mismatch for input_len {}",
            case.input_len
        );

        // 3. Derive Key One-Shot (first 32 bytes)
        let derived_oneshot = derive_key(context, &input);
        assert_eq!(
            &derived_oneshot[..],
            &expected_derive_full[..32],
            "Derive key 32-byte subkey mismatch for input_len {}",
            case.input_len
        );

        // 4. Derive Key Extended Output (derive_key_xof stream 131 bytes)
        let mut derive_xof_out = vec![0u8; expected_derive_full.len()];
        derive_key_xof(context, &input).fill(&mut derive_xof_out);
        assert_eq!(
            derive_xof_out, expected_derive_full,
            "derive_key_xof full extended XOF mismatch for input_len {}",
            case.input_len
        );

        // 5. Derive Key Direct Buffer (derive_key_into 131 bytes)
        let mut derive_into_out = vec![0u8; expected_derive_full.len()];
        derive_key_into(context, &input, &mut derive_into_out);
        assert_eq!(
            derive_into_out, expected_derive_full,
            "derive_key_into full extended output mismatch for input_len {}",
            case.input_len
        );

        // 6. Incremental Hasher new_derive_key
        let mut incremental_derive_hasher = new_derive_key(context);
        incremental_derive_hasher.update(&input);
        let derived_incremental = incremental_derive_hasher.finalize();
        assert_eq!(
            derived_incremental, derived_oneshot,
            "new_derive_key incremental finalize must match derive_key for input_len {}",
            case.input_len
        );
    }
}
