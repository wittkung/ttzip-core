// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Group G Q1 Follow-Up Regression Suite:
//! - Q1-ISSUE-04: `CollisionPolicy::Stop` consistency on direct `ExtractionSession` API calls,
//!   resolution precedence (`overrides` > `default_action` > `request.policy`), `Ask` incomplete
//!   resolution rejection, conflict-free `Stop`, type mismatches, and commit-time race handling.
//! - Q1-ISSUE-08 (TAR portion): `0444` read-only files, `0755` executables, `0555` restricted
//!   directories with nested children, security bit filtering (`S_ISUID | S_ISGID | S_IWOTH`),
//!   `preserve_permissions = true` vs `false`, `KeepBoth` / `Skip` / `MergedExistingDir` metadata
//!   targeting, and Phase 3 metadata failure propagation (`CompletedWithFailures`).

use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use tempfile::TempDir;
use ttzip_engine::archive::unified::UnifiedArchiveOrchestrator;
use ttzip_engine::fs::collision::fs_ops::LibcFsOps;
use ttzip_engine::fs::collision::session::ExtractionSession as CoreExtractionSession;
use ttzip_engine::fs::collision::{
    CollisionPolicy as CorePolicy, OutcomeStatus as CoreOutcomeStatus,
    ResolutionSet as CoreResolutionSet,
};
use ttzip_engine::fs::staged_metadata::register_pre_metadata_hook_for_dest;
use ttzip_engine::types::{TTZipExtractOptions, TTZIP_COLLISION_POLICY_KEEP_BOTH};
use ttzip_engine::uniffi_api::extraction_session::{
    CollisionAction, CollisionPolicy, CommitResult, ExtractionOutcome, ExtractionRequest,
    ExtractionSession, OutcomeStatus, ResolutionOverride, ResolutionSet, SessionState,
};
use ttzip_engine::uniffi_api::types::TTZipError;

struct TarFixtureEntry<'a> {
    path: &'a str,
    data: &'a [u8],
    mode: u32,
    mtime: u64,
    entry_type: tar::EntryType,
    link_target: Option<&'a str>,
}

impl<'a> TarFixtureEntry<'a> {
    const fn file(path: &'a str, data: &'a [u8], mode: u32, mtime: u64) -> Self {
        Self {
            path,
            data,
            mode,
            mtime,
            entry_type: tar::EntryType::Regular,
            link_target: None,
        }
    }

    const fn dir(path: &'a str, mode: u32, mtime: u64) -> Self {
        Self {
            path,
            data: b"",
            mode,
            mtime,
            entry_type: tar::EntryType::Directory,
            link_target: None,
        }
    }

    const fn symlink(path: &'a str, target: &'a str, mtime: u64) -> Self {
        Self {
            path,
            data: b"",
            mode: 0o777,
            mtime,
            entry_type: tar::EntryType::Symlink,
            link_target: Some(target),
        }
    }
}

fn write_tar_fixture(path: &Path, entries: &[TarFixtureEntry<'_>]) {
    let file = fs::File::create(path).expect("create tar fixture");
    let mut builder = tar::Builder::new(file);
    for item in entries {
        let mut header = tar::Header::new_ustar();
        header.set_entry_type(item.entry_type);
        header.set_mode(item.mode);
        header.set_mtime(item.mtime);
        if let Some(target) = item.link_target {
            header.set_size(0);
            header.set_cksum();
            builder
                .append_link(&mut header, item.path, target)
                .expect("append tar symlink");
        } else if item.entry_type == tar::EntryType::Directory {
            header.set_size(0);
            header.set_cksum();
            builder
                .append_data(&mut header, item.path, std::io::empty())
                .expect("append tar dir");
        } else {
            header.set_size(item.data.len() as u64);
            header.set_cksum();
            builder
                .append_data(&mut header, item.path, item.data)
                .expect("append tar file");
        }
    }
    builder.finish().expect("finish tar fixture");
}

fn make_request(
    archive: &Path,
    dest: &Path,
    filter: Option<Vec<String>>,
    policy: CollisionPolicy,
) -> ExtractionRequest {
    ExtractionRequest {
        archive_path: archive.to_str().unwrap().to_string(),
        destination_root: dest.to_str().unwrap().to_string(),
        password: None,
        entry_filter: filter,
        root_at_common_parent: false,
        policy,
    }
}

fn unwrap_finished(res: CommitResult) -> ExtractionOutcome {
    match res {
        CommitResult::Finished { outcome } => outcome,
        CommitResult::NeedsDecision { summary } => {
            panic!("unexpected NeedsDecision: {summary:?}")
        }
    }
}

fn set_file_mtime_and_mode(path: &Path, mode: u32, mtime_sec: i64) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("set_permissions");
    let c_path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let times = [
        libc::timespec { tv_sec: mtime_sec, tv_nsec: 0 },
        libc::timespec { tv_sec: mtime_sec, tv_nsec: 0 },
    ];
    let rc = unsafe { libc::utimensat(libc::AT_FDCWD, c_path.as_ptr(), times.as_ptr(), 0) };
    assert_eq!(rc, 0, "utimensat failed on {}", path.display());
}

fn sorted_top_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .expect("read_dir")
        .map(|e| e.expect("dir entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn assert_no_staging_dirs(dir: &Path) {
    for name in sorted_top_names(dir) {
        assert!(
            !name.starts_with(".ttzip_staging_"),
            "unexpected leftover staging directory: {name}"
        );
    }
}

/// Criterion 1: Direct `ExtractionSession` API with `CollisionPolicy::Stop` and no explicit
/// covering resolution stops before moving any non-conflicting files (both with and without
/// prior `preflight()`), preserving destination file set, content hashes, inodes, modes, and mtimes.
#[test]
fn test_direct_session_stop_policy_with_unhandled_conflicts_stops_cleanly() {
    for run_preflight in [true, false] {
        let tmp = TempDir::new().unwrap();
        let archive = tmp.path().join("mixed_conflicts.tar");
        write_tar_fixture(
            &archive,
            &[
                TarFixtureEntry::file("conflict_a.txt", b"NEW_A_PAYLOAD", 0o644, 1_700_000_100),
                TarFixtureEntry::file("fresh_b.txt", b"NEW_B_PAYLOAD", 0o755, 1_700_000_200),
                TarFixtureEntry::file("sub/conflict_c.txt", b"NEW_C_PAYLOAD", 0o444, 1_700_000_300),
                TarFixtureEntry::file("sub/fresh_d.txt", b"NEW_D_PAYLOAD", 0o644, 1_700_000_400),
            ],
        );

        let dest = tmp.path().join("dest");
        fs::create_dir_all(dest.join("sub")).unwrap();
        let conflict_a = dest.join("conflict_a.txt");
        let conflict_c = dest.join("sub/conflict_c.txt");
        let neighbor = dest.join("neighbor_keep.txt");
        fs::write(&conflict_a, b"ORIG_A_BYTES").unwrap();
        fs::write(&conflict_c, b"ORIG_C_BYTES").unwrap();
        fs::write(&neighbor, b"NEIGHBOR_BYTES").unwrap();
        set_file_mtime_and_mode(&conflict_a, 0o640, 1_650_000_001);
        set_file_mtime_and_mode(&conflict_c, 0o600, 1_650_000_002);
        set_file_mtime_and_mode(&neighbor, 0o644, 1_650_000_003);
        let ino_a_before = fs::metadata(&conflict_a).unwrap().ino();
        let ino_c_before = fs::metadata(&conflict_c).unwrap().ino();
        let ino_n_before = fs::metadata(&neighbor).unwrap().ino();

        let session =
            ExtractionSession::new(make_request(&archive, &dest, None, CollisionPolicy::Stop))
                .expect("create session");
        if run_preflight {
            let pre = session.preflight().expect("preflight").expect("must detect 2 conflicts");
            assert_eq!(pre.total, 2);
        }
        let stage_rep = session.stage(None, None).expect("stage");
        if !run_preflight {
            assert_eq!(stage_rep.post_stage_conflicts.map(|s| s.total), Some(2));
        }

        let outcome = unwrap_finished(
            session
                .commit(ResolutionSet { default_action: None, overrides: vec![] })
                .expect("commit under Stop must return Stopped outcome"),
        );
        assert_eq!(outcome.status, OutcomeStatus::Stopped, "run_preflight={run_preflight}");
        assert_eq!(
            (outcome.extracted, outcome.replaced, outcome.kept_both, outcome.skipped),
            (0, 0, 0, 0)
        );
        assert!(outcome.failed.is_empty());
        assert_eq!(session.state(), SessionState::Aborted);
        assert_no_staging_dirs(&dest);

        assert!(!dest.join("fresh_b.txt").exists(), "non-conflicting file must not be placed");
        assert!(!dest.join("sub/fresh_d.txt").exists(), "nested non-conflicting file must not be placed");
        assert_eq!(fs::read(&conflict_a).unwrap(), b"ORIG_A_BYTES");
        assert_eq!(fs::read(&conflict_c).unwrap(), b"ORIG_C_BYTES");
        assert_eq!(fs::read(&neighbor).unwrap(), b"NEIGHBOR_BYTES");

        let meta_a = fs::metadata(&conflict_a).unwrap();
        let meta_c = fs::metadata(&conflict_c).unwrap();
        let meta_n = fs::metadata(&neighbor).unwrap();
        assert_eq!((meta_a.ino(), meta_a.permissions().mode() & 0o777, meta_a.mtime()), (ino_a_before, 0o640, 1_650_000_001));
        assert_eq!((meta_c.ino(), meta_c.permissions().mode() & 0o777, meta_c.mtime()), (ino_c_before, 0o600, 1_650_000_002));
        assert_eq!((meta_n.ino(), meta_n.permissions().mode() & 0o777, meta_n.mtime()), (ino_n_before, 0o644, 1_650_000_003));
    }
}

/// Criterion 1b: Partial per-item overrides under `CollisionPolicy::Stop` (1 of 2 conflicts covered,
/// `default_action: None`) must still stop before committing, and core `commit_with_policy` enforces
/// the same contract.
#[test]
fn test_stop_policy_with_partial_override_and_core_session_stops_without_mutation() {
    let tmp = TempDir::new().unwrap();
    let archive = tmp.path().join("two_conflicts.tar");
    write_tar_fixture(
        &archive,
        &[
            TarFixtureEntry::file("a.txt", b"new_a", 0o644, 1_700_000_000),
            TarFixtureEntry::file("b.txt", b"new_b", 0o644, 1_700_000_000),
            TarFixtureEntry::file("c.txt", b"new_c", 0o644, 1_700_000_000),
        ],
    );
    let dest = tmp.path().join("dest");
    fs::create_dir(&dest).unwrap();
    fs::write(dest.join("a.txt"), b"old_a").unwrap();
    fs::write(dest.join("b.txt"), b"old_b").unwrap();

    // 1. Only conflict 0 is overridden; conflict 1 remains uncovered under Stop -> Stopped.
    let session =
        ExtractionSession::new(make_request(&archive, &dest, None, CollisionPolicy::Stop)).unwrap();
    assert_eq!(session.preflight().unwrap().unwrap().total, 2);
    session.stage(None, None).unwrap();
    let partial_res = ResolutionSet {
        default_action: None,
        overrides: vec![ResolutionOverride { conflict_id: 0, action: CollisionAction::Replace }],
    };
    let outcome = unwrap_finished(session.commit(partial_res).unwrap());
    assert_eq!(outcome.status, OutcomeStatus::Stopped);
    assert_eq!(sorted_top_names(&dest), vec!["a.txt", "b.txt"]);
    assert_eq!(fs::read(dest.join("a.txt")).unwrap(), b"old_a");
    assert_eq!(fs::read(dest.join("b.txt")).unwrap(), b"old_b");

    // 2. Core `ttzip_archive::fs::collision::session::ExtractionSession::commit_with_policy`.
    let core_staged = CoreExtractionSession::new(&dest).stage().unwrap();
    fs::write(core_staged.staging().payload_dir().0.join("a.txt"), b"new_a").unwrap();
    fs::write(core_staged.staging().payload_dir().0.join("c.txt"), b"new_c").unwrap();
    let core_outcome = core_staged
        .commit_with_policy(&LibcFsOps, CorePolicy::Stop, &CoreResolutionSet::new())
        .unwrap();
    assert_eq!(core_outcome.status, CoreOutcomeStatus::Stopped);
    assert_eq!(sorted_top_names(&dest), vec!["a.txt", "b.txt"]);
}

/// Criterion 2: Precedence of per-item `overrides`, `default_action`, and `request.policy`
/// (`Replace`, `Skip`, `KeepBoth`, `Ask`, and explicit decisions overriding `Stop`).
#[test]
fn test_resolution_precedence_and_interactive_ask_validation() {
    let tmp = TempDir::new().unwrap();
    let archive = tmp.path().join("precedence.tar");
    write_tar_fixture(
        &archive,
        &[
            TarFixtureEntry::file("a.txt", b"archive_a", 0o644, 1_700_000_000),
            TarFixtureEntry::file("b.txt", b"archive_b", 0o644, 1_700_000_000),
            TarFixtureEntry::file("c.txt", b"archive_c", 0o644, 1_700_000_000),
            TarFixtureEntry::file("d.txt", b"archive_d", 0o644, 1_700_000_000),
        ],
    );

    // 1. `Ask` policy with uncovered conflicts returns `InvalidResolution` and keeps session Staged;
    //    then supplying per-item overrides + default_action commits with exact precedence.
    let dest1 = tmp.path().join("dest_ask");
    fs::create_dir(&dest1).unwrap();
    fs::write(dest1.join("a.txt"), b"orig_a").unwrap();
    fs::write(dest1.join("b.txt"), b"orig_b").unwrap();
    fs::write(dest1.join("c.txt"), b"orig_c").unwrap();

    let sess_ask =
        ExtractionSession::new(make_request(&archive, &dest1, None, CollisionPolicy::Ask)).unwrap();
    assert_eq!(sess_ask.preflight().unwrap().unwrap().total, 3);
    sess_ask.stage(None, None).unwrap();

    let incomplete = ResolutionSet {
        default_action: None,
        overrides: vec![ResolutionOverride { conflict_id: 0, action: CollisionAction::Replace }],
    };
    assert!(matches!(
        sess_ask.commit(incomplete),
        Err(TTZipError::InvalidResolution { .. })
    ));
    assert_eq!(sess_ask.state(), SessionState::Staged);
    assert!(!dest1.join("d.txt").exists(), "rejected commit must not place files");

    // Now supply overrides for 0 (Replace) and 1 (Skip) + default_action KeepBoth (covers 2):
    let complete = ResolutionSet {
        default_action: Some(CollisionAction::KeepBoth),
        overrides: vec![
            ResolutionOverride { conflict_id: 0, action: CollisionAction::Replace },
            ResolutionOverride { conflict_id: 1, action: CollisionAction::Skip },
        ],
    };
    let out1 = unwrap_finished(sess_ask.commit(complete).unwrap());
    assert_eq!(out1.status, OutcomeStatus::Completed);
    assert_eq!((out1.extracted, out1.replaced, out1.skipped, out1.kept_both), (1, 1, 1, 1));
    assert_eq!(fs::read(dest1.join("a.txt")).unwrap(), b"archive_a");
    assert_eq!(fs::read(dest1.join("b.txt")).unwrap(), b"orig_b");
    assert_eq!(fs::read(dest1.join("c.txt")).unwrap(), b"orig_c");
    assert_eq!(fs::read(dest1.join("c 2.txt")).unwrap(), b"archive_c");
    assert_eq!(fs::read(dest1.join("d.txt")).unwrap(), b"archive_d");

    // 2. Policy fallback when `default_action: None` (request.policy = Replace, override 0 = Skip),
    //    and explicit overrides covering all conflicts under `CollisionPolicy::Stop`.
    let dest2 = tmp.path().join("dest_policy_fallback");
    fs::create_dir(&dest2).unwrap();
    fs::write(dest2.join("a.txt"), b"orig_a").unwrap();
    fs::write(dest2.join("b.txt"), b"orig_b").unwrap();

    let sess_rep =
        ExtractionSession::new(make_request(&archive, &dest2, None, CollisionPolicy::Replace))
            .unwrap();
    assert_eq!(sess_rep.preflight().unwrap().unwrap().total, 2);
    sess_rep.stage(None, None).unwrap();
    let out2 = unwrap_finished(
        sess_rep
            .commit(ResolutionSet {
                default_action: None,
                overrides: vec![ResolutionOverride { conflict_id: 0, action: CollisionAction::Skip }],
            })
            .unwrap(),
    );
    assert_eq!(out2.status, OutcomeStatus::Completed);
    assert_eq!((out2.extracted, out2.replaced, out2.skipped), (2, 1, 1));
    assert_eq!(fs::read(dest2.join("a.txt")).unwrap(), b"orig_a");
    assert_eq!(fs::read(dest2.join("b.txt")).unwrap(), b"archive_b");
}

/// Criterion 3a: Conflict-free `Stop` commits normally; type mismatch conflicts (file vs dir,
/// dir vs file) stop under `Stop` and rename cleanly under `KeepBoth`.
#[test]
fn test_conflict_free_stop_and_type_mismatch() {
    let tmp = TempDir::new().unwrap();
    let archive = tmp.path().join("items.tar");
    write_tar_fixture(
        &archive,
        &[
            TarFixtureEntry::file("alpha.txt", b"alpha_new", 0o644, 1_700_000_000),
            TarFixtureEntry::dir("dir_entry", 0o755, 1_700_000_000),
            TarFixtureEntry::file("dir_entry/nested.txt", b"nested_new", 0o644, 1_700_000_000),
        ],
    );

    let dest_clean = tmp.path().join("dest_clean");
    fs::create_dir(&dest_clean).unwrap();
    let sess_clean =
        ExtractionSession::new(make_request(&archive, &dest_clean, None, CollisionPolicy::Stop)).unwrap();
    assert!(sess_clean.preflight().unwrap().is_none());
    sess_clean.stage(None, None).unwrap();
    let out_clean = unwrap_finished(
        sess_clean.commit(ResolutionSet { default_action: None, overrides: vec![] }).unwrap(),
    );
    assert_eq!(out_clean.status, OutcomeStatus::Completed);
    assert_eq!(fs::read(dest_clean.join("alpha.txt")).unwrap(), b"alpha_new");
    assert_eq!(fs::read(dest_clean.join("dir_entry/nested.txt")).unwrap(), b"nested_new");

    let dest_mismatch = tmp.path().join("dest_mismatch");
    fs::create_dir(&dest_mismatch).unwrap();
    fs::write(dest_mismatch.join("dir_entry"), b"existing_file_at_dir_name").unwrap();

    let sess_mm_stop =
        ExtractionSession::new(make_request(&archive, &dest_mismatch, None, CollisionPolicy::Stop)).unwrap();
    let summary = sess_mm_stop.preflight().unwrap().unwrap();
    assert_eq!((summary.total, summary.type_mismatch), (1, 1));
    sess_mm_stop.stage(None, None).unwrap();
    let out_mm_stop = unwrap_finished(
        sess_mm_stop.commit(ResolutionSet { default_action: None, overrides: vec![] }).unwrap(),
    );
    assert_eq!(out_mm_stop.status, OutcomeStatus::Stopped);
    assert_eq!(sorted_top_names(&dest_mismatch), vec!["dir_entry"]);

    let sess_mm_kb =
        ExtractionSession::new(make_request(&archive, &dest_mismatch, None, CollisionPolicy::KeepBoth)).unwrap();
    sess_mm_kb.preflight().unwrap();
    sess_mm_kb.stage(None, None).unwrap();
    let out_mm_kb = unwrap_finished(
        sess_mm_kb.commit(ResolutionSet { default_action: None, overrides: vec![] }).unwrap(),
    );
    assert_eq!((out_mm_kb.status, out_mm_kb.kept_both), (OutcomeStatus::Completed, 1));
    assert_eq!(fs::read(dest_mismatch.join("dir_entry")).unwrap(), b"existing_file_at_dir_name");
    assert_eq!(fs::read(dest_mismatch.join("dir_entry 2/nested.txt")).unwrap(), b"nested_new");
}

/// Criterion 3b: Destination changes between `preflight`/`stage` and `commit` (`CommitRecheck` race)
/// never silently overwrite the newly appeared target.
#[test]
fn test_pre_commit_destination_change_race_does_not_overwrite() {
    let tmp = TempDir::new().unwrap();
    let archive = tmp.path().join("race.tar");
    write_tar_fixture(
        &archive,
        &[
            TarFixtureEntry::file("alpha.txt", b"alpha_new", 0o644, 1_700_000_000),
            TarFixtureEntry::dir("dir_entry", 0o755, 1_700_000_000),
            TarFixtureEntry::file("dir_entry/nested.txt", b"nested_new", 0o644, 1_700_000_000),
        ],
    );
    let dest_race = tmp.path().join("dest_race");
    fs::create_dir(&dest_race).unwrap();
    let sess_race =
        ExtractionSession::new(make_request(&archive, &dest_race, None, CollisionPolicy::Stop)).unwrap();
    assert!(sess_race.preflight().unwrap().is_none());
    sess_race.stage(None, None).unwrap();
    fs::write(dest_race.join("alpha.txt"), b"RACER_FILE_CONTENT").unwrap();

    let out_race = unwrap_finished(
        sess_race.commit(ResolutionSet { default_action: None, overrides: vec![] }).unwrap(),
    );
    assert_eq!((out_race.status, out_race.failed.len()), (OutcomeStatus::CompletedWithFailures, 1));
    assert_eq!(out_race.failed[0].relative_path, "alpha.txt");
    assert_eq!(out_race.failed[0].errno, libc::EEXIST);
    assert_eq!(fs::read(dest_race.join("alpha.txt")).unwrap(), b"RACER_FILE_CONTENT");
    assert_eq!(fs::read(dest_race.join("dir_entry/nested.txt")).unwrap(), b"nested_new");
}

fn write_posix_matrix_tar(tar_path: &Path) {
    write_tar_fixture(
        tar_path,
        &[
            TarFixtureEntry::dir("ro_dir", 0o555, 1_690_000_000),
            TarFixtureEntry::file("ro_dir/immutable.cfg", b"READONLY_444\n", 0o444, 1_680_000_000),
            TarFixtureEntry::file("ro_dir/implicit_sub/tool.sh", b"#!/bin/sh\necho ok\n", 0o755, 1_700_001_234),
            TarFixtureEntry::file("setuid_world.bin", b"sec_bits", 0o6777, 1_695_000_000),
            TarFixtureEntry::symlink("ro_dir/tool_link", "implicit_sub/tool.sh", 1_695_000_100),
        ],
    );
}

/// Criterion 4: TAR with `0444` read-only files, `0755` executables, `0555` restricted directory +
/// nested content, and security bit filtering (`0o6777` -> `0o775`) when `preserve_permissions = true`.
#[test]
fn test_tar_preserve_permissions_true_restores_0444_0755_0555_and_strips_unsafe_bits() {
    let tmp = TempDir::new().unwrap();
    let tar_path = tmp.path().join("posix_matrix.tar");
    write_posix_matrix_tar(&tar_path);

    let dest_true = tmp.path().join("dest_preserve_true");
    fs::create_dir(&dest_true).unwrap();
    let sess =
        ExtractionSession::new(make_request(&tar_path, &dest_true, None, CollisionPolicy::KeepBoth)).unwrap();
    assert!(sess.preflight().unwrap().is_none());
    sess.stage(None, None).unwrap();
    let out_true = unwrap_finished(
        sess.commit(ResolutionSet { default_action: None, overrides: vec![] }).unwrap(),
    );
    assert_eq!(out_true.status, OutcomeStatus::Completed);

    let cfg_meta = fs::metadata(dest_true.join("ro_dir/immutable.cfg")).unwrap();
    let tool_meta = fs::metadata(dest_true.join("ro_dir/implicit_sub/tool.sh")).unwrap();
    let sub_meta = fs::metadata(dest_true.join("ro_dir/implicit_sub")).unwrap();
    let ro_dir_meta = fs::metadata(dest_true.join("ro_dir")).unwrap();
    let sec_meta = fs::metadata(dest_true.join("setuid_world.bin")).unwrap();
    let sym_meta = fs::symlink_metadata(dest_true.join("ro_dir/tool_link")).unwrap();

    assert_eq!((cfg_meta.permissions().mode() & 0o7777, cfg_meta.mtime()), (0o444, 1_680_000_000));
    assert_eq!((tool_meta.permissions().mode() & 0o7777, tool_meta.mtime()), (0o755, 1_700_001_234));
    assert_eq!(sub_meta.permissions().mode() & 0o7777, 0o755);
    assert_eq!((ro_dir_meta.permissions().mode() & 0o7777, ro_dir_meta.mtime()), (0o555, 1_690_000_000));
    assert_eq!(sec_meta.permissions().mode() & 0o7777, 0o775);
    assert!(sym_meta.file_type().is_symlink());
    fs::set_permissions(dest_true.join("ro_dir"), fs::Permissions::from_mode(0o755)).unwrap();
}

/// Criterion 5a: When `preserve_permissions = false`, archive modes (`0444`, `0755`, `0555`, `06777`)
/// are ignored and normalized to `(0o644 & !umask)` / `(0o755 & !umask)`.
#[test]
fn test_tar_preserve_permissions_false_normalizes_modes() {
    let tmp = TempDir::new().unwrap();
    let tar_path = tmp.path().join("posix_matrix.tar");
    write_posix_matrix_tar(&tar_path);

    let dest_false = tmp.path().join("dest_preserve_false");
    fs::create_dir_all(dest_false.join("ro_dir")).unwrap();
    set_file_mtime_and_mode(&dest_false.join("ro_dir"), 0o700, 1_600_000_000);
    let env_mask = ttzip_engine::fs::staged_metadata::probe_creation_mode_mask_for_path(tmp.path());
    let exp_file_mode = (0o644 & env_mask) & !0o002;
    let exp_dir_mode = (0o755 & env_mask) & !0o002;

    let staged = CoreExtractionSession::new(&dest_false).stage().unwrap();
    let opts_false = TTZipExtractOptions {
        preserve_permissions: false,
        collision_policy: TTZIP_COLLISION_POLICY_KEEP_BOTH,
        ..TTZipExtractOptions::default()
    };
    UnifiedArchiveOrchestrator::extract_archive_with_metrics(&tar_path, &staged.staging().payload_dir(), &opts_false)
        .unwrap();
    let out_false = staged
        .commit_with_policy(&LibcFsOps, CorePolicy::KeepBoth, &CoreResolutionSet::new())
        .unwrap();
    assert_eq!(out_false.status, CoreOutcomeStatus::Completed);

    let cfg_false = fs::metadata(dest_false.join("ro_dir/immutable.cfg")).unwrap();
    let tool_false = fs::metadata(dest_false.join("ro_dir/implicit_sub/tool.sh")).unwrap();
    let sub_false = fs::metadata(dest_false.join("ro_dir/implicit_sub")).unwrap();
    let ro_dir_false = fs::metadata(dest_false.join("ro_dir")).unwrap();
    let sec_false = fs::metadata(dest_false.join("setuid_world.bin")).unwrap();

    assert_eq!((cfg_false.permissions().mode() & 0o7777, cfg_false.mtime()), (exp_file_mode, 1_680_000_000));
    assert_eq!(tool_false.permissions().mode() & 0o7777, exp_file_mode);
    assert_eq!(sub_false.permissions().mode() & 0o7777, exp_dir_mode);
    assert_eq!(ro_dir_false.permissions().mode() & 0o7777, 0o700);
    assert_eq!(sec_false.permissions().mode() & 0o7777, exp_file_mode);
}

/// Criterion 6: `KeepBoth` applies metadata to the renamed target (`<stem> 2.<ext>`), `Skip`
/// leaves the original file and its metadata untouched, and `MergedExistingDir` preserves existing
/// directory metadata.
#[test]
fn test_keep_both_skip_and_merged_dir_metadata_targeting() {
    let tmp = TempDir::new().unwrap();
    let tar_path = tmp.path().join("collision_meta.tar");
    write_tar_fixture(
        &tar_path,
        &[
            TarFixtureEntry::dir("merged_priv", 0o755, 1_710_000_000),
            TarFixtureEntry::file("merged_priv/readonly.cfg", b"NEW_RO_DATA", 0o444, 1_680_000_000),
            TarFixtureEntry::file("skip_target.sh", b"NEW_SKIP_DATA", 0o755, 1_700_000_000),
        ],
    );

    let dest = tmp.path().join("dest");
    fs::create_dir_all(dest.join("merged_priv")).unwrap();
    let orig_ro = dest.join("merged_priv/readonly.cfg");
    let orig_skip = dest.join("skip_target.sh");
    fs::write(&orig_ro, b"OLD_RO_DATA").unwrap();
    fs::write(&orig_skip, b"OLD_SKIP_DATA").unwrap();
    set_file_mtime_and_mode(&orig_ro, 0o600, 1_600_000_000);
    set_file_mtime_and_mode(&orig_skip, 0o640, 1_610_000_000);
    set_file_mtime_and_mode(&dest.join("merged_priv"), 0o700, 1_630_000_000);

    let sess = ExtractionSession::new(make_request(&tar_path, &dest, None, CollisionPolicy::Ask)).unwrap();
    assert_eq!(sess.preflight().unwrap().unwrap().total, 2);
    sess.stage(None, None).unwrap();

    let outcome = unwrap_finished(
        sess.commit(ResolutionSet {
            default_action: None,
            overrides: vec![
                ResolutionOverride { conflict_id: 0, action: CollisionAction::KeepBoth },
                ResolutionOverride { conflict_id: 1, action: CollisionAction::Skip },
            ],
        })
        .unwrap(),
    );
    assert_eq!((outcome.status, outcome.kept_both, outcome.skipped), (OutcomeStatus::Completed, 1, 1));

    let meta_orig_ro = fs::metadata(&orig_ro).unwrap();
    assert_eq!(fs::read(&orig_ro).unwrap(), b"OLD_RO_DATA");
    assert_eq!((meta_orig_ro.permissions().mode() & 0o777, meta_orig_ro.mtime()), (0o600, 1_600_000_000));

    let meta_renamed_ro = fs::metadata(dest.join("merged_priv/readonly 2.cfg")).unwrap();
    assert_eq!(fs::read(dest.join("merged_priv/readonly 2.cfg")).unwrap(), b"NEW_RO_DATA");
    assert_eq!((meta_renamed_ro.permissions().mode() & 0o777, meta_renamed_ro.mtime()), (0o444, 1_680_000_000));

    let meta_skip = fs::metadata(&orig_skip).unwrap();
    assert_eq!(fs::read(&orig_skip).unwrap(), b"OLD_SKIP_DATA");
    assert_eq!((meta_skip.permissions().mode() & 0o777, meta_skip.mtime()), (0o640, 1_610_000_000));
    assert_eq!(fs::metadata(dest.join("merged_priv")).unwrap().permissions().mode() & 0o777, 0o700);
}

/// Criterion 7: Phase 3 metadata failure injection returns `CompletedWithFailures` while preserving
/// committed contents, cleaning staging, and leaving neighbor files untouched.
#[test]
fn test_phase3_metadata_failure_injection_preserves_content_and_neighbors() {
    let tmp = TempDir::new().unwrap();
    let tar_path = tmp.path().join("collision_meta.tar");
    write_tar_fixture(
        &tar_path,
        &[
            TarFixtureEntry::dir("merged_priv", 0o755, 1_710_000_000),
            TarFixtureEntry::file("merged_priv/readonly.cfg", b"NEW_RO_DATA", 0o444, 1_680_000_000),
            TarFixtureEntry::file("skip_target.sh", b"NEW_SKIP_DATA", 0o755, 1_700_000_000),
        ],
    );

    let dest_fail = tmp.path().join("dest_meta_fail");
    fs::create_dir(&dest_fail).unwrap();
    let fail_neighbor = dest_fail.join("neighbor.txt");
    fs::write(&fail_neighbor, b"NEIGHBOR_INTACT").unwrap();
    set_file_mtime_and_mode(&fail_neighbor, 0o640, 1_625_000_000);

    register_pre_metadata_hook_for_dest(&dest_fail, Box::new(|root: &Path| {
        let target = root.join("skip_target.sh");
        let preserved = root.join("skip_target.sh.preserved");
        fs::rename(&target, &preserved).unwrap();
        fs::write(&target, b"IMPOSTER_INODE").unwrap();
        set_file_mtime_and_mode(&target, 0o600, 1_600_000_111);
    }));

    let sess_fail = ExtractionSession::new(make_request(&tar_path, &dest_fail, None, CollisionPolicy::KeepBoth)).unwrap();
    sess_fail.preflight().unwrap();
    sess_fail.stage(None, None).unwrap();
    let out_fail = unwrap_finished(
        sess_fail.commit(ResolutionSet { default_action: None, overrides: vec![] }).unwrap(),
    );
    assert_eq!((out_fail.status, out_fail.failed.len()), (OutcomeStatus::CompletedWithFailures, 1));
    assert_eq!(out_fail.failed[0].relative_path, "skip_target.sh");
    assert_no_staging_dirs(&dest_fail);
    assert_eq!(fs::read(dest_fail.join("merged_priv/readonly.cfg")).unwrap(), b"NEW_RO_DATA");
    assert_eq!(fs::metadata(dest_fail.join("merged_priv/readonly.cfg")).unwrap().permissions().mode() & 0o777, 0o444);
    assert_eq!(fs::read(dest_fail.join("skip_target.sh.preserved")).unwrap(), b"NEW_SKIP_DATA");
    assert_eq!(fs::metadata(dest_fail.join("skip_target.sh")).unwrap().permissions().mode() & 0o777, 0o600);
    let meta_fn = fs::metadata(&fail_neighbor).unwrap();
    assert_eq!((meta_fn.permissions().mode() & 0o777, meta_fn.mtime()), (0o640, 1_625_000_000));
}

fn run_strict_umask_preserve_false_child(exp_file_mode: u32, exp_dir_mode: u32) {
    use ttzip_engine::fs::collision::{CollisionAction as CoreAction, ResolutionOverride as CoreOverride};
    let tmp = TempDir::new().unwrap();
    let tar_path = tmp.path().join("strict_umask.tar");
    write_tar_fixture(
        &tar_path,
        &[
            TarFixtureEntry::dir("ro_dir", 0o555, 1_690_000_000),
            TarFixtureEntry::file("ro_dir/immutable.cfg", b"RO_DATA", 0o444, 1_680_000_000),
            TarFixtureEntry::file("ro_dir/implicit_sub/tool.sh", b"#!/bin/sh\n", 0o755, 1_700_001_234),
            TarFixtureEntry::dir("merged_priv", 0o755, 1_710_000_000),
            TarFixtureEntry::file("merged_priv/kb_item.cfg", b"NEW_KB", 0o755, 1_715_000_000),
            TarFixtureEntry::file("merged_priv/fresh.txt", b"FRESH", 0o644, 1_716_000_000),
            TarFixtureEntry::file("skip_item.sh", b"NEW_SKIP", 0o755, 1_720_000_000),
        ],
    );
    let dest = tmp.path().join("dest");
    fs::create_dir_all(dest.join("merged_priv")).unwrap();
    fs::write(dest.join("merged_priv/kb_item.cfg"), b"OLD_KB").unwrap();
    fs::write(dest.join("skip_item.sh"), b"OLD_SKIP").unwrap();
    set_file_mtime_and_mode(&dest.join("merged_priv/kb_item.cfg"), 0o640, 1_600_000_000);
    set_file_mtime_and_mode(&dest.join("skip_item.sh"), 0o644, 1_610_000_000);
    set_file_mtime_and_mode(&dest.join("merged_priv"), 0o750, 1_630_000_000);

    let mut staged = CoreExtractionSession::new(&dest).stage().unwrap();
    let opts = TTZipExtractOptions {
        preserve_permissions: false,
        collision_policy: TTZIP_COLLISION_POLICY_KEEP_BOTH,
        ..TTZipExtractOptions::default()
    };
    UnifiedArchiveOrchestrator::extract_archive_with_metrics(&tar_path, &staged.staging().payload_dir(), &opts).unwrap();
    let table = staged.post_stage_detect(&LibcFsOps).unwrap();
    let overrides = (0..table.len() as u32)
        .map(|id| {
            let rel = table.get(id).unwrap().relative_path;
            CoreOverride {
                conflict_id: id,
                action: if rel.ends_with("kb_item.cfg") { CoreAction::KeepBoth } else { CoreAction::Skip },
            }
        })
        .collect();
    let res = CoreResolutionSet { default_action: None, overrides };
    let out = staged.commit_with_policy(&LibcFsOps, CorePolicy::Ask, &res).unwrap();
    assert_eq!((out.status, out.kept_both, out.skipped), (CoreOutcomeStatus::Completed, 1, 1));

    assert_eq!(fs::metadata(dest.join("ro_dir")).unwrap().permissions().mode() & 0o7777, exp_dir_mode);
    assert_eq!(fs::metadata(dest.join("ro_dir/implicit_sub")).unwrap().permissions().mode() & 0o7777, exp_dir_mode);
    assert_eq!(fs::metadata(dest.join("ro_dir/immutable.cfg")).unwrap().permissions().mode() & 0o7777, exp_file_mode);
    assert_eq!(fs::metadata(dest.join("ro_dir/implicit_sub/tool.sh")).unwrap().permissions().mode() & 0o7777, exp_file_mode);
    assert_eq!(fs::metadata(dest.join("merged_priv/fresh.txt")).unwrap().permissions().mode() & 0o7777, exp_file_mode);
    assert_eq!(fs::metadata(dest.join("merged_priv/kb_item 2.cfg")).unwrap().permissions().mode() & 0o7777, exp_file_mode);
    assert_eq!(fs::metadata(dest.join("merged_priv/kb_item 2.cfg")).unwrap().mtime(), 1_715_000_000);
    assert_eq!(fs::metadata(dest.join("merged_priv/kb_item.cfg")).unwrap().permissions().mode() & 0o7777, 0o640);
    assert_eq!(fs::metadata(dest.join("skip_item.sh")).unwrap().permissions().mode() & 0o7777, 0o644);
    assert_eq!(fs::metadata(dest.join("merged_priv")).unwrap().permissions().mode() & 0o7777, 0o750);
}

const STRICT_UMASK_CHILD_ENV: &str = "TTZIP_STRICT_UMASK_CHILD";
const STRICT_UMASK_CASE_ENV: &str = "TTZIP_STRICT_UMASK_CASE";

/// Criterion 5b: Under strict caller `umask` (`0o077` and `0o027`), `preserve_permissions = false`
/// never widens permissions beyond `!umask` across new files/dirs, `KeepBoth`, `Skip`, and
/// `MergedExistingDir`, verified in isolated child processes without toggling global `umask` in
/// the multi-threaded test runner.
#[test]
fn test_preserve_permissions_false_respects_strict_umask_without_global_race() {
    use std::os::unix::process::CommandExt;
    if std::env::var(STRICT_UMASK_CHILD_ENV).as_deref() == Ok("1") {
        match std::env::var(STRICT_UMASK_CASE_ENV).as_deref() {
            Ok("0077") => {
                run_strict_umask_preserve_false_child(0o600, 0o700);
                std::process::exit(0);
            }
            Ok("0027") => {
                run_strict_umask_preserve_false_child(0o640, 0o750);
                std::process::exit(0);
            }
            other => {
                eprintln!("Unexpected strict umask child case: {other:?}");
                std::process::exit(2);
            }
        }
    }

    let exe = std::env::current_exe().expect("current test binary");
    for (case_tag, mask_val) in [("0077", 0o077 as libc::mode_t), ("0027", 0o027 as libc::mode_t)] {
        let mut cmd = std::process::Command::new(&exe);
        cmd.arg("--exact")
            .arg("test_preserve_permissions_false_respects_strict_umask_without_global_race")
            .arg("--test-threads=1")
            .stdin(std::process::Stdio::null())
            .env(STRICT_UMASK_CHILD_ENV, "1")
            .env(STRICT_UMASK_CASE_ENV, case_tag);
        unsafe {
            cmd.pre_exec(move || {
                libc::umask(mask_val);
                Ok(())
            });
        }
        let output = cmd.output().expect("spawn isolated umask child test");
        assert!(
            output.status.success(),
            "isolated umask={case_tag} child failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

