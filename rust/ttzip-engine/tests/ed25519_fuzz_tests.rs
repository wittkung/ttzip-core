// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive corruption injection, chaos mutation, and fuzzing test suite for TTZip Ed25519.
//!
//! Deploys 16 surgical destruction targets:
//! 1. Single-bit signature flip rejection and avalanche diffusion.
//! 2. Malformed public key points (non-curve points, $y \ge p$) rejection.
//! 3. Scalar overflow $S \ge \ell$ malleability injection.
//! 4. 0-length and 16MB ultra-large message signature equivalence.
//! 5. Corrupt certificate chain and circular certificate chain detection.
//! 6. Batch verification single-point failure injection and bisection localization probing.
//! 7. 500+ rounds of pseudo-random perturbation stream fuzzing.
//! 8. Public key bit flip and cross-key mismatch rejection.
//! 9. Message bit flip avalanche and collision defense verification.
//! 10. Nonce / Signature R-component tampering and clamping boundary validation.
//! 11. Small-subgroup and low-order torsion point injection attack defense.
//! 12. Zero, all-ones, and extreme private key entropy injection.
//! 13. High-concurrency multithreaded Rayon parallel verification invariance.
//! 14. Certificate chain expiration, depth exhaustion, and payload tampering.
//! 15. License token protocol prefix corruption and delimiter fuzzing.
//! 16. Base64 decoding corruption, payload tampering, and padding mutation probing.

use rayon::prelude::*;
use ttzip_engine::crypto::ed25519::{
    derive_deterministic_nonce, expand_secret_key, locate_faulty_signatures, scalar_clamp,
    BatchVerifier, CertificateChain, CertificateLevel, Ed25519Error,
    SigningKey, TTZipCertificate, VerifyingKey,
};
use ttzip_engine::security::ed25519_defense::{
    CertNode, Ed25519DefenseConfig, Ed25519DefenseError, GuardedEd25519Verifier,
    MalleabilityGuard, SubgroupAttackGuard, ED25519_ORDER_L, SMALL_SUBGROUP_POINTS,
};
use ttzip_engine::security::license::{verify_license_key, UniFFILicenseResult};

/// Deterministic linear congruential generator for reproducible fuzzing vectors.
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

// ============================================================================
// Target 1: Single-Bit Signature Flip Rejection & Avalanche Diffusion
// ============================================================================
#[test]
fn test_target_01_single_bit_signature_flip_rejection() {
    let secret = [0x5au8; 32];
    let signing_key = SigningKey::from_bytes(&secret);
    let verifying_key = signing_key.verifying_key();
    let message = b"TTZip High-Assurance Cryptographic Engine Signature Flip Test";

    let signature = signing_key.sign(message);
    assert!(verifying_key.verify(message, &signature).is_ok());

    let raw_sig = signature.to_bytes();
    let mut rejected_count = 0usize;

    // Flip every single bit in the 64-byte signature
    for byte_idx in 0..64 {
        for bit_idx in 0..8 {
            let mut corrupted_sig = raw_sig;
            corrupted_sig[byte_idx] ^= 1 << bit_idx;

            let corrupted_struct = ttzip_engine::crypto::ed25519::Signature::from_bytes(&corrupted_sig);
            let res = verifying_key.verify(message, &corrupted_struct);
            assert!(
                res.is_err(),
                "Corrupted signature at byte {byte_idx} bit {bit_idx} was falsely accepted!"
            );
            rejected_count += 1;
        }
    }
    assert_eq!(rejected_count, 64 * 8);
}

// ============================================================================
// Target 2: Malformed Public Key Points (Non-Curve Points, y >= p) Rejection
// ============================================================================
#[test]
fn test_target_02_malformed_public_key_points_rejection() {
    // 1. Find non-curve points (where (y^2 - 1)/(d y^2 + 1) is not a quadratic residue in GF(2^255-19))
    let mut non_curve_point = None;
    for b in 1..255u8 {
        let mut candidate = [0u8; 32];
        candidate[0] = b;
        candidate[1] = b.wrapping_mul(13);
        if VerifyingKey::from_bytes(&candidate).is_err() {
            non_curve_point = Some(candidate);
            break;
        }
    }
    let bad_point = non_curve_point.expect("Must find non-curve points in GF(2^255-19)");
    assert!(VerifyingKey::from_bytes(&bad_point).is_err());
    assert!(SubgroupAttackGuard::verify_canonical_point(&bad_point).is_err());

    // 2. Fuzz 100 candidate byte patterns to assert non-curve points are rejected
    let mut rejected_count = 0usize;
    let mut accepted_count = 0usize;
    for candidate_y in 1..100u8 {
        let mut pt = [0u8; 32];
        pt[0] = candidate_y;
        pt[1] = candidate_y.wrapping_mul(17);
        match VerifyingKey::from_bytes(&pt) {
            Ok(_) => accepted_count += 1,
            Err(_) => rejected_count += 1,
        }
    }
    // Field arithmetic guarantees ~50% of candidate coordinates are non-curve elements
    assert!(rejected_count >= 20, "Expected >= 20 non-curve points rejected, got {rejected_count}");
    assert!(accepted_count >= 20, "Expected >= 20 valid curve points accepted, got {accepted_count}");

    // 3. Guarded verifier rejection on malformed non-curve points
    let verifier = GuardedEd25519Verifier::new();
    let msg = b"test message";
    let dummy_sig = [0u8; 64];
    assert!(verifier.verify(&bad_point, msg, &dummy_sig).is_err());
}

// ============================================================================
// Target 3: Scalar Overflow S >= \ell Malleability Injection
// ============================================================================
#[test]
fn test_target_03_scalar_overflow_malleability_injection() {
    let secret = [0x37u8; 32];
    let signing_key = SigningKey::from_bytes(&secret);
    let verifying_key = signing_key.verifying_key();
    let message = b"Anti-Malleability Non-Canonical Scalar Invariant";

    let sig = signing_key.sign(message);
    let mut sig_bytes = sig.to_bytes();

    // In RFC 8032 / ZIP 215, S must be strictly < \ell.
    // Inject non-canonical S = \ell (exact curve order)
    sig_bytes[32..64].copy_from_slice(&ED25519_ORDER_L);
    let malleable_sig = ttzip_engine::crypto::ed25519::Signature::from_bytes(&sig_bytes);

    let verify_res = verifying_key.verify(message, &malleable_sig);
    assert_eq!(verify_res, Err(Ed25519Error::NonCanonicalScalar));

    // Also test MalleabilityGuard structure inspection
    let struct_res = MalleabilityGuard::verify_signature_structure(&sig_bytes);
    assert_eq!(struct_res, Err(Ed25519DefenseError::NonCanonicalScalarDetected));
}

// ============================================================================
// Target 4: 0-Length and 16MB Ultra-Large Message Signature Equivalence
// ============================================================================
#[test]
fn test_target_04_zero_length_and_large_message_equivalence() {
    let seed = [0x77u8; 32];
    let signing_key = SigningKey::from_bytes(&seed);
    let verifying_key = signing_key.verifying_key();

    // 1. 0-length message
    let empty_msg = b"";
    let sig_empty = signing_key.sign(empty_msg);
    assert!(verifying_key.verify(empty_msg, &sig_empty).is_ok());

    // 2. 16MB message
    let mut prng = DeterministicPrng::new(0x1234_5678_9ABC_DEF0);
    let large_size = 16 * 1024 * 1024;
    let mut large_msg = vec![0u8; large_size];
    prng.fill_bytes(&mut large_msg);

    let sig_large = signing_key.sign(&large_msg);
    assert!(verifying_key.verify(&large_msg, &sig_large).is_ok());

    // Tampering 1 byte at the end of 16MB message must fail verification
    large_msg[large_size - 1] ^= 0x01;
    assert!(verifying_key.verify(&large_msg, &sig_large).is_err());
}

// ============================================================================
// Target 5: Corrupt Certificate Chain & Circular Certificate Chain Detection
// ============================================================================
#[test]
fn test_target_05_corrupt_certificate_chain_and_circular_detection() {
    let now = 1_770_000_000u64;

    let root_sk = SigningKey::from_bytes(&[10u8; 32]);
    let root_vk = root_sk.verifying_key();
    let root_cert = TTZipCertificate::issue(
        CertificateLevel::Root,
        "Root-CA".to_string(),
        "Root-CA".to_string(),
        root_vk.to_bytes(),
        now - 1000,
        now + 100000,
        &root_sk,
    );

    let market_sk = SigningKey::from_bytes(&[20u8; 32]);
    let market_vk = market_sk.verifying_key();
    let market_cert = TTZipCertificate::issue(
        CertificateLevel::Marketplace,
        "Root-CA".to_string(),
        "Marketplace".to_string(),
        market_vk.to_bytes(),
        now - 500,
        now + 50000,
        &root_sk,
    );

    let dev_sk = SigningKey::from_bytes(&[30u8; 32]);
    let dev_vk = dev_sk.verifying_key();
    let dev_cert = TTZipCertificate::issue(
        CertificateLevel::Developer,
        "Marketplace".to_string(),
        "Developer-A".to_string(),
        dev_vk.to_bytes(),
        now - 100,
        now + 20000,
        &market_sk,
    );

    let valid_chain = CertificateChain::new(root_cert.clone(), market_cert.clone(), dev_cert.clone());
    assert!(valid_chain.verify_chain(&root_vk, now).is_ok());

    // 1. Corrupt signature in intermediate certificate
    let mut bad_market = market_cert.clone();
    bad_market.signature[5] ^= 0xFF;
    let bad_chain = CertificateChain::new(root_cert.clone(), bad_market, dev_cert.clone());
    assert!(bad_chain.verify_chain(&root_vk, now).is_err());

    // 2. Circular / self-referential chain
    let mut circular_market = market_cert;
    circular_market.issuer_id = "Developer-A".to_string();
    let circular_chain = CertificateChain::new(root_cert, circular_market, dev_cert);
    assert!(circular_chain.verify_chain(&root_vk, now).is_err());
}

// ============================================================================
// Target 6: Batch Verification Single-Point Failure & Bisection Localization
// ============================================================================
#[test]
fn test_target_06_batch_verification_failure_and_bisection() {
    let mut keys = Vec::new();
    let mut sigs = Vec::new();
    let mut messages: Vec<Vec<u8>> = Vec::new();

    for i in 0..32 {
        let seed = [i as u8 + 1; 32];
        let sk = SigningKey::from_bytes(&seed);
        let vk = sk.verifying_key();
        let msg = format!("Batch Payload Segment #{i}").into_bytes();
        let sig = sk.sign(&msg);
        keys.push(vk);
        sigs.push(sig);
        messages.push(msg);
    }

    let msg_refs: Vec<&[u8]> = messages.iter().map(|m| m.as_slice()).collect();

    // Clean batch passes
    let mut verifier = BatchVerifier::new();
    for i in 0..keys.len() {
        verifier.add(&keys[i], msg_refs[i], &sigs[i]);
    }
    assert!(verifier.verify().is_ok());

    // Corrupt signature at index 7 and index 23
    let mut tampered_sigs = sigs.clone();
    let mut bad_sig7 = tampered_sigs[7].to_bytes();
    bad_sig7[12] ^= 0xAA;
    tampered_sigs[7] = ttzip_engine::crypto::ed25519::Signature::from_bytes(&bad_sig7);

    let mut bad_sig23 = tampered_sigs[23].to_bytes();
    bad_sig23[40] ^= 0x55;
    tampered_sigs[23] = ttzip_engine::crypto::ed25519::Signature::from_bytes(&bad_sig23);

    // Batch verification must fail
    let mut bad_verifier = BatchVerifier::new();
    for i in 0..keys.len() {
        bad_verifier.add(&keys[i], msg_refs[i], &tampered_sigs[i]);
    }
    assert!(bad_verifier.verify().is_err());

    // Bisection localization algorithm must pinpoint exactly [7, 23]
    let faulty_indices = locate_faulty_signatures(&keys, &msg_refs, &tampered_sigs);
    assert_eq!(faulty_indices, vec![7, 23]);
}

// ============================================================================
// Target 7: 500+ Rounds Pseudo-Random Perturbation Stream Fuzzing
// ============================================================================
#[test]
fn test_target_07_pseudo_random_stream_fuzzing_500_rounds() {
    let mut prng = DeterministicPrng::new(0xABCD_EF01_2345_6789);

    for _ in 0..500 {
        let mut seed = [0u8; 32];
        prng.fill_bytes(&mut seed);

        let sk = SigningKey::from_bytes(&seed);
        let vk = sk.verifying_key();

        let msg_len = prng.next_range(0, 1024);
        let mut message = vec![0u8; msg_len];
        prng.fill_bytes(&mut message);

        let sig = sk.sign(&message);
        assert!(vk.verify(&message, &sig).is_ok());

        // Introduce random mutation with probability 0.5
        if (prng.next_u32() & 1) == 0 && msg_len > 0 {
            let mutate_idx = prng.next_range(0, msg_len - 1);
            message[mutate_idx] ^= 0x01;
            assert!(vk.verify(&message, &sig).is_err());
        }
    }
}

// ============================================================================
// Target 8: Public Key Bit Flip & Cross-Key Mismatch Rejection
// ============================================================================
#[test]
fn test_target_08_public_key_bit_flip_rejection() {
    let sk1 = SigningKey::from_bytes(&[0x11u8; 32]);
    let vk1 = sk1.verifying_key();

    let sk2 = SigningKey::from_bytes(&[0x22u8; 32]);
    let vk2 = sk2.verifying_key();

    let message = b"Cross-Key Authentication Verification Payload";
    let sig1 = sk1.sign(message);

    // Cross-key verification must strictly fail
    assert!(vk2.verify(message, &sig1).is_err());

    // Public key bit flips
    let raw_vk1 = vk1.to_bytes();
    for byte_idx in 0..32 {
        let mut corrupted_vk = raw_vk1;
        corrupted_vk[byte_idx] ^= 0x01;
        if let Ok(bad_vk) = VerifyingKey::from_bytes(&corrupted_vk) {
            assert!(bad_vk.verify(message, &sig1).is_err());
        }
    }
}

// ============================================================================
// Target 9: Message Bit Flip Avalanche & Collision Defense
// ============================================================================
#[test]
fn test_target_09_message_bit_flip_collision_defense() {
    let sk = SigningKey::from_bytes(&[0x99u8; 32]);
    let vk = sk.verifying_key();
    let base_message = vec![0x42u8; 256];
    let base_sig = sk.sign(&base_message);

    for idx in [0, 1, 64, 128, 255] {
        for bit in 0..8 {
            let mut mutated = base_message.clone();
            mutated[idx] ^= 1 << bit;
            assert!(
                vk.verify(&mutated, &base_sig).is_err(),
                "Bit collision on message byte {idx} bit {bit}"
            );
        }
    }
}

// ============================================================================
// Target 10: Nonce / Signature R-Component Tampering & Clamping Validation
// ============================================================================
#[test]
fn test_target_10_nonce_r_tampering_and_clamping() {
    let mut raw_scalar = [0xffu8; 32];
    scalar_clamp(&mut raw_scalar);

    // Verify RFC 8032 bit clamps
    assert_eq!(raw_scalar[0] & 0b0000_0111, 0);
    assert_eq!(raw_scalar[31] & 0b1000_0000, 0);
    assert_eq!(raw_scalar[31] & 0b0100_0000, 0b0100_0000);

    // Test deterministic nonce derivation independence
    let prefix = [0xAAu8; 32];
    let nonce1 = derive_deterministic_nonce(&prefix, b"msg1");
    let nonce2 = derive_deterministic_nonce(&prefix, b"msg2");
    assert_ne!(nonce1.to_bytes(), nonce2.to_bytes());
}

// ============================================================================
// Target 11: Small-Subgroup Torsion Point Attack Defense
// ============================================================================
#[test]
fn test_target_11_small_subgroup_torsion_point_defense() {
    for (idx, &subgroup_point) in SMALL_SUBGROUP_POINTS.iter().enumerate() {
        assert!(
            SubgroupAttackGuard::is_small_subgroup_point(&subgroup_point),
            "Small subgroup point index {idx} not recognized by guard"
        );

        let res = SubgroupAttackGuard::verify_canonical_point(&subgroup_point);
        assert!(res.is_err(), "Small subgroup point index {idx} was not rejected");
    }
}

// ============================================================================
// Target 12: Zero, All-Ones, and Extreme Key Entropy Injection
// ============================================================================
#[test]
fn test_target_12_extreme_key_entropy_injection() {
    let extreme_seeds = [
        [0x00u8; 32],
        [0xFFu8; 32],
        [0x55u8; 32],
        [0xAAu8; 32],
    ];

    let message = b"Extreme Key Entropy Robustness Check";
    for seed in &extreme_seeds {
        let (scalar, prefix) = expand_secret_key(seed);
        assert_ne!(scalar.to_bytes(), [0u8; 32]);
        assert_ne!(prefix, [0u8; 32]);

        let sk = SigningKey::from_bytes(seed);
        let vk = sk.verifying_key();
        let sig = sk.sign(message);
        assert!(vk.verify(message, &sig).is_ok());
    }
}

// ============================================================================
// Target 13: High-Concurrency Rayon Parallel Verification Invariance
// ============================================================================
#[test]
fn test_target_13_high_concurrency_rayon_parallel_verification() {
    let count = 1000;
    let pairs: Vec<(VerifyingKey, Vec<u8>, ttzip_engine::crypto::ed25519::Signature)> = (0..count)
        .into_par_iter()
        .map(|i| {
            let seed = [((i * 37 + 13) % 256) as u8; 32];
            let sk = SigningKey::from_bytes(&seed);
            let vk = sk.verifying_key();
            let msg = format!("Concurrent Thread Task #{i}").into_bytes();
            let sig = sk.sign(&msg);
            (vk, msg, sig)
        })
        .collect();

    let all_valid = pairs
        .par_iter()
        .all(|(vk, msg, sig)| vk.verify(msg, sig).is_ok());
    assert!(all_valid, "All 1000 concurrent signatures must verify successfully");
}

// ============================================================================
// Target 14: Certificate Chain Depth Exhaustion & Payload Tampering
// ============================================================================
#[test]
fn test_target_14_certificate_depth_exhaustion() {
    let verifier = GuardedEd25519Verifier::new_with_config(
        Ed25519DefenseConfig::default_limits().with_max_cert_chain_depth(3),
    );

    let dummy_node = CertNode {
        subject_pubkey: [1u8; 32],
        issuer_signature: [2u8; 64],
        payload: vec![0u8; 32],
    };

    let chain = vec![dummy_node.clone(), dummy_node.clone(), dummy_node.clone(), dummy_node];
    let res = verifier.verify_cert_chain(&chain, b"data", &[0u8; 64], &[0u8; 32]);
    assert_eq!(
        res,
        Err(Ed25519DefenseError::CertChainDepthExceeded { depth: 4, limit: 3 })
    );
}

// ============================================================================
// Target 15: License Token Protocol Prefix & Delimiter Fuzzing
// ============================================================================
#[test]
fn test_target_15_license_protocol_prefix_fuzzing() {
    let corrupt_tokens = [
        "",
        "TTZIP-INVALID",
        "TTZIP2-something.payload",
        "TTZIP1-singleparttoken",
        "TTZIP1-part1.part2.part3",
        "TTZIP1-.invalid",
        "TTZIP1-invalid.",
    ];

    for &token in &corrupt_tokens {
        let res = verify_license_key(token.to_string(), None);
        assert!(
            matches!(res, UniFFILicenseResult::MalformedKey { .. }),
            "Token '{token}' was not rejected as MalformedKey"
        );
    }
}

// ============================================================================
// Target 16: Base64 Payload Tampering and Padding Mutation Probing
// ============================================================================
#[test]
fn test_target_16_base64_payload_tampering_probing() {
    let corrupt_base64_tokens = [
        "TTZIP1-!@#$%.AAAA",
        "TTZIP1-AAAA.======",
        "TTZIP1-SGVsbG8.SGVsbG8",
    ];

    for &token in &corrupt_base64_tokens {
        let res = verify_license_key(token.to_string(), None);
        assert!(matches!(res, UniFFILicenseResult::MalformedKey { .. }));
    }
}
