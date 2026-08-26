// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit test suite for universal in-place archive editing engine.

use super::*;
use crate::archive::in_place_edit::tar::write_tar_entry_from_file;
use crate::archive::tar::reader::TarArchive;
use crate::codecs::deflate::gzip_compress;
use crate::sevenz::{create_7z_solid_archive_bytes, SevenZArchive};
use crate::zip::reader::ZipArchive;
use crate::zip::writer::{assemble_zip_archive, ZipInputItem};
use std::fs;

#[test]
fn test_in_place_zip_append_replace_delete_transaction() {
    let temp_dir = std::env::temp_dir().join(format!("ttzip_test_inplace_zip_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);

    let archive_path = temp_dir.join("test_inplace.zip");
    let initial_items = vec![
        ZipInputItem { rel_path: "file1.txt".to_string(), data: b"Original Content 1".to_vec(), mtime_epoch_secs: 1700000000, mode: 0o644, is_directory: false },
        ZipInputItem { rel_path: "file2.txt".to_string(), data: b"Original Content 2".to_vec(), mtime_epoch_secs: 1700000000, mode: 0o644, is_directory: false },
        ZipInputItem { rel_path: "file3.txt".to_string(), data: b"Original Content 3".to_vec(), mtime_epoch_secs: 1700000000, mode: 0o644, is_directory: false },
    ];
    let compressed = crate::zip::writer::compress_items_parallel(initial_items, 6, crate::types::TTZipEncryptionMethod::None, None, 2).unwrap();
    let zip_bytes = assemble_zip_archive(&compressed).unwrap();
    fs::write(&archive_path, zip_bytes).unwrap();

    let f_rep = temp_dir.join("replaced2.txt");
    let f_app = temp_dir.join("appended4.txt");
    fs::write(&f_rep, b"UPDATED CONTENT 2").unwrap();
    fs::write(&f_app, b"NEW CONTENT 4").unwrap();

    let mut session = InPlaceArchiveSession::begin(&archive_path, Some(TTZipArchiveFormat::Zip)).unwrap();
    session.replace("file2.txt", &f_rep).unwrap();
    session.delete("file1.txt").unwrap();
    session.append("file4.txt", &f_app).unwrap();
    session.commit().unwrap();

    let mapped = fs::read(&archive_path).unwrap();
    let zip = ZipArchive::open_slice(&mapped).unwrap();
    let paths: Vec<String> = zip.entries().iter().map(|e| e.rel_path.clone()).collect();
    assert!(!paths.contains(&"file1.txt".to_string()));
    assert!(paths.contains(&"file2.txt".to_string()));
    assert!(paths.contains(&"file3.txt".to_string()));
    assert!(paths.contains(&"file4.txt".to_string()));

    let idx2 = zip.entries().iter().position(|e| e.rel_path == "file2.txt").unwrap();
    assert_eq!(zip.extract_entry_bytes(idx2, None).unwrap(), b"UPDATED CONTENT 2");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_in_place_zip_rollback_on_cancel() {
    let temp_dir = std::env::temp_dir().join(format!("ttzip_test_rollback_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);

    let archive_path = temp_dir.join("test_rollback.zip");
    let initial_items = vec![
        ZipInputItem { rel_path: "keep.txt".to_string(), data: b"Keep me unchanged".to_vec(), mtime_epoch_secs: 1700000000, mode: 0o644, is_directory: false },
    ];
    let compressed = crate::zip::writer::compress_items_parallel(initial_items, 6, crate::types::TTZipEncryptionMethod::None, None, 2).unwrap();
    let zip_bytes = assemble_zip_archive(&compressed).unwrap();
    fs::write(&archive_path, &zip_bytes).unwrap();

    let f_junk = temp_dir.join("junk.txt");
    fs::write(&f_junk, b"JUNK").unwrap();

    let mut session = InPlaceArchiveSession::begin(&archive_path, Some(TTZipArchiveFormat::Zip)).unwrap();
    session.replace("keep.txt", &f_junk).unwrap();
    session.cancel().unwrap();

    let mapped = fs::read(&archive_path).unwrap();
    let zip = ZipArchive::open_slice(&mapped).unwrap();
    let data = zip.extract_entry_bytes(0, None).unwrap();
    assert_eq!(data, b"Keep me unchanged");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_in_place_7z_append_replace_delete() {
    let temp_dir = std::env::temp_dir().join(format!("ttzip_test_inplace_7z_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);

    let archive_path = temp_dir.join("test.7z");
    let initial_items = vec![
        ZipInputItem { rel_path: "doc1.txt".to_string(), data: b"Doc 1".to_vec(), mtime_epoch_secs: 1700000000, mode: 0o644, is_directory: false },
        ZipInputItem { rel_path: "doc2.txt".to_string(), data: b"Doc 2".to_vec(), mtime_epoch_secs: 1700000000, mode: 0o644, is_directory: false },
    ];
    let bytes = create_7z_solid_archive_bytes(&initial_items, 3, 2).unwrap();
    fs::write(&archive_path, bytes).unwrap();

    let f_rep = temp_dir.join("rep.txt");
    let f_app = temp_dir.join("app.txt");
    fs::write(&f_rep, b"Replaced Doc 2").unwrap();
    fs::write(&f_app, b"Appended Doc 3").unwrap();

    let mut session = InPlaceArchiveSession::begin(&archive_path, Some(TTZipArchiveFormat::SevenZip)).unwrap();
    session.delete("doc1.txt").unwrap();
    session.replace("doc2.txt", &f_rep).unwrap();
    session.append("doc3.txt", &f_app).unwrap();
    session.commit().unwrap();

    let mapped = fs::read(&archive_path).unwrap();
    let archive = SevenZArchive::open_slice(&mapped).unwrap();
    assert_eq!(archive.len(), 2);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_in_place_tar_append_replace_delete() {
    let temp_dir = std::env::temp_dir().join(format!("ttzip_test_inplace_tar_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);

    let archive_path = temp_dir.join("test.tar");
    let mut initial_tar = Vec::new();
    let f1 = temp_dir.join("a.txt");
    let f2 = temp_dir.join("b.txt");
    fs::write(&f1, b"File A Original Content").unwrap();
    fs::write(&f2, b"File B Original Content").unwrap();

    write_tar_entry_from_file(&mut initial_tar, "a.txt", &f1).unwrap();
    write_tar_entry_from_file(&mut initial_tar, "b.txt", &f2).unwrap();
    initial_tar.extend_from_slice(&[0u8; 1024]);
    fs::write(&archive_path, &initial_tar).unwrap();

    let f_b_new = temp_dir.join("b_new.txt");
    let f_c = temp_dir.join("c.txt");
    fs::write(&f_b_new, b"File B Replaced").unwrap();
    fs::write(&f_c, b"File C Appended").unwrap();

    let mut session = InPlaceArchiveSession::begin(&archive_path, Some(TTZipArchiveFormat::Tar)).unwrap();
    session.delete("a.txt").unwrap();
    session.replace("b.txt", &f_b_new).unwrap();
    session.append("c.txt", &f_c).unwrap();
    session.commit().unwrap();

    let mapped = fs::read(&archive_path).unwrap();
    let archive = TarArchive::open_slice(&mapped).unwrap();
    assert_eq!(archive.len(), 2);
    assert_eq!(archive.entries()[0].path, "b.txt");
    assert_eq!(archive.extract_entry_bytes(0).unwrap(), b"File B Replaced");
    assert_eq!(archive.entries()[1].path, "c.txt");
    assert_eq!(archive.extract_entry_bytes(1).unwrap(), b"File C Appended");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_in_place_single_stream_and_wal() {
    let temp_dir = std::env::temp_dir().join(format!("ttzip_test_inplace_single_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);

    let gz_path = temp_dir.join("data.txt.gz");
    let mut gz_buf = vec![0u8; 1024];
    let c_len = gzip_compress(b"Initial single stream text", &mut gz_buf, 6).unwrap();
    gz_buf.truncate(c_len);
    fs::write(&gz_path, &gz_buf).unwrap();

    let new_src = temp_dir.join("new_data.txt");
    fs::write(&new_src, b"Updated Single Stream Data Content").unwrap();

    let mut session = InPlaceArchiveSession::begin(&gz_path, None).unwrap();
    session.replace("data.txt", &new_src).unwrap();
    session.commit().unwrap();

    let read_gz = fs::read(&gz_path).unwrap();
    let mut decomp = vec![0u8; 1024];
    let d_len = crate::codecs::deflate::gzip_decompress(&read_gz, &mut decomp).unwrap();
    assert_eq!(&decomp[..d_len], b"Updated Single Stream Data Content");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_piece_tree_interval_remapping_assembly() {
    use crate::archive::wal_mutation::{PieceSource, PieceTree};

    let mut pt = PieceTree::new(100);
    assert_eq!(pt.total_length(), 100);
    assert_eq!(pt.pieces.len(), 1);

    // Replace range [20..30] (length 10) with WAL payload of length 15
    pt.replace_range(20, 10, PieceSource::WalPayload { wal_offset: 0, len: 15 }, 15);
    assert_eq!(pt.total_length(), 105);
    assert_eq!(pt.pieces.len(), 3);

    // Test assembling to file
    let temp_dir = std::env::temp_dir().join(format!("ttzip_test_pt_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);
    let orig_p = temp_dir.join("orig.bin");
    let wal_p = temp_dir.join("wal.bin");
    let out_p = temp_dir.join("out.bin");

    let orig_data = vec![0xAAu8; 100];
    let wal_data = vec![0xBBu8; 15];
    fs::write(&orig_p, &orig_data).unwrap();
    fs::write(&wal_p, &wal_data).unwrap();

    let mut orig_f = fs::File::open(&orig_p).unwrap();
    let mut wal_f = fs::File::open(&wal_p).unwrap();
    let mut out_f = fs::File::create(&out_p).unwrap();

    let assembled = pt.assemble_to(&mut orig_f, &mut wal_f, &mut out_f).unwrap();
    assert_eq!(assembled, 105);

    let result = fs::read(&out_p).unwrap();
    assert_eq!(result.len(), 105);
    assert_eq!(&result[0..20], &vec![0xAAu8; 20][..]);
    assert_eq!(&result[20..35], &vec![0xBBu8; 15][..]);
    assert_eq!(&result[35..105], &vec![0xAAu8; 70][..]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_wal_mutation_apfs_atomic_commit_and_crash_rollback() {
    use crate::archive::wal_mutation::{append_wal_mutation, commit_wal_to_archive, rollback_wal_mutation, inspect_wal_status};

    let temp_dir = std::env::temp_dir().join(format!("ttzip_test_wal_apfs_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);
    let archive_p = temp_dir.join("data_archive.bin");

    // Write initial 1MB payload
    let initial_data = vec![1u8; 1024 * 1024];
    fs::write(&archive_p, &initial_data).unwrap();

    // Stage WAL delta
    let summary = append_wal_mutation(&archive_p, "inner/file.bin", 512, 1024, b"MODIFIED_INCREMENTAL_DELTA").unwrap();
    assert!(summary.is_staged);
    assert_eq!(summary.delta_bytes, 26);

    let status = inspect_wal_status(&archive_p).unwrap();
    assert!(status.is_some());

    // Commit WAL
    let commit_res = commit_wal_to_archive(&archive_p).unwrap();
    assert!(commit_res.success);

    // Verify WAL is removed and committed file content has delta
    let updated = fs::read(&archive_p).unwrap();
    assert_eq!(&updated[512..512 + 26], b"MODIFIED_INCREMENTAL_DELTA");
    assert!(inspect_wal_status(&archive_p).unwrap().is_none());

    // Test rollback behavior
    let _ = append_wal_mutation(&archive_p, "inner/file.bin", 100, 50, b"ABORT_DELTA").unwrap();
    let cleaned = rollback_wal_mutation(&archive_p).unwrap();
    assert!(cleaned);
    assert!(inspect_wal_status(&archive_p).unwrap().is_none());

    let _ = fs::remove_dir_all(&temp_dir);
}
