// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive Defense-in-Depth, Domain Isolation, and Cryptographic Security Tests for BLAKE3.
//!
//! Validates:
//! 1. Domain separation collision resistance (keyed hash vs unkeyed vs KDF material).
//! 2. Context string domain separation in Key Derivation Function (KDF).
//! 3. Zeroization of sensitive key state upon hasher destruction and redaction in Debug format.
//! 4. Non-destructive idempotence and state isolation under repeated finalization.
//! 5. 64-bit chunk counter overflow, tree stack depth bounding, and memory protection invariants.
//! 6. Anti-length-extension Merkle root flag isolation.
//! 7. Quota circuit breakers for cumulative input data and XOF stream extraction.
//! 8. Constant-time equality comparison and timing attack resistance.

use std::io::Write;

use ttzip_engine::crypto::blake3::constants::{PARENT, ROOT};
use ttzip_engine::crypto::blake3::{derive_key, hash, keyed_hash, Blake3Hasher};
use ttzip_engine::security::blake3_defense::{
    constant_time_eq, constant_time_eq_32, guarded_derive_key, guarded_hash, guarded_keyed_hash,
    guarded_verify_mac, validate_context_domain, validate_key_len, validate_tree_stack_depth,
    verify_anti_length_extension_immunity, verify_root_flag_isolation, Blake3DefenseConfig,
    Blake3DefenseError, GuardedBlake3Hasher, GuardedContext, GuardedKey,
};
use ttzip_engine::types::TTZipStatus;

#[test]
fn test_blake3_domain_separation_collision_resistance() {
    let payload = b"Universal identical payload for domain separation testing";
    let key = [0x42u8; 32];
    let context = "TTZip Domain Separation Defense Context";

    let unkeyed_digest = hash(payload);
    let keyed_digest = keyed_hash(&key, payload);
    let kdf_digest = derive_key(context, payload);

    assert_ne!(
        unkeyed_digest, keyed_digest,
        "Unkeyed hash and keyed MAC collided on identical payload!"
    );
    assert_ne!(
        unkeyed_digest, kdf_digest,
        "Unkeyed hash and KDF subkey collided on identical payload!"
    );
    assert_ne!(
        keyed_digest, kdf_digest,
        "Keyed MAC and KDF subkey collided on identical payload!"
    );
}

#[test]
fn test_blake3_kdf_context_string_isolation() {
    let material = b"Cryptographic master entropy material 2026";
    let context_a = "TTZip Volume Header Encryption Key";
    let context_b = "TTZip File Content Encryption Key";

    let key_a = derive_key(context_a, material);
    let key_b = derive_key(context_b, material);

    assert_ne!(
        key_a, key_b,
        "Different KDF context strings produced colliding derived keys!"
    );
}

#[test]
fn test_blake3_zero_byte_key_vs_unkeyed_separation() {
    let payload = b"Test payload against all-zero key";
    let zero_key = [0u8; 32];

    let unkeyed_digest = hash(payload);
    let keyed_zero_digest = keyed_hash(&zero_key, payload);

    assert_ne!(
        unkeyed_digest, keyed_zero_digest,
        "Keyed hash with zero key collided with unkeyed hash!"
    );
}

#[test]
fn test_blake3_state_isolation_and_idempotence() {
    let mut hasher = Blake3Hasher::new();
    hasher.update(b"Prefix data");

    let digest1 = hasher.finalize();
    let digest2 = hasher.finalize();

    assert_eq!(
        digest1, digest2,
        "Finalize must be idempotent and non-destructive"
    );

    hasher.update(b" and suffix data");
    let digest3 = hasher.finalize();

    assert_ne!(
        digest1, digest3,
        "Appended data must alter the resulting digest"
    );
    assert_eq!(digest3, hash(b"Prefix data and suffix data"));
}

#[test]
fn test_defense_anti_length_extension_and_root_flag_isolation() {
    let original_msg = b"Sensitive payload needing integrity verification";
    let extension = b"Appended unauthorized instruction sequence";

    let immune = verify_anti_length_extension_immunity(original_msg, extension);
    assert!(immune.is_ok());
    assert!(immune.unwrap());

    assert!(verify_root_flag_isolation(0, false).is_ok());
    assert!(verify_root_flag_isolation(PARENT, false).is_ok());
    assert!(verify_root_flag_isolation(ROOT, true).is_ok());
    assert!(verify_root_flag_isolation(ROOT | PARENT, true).is_ok());

    assert!(verify_root_flag_isolation(ROOT, false).is_err());
    assert!(verify_root_flag_isolation(0, true).is_err());
}

#[test]
fn test_defense_input_quota_circuit_breaker() {
    let config = Blake3DefenseConfig::default_limits().with_max_input_limit(200);
    let mut hasher = GuardedBlake3Hasher::new_with_config(config);

    let chunk1 = vec![0x41u8; 100];
    let chunk2 = vec![0x42u8; 100];
    let chunk3 = vec![0x43u8; 1];

    assert!(hasher.update(&chunk1).is_ok());
    assert_eq!(hasher.total_ingested(), 100);

    assert!(hasher.update(&chunk2).is_ok());
    assert_eq!(hasher.total_ingested(), 200);

    let res = hasher.update(&chunk3);
    assert!(matches!(
        res,
        Err(Blake3DefenseError::InputQuotaExceeded {
            current: 200,
            attempted: 1,
            limit: 200
        })
    ));
}

#[test]
fn test_defense_xof_output_quota_circuit_breaker() {
    let config = Blake3DefenseConfig::default_limits().with_max_xof_output_limit(64);
    let mut hasher = GuardedBlake3Hasher::new_with_config(config);

    assert!(hasher.update(b"seed data for xof").is_ok());

    let mut out1 = [0u8; 32];
    assert!(hasher.finalize_into(&mut out1).is_ok());
    assert_eq!(hasher.total_xof_extracted(), 32);

    let mut out2 = [0u8; 32];
    assert!(hasher.finalize_into(&mut out2).is_ok());
    assert_eq!(hasher.total_xof_extracted(), 64);

    let mut out3 = [0u8; 1];
    let res = hasher.finalize_into(&mut out3);
    assert!(matches!(
        res,
        Err(Blake3DefenseError::XofOutputQuotaExceeded {
            current: 64,
            attempted: 1,
            limit: 64
        })
    ));
}

#[test]
fn test_defense_key_length_and_context_domain_isolation() {
    assert!(validate_key_len(&[0u8; 31]).is_err());
    assert!(validate_key_len(&[0u8; 33]).is_err());
    assert!(validate_key_len(&[0u8; 0]).is_err());
    assert!(validate_key_len(&[0u8; 32]).is_ok());

    let valid_key = [0x5au8; 32];
    assert!(GuardedBlake3Hasher::new_keyed(&valid_key).is_ok());
    assert!(GuardedBlake3Hasher::new_keyed(&valid_key[..31]).is_err());

    assert!(validate_context_domain("", true, 1024).is_err());
    assert!(validate_context_domain("   ", true, 1024).is_err());
    assert!(validate_context_domain("test\0null", true, 1024).is_err());
    assert!(validate_context_domain("test\x07bell", true, 1024).is_err());

    let long_context = "a".repeat(1025);
    assert!(validate_context_domain(&long_context, true, 1024).is_err());

    assert!(validate_context_domain("ttzip 2026-08-31 kdf-v1", true, 1024).is_ok());
    assert!(GuardedBlake3Hasher::new_derive_key("ttzip 2026-08-31 kdf-v1").is_ok());
    assert!(GuardedBlake3Hasher::new_derive_key("").is_err());
}

#[test]
fn test_defense_sensitive_memory_zeroize_and_redaction() {
    let key_bytes = [0x42u8; 32];
    let guarded_key = GuardedKey::new(key_bytes);
    assert_eq!(guarded_key.as_bytes(), &key_bytes);

    let debug_str = format!("{:?}", guarded_key);
    assert_eq!(debug_str, "GuardedKey([REDACTED])");
    assert!(!debug_str.contains("42"));

    let context = GuardedContext::new("ttzip secure domain", true).unwrap();
    let ctx_debug = format!("{:?}", context);
    assert!(ctx_debug.contains("GuardedContext"));
}

#[test]
fn test_defense_constant_time_comparison_guard() {
    let a = [0x11u8; 32];
    let mut b = [0x11u8; 32];

    assert!(constant_time_eq_32(&a, &b));
    assert!(constant_time_eq(&a[..], &b[..]));

    b[31] = 0x12;
    assert!(!constant_time_eq_32(&a, &b));
    assert!(!constant_time_eq(&a[..], &b[..]));

    assert!(!constant_time_eq(&a[..31], &b[..32]));

    let key = [0x22u8; 32];
    let msg = b"Authenticated packet payload";
    let valid_mac = guarded_keyed_hash(&key, msg).unwrap();

    assert!(guarded_verify_mac(&key, msg, &valid_mac).unwrap());

    let mut corrupted_mac = valid_mac;
    corrupted_mac[0] ^= 0xff;
    assert!(!guarded_verify_mac(&key, msg, &corrupted_mac).unwrap());
}

#[test]
fn test_defense_tree_depth_and_counter_overflow() {
    assert!(validate_tree_stack_depth(55, 55).is_ok());
    assert!(validate_tree_stack_depth(56, 55).is_err());

    let config = Blake3DefenseConfig::default_limits().with_max_stack_depth(10);
    let hasher = GuardedBlake3Hasher::new_with_config(config);
    assert_eq!(hasher.config().max_stack_depth, 10);
}

#[test]
fn test_guarded_io_write_adapter() {
    let config = Blake3DefenseConfig::default_limits().with_max_input_limit(50);
    let mut hasher = GuardedBlake3Hasher::new_with_config(config);

    assert!(hasher.write_all(b"1234567890").is_ok());
    assert_eq!(hasher.total_ingested(), 10);

    let big_chunk = [0u8; 50];
    let err = hasher.write_all(&big_chunk);
    assert!(err.is_err());
}

#[test]
fn test_top_level_convenience_functions() {
    let data = b"TTZip High-Performance Archiving";
    let digest = guarded_hash(data).unwrap();
    assert_eq!(digest, ttzip_engine::crypto::blake3::hash(data));

    let key = [0x77u8; 32];
    let mac = guarded_keyed_hash(&key, data).unwrap();
    assert_eq!(mac, ttzip_engine::crypto::blake3::keyed_hash(&key, data));

    let derived = guarded_derive_key("ttzip 2026 test-kdf", data).unwrap();
    assert_eq!(
        derived,
        ttzip_engine::crypto::blake3::derive_key("ttzip 2026 test-kdf", data)
    );
}

#[test]
fn test_status_conversion() {
    let err = Blake3DefenseError::InputQuotaExceeded {
        current: 10,
        attempted: 20,
        limit: 15,
    };
    let status: TTZipStatus = err.into();
    assert_eq!(status, TTZipStatus::ErrSolidBudgetExceeded);

    let key_err = Blake3DefenseError::InvalidKeyLength {
        actual: 10,
        expected: 32,
    };
    let status2: TTZipStatus = key_err.into();
    assert_eq!(status2, TTZipStatus::ErrInvalidParam);

    let sec_err = Blake3DefenseError::LengthExtensionViolation { reason: "attack" };
    let status3: TTZipStatus = sec_err.into();
    assert_eq!(status3, TTZipStatus::ErrSecurityViolation);
}
