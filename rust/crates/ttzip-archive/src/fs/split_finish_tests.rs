// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Split abort regressions using the real close path and thread-local errno injection.

use super::TEST_CLOSE_INJECTION;
use crate::archive::split::{SplitVolumeWriter, VolumeNamingScheme};
use crate::archive::unified::create::{CreateStageCommitError, create_archive_detailed_outcome};
use crate::archive::unified::create_transaction::CreateTransactionContext;
use crate::types::{TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipStatus};
use std::ffi::{c_char, c_void};
use std::fs;
use std::io::Write;
use tempfile::tempdir;

struct CloseFault;

impl CloseFault {
    fn install(errno: i32) -> Self {
        TEST_CLOSE_INJECTION.with(|slot| *slot.borrow_mut() = Some((errno, 0)));
        Self
    }
}

impl Drop for CloseFault {
    fn drop(&mut self) {
        TEST_CLOSE_INJECTION.with(|slot| *slot.borrow_mut() = None);
    }
}

#[test]
fn split_abort_cleans_active_retained_and_rotated_volumes_without_false_error() {
    for status in [TTZipStatus::Cancelled, TTZipStatus::ErrCompressionFailed] {
        for naming in [VolumeNamingScheme::PkzipSpanned, VolumeNamingScheme::NumberedExtension] {
            let dir = tempdir().unwrap();
            let ctx = CreateTransactionContext::new(dir.path().to_path_buf());
            let mut writer = SplitVolumeWriter::new_in_txn(
                dir.path().join("archive.zip"), 4, naming, Some(&ctx),
            ).unwrap();
            writer.write_all(b"fourfourfourfour").unwrap();
            assert_eq!(writer.generated_volumes().len(), 4);
            writer.cancel_and_cleanup_in_txn(status).unwrap();
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
            assert!(ctx.is_clean_for_primary_error(false));
            let report = ctx.freeze_failure_report(status, false, || false);
            assert_eq!(report.primary_status, status);
            assert!(report.cleanup_failures.is_empty());
            assert!(report.finish_diagnostics.is_empty());
            writer.cancel_and_cleanup_in_txn(status).unwrap();
        }
    }
}

#[test]
fn split_abort_reports_real_unlink_identity_failure_and_preserves_replacement() {
    let dir = tempdir().unwrap();
    let ctx = CreateTransactionContext::new(dir.path().to_path_buf());
    let mut writer = SplitVolumeWriter::new_in_txn(
        dir.path().join("archive.zip"), 1024, VolumeNamingScheme::PkzipSpanned, Some(&ctx),
    ).unwrap();
    writer.write_all(b"partial archive").unwrap();
    let volume = writer.generated_volumes()[0].clone();
    let displaced = dir.path().join("displaced");
    fs::rename(&volume, &displaced).unwrap();
    fs::write(&volume, b"external sentinel").unwrap();
    assert!(writer.cancel_and_cleanup_in_txn(TTZipStatus::Cancelled).is_err());
    assert_eq!(fs::read(&volume).unwrap(), b"external sentinel");
    assert_eq!(fs::read(&displaced).unwrap(), b"partial archive");
    let report = ctx.freeze_failure_report(TTZipStatus::Cancelled, true, || true);
    assert!(report.was_primary_cancelled);
    assert!(!report.cleanup_failures.is_empty());
    assert_eq!(report.unconfirmed_release_count, 0);
}

#[test]
fn split_abort_preserves_actual_close_error_and_release_uncertainty() {
    for errno in [libc::EIO, libc::EINTR] {
        let dir = tempdir().unwrap();
        let ctx = CreateTransactionContext::new(dir.path().to_path_buf());
        let mut writer = SplitVolumeWriter::new_in_txn(
            dir.path().join("archive.zip"), 1024, VolumeNamingScheme::PkzipSpanned, Some(&ctx),
        ).unwrap();
        writer.write_all(b"partial").unwrap();
        let fault = CloseFault::install(errno);
        let error = writer.cancel_and_cleanup_in_txn(TTZipStatus::Cancelled).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(errno));
        let calls = TEST_CLOSE_INJECTION.with(|slot| slot.borrow().unwrap().1);
        assert_eq!(calls, 1, "close must never be retried");
        drop(fault);
        let report = ctx.freeze_failure_report(TTZipStatus::Cancelled, true, || true);
        assert!(report.was_primary_cancelled);
        assert_eq!(report.finish_diagnostics.len(), 1);
        assert_eq!(report.finish_diagnostics[0].close_os_error, Some(errno));
        assert_eq!(report.finish_diagnostics[0].primary_status, TTZipStatus::Cancelled);
        let released = cfg!(target_os = "macos") && errno == libc::EIO;
        assert_eq!(report.unconfirmed_release_count, u32::from(!released));
    }
}

unsafe extern "C" fn cancel_progress(_: u64, _: u64, _: *const c_char, _: *mut c_void) -> bool {
    false
}

#[test]
fn split_zip_cancel_returns_clean_primary_and_removes_transaction_directory() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("source.txt");
    let output = dir.path().join("output");
    fs::create_dir(&output).unwrap();
    fs::write(&source, b"ordinary input payload").unwrap();
    let options = TTZipCreateOptions {
        format: TTZipArchiveFormat::Zip,
        level: TTZipCompressionLevel::Store,
        progress_callback: Some(cancel_progress),
        thread_budget: 1,
        ..Default::default()
    };
    let result = create_archive_detailed_outcome(
        std::slice::from_ref(&source), &output.join("archive.zip"), &options, 1024,
    );
    assert!(matches!(result, Err(CreateStageCommitError::Status(TTZipStatus::Cancelled))), "{result:?}");
    assert_eq!(fs::read(source).unwrap(), b"ordinary input payload");
    assert_eq!(fs::read_dir(output).unwrap().count(), 0);
}

#[test]
fn split_guard_parent_descriptors_stay_bounded_across_many_volumes() {
    use std::os::unix::fs::MetadataExt;
    let dir = tempdir().unwrap();
    let parent_id = fs::metadata(dir.path()).unwrap();
    let mut writer = SplitVolumeWriter::new(
        dir.path().join("many.7z"), 4, VolumeNamingScheme::NumberedExtension,
    ).unwrap();
    for n in 1..=256 {
        writer.write_all(b"four").unwrap();
        if [8, 64, 256].contains(&n) {
            let anchors = (0..1024).filter(|fd| {
                super::super::abort_guard::fd_stat_info(*fd).is_ok_and(|(dev, ino, _, _)| {
                    dev == parent_id.dev() && ino == parent_id.ino()
                })
            }).count();
            assert_eq!(anchors, 1, "parent anchor count at {n} volumes");
        }
    }
    writer.cancel_and_cleanup_in_txn(TTZipStatus::Cancelled).unwrap();
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}
