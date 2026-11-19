// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use super::*;
use crate::types::TTZipEncryptionMethod;
use crate::zip::writer::{assemble_zip_archive, compress_items_parallel, ZipInputItem};

fn build_test_zip(rel_path: &str, data: Vec<u8>, level: i32) -> Vec<u8> {
    let items = vec![ZipInputItem {
        rel_path: rel_path.to_string(),
        data,
        mtime_epoch_secs: 1700000000,
        mode: 0o644,
        is_directory: false,
    }];
    let compressed = compress_items_parallel(items, level, TTZipEncryptionMethod::None, None, 1)
        .expect("compression failed");
    assemble_zip_archive(&compressed).expect("assembly failed")
}

#[test]
fn test_extract_entry_to_sink_streaming() {
    let payload = b"Streaming extraction test payload with micro-buffer quota".repeat(100);
    let zip_bytes = build_test_zip("stream_test.txt", payload.clone(), 6);
    let archive = ZipArchive::open_slice(&zip_bytes).expect("open zip slice");
    assert_eq!(archive.entries().len(), 1);
    let mut sink = Vec::new();
    let written = archive
        .extract_entry_to_sink(0, None, &mut sink)
        .expect("extract to sink");
    assert_eq!(written, payload.len() as u64);
    assert_eq!(sink, payload);
}

#[test]
fn test_extract_entry_bytes_clamps_at_64mb() {
    let zip_bytes = build_test_zip("mock.txt", b"small payload".to_vec(), 0);
    let mut archive = ZipArchive::open_slice(&zip_bytes).expect("open zip slice");
    archive.entries[0].uncompressed_size = (MAX_IN_MEMORY_DECOMPRESS_BYTES as u64) + 1;
    let res = archive.extract_entry_bytes(0, None);
    assert_eq!(res, Err(TTZipStatus::ErrOutOfMemory));
}

#[test]
fn test_extract_entry_to_sink_circuit_breaker() {
    let zip_bytes = build_test_zip("bomb.txt", vec![b'A'; 10 * 1024], 6);
    let mut archive = ZipArchive::open_slice(&zip_bytes).expect("open zip slice");
    archive.entries[0].uncompressed_size = 10;
    let mut sink = Vec::new();
    let res = archive.extract_entry_to_sink(0, None, &mut sink);
    assert_eq!(res, Err(TTZipStatus::ErrSecurityViolation));
}

#[test]
fn test_zip_intermediate_symlink_traversal_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("extract_root");
    fs::create_dir_all(&dest).unwrap();
    let outside = tmp.path().join("outside_target");
    fs::create_dir_all(&outside).unwrap();
    let symlink_dir = dest.join("symdir");
    std::os::unix::fs::symlink(&outside, &symlink_dir).unwrap();
    let zip_bytes = build_test_zip("symdir/evil.txt", b"evil content".to_vec(), 0);
    let archive = ZipArchive::open_slice(&zip_bytes).expect("open zip slice");
    let opts = TTZipExtractOptions::default();
    let res = archive.extract_all(&dest, &opts);
    assert_eq!(res.err(), Some(TTZipStatus::ErrSecurityViolation));
    assert!(!outside.join("evil.txt").exists());
}

#[test]
fn test_extract_entry_to_sink_cancellation_probe() {
    let zip_bytes = build_test_zip("large.bin", vec![0x42u8; 256 * 1024], 1);
    let archive = ZipArchive::open_slice(&zip_bytes).expect("open zip slice");
    let sheet = ProgressAtomicSheet::new();
    sheet.cancel();
    let mut sink = Vec::new();
    let res = archive.extract_entry_to_sink_with_sheet(0, None, &mut sink, Some(&sheet));
    assert_eq!(res.err(), Some(TTZipStatus::Cancelled));
}

#[test]
fn test_extract_all_with_sheet_progress_and_cancel() {
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("extract_cancel");
    fs::create_dir_all(&dest).unwrap();
    let zip_bytes = build_test_zip("item.bin", vec![0x99u8; 128 * 1024], 1);
    let archive = ZipArchive::open_slice(&zip_bytes).expect("open zip slice");
    let sheet = ProgressAtomicSheet::new();
    let opts = TTZipExtractOptions::default();
    let report = archive
        .extract_all_with_sheet(&dest, &opts, Some(&sheet))
        .unwrap();
    assert_eq!(report.processed_entries_count, 1);
    let snap = sheet.read_snapshot();
    assert!(snap.is_finished);
    assert_eq!(snap.processed_bytes, 128 * 1024);
    let cancel_sheet = ProgressAtomicSheet::new();
    cancel_sheet.cancel();
    let cancel_dest = tmp.path().join("dest2");
    let res = archive.extract_all_with_sheet(&cancel_dest, &opts, Some(&cancel_sheet));
    assert_eq!(res.err(), Some(TTZipStatus::Cancelled));
}

#[test]
fn test_parallel_extract_progress_callback_and_sheet_are_monotonic() {
    use std::sync::Mutex as StdMutex;
    static OBSERVED_DONE: StdMutex<Vec<u64>> = StdMutex::new(Vec::new());
    unsafe extern "C" fn record_progress_cb(
        done: u64,
        _total: u64,
        _file: *const std::os::raw::c_char,
        _user_data: *mut std::os::raw::c_void,
    ) -> bool {
        OBSERVED_DONE.lock().unwrap().push(done);
        true
    }
    OBSERVED_DONE.lock().unwrap().clear();
    let tmp = tempfile::tempdir().unwrap();
    let items: Vec<_> = (0..16)
        .map(|i| ZipInputItem {
            rel_path: format!("f_{i:02}.bin"),
            data: vec![(i as u8).wrapping_mul(17); 16 * 1024],
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        })
        .collect();
    let comp = compress_items_parallel(items, 1, TTZipEncryptionMethod::None, None, 4).unwrap();
    let zip_bytes = assemble_zip_archive(&comp).unwrap();
    let archive = ZipArchive::open_slice(&zip_bytes).unwrap();
    let dest = tmp.path().join("out");
    let sheet = ProgressAtomicSheet::new();
    let opts = TTZipExtractOptions {
        thread_budget: 4,
        progress_callback: Some(record_progress_cb),
        ..Default::default()
    };
    archive
        .extract_all_with_sheet(&dest, &opts, Some(&sheet))
        .unwrap();
    let recorded = OBSERVED_DONE.lock().unwrap().clone();
    assert_eq!(recorded.len(), 16);
    assert!(recorded.windows(2).all(|w| w[0] <= w[1]));
    assert_eq!(*recorded.last().unwrap(), 16 * 16 * 1024);
    assert_eq!(sheet.read_snapshot().processed_bytes, 16 * 16 * 1024);
}
