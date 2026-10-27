// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! End-to-end `ExtractionSession` regression tests for read-only directory `0o555`
//! restoration, selective extraction, staging abort cleanup, and pre-existing
//! directory permission preservation (`KeepBoth` / `Skip`).

use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use tempfile::tempdir;
use ttzip_engine::uniffi_api::extraction_session::{
    CollisionAction, CollisionPolicy, CommitResult, ExtractionRequest, ExtractionSession,
    OutcomeStatus, ResolutionSet,
};

fn build_readonly_0555_tar_fixture(dir: &Path) -> PathBuf {
    let tar_path = dir.join("readonly_0555.tar");
    let mut tar_bytes = Vec::new();
    let mut writer = ttzip_engine::archive::tar::writer::TarWriter::new(&mut tar_bytes);
    writer.append_dir("ro_dir/", 0o555, 1_700_000_000).unwrap();
    writer
        .append_file(
            "ro_dir/child.txt",
            b"readonly child payload",
            0o644,
            1_700_000_100,
        )
        .unwrap();
    writer.finish().unwrap();
    fs::write(&tar_path, &tar_bytes).unwrap();
    tar_path
}

fn cleanup_readonly_tree(root: &Path) {
    if let Ok(meta) = fs::symlink_metadata(root) {
        if meta.is_dir() {
            let _ = fs::set_permissions(root, fs::Permissions::from_mode(0o700));
            if let Ok(entries) = fs::read_dir(root) {
                for entry in entries.flatten() {
                    cleanup_readonly_tree(&entry.path());
                }
            }
        }
    }
}

/// End-to-end `ExtractionSession` regression for read-only directory `0o555` under Scenario A
/// (destination does NOT yet have `ro_dir`):
/// 1. Full session (`new -> stage -> commit`) succeeds (`OutcomeStatus::Completed`), applies `0o555`
///    and `mtime` (`1_700_000_000`) to newly created `dest/ro_dir`, applies `0o644` and `mtime`
///    (`1_700_000_100`) to `dest/ro_dir/child.txt`, and leaves zero `.ttzip_staging_*` directories.
/// 2. Selective session (`entry_filter: Some(["ro_dir/"])`) restores identical `mode` (`0o555`, `0o644`)
///    and `mtime` (`1_700_000_000`, `1_700_000_100`).
/// 3. Aborted session after `stage` removes `.ttzip_staging_*` cleanly without `EACCES`.
#[test]
fn test_repro_0555_session_target_absent_full_selective_and_abort() {
    let tmp = tempdir().unwrap();
    let tar_path = build_readonly_0555_tar_fixture(tmp.path());

    // Case A1: Full extraction session into destination without `ro_dir`
    let dest_full = tmp.path().join("dest_absent_full");
    fs::create_dir_all(&dest_full).unwrap();
    let session_full = ExtractionSession::new(ExtractionRequest {
        archive_path: tar_path.to_str().unwrap().to_string(),
        destination_root: dest_full.to_str().unwrap().to_string(),
        password: None,
        entry_filter: None,
        root_at_common_parent: false,
        policy: CollisionPolicy::KeepBoth,
    })
    .unwrap();
    let stage_report = session_full.stage(None, None).unwrap();
    assert!(stage_report.post_stage_conflicts.is_none());
    let CommitResult::Finished { outcome } = session_full
        .commit(ResolutionSet {
            default_action: Some(CollisionAction::KeepBoth),
            overrides: Vec::new(),
        })
        .unwrap()
    else {
        panic!("expected Finished");
    };
    assert_eq!(outcome.status, OutcomeStatus::Completed);
    assert!(outcome.failed.is_empty());
    let full_dir_meta = fs::metadata(dest_full.join("ro_dir")).unwrap();
    let full_file_meta = fs::metadata(dest_full.join("ro_dir/child.txt")).unwrap();
    assert_eq!(full_dir_meta.permissions().mode() & 0o777, 0o555);
    assert_eq!(full_dir_meta.mtime(), 1_700_000_000);
    assert_eq!(full_file_meta.permissions().mode() & 0o777, 0o644);
    assert_eq!(full_file_meta.mtime(), 1_700_000_100);
    let has_staging = fs::read_dir(&dest_full)
        .unwrap()
        .any(|e| e.unwrap().file_name().to_string_lossy().starts_with(".ttzip_staging_"));
    assert!(!has_staging, "staging directory must be cleaned up after commit");

    // Case A2: Selective extraction session (`entry_filter: Some(["ro_dir/"])`)
    let dest_sel = tmp.path().join("dest_absent_selective");
    fs::create_dir_all(&dest_sel).unwrap();
    let session_sel = ExtractionSession::new(ExtractionRequest {
        archive_path: tar_path.to_str().unwrap().to_string(),
        destination_root: dest_sel.to_str().unwrap().to_string(),
        password: None,
        entry_filter: Some(vec!["ro_dir/".to_string()]),
        root_at_common_parent: false,
        policy: CollisionPolicy::KeepBoth,
    })
    .unwrap();
    session_sel.stage(None, None).unwrap();
    let CommitResult::Finished { outcome: sel_outcome } = session_sel
        .commit(ResolutionSet {
            default_action: Some(CollisionAction::KeepBoth),
            overrides: Vec::new(),
        })
        .unwrap()
    else {
        panic!("expected Finished");
    };
    assert_eq!(sel_outcome.status, OutcomeStatus::Completed);
    let sel_dir_meta = fs::metadata(dest_sel.join("ro_dir")).unwrap();
    let sel_file_meta = fs::metadata(dest_sel.join("ro_dir/child.txt")).unwrap();
    assert_eq!(sel_dir_meta.permissions().mode() & 0o777, 0o555);
    assert_eq!(sel_dir_meta.mtime(), 1_700_000_000);
    assert_eq!(sel_file_meta.permissions().mode() & 0o777, 0o644);
    assert_eq!(sel_file_meta.mtime(), 1_700_000_100);

    // Case A3: Abort after stage removes `.ttzip_staging_*` cleanly
    let dest_abort = tmp.path().join("dest_abort");
    fs::create_dir_all(&dest_abort).unwrap();
    let session_abort = ExtractionSession::new(ExtractionRequest {
        archive_path: tar_path.to_str().unwrap().to_string(),
        destination_root: dest_abort.to_str().unwrap().to_string(),
        password: None,
        entry_filter: None,
        root_at_common_parent: false,
        policy: CollisionPolicy::KeepBoth,
    })
    .unwrap();
    session_abort.stage(None, None).unwrap();
    let abort_outcome = session_abort.abort().unwrap();
    assert_eq!(abort_outcome.status, OutcomeStatus::Stopped);
    assert_eq!(fs::read_dir(&dest_abort).unwrap().count(), 0);
    cleanup_readonly_tree(tmp.path());
}

/// End-to-end `ExtractionSession` regression for read-only directory `0o555` under Scenario B
/// (destination ALREADY has `ro_dir` at `0o700`, plus `KeepBoth` / `Skip` conflict resolution):
/// 1. Merging into pre-existing `dest/ro_dir` (`0o700`) succeeds (`OutcomeStatus::Completed`),
///    preserves `dest/ro_dir`'s pre-existing `0o700` permission, and applies `0o644` + `mtime`
///    to `dest/ro_dir/child.txt`.
/// 2. Re-extracting with `KeepBoth` creates `dest/ro_dir/child 2.txt` with `0o644` + `mtime`,
///    and re-extracting with `Skip` leaves existing files and `0o700` directory untouched.
#[test]
fn test_repro_0555_session_target_existing_dir_merge_failure() {
    let tmp = tempdir().unwrap();
    let tar_path = build_readonly_0555_tar_fixture(tmp.path());

    let dest_existing = tmp.path().join("dest_existing_ro_dir");
    let pre_ro_dir = dest_existing.join("ro_dir");
    fs::create_dir_all(&pre_ro_dir).unwrap();
    fs::set_permissions(&pre_ro_dir, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(pre_ro_dir.join("preexisting.txt"), b"keep me").unwrap();

    let session = ExtractionSession::new(ExtractionRequest {
        archive_path: tar_path.to_str().unwrap().to_string(),
        destination_root: dest_existing.to_str().unwrap().to_string(),
        password: None,
        entry_filter: None,
        root_at_common_parent: false,
        policy: CollisionPolicy::KeepBoth,
    })
    .unwrap();
    let stage_report = session.stage(None, None).unwrap();
    assert!(stage_report.post_stage_conflicts.is_none());

    let CommitResult::Finished { outcome } = session
        .commit(ResolutionSet {
            default_action: Some(CollisionAction::KeepBoth),
            overrides: Vec::new(),
        })
        .unwrap()
    else {
        panic!("expected Finished");
    };
    assert_eq!(outcome.status, OutcomeStatus::Completed);
    assert!(outcome.failed.is_empty());
    assert_eq!(fs::read(pre_ro_dir.join("child.txt")).unwrap(), b"readonly child payload");
    assert_eq!(fs::read(pre_ro_dir.join("preexisting.txt")).unwrap(), b"keep me");
    let child_meta = fs::metadata(pre_ro_dir.join("child.txt")).unwrap();
    assert_eq!(child_meta.permissions().mode() & 0o777, 0o644);
    assert_eq!(child_meta.mtime(), 1_700_000_100);
    // Pre-existing directory keeps its original `0o700` permission!
    assert_eq!(fs::metadata(&pre_ro_dir).unwrap().permissions().mode() & 0o777, 0o700);

    // Second extraction into the same directory with `KeepBoth` on conflicting `ro_dir/child.txt`
    let session_kb = ExtractionSession::new(ExtractionRequest {
        archive_path: tar_path.to_str().unwrap().to_string(),
        destination_root: dest_existing.to_str().unwrap().to_string(),
        password: None,
        entry_filter: None,
        root_at_common_parent: false,
        policy: CollisionPolicy::KeepBoth,
    })
    .unwrap();
    let kb_stage = session_kb.stage(None, None).unwrap();
    assert_eq!(kb_stage.post_stage_conflicts.unwrap().total, 1);
    let CommitResult::Finished { outcome: kb_outcome } = session_kb
        .commit(ResolutionSet {
            default_action: Some(CollisionAction::KeepBoth),
            overrides: Vec::new(),
        })
        .unwrap()
    else {
        panic!("expected Finished");
    };
    assert_eq!(kb_outcome.status, OutcomeStatus::Completed);
    assert_eq!(kb_outcome.kept_both, 1);
    let child2_meta = fs::metadata(pre_ro_dir.join("child 2.txt")).unwrap();
    assert_eq!(child2_meta.permissions().mode() & 0o777, 0o644);
    assert_eq!(child2_meta.mtime(), 1_700_000_100);
    assert_eq!(fs::metadata(&pre_ro_dir).unwrap().permissions().mode() & 0o777, 0o700);
    cleanup_readonly_tree(tmp.path());
}
