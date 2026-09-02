// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unit tests for system defense guards and invariants.

use std::path::{Path, PathBuf};

use crate::crypto::ed25519::signing::SigningKey;
use super::*;

#[test]
fn test_sandbox_escaping_guard_basic() {
    let jail = PathBuf::from("/tmp/ttzip_unit_jail");
    let guard = SandboxEscapingGuard::with_jail_root(&jail);

    let norm = SandboxEscapingGuard::normalize_lexical_path(Path::new("a/b/../c/./d"));
    assert_eq!(norm, PathBuf::from("a/c/d"));

    let valid_path = guard.validate_path(Path::new("updates/patch.bin")).unwrap();
    assert_eq!(valid_path, jail.join("updates/patch.bin"));

    let invalid_path = guard.validate_path(Path::new("../../etc/passwd"));
    assert!(invalid_path.is_err());
}

#[test]
fn test_delta_budget_guard_basic() {
    let guard = BinaryDeltaMemoryBudgetGuard::with_default_budget();

    assert!(guard.validate_patch_size(1024).is_ok());
    assert!(guard.validate_patch_size(1024 * 1024 * 1024).is_err());

    assert!(guard.validate_expansion_ratio(1024, 100 * 1024).is_ok());
    assert!(guard.validate_expansion_ratio(10, 10 * 1024 * 1024).is_err());

    assert!(guard.validate_instruction_count(10_000).is_ok());
    assert!(guard.validate_instruction_count(200_000).is_err());
}

#[test]
fn test_appcast_sig_guard_basic() {
    let guard = AppcastSignatureGuard::new();
    let seed = [0x11u8; 32];
    let signing_key = SigningKey::from_bytes(&seed);
    let pub_key = signing_key.verifying_key().to_bytes();

    let msg = b"Release payload 1.0.0";
    let sig = signing_key.sign(msg).to_bytes();

    assert!(guard.verify_signature(&pub_key, &sig, msg).is_ok());
    assert!(guard.verify_signature(&pub_key, &sig, b"Tampered").is_err());

    assert!(guard.assert_version_monotonicity("1.0.0", "1.1.0").is_ok());
    assert!(guard.assert_version_monotonicity("2.0.0", "1.9.0").is_err());
}

#[test]
fn test_temp_cleanup_guard_basic() {
    let guard = TempDirectoryCleanupGuard::new();
    let uuid = TempDirectoryCleanupGuard::generate_uuid_v4();
    assert_eq!(uuid.len(), 36);
    assert_eq!(uuid.chars().nth(14), Some('4'));

    let temp_dir = guard.create_isolated_temp_dir("ttzip_unit_temp", None).unwrap();
    let path = temp_dir.path().to_path_buf();
    assert!(path.exists());
    drop(temp_dir);
    assert!(!path.exists());
}

#[test]
fn test_path_traversal_guard_basic() {
    let guard = PathTraversalProtectionGuard::strict();

    assert_eq!(guard.sanitize_path("foo/bar/test.txt").unwrap(), "foo/bar/test.txt");
    assert!(guard.sanitize_path("../escaped.txt").is_err());
    assert!(guard.sanitize_path("CON").is_err());
    assert!(guard.sanitize_path("foo/NUL/bar").is_err());
    assert!(guard.sanitize_path("null\0byte").is_err());
}

#[test]
fn test_sensitive_credential_basic() {
    let buf = SensitiveCredentialBuffer::from_slice(b"secret_key_123");
    assert_eq!(buf.expose_secret(), b"secret_key_123");
    assert_eq!(buf.len(), 14);

    let s = SensitiveCredentialString::from_str_slice("pass_token");
    assert_eq!(s.expose_secret(), "pass_token");
}

#[test]
fn test_pipeline_preflight_flow() {
    let pipeline = SystemSecurityPipeline::strict_default();
    let seed = [0x22u8; 32];
    let signing_key = SigningKey::from_bytes(&seed);
    let pub_key = signing_key.verifying_key().to_bytes();
    let payload = b"Appcast 3.0.0";
    let sig = signing_key.sign(payload).to_bytes();

    let req = SystemUpdateRequest {
        target_path: "updates/app.pkg".to_string(),
        expected_version: "3.0.0".to_string(),
        current_version: "2.0.0".to_string(),
        patch_bytes: payload.to_vec(),
        signature_bytes: sig.to_vec(),
        public_key_bytes: pub_key.to_vec(),
        jail_root: String::new(),
    };

    let report = pipeline.validate_update_preflight(&req).unwrap();
    assert!(report.authorized);
    assert!(report.is_signature_valid);
    assert!(report.is_version_upgrade_valid);
    assert!(report.is_budget_approved);
}
