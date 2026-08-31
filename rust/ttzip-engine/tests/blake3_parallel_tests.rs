// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration and concurrency verification test suite for the BLAKE3
//! multi-threaded divide-and-conquer parallel tree hasher (`ParallelTreeHasher`).
//!
//! Validates:
//! 1. 100% Bit-Exact conformance between `ParallelTreeHasher` and single-threaded `Blake3`
//!    across standard scales (0B, 1B, 1KB, 16KB, 32KB, 64KB, 1MB, 16MB).
//! 2. Extreme asymmetric data split points (17KB = 16KB + 1KB, 33KB, 65KB, prime offsets).
//! 3. Keyed hashing and key derivation modes under parallel evaluation.
//! 4. Arbitrary XOF extraction and seeking parity between parallel and serial trees.
//! 5. Work-stealing thread pool saturation, high-concurrency contention, and zero-deadlock safety.
//! 6. Dynamic parallel threshold scaling and fine-grained parallel subtree partitioning.
//! 7. Multi-core throughput benchmark sanity checks.

use std::sync::Arc;
use std::time::Instant;

use ttzip_engine::crypto::blake3::{
    blake3, blake3_parallel, derive_key_parallel, hash_parallel, keyed_hash_parallel,
    Blake3, ParallelTreeHasher, BLAKE3_CHUNK_LEN,
};

/// Helper to generate deterministic test buffer with non-trivial byte distribution.
fn generate_deterministic_payload(size: usize, seed: u8) -> Vec<u8> {
    let mut buf = vec![0u8; size];
    for (i, b) in buf.iter_mut().enumerate() {
        let idx = i as u64;
        let s = seed as u64;
        *b = ((idx.wrapping_mul(31).wrapping_add(s.wrapping_mul(17)).wrapping_add(13)) % 251) as u8;
    }
    buf
}

// ============================================================================
// 1. Multi-Scale Bit-Exact Conformance Tests (0B to 16MB)
// ============================================================================

#[test]
fn test_parallel_exact_match_various_scales() {
    let test_sizes = [
        0,                      // Empty
        1,                      // 1 byte
        63,                     // Sub-block
        64,                     // 1 block
        65,                     // 1 block + 1 byte
        512,                    // Half chunk
        1023,                   // Chunk - 1
        1024,                   // 1 KB (1 chunk)
        1025,                   // 1 chunk + 1 byte
        2048,                   // 2 KB (2 chunks)
        4096,                   // 4 KB (4 chunks)
        8192,                   // 8 KB (8 chunks)
        16 * 1024,              // 16 KB (16 chunks - exact PARALLEL_THRESHOLD)
        32 * 1024,              // 32 KB (32 chunks)
        64 * 1024,              // 64 KB (64 chunks)
        128 * 1024,             // 128 KB
        512 * 1024,             // 512 KB
        1024 * 1024,            // 1 MB
        4 * 1024 * 1024,        // 4 MB
        16 * 1024 * 1024,       // 16 MB
    ];

    for &size in &test_sizes {
        let payload = generate_deterministic_payload(size, (size % 239) as u8);

        let serial_hash = blake3(&payload);
        let parallel_hash = hash_parallel(&payload);
        let hasher_parallel_hash = ParallelTreeHasher::new().hash(&payload);
        let blake3_par_hash = Blake3::hash_parallel(&payload);
        let blake3_top_par = blake3_parallel(&payload);

        assert_eq!(
            parallel_hash, serial_hash,
            "hash_parallel mismatch for payload size {size} bytes"
        );
        assert_eq!(
            hasher_parallel_hash, serial_hash,
            "ParallelTreeHasher::hash mismatch for payload size {size} bytes"
        );
        assert_eq!(
            blake3_par_hash, serial_hash,
            "Blake3::hash_parallel mismatch for payload size {size} bytes"
        );
        assert_eq!(
            blake3_top_par, serial_hash,
            "blake3_parallel mismatch for payload size {size} bytes"
        );
    }
}

// ============================================================================
// 2. Extreme Asymmetric Data Split and Irregular Boundary Tests
// ============================================================================

#[test]
fn test_asymmetric_tree_splits() {
    let asymmetric_sizes = [
        16 * 1024 + 1,              // 16 KB + 1 byte (17 chunks total, right has 1 byte)
        16 * 1024 + 1024,           // 17 KB = 16 KB + 1 KB
        16 * 1024 + 1025,           // 17 KB + 1 byte
        32 * 1024 + 1,              // 32 KB + 1 byte
        32 * 1024 + 1024,           // 33 KB = 32 KB + 1 KB
        64 * 1024 + 1024,           // 65 KB = 64 KB + 1 KB
        64 * 1024 + 31 * 1024 + 7,  // 95 KB + 7 bytes
        100_000,                    // 100,000 bytes
        123_456,                    // Arbitrary irregular length
        999_999,                    // Near 1 MB
        1_234_567,                  // 1.23 MB
        3_141_592,                  // ~3.14 MB
    ];

    for &size in &asymmetric_sizes {
        let payload = generate_deterministic_payload(size, 42);

        let serial_hash = blake3(&payload);
        let parallel_hash = hash_parallel(&payload);

        assert_eq!(
            parallel_hash, serial_hash,
            "Asymmetric split hash mismatch for size {size} bytes"
        );
    }
}

// ============================================================================
// 3. Keyed Hashing and Key Derivation Parallel Tests
// ============================================================================

#[test]
fn test_keyed_hash_parallel() {
    let key = [0x5Au8; 32];
    let test_sizes = [0, 100, 1024, 16384, 17408, 65536, 1048576];

    for &size in &test_sizes {
        let payload = generate_deterministic_payload(size, 77);

        let mut serial_keyed = Blake3::new_keyed(&key);
        serial_keyed.update(&payload);
        let expected_hash = serial_keyed.finalize();

        let parallel_keyed_hash = keyed_hash_parallel(&key, &payload);
        let hasher_keyed_hash = ParallelTreeHasher::new_keyed(&key).hash(&payload);

        assert_eq!(
            parallel_keyed_hash, expected_hash,
            "keyed_hash_parallel mismatch for size {size}"
        );
        assert_eq!(
            hasher_keyed_hash, expected_hash,
            "ParallelTreeHasher::new_keyed mismatch for size {size}"
        );
    }
}

#[test]
fn test_derive_key_parallel() {
    let context = "ttzip blake3 parallel kdf test context 2026";
    let test_sizes = [0, 50, 1024, 16384, 32768, 500000];

    for &size in &test_sizes {
        let material = generate_deterministic_payload(size, 99);

        let mut serial_kdf = Blake3::new_derive_key(context);
        serial_kdf.update(&material);
        let expected_kdf = serial_kdf.finalize();

        let parallel_kdf = derive_key_parallel(context, &material);
        let hasher_kdf = ParallelTreeHasher::new_derive_key(context).hash(&material);

        assert_eq!(
            parallel_kdf, expected_kdf,
            "derive_key_parallel mismatch for material size {size}"
        );
        assert_eq!(
            hasher_kdf, expected_kdf,
            "ParallelTreeHasher::new_derive_key mismatch for material size {size}"
        );
    }
}

// ============================================================================
// 4. Extensible Output Function (XOF) Parallel Extraction Tests
// ============================================================================

#[test]
fn test_xof_parallel_fill_and_seek() {
    let payload = generate_deterministic_payload(256 * 1024, 123); // 256 KB

    let mut serial_hasher = Blake3::new();
    serial_hasher.update(&payload);
    let mut serial_xof = serial_hasher.finalize_xof();

    let parallel_hasher = ParallelTreeHasher::new();
    let mut parallel_xof = parallel_hasher.hash_xof(&payload);

    // Test multi-block extraction
    let mut serial_buf = [0u8; 1000];
    let mut parallel_buf = [0u8; 1000];
    serial_xof.fill(&mut serial_buf);
    parallel_xof.fill(&mut parallel_buf);
    assert_eq!(parallel_buf, serial_buf);

    // Test fill into slice via ParallelTreeHasher::hash_into
    let mut hash_into_buf = [0u8; 1000];
    parallel_hasher.hash_into(&payload, &mut hash_into_buf);
    assert_eq!(hash_into_buf, serial_buf);

    // Test seek parity
    serial_xof.seek(4096);
    parallel_xof.seek(4096);
    assert_eq!(serial_xof.position(), 4096);
    assert_eq!(parallel_xof.position(), 4096);

    let mut serial_seek_buf = [0u8; 128];
    let mut parallel_seek_buf = [0u8; 128];
    serial_xof.fill(&mut serial_seek_buf);
    parallel_xof.fill(&mut parallel_seek_buf);
    assert_eq!(parallel_seek_buf, serial_seek_buf);
}

// ============================================================================
// 5. Custom Threshold Scaling and Fine-Grained Partitioning Tests
// ============================================================================

#[test]
fn test_custom_threshold_fine_grained_concurrency() {
    let payload = generate_deterministic_payload(64 * 1024, 88); // 64 KB
    let expected = blake3(&payload);

    // Test thresholds from 1 KB up to 64 KB
    let thresholds = [
        BLAKE3_CHUNK_LEN,      // 1 KB (parallelizes down to individual chunk pairs)
        2 * BLAKE3_CHUNK_LEN,  // 2 KB
        4 * BLAKE3_CHUNK_LEN,  // 4 KB
        8 * BLAKE3_CHUNK_LEN,  // 8 KB
        16 * BLAKE3_CHUNK_LEN, // 16 KB (default)
        32 * BLAKE3_CHUNK_LEN, // 32 KB
        64 * BLAKE3_CHUNK_LEN, // 64 KB
        128 * BLAKE3_CHUNK_LEN,// 128 KB (all serial)
    ];

    for &threshold in &thresholds {
        let hasher = ParallelTreeHasher::new().with_threshold(threshold);
        assert_eq!(hasher.threshold(), threshold);
        let digest = hasher.hash(&payload);
        assert_eq!(
            digest, expected,
            "Digest mismatch with custom threshold = {threshold}"
        );
    }
}

// ============================================================================
// 6. Thread Pool Saturation and Zero-Deadlock Safety Tests
// ============================================================================

#[test]
fn test_thread_pool_saturation_and_zero_deadlock() {
    use rayon::prelude::*;

    let num_concurrent_tasks = 64;
    let payload_size = 512 * 1024; // 512 KB per task
    let payload = Arc::new(generate_deterministic_payload(payload_size, 33));
    let expected_hash = blake3(&payload);

    // Launch 64 concurrent Rayon tasks simultaneously invoking ParallelTreeHasher
    let results: Vec<[u8; 32]> = (0..num_concurrent_tasks)
        .into_par_iter()
        .map(|_| {
            let hasher = ParallelTreeHasher::new();
            hasher.hash(&payload)
        })
        .collect();

    assert_eq!(results.len(), num_concurrent_tasks);
    for (i, hash) in results.iter().enumerate() {
        assert_eq!(
            *hash, expected_hash,
            "Saturation task {i} produced incorrect hash"
        );
    }
}

#[test]
fn test_nested_custom_rayon_pool_zero_deadlock() {
    // Verify execution inside a constrained 2-thread Rayon pool
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .expect("Failed to build 2-thread pool");

    let payload = generate_deterministic_payload(2 * 1024 * 1024, 71); // 2 MB
    let expected = blake3(&payload);

    let actual = pool.install(|| {
        hash_parallel(&payload)
    });

    assert_eq!(actual, expected, "Constrained 2-thread pool hash mismatch");
}

// ============================================================================
// 7. Multi-Core Throughput Scaling Test
// ============================================================================

#[test]
fn test_multicore_throughput_scaling() {
    let size = 16 * 1024 * 1024; // 16 MB
    let payload = generate_deterministic_payload(size, 202);

    // Warm-up
    let _ = hash_parallel(&payload);

    let start = Instant::now();
    let digest = hash_parallel(&payload);
    let elapsed = start.elapsed();

    let expected = blake3(&payload);
    assert_eq!(digest, expected);

    let throughput_mb_s = (size as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64();
    println!(
        "BLAKE3 Parallel throughput for 16MB: {:.2} MB/s (elapsed: {:?})",
        throughput_mb_s, elapsed
    );

    assert!(
        throughput_mb_s > 50.0,
        "Parallel throughput {:.2} MB/s is lower than expected baseline",
        throughput_mb_s
    );
}
