// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use std::fs;
use tempfile::tempdir;
use ttzip_engine::archive::in_place_edit::InPlaceArchiveSession;
use ttzip_engine::types::{TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod};
use ttzip_engine::zip::create_zip_archive;
use ttzip_engine::zip::reader::ZipArchive;

#[test]
fn test_in_place_zip_mutation_roundtrip() {
    let dir = tempdir().unwrap();
    let zip_path = dir.path().join("inplace_test.zip");

    // 1. Create base archive
    let f1 = dir.path().join("f1.txt");
    let f2 = dir.path().join("f2.txt");
    let f3 = dir.path().join("f3.txt");
    fs::write(&f1, b"Initial Content 1").unwrap();
    fs::write(&f2, b"Initial Content 2").unwrap();
    fs::write(&f3, b"Initial Content 3").unwrap();

    let options = TTZipCreateOptions {
        struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
        abi_version: ttzip_engine::types::TTZIP_ABI_VERSION_2,
        format: TTZipArchiveFormat::Zip,
        level: TTZipCompressionLevel::Normal,
        encryption: TTZipEncryptionMethod::None,
        password: std::ptr::null(),
        thread_budget: 2,
        solid_block_size_mb: 0,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };

    create_zip_archive(&zip_path, &[f1, f2, f3], &options).unwrap();

    // 2. Perform in-place actions: replace f2, delete f1, append f4
    let f2_new = dir.path().join("f2_new.txt");
    let f4 = dir.path().join("f4.txt");
    fs::write(&f2_new, b"Replaced Content 2").unwrap();
    fs::write(&f4, b"Appended Content 4").unwrap();

    let mut session = InPlaceArchiveSession::begin(&zip_path, Some(TTZipArchiveFormat::Zip)).unwrap();
    session.delete("f1.txt").unwrap();
    session.replace("f2.txt", &f2_new).unwrap();
    session.append("f4.txt", &f4).unwrap();
    session.commit().unwrap();

    // 3. Verify modified archive
    let mapped = fs::read(&zip_path).unwrap();
    let archive = ZipArchive::open_slice(&mapped).unwrap();

    let names: Vec<String> = archive.entries().iter().map(|e| e.rel_path.clone()).collect();
    assert!(!names.iter().any(|n| n.ends_with("f1.txt")));
    assert!(names.iter().any(|n| n.ends_with("f2.txt")));
    assert!(names.iter().any(|n| n.ends_with("f3.txt")));
    assert!(names.iter().any(|n| n.ends_with("f4.txt")));

    let (f2_idx, _) = archive.entries().iter().enumerate().find(|(_, e)| e.rel_path.ends_with("f2.txt")).unwrap();
    let f2_data = archive.extract_entry_bytes(f2_idx, None).unwrap();
    assert_eq!(f2_data, b"Replaced Content 2");
}
