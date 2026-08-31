// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration tests for SecurePathExtractor sandbox isolation,
//! Zip-Slip traversal defense, intermediate symlink escape interception,
//! and bottom-up POSIX metadata restoration.

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tempfile::tempdir;
use ttzip_engine::security::{DeferredSecureEntry, SecurePathExtractor, SecurityFlags};
use ttzip_engine::types::TTZipStatus;

#[test]
fn test_security_flags_operations() {
    let flags = SecurityFlags::SECURE_NODOTDOT | SecurityFlags::SECURE_SYMLINKS;
    assert!(flags.contains(SecurityFlags::SECURE_NODOTDOT));
    assert!(flags.contains(SecurityFlags::SECURE_SYMLINKS));
    assert!(!flags.contains(SecurityFlags::SECURE_NOABSOLUTEPATHS));

    let mut mutable_flags = SecurityFlags::empty();
    assert!(mutable_flags.is_empty());
    mutable_flags.insert(SecurityFlags::SECURE_UNLINK_FIRST);
    assert!(mutable_flags.contains(SecurityFlags::SECURE_UNLINK_FIRST));

    mutable_flags.toggle(SecurityFlags::SECURE_UNLINK_FIRST);
    assert!(mutable_flags.is_empty());

    let all = SecurityFlags::all();
    assert!(all.contains(SecurityFlags::DEFAULT));
    assert_eq!(all.bits(), 0b11111);

    let diff = all.difference(SecurityFlags::RESTORE_PERMISSIONS);
    assert!(!diff.contains(SecurityFlags::RESTORE_PERMISSIONS));
    assert!(diff.contains(SecurityFlags::SECURE_NODOTDOT));
}

#[test]
fn test_deferred_entry_depth_sorting() {
    let e1 = DeferredSecureEntry {
        rel_path: Path::new("a").to_path_buf(),
        mode: 0o755,
        mtime_epoch_secs: 100,
        mtime_nanos: 0,
        is_directory: true,
    };
    let e2 = DeferredSecureEntry {
        rel_path: Path::new("a/b/c").to_path_buf(),
        mode: 0o755,
        mtime_epoch_secs: 100,
        mtime_nanos: 0,
        is_directory: true,
    };
    assert_eq!(e1.depth(), 1);
    assert_eq!(e2.depth(), 3);
    assert!(e2.depth() > e1.depth());
}

#[test]
fn test_zipslip_traversal_interception() {
    let tmp = tempdir().expect("Failed to create tempdir");
    let extractor = SecurePathExtractor::new(tmp.path(), SecurityFlags::DEFAULT)
        .expect("Extractor should initialize");

    // 1. Classic dot-dot escape
    assert_eq!(
        extractor.sanitize_and_validate_path("../../../../etc/passwd"),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 2. Nested parent traversal escape
    assert_eq!(
        extractor.sanitize_and_validate_path("foo/bar/../../../etc/shadow"),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 3. Current dir dot-dot escape
    assert_eq!(
        extractor.sanitize_and_validate_path("./../secret"),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 4. Backslash escaped traversal
    assert_eq!(
        extractor.sanitize_and_validate_path(r"a\b\..\..\..\..\escape.sh"),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 5. Multi-dot obfuscation
    assert_eq!(
        extractor.sanitize_and_validate_path(".../evil.sh"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        extractor.sanitize_and_validate_path("..../payload"),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 6. Null byte injection
    assert_eq!(
        extractor.sanitize_and_validate_path("valid_file.txt\0/malicious"),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 7. URI schemes
    assert_eq!(
        extractor.sanitize_and_validate_path("file:///etc/passwd"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        extractor.sanitize_and_validate_path("http://evil.com/exploit"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
}

#[test]
fn test_absolute_paths_interception_and_sandboxing() {
    let tmp = tempdir().expect("Failed to create tempdir");

    // Extractor with SECURE_NOABSOLUTEPATHS (default)
    let extractor = SecurePathExtractor::new(tmp.path(), SecurityFlags::DEFAULT)
        .expect("Extractor should initialize");

    assert_eq!(
        extractor.sanitize_and_validate_path("/etc/hosts"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        extractor.sanitize_and_validate_path(r"\Windows\System32"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        extractor.sanitize_and_validate_path(r"C:\Windows\System32"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        extractor.sanitize_and_validate_path(r"\\remote_host\share\data"),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // Valid relative path passes
    let rel = extractor
        .sanitize_and_validate_path("subfolder/file.txt")
        .expect("Valid path should pass");
    assert_eq!(rel, Path::new("subfolder/file.txt"));
}

#[test]
fn test_symlink_escape_attack_interception() {
    let tmp_sandbox = tempdir().expect("Failed to create sandbox tempdir");
    let tmp_victim = tempdir().expect("Failed to create victim tempdir");

    let sandbox_path = tmp_sandbox.path();
    let victim_path = tmp_victim.path();

    let mut extractor = SecurePathExtractor::new(sandbox_path, SecurityFlags::DEFAULT)
        .expect("Extractor should initialize");

    // 1. Attempt to create a symlink pointing to an external directory (victim dir)
    let symlink_name = Path::new("link_dir");
    let victim_target_str = victim_path.to_str().expect("Path to str");

    // create_symlink_secure with absolute target must be rejected
    assert_eq!(
        extractor.create_symlink_secure(symlink_name, victim_target_str),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // Manually create symlink inside sandbox to simulate an archive containing an intermediate symlink
    let full_symlink = sandbox_path.join("external_link");
    std::os::unix::fs::symlink(victim_path, &full_symlink).expect("Create symlink");

    // 2. Now attempt to extract a file inside the symlink directory ("external_link/payload.txt")
    let payload_path = Path::new("external_link/payload.txt");
    let res = extractor.create_file_secure(payload_path, 0o644, 1700000000, 0, true);

    // Assert that intermediate path validation intercepts the symlink and rejects the write
    assert_eq!(res.map(|_| ()), Err(TTZipStatus::ErrSecurityViolation));

    // Ensure victim directory was NOT modified
    let victim_payload = victim_path.join("payload.txt");
    assert!(!victim_payload.exists());

    // 3. Attempt to create directory through intermediate symlink
    let payload_dir = Path::new("external_link/nested_dir");
    let dir_res = extractor.create_dir_all_secure(payload_dir, 0o755, 1700000000, 0);
    assert_eq!(dir_res, Err(TTZipStatus::ErrSecurityViolation));
}

#[test]
fn test_toctou_atomic_unlink_and_file_creation() {
    let tmp = tempdir().expect("Failed to create tempdir");
    let sandbox = tmp.path();

    let mut extractor = SecurePathExtractor::new(sandbox, SecurityFlags::DEFAULT)
        .expect("Extractor should initialize");

    let file_rel = Path::new("output/data.bin");

    // Create file
    let mut f = extractor
        .create_file_secure(file_rel, 0o644, 1700000000, 0, false)
        .expect("File creation should succeed");
    f.write_all(b"Initial content").expect("Write should succeed");
    drop(f);

    let full_path = sandbox.join(file_rel);
    assert!(full_path.exists());
    let content = fs::read_to_string(&full_path).expect("Read file");
    assert_eq!(content, "Initial content");

    // Overwrite file with SECURE_UNLINK_FIRST active
    let mut f2 = extractor
        .create_file_secure(file_rel, 0o600, 1700000001, 0, true)
        .expect("Overwrite should succeed");
    f2.write_all(b"Updated content").expect("Write should succeed");
    drop(f2);

    let updated = fs::read_to_string(&full_path).expect("Read file");
    assert_eq!(updated, "Updated content");
}

#[test]
fn test_two_stage_bottom_up_permission_and_timestamp_restoration() {
    let tmp = tempdir().expect("Failed to create tempdir");
    let sandbox = tmp.path();

    let mut extractor = SecurePathExtractor::new(sandbox, SecurityFlags::DEFAULT)
        .expect("Extractor should initialize");

    // Create multi-tier nested directory: parent/child/grandchild
    let parent_dir = Path::new("parent");
    let child_dir = Path::new("parent/child");
    let file_path = Path::new("parent/child/test.txt");

    // Set parent directory to restrictive read-only mode (0555) in archive metadata
    extractor
        .create_dir_all_secure(parent_dir, 0o555, 1700000000, 0)
        .expect("Parent dir creation should succeed");
    extractor
        .create_dir_all_secure(child_dir, 0o755, 1700000000, 0)
        .expect("Child dir creation should succeed");

    // Create child file with 0644 mode
    let mut f = extractor
        .create_file_secure(file_path, 0o644, 1700000000, 0, true)
        .expect("File creation should succeed");
    f.write_all(b"Nested secure file content")
        .expect("Write file");
    drop(f);

    // Before apply_deferred_metadata, stage 1 permissions are user-accessible (0700/0600)
    let parent_meta = fs::metadata(sandbox.join(parent_dir)).expect("Metadata");
    assert_eq!(parent_meta.permissions().mode() & 0o777, 0o700);

    let file_meta = fs::metadata(sandbox.join(file_path)).expect("Metadata");
    assert_eq!(file_meta.permissions().mode() & 0o777, 0o600);

    // Apply deferred metadata in bottom-up order
    extractor
        .apply_deferred_metadata()
        .expect("Metadata application should succeed");

    // Verify final restored permissions
    let final_file_meta = fs::metadata(sandbox.join(file_path)).expect("Metadata");
    assert_eq!(final_file_meta.permissions().mode() & 0o777, 0o644);

    let final_child_meta = fs::metadata(sandbox.join(child_dir)).expect("Metadata");
    assert_eq!(final_child_meta.permissions().mode() & 0o777, 0o755);

    let final_parent_meta = fs::metadata(sandbox.join(parent_dir)).expect("Metadata");
    assert_eq!(final_parent_meta.permissions().mode() & 0o777, 0o555);
}
