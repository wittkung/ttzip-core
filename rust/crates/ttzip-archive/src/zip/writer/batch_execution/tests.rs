// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use super::execution_probe::{Probe, Registration, ReleaseOnDrop};
use super::*;
use crate::types::{TTZipCompressionLevel, TTZipCreateOptions};
use crate::zip::reader::ZipArchive;
use crate::zip::writer::create_zip_streaming_parallel_explicit_with_sheet;
use std::fs;
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::thread::ThreadId;
use std::time::Duration;

struct Fixture {
    _temp: tempfile::TempDir,
    entries: Vec<(PathBuf, String)>,
    output: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let mut entries = Vec::new();
        for i in 0..6 {
            let path = temp.path().join(format!("item-{i}"));
            fs::write(&path, vec![i as u8; 8192]).unwrap();
            entries.push((path, format!("item-{i}")));
        }
        let dir = temp.path().join("directory");
        fs::create_dir(&dir).unwrap();
        entries.push((dir, "directory/".to_owned()));
        let empty = temp.path().join("empty");
        fs::write(&empty, []).unwrap();
        entries.push((empty, "empty".to_owned()));
        Self {
            output: temp.path().join("output.zip"),
            _temp: temp,
            entries,
        }
    }

    fn verify_archive(&self) {
        let bytes = fs::read(&self.output).unwrap();
        let archive = ZipArchive::open_slice(&bytes).unwrap();
        let names: Vec<_> = archive
            .entries()
            .iter()
            .map(|entry| entry.rel_path.as_str())
            .collect();
        let expected: Vec<_> = self.entries.iter().map(|(_, name)| name.as_str()).collect();
        assert_eq!(names, expected);
        for (index, (path, name)) in self.entries.iter().enumerate() {
            if !name.ends_with('/') {
                assert_eq!(
                    archive.extract_entry_bytes(index, None).unwrap(),
                    fs::read(path).unwrap()
                );
            }
        }
    }
}

struct WriterReceipt {
    caller: ThreadId,
    result: Result<(), TTZipStatus>,
    fixture: Fixture,
}

fn run_writer(budget: u32, cancel: bool, remove_file: bool) -> (WriterReceipt, Arc<Probe>) {
    let fixture = Fixture::new();
    let removed_path = fixture.entries[0].0.clone();
    let sheet = Arc::new(ProgressAtomicSheet::new());
    let (probe, entered) = Probe::new();
    let _release = ReleaseOnDrop(probe.clone());
    let worker_probe = probe.clone();
    let worker_sheet = sheet.clone();
    let (done, receipt) = mpsc::channel();
    let _worker = std::thread::spawn(move || {
        let _registration = Registration::install(worker_probe);
        let caller = std::thread::current().id();
        let options = TTZipCreateOptions {
            level: TTZipCompressionLevel::Normal,
            thread_budget: budget,
            ..Default::default()
        };
        let result = create_zip_streaming_parallel_explicit_with_sheet(
            &fixture.output,
            &fixture.entries,
            &options,
            Some(&worker_sheet),
        )
        .map(|_| ());
        let _ = done.send(WriterReceipt {
            caller,
            result,
            fixture,
        });
    });
    let expected_entries = resolve_workers(budget, 8);
    let received: Vec<_> = (0..expected_entries)
        .map(|_| entered.recv_timeout(Duration::from_secs(2)))
        .collect();
    let removal = if remove_file {
        fs::remove_file(removed_path)
    } else {
        Ok(())
    };
    if cancel {
        sheet.cancel();
    }
    probe.release_all();
    let result = receipt.recv_timeout(Duration::from_secs(5));
    assert!(
        received.iter().all(Result::is_ok),
        "missing compression entry receipt"
    );
    assert!(removal.is_ok(), "failed to prepare the real I/O error");
    let result = result.expect("missing writer completion receipt after release");
    let stats = probe.stats.lock().unwrap();
    assert!(!stats.timed_out, "worker gate deadline was reached");
    assert_eq!(stats.active, 0, "completion preceded worker cleanup");
    drop(stats);
    (result, probe)
}

#[test]
fn explicit_one_runs_the_real_mixed_writer_on_its_caller() {
    let (receipt, probe) = run_writer(1, false, false);
    assert_eq!(receipt.result, Ok(()));
    let stats = probe.stats.lock().unwrap();
    assert_eq!(stats.peak, 1);
    assert_eq!(stats.threads.len(), 1);
    assert!(stats.threads.contains(&receipt.caller));
    receipt.fixture.verify_archive();
}

#[test]
fn explicit_two_bounds_real_workers_and_preserves_archive_order() {
    let (receipt, probe) = run_writer(2, false, false);
    assert_eq!(receipt.result, Ok(()));
    let stats = probe.stats.lock().unwrap();
    assert_eq!(stats.peak, 2);
    assert_eq!(stats.threads.len(), 2);
    assert!(!stats.threads.contains(&receipt.caller));
    receipt.fixture.verify_archive();
}

#[test]
fn cancellation_after_both_workers_enter_releases_and_joins_every_worker() {
    let (receipt, probe) = run_writer(2, true, false);
    assert_eq!(receipt.result, Err(TTZipStatus::Cancelled));
    assert_eq!(probe.stats.lock().unwrap().peak, 2);
    assert!(
        !receipt.fixture.output.exists(),
        "cancelled writer published an archive"
    );
}

#[test]
fn real_file_open_error_does_not_leave_the_other_worker_waiting() {
    let (receipt, probe) = run_writer(2, false, true);
    assert_eq!(receipt.result, Err(TTZipStatus::ErrFileNotFound));
    assert_eq!(probe.stats.lock().unwrap().peak, 2);
    assert!(
        !receipt.fixture.output.exists(),
        "failed writer published an archive"
    );
}

fn options<'a>(budget: u32) -> BatchCompressionOptions<'a> {
    BatchCompressionOptions {
        level: 6,
        format: TTZipArchiveFormat::Zip,
        encryption: TTZipEncryptionMethod::None,
        password: None,
        sheet: None,
        thread_budget: budget,
    }
}

#[test]
fn empty_batch_does_not_enter_compression() {
    let (probe, entered) = Probe::new();
    let _registration = Registration::install(probe);
    assert!(execute_batch_parallel_with_options(Vec::new(), options(2)).is_empty());
    assert!(matches!(entered.try_recv(), Err(mpsc::TryRecvError::Empty)));
}

#[test]
fn one_real_entry_with_large_budget_still_uses_the_caller() {
    let fixture = Fixture::new();
    let items =
        super::super::parallel_batch::build_explicit_plan_items(&fixture.entries[..1]).unwrap();
    let (probe, _entered) = Probe::new();
    probe.release_all();
    let _registration = Registration::install(probe.clone());
    let result = execute_batch_parallel_with_options(items, options(64));
    assert_eq!(result.len(), 1);
    assert!(result[0].is_ok());
    let stats = probe.stats.lock().unwrap();
    assert_eq!(stats.threads.len(), 1);
    assert!(stats.threads.contains(&std::thread::current().id()));
}

#[test]
fn zero_retains_automatic_budget_and_explicit_requests_clamp_like_other_writers() {
    assert_eq!(
        resolve_workers(0, 256),
        ttzip_types::recommended_compute_threads().clamp(1, 256)
    );
    assert_eq!(resolve_workers(1, 256), 1);
    assert_eq!(resolve_workers(2, 256), 2);
    assert_eq!(resolve_workers(u32::MAX, 256), 64);
}

#[test]
fn batch_errors_keep_their_original_positions_among_successes() {
    let fixture = Fixture::new();
    let items = super::super::parallel_batch::build_explicit_plan_items(&fixture.entries).unwrap();
    fs::remove_file(&fixture.entries[1].0).unwrap();
    let results = execute_batch_parallel_with_options(items, options(2));
    assert_eq!(results.len(), fixture.entries.len());
    assert!(results[0].is_ok());
    assert_eq!(
        results[1].as_ref().err(),
        Some(&TTZipStatus::ErrFileNotFound)
    );
    assert!(results.iter().skip(2).all(Result::is_ok));
}

#[test]
fn zero_budget_preserves_automatic_execution_in_the_real_writer() {
    let (receipt, probe) = run_writer(0, false, false);
    assert_eq!(receipt.result, Ok(()));
    let expected = ttzip_types::recommended_compute_threads().clamp(1, 8);
    let stats = probe.stats.lock().unwrap();
    assert_eq!(stats.threads.len(), expected);
    assert_eq!(stats.peak, expected);
    receipt.fixture.verify_archive();
}

#[test]
fn already_cancelled_batch_does_not_enter_compression() {
    let fixture = Fixture::new();
    let items = super::super::parallel_batch::build_explicit_plan_items(&fixture.entries).unwrap();
    let sheet = ProgressAtomicSheet::new();
    sheet.cancel();
    let (probe, entered) = Probe::new();
    let _registration = Registration::install(probe);
    let mut context = options(2);
    context.sheet = Some(&sheet);
    let results = execute_batch_parallel_with_options(items, context);
    assert_eq!(results.len(), fixture.entries.len());
    assert!(results
        .iter()
        .all(|result| matches!(result, Err(TTZipStatus::Cancelled))));
    assert!(matches!(entered.try_recv(), Err(mpsc::TryRecvError::Empty)));
}
