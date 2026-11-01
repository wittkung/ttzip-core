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

#[test]
fn test_u1_ac01_ac02_ac03_ac04_sevenz_close_eio_and_eintr_unconfirmed_release() {
    use std::os::fd::AsRawFd;
    use ttzip_archive::archive::unified::create::create_archive_detailed_outcome_with_cancel;
    use ttzip_archive::archive::unified::create_staging::CreateStageCommitError;
    use ttzip_archive::archive::unified::create_transaction::{
        ArchiveCleanupDisposition, DescriptorReleaseStatus, ResidualPathRole,
        set_create_txn_close_fault_hook,
    };
    use ttzip_archive::types::{
        TTZIP_ABI_VERSION_3, TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions,
        TTZipEncryptionMethod, TTZipStatus,
    };

    let tmp = tempdir().expect("tempdir");
    let src_file = tmp.path().join("input.txt");
    fs::write(&src_file, vec![b'7'; 8192]).unwrap();
    let base_opts = TTZipCreateOptions {
        struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
        abi_version: TTZIP_ABI_VERSION_3,
        format: TTZipArchiveFormat::SevenZip,
        level: TTZipCompressionLevel::Fast,
        encryption: TTZipEncryptionMethod::None,
        password: std::ptr::null(),
        thread_budget: 1,
        solid_block_size_mb: 16,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };

    // Case 1 (AC-U1-01): encoder succeeds, close(2) returns EIO (ConfirmedReleased), unlink succeeds -> StatusWithFinishDiagnostics(ErrCompressionFailed)
    let out_eio = tmp.path().join("close_eio.7z");
    set_create_txn_close_fault_hook(|owned_fd, _scope| {
        let raw = owned_fd.as_raw_fd();
        drop(owned_fd);
        (
            raw,
            Err(std::io::Error::from_raw_os_error(libc::EIO)),
            DescriptorReleaseStatus::ConfirmedReleased,
        )
    });
    let res_eio = create_archive_detailed_outcome_with_cancel(
        std::slice::from_ref(&src_file),
        &out_eio,
        &base_opts,
        0,
        || false,
    );
    match res_eio {
        Err(CreateStageCommitError::StatusWithFinishDiagnostics {
            status,
            finish_diagnostics,
        }) => {
            assert_eq!(status, TTZipStatus::ErrCompressionFailed);
            assert_eq!(finish_diagnostics.len(), 1);
            assert_eq!(finish_diagnostics[0].close_os_error, Some(libc::EIO));
            assert_eq!(
                finish_diagnostics[0].release_status,
                DescriptorReleaseStatus::ConfirmedReleased
            );
        }
        other => panic!("Expected StatusWithFinishDiagnostics on close EIO, got: {other:?}"),
    }
    assert!(!out_eio.exists(), "Close EIO must never commit destination archive");

    // Case 2 (AC-U1-02 / AC-U1-03): primary cancel + secondary close EIO (ConfirmedReleased), clean unlink -> StatusWithFinishDiagnostics(Cancelled)
    unsafe extern "C" fn cancel_after_open_cb(
        processed: u64,
        _total: u64,
        _entry: *const libc::c_char,
        _user_data: *mut libc::c_void,
    ) -> bool {
        processed == 0
    }
    let mut cancel_opts = base_opts;
    cancel_opts.progress_callback = Some(cancel_after_open_cb);
    let out_cancel_eio = tmp.path().join("cancel_close_eio.7z");
    set_create_txn_close_fault_hook(|owned_fd, _scope| {
        let raw = owned_fd.as_raw_fd();
        drop(owned_fd);
        (
            raw,
            Err(std::io::Error::from_raw_os_error(libc::EIO)),
            DescriptorReleaseStatus::ConfirmedReleased,
        )
    });
    let res_cancel_eio = create_archive_detailed_outcome_with_cancel(
        std::slice::from_ref(&src_file),
        &out_cancel_eio,
        &cancel_opts,
        0,
        || true,
    );
    match res_cancel_eio {
        Err(CreateStageCommitError::StatusWithFinishDiagnostics {
            status,
            finish_diagnostics,
        }) => {
            assert_eq!(status, TTZipStatus::Cancelled);
            assert_eq!(finish_diagnostics.len(), 1);
            assert_eq!(finish_diagnostics[0].close_os_error, Some(libc::EIO));
        }
        other => panic!("Expected StatusWithFinishDiagnostics(Cancelled), got: {other:?}"),
    }

    // Case 3 (AC-U1-04): close(2) returns EINTR (ReleaseUnconfirmed) -> TransactionIncomplete with stage_dir_preserved = true
    let out_eintr = tmp.path().join("close_eintr.7z");
    set_create_txn_close_fault_hook(|owned_fd, _scope| {
        let raw = owned_fd.as_raw_fd();
        drop(owned_fd);
        (
            raw,
            Err(std::io::Error::from_raw_os_error(libc::EINTR)),
            DescriptorReleaseStatus::ReleaseUnconfirmed,
        )
    });
    let res_eintr = create_archive_detailed_outcome_with_cancel(
        std::slice::from_ref(&src_file),
        &out_eintr,
        &base_opts,
        0,
        || false,
    );
    match res_eintr {
        Err(CreateStageCommitError::TransactionIncomplete(report)) => {
            assert_eq!(report.primary_status, TTZipStatus::ErrCompressionFailed);
            assert_eq!(report.unconfirmed_release_count, 1);
            assert!(report.stage_dir_preserved);
            assert!(report.stage_dir.exists());
            assert!(report.cleanup_failures.iter().any(|f| {
                f.role == ResidualPathRole::StagedArtifact
                    && f.disposition == ArchiveCleanupDisposition::DescriptorReleaseUnconfirmed
                    && f.os_error_code == Some(libc::EINTR)
            }));
            assert!(report.finish_diagnostics.iter().any(|d| {
                d.release_status == DescriptorReleaseStatus::ReleaseUnconfirmed
                    && d.close_os_error == Some(libc::EINTR)
            }));
        }
        other => panic!("Expected TransactionIncomplete on EINTR close, got: {other:?}"),
    }
}

#[test]
fn test_u1_ac06_ac07_split_volume_backup_rollback_and_partial_recovery() {
    use ttzip_archive::archive::split::VolumeNamingScheme;
    use ttzip_archive::archive::unified::create_staging::{
        CreateCommitMode, CreateStageCommitError,
        with_staged_archive_destination_detailed_using_outcome,
    };
    use ttzip_archive::archive::unified::create_transaction::CreateBackupRecoveryState;
    use ttzip_archive::types::TTZipStatus;

    let tmp = tempdir().expect("tempdir");

    // Part A (AC-U1-06): split commit fails on 2nd staged volume move, backup restoration succeeds cleanly -> Status(ErrOpenFailed)
    let dir_a = tmp.path().join("split_a");
    fs::create_dir_all(&dir_a).unwrap();
    let target_a = dir_a.join("vol.7z.001");
    fs::write(dir_a.join("vol.7z.001"), b"OLD_PART_1").unwrap();
    fs::write(dir_a.join("vol.7z.002"), b"OLD_PART_2").unwrap();

    let mut stage_moves = 0usize;
    let res_a = with_staged_archive_destination_detailed_using_outcome(
        &target_a,
        CreateCommitMode::SplitVolume(VolumeNamingScheme::NumberedExtension),
        |staged_dest, _txn_ctx| {
            let sdir = staged_dest.parent().unwrap();
            fs::write(sdir.join("vol.7z.001"), b"NEW_PART_1").unwrap();
            fs::write(sdir.join("vol.7z.002"), b"NEW_PART_2").unwrap();
            Ok(())
        },
        move |src, dst| {
            stage_moves += 1;
            if stage_moves == 2 {
                return Err(std::io::Error::from_raw_os_error(libc::EIO));
            }
            fs::rename(src, dst)
        },
        |src, dst| fs::rename(src, dst),
        || false,
    );
    assert!(
        matches!(res_a, Err(CreateStageCommitError::Status(TTZipStatus::ErrOpenFailed))),
        "Clean rollback of all split volumes must return clean Status(ErrOpenFailed), got: {res_a:?}"
    );
    assert_eq!(fs::read(dir_a.join("vol.7z.001")).unwrap(), b"OLD_PART_1");
    assert_eq!(fs::read(dir_a.join("vol.7z.002")).unwrap(), b"OLD_PART_2");

    // Part B (AC-U1-07): split commit fails on 2nd staged move AND 2nd backup restore fails -> RollbackIncompletePreservedInBackup
    let dir_b = tmp.path().join("split_b");
    fs::create_dir_all(&dir_b).unwrap();
    let target_b = dir_b.join("volb.7z.001");
    fs::write(dir_b.join("volb.7z.001"), b"OLD_B_1").unwrap();
    fs::write(dir_b.join("volb.7z.002"), b"OLD_B_2").unwrap();

    let mut stage_moves_b = 0usize;
    let mut restore_moves_b = 0usize;
    let res_b = with_staged_archive_destination_detailed_using_outcome(
        &target_b,
        CreateCommitMode::SplitVolume(VolumeNamingScheme::NumberedExtension),
        |staged_dest, _txn_ctx| {
            let sdir = staged_dest.parent().unwrap();
            fs::write(sdir.join("volb.7z.001"), b"NEW_B_1").unwrap();
            fs::write(sdir.join("volb.7z.002"), b"NEW_B_2").unwrap();
            Ok(())
        },
        move |src, dst| {
            stage_moves_b += 1;
            if stage_moves_b == 2 {
                return Err(std::io::Error::from_raw_os_error(libc::EIO));
            }
            fs::rename(src, dst)
        },
        move |src, dst| {
            restore_moves_b += 1;
            if restore_moves_b == 2 {
                return Err(std::io::Error::from_raw_os_error(libc::EACCES));
            }
            fs::rename(src, dst)
        },
        || false,
    );
    match res_b {
        Err(CreateStageCommitError::TransactionIncomplete(report)) => {
            let rec = report.recovery_attempt.expect("recovery_attempt must be present");
            assert_eq!(
                rec.state,
                CreateBackupRecoveryState::RollbackIncompletePreservedInBackup
            );
            assert_eq!(rec.restored_targets.len(), 1);
            assert_eq!(rec.failed_entries.len(), 1);
            assert!(rec.backup_dir.expect("backup_dir must be Some").exists());
        }
        other => panic!("Expected TransactionIncomplete with RollbackIncompletePreservedInBackup, got: {other:?}"),
    }
}

#[test]
fn test_u1_ac08_ac10_ac11_ac12_ac13_fstat_dup_temp_readback_barrier_and_t_sample() {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use ttzip_archive::archive::unified::create::create_archive_detailed_outcome_with_cancel;
    use ttzip_archive::archive::unified::create_staging::{
        CreateCommitMode, CreateStageCommitError, with_staged_archive_destination_detailed_outcome,
    };
    use ttzip_archive::archive::unified::create_transaction::{
        ArchiveCleanupDisposition, CreateFileAccessMode, CreateTransactionContext,
        DescriptorReleaseStatus, ResidualPathRole, set_abort_guard_dup_fd_hook,
        set_abort_guard_fstat_hook, set_create_txn_close_fault_hook,
    };
    use ttzip_archive::fs::FileAbortGuard;
    use ttzip_archive::types::{
        TTZIP_ABI_VERSION_3, TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions,
        TTZipEncryptionMethod, TTZipStatus,
    };

    let tmp = tempdir().expect("tempdir");

    // 1. AC-U1-08 (F2.1): post-openat fstat failure closes fd and records OwnershipOrLocationUnconfirmed
    let stage_root = tmp.path().join("f21_stage");
    fs::create_dir_all(&stage_root).unwrap();
    let ctx_f21 = CreateTransactionContext::new(stage_root.clone());
    set_abort_guard_fstat_hook(|| Err(std::io::Error::from_raw_os_error(libc::EIO)));
    let fstat_res = FileAbortGuard::create_staged_file_in_txn(
        &stage_root.join("fstat_fail.bin"),
        CreateFileAccessMode::WriteOnly,
        Some(&ctx_f21),
        ResidualPathRole::StagedArtifact,
    );
    assert!(fstat_res.is_err());
    assert!(ctx_f21.should_preserve_outer_stage_dir());

    // 2. AC-U1-08 (F2.1): dup_fd_cloexec failure unlinks created file via quarantine_verify_and_unlink
    let dup_stage = tmp.path().join("dup_stage");
    let (pfd, pid) = {
        fs::create_dir_all(&dup_stage).unwrap();
        let df = fs::File::open(&dup_stage).unwrap();
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        unsafe { libc::fstat(df.as_raw_fd(), &mut st) };
        (df, (st.st_dev as u64, st.st_ino))
    };
    let ctx_dup = CreateTransactionContext::with_captured_parent(
        dup_stage.clone(),
        pfd.as_raw_fd(),
        pid,
    );
    set_abort_guard_dup_fd_hook(|| Err(std::io::Error::from_raw_os_error(libc::EMFILE)));
    let dup_target = dup_stage.join("dup_fail.bin");
    let dup_res = FileAbortGuard::create_staged_file_in_txn(
        &dup_target,
        CreateFileAccessMode::WriteOnly,
        Some(&ctx_dup),
        ResidualPathRole::StagedArtifact,
    );
    assert!(dup_res.is_err());
    assert!(!dup_target.exists(), "dup_fd failure must unlink newly created file");

    // 3. AC-U1-10: ScopedTempFile readback close fault (EINTR / ReleaseUnconfirmed) on XAR creation
    let src_file = tmp.path().join("xar_src.txt");
    fs::write(&src_file, b"XAR_TEMP_READBACK_TEST").unwrap();
    let xar_out = tmp.path().join("out.xar");
    let xar_opts = TTZipCreateOptions {
        struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
        abi_version: TTZIP_ABI_VERSION_3,
        format: TTZipArchiveFormat::Xar,
        level: TTZipCompressionLevel::Normal,
        encryption: TTZipEncryptionMethod::None,
        password: std::ptr::null(),
        thread_budget: 1,
        solid_block_size_mb: 16,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };
    set_create_txn_close_fault_hook(|owned_fd, _scope| {
        let raw = owned_fd.as_raw_fd();
        drop(owned_fd);
        (
            raw,
            Err(std::io::Error::from_raw_os_error(libc::EINTR)),
            DescriptorReleaseStatus::ReleaseUnconfirmed,
        )
    });
    let xar_res = create_archive_detailed_outcome_with_cancel(
        std::slice::from_ref(&src_file),
        &xar_out,
        &xar_opts,
        0,
        || false,
    );
    match xar_res {
        Err(CreateStageCommitError::TransactionIncomplete(report)) => {
            assert_eq!(report.unconfirmed_release_count, 1);
            assert!(report.stage_dir_preserved);
            assert!(report.cleanup_failures.iter().any(|f| {
                f.role == ResidualPathRole::StagedTempFile
                    && f.disposition == ArchiveCleanupDisposition::DescriptorReleaseUnconfirmed
            }));
        }
        other => panic!("Expected TransactionIncomplete on XAR temp readback EINTR, got: {other:?}"),
    }

    // 4. AC-U1-11 & AC-U1-12 & AC-U1-13: Drop fallback observer, active producer barrier, and T_sample linearization
    let barrier_root = tmp.path().join("barrier_root");
    fs::create_dir_all(&barrier_root).unwrap();
    let late_cancel_flag = Arc::new(AtomicBool::new(false));
    let late_flag_hook = Arc::clone(&late_cancel_flag);
    let fin_err = with_staged_archive_destination_detailed_outcome(
        &barrier_root.join("barrier.zip"),
        CreateCommitMode::SingleFile,
        |staged_dest, ctx_bar| {
            let stage_dir_path = staged_dest.parent().unwrap().to_path_buf();
            let (_f_drop, guard_drop) = FileAbortGuard::create_staged_file_in_txn(
                &stage_dir_path.join("drop_fallback.bin"),
                CreateFileAccessMode::WriteOnly,
                Some(ctx_bar),
                ResidualPathRole::StagedArtifact,
            )
            .unwrap();
            fs::set_permissions(&stage_dir_path, fs::Permissions::from_mode(0o500)).unwrap();
            drop(_f_drop);
            drop(guard_drop);
            fs::set_permissions(&stage_dir_path, fs::Permissions::from_mode(0o700)).unwrap();
            assert!(
                ctx_bar.should_preserve_outer_stage_dir(),
                "Drop fallback observer must record cleanup failure into ctx"
            );
            ctx_bar.set_after_final_cancel_sample_before_unlock_hook(move || {
                late_flag_hook.store(true, Ordering::SeqCst);
            });
            Err(TTZipStatus::ErrOpenFailed)
        },
        || late_cancel_flag.load(Ordering::SeqCst),
    );
    match fin_err {
        Err(CreateStageCommitError::TransactionIncomplete(report)) => {
            assert_eq!(report.primary_status, TTZipStatus::ErrOpenFailed);
            assert!(!report.was_primary_cancelled);
            assert!(
                !report.is_cancellation_requested,
                "Cancel flipped after T_sample must not retroactively set is_cancellation_requested"
            );
            assert!(report.stage_dir_preserved);
        }
        other => panic!("Expected TransactionIncomplete, got: {other:?}"),
    }
}
