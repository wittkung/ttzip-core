// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive corruption injection, chaos mutation, and fuzzing test suite for TTZip BLAKE3.
//!
//! Deploys 16 surgical destruction targets:
//! 1. Truncated stream ingestion and partial-tail boundary progress.
//! 2. Corrupt flag bit conflict injection (illegal combinations: `CHUNK_START | PARENT`, etc.).
//! 3. Key ingestion, extreme key entropy, and boundary key length patterns.
//! 4. Empty and extreme context key derivation (KDF) domain isolation.
//! 5. Stepped micro-block streaming vs one-shot mutation equivalence (1B..512B random steps).
//! 6. XOF cross-block random seeking and ultra-long squeeze overflow probing.
//! 7. Tree reduction right spine folding boundary jitter ($2^k, 2^k \pm 1$ chunks).
//! 8. Single-bit / single-byte flip avalanche effect and collision defense validation.
//! 9. Extreme 100KB+ sparse zero-filled, uniform, and alternating stream hashing.
//! 10. 500+ rounds of pseudo-random ChaCha-style perturbation stream fuzzing.
//! 11. TreeStack subtree reduction stack manual push/pop and merge fuzzing.
//! 12. ChunkState micro-block buffering replay and flag state mutation.
//! 13. Concurrent and parallel tree split invariance under random slicing.
//! 14. Hasher state recycling and reset replay under corrupt sequences.
//! 15. Keyed MAC nonce/key bit flip strict domain isolation verification.
//! 16. XOF long-range counter seek and multi-gigabyte virtual stream probing.

use std::io::{Read, Seek, SeekFrom};
use ttzip_engine::crypto::blake3::{
    blake3, blake3_parallel, compress_in_place, derive_key, hash, hash_parallel, hash_xof,
    keyed_hash, Blake3Hasher, ChunkState, Output, OutputReader, TreeStack, BLOCK_LEN, CHUNK_END,
    CHUNK_START, DERIVE_KEY_CONTEXT, DERIVE_KEY_MATERIAL, IV, KEYED_HASH, PARENT, ROOT,
};

/// High-speed deterministic linear congruential generator for reproducible fuzzing vectors.
#[derive(Clone, Debug)]
struct DeterministicPrng {
    state: u64,
}

impl DeterministicPrng {
    #[inline]
    const fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x853c_49e6_748f_ea9b } else { seed },
        }
    }

    #[inline]
    fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.state >> 32) as u32
    }

    #[inline]
    fn next_range(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            return min;
        }
        let span = (max - min + 1) as u64;
        min + (self.next_u32() as u64 % span) as usize
    }

    fn fill_bytes(&mut self, buf: &mut [u8]) {
        for chunk in buf.chunks_mut(4) {
            let bytes = self.next_u32().to_le_bytes();
            let len = chunk.len().min(4);
            chunk.copy_from_slice(&bytes[..len]);
        }
    }
}

/// Computes the bitwise Hamming distance between two 32-byte digests.
fn hamming_distance_bits(a: &[u8; 32], b: &[u8; 32]) -> u32 {
    let mut diff_bits = 0u32;
    for i in 0..32 {
        diff_bits += (a[i] ^ b[i]).count_ones();
    }
    diff_bits
}

// ============================================================================
// Target 1: Truncated Stream Ingestion & Tail Boundary Progress
// ============================================================================
#[test]
fn test_target_01_truncated_stream_and_tail_boundary_progress() {
    let mut prng = DeterministicPrng::new(0x1111_2222_3333_4444);
    let mut full_payload = vec![0u8; 8192];
    prng.fill_bytes(&mut full_payload);

    let truncation_points = [0, 1, 63, 64, 65, 1023, 1024, 1025, 2048, 2049, 4096, 7777, 8192];

    for &trunc_len in &truncation_points {
        let truncated = &full_payload[..trunc_len];
        let expected_hash = hash(truncated);
        assert_eq!(blake3(truncated), expected_hash);

        let mut hasher = Blake3Hasher::new();
        let mut offset = 0;
        while offset < trunc_len {
            let step = prng.next_range(1, 127).min(trunc_len - offset);
            hasher.update(&truncated[offset..offset + step]);
            offset += step;
            assert_eq!(hasher.count(), offset as u64);
        }

        assert_eq!(hasher.finalize(), expected_hash);
    }
}

// ============================================================================
// Target 2: Corrupt Flag Bit Conflict Injection
// ============================================================================
#[test]
fn test_target_02_corrupt_flag_state_bit_conflict_injection() {
    let block = [0x5Au8; BLOCK_LEN];
    let key_words = IV;

    // Test contradictory and non-standard flag combinations in raw compression
    let conflicting_flags = [
        CHUNK_START | PARENT,
        ROOT | PARENT,
        KEYED_HASH | DERIVE_KEY_CONTEXT,
        KEYED_HASH | DERIVE_KEY_MATERIAL,
        DERIVE_KEY_CONTEXT | DERIVE_KEY_MATERIAL,
        CHUNK_START | CHUNK_END | PARENT,
        0xFF,
    ];

    let mut outputs = Vec::new();
    for &flags in &conflicting_flags {
        let out_cv = compress_in_place(&key_words, &block, BLOCK_LEN as u8, 0, flags);
        outputs.push(out_cv);
    }

    // Verify all conflicting flag invocations produce distinct, deterministic chaining values
    for i in 0..outputs.len() {
        for j in (i + 1)..outputs.len() {
            assert_ne!(
                outputs[i], outputs[j],
                "Conflicting flags indices {} and {} must produce distinct outputs",
                i, j
            );
        }
    }
}

// ============================================================================
// Target 3: Key Ingestion, Extreme Entropy, and Boundary Key Patterns
// ============================================================================
#[test]
fn test_target_03_key_ingestion_and_boundary_length_injection() {
    let test_keys: [[u8; 32]; 6] = [
        [0x00; 32],
        [0xFF; 32],
        [0xAA; 32],
        [0x55; 32],
        *b"0123456789abcdef0123456789abcdef",
        *b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10\x11\x12\x13\x14\x15\x16\x17\x18\x19\x1a\x1b\x1c\x1d\x1e\x1f",
    ];

    let payload = b"TTZip High-Performance Cryptographic Engine Fuzzing Payload 2026";

    let mut digests = Vec::new();
    for key in &test_keys {
        let mac = keyed_hash(key, payload);
        let mut hasher = Blake3Hasher::new_keyed(key);
        hasher.update(payload);
        assert_eq!(hasher.finalize(), mac);
        digests.push(mac);
    }

    // Unkeyed hash must differ from all keyed hashes
    let unkeyed = hash(payload);
    for (idx, mac) in digests.iter().enumerate() {
        assert_ne!(
            *mac, unkeyed,
            "Keyed hash {} must strictly differ from unkeyed hash",
            idx
        );
    }

    // Every key pattern must produce mutually distinct MACs
    for i in 0..digests.len() {
        for j in (i + 1)..digests.len() {
            assert_ne!(digests[i], digests[j]);
        }
    }
}

// ============================================================================
// Target 4: Empty and Extreme Context KDF Injection
// ============================================================================
#[test]
fn test_target_04_empty_and_extreme_context_kdf_injection() {
    let material = b"cryptographic user master key material input 2026";

    // 1. Empty context string
    let kdf_empty_ctx = derive_key("", material);
    assert_ne!(kdf_empty_ctx, hash(material));

    // 2. Short vs whitespace vs null contexts
    let ctx_a = derive_key("a", material);
    let ctx_space = derive_key(" ", material);
    let ctx_null = derive_key("\0", material);
    assert_ne!(kdf_empty_ctx, ctx_a);
    assert_ne!(ctx_a, ctx_space);
    assert_ne!(ctx_space, ctx_null);

    // 3. Massive 16KB context string
    let huge_context = "x".repeat(16384);
    let kdf_huge_ctx = derive_key(&huge_context, material);
    assert_ne!(kdf_huge_ctx, kdf_empty_ctx);

    // 4. Empty material with various contexts
    let key_empty_mat_1 = derive_key("context-1", b"");
    let key_empty_mat_2 = derive_key("context-2", b"");
    assert_ne!(key_empty_mat_1, key_empty_mat_2);
}

// ============================================================================
// Target 5: Stepped Micro-Block Streaming vs One-Shot Mutation Equivalence
// ============================================================================
#[test]
fn test_target_05_stepped_microblock_streaming_mutation_equivalence() {
    let mut prng = DeterministicPrng::new(0xCAFE_BABE_DEAD_BEEF);
    let test_lengths = [1, 17, 63, 64, 65, 128, 512, 1023, 1024, 1025, 2048, 4096, 9999];

    for &len in &test_lengths {
        let mut data = vec![0u8; len];
        prng.fill_bytes(&mut data);
        let reference = hash(&data);

        // Test arbitrary step sizes from 1 to 512
        for step in [1, 2, 7, 31, 64, 127, 255, 512] {
            let mut hasher = Blake3Hasher::new();
            let mut offset = 0;
            while offset < len {
                let end = (offset + step).min(len);
                hasher.update(&data[offset..end]);
                offset = end;
            }
            assert_eq!(
                hasher.finalize(),
                reference,
                "Mismatch on length {len} with step size {step}"
            );
        }
    }
}

// ============================================================================
// Target 6: XOF Cross-Block Random Seeking and Squeeze Overflow Probing
// ============================================================================
#[test]
fn test_target_06_xof_cross_block_seek_and_overflow_probing() {
    let payload = b"TTZip Extensible Output Function Random Seeking Chaos Test";
    let mut reader = hash_xof(payload);

    // Generate reference stream of 1024 bytes
    let mut reference = [0u8; 1024];
    reader.fill(&mut reference);

    // Test seeking forward, backward, and across 64-byte block boundaries
    let seek_offsets = [0u64, 1, 63, 64, 65, 127, 128, 129, 255, 256, 500, 777, 1000];

    for &pos in &seek_offsets {
        let mut seeker = hash_xof(payload);
        let actual_pos = Seek::seek(&mut seeker, SeekFrom::Start(pos)).expect("Seek must succeed");
        assert_eq!(actual_pos, pos);

        let mut sample = [0u8; 24];
        let bytes_read = seeker.read(&mut sample).expect("Read must succeed");
        assert_eq!(bytes_read, 24);

        let ref_slice = &reference[pos as usize..pos as usize + 24];
        assert_eq!(&sample, ref_slice, "XOF byte mismatch at seek position {pos}");

        // Relative seek backwards
        Seek::seek(&mut seeker, SeekFrom::Current(-12)).expect("Seek backwards must succeed");
        let mut back_sample = [0u8; 12];
        seeker.read_exact(&mut back_sample).expect("Read exact must succeed");
        let ref_back_slice = &reference[pos as usize + 12..pos as usize + 24];
        assert_eq!(&back_sample, ref_back_slice);
    }

    // Probing invalid SeekFrom::End which is rejected on infinite streams
    let mut invalid_seeker = hash_xof(payload);
    assert!(Seek::seek(&mut invalid_seeker, SeekFrom::End(0)).is_err());
}

// ============================================================================
// Target 7: Tree Reduction Right Spine Folding Boundary Jitter
// ============================================================================
#[test]
fn test_target_07_tree_reduction_right_spine_folding_jitter() {
    let mut prng = DeterministicPrng::new(0x9876_5432_10FE_DCBA);

    // Boundary jitter points around 1, 2, 3, 4, 5, 8 chunks
    let chunk_boundaries = [
        1023, 1024, 1025,
        2047, 2048, 2049,
        3071, 3072, 3073,
        4095, 4096, 4097,
        5119, 5120, 5121,
        8191, 8192, 8193,
        16383, 16384, 16385,
    ];

    for &len in &chunk_boundaries {
        let mut buffer = vec![0u8; len];
        prng.fill_bytes(&mut buffer);

        let one_shot = hash(&buffer);
        let parallel = blake3_parallel(&buffer);
        assert_eq!(one_shot, parallel, "Tree reduction mismatch at boundary {len}");

        // Incremental streaming with random cuts
        let mut hasher = Blake3Hasher::new();
        let mut cursor = 0;
        while cursor < len {
            let chunk_sz = prng.next_range(17, 1031).min(len - cursor);
            hasher.update(&buffer[cursor..cursor + chunk_sz]);
            cursor += chunk_sz;
        }
        assert_eq!(hasher.finalize(), one_shot);
    }
}

// ============================================================================
// Target 8: Single-Bit & Single-Byte Flip Avalanche and Collision Defense
// ============================================================================
#[test]
fn test_target_08_single_bit_flip_avalanche_and_collision_defense() {
    let base_payload = vec![0x37u8; 1024];
    let base_digest = hash(&base_payload);

    let mut total_diff_bits = 0u64;
    let mut trials = 0u64;

    // Test flipping bits across first 64 bytes and boundary bytes
    let test_byte_indices = [0, 1, 31, 32, 63, 64, 127, 255, 511, 1023];

    for &byte_idx in &test_byte_indices {
        for bit_idx in 0..8 {
            let mut mutated = base_payload.clone();
            mutated[byte_idx] ^= 1 << bit_idx;

            let mutated_digest = hash(&mutated);
            assert_ne!(
                mutated_digest, base_digest,
                "Collision detected at byte {byte_idx} bit {bit_idx}!"
            );

            let diff = hamming_distance_bits(&base_digest, &mutated_digest);
            // Strict cryptographic avalanche criteria: between 70 and 186 bit flips
            assert!(
                (70..=186).contains(&diff),
                "Avalanche failure at byte {byte_idx} bit {bit_idx}: diff bits = {diff}"
            );

            total_diff_bits += diff as u64;
            trials += 1;
        }
    }

    // Average bit flip distance should be very close to 128 (50% of 256 bits)
    let avg_diff = total_diff_bits as f64 / trials as f64;
    assert!(
        (avg_diff - 128.0).abs() < 12.0,
        "Average avalanche distance {avg_diff} deviated too far from 128.0"
    );
}

// ============================================================================
// Target 9: Extreme 100KB+ Sparse Zero-Filled, Uniform, and Alternating Stream
// ============================================================================
#[test]
fn test_target_09_extreme_sparse_zero_filled_stream_probing() {
    let sizes = [65536, 131072, 262144];

    for &size in &sizes {
        // 1. All-zeros stream
        let zeros = vec![0u8; size];
        let hash_zeros_one_shot = hash(&zeros);
        let hash_zeros_parallel = blake3_parallel(&zeros);
        assert_eq!(hash_zeros_one_shot, hash_zeros_parallel);

        // 2. All-0xFF stream
        let ones = vec![0xFFu8; size];
        let hash_ones_one_shot = hash(&ones);
        let hash_ones_parallel = blake3_parallel(&ones);
        assert_eq!(hash_ones_one_shot, hash_ones_parallel);
        assert_ne!(hash_zeros_one_shot, hash_ones_one_shot);

        // 3. Alternating 0x55 / 0xAA stream
        let mut alt = vec![0u8; size];
        for (i, b) in alt.iter_mut().enumerate() {
            *b = if i % 2 == 0 { 0x55 } else { 0xAA };
        }
        let hash_alt = hash(&alt);
        assert_eq!(hash_alt, blake3_parallel(&alt));
        assert_ne!(hash_alt, hash_zeros_one_shot);
    }
}

// ============================================================================
// Target 10: 500+ Rounds Pseudo-Random Perturbation Stream Fuzzing
// ============================================================================
#[test]
fn test_target_10_pseudo_random_stream_fuzzing_500_rounds() {
    let mut prng = DeterministicPrng::new(0x55AA_1234_9876_ABCD);

    for round in 0..500 {
        let len = prng.next_range(0, 16384);
        let mut payload = vec![0u8; len];
        prng.fill_bytes(&mut payload);

        let ref_hash = hash(&payload);
        let parallel_hash = blake3_parallel(&payload);
        assert_eq!(
            ref_hash, parallel_hash,
            "Round {round}: Parallel vs One-Shot mismatch on length {len}"
        );

        let mut hasher = Blake3Hasher::new();
        let mut offset = 0;
        while offset < len {
            let step = prng.next_range(1, 2048).min(len - offset);
            hasher.update(&payload[offset..offset + step]);
            offset += step;
        }

        assert_eq!(
            hasher.finalize(),
            ref_hash,
            "Round {round}: Streaming vs Reference mismatch on length {len}"
        );
    }
}

// ============================================================================
// Target 11: TreeStack Subtree Reduction Stack Manual Push/Pop & Merge Fuzz
// ============================================================================
#[test]
fn test_target_11_treestack_manual_push_pop_and_merge_fuzz() {
    let mut stack = TreeStack::new();
    assert!(stack.is_empty());
    assert_eq!(stack.len(), 0);

    let mut prng = DeterministicPrng::new(0x7777_8888_9999_AAAA);

    // Test pushing up to 54
    for i in 0..54 {
        let mut cv = [0u8; 32];
        prng.fill_bytes(&mut cv);
        stack.push(cv);
        assert_eq!(stack.len(), i + 1);
    }

    // Verify pop in LIFO order
    while !stack.is_empty() {
        let prev_len = stack.len();
        let popped = stack.pop();
        assert!(popped.is_some());
        assert_eq!(stack.len(), prev_len - 1);
    }

    // Test Hamming weight merging simulation
    let mut tree_stack = TreeStack::new();
    for chunk_idx in 1..=32u64 {
        let mut dummy_cv = [0u8; 32];
        dummy_cv[0] = chunk_idx as u8;
        tree_stack.push(dummy_cv);
        tree_stack.merge_cv_stack(chunk_idx, &IV, 0);
        assert_eq!(tree_stack.len(), chunk_idx.count_ones() as usize);
    }
}

// ============================================================================
// Target 12: ChunkState Micro-Block Buffering Replay & Flag State Mutation
// ============================================================================
#[test]
fn test_target_12_chunkstate_microblock_replay_and_flag_mutation() {
    let mut state = ChunkState::new(IV, 0, 0);
    assert_eq!(state.len(), 0);
    assert_eq!(state.blocks_compressed, 0);

    let mut data = [0x42u8; 1024];
    for (i, b) in data.iter_mut().enumerate() {
        *b = (i % 256) as u8;
    }

    // Ingest 63 bytes (under one block)
    let consumed_1 = state.update(&data[..63]);
    assert_eq!(consumed_1, 63);
    assert_eq!(state.len(), 63);
    assert_eq!(state.blocks_compressed, 0);

    // Ingest remaining to complete chunk
    let consumed_2 = state.update(&data[63..]);
    assert_eq!(consumed_2, 1024 - 63);
    assert_eq!(state.len(), 1024);

    let out: Output = state.output();
    let cv = out.chaining_value();
    assert_ne!(cv, [0u8; 32]);

    // Reset and replay
    state.reset(IV, 0);
    assert_eq!(state.len(), 0);
    let consumed_all = state.update(&data);
    assert_eq!(consumed_all, 1024);
    let out2 = state.output();
    assert_eq!(out2.chaining_value(), cv);
}

// ============================================================================
// Target 13: Concurrent and Parallel Tree Split Invariance Under Fuzzing
// ============================================================================
#[test]
fn test_target_13_parallel_tree_split_invariance_under_fuzzing() {
    let mut prng = DeterministicPrng::new(0x3344_5566_7788_9900);

    for _ in 0..50 {
        let size = prng.next_range(4096, 65536);
        let mut buf = vec![0u8; size];
        prng.fill_bytes(&mut buf);

        let serial_hash = hash(&buf);
        let rayon_hash = hash_parallel(&buf);
        assert_eq!(serial_hash, rayon_hash);

        // Test keyed parallel vs keyed serial
        let key = *b"01234567890123456789012345678901";
        let keyed_serial = keyed_hash(&key, &buf);
        let keyed_par = ttzip_engine::crypto::blake3::keyed_hash_parallel(&key, &buf);
        assert_eq!(keyed_serial, keyed_par);
    }
}

// ============================================================================
// Target 14: Hasher State Recycling and Reset Replay Under Corrupt Sequence
// ============================================================================
#[test]
fn test_target_14_hasher_state_recycling_under_corrupt_sequence() {
    let mut hasher = Blake3Hasher::new();
    let corrupt_payload = b"incomplete corrupt stream data before sudden reset";
    hasher.update(corrupt_payload);
    assert_eq!(hasher.count(), corrupt_payload.len() as u64);

    // Reset hasher and verify clean state
    hasher.reset();
    assert_eq!(hasher.count(), 0);

    let clean_payload = b"clean new payload data stream after reset";
    hasher.update(clean_payload);
    let digest_after_reset = hasher.finalize();

    let fresh_digest = hash(clean_payload);
    assert_eq!(digest_after_reset, fresh_digest);
}

// ============================================================================
// Target 15: Keyed MAC Nonce/Key Bit Flip Strict Domain Isolation
// ============================================================================
#[test]
fn test_target_15_keyed_mac_domain_isolation_bit_flip() {
    let base_key = *b"super secure secret mac key 2026";
    let message = b"authenticated payload message";

    let base_mac = keyed_hash(&base_key, message);

    for byte_idx in 0..32 {
        for bit_idx in 0..8 {
            let mut mutated_key = base_key;
            mutated_key[byte_idx] ^= 1 << bit_idx;

            let mutated_mac = keyed_hash(&mutated_key, message);
            assert_ne!(
                mutated_mac, base_mac,
                "Key bit flip at byte {byte_idx} bit {bit_idx} failed domain isolation"
            );

            let diff_bits = hamming_distance_bits(&base_mac, &mutated_mac);
            assert!(
                diff_bits >= 70,
                "MAC key avalanche too low: {diff_bits} bits"
            );
        }
    }
}

// ============================================================================
// Target 16: XOF Long-Range Counter Seek and Multi-Gigabyte Virtual Stream
// ============================================================================
#[test]
fn test_target_16_xof_long_range_counter_wrap_and_seek_emulation() {
    let payload = b"virtual multi-gigabyte XOF seeking test payload";
    let mut reader = hash_xof(payload);

    // Inherent seek method to 4GB offset (block 67108864)
    let offset_4gb = 4 * 1024 * 1024 * 1024u64;
    OutputReader::seek(&mut reader, offset_4gb);
    assert_eq!(reader.position(), offset_4gb);

    let mut buf_at_4gb = [0u8; 64];
    reader.read_exact(&mut buf_at_4gb).expect("Read at 4GB must succeed");
    assert_eq!(reader.position(), offset_4gb + 64);

    // Relative seek backwards using std::io::Seek trait
    Seek::seek(&mut reader, SeekFrom::Current(-32)).expect("Relative seek back must succeed");
    let mut buf_half = [0u8; 32];
    reader.read_exact(&mut buf_half).expect("Read half must succeed");
    assert_eq!(&buf_half[..], &buf_at_4gb[32..]);
}
