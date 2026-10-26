// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Fault injection tests for commit walker.

use std::cell::Cell;
use std::fs;
use std::os::unix::io::{OwnedFd, RawFd};
use std::path::{Path, PathBuf};

use tempfile::tempdir;
use ttzip_archive::fs::collision::fs_ops::{FsOps, LibcFsOps, VolCaps};
use ttzip_archive::fs::collision::session::Session;
use ttzip_archive::fs::collision::{
    CollisionAction, OutcomeStatus, ResolutionSet,
};
use ttzip_archive::fs::collision::ItemMeta;
use ttzip_archive::fs::collision::sweep_stale;

struct FaultFsOps<F: FsOps> {
    inner: F,
    mutate_countdown: Cell<usize>,
}

impl<F: FsOps> FaultFsOps<F> {
    fn new(inner: F, fail_at: usize) -> Self {
        Self {
            inner,
            mutate_countdown: Cell::new(fail_at),
        }
    }

    fn tick(&self) -> Result<(), std::io::Error> {
        let count = self.mutate_countdown.get();
        if count == 1 {
            self.mutate_countdown.set(0);
            Err(std::io::Error::from_raw_os_error(libc::EIO))
        } else if count > 1 {
            self.mutate_countdown.set(count - 1);
            Ok(())
        } else {
            Ok(())
        }
    }
}

impl<F: FsOps> FsOps for FaultFsOps<F> {
    fn openat_dir_nofollow(&self, dirfd: RawFd, path: &Path) -> Result<OwnedFd, std::io::Error> {
        self.inner.openat_dir_nofollow(dirfd, path)
    }

    fn fstatat_nofollow(&self, dirfd: RawFd, path: &Path) -> Result<Option<ItemMeta>, std::io::Error> {
        self.inner.fstatat_nofollow(dirfd, path)
    }

    fn getattrlistat_name(&self, dirfd: RawFd, path: &Path) -> Result<Option<String>, std::io::Error> {
        self.inner.getattrlistat_name(dirfd, path)
    }

    fn renameatx(&self, from_fd: RawFd, from: &Path, to_fd: RawFd, to: &Path, flags: u32) -> Result<(), std::io::Error> {
        self.tick()?;
        self.inner.renameatx(from_fd, from, to_fd, to, flags)
    }

    fn unlinkat(&self, dirfd: RawFd, path: &Path, is_dir: bool) -> Result<(), std::io::Error> {
        self.tick()?;
        self.inner.unlinkat(dirfd, path, is_dir)
    }

    fn remove_tree(&self, dirfd: RawFd, path: &Path) -> Result<(), std::io::Error> {
        self.tick()?;
        self.inner.remove_tree(dirfd, path)
    }

    fn probe_vol_caps(&self, path: &Path) -> Result<VolCaps, std::io::Error> {
        self.inner.probe_vol_caps(path)
    }
}

#[test]
fn test_fault_injection_matrix() {
    let actions = [CollisionAction::Replace, CollisionAction::KeepBoth];
    for action in actions {
        for is_mismatch in [false, true] {
            let mut k = 1;
            loop {
                let tmp = tempdir().expect("tempdir");
                let dest_dir = tmp.path().join("dest");
                fs::create_dir_all(&dest_dir).expect("create dest");

                if !is_mismatch {
                    fs::write(dest_dir.join("item1"), b"old 1").unwrap();
                    fs::write(dest_dir.join("item2"), b"old 2").unwrap();
                } else {
                    fs::write(dest_dir.join("item1"), b"old file").unwrap();
                    fs::create_dir(dest_dir.join("item2")).unwrap();
                }

                let session = Session::new(&dest_dir);
                let mut staged = session.stage().expect("stage");
                let payload_dir = staged.staging().payload_dir().0.clone();

                if !is_mismatch {
                    fs::write(payload_dir.join("item1"), b"new 1").unwrap();
                    fs::write(payload_dir.join("item2"), b"new 2").unwrap();
                } else {
                    fs::create_dir(payload_dir.join("item1")).unwrap();
                    fs::write(payload_dir.join("item2"), b"new file").unwrap();
                }

                let fs_ops = LibcFsOps;
                let _ = staged.post_stage_detect(&fs_ops).expect("detect");

                let fault_fs = FaultFsOps::new(LibcFsOps, k);
                let resolutions = ResolutionSet::from_action(action);

                let res = staged.commit(&fault_fs, &resolutions);
                match res {
                    Ok((_committed, outcome)) => {
                        if outcome.status == OutcomeStatus::Completed {
                            break;
                        }
                        assert_eq!(outcome.status, OutcomeStatus::CompletedWithFailures);
                        let failed = outcome.failed.iter().any(|f| f.errno == libc::EIO);
                        assert!(failed, "Expected an EIO failure");
                        
                        let content1 = if dest_dir.join("item1").is_dir() { "dir".to_string() } else { fs::read_to_string(dest_dir.join("item1")).unwrap_or_default() };
                        let content2 = if dest_dir.join("item2").is_dir() { "dir".to_string() } else { fs::read_to_string(dest_dir.join("item2")).unwrap_or_default() };
                        
                        let valid1 = ["old 1", "new 1", "old file", "dir"];
                        let valid2 = ["old 2", "new 2", "new file", "dir"];
                        
                        assert!(valid1.contains(&content1.as_str()));
                        assert!(valid2.contains(&content2.as_str()));
                    }
                    Err(_) => {
                        let content1 = if dest_dir.join("item1").is_dir() { "dir".to_string() } else { fs::read_to_string(dest_dir.join("item1")).unwrap_or_default() };
                        let content2 = if dest_dir.join("item2").is_dir() { "dir".to_string() } else { fs::read_to_string(dest_dir.join("item2")).unwrap_or_default() };
                        
                        let valid1 = ["old 1", "new 1", "old file", "dir"];
                        let valid2 = ["old 2", "new 2", "new file", "dir"];
                        
                        assert!(valid1.contains(&content1.as_str()));
                        assert!(valid2.contains(&content2.as_str()));
                    }
                }
                k += 1;
                if k > 50 {
                    panic!("Too many operations for a single commit");
                }
            }
        }
    }
}

struct AbortFsOps;
impl FsOps for AbortFsOps {
    fn openat_dir_nofollow(&self, dirfd: RawFd, path: &Path) -> Result<OwnedFd, std::io::Error> {
        LibcFsOps.openat_dir_nofollow(dirfd, path)
    }
    fn fstatat_nofollow(&self, dirfd: RawFd, path: &Path) -> Result<Option<ItemMeta>, std::io::Error> {
        LibcFsOps.fstatat_nofollow(dirfd, path)
    }
    fn getattrlistat_name(&self, dirfd: RawFd, path: &Path) -> Result<Option<String>, std::io::Error> {
        LibcFsOps.getattrlistat_name(dirfd, path)
    }
    fn probe_vol_caps(&self, path: &Path) -> Result<VolCaps, std::io::Error> {
        LibcFsOps.probe_vol_caps(path)
    }
    fn renameatx(&self, _from_fd: RawFd, _from: &Path, _to_fd: RawFd, _to: &Path, _flags: u32) -> Result<(), std::io::Error> {
        std::process::abort();
    }
    fn unlinkat(&self, dirfd: RawFd, path: &Path, is_dir: bool) -> Result<(), std::io::Error> {
        LibcFsOps.unlinkat(dirfd, path, is_dir)
    }
    fn remove_tree(&self, dirfd: RawFd, path: &Path) -> Result<(), std::io::Error> {
        LibcFsOps.remove_tree(dirfd, path)
    }
}

#[test]
fn test_kill_crash_simulate() {
    if let Ok(tmp_path) = std::env::var("CRASH_TARGET") {
        let dest_dir = PathBuf::from(tmp_path);
        let session = Session::new(&dest_dir);
        let mut staged = session.stage().expect("stage");
        let payload_dir = staged.staging().payload_dir().0.clone();
        fs::write(payload_dir.join("target"), b"new target").unwrap();
        
        let fs_ops = LibcFsOps;
        let _ = staged.post_stage_detect(&fs_ops).expect("detect");
        
        let resolutions = ResolutionSet::from_action(CollisionAction::Replace);
        let _ = staged.commit(&AbortFsOps, &resolutions);
        
        return;
    }
    
    let tmp = tempdir().expect("tempdir");
    let dest_dir = tmp.path().join("dest");
    fs::create_dir_all(&dest_dir).expect("create dest");
    fs::write(dest_dir.join("target"), b"old target").unwrap();
    
    let exe = std::env::current_exe().unwrap();
    let status = std::process::Command::new(exe)
        .env("CRASH_TARGET", dest_dir.to_str().unwrap())
        .arg("--nocapture")
        .arg("test_kill_crash_simulate")
        .status()
        .unwrap();
        
    assert!(!status.success());
    
    let count = sweep_stale(&dest_dir);
    assert_eq!(count, 1, "Should have swept 1 staging directory");
    
    assert_eq!(fs::read_to_string(dest_dir.join("target")).unwrap(), "old target");
}

#[test]
fn test_replace_failure_does_not_increment_replaced_counter() {
    let tmp = tempdir().expect("tempdir");
    let dest_dir = tmp.path().join("dest");
    fs::create_dir_all(&dest_dir).expect("create dest");
    fs::write(dest_dir.join("item1.txt"), b"original_1").unwrap();
    fs::write(dest_dir.join("item2.txt"), b"original_2").unwrap();

    let session = Session::new(&dest_dir);
    let mut staged = session.stage().expect("stage");
    let payload_dir = staged.staging().payload_dir().0.clone();
    fs::write(payload_dir.join("item1.txt"), b"updated_1").unwrap();
    fs::write(payload_dir.join("item2.txt"), b"updated_2").unwrap();

    let _ = staged.post_stage_detect(&LibcFsOps).expect("detect");

    // Fail the 1st renameatx call; the 2nd renameatx call succeeds
    let fault_fs = FaultFsOps::new(LibcFsOps, 1);
    let resolutions = ResolutionSet::from_action(CollisionAction::Replace);
    let (_committed, outcome) = staged.commit(&fault_fs, &resolutions).expect("commit must not abort walk");

    assert_eq!(outcome.status, OutcomeStatus::CompletedWithFailures);
    assert_eq!(outcome.replaced, 1, "Only the 1 succeeded replacement may increment outcome.replaced");
    assert_eq!(outcome.failed.len(), 1, "The failed replacement must be recorded in outcome.failed");
    assert_eq!(outcome.failed[0].errno, libc::EIO);

    // One file must retain original content and the other must have updated content
    let c1 = fs::read_to_string(dest_dir.join("item1.txt")).unwrap();
    let c2 = fs::read_to_string(dest_dir.join("item2.txt")).unwrap();
    assert!(
        (c1 == "original_1" && c2 == "updated_2") || (c1 == "updated_1" && c2 == "original_2"),
        "Failed item must keep original content while sibling succeeds: c1={c1}, c2={c2}"
    );
}

#[test]
fn test_commit_recheck_dynamic_collision_replace_and_failure() {
    // 1. Dynamic conflict appearing AFTER post_stage_detect with Replace failing on renameatx
    let tmp = tempdir().expect("tempdir");
    let dest_dir = tmp.path().join("dest");
    fs::create_dir_all(&dest_dir).expect("create dest");

    let session = Session::new(&dest_dir);
    let mut staged = session.stage().expect("stage");
    let payload_dir = staged.staging().payload_dir().0.clone();
    fs::write(payload_dir.join("dynamic.txt"), b"staged_payload").unwrap();

    // Detect when destination is still empty (0 conflicts in table)
    let summary = staged.post_stage_detect(&LibcFsOps).expect("detect");
    assert_eq!(summary.len(), 0);

    // Now create a conflicting file at destination before commit (triggers CommitRecheck EEXIST)
    fs::write(dest_dir.join("dynamic.txt"), b"preexisting_dynamic").unwrap();

    // 1st renameatx is RENAME_EXCL (returns EEXIST -> enters CommitRecheck);
    // 2nd renameatx is the CommitRecheck Replace call -> inject EIO on call #2!
    let fault_fs = FaultFsOps::new(LibcFsOps, 2);
    let resolutions = ResolutionSet::from_action(CollisionAction::Replace);
    let (_committed, outcome) = staged.commit(&fault_fs, &resolutions).expect("commit");

    assert_eq!(outcome.status, OutcomeStatus::CompletedWithFailures);
    assert_eq!(
        outcome.replaced, 0,
        "CommitRecheck Replace failure must NOT increment outcome.replaced"
    );
    assert_eq!(outcome.failed.len(), 1);
    assert_eq!(outcome.failed[0].relative_path, "dynamic.txt");
    assert_eq!(outcome.failed[0].errno, libc::EIO);
    assert_eq!(
        fs::read_to_string(dest_dir.join("dynamic.txt")).unwrap(),
        "preexisting_dynamic",
        "Pre-existing file must remain intact when CommitRecheck Replace fails"
    );
}

struct OpenatDirFaultFsOps;
impl FsOps for OpenatDirFaultFsOps {
    fn openat_dir_nofollow(&self, dirfd: RawFd, path: &Path) -> Result<OwnedFd, std::io::Error> {
        if path == Path::new("bad_merge_dir") {
            return Err(std::io::Error::from_raw_os_error(libc::EACCES));
        }
        LibcFsOps.openat_dir_nofollow(dirfd, path)
    }
    fn fstatat_nofollow(&self, dirfd: RawFd, path: &Path) -> Result<Option<ItemMeta>, std::io::Error> {
        LibcFsOps.fstatat_nofollow(dirfd, path)
    }
    fn getattrlistat_name(&self, dirfd: RawFd, path: &Path) -> Result<Option<String>, std::io::Error> {
        LibcFsOps.getattrlistat_name(dirfd, path)
    }
    fn renameatx(&self, from_fd: RawFd, from: &Path, to_fd: RawFd, to: &Path, flags: u32) -> Result<(), std::io::Error> {
        LibcFsOps.renameatx(from_fd, from, to_fd, to, flags)
    }
    fn unlinkat(&self, dirfd: RawFd, path: &Path, is_dir: bool) -> Result<(), std::io::Error> {
        LibcFsOps.unlinkat(dirfd, path, is_dir)
    }
    fn remove_tree(&self, dirfd: RawFd, path: &Path) -> Result<(), std::io::Error> {
        LibcFsOps.remove_tree(dirfd, path)
    }
    fn probe_vol_caps(&self, path: &Path) -> Result<VolCaps, std::io::Error> {
        LibcFsOps.probe_vol_caps(path)
    }
}

#[test]
fn test_directory_merge_openat_failure_records_failed_item_without_aborting_sibling() {
    let tmp = tempdir().expect("tempdir");
    let dest_dir = tmp.path().join("dest");
    fs::create_dir_all(dest_dir.join("bad_merge_dir")).expect("create dest dir");

    let session = Session::new(&dest_dir);
    let mut staged = session.stage().expect("stage");
    let payload_dir = staged.staging().payload_dir().0.clone();
    fs::create_dir_all(payload_dir.join("bad_merge_dir")).unwrap();
    fs::write(payload_dir.join("bad_merge_dir/inside.txt"), b"inside").unwrap();
    fs::write(payload_dir.join("z_sibling.txt"), b"sibling_ok").unwrap();

    let _ = staged.post_stage_detect(&LibcFsOps).expect("detect");
    let resolutions = ResolutionSet::from_action(CollisionAction::Replace);
    let (_committed, outcome) = staged
        .commit(&OpenatDirFaultFsOps, &resolutions)
        .expect("directory merge failure must record FailedItem without aborting commit walk");

    assert_eq!(outcome.status, OutcomeStatus::CompletedWithFailures);
    assert_eq!(outcome.merged_dirs, 0);
    assert_eq!(outcome.extracted, 1, "Sibling file must still be extracted");
    assert_eq!(outcome.failed.len(), 1);
    assert_eq!(outcome.failed[0].relative_path, "bad_merge_dir");
    assert_eq!(outcome.failed[0].errno, libc::EACCES);
    assert_eq!(fs::read_to_string(dest_dir.join("z_sibling.txt")).unwrap(), "sibling_ok");
}

#[test]
fn test_replace_via_swap_post_swap_staging_cleanup_failure_is_not_item_commit_failure() {
    let tmp = tempdir().expect("tempdir");
    let dest_dir = tmp.path().join("dest");
    // Pre-existing directory at dest/entry vs incoming file at payload/entry -> TypeMismatch -> replace_via_swap
    fs::create_dir_all(dest_dir.join("entry")).expect("create dest dir");
    fs::write(dest_dir.join("entry/old_child.txt"), b"old_child").unwrap();

    let session = Session::new(&dest_dir);
    let mut staged = session.stage().expect("stage");
    let payload_dir = staged.staging().payload_dir().0.clone();
    fs::write(payload_dir.join("entry"), b"new_file_content").unwrap();

    let table = staged.post_stage_detect(&LibcFsOps).expect("detect");
    assert_eq!(table.len(), 1);

    // Call #1 is RENAME_SWAP (succeeds); Call #2 is parking displaced old dir into displaced_dirfd (fails with EIO)
    let fault_fs = FaultFsOps::new(LibcFsOps, 2);
    let resolutions = ResolutionSet::from_action(CollisionAction::Replace);
    let (_committed, outcome) = staged
        .commit(&fault_fs, &resolutions)
        .expect("post-swap staging cleanup failure must not fail commit");

    assert_eq!(outcome.status, OutcomeStatus::Completed);
    assert_eq!(outcome.replaced, 1);
    assert!(
        outcome.failed.is_empty(),
        "Post-swap staging cleanup error must not be recorded as an item commit failure in outcome.failed"
    );
    assert_eq!(fs::read_to_string(dest_dir.join("entry")).unwrap(), "new_file_content");
}


