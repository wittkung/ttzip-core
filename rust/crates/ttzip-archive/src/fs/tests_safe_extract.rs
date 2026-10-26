// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Unit tests for `SafeExtractEngine`, `DirJail` path sanitization, and two-stage
//! directory permission policies (pre-existing vs newly created implicit vs explicit).

use super::safe_extract::*;
use crate::types::TTZipStatus;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use tempfile::tempdir;

#[test]
fn test_zipslip_traversal_rejection() {
    let tmp = tempdir().unwrap();
    let dest = tmp.path();

    assert_eq!(
        sanitize_and_validate_path(dest, "../../etc/passwd"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        sanitize_and_validate_path(dest, "/etc/shadow"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        sanitize_and_validate_path(dest, "a/../../etc/passwd"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        sanitize_and_validate_path(dest, "../foo"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        sanitize_and_validate_path(dest, "a/../../bar"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        sanitize_and_validate_path(dest, "foo/.."),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        sanitize_and_validate_path(dest, "foo/../bar"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        sanitize_and_validate_path(dest, ".."),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        sanitize_and_validate_path(dest, "."),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        sanitize_and_validate_path(dest, "..\\foo"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        sanitize_and_validate_path(dest, "foo\\..\\bar"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        sanitize_and_validate_path(dest, "C:\\Windows\\System32"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        sanitize_and_validate_path(dest, "\\\\evil_server\\share"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
    assert_eq!(
        sanitize_and_validate_path(dest, "valid_file.txt\0payload"),
        Err(TTZipStatus::ErrSecurityViolation)
    );
}

#[test]
fn test_valid_path_sanitization() {
    let tmp = tempdir().unwrap();
    let dest = tmp.path();
    let valid = sanitize_and_validate_path(dest, "folder/subfolder/file.txt").unwrap();
    assert_eq!(valid, dest.join("folder/subfolder/file.txt"));

    let dotted = sanitize_and_validate_path(dest, "./dir/./nested/./test.dat").unwrap();
    assert_eq!(dotted, dest.join("dir/nested/test.dat"));

    // Consecutive dots within legitimate file and directory names must be permitted
    let multi_dot = sanitize_and_validate_path(dest, "archive..v1.0.tar").unwrap();
    assert_eq!(multi_dot, dest.join("archive..v1.0.tar"));

    let multi_dot_nested = sanitize_and_validate_path(dest, "release..dir/nested..file.ext").unwrap();
    assert_eq!(multi_dot_nested, dest.join("release..dir/nested..file.ext"));

    let complex_dots = sanitize_and_validate_path(dest, "pkg/version..1.0..final.tar.gz").unwrap();
    assert_eq!(complex_dots, dest.join("pkg/version..1.0..final.tar.gz"));
}

#[test]
fn test_two_stage_extraction_and_bottom_up_metadata() {
    let tmp_dir = tempdir().unwrap();
    let temp_dir = tmp_dir.path();

    let mut engine = SafeExtractEngine::new();

    let sub_dir = temp_dir.join("level1/level2");
    engine.create_dir_all_secure(&sub_dir, 0o755, 1700000000).unwrap();

    let file_path = sub_dir.join("sample.txt");
    let mut f = engine.create_file_secure(&file_path, 0o644, 1700000000, true).unwrap();
    f.write_all(b"Safe extraction content").unwrap();
    drop(f);

    // Verify stage 1 temporary permissions
    let file_meta = fs::metadata(&file_path).unwrap();
    assert_eq!(file_meta.permissions().mode() & 0o777, 0o600);

    // Stage 2: Apply deferred metadata bottom-up
    engine.apply_deferred_metadata(true).unwrap();

    // Verify stage 2 final permissions applied
    let final_file_meta = fs::metadata(&file_path).unwrap();
    assert_eq!(final_file_meta.permissions().mode() & 0o777, 0o644);

    let final_dir_meta = fs::metadata(&sub_dir).unwrap();
    assert_eq!(final_dir_meta.permissions().mode() & 0o777, 0o755);
}

#[test]
fn test_open_extracted_file_in_dir_nofollow_intermediate_symlink() {
    let tmp = tempdir().unwrap();
    let dest = tmp.path();
    let outside = tempdir().unwrap();

    // Create an intermediate symlink directory pointing outside dest
    let symlink_dir = dest.join("symdir");
    std::os::unix::fs::symlink(outside.path(), &symlink_dir).unwrap();

    let target_file = symlink_dir.join("evil.txt");
    let res = open_extracted_file_in_dir_nofollow(dest, &target_file);
    assert_eq!(res.err(), Some(TTZipStatus::ErrSecurityViolation));
}

#[test]
fn test_preexisting_0700_and_new_implicit_and_explicit_dir_coexistence() {
    let tmp_dir = tempdir().unwrap();
    let root = tmp_dir.path();

    // 1. Pre-existing private directory (`0o700`) already present before extraction begins
    let preexisting_priv = root.join("existing_private");
    fs::create_dir(&preexisting_priv).unwrap();
    fs::set_permissions(&preexisting_priv, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(fs::metadata(&preexisting_priv).unwrap().permissions().mode() & 0o777, 0o700);

    let jail = DirJail::open(root).unwrap();
    let mut engine = SafeExtractEngine::for_root(root);

    // 2. Extract file inside pre-existing 0o700 parent + newly created implicit child directory
    let nested_file_path = root.join("existing_private/new_implicit/nested.bin");
    let mut nf = jail
        .open_file_in_jail("existing_private/new_implicit/nested.bin", 0o600, true)
        .unwrap();
    nf.write_all(b"nested").unwrap();
    drop(nf);
    engine.register_entry(nested_file_path.clone(), 0o644, 1700000000, 0, false);

    // 3. Explicit archive directories with header modes 0o700 and 0o750
    let explicit_strict_path = root.join("explicit_strict");
    jail.create_dir_all_in_jail("explicit_strict").unwrap();
    engine.register_entry(explicit_strict_path.clone(), 0o700, 1700000000, 0, true);

    let explicit_group_path = root.join("explicit_group");
    jail.create_dir_all_in_jail("explicit_group").unwrap();
    engine.register_entry(explicit_group_path.clone(), 0o750, 1700000000, 0, true);

    // Stage 1: Newly created implicit and explicit directories all have temporary 0o700 permissions
    let new_implicit_dir = root.join("existing_private/new_implicit");
    assert_eq!(fs::metadata(&preexisting_priv).unwrap().permissions().mode() & 0o777, 0o700);
    assert_eq!(fs::metadata(&new_implicit_dir).unwrap().permissions().mode() & 0o777, 0o700);
    assert_eq!(fs::metadata(&explicit_strict_path).unwrap().permissions().mode() & 0o777, 0o700);
    assert_eq!(fs::metadata(&explicit_group_path).unwrap().permissions().mode() & 0o777, 0o700);

    // Stage 2: Apply deferred metadata bottom-up
    engine.apply_all().unwrap();

    // Verify coexistence contract:
    // - Pre-existing directory keeps its original 0o700 permission (not loosened to 0o755)
    assert_eq!(fs::metadata(&preexisting_priv).unwrap().permissions().mode() & 0o777, 0o700);
    // - Newly created implicit parent directory transitions from 0o700 to default 0o755
    assert_eq!(fs::metadata(&new_implicit_dir).unwrap().permissions().mode() & 0o777, 0o755);
    // - Explicit archive directories restore their exact archive modes (0o700 and 0o750)
    assert_eq!(fs::metadata(&explicit_strict_path).unwrap().permissions().mode() & 0o777, 0o700);
    assert_eq!(fs::metadata(&explicit_group_path).unwrap().permissions().mode() & 0o750, 0o750);
    assert_eq!(fs::metadata(&nested_file_path).unwrap().permissions().mode() & 0o777, 0o644);
}

#[test]
fn test_dir_jail_metadata_symlink_nofollow_and_parent_leaf_replacement() {
    use std::os::unix::fs::MetadataExt;

    let tmp = tempdir().unwrap();
    let root = tmp.path().join("root");
    fs::create_dir_all(&root).unwrap();
    let jail = DirJail::open(&root).unwrap();

    // 1. Symlink timestamp uses explicit no-follow semantics: updates symlink's own mtime without
    //    modifying target file's mode or mtime.
    let mut target_f = jail.open_file_in_jail("target.txt", 0o600, true).unwrap();
    target_f.write_all(b"target").unwrap();
    drop(target_f);
    jail.create_symlink_in_jail("link.txt", "target.txt").unwrap();

    let mut engine = SafeExtractEngine::for_root(&root);
    engine.register_entry_typed(root.join("target.txt"), 0o600, 1_650_000_000, 0, false, false);
    engine.register_entry_typed(root.join("link.txt"), 0o777, 1_700_000_555, 0, false, true);
    engine.apply_all().unwrap();

    let target_meta = fs::metadata(root.join("target.txt")).unwrap();
    let link_meta = fs::symlink_metadata(root.join("link.txt")).unwrap();
    assert_eq!(target_meta.permissions().mode() & 0o777, 0o600);
    assert_eq!(target_meta.mtime(), 1_650_000_000);
    assert_eq!(link_meta.mtime(), 1_700_000_555);

    // 2. Parent directory replaced with symlink to outside directory before metadata application
    let outside = tmp.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    let outside_file = outside.join("victim.txt");
    fs::write(&outside_file, b"safe").unwrap();
    fs::set_permissions(&outside_file, fs::Permissions::from_mode(0o600)).unwrap();
    let orig_outside_mtime = fs::metadata(&outside_file).unwrap().mtime();

    let mut engine2 = SafeExtractEngine::for_root(&root);
    std::os::unix::fs::symlink(&outside, root.join("sub_escape")).unwrap();
    engine2.register_entry(root.join("sub_escape/victim.txt"), 0o755, 1_700_000_999, 0, false);
    assert_eq!(engine2.apply_all(), Err(TTZipStatus::ErrSecurityViolation));
    assert_eq!(fs::metadata(&outside_file).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(fs::metadata(&outside_file).unwrap().mtime(), orig_outside_mtime);
}

#[test]
fn test_transactional_staged_identity_capture_failure_rejects_and_never_modifies_replacement_object() {
    use std::os::unix::fs::MetadataExt;
    use std::path::PathBuf;
    use crate::fs::collision::{
        CommitPathMapping, CommittedEntryDisposition, CommittedPathEntry, ExtractionOutcome,
        FailureReason, ItemKind, OutcomeStatus,
    };
    use crate::fs::staged_metadata::{
        apply_committed_metadata, store_staged_metadata_plan, StagedMetadataPlan,
    };

    let tmp = tempdir().unwrap();
    let payload = tmp.path().join(".ttzip_staging_ident_test").join("payload");
    fs::create_dir_all(&payload).unwrap();
    let dest = tmp.path().join("dest");
    fs::create_dir_all(dest.join("sub")).unwrap();
    let outside = tmp.path().join("outside");
    fs::create_dir_all(&outside).unwrap();

    let outside_victim = outside.join("victim.txt");
    fs::write(&outside_victim, b"untouched").unwrap();
    fs::set_permissions(&outside_victim, fs::Permissions::from_mode(0o600)).unwrap();
    let orig_mtime = fs::metadata(&outside_victim).unwrap().mtime();
    fs::hard_link(&outside_victim, dest.join("sub/file.txt")).unwrap();

    // 1. Stage 1 identity capture failure on transactional staging payload must fail explicitly
    //    instead of storing a plan with expected_identity = None.
    let mut engine = SafeExtractEngine::for_root(&payload);
    engine.register_entry(payload.join("sub/file.txt"), 0o755, 1_700_000_888, 0, false);
    assert_eq!(
        engine.apply_deferred_metadata(true),
        Err(TTZipStatus::ErrFileNotFound),
        "Missing staged entry must fail identity capture explicitly during Stage 1"
    );

    let mut mapping = CommitPathMapping::new();
    mapping.push(CommittedPathEntry {
        staged_rel_path: PathBuf::from("sub/file.txt"),
        committed_rel_path: PathBuf::from("sub/file.txt"),
        kind: ItemKind::File,
        disposition: CommittedEntryDisposition::Extracted,
        whole_subtree: false,
    });
    let mut outcome = ExtractionOutcome {
        status: OutcomeStatus::Completed,
        extracted: 1,
        ..Default::default()
    };
    apply_committed_metadata(&payload, &dest, &mapping, &mut outcome);
    assert_eq!(fs::metadata(&outside_victim).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(fs::metadata(&outside_victim).unwrap().mtime(), orig_mtime);

    // 2. Defense-in-depth at Stage 3: if a StagedMetadataPlan entry has expected_identity = None,
    //    apply_committed_metadata must reject it and never modify the replacement object.
    store_staged_metadata_plan(StagedMetadataPlan {
        staging_payload_root: payload.clone(),
        preserve_permissions: true,
        creation_mode_mask: 0o777,
        files: vec![DeferredEntryMetadata {
            path: payload.join("sub/file.txt"),
            mode: 0o755,
            mtime_epoch_secs: 1_700_000_888,
            mtime_nanos: 0,
            is_directory: false,
            is_symlink: false,
            expected_identity: None,
        }],
        explicit_dirs: Vec::new(),
        implicit_dirs: Vec::new(),
    });
    apply_committed_metadata(&payload, &dest, &mapping, &mut outcome);
    assert_eq!(outcome.status, OutcomeStatus::CompletedWithFailures);
    assert_eq!(outcome.failed.len(), 1);
    assert_eq!(outcome.failed[0].relative_path, "sub/file.txt");
    assert_eq!(outcome.failed[0].reason, FailureReason::PermissionDenied);
    assert_eq!(outcome.failed[0].errno, libc::EPERM);
    assert_eq!(fs::metadata(&outside_victim).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(fs::metadata(&outside_victim).unwrap().mtime(), orig_mtime);
}

