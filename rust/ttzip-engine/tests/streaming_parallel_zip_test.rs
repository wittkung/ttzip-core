// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use std::fs;
use tempfile::tempdir;
use ttzip_engine::types::{TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod};
use ttzip_engine::zip::writer::streaming_parallel::create_zip_streaming_parallel;
use ttzip_engine::zip::reader::ZipArchive;

#[test]
fn test_streaming_parallel_zip_creation_and_inspection() {
    let dir = tempdir().unwrap();
    let src_dir = dir.path().join("source_tree");
    fs::create_dir_all(src_dir.join("subfolder")).unwrap();

    let f1 = src_dir.join("file1.bin");
    let f2 = src_dir.join("subfolder").join("file2.txt");
    fs::write(&f1, vec![0xAB; 100_000]).unwrap();
    fs::write(&f2, b"Hello Parallel Streaming Zip").unwrap();

    let zip_dest = dir.path().join("output.zip");

    let options = TTZipCreateOptions {
        struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
        abi_version: ttzip_engine::types::TTZIP_ABI_VERSION_2,
        format: TTZipArchiveFormat::Zip,
        level: TTZipCompressionLevel::Fast,
        encryption: TTZipEncryptionMethod::None,
        password: std::ptr::null(),
        thread_budget: 4,
        solid_block_size_mb: 0,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };

    let report = create_zip_streaming_parallel(
        &zip_dest,
        &[src_dir],
        &options,
    )
    .expect("Streaming parallel ZIP creation failed");

    assert!(report.total_entries >= 2);
    assert!(report.total_compressed_bytes > 0);
    assert!(zip_dest.exists());

    // Verify created archive with ZipArchive reader
    let mapped = fs::read(&zip_dest).unwrap();
    let archive = ZipArchive::open_slice(&mapped).expect("Failed to open generated ZIP");
    assert!(archive.entries().len() >= 2);

    let (idx2, _entry2) = archive
        .entries()
        .iter()
        .enumerate()
        .find(|(_, e)| e.rel_path.ends_with("file2.txt"))
        .expect("file2 not found");
    let extracted2 = archive.extract_entry_bytes(idx2, None).expect("failed to extract file2");
    assert_eq!(extracted2, b"Hello Parallel Streaming Zip");
}
