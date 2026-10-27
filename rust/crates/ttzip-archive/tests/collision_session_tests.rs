// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Integration tests for collision session lifecycle, preflight prefix pruning, and commit walker.

use std::fs;
use std::os::unix::io::{OwnedFd, RawFd};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use tempfile::tempdir;
use ttzip_archive::fs::collision::fs_ops::{FsOps, LibcFsOps, VolCaps};
use ttzip_archive::fs::collision::preflight::preflight_detect;
use ttzip_archive::fs::collision::session::Session;
use ttzip_archive::fs::collision::{
    CollisionAction, ItemKind, ItemMeta, OutcomeStatus, ResolutionSet,
};

struct CountingFsOps {
    inner: LibcFsOps,
    fstatat_count: AtomicUsize,
}

impl CountingFsOps {
    fn new() -> Self {
        Self {
            inner: LibcFsOps,
            fstatat_count: AtomicUsize::new(0),
        }
    }

    fn fstatat_calls(&self) -> usize {
        self.fstatat_count.load(Ordering::SeqCst)
    }
}

impl FsOps for CountingFsOps {
    fn openat_dir_nofollow(&self, dirfd: RawFd, path: &Path) -> Result<OwnedFd, std::io::Error> {
        self.inner.openat_dir_nofollow(dirfd, path)
    }

    fn fstatat_nofollow(&self, dirfd: RawFd, path: &Path) -> Result<Option<ItemMeta>, std::io::Error> {
        self.fstatat_count.fetch_add(1, Ordering::SeqCst);
        self.inner.fstatat_nofollow(dirfd, path)
    }

    fn getattrlistat_name(&self, dirfd: RawFd, path: &Path) -> Result<Option<String>, std::io::Error> {
        self.inner.getattrlistat_name(dirfd, path)
    }

    fn renameatx(&self, from_fd: RawFd, from: &Path, to_fd: RawFd, to: &Path, flags: u32) -> Result<(), std::io::Error> {
        self.inner.renameatx(from_fd, from, to_fd, to, flags)
    }

    fn unlinkat(&self, dirfd: RawFd, path: &Path, is_dir: bool) -> Result<(), std::io::Error> {
        self.inner.unlinkat(dirfd, path, is_dir)
    }

    fn remove_tree(&self, dirfd: RawFd, path: &Path) -> Result<(), std::io::Error> {
        self.inner.remove_tree(dirfd, path)
    }

    fn probe_vol_caps(&self, path: &Path) -> Result<VolCaps, std::io::Error> {
        self.inner.probe_vol_caps(path)
    }
}

#[test]
fn test_preflight_prefix_pruning() {
    let tmp = tempdir().expect("tempdir");
    let dest_dir = tmp.path().join("dest");
    fs::create_dir_all(dest_dir.join("existing_dir")).expect("create existing_dir");
    fs::write(dest_dir.join("existing_dir/existing_file.txt"), b"already exists").expect("write existing_file");

    let items = vec![
        (
            PathBuf::from("absent_dir/sub1/deep1.txt"),
            ItemMeta { kind: ItemKind::File, size: 10, mtime_unix_ns: 0 },
        ),
        (
            PathBuf::from("absent_dir/sub1/deep2.txt"),
            ItemMeta { kind: ItemKind::File, size: 20, mtime_unix_ns: 0 },
        ),
        (
            PathBuf::from("absent_dir/sub2/deep3.txt"),
            ItemMeta { kind: ItemKind::File, size: 30, mtime_unix_ns: 0 },
        ),
        (
            PathBuf::from("existing_dir/existing_file.txt"),
            ItemMeta { kind: ItemKind::File, size: 40, mtime_unix_ns: 0 },
        ),
        (
            PathBuf::from("existing_dir/new_file.txt"),
            ItemMeta { kind: ItemKind::File, size: 50, mtime_unix_ns: 0 },
        ),
    ];

    let counting_fs = CountingFsOps::new();
    let dest_c = std::ffi::CString::new(dest_dir.as_os_str().to_str().unwrap()).unwrap();
    // SAFETY: open directory
    let dirfd = unsafe { libc::open(dest_c.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) };
    assert!(dirfd >= 0);

    let table = preflight_detect(&counting_fs, dirfd, &dest_dir, &items).expect("preflight");

    // SAFETY: close dirfd
    unsafe { libc::close(dirfd); }

    assert_eq!(table.len(), 1);
    let record = table.find_by_path("existing_dir/existing_file.txt").expect("conflict record");
    assert_eq!(record.relative_path, "existing_dir/existing_file.txt");

    // Preflight must probe absent_dir once, then skip all 3 children without additional probes
    // Probes:
    // 1. parent absent_dir -> Absent
    // 2. deep2 -> skipped via absent_dirs
    // 3. deep3 -> skipped via absent_dirs
    // 4. parent existing_dir -> Present
    // 5. existing_file.txt -> Conflict!
    // 6. new_file.txt -> Absent
    assert!(counting_fs.fstatat_calls() <= 4, "fstatat calls: {}", counting_fs.fstatat_calls());
}

#[test]
fn test_commit_replace_same_type() {
    let tmp = tempdir().expect("tempdir");
    let dest_dir = tmp.path().join("dest");
    fs::create_dir_all(&dest_dir).expect("create dest");
    fs::write(dest_dir.join("file.txt"), b"old content").expect("write old file");

    let session = Session::new(&dest_dir);
    let mut staged = session.stage().expect("stage");

    fs::write(staged.staging().payload_dir().0.join("file.txt"), b"new content").expect("write new payload");

    let fs_ops = LibcFsOps;
    let _ = staged.post_stage_detect(&fs_ops).expect("post stage detect");
    assert_eq!(staged.table().map(|t| t.len()), Some(1));

    let resolutions = ResolutionSet::from_action(CollisionAction::Replace);
    let (_committed, outcome) = staged.commit(&fs_ops, &resolutions).expect("commit");

    assert_eq!(outcome.status, OutcomeStatus::Completed);
    assert_eq!(outcome.replaced, 1);
    assert_eq!(outcome.failed.len(), 0);

    let content = fs::read_to_string(dest_dir.join("file.txt")).expect("read file");
    assert_eq!(content, "new content");
}

#[test]
fn test_commit_keep_both() {
    let tmp = tempdir().expect("tempdir");
    let dest_dir = tmp.path().join("dest");
    fs::create_dir_all(&dest_dir).expect("create dest");
    fs::write(dest_dir.join("photo.png"), b"v1").expect("write photo");
    fs::write(dest_dir.join("photo 2.png"), b"v2").expect("write photo 2");

    let session = Session::new(&dest_dir);
    let mut staged = session.stage().expect("stage");

    fs::write(staged.staging().payload_dir().0.join("photo.png"), b"v3").expect("write new photo");

    let fs_ops = LibcFsOps;
    let _ = staged.post_stage_detect(&fs_ops).expect("detect");
    assert_eq!(staged.table().map(|t| t.len()), Some(1));

    let resolutions = ResolutionSet::from_action(CollisionAction::KeepBoth);
    let (_committed, outcome) = staged.commit(&fs_ops, &resolutions).expect("commit");

    assert_eq!(outcome.status, OutcomeStatus::Completed);
    assert_eq!(outcome.kept_both, 1);
    assert_eq!(outcome.failed.len(), 0);

    assert_eq!(fs::read_to_string(dest_dir.join("photo.png")).unwrap(), "v1");
    assert_eq!(fs::read_to_string(dest_dir.join("photo 2.png")).unwrap(), "v2");
    assert_eq!(fs::read_to_string(dest_dir.join("photo 3.png")).unwrap(), "v3");
}

#[test]
fn test_commit_skip() {
    let tmp = tempdir().expect("tempdir");
    let dest_dir = tmp.path().join("dest");
    fs::create_dir_all(&dest_dir).expect("create dest");
    fs::write(dest_dir.join("skip_me.txt"), b"preserve me").expect("write original");

    let session = Session::new(&dest_dir);
    let mut staged = session.stage().expect("stage");

    fs::write(staged.staging().payload_dir().0.join("skip_me.txt"), b"incoming payload").expect("write incoming");

    let fs_ops = LibcFsOps;
    let _ = staged.post_stage_detect(&fs_ops).expect("detect");

    let resolutions = ResolutionSet::from_action(CollisionAction::Skip);
    let (_committed, outcome) = staged.commit(&fs_ops, &resolutions).expect("commit");

    assert_eq!(outcome.status, OutcomeStatus::Completed);
    assert_eq!(outcome.skipped, 1);
    assert_eq!(fs::read_to_string(dest_dir.join("skip_me.txt")).unwrap(), "preserve me");
}

#[test]
fn test_commit_type_mismatch_swap() {
    let tmp = tempdir().expect("tempdir");
    let dest_dir = tmp.path().join("dest");
    let dir_item = dest_dir.join("mismatch_target");
    fs::create_dir_all(&dir_item).expect("create mismatch dir");
    fs::write(dir_item.join("nested.txt"), b"nested content").expect("write nested");

    let session = Session::new(&dest_dir);
    let mut staged = session.stage().expect("stage");

    fs::write(staged.staging().payload_dir().0.join("mismatch_target"), b"i am a file").expect("write payload file");

    let fs_ops = LibcFsOps;
    let caps = fs_ops.probe_vol_caps(&dest_dir).expect("probe caps");

    let _ = staged.post_stage_detect(&fs_ops).expect("detect");
    let resolutions = ResolutionSet::from_action(CollisionAction::Replace);
    let (_committed, outcome) = staged.commit(&fs_ops, &resolutions).expect("commit");

    if caps.rename_swap {
        assert_eq!(outcome.status, OutcomeStatus::Completed);
        assert_eq!(outcome.replaced, 1);
        assert!(dest_dir.join("mismatch_target").is_file());
        assert_eq!(fs::read_to_string(dest_dir.join("mismatch_target")).unwrap(), "i am a file");
    } else {
        assert_eq!(outcome.failed.len(), 1);
    }
}

#[test]
fn test_session_typestate_transitions_and_abort() {
    let tmp = tempdir().expect("tempdir");
    let dest_dir = tmp.path().join("dest");

    let session = Session::new(&dest_dir);
    let staged = session.stage().expect("stage");
    let staging_path = staged.staging().staging_dir.clone();

    assert!(staging_path.exists());

    let (_aborted, outcome) = staged.abort();
    assert_eq!(outcome.status, OutcomeStatus::Stopped);
    assert!(!staging_path.exists(), "Staging dir should be deleted on abort");
}
