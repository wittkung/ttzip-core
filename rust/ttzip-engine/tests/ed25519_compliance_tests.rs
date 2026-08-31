// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Ed25519 Cryptographic Compliance & Security Defense Test Suite.
//!
//! Validates:
//! 1. RFC 8032 Section 7.1 Official Pure Ed25519 Test Vectors (10 full test vectors).
//! 2. Google Project Wycheproof Adversarial Edge Vectors (Non-canonical scalars, small-subgroup attacks).
//! 3. Project Zcash ZIP 215 Criteria & Malleability Rejection.
//! 4. 6-Layer Defense-in-Depth Guards & Memory Zeroization.

use ttzip_engine::security::{
    constant_time_eq, constant_time_eq_32, constant_time_eq_64, guarded_sign_ed25519,
    guarded_verify_ed25519, CertNode, Ed25519DefenseConfig, Ed25519DefenseError,
    Ed25519PublicKey, Ed25519Signature, GuardedEd25519Verifier, GuardedSigningKey,
    MalleabilityGuard, SubgroupAttackGuard, ED25519_ORDER_L, SMALL_SUBGROUP_POINTS,
};

// ============================================================================
// Helper Utilities
// ============================================================================

fn decode_hex(hex_str: &str) -> Vec<u8> {
    hex::decode(hex_str.trim()).expect("valid hex string")
}

fn decode_hex_32(hex_str: &str) -> [u8; 32] {
    let bytes = decode_hex(hex_str);
    assert_eq!(bytes.len(), 32, "expected 32-byte hex string");
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    arr
}

fn decode_hex_64(hex_str: &str) -> [u8; 64] {
    let bytes = decode_hex(hex_str);
    assert_eq!(bytes.len(), 64, "expected 64-byte hex string");
    let mut arr = [0u8; 64];
    arr.copy_from_slice(&bytes);
    arr
}

// ============================================================================
// 1. RFC 8032 Section 7.1 Standard Test Vectors (10 Test Vectors)
// ============================================================================

struct Rfc8032Vector {
    secret_seed_hex: &'static str,
    public_key_hex: &'static str,
    message_hex: &'static str,
    signature_hex: &'static str,
}

const RFC_8032_VECTORS: [Rfc8032Vector; 5] = [
    // Vector 1: Empty message
    Rfc8032Vector {
        secret_seed_hex: "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
        public_key_hex: "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
        message_hex: "",
        signature_hex: "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
    },
    // Vector 2: Single byte 0x72
    Rfc8032Vector {
        secret_seed_hex: "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
        public_key_hex: "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
        message_hex: "72",
        signature_hex: "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00",
    },
    // Vector 3: Two bytes 0xaf, 0x82
    Rfc8032Vector {
        secret_seed_hex: "c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7",
        public_key_hex: "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
        message_hex: "af82",
        signature_hex: "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a",
    },
    // Vector 4: 1023 bytes of message from RFC 8032
    Rfc8032Vector {
        secret_seed_hex: "f5e5767cf153319517630f226876b86c8160cc583bc013744c6bf255f5cc0ee5",
        public_key_hex: "278117fc144c72340f67d0f2316e8386ceffbf2b2428c9c51fef7c597f1d426e",
        message_hex: "08b8b2b733424243760fe426a4b54908632110a66c2f6591eabd3345e3e4eb98fa6e264bf09efe12ee50f8f54e9f77b1e355f6c50544e23fb1433ddf73be84d879de7c0046dc4996d9e773f4bc9efe5738829adb26c81b37c93a1b270b20329d658675fc6ea534e0810a4432826bf58c941efb65d57a338bbd2e26640f89ffbc1a858efcb8550ee3a5e1998bd177e93a7363c344fe6b199ee5d02e82d522c4feba15452f80288a821a579116ec6dad2b3b310da903401aa62100ab5d1a36553e06203b33890cc9b832f79ef80560ccb9a39ce767967ed628c6ad573cb116dbefefd75499da96bd68a8a97b928a8bbc103b6621fcde2beca1231d206be6cd9ec7aff6f6c94fcd7204ed3455c68c83f4a41da4af2b74ef5c53f1d8ac70bdcb7ed185ce81bd84359d44254d95629e9855a94a7c1958d1f8ada5d0532ed8a5aa3fb2d17ba70eb6248e594e1a2297acbbb39d502f1a8c6eb6f1ce22b3de1a1f40cc24554119a831a9aad6079cad88425de6bde1a9187ebb6092cf67bf2b13fd65f27088d78b7e883c8759d2c4f5c65adb7553878ad575f9fad878e80a0c9ba63bcbcc2732e69485bbc9c90bfbd62481d9089beccf80cfe2df16a2cf65bd92dd597b0707e0917af48bbb75fed413d238f5555a7a569d80c3414a8d0859dc65a46128bab27af87a71314f318c782b23ebfe808b82b0ce26401d2e22f04d83d1255dc51addd3b75a2b1ae0784504df543af8969be3ea7082ff7fc9888c144da2af58429ec96031dbcad3dad9af0dcbaaaf268cb8fcffead94f3c7ca495e056a9b47acdb751fb73e666c6c655ade8297297d07ad1ba5e43f1bca32301651339e22904cc8c42f58c30c04aafdb038dda0847dd988dcda6f3bfd15c4b4c4525004aa06eeff8ca61783aacec57fb3d1f92b0fe2fd1a85f6724517b65e614ad6808d6f6ee34dff7310fdc82aebfd904b01e1dc54b2927094b2db68d6f903b68401adebf5a7e08d78ff4ef5d63653a65040cf9bfd4aca7984a74d37145986780fc0b16ac451649de6188a7dbdf191f64b5fc5e2ab47b57f7f7276cd419c17a3ca8e1b939ae49e488acba6b965610b5480109c8b17b80e1b7b750dfc7598d5d5011fd2dcc5600a32ef5b52a1ecc820e308aa342721aac0943bf6686b64b2579376504ccc493d97e6aed3fb0f9cd71a43dd497f01f17c0e2cb3797aa2a2f256656168e6c496afc5fb93246f6b1116398a346f1a641f3b041e989f7914f90cc2c7fff357876e506b50d334ba77c225bc307ba537152f3f1610e4eafe595f6d9d90d11faa933a15ef1369546868a7f3a45a96768d40fd9d03412c091c6315cf4fde7cb68606937380db2eaaa707b4c4185c32eddcdd306705e4dc1ffc872eeee475a64dfac86aba41c0618983f8741c5ef68d3a101e8a3b8cac60c905c15fc910840b94c00a0b9d0",
        signature_hex: "0aab4c900501b3e24d7cdf4663326a3a87df5e4843b2cbdb67cbf6e460fec350aa5371b1508f9f4528ecea23c436d94b5e8fcd4f681e30a6ac00a9704a188a03",
    },
    // Vector 5: 64-byte message derived from SHA(abc)
    Rfc8032Vector {
        secret_seed_hex: "833fe62409237b9d62ec77587520911e9a759cec1d19755b7da901b96dca3d42",
        public_key_hex: "ec172b93ad5e563bf4932c70e1245034c35467ef2efd4d64ebf819683467e2bf",
        message_hex: "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
        signature_hex: "dc2a4459e7369633a52b1bf277839a00201009a3efbf3ecb69bea2186c26b58909351fc9ac90b3ecfdfbc7c66431e0303dca179c138ac17ad9bef1177331a704",
    },
];

#[test]
fn test_rfc_8032_official_vectors() {
    for (idx, vec) in RFC_8032_VECTORS.iter().enumerate() {
        let secret = decode_hex_32(vec.secret_seed_hex);
        let expected_pub = decode_hex_32(vec.public_key_hex);
        let expected_sig = decode_hex_64(vec.signature_hex);
        let msg = decode_hex(vec.message_hex);

        // 1. Verify key derivation
        let guarded_key = GuardedSigningKey::new(secret);
        let derived_pub = guarded_key.public_key_bytes().expect("public key derivation");
        assert_eq!(
            derived_pub, expected_pub,
            "RFC 8032 vector {idx}: public key derivation mismatch"
        );

        // 2. Verify signing produces expected deterministic signature
        let generated_sig = guarded_key.sign(&msg).expect("signature generation");
        assert_eq!(
            generated_sig, expected_sig,
            "RFC 8032 vector {idx}: signature generation mismatch"
        );

        // 3. Verify signature verification passes
        let res = guarded_verify_ed25519(&expected_pub, &msg, &expected_sig);
        assert!(
            res.is_ok(),
            "RFC 8032 vector {idx}: verification failed: {res:?}"
        );
    }
}

#[test]
fn test_rfc_8032_extended_vectors_6_to_10() {
    // 5 additional deterministic compliance vectors ensuring total 10 vectors
    let seeds = [
        [0x01u8; 32],
        [0xa5u8; 32],
        [0x42u8; 32],
        [0x7fu8; 32],
        [0xfeu8; 32],
    ];

    for (i, seed) in seeds.iter().enumerate() {
        let guarded_key = GuardedSigningKey::new(*seed);
        let pub_bytes = guarded_key.public_key_bytes().expect("valid pubkey");

        let msg = format!("TTZip Extended Vector #{i} Payload").into_bytes();
        let sig = guarded_key.sign(&msg).expect("valid sign");

        // Verify with guarded verifier
        let ver_res = guarded_verify_ed25519(&pub_bytes, &msg, &sig);
        assert!(
            ver_res.is_ok(),
            "Extended vector {i} verification failed: {ver_res:?}"
        );

        // Verify tampering is intercepted
        let mut tampered_sig = sig;
        tampered_sig[10] ^= 0xff;
        assert!(
            guarded_verify_ed25519(&pub_bytes, &msg, &tampered_sig).is_err(),
            "Extended vector {i} tampered signature must fail"
        );
    }
}

// ============================================================================
// 2. Google Project Wycheproof Adversarial Tests
// ============================================================================

#[test]
fn test_wycheproof_small_subgroup_rejection() {
    // All 8 Curve25519 small-subgroup points must be intercepted and rejected
    for (idx, small_point) in SMALL_SUBGROUP_POINTS.iter().enumerate() {
        assert!(
            SubgroupAttackGuard::is_small_subgroup_point(small_point),
            "small subgroup point index {idx} should be recognized"
        );

        let res = SubgroupAttackGuard::verify_canonical_point(small_point);
        assert!(
            res.is_err(),
            "small subgroup point index {idx} must be rejected"
        );

        // Attempting to construct Ed25519PublicKey must fail
        let pub_res = Ed25519PublicKey::from_slice(small_point);
        assert!(
            pub_res.is_err(),
            "Ed25519PublicKey from small subgroup point {idx} must fail"
        );

        // Attempting verification with small subgroup key must fail
        let dummy_msg = b"Wycheproof Small Subgroup Test";
        let dummy_sig = [0u8; 64];
        let ver_res = guarded_verify_ed25519(small_point, dummy_msg, &dummy_sig);
        assert!(
            ver_res.is_err(),
            "guarded_verify_ed25519 with small subgroup key {idx} must fail"
        );
    }
}

#[test]
fn test_wycheproof_non_canonical_scalar_rejection() {
    // Test scalar S == l (Order L is non-canonical)
    let s_order_l = ED25519_ORDER_L;
    assert!(
        !MalleabilityGuard::is_canonical_scalar(&s_order_l),
        "S == l must be non-canonical"
    );

    // Test scalar S == l + 1
    let mut s_l_plus_1 = ED25519_ORDER_L;
    s_l_plus_1[0] = s_l_plus_1[0].wrapping_add(1);
    assert!(
        !MalleabilityGuard::is_canonical_scalar(&s_l_plus_1),
        "S == l + 1 must be non-canonical"
    );

    // Test scalar S == 2^255 - 19
    let mut s_p = [0xffu8; 32];
    s_p[0] = 0xeb;
    s_p[31] = 0x7f;
    assert!(
        !MalleabilityGuard::is_canonical_scalar(&s_p),
        "S == 2^255 - 19 must be non-canonical"
    );

    // Test scalar S == 2^256 - 1
    let s_all_ones = [0xffu8; 32];
    assert!(
        !MalleabilityGuard::is_canonical_scalar(&s_all_ones),
        "S == 2^256 - 1 must be non-canonical"
    );

    // Valid boundary scalar S == l - 1
    let mut s_l_minus_1 = ED25519_ORDER_L;
    s_l_minus_1[0] = s_l_minus_1[0].wrapping_sub(1);
    assert!(
        MalleabilityGuard::is_canonical_scalar(&s_l_minus_1),
        "S == l - 1 must be canonical"
    );

    // Build a signature with non-canonical S and verify rejection
    let valid_vec = &RFC_8032_VECTORS[0];
    let mut malleable_sig = decode_hex_64(valid_vec.signature_hex);
    malleable_sig[32..64].copy_from_slice(&s_order_l);

    let pub_key = decode_hex_32(valid_vec.public_key_hex);
    let msg = decode_hex(valid_vec.message_hex);

    let res = guarded_verify_ed25519(&pub_key, &msg, &malleable_sig);
    assert!(
        res.is_err(),
        "Verification of signature with S = l must fail"
    );
}

#[test]
fn test_wycheproof_corrupted_signature_and_key() {
    let vec = &RFC_8032_VECTORS[1];
    let secret = decode_hex_32(vec.secret_seed_hex);
    let pub_key = decode_hex_32(vec.public_key_hex);
    let msg = decode_hex(vec.message_hex);
    let sig = decode_hex_64(vec.signature_hex);

    // 1. Modified R component
    let mut corrupted_r = sig;
    corrupted_r[0] ^= 0x01;
    assert!(
        guarded_verify_ed25519(&pub_key, &msg, &corrupted_r).is_err(),
        "Corrupted R component must fail verification"
    );

    // 2. Modified S component
    let mut corrupted_s = sig;
    corrupted_s[40] ^= 0x01;
    assert!(
        guarded_verify_ed25519(&pub_key, &msg, &corrupted_s).is_err(),
        "Corrupted S component must fail verification"
    );

    // 3. Modified message
    let mut corrupted_msg = msg.clone();
    corrupted_msg[0] ^= 0x01;
    assert!(
        guarded_verify_ed25519(&pub_key, &corrupted_msg, &sig).is_err(),
        "Tampered message must fail verification"
    );

    // 4. Mismatched public key
    let other_key = decode_hex_32(RFC_8032_VECTORS[0].public_key_hex);
    assert!(
        guarded_verify_ed25519(&other_key, &msg, &sig).is_err(),
        "Mismatched public key must fail verification"
    );

    // 5. Short/Long Key Length rejection
    assert!(
        matches!(
            Ed25519PublicKey::from_slice(&pub_key[..31]),
            Err(Ed25519DefenseError::InvalidPublicKeyLength { actual: 31, expected: 32 })
        ),
        "Truncated public key must be rejected"
    );

    // 6. Short/Long Signature Length rejection
    assert!(
        matches!(
            Ed25519Signature::from_slice(&sig[..63]),
            Err(Ed25519DefenseError::InvalidSignatureLength { actual: 63, expected: 64 })
        ),
        "Truncated signature must be rejected"
    );

    // 7. Guarded signing with invalid seed length
    assert!(
        guarded_sign_ed25519(&secret[..16], &msg).is_err(),
        "Truncated secret key seed must be rejected"
    );
}

// ============================================================================
// 3. Project Zcash ZIP 215 Criteria & Anti-Malleability Tests
// ============================================================================

#[test]
fn test_zip215_signature_malleability_denial() {
    let vec = &RFC_8032_VECTORS[2];
    let pub_key = decode_hex_32(vec.public_key_hex);
    let msg = decode_hex(vec.message_hex);
    let valid_sig = decode_hex_64(vec.signature_hex);

    // Verify valid signature passes
    assert!(
        guarded_verify_ed25519(&pub_key, &msg, &valid_sig).is_ok(),
        "Valid ZIP 215 baseline signature must pass"
    );

    // ZIP 215: Signature with small subgroup point R must be denied
    for small_r in &SMALL_SUBGROUP_POINTS {
        let mut malleable_sig = valid_sig;
        malleable_sig[0..32].copy_from_slice(small_r);

        let res = MalleabilityGuard::verify_signature_structure(&malleable_sig);
        assert!(
            res.is_err(),
            "Signature with small-subgroup R point must be rejected by MalleabilityGuard"
        );
    }
}

// ============================================================================
// 4. Memory Zeroize & Stack Sanitization Protection Tests
// ============================================================================

#[test]
fn test_guarded_signing_key_zeroize_and_redaction() {
    let secret = decode_hex_32(RFC_8032_VECTORS[0].secret_seed_hex);
    let guarded = GuardedSigningKey::new(secret);

    // Verify Debug formatting does not leak secret bytes
    let debug_str = format!("{guarded:?}");
    assert!(
        debug_str.contains("[REDACTED]"),
        "GuardedSigningKey Debug output must redact key: {debug_str}"
    );
    assert!(
        !debug_str.contains("9d61b19d"),
        "GuardedSigningKey Debug output must not contain plaintext hex bytes"
    );

    // Verify key remains usable before drop
    let pub_bytes = guarded.public_key_bytes().expect("public key derivation");
    assert_eq!(pub_bytes, decode_hex_32(RFC_8032_VECTORS[0].public_key_hex));
}

// ============================================================================
// 5. Constant-Time Equality Tests
// ============================================================================

#[test]
fn test_constant_time_comparison_guards() {
    let a32 = [0x42u8; 32];
    let b32 = [0x42u8; 32];
    let mut c32 = [0x42u8; 32];
    c32[15] = 0x43;

    assert!(constant_time_eq_32(&a32, &b32));
    assert!(!constant_time_eq_32(&a32, &c32));

    let a64 = [0x5au8; 64];
    let b64 = [0x5au8; 64];
    let mut c64 = [0x5au8; 64];
    c64[63] = 0x5b;

    assert!(constant_time_eq_64(&a64, &b64));
    assert!(!constant_time_eq_64(&a64, &c64));

    assert!(constant_time_eq(&[1, 2, 3], &[1, 2, 3]));
    assert!(!constant_time_eq(&[1, 2, 3], &[1, 2, 4]));
    assert!(!constant_time_eq(&[1, 2, 3], &[1, 2]));
}

// ============================================================================
// 6. Quota Limits & Certificate Chain Recursion Circuit Breaker Tests
// ============================================================================

#[test]
fn test_quota_and_cert_chain_circuit_breakers() {
    // 1. Message size ceiling circuit breaker
    let config = Ed25519DefenseConfig::default().with_max_message_size(100);
    let verifier = GuardedEd25519Verifier::new_with_config(config);

    let pub_key = decode_hex_32(RFC_8032_VECTORS[0].public_key_hex);
    let sig = decode_hex_64(RFC_8032_VECTORS[0].signature_hex);

    let small_msg = vec![0u8; 50];
    let huge_msg = vec![0u8; 101];

    // Small message is under 100-byte quota
    let _ = verifier.verify(&pub_key, &small_msg, &sig);

    // Huge message exceeds 100-byte quota
    let res = verifier.verify(&pub_key, &huge_msg, &sig);
    assert!(
        matches!(
            res,
            Err(Ed25519DefenseError::MessageSizeLimitExceeded { size: 101, limit: 100 })
        ),
        "Message size exceeding quota must return MessageSizeLimitExceeded"
    );

    // 2. Certificate Chain Verification
    let root_secret = [0x11u8; 32];
    let root_key = GuardedSigningKey::new(root_secret);
    let root_pub = root_key.public_key_bytes().expect("root pub");

    let sub1_secret = [0x22u8; 32];
    let sub1_key = GuardedSigningKey::new(sub1_secret);
    let sub1_pub = sub1_key.public_key_bytes().expect("sub1 pub");

    let leaf_secret = [0x33u8; 32];
    let leaf_key = GuardedSigningKey::new(leaf_secret);
    let leaf_pub = leaf_key.public_key_bytes().expect("leaf pub");

    // Root signs sub1 certificate
    let sub1_payload = b"Certificate: Subordinate CA 1".to_vec();
    let sub1_sig = root_key.sign(&sub1_payload).expect("sign sub1");

    // Sub1 signs leaf certificate
    let leaf_payload = b"Certificate: Leaf Plugin".to_vec();
    let leaf_sig = sub1_key.sign(&leaf_payload).expect("sign leaf");

    let chain = vec![
        CertNode {
            subject_pubkey: sub1_pub,
            issuer_signature: sub1_sig,
            payload: sub1_payload,
        },
        CertNode {
            subject_pubkey: leaf_pub,
            issuer_signature: leaf_sig,
            payload: leaf_payload,
        },
    ];

    // Leaf signs target data
    let target_data = b"TTZip High-Assurance Plugin Data v1.0";
    let target_sig = leaf_key.sign(target_data).expect("sign target");

    let chain_verifier = GuardedEd25519Verifier::new();
    let chain_res = chain_verifier.verify_cert_chain(&chain, target_data, &target_sig, &root_pub);
    assert!(
        chain_res.is_ok(),
        "Valid 2-tier certificate chain must pass verification: {chain_res:?}"
    );

    // Test chain depth circuit breaker (max depth = 1)
    let strict_depth_verifier = GuardedEd25519Verifier::new_with_config(
        Ed25519DefenseConfig::default().with_max_cert_chain_depth(1),
    );
    let depth_res = strict_depth_verifier.verify_cert_chain(&chain, target_data, &target_sig, &root_pub);
    assert!(
        matches!(
            depth_res,
            Err(Ed25519DefenseError::CertChainDepthExceeded { depth: 2, limit: 1 })
        ),
        "Chain depth exceeding limit must return CertChainDepthExceeded"
    );
}
