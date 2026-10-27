// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! FR-022 regression tests: free-name placement must use only provably atomic primitives
//! (`RENAME_EXCL` or `linkat` for regular files), never overwrite an existing file or empty
//! directory, fail with `FailureReason::NotSupportedByVolume` when no atomic primitive is
//! available, preserve both destination and staged source on failure, and leave zero reservation
//! artifacts behind.

use std::cell::{Cell, RefCell};
use std::fs;
use std::os::unix::fs as unix_fs;
use std::os::unix::io::{AsRawFd, OwnedFd, RawFd};
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::DFSCommitWalker;
use crate::fs::collision::fs_ops::{FsOps, LibcFsOps, VolCaps, RENAME_EXCL, RENAME_SWAP};
use crate::fs::collision::session::ExtractionSession;
use crate::fs::collision::table::ConflictTable;
use crate::fs::collision::{
    CollisionAction, ExtractionOutcome, FailureReason, ItemMeta, OutcomeStatus, ResolutionSet,
};

const NO_EXCL: VolCaps = VolCaps { rename_swap: true, rename_excl: false };
const WITH_EXCL: VolCaps = VolCaps { rename_swap: true, rename_excl: true };

#[derive(Clone)]
enum RaceKind {
    File(&'static [u8]),
    DirectoryWithFile(&'static str, &'static [u8]),
}

struct Race {
    dest: PathBuf,
    name: PathBuf,
    kind: RaceKind,
}

/// Real syscalls with switchable volume limitations and a one-shot pre-claim race injector.
#[derive(Default)]
struct ScriptedFs {
    rename_excl_runtime_enotsup: bool,
    link_err: Option<i32>,
    probe_fails: bool,
    plain_rename_calls: Cell<u32>,
    race: RefCell<Option<Race>>,
    fired: Cell<bool>,
}

impl ScriptedFs {
    fn with_file_race(dest: &Path, name: &str, content: &'static [u8]) -> Self {
        let race = Race {
            dest: dest.to_path_buf(),
            name: PathBuf::from(name),
            kind: RaceKind::File(content),
        };
        Self { race: RefCell::new(Some(race)), ..Self::default() }
    }

    fn with_dir_race(dest: &Path, name: &str, child: &'static str, content: &'static [u8]) -> Self {
        let race = Race {
            dest: dest.to_path_buf(),
            name: PathBuf::from(name),
            kind: RaceKind::DirectoryWithFile(child, content),
        };
        Self { race: RefCell::new(Some(race)), ..Self::default() }
    }

    fn fire_before_claim(&self, target: &Path) {
        let mut slot = self.race.borrow_mut();
        if !matches!(slot.as_ref(), Some(r) if r.name == target) {
            return;
        }
        let race = slot.take().expect("race armed");
        let path = race.dest.join(&race.name);
        match race.kind {
            RaceKind::File(content) => fs::write(&path, content).expect("write racer file"),
            RaceKind::DirectoryWithFile(child, content) => {
                fs::create_dir(&path).expect("create racer dir");
                fs::write(path.join(child), content).expect("write racer dir child");
            }
        }
        self.fired.set(true);
    }
}

impl FsOps for ScriptedFs {
    fn openat_dir_nofollow(&self, dirfd: RawFd, path: &Path) -> Result<OwnedFd, std::io::Error> {
        LibcFsOps.openat_dir_nofollow(dirfd, path)
    }

    fn fstatat_nofollow(&self, dirfd: RawFd, path: &Path) -> Result<Option<ItemMeta>, std::io::Error> {
        LibcFsOps.fstatat_nofollow(dirfd, path)
    }

    fn getattrlistat_name(&self, dirfd: RawFd, path: &Path) -> Result<Option<String>, std::io::Error> {
        LibcFsOps.getattrlistat_name(dirfd, path)
    }

    fn renameatx(&self, from_fd: RawFd, from: &Path, to_fd: RawFd, to: &Path, flags: u32) -> Result<(), std::io::Error> {
        if flags & RENAME_EXCL != 0 {
            if self.rename_excl_runtime_enotsup {
                return Err(std::io::Error::from_raw_os_error(libc::ENOTSUP));
            }
            self.fire_before_claim(to);
        } else if flags & RENAME_SWAP == 0 {
            self.plain_rename_calls.set(self.plain_rename_calls.get() + 1);
        }
        LibcFsOps.renameatx(from_fd, from, to_fd, to, flags)
    }

    fn unlinkat(&self, dirfd: RawFd, path: &Path, is_dir: bool) -> Result<(), std::io::Error> {
        LibcFsOps.unlinkat(dirfd, path, is_dir)
    }

    fn remove_tree(&self, dirfd: RawFd, path: &Path) -> Result<(), std::io::Error> {
        LibcFsOps.remove_tree(dirfd, path)
    }

    fn probe_vol_caps(&self, path: &Path) -> Result<VolCaps, std::io::Error> {
        if self.probe_fails {
            return Err(std::io::Error::from_raw_os_error(libc::EIO));
        }
        LibcFsOps.probe_vol_caps(path)
    }

    fn linkat(&self, from_fd: RawFd, from: &Path, to_fd: RawFd, to: &Path) -> Result<(), std::io::Error> {
        if let Some(errno) = self.link_err {
            return Err(std::io::Error::from_raw_os_error(errno));
        }
        self.fire_before_claim(to);
        LibcFsOps.linkat(from_fd, from, to_fd, to)
    }
}

struct Fixture {
    _tmp: TempDir,
    dest: PathBuf,
    payload: PathBuf,
    displaced: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let tmp = TempDir::new().expect("tempdir");
        let (dest, payload, displaced) =
            (tmp.path().join("dest"), tmp.path().join("payload"), tmp.path().join("displaced"));
        for dir in [&dest, &payload, &displaced] {
            fs::create_dir(dir).expect("create fixture dir");
        }
        Self { _tmp: tmp, dest, payload, displaced }
    }

    fn commit<F: FsOps>(&self, fs_ops: &F, caps: VolCaps, resolutions: &ResolutionSet) -> ExtractionOutcome {
        let open = |p: &Path| LibcFsOps.openat_dir_nofollow(libc::AT_FDCWD, p).expect("open fixture dir");
        let (dest, payload, displaced) = (open(&self.dest), open(&self.payload), open(&self.displaced));
        let table = ConflictTable::builder().build().expect("empty conflict table");
        DFSCommitWalker::new(fs_ops, dest.as_raw_fd(), payload.as_raw_fd(), displaced.as_raw_fd(), caps)
            .commit(&table, resolutions)
            .expect("commit walk")
    }
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn sorted_names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn keep_both_skips_taken_file_candidate_with_rename_excl_and_linkat() {
    for (label, caps, runtime_enotsup) in [
        ("rename_excl", WITH_EXCL, false),
        ("linkat_from_caps", NO_EXCL, false),
        ("linkat_after_runtime_enotsup", WITH_EXCL, true),
    ] {
        let fx = Fixture::new();
        fs::write(fx.dest.join("a.txt"), "old").unwrap();
        fs::write(fx.dest.join("a 2.txt"), "user 2").unwrap();
        fs::write(fx.payload.join("a.txt"), "new").unwrap();
        let fs_ops = ScriptedFs { rename_excl_runtime_enotsup: runtime_enotsup, ..ScriptedFs::default() };

        let outcome = fx.commit(&fs_ops, caps, &ResolutionSet::from_action(CollisionAction::KeepBoth));

        assert_eq!(outcome.status, OutcomeStatus::Completed, "{label}: {outcome:?}");
        assert_eq!(outcome.kept_both, 1, "{label}");
        assert_eq!(fs_ops.plain_rename_calls.get(), 0, "{label}: must not use plain rename");
        assert_eq!(read(&fx.dest.join("a.txt")), "old", "{label}");
        assert_eq!(read(&fx.dest.join("a 2.txt")), "user 2", "{label}: taken candidate overwritten");
        assert_eq!(read(&fx.dest.join("a 3.txt")), "new", "{label}");
        assert!(!fx.payload.join("a.txt").exists(), "{label}: staged file must be unlinked");
    }
}

#[test]
fn keep_both_skips_taken_directory_candidates_including_empty_ones() {
    let fx = Fixture::new();
    fs::write(fx.dest.join("d"), "file d").unwrap();
    fs::create_dir(fx.dest.join("d 2")).unwrap();
    fs::write(fx.dest.join("d 2/keep.txt"), "user").unwrap();
    fs::create_dir(fx.dest.join("d 3")).unwrap();
    fs::create_dir(fx.payload.join("d")).unwrap();
    fs::write(fx.payload.join("d/inner.txt"), "new").unwrap();
    let fs_ops = ScriptedFs::default();

    let outcome = fx.commit(&fs_ops, WITH_EXCL, &ResolutionSet::from_action(CollisionAction::KeepBoth));

    assert_eq!(outcome.status, OutcomeStatus::Completed, "{outcome:?}");
    assert_eq!(fs_ops.plain_rename_calls.get(), 0);
    assert_eq!(read(&fx.dest.join("d")), "file d");
    assert_eq!(read(&fx.dest.join("d 2/keep.txt")), "user");
    assert!(!fx.dest.join("d 2/inner.txt").exists(), "merged into foreign dir");
    assert_eq!(fs::read_dir(fx.dest.join("d 3")).unwrap().count(), 0, "empty dir replaced");
    assert_eq!(read(&fx.dest.join("d 4/inner.txt")), "new");
}

#[test]
fn unsupported_file_placement_returns_not_supported_and_preserves_staged_source() {
    for errno in [libc::ENOTSUP, libc::EPERM] {
        let fx = Fixture::new();
        fs::write(fx.dest.join("a.txt"), "old").unwrap();
        fs::write(fx.dest.join("a 2.txt"), "user 2").unwrap();
        fs::write(fx.payload.join("a.txt"), "new a").unwrap();
        fs::write(fx.payload.join("free.txt"), "new free").unwrap();
        let fs_ops = ScriptedFs { link_err: Some(errno), ..ScriptedFs::default() };

        let outcome = fx.commit(&fs_ops, NO_EXCL, &ResolutionSet::from_action(CollisionAction::KeepBoth));

        assert_eq!(outcome.status, OutcomeStatus::CompletedWithFailures, "{outcome:?}");
        assert_eq!(outcome.failed.len(), 2, "{outcome:?}");
        for failed in &outcome.failed {
            assert_eq!(failed.reason, FailureReason::NotSupportedByVolume, "{failed:?}");
            assert_eq!(failed.errno, libc::ENOTSUP, "{failed:?}");
        }
        assert_eq!(fs_ops.plain_rename_calls.get(), 0, "must never fall back to plain rename");
        assert_eq!(sorted_names(&fx.dest), vec!["a 2.txt", "a.txt"], "no reservation or new file allowed");
        assert_eq!(read(&fx.dest.join("a.txt")), "old");
        assert_eq!(read(&fx.dest.join("a 2.txt")), "user 2");
        assert_eq!(read(&fx.payload.join("a.txt")), "new a", "staged source must remain intact");
        assert_eq!(read(&fx.payload.join("free.txt")), "new free", "staged source must remain intact");
    }
}

#[test]
fn unsupported_directory_and_symlink_placement_never_replaces_empty_dir_or_leaves_placeholder() {
    for (caps, runtime_enotsup) in [(NO_EXCL, false), (WITH_EXCL, true)] {
        let fx = Fixture::new();
        fs::write(fx.dest.join("d"), "file d").unwrap();
        // Existing empty directory at candidate `d 2`: plain rename would overwrite it!
        fs::create_dir(fx.dest.join("d 2")).unwrap();
        let empty_dir_ino = fs::metadata(fx.dest.join("d 2")).unwrap();
        fs::create_dir(fx.payload.join("d")).unwrap();
        fs::write(fx.payload.join("d/inner.txt"), "staged dir").unwrap();
        fs::create_dir(fx.payload.join("fresh_dir")).unwrap();
        fs::write(fx.payload.join("fresh_dir/f.txt"), "fresh").unwrap();
        unix_fs::symlink("target.txt", fx.payload.join("sym")).unwrap();
        let fs_ops = ScriptedFs { rename_excl_runtime_enotsup: runtime_enotsup, ..ScriptedFs::default() };

        let outcome = fx.commit(&fs_ops, caps, &ResolutionSet::from_action(CollisionAction::KeepBoth));

        assert_eq!(outcome.status, OutcomeStatus::CompletedWithFailures, "{outcome:?}");
        assert_eq!(outcome.failed.len(), 3, "{outcome:?}");
        for failed in &outcome.failed {
            assert_eq!(failed.reason, FailureReason::NotSupportedByVolume, "{failed:?}");
            assert_eq!(failed.errno, libc::ENOTSUP, "{failed:?}");
        }
        assert_eq!(fs_ops.plain_rename_calls.get(), 0, "must never call plain rename");
        assert_eq!(sorted_names(&fx.dest), vec!["d", "d 2"], "no placeholder entry allowed in dest");
        assert_eq!(read(&fx.dest.join("d")), "file d");
        assert_eq!(fs::read_dir(fx.dest.join("d 2")).unwrap().count(), 0, "empty dir must stay empty");
        assert_eq!(
            std::os::unix::fs::MetadataExt::ino(&fs::metadata(fx.dest.join("d 2")).unwrap()),
            std::os::unix::fs::MetadataExt::ino(&empty_dir_ino),
            "empty directory inode must not be replaced"
        );
        assert_eq!(read(&fx.payload.join("d/inner.txt")), "staged dir");
        assert_eq!(read(&fx.payload.join("fresh_dir/f.txt")), "fresh");
        assert!(fx.payload.join("sym").symlink_metadata().unwrap().file_type().is_symlink());
    }
}

#[test]
fn same_name_file_appearing_before_claim_is_preserved() {
    for (label, caps) in [("rename_excl", WITH_EXCL), ("linkat", NO_EXCL)] {
        for action in [None, Some(CollisionAction::KeepBoth)] {
            let fx = Fixture::new();
            fs::write(fx.payload.join("x.txt"), "new").unwrap();
            let fs_ops = ScriptedFs::with_file_race(&fx.dest, "x.txt", b"racer");
            let resolutions = ResolutionSet { default_action: action, overrides: Vec::new() };

            let outcome = fx.commit(&fs_ops, caps, &resolutions);

            assert!(fs_ops.fired.get(), "{label}: race not injected");
            assert_eq!(fs_ops.plain_rename_calls.get(), 0, "{label}");
            assert_eq!(read(&fx.dest.join("x.txt")), "racer", "{label} {action:?}: racer overwritten");
            if action.is_none() {
                assert_eq!(outcome.status, OutcomeStatus::CompletedWithFailures, "{label}");
                assert_eq!(outcome.failed[0].errno, libc::EEXIST, "{label}");
                assert_eq!(sorted_names(&fx.dest), vec!["x.txt"], "{label}: no extra artifacts");
                assert_eq!(read(&fx.payload.join("x.txt")), "new", "{label}");
            } else {
                assert_eq!(outcome.status, OutcomeStatus::Completed, "{label}");
                assert_eq!(outcome.kept_both, 1, "{label}: {outcome:?}");
                assert_eq!(read(&fx.dest.join("x 2.txt")), "new", "{label}");
            }
        }
    }
}

#[test]
fn concurrent_file_or_directory_before_directory_claim_is_handled_safely() {
    // 1. Concurrent regular file appearing at `dd` with no default action -> EEXIST failure, racer preserved.
    let fx = Fixture::new();
    fs::create_dir(fx.payload.join("dd")).unwrap();
    fs::write(fx.payload.join("dd/f.txt"), "new").unwrap();
    let fs_ops = ScriptedFs::with_file_race(&fx.dest, "dd", b"racer file");

    let outcome = fx.commit(&fs_ops, WITH_EXCL, &ResolutionSet::new());

    assert!(fs_ops.fired.get());
    assert_eq!(outcome.status, OutcomeStatus::CompletedWithFailures, "{outcome:?}");
    assert_eq!(outcome.failed[0].errno, libc::EEXIST);
    assert_eq!(read(&fx.dest.join("dd")), "racer file");
    assert_eq!(read(&fx.payload.join("dd/f.txt")), "new");

    // 2. Concurrent regular file appearing at `dd` under KeepBoth -> lands at `dd 2`.
    let fx2 = Fixture::new();
    fs::create_dir(fx2.payload.join("dd")).unwrap();
    fs::write(fx2.payload.join("dd/f.txt"), "new").unwrap();
    let fs_ops2 = ScriptedFs::with_file_race(&fx2.dest, "dd", b"racer file");

    let outcome2 = fx2.commit(&fs_ops2, WITH_EXCL, &ResolutionSet::from_action(CollisionAction::KeepBoth));

    assert!(fs_ops2.fired.get());
    assert_eq!(outcome2.status, OutcomeStatus::Completed, "{outcome2:?}");
    assert_eq!(outcome2.kept_both, 1);
    assert_eq!(read(&fx2.dest.join("dd")), "racer file");
    assert_eq!(read(&fx2.dest.join("dd 2/f.txt")), "new");

    // 3. Concurrent directory appearing at `dd` -> merges children without losing existing files.
    let fx3 = Fixture::new();
    fs::create_dir(fx3.payload.join("dd")).unwrap();
    fs::write(fx3.payload.join("dd/f.txt"), "new").unwrap();
    let fs_ops3 = ScriptedFs::with_dir_race(&fx3.dest, "dd", "racer.txt", b"racer child");

    let outcome3 = fx3.commit(&fs_ops3, WITH_EXCL, &ResolutionSet::new());

    assert!(fs_ops3.fired.get());
    assert_eq!(outcome3.status, OutcomeStatus::Completed, "{outcome3:?}");
    assert_eq!(outcome3.merged_dirs, 1);
    assert_eq!(outcome3.extracted, 1);
    assert_eq!(read(&fx3.dest.join("dd/racer.txt")), "racer child");
    assert_eq!(read(&fx3.dest.join("dd/f.txt")), "new");
}

#[test]
fn failed_capability_probe_never_selects_overwriting_rename() {
    // 1. With linkat available, fallback VolCaps::default() places files exclusively via linkat.
    let fx = Fixture::new();
    fs::write(fx.dest.join("a.txt"), "old").unwrap();
    fs::write(fx.dest.join("a 2.txt"), "user 2").unwrap();
    fs::write(fx.payload.join("a.txt"), "new").unwrap();
    let fs_ops = ScriptedFs { probe_fails: true, ..ScriptedFs::default() };

    let caps = fs_ops.probe_vol_caps(&fx.dest).unwrap_or_default();
    assert_eq!(caps, VolCaps::default());
    let outcome = fx.commit(&fs_ops, caps, &ResolutionSet::from_action(CollisionAction::KeepBoth));

    assert_eq!(outcome.status, OutcomeStatus::Completed, "{outcome:?}");
    assert_eq!(fs_ops.plain_rename_calls.get(), 0);
    assert_eq!(read(&fx.dest.join("a.txt")), "old");
    assert_eq!(read(&fx.dest.join("a 2.txt")), "user 2");
    assert_eq!(read(&fx.dest.join("a 3.txt")), "new");

    // 2. Without linkat, fallback VolCaps::default() reports NotSupportedByVolume without mutation.
    let fx2 = Fixture::new();
    fs::write(fx2.dest.join("a.txt"), "old").unwrap();
    fs::write(fx2.dest.join("a 2.txt"), "user 2").unwrap();
    fs::write(fx2.payload.join("a.txt"), "new").unwrap();
    let fs_ops2 = ScriptedFs { probe_fails: true, link_err: Some(libc::ENOTSUP), ..ScriptedFs::default() };

    let caps2 = fs_ops2.probe_vol_caps(&fx2.dest).unwrap_or_default();
    let outcome2 = fx2.commit(&fs_ops2, caps2, &ResolutionSet::from_action(CollisionAction::KeepBoth));

    assert_eq!(outcome2.status, OutcomeStatus::CompletedWithFailures);
    assert_eq!(outcome2.failed[0].reason, FailureReason::NotSupportedByVolume);
    assert_eq!(fs_ops2.plain_rename_calls.get(), 0);
    assert_eq!(sorted_names(&fx2.dest), vec!["a 2.txt", "a.txt"]);
    assert_eq!(read(&fx2.payload.join("a.txt")), "new");
}

#[test]
fn failed_capability_probe_aborts_session_commit_without_mutation() {
    let fx = Fixture::new();
    fs::write(fx.dest.join("a.txt"), "old").unwrap();
    let staged = ExtractionSession::new(&fx.dest).stage().expect("stage");
    let payload = staged.staging().payload_dir().0;
    fs::write(payload.join("a.txt"), "new").unwrap();
    fs::write(payload.join("b.txt"), "new b").unwrap();
    let fs_ops = ScriptedFs { probe_fails: true, ..ScriptedFs::default() };

    let result = staged.commit(&fs_ops, &ResolutionSet::from_action(CollisionAction::Replace));

    assert!(result.is_err(), "probe failure must not reach the commit walk");
    assert_eq!(read(&fx.dest.join("a.txt")), "old");
    assert!(!fx.dest.join("b.txt").exists());
}

#[test]
fn commit_with_mapping_tracks_file_subtree_merged_and_keep_both() {
    use crate::fs::collision::{CommittedEntryDisposition, CommittedPathEntry, ItemKind};

    let fx = Fixture::new();
    // Pre-existing destination items:
    // - `a.txt` (file) -> collides with staged `a.txt` (KeepBoth -> `a 2.txt`)
    // - `pkg` (file) -> collides with staged directory `pkg` (KeepBoth -> `pkg 2`)
    // - `existing_dir` (directory) -> merges with staged `existing_dir` (`MergedExistingDir`)
    fs::write(fx.dest.join("a.txt"), "old a").unwrap();
    fs::write(fx.dest.join("pkg"), "file pkg").unwrap();
    fs::create_dir(fx.dest.join("existing_dir")).unwrap();

    // Staged payload items:
    fs::write(fx.payload.join("a.txt"), "new a").unwrap();
    fs::write(fx.payload.join("fresh.txt"), "fresh").unwrap();
    fs::create_dir_all(fx.payload.join("pkg/bin")).unwrap();
    fs::write(fx.payload.join("pkg/bin/run.sh"), "#!/bin/sh\n").unwrap();
    fs::create_dir_all(fx.payload.join("existing_dir/new_sub")).unwrap();
    fs::write(fx.payload.join("existing_dir/new_sub/nested.txt"), "nested").unwrap();

    let open = |p: &Path| LibcFsOps.openat_dir_nofollow(libc::AT_FDCWD, p).expect("open dir");
    let (dest, payload, displaced) = (open(&fx.dest), open(&fx.payload), open(&fx.displaced));
    let table = ConflictTable::builder().build().expect("empty conflict table");
    let walker = DFSCommitWalker::new(
        &LibcFsOps,
        dest.as_raw_fd(),
        payload.as_raw_fd(),
        displaced.as_raw_fd(),
        WITH_EXCL,
    );
    let resolutions = ResolutionSet::from_action(CollisionAction::KeepBoth);
    let (outcome, mapping) = walker.commit_with_mapping(&table, &resolutions).expect("commit_with_mapping");

    assert_eq!(outcome.status, OutcomeStatus::Completed);
    assert!(mapping.entries().contains(&CommittedPathEntry {
        staged_rel_path: PathBuf::from("a.txt"),
        committed_rel_path: PathBuf::from("a 2.txt"),
        kind: ItemKind::File,
        disposition: CommittedEntryDisposition::KeptBoth,
        whole_subtree: false,
    }));
    assert!(mapping.entries().contains(&CommittedPathEntry {
        staged_rel_path: PathBuf::from("fresh.txt"),
        committed_rel_path: PathBuf::from("fresh.txt"),
        kind: ItemKind::File,
        disposition: CommittedEntryDisposition::Extracted,
        whole_subtree: false,
    }));
    assert!(mapping.entries().contains(&CommittedPathEntry {
        staged_rel_path: PathBuf::from("pkg"),
        committed_rel_path: PathBuf::from("pkg 2"),
        kind: ItemKind::Directory,
        disposition: CommittedEntryDisposition::KeptBoth,
        whole_subtree: true,
    }));
    assert!(mapping.entries().contains(&CommittedPathEntry {
        staged_rel_path: PathBuf::from("existing_dir"),
        committed_rel_path: PathBuf::from("existing_dir"),
        kind: ItemKind::Directory,
        disposition: CommittedEntryDisposition::MergedExistingDir,
        whole_subtree: false,
    }));
    assert!(mapping.entries().contains(&CommittedPathEntry {
        staged_rel_path: PathBuf::from("existing_dir/new_sub"),
        committed_rel_path: PathBuf::from("existing_dir/new_sub"),
        kind: ItemKind::Directory,
        disposition: CommittedEntryDisposition::Extracted,
        whole_subtree: true,
    }));

    assert_eq!(
        mapping.resolve_with_disposition(Path::new("pkg/bin/run.sh")),
        Some((PathBuf::from("pkg 2/bin/run.sh"), CommittedEntryDisposition::KeptBoth))
    );
    assert!(mapping.is_merged_existing_dir(Path::new("existing_dir")));
    assert_eq!(mapping.resolve_committed_path(Path::new("existing_dir")), None);
    assert_eq!(
        mapping.resolve_committed_path(Path::new("existing_dir/new_sub/nested.txt")),
        Some(PathBuf::from("existing_dir/new_sub/nested.txt"))
    );
}
