// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Integration tests of the UniFFI `ExtractionSession` and the one-shot policy
//! path (feature 229): partial and single-entry extraction, collision policies,
//! cancellation, and staging hygiene. Archives are generated at test time.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ttzip_engine::uniffi_api::callback::{UniFFICancellationToken, UniFFIProgressCallback};
use ttzip_engine::uniffi_api::extraction::extract_archive_stream;
use ttzip_engine::uniffi_api::extraction_session::{
    sweep_stale_staging, CollisionAction, CollisionPolicy, CommitResult, ExtractionOutcome,
    ExtractionRequest, ExtractionSession, OutcomeStatus, ResolutionOverride, ResolutionSet,
    SessionState,
};
use ttzip_engine::uniffi_api::types::TTZipError;

fn write_zip(dir: &Path, files: &[(&str, &[u8])]) -> PathBuf {
    let items = files
        .iter()
        .map(|(name, data)| ttzip_engine::zip::ZipInputItem {
            rel_path: name.to_string(),
            data: data.to_vec(),
            mtime_epoch_secs: 1_700_000_000,
            mode: 0o644,
            is_directory: false,
        })
        .collect();
    let compressed = ttzip_engine::zip::compress_items_parallel(
        items,
        6,
        ttzip_engine::types::TTZipEncryptionMethod::None,
        None,
        2,
    )
    .unwrap();
    let path = dir.join("fixture.zip");
    fs::write(&path, ttzip_engine::zip::assemble_zip_archive(&compressed).unwrap()).unwrap();
    path
}

fn request(
    archive: &Path,
    dest: &Path,
    filter: Option<&[&str]>,
    root_at_common_parent: bool,
    policy: CollisionPolicy,
) -> ExtractionRequest {
    ExtractionRequest {
        archive_path: archive.to_str().unwrap().to_string(),
        destination_root: dest.to_str().unwrap().to_string(),
        password: None,
        entry_filter: filter.map(|f| f.iter().map(|s| s.to_string()).collect()),
        root_at_common_parent,
        policy,
    }
}

fn resolve_all(action: Option<CollisionAction>) -> ResolutionSet {
    ResolutionSet { default_action: action, overrides: Vec::new() }
}

fn finished(result: CommitResult) -> ExtractionOutcome {
    match result {
        CommitResult::Finished { outcome } => outcome,
        CommitResult::NeedsDecision { summary } => panic!("unexpected NeedsDecision: {summary:?}"),
    }
}

/// Names of every entry in `dest`, sorted; staging leftovers would show up here.
fn listing(dest: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dest)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn read(path: PathBuf) -> String {
    fs::read_to_string(path).unwrap()
}

#[test]
fn test_uniffi_extraction_session_lifecycle() {
    let root = tempfile::tempdir().unwrap();
    let archive_path = root.path().join("dummy.zip");
    fs::write(&archive_path, b"dummy").unwrap();
    let dest_root = root.path().join("dest");
    fs::create_dir_all(&dest_root).unwrap();

    let session =
        ExtractionSession::new(request(&archive_path, &dest_root, None, false, CollisionPolicy::KeepBoth))
            .expect("Failed to create session");
    assert_eq!(session.state(), SessionState::Created);

    let outcome = session.abort().expect("Abort failed");
    assert_eq!(session.state(), SessionState::Aborted);
    assert_eq!(outcome.extracted, 0);
    assert_eq!(session.abort().expect("abort is idempotent").status, OutcomeStatus::Stopped);

    let swept = sweep_stale_staging(dest_root.to_str().unwrap().to_string()).unwrap();
    assert_eq!(swept, 0);
}



#[test]
fn single_entry_keep_both_preserves_existing_and_asks_once() {
    let root = tempfile::tempdir().unwrap();
    let archive = write_zip(root.path(), &[("docs/report.txt", b"incoming"), ("docs/other.txt", b"x")]);
    let dest = root.path().join("dest");
    fs::create_dir(&dest).unwrap();
    fs::write(dest.join("report.txt"), b"edited").unwrap();

    let filter: &[&str] = &["docs/report.txt"];
    let session =
        ExtractionSession::new(request(&archive, &dest, Some(filter), true, CollisionPolicy::KeepBoth)).unwrap();
    let summary = session.preflight().unwrap().expect("preflight must see the conflict");
    assert_eq!(summary.total, 1);
    let page = session.conflict_page(0, 10).unwrap();
    assert_eq!(page.records[0].relative_path, "report.txt");

    let report = session.stage(None, None).unwrap();
    assert_eq!(report.post_stage_conflicts, None, "a preflight decision must not be requested again");
    assert_eq!(report.staged_items, 1);
    assert_eq!(report.staged_bytes, 8);

    let outcome = finished(session.commit(resolve_all(Some(CollisionAction::KeepBoth))).unwrap());
    assert_eq!(outcome.status, OutcomeStatus::Completed);
    assert_eq!(outcome.kept_both, 1);
    assert_eq!(read(dest.join("report.txt")), "edited");
    assert_eq!(read(dest.join("report 2.txt")), "incoming");
    assert_eq!(listing(&dest), ["report 2.txt", "report.txt"], "no staging directory may remain");
}

#[test]
fn stop_policy_with_partial_conflicts_writes_nothing() {
    let root = tempfile::tempdir().unwrap();
    let names = ["set/a.txt", "set/b.txt", "set/c.txt", "set/d.txt", "set/e.txt"];
    let files: Vec<(&str, &[u8])> = names.iter().map(|n| (*n, b"new".as_slice())).collect();
    let archive = write_zip(root.path(), &files);
    let dest = root.path().join("dest");
    fs::create_dir(&dest).unwrap();
    fs::write(dest.join("b.txt"), b"old-b").unwrap();
    fs::write(dest.join("d.txt"), b"old-d").unwrap();

    let session =
        ExtractionSession::new(request(&archive, &dest, Some(&names), true, CollisionPolicy::Stop)).unwrap();
    assert_eq!(session.preflight().unwrap().unwrap().total, 2);
    assert_eq!(session.abort().unwrap().status, OutcomeStatus::Stopped);

    assert_eq!(listing(&dest), ["b.txt", "d.txt"]);
    assert_eq!(read(dest.join("b.txt")), "old-b");
    assert_eq!(read(dest.join("d.txt")), "old-d");
}

#[test]
fn directory_selection_extracts_descendants_at_common_parent() {
    let root = tempfile::tempdir().unwrap();
    let archive = write_zip(
        root.path(),
        &[("docs/img/a.png", b"a"), ("docs/img/sub/b.png", b"b"), ("docs/readme.md", b"r")],
    );
    let dest = root.path().join("dest");
    fs::create_dir(&dest).unwrap();

    let filter: &[&str] = &["docs/img/"];
    let session =
        ExtractionSession::new(request(&archive, &dest, Some(filter), true, CollisionPolicy::KeepBoth)).unwrap();
    assert!(session.preflight().unwrap().is_none());
    session.stage(None, None).unwrap();
    let outcome = finished(session.commit(resolve_all(Some(CollisionAction::KeepBoth))).unwrap());

    assert_eq!(outcome.status, OutcomeStatus::Completed);
    assert_eq!(read(dest.join("img/a.png")), "a");
    assert_eq!(read(dest.join("img/sub/b.png")), "b");
    assert_eq!(listing(&dest), ["img"]);
}

#[test]
fn selection_without_root_at_common_parent_keeps_archive_paths() {
    let root = tempfile::tempdir().unwrap();
    let archive = write_zip(root.path(), &[("docs/readme.md", b"r"), ("other.txt", b"o")]);
    let dest = root.path().join("dest");
    fs::create_dir(&dest).unwrap();

    let filter: &[&str] = &["./docs/readme.md"];
    let session =
        ExtractionSession::new(request(&archive, &dest, Some(filter), false, CollisionPolicy::KeepBoth)).unwrap();
    session.preflight().unwrap();
    session.stage(None, None).unwrap();
    finished(session.commit(resolve_all(Some(CollisionAction::KeepBoth))).unwrap());

    assert_eq!(read(dest.join("docs/readme.md")), "r");
    assert_eq!(listing(&dest), ["docs"]);
}

#[test]
fn empty_entry_filter_is_rejected() {
    let root = tempfile::tempdir().unwrap();
    let archive = write_zip(root.path(), &[("a.txt", b"a")]);
    for filter in [&[][..], &["/", "./"][..]] {
        let result = ExtractionSession::new(request(&archive, root.path(), Some(filter), false, CollisionPolicy::KeepBoth));
        assert!(matches!(result, Err(TTZipError::InvalidArgument { .. })), "filter {filter:?}");
    }
}

#[test]
fn stage_without_preflight_reports_post_stage_conflicts() {
    let root = tempfile::tempdir().unwrap();
    let archive = write_zip(root.path(), &[("a.txt", b"new"), ("b.txt", b"b")]);
    let dest = root.path().join("dest");
    fs::create_dir(&dest).unwrap();
    fs::write(dest.join("a.txt"), b"old").unwrap();

    let session = ExtractionSession::new(request(&archive, &dest, None, false, CollisionPolicy::KeepBoth)).unwrap();
    let report = session.stage(None, None).unwrap();
    assert_eq!(report.post_stage_conflicts.map(|s| s.total), Some(1));
    assert_eq!(session.state(), SessionState::AwaitingDecision);

    let outcome = finished(session.commit(resolve_all(Some(CollisionAction::Skip))).unwrap());
    assert_eq!((outcome.extracted, outcome.skipped), (1, 1));
    assert_eq!(read(dest.join("a.txt")), "old");
    assert_eq!(listing(&dest), ["a.txt", "b.txt"]);
}

#[test]
fn invalid_resolution_keeps_session_committable() {
    let root = tempfile::tempdir().unwrap();
    let archive = write_zip(root.path(), &[("a.txt", b"new")]);
    let dest = root.path().join("dest");
    fs::create_dir(&dest).unwrap();
    fs::write(dest.join("a.txt"), b"old").unwrap();

    let session = ExtractionSession::new(request(&archive, &dest, None, false, CollisionPolicy::KeepBoth)).unwrap();
    session.preflight().unwrap().unwrap();
    session.stage(None, None).unwrap();
    let out_of_range = ResolutionSet {
        default_action: None,
        overrides: vec![ResolutionOverride { conflict_id: 7, action: CollisionAction::Replace }],
    };
    assert!(matches!(session.commit(out_of_range), Err(TTZipError::InvalidResolution { .. })));
    assert_eq!(session.state(), SessionState::Staged);

    let outcome = finished(session.commit(resolve_all(Some(CollisionAction::Replace))).unwrap());
    assert_eq!(outcome.replaced, 1);
    assert_eq!(read(dest.join("a.txt")), "new");
    assert_eq!(listing(&dest), ["a.txt"]);
}

#[test]
fn cancelled_token_aborts_stage_and_cleans_staging() {
    let root = tempfile::tempdir().unwrap();
    let archive = write_zip(root.path(), &[("a.txt", b"a")]);
    let dest = root.path().join("dest");
    fs::create_dir(&dest).unwrap();

    let session = ExtractionSession::new(request(&archive, &dest, None, false, CollisionPolicy::KeepBoth)).unwrap();
    let token = UniFFICancellationToken::new();
    token.cancel();
    assert!(matches!(session.stage(None, Some(token)), Err(TTZipError::Cancelled)));
    assert_eq!(session.state(), SessionState::Aborted);
    assert!(listing(&dest).is_empty());
}

struct RejectingProgress;

impl UniFFIProgressCallback for RejectingProgress {
    fn on_progress(&self, _processed: u64, _total: u64, _entry: Option<String>) -> bool {
        false
    }
}

#[test]
fn progress_callback_returning_false_cancels_stage() {
    let root = tempfile::tempdir().unwrap();
    let archive = write_zip(root.path(), &[("dir/a.txt", b"aaaa"), ("dir/b.txt", b"bbbb")]);
    let dest = root.path().join("dest");
    fs::create_dir(&dest).unwrap();

    for filter in [None, Some(&["dir/"][..])] {
        let session = ExtractionSession::new(request(&archive, &dest, filter, false, CollisionPolicy::KeepBoth)).unwrap();
        let result = session.stage(Some(Box::new(RejectingProgress)), None);
        assert!(matches!(result, Err(TTZipError::Cancelled)), "filter {filter:?}: {result:?}");
        assert_eq!(session.state(), SessionState::Aborted);
        assert!(listing(&dest).is_empty());
    }
}

#[test]
fn abort_after_stage_removes_destination_created_by_session() {
    let root = tempfile::tempdir().unwrap();
    let archive = write_zip(root.path(), &[("a.txt", b"a")]);
    let dest = root.path().join("fresh");

    let session = ExtractionSession::new(request(&archive, &dest, None, false, CollisionPolicy::KeepBoth)).unwrap();
    assert!(!dest.exists(), "construction must not touch the destination");
    assert!(session.preflight().unwrap().is_none());
    session.stage(None, None).unwrap();
    assert!(dest.exists());
    session.abort().unwrap();
    assert!(!dest.exists());
}

#[test]
fn one_shot_stream_applies_policy_and_rejects_ask() {
    let root = tempfile::tempdir().unwrap();
    let archive = write_zip(root.path(), &[("a.txt", b"new"), ("b.txt", b"b")]);
    let dest = root.path().join("dest");
    fs::create_dir(&dest).unwrap();
    fs::write(dest.join("a.txt"), b"old").unwrap();
    let call = |policy| {
        extract_archive_stream(
            archive.to_str().unwrap().to_string(),
            dest.to_str().unwrap().to_string(),
            policy,
            None,
            None,
            None,
        )
    };

    assert!(matches!(call(CollisionPolicy::Ask), Err(TTZipError::InvalidArgument { .. })));

    let stopped = call(CollisionPolicy::Stop).unwrap().outcome.unwrap();
    assert_eq!(stopped.status, OutcomeStatus::Stopped);
    assert_eq!(listing(&dest), ["a.txt"]);

    let report = call(CollisionPolicy::KeepBoth).unwrap();
    let outcome = report.outcome.unwrap();
    assert_eq!((outcome.extracted, outcome.kept_both), (1, 1));
    assert_eq!(report.uncompressed_bytes, 4);
    assert_eq!(read(dest.join("a.txt")), "old");
    assert_eq!(read(dest.join("a 2.txt")), "new");
    assert_eq!(listing(&dest), ["a 2.txt", "a.txt", "b.txt"]);
}

#[test]
fn concurrent_sessions_share_nothing() {
    let root = tempfile::tempdir().unwrap();
    let archive = write_zip(root.path(), &[("a.txt", b"a")]);
    let archive = Arc::new(archive);
    let handles: Vec<_> = (0..4)
        .map(|i| {
            let archive = Arc::clone(&archive);
            let dest = root.path().join(format!("dest{i}"));
            std::thread::spawn(move || {
                let session =
                    ExtractionSession::new(request(&archive, &dest, None, false, CollisionPolicy::KeepBoth)).unwrap();
                session.preflight().unwrap();
                session.stage(None, None).unwrap();
                finished(session.commit(resolve_all(Some(CollisionAction::KeepBoth))).unwrap());
                listing(&dest)
            })
        })
        .collect();
    for handle in handles {
        assert_eq!(handle.join().unwrap(), ["a.txt"]);
    }
}
