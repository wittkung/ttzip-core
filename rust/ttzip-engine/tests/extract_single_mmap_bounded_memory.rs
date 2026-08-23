// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

use std::fs;
use tempfile::tempdir;
use ttzip_engine::archive::unified::extract_single::extract_single_entry_memory;
use ttzip_engine::types::{TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod};
use ttzip_engine::zip::create_zip_archive;

#[test]
fn test_extract_single_entry_mmap_bounded() {
    let dir = tempdir().unwrap();
    let zip_path = dir.path().join("test_preview.zip");

    let f1 = dir.path().join("file1.txt");
    let f2 = dir.path().join("file2.txt");
    fs::write(&f1, b"Payload Content 1").unwrap();
    fs::write(&f2, b"Payload Content 2 - Target Entry").unwrap();

    let options = TTZipCreateOptions {
        format: TTZipArchiveFormat::Zip,
        level: TTZipCompressionLevel::Normal,
        encryption: TTZipEncryptionMethod::None,
        password: std::ptr::null(),
        thread_budget: 2,
        solid_block_size_mb: 0,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };

    create_zip_archive(
        &zip_path,
        &[f1.clone(), f2.clone()],
        &options,
    )
    .expect("Failed to create zip archive");

    // 2. Extract single entry using fast path
    let extracted = extract_single_entry_memory(
        &zip_path,
        Some("file2.txt"),
        -1,
        None,
    )
    .expect("Single entry extraction failed");

    assert_eq!(extracted, b"Payload Content 2 - Target Entry");

    // 3. Extract by index
    let extracted_idx = extract_single_entry_memory(
        &zip_path,
        None,
        0,
        None,
    )
    .expect("Single entry extraction by index failed");

    assert_eq!(extracted_idx, b"Payload Content 1");
}
