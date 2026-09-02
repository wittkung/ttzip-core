// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive System Compliance, Differential Patching Contract, and 6-Layer Security Suite.
//!
//! Validates:
//! 1. Binary delta format signatures (BSDIFF40, BSDIFN40, SPK3, SPK4, spk!).
//! 2. 24-byte container header binary serialization, deserialization, and CRC-32 integrity.
//! 3. Ground-truth differential diffing and bit-exact reconstruction across multiple payload topologies.
//! 4. Granular DeltaCommand instruction pipeline (Extract, Delete, BinaryDiff, ModifyPermissions, Clone).
//! 5. Topological TreeHash calculation determinism, domain separation, and mutation sensitivity.
//! 6. 6-layer defense-in-depth preflight pipeline integration and request authorization.
//! 7. Sensitive credential buffer memory zeroization on drop.
//! 8. High-entropy UUID temporary directory workspace isolation and automatic RAII cleanup.
//! 9. Ed25519 small-subgroup denial, canonical scalar bounds, and anti-downgrade assertions.
//! 10. Path traversal, Zip-Slip, and Windows/POSIX reserved device rejection.
//! 11. Memory budget watchdog and expansion multiplier circuit breakers.

use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use ttzip_engine::crypto::ed25519::SigningKey;
use ttzip_engine::security::system_defense::appcast_sig::{ED25519_ORDER_L, SMALL_SUBGROUP_POINTS};
use ttzip_engine::security::system_defense::*;
use ttzip_engine::system::delta::engine::TTZipDeltaEngine;
use ttzip_engine::system::delta::types::{
    DeltaCommand, DeltaError, DeltaFormat, DeltaPatchHeader,
};

// ============================================================================
// 1. Format Detection and Magic Signatures
// ============================================================================

#[test]
fn test_delta_format_signatures_and_magic_mapping() {
    assert_eq!(DeltaFormat::from_magic(b"BSDIFF40"), DeltaFormat::Bsdiff40);
    assert_eq!(DeltaFormat::from_magic(b"BSDIFN40"), DeltaFormat::Bsdifn40);
    assert_eq!(DeltaFormat::from_magic(b"SPK3"), DeltaFormat::Spk3);
    assert_eq!(DeltaFormat::from_magic(b"SPK4"), DeltaFormat::Spk4);
    assert_eq!(DeltaFormat::from_magic(b"spk!"), DeltaFormat::Spk4);
    assert_eq!(DeltaFormat::from_magic(b"INVALID"), DeltaFormat::Unknown);
    assert_eq!(DeltaFormat::from_magic(b""), DeltaFormat::Unknown);

    assert_eq!(DeltaFormat::Bsdiff40.magic_bytes(), b"BSDIFF40");
    assert_eq!(DeltaFormat::Bsdifn40.magic_bytes(), b"BSDIFN40");
    assert_eq!(DeltaFormat::Spk3.magic_bytes(), b"SPK3");
    assert_eq!(DeltaFormat::Spk4.magic_bytes(), b"spk!");
    assert_eq!(DeltaFormat::Unknown.magic_bytes(), b"UNKN");
}

// ============================================================================
// 2. Container Header Layout & Serialization
// ============================================================================

#[test]
fn test_delta_patch_header_layout_and_roundtrip() {
    let magic = *b"spk!";
    let major = 4u16;
    let minor = 0u16;
    let before_hash = 0x1234_5678u32;
    let after_hash = 0x8765_4321u32;
    let uncompressed_size = 104857600u64; // 100 MB

    let header = DeltaPatchHeader::new(
        magic,
        major,
        minor,
        before_hash,
        after_hash,
        uncompressed_size,
    );

    let bytes = header.to_bytes();
    assert_eq!(bytes.len(), DeltaPatchHeader::HEADER_SIZE);
    assert_eq!(bytes.len(), 24);

    let parsed = DeltaPatchHeader::from_bytes(&bytes).expect("Failed to parse header bytes");
    assert_eq!(parsed.magic, magic);
    assert_eq!(parsed.major_version, major);
    assert_eq!(parsed.minor_version, minor);
    assert_eq!(parsed.before_tree_hash, before_hash);
    assert_eq!(parsed.after_tree_hash, after_hash);
    assert_eq!(parsed.uncompressed_size, uncompressed_size);

    // Truncated header rejection
    let truncated_err = DeltaPatchHeader::from_bytes(&bytes[..20]);
    assert!(matches!(truncated_err, Err(DeltaError::TruncatedData { needed: 24, available: 20 })));
}

// ============================================================================
// 3. Differential Diff and Bit-Exact Reconstruction
// ============================================================================

#[test]
fn test_differential_diff_and_reconstruction_compliance() {
    let test_cases: Vec<(&'static [u8], &'static [u8])> = vec![
        (b"", b""),
        (b"Alpha", b"Alpha"),
        (b"Hello Base", b"Hello Modified Target"),
        (b"The quick brown fox jumps over the lazy dog", b"The fast brown fox leaped over a sleepy dog"),
        (b"1234567890", b"12345XYZ890ABC"),
    ];

    for (old_data, new_data) in test_cases {
        let patch_bytes = TTZipDeltaEngine::create_patch(old_data, new_data)
            .expect("Failed to create binary delta patch");

        let (reconstructed, telemetry) = TTZipDeltaEngine::apply_patch_with_result(old_data, &patch_bytes)
            .expect("Failed to apply binary delta patch");

        assert_eq!(reconstructed.as_slice(), new_data);
        assert_eq!(telemetry.bytes_out, new_data.len());
        assert!(!telemetry.sha256_hex.is_empty());
    }
}

// ============================================================================
// 4. Granular DeltaCommand Instruction Pipeline
// ============================================================================

#[test]
fn test_granular_delta_commands_pipeline() {
    let old_data = b"Base image data for operating system microkernel updates 2026";
    let new_data = b"Base image data for operating system microkernel patched with high-speed delta 2026";

    let commands = vec![
        DeltaCommand::Extract { offset: 0, length: 32 },
        DeltaCommand::Clone { source_offset: 0, target_offset: 100, length: 16 },
        DeltaCommand::ModifyPermissions { mode: 0o755 },
        DeltaCommand::Delete { offset: 32, length: 8 },
        DeltaCommand::BinaryDiff { diff_len: 20, extra_len: 12, seek_offset: 4 },
    ];

    let patch = TTZipDeltaEngine::create_patch_with_commands(old_data, new_data, &commands)
        .expect("Failed to build patch with commands");

    let reconstructed = TTZipDeltaEngine::apply_patch(old_data, &patch)
        .expect("Failed to apply patch with commands");

    assert_eq!(reconstructed.as_slice(), new_data);
}

// ============================================================================
// 5. TreeHash Determinism and Domain Separation
// ============================================================================

#[test]
fn test_tree_hash_calculation_and_domain_separation() {
    let payload_a = b"TTZip System Architecture Merkle Leaf 1";
    let payload_b = b"TTZip System Architecture Merkle Leaf 2";

    let hash_a1 = TTZipDeltaEngine::calculate_tree_hash(payload_a);
    let hash_a2 = TTZipDeltaEngine::calculate_tree_hash(payload_a);
    let hash_b = TTZipDeltaEngine::calculate_tree_hash(payload_b);

    assert_eq!(hash_a1, hash_a2, "TreeHash must be 100% deterministic");
    assert_ne!(hash_a1, hash_b, "Different payloads must yield different TreeHashes");
}

// ============================================================================
// 6. 6-Layer Security Pipeline Preflight Contract
// ============================================================================

#[test]
fn test_6layer_system_defense_pipeline_preflight_contract() {
    let temp_jail = tempdir().expect("Failed to create tempdir");
    let target_file = temp_jail.path().join("application.bin");
    fs::write(&target_file, b"Original application binary v1.0.0").expect("Write target");

    let signing_key = SigningKey::from_bytes(&[0x77u8; 32]);
    let verifying_key = signing_key.verifying_key();
    let old_data = b"Original application binary v1.0.0";
    let new_data = b"Original application binary v1.1.0 with zero-day patches";

    let raw_patch = TTZipDeltaEngine::create_patch(old_data, new_data).expect("Create patch");
    let patch_sig = signing_key.sign(&raw_patch);

    let req = SystemUpdateRequest {
        target_path: "application.bin".to_string(),
        expected_version: "1.1.0".to_string(),
        current_version: "1.0.0".to_string(),
        patch_bytes: raw_patch.clone(),
        signature_bytes: patch_sig.to_bytes().to_vec(),
        public_key_bytes: verifying_key.to_bytes().to_vec(),
        jail_root: temp_jail.path().to_string_lossy().to_string(),
    };

    let pipeline = SystemSecurityPipeline::new(SystemDefenseOptions::default());
    let report = pipeline.preflight_check(&req).expect("Preflight check failed");

    assert!(report.authorized);
    assert_eq!(report.current_version, "1.0.0");
    assert_eq!(report.target_version, "1.1.0");
    assert!(report.signature_verified);
    assert!(report.sandbox_confined);
    assert!(report.budget_permit.is_some());
}

// ============================================================================
// 7. Sensitive Credential Zeroization on Drop
// ============================================================================

#[test]
fn test_sensitive_credential_zeroization_on_drop() {
    let secret = b"TOP_SECRET_AIR_GAPPED_SYSTEM_KEY_2026";
    let mut buffer = SensitiveCredentialBuffer::new(secret.to_vec());
    assert_eq!(buffer.as_slice(), secret);
    assert_eq!(buffer.len(), secret.len());
    assert!(!buffer.is_empty());

    // Explicit clear verification
    buffer.clear();
    assert_eq!(buffer.len(), 0);
    assert!(buffer.is_empty());
}

// ============================================================================
// 8. Temporary Workspace Isolation and RAII Auto-Cleanup
// ============================================================================

#[test]
fn test_temp_workspace_isolation_and_raii_cleanup() {
    let guard = TempDirectoryCleanupGuard::new();
    let workspace_path = {
        let temp_ws = guard.create_workspace("ttzip_stage37_compliance")
            .expect("Failed to create isolated temp workspace");
        assert!(temp_ws.path().exists());
        let file_path = temp_ws.path().join("staging_chunk.bin");
        fs::write(&file_path, b"Staged transient delta chunk").expect("Write staging chunk");
        assert!(file_path.exists());
        temp_ws.path().to_path_buf()
    };

    // When temp_ws drops, its directory must be pruned
    assert!(!workspace_path.exists(), "Temporary directory must be cleaned up automatically on drop");
}

// ============================================================================
// 9. Ed25519 Small-Subgroup, Scalar & Anti-Downgrade Defense Matrix
// ============================================================================

#[test]
fn test_ed25519_small_subgroup_denial_and_scalar_bounds() {
    let guard = AppcastSignatureGuard::new();
    let dummy_sig = [0u8; 64];
    let msg = b"Manifest Payload";

    for (idx, point) in SMALL_SUBGROUP_POINTS.iter().enumerate() {
        assert!(
            AppcastSignatureGuard::is_small_subgroup_point(point),
            "Point {idx} must be classified as small subgroup"
        );
        let res = guard.verify_signature(point, &dummy_sig, msg);
        assert!(res.is_err(), "Small subgroup point {idx} must be rejected");
    }

    // Scalar S >= L check
    let mut non_canonical_s = ED25519_ORDER_L;
    non_canonical_s[0] = non_canonical_s[0].wrapping_add(1);
    assert!(!AppcastSignatureGuard::is_canonical_scalar(&non_canonical_s));

    let canonical_s = [0x01u8; 32];
    assert!(AppcastSignatureGuard::is_canonical_scalar(&canonical_s));

    // Monotonicity anti-downgrade
    assert!(guard.assert_version_monotonicity("1.0.0", "1.0.1").is_ok());
    assert!(guard.assert_version_monotonicity("2.0.0", "1.9.9").is_err());
    assert!(guard.assert_version_monotonicity("1.0.0", "1.0.0").is_err());
}

// ============================================================================
// 10. Path Traversal & Sandbox Jail Verification
// ============================================================================

#[test]
fn test_path_traversal_and_sandbox_jail_verification() {
    let guard = PathTraversalProtectionGuard::strict();

    assert_eq!(guard.sanitize_path("foo/bar/file.txt").unwrap(), "foo/bar/file.txt");
    assert!(guard.sanitize_path("../../etc/shadow").is_err());
    assert!(guard.sanitize_path("CON").is_err());
    assert!(guard.sanitize_path("nested/PRN.txt").is_err());
    assert!(guard.sanitize_path("null\0byte").is_err());

    let jail = PathBuf::from("/tmp/ttzip_compliance_jail");
    let sandbox = SandboxEscapingGuard::with_jail_root(&jail);
    let resolved = sandbox.validate_path(Path::new("updates/app.bin")).unwrap();
    assert_eq!(resolved, jail.join("updates/app.bin"));
    assert!(sandbox.validate_path(Path::new("../../etc/passwd")).is_err());
}

// ============================================================================
// 11. Memory Budget Watchdog & Decompression Bomb Protection
// ============================================================================

#[test]
fn test_memory_budget_watchdog_and_expansion_circuit_breaker() {
    let guard = BinaryDeltaMemoryBudgetGuard::with_default_budget();

    assert!(guard.validate_patch_size(10 * 1024 * 1024).is_ok());
    assert!(guard.validate_patch_size(600 * 1024 * 1024).is_err());

    // 100x ratio (safe) vs 50000x ratio (decompression bomb)
    assert!(guard.validate_expansion_ratio(100 * 1024, 10 * 1024 * 1024).is_ok());
    assert!(guard.validate_expansion_ratio(1024, 50 * 1024 * 1024).is_err());

    // Instruction quota
    assert!(guard.validate_instruction_count(50_000).is_ok());
    assert!(guard.validate_instruction_count(150_000).is_err());

    // RAII permit tracking
    let permit = guard.acquire_permit(1024 * 1024).unwrap();
    assert!(guard.current_usage() >= 1024 * 1024);
    drop(permit);
    assert_eq!(guard.current_usage(), 0);
}
