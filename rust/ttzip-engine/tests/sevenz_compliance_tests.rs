// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive 7-Zip Compliance Test Matrix & Multi-Codec Integration Suite.
//!
//! Covers 30+ real fixture compliance tests, edge cases, encryption interception,
//! corruption fuzz/rejection, and full end-to-end roundtrip verification.

use std::fs;
use std::path::PathBuf;
use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::sevenz::{
    create_7z_solid_archive_bytes, parse_7z_metadata, SevenZArchive, SevenZReader,
};
use ttzip_engine::zip::writer::ZipInputItem;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../vendor/sevenz-rust/tests/resources")
        .join(name)
}

fn read_fixture(name: &str) -> Vec<u8> {
    let p = fixture_path(name);
    fs::read(&p).unwrap_or_else(|e| panic!("Failed to read fixture {:?}: {:?}", p, e))
}

// MARK: - 1. Empty & Single/Multiple Entries Matrix

#[test]
fn test_fixture_01_single_empty_file() {
    let data = read_fixture("single_empty_file.7z");
    let reader = SevenZReader::open_slice(&data).expect("open single_empty_file");
    assert_eq!(reader.len(), 1);
    let meta = &reader.files()[0];
    assert_eq!(meta.rel_path, "empty.txt");
    assert!(meta.is_empty_stream);
    let content = reader.extract_entry_bytes_stream(0, None).expect("extract empty file");
    assert!(content.is_empty());
}

#[test]
fn test_fixture_02_two_empty_file() {
    let data = read_fixture("two_empty_file.7z");
    let reader = SevenZReader::open_slice(&data).expect("open two_empty_file");
    assert_eq!(reader.len(), 2);
    for i in 0..2 {
        let content = reader.extract_entry_bytes_stream(i, None).expect("extract empty");
        assert!(content.is_empty());
    }
}

#[test]
fn test_fixture_03_single_file_with_content_lzma() {
    let data = read_fixture("single_file_with_content_lzma.7z");
    let reader = SevenZReader::open_slice(&data).expect("open single_file_with_content_lzma");
    assert_eq!(reader.len(), 1);
    let meta = &reader.files()[0];
    assert_eq!(meta.rel_path, "file.txt");
    let content = reader.extract_entry_bytes_stream(0, None).expect("extract file.txt");
    assert_eq!(content, b"this is a file\n");
}

#[test]
fn test_fixture_04_two_files_with_content_lzma() {
    let data = read_fixture("two_files_with_content_lzma.7z");
    let reader = SevenZReader::open_slice(&data).expect("open two_files_with_content_lzma");
    assert_eq!(reader.len(), 2);
    let f1 = reader.extract_entry_bytes_stream(0, None).expect("extract file1");
    let f2 = reader.extract_entry_bytes_stream(1, None).expect("extract file2");
    assert_eq!(f1, b"file one content\n");
    assert_eq!(f2, b"file two content\n");
}

// MARK: - 2. Solid vs Non-Solid Multi-Streams

#[test]
fn test_fixture_05_solid_archive() {
    let data = read_fixture("solid.7z");
    let reader = SevenZReader::open_slice(&data).expect("open solid.7z");
    assert_eq!(reader.len(), 3);
    let t1 = reader.extract_entry_bytes_stream(1, None).expect("extract test1.txt");
    let t2 = reader.extract_entry_bytes_stream(2, None).expect("extract test2.txt");
    assert_eq!(t1.len(), 3447);
    assert_eq!(t2.len(), 2654);
}

#[test]
fn test_fixture_06_non_solid_archive() {
    let data = read_fixture("non_solid.7z");
    let reader = SevenZReader::open_slice(&data).expect("open non_solid.7z");
    assert_eq!(reader.len(), 3);
    let t1 = reader.extract_entry_bytes_stream(1, None).expect("extract test1.txt");
    let t2 = reader.extract_entry_bytes_stream(2, None).expect("extract test2.txt");
    assert_eq!(t1.len(), 3447);
    assert_eq!(t2.len(), 2654);
}

// MARK: - 3. Multi-Codec & Filter Algorithm Matrix

#[test]
fn test_fixture_07_copy_method() {
    let data = read_fixture("copy.7z");
    let reader = SevenZReader::open_slice(&data).expect("open copy.7z");
    assert_eq!(reader.len(), 1);
    let content = reader.extract_entry_bytes_stream(0, None).expect("extract copy.txt");
    assert_eq!(content, b"simple copy encoding");
}

#[test]
fn test_fixture_08_delta_filter() {
    let data = read_fixture("delta.7z");
    let reader = SevenZReader::open_slice(&data).expect("open delta.7z");
    assert_eq!(reader.len(), 1);
    let content = reader.extract_entry_bytes_stream(0, None).expect("extract delta.txt");
    assert_eq!(content, b"aaaabbbbcccc");
}

#[test]
fn test_fixture_09_delta_bcj2_dag() {
    let data = read_fixture("delta_bcj2.7z");
    let reader = SevenZReader::open_slice(&data).expect("open delta_bcj2.7z");
    assert_eq!(reader.len(), 3);
    let c1 = reader.extract_entry_bytes_stream(0, None).expect("extract code1.bin");
    let c2 = reader.extract_entry_bytes_stream(1, None).expect("extract code2.bin");
    let w = reader.extract_entry_bytes_stream(2, None).expect("extract wave.bin");
    assert_eq!(c1.len(), 9000);
    assert_eq!(c2.len(), 7000);
    assert_eq!(w.len(), 16000);
}

#[test]
fn test_fixture_10_7za433_lzma2_bcj2_x86_binary() {
    let data = read_fixture("7za433_7zip_lzma2_bcj2.7z");
    let reader = SevenZReader::open_slice(&data).expect("open 7za433");
    assert_eq!(reader.len(), 6);
    let idx = reader
        .seek_index()
        .get_by_path("7za433_7zip_lzma2_bcj2/bin/7za.exe")
        .expect("find 7za.exe")
        .file_index;
    let exe = reader.extract_entry_bytes_stream(idx, None).expect("extract 7za.exe");
    assert_eq!(exe.len(), 462336);
    assert_eq!(&exe[..2], b"MZ");
}

#[test]
fn test_fixture_11_bcj_arm64_binary() {
    let data = read_fixture("decompress_example_bcj_arm64.7z");
    let reader = SevenZReader::open_slice(&data).expect("open bcj_arm64");
    assert_eq!(reader.len(), 1);
    let exe = reader.extract_entry_bytes_stream(0, None).expect("extract decompress_arm64.exe");
    assert_eq!(exe.len(), 357888);
    assert_eq!(&exe[..2], b"MZ");
}

#[test]
fn test_fixture_12_lzma2_bcj_x86_binary() {
    let data = read_fixture("decompress_example_lzma2_bcj_x86.7z");
    let reader = SevenZReader::open_slice(&data).expect("open lzma2_bcj_x86");
    assert_eq!(reader.len(), 1);
    let exe = reader.extract_entry_bytes_stream(0, None).expect("extract decompress.exe");
    assert_eq!(exe.len(), 367104);
    assert_eq!(&exe[..2], b"MZ");
}

#[test]
fn test_fixture_13_ppmd() {
    let data = read_fixture("ppmd.7z");
    let reader = SevenZReader::open_slice(&data).expect("open ppmd.7z");
    assert_eq!(reader.len(), 1);
    let txt = reader.extract_entry_bytes_stream(0, None).expect("extract apache2.txt");
    assert_eq!(txt.len(), 11356);
    assert!(txt.starts_with(b"                                 Apache License"));
}

#[test]
fn test_fixture_14_bzip2() {
    let data = read_fixture("bzip2_file.7z");
    let reader = SevenZReader::open_slice(&data).expect("open bzip2_file.7z");
    assert_eq!(reader.len(), 2);
    let h = reader.extract_entry_bytes_stream(0, None).expect("extract hello.txt");
    let f = reader.extract_entry_bytes_stream(1, None).expect("extract foo.txt");
    assert_eq!(h, b"world\n");
    assert_eq!(f, b"bar\n");
}

#[test]
fn test_fixture_15_zstdmt_lz4() {
    let data = read_fixture("zstdmt-lz4.7z");
    let reader = SevenZReader::open_slice(&data).expect("open zstdmt-lz4.7z");
    assert_eq!(reader.len(), 1);
    let lic = reader.extract_entry_bytes_stream(0, None).expect("extract LICENSE");
    assert_eq!(lic.len(), 11556);
    assert!(lic.starts_with(b"                                 Apache License"));
}

#[test]
fn test_fixture_16_zstdmt_brotli() {
    let data = read_fixture("zstdmt-brotli.7z");
    let reader = SevenZReader::open_slice(&data).expect("open zstdmt-brotli.7z");
    assert_eq!(reader.len(), 1);
    let lic = reader.extract_entry_bytes_stream(0, None).expect("extract LICENSE");
    assert_eq!(lic.len(), 11556);
    assert!(lic.starts_with(b"                                 Apache License"));
}

// MARK: - 4. AES-256 Encryption & Interception Matrix

#[test]
fn test_fixture_17_aes_data_encryption() {
    let data = read_fixture("aes_small_test.7z");
    let reader = SevenZReader::open_slice_with_password(&data, Some("iBlm8NTigvru0Jr0"))
        .expect("open aes_small_test");
    assert_eq!(reader.len(), 1);
    let content = reader
        .extract_entry_bytes_stream(0, Some("iBlm8NTigvru0Jr0"))
        .expect("extract encrypted data");
    assert_eq!(content.len(), 129421);
    assert_eq!(crc32_fast(0, &content), 263185209);
}

#[test]
fn test_fixture_18_encrypted_header_and_data() {
    let data = read_fixture("encrypted.7z");
    let reader = SevenZReader::open_slice_with_password(&data, Some("sevenz-rust"))
        .expect("open encrypted.7z");
    assert_eq!(reader.len(), 3);
    let f1 = reader.extract_entry_bytes_stream(1, Some("sevenz-rust")).expect("extract 7zFormat.txt");
    let f2 = reader.extract_entry_bytes_stream(2, Some("sevenz-rust")).expect("extract 7ziplogo.png");
    assert_eq!(f1.len(), 2247);
    assert_eq!(f2.len(), 1417);
    assert_eq!(crc32_fast(0, &f1), 1351509981);
    assert_eq!(crc32_fast(0, &f2), 688999252);
}

#[test]
fn test_wrong_password_interception_data() {
    let data = read_fixture("aes_small_test.7z");
    let reader = SevenZReader::open_slice_with_password(&data, Some("wrong_password")).expect("header open");
    let res = reader.extract_entry_bytes_stream(0, Some("wrong_password"));
    assert!(res.is_err());
}

#[test]
fn test_wrong_password_interception_encoded_header() {
    let data = read_fixture("encrypted.7z");
    let reader = SevenZReader::open_slice_with_password(&data, Some("wrong_password")).expect("header parse");
    let res = reader.extract_entry_bytes_stream(1, Some("wrong_password"));
    assert!(res.is_err());
}

#[test]
fn test_missing_password_interception_encrypted_archive() {
    let data = read_fixture("encrypted.7z");
    let reader = SevenZReader::open_slice(&data).expect("open without pass");
    let res = reader.extract_entry_bytes_stream(1, None);
    assert!(res.is_err());
}

// MARK: - 5. Malformed & Corrupted Archive Rejection

#[test]
fn test_corrupted_signature_header_crc_rejection() {
    let mut data = read_fixture("copy.7z");
    // Corrupt start_header_crc at offset 8
    data[8] ^= 0xFF;
    let res = parse_7z_metadata(&data, None);
    assert!(res.is_err());
}

#[test]
fn test_corrupted_magic_signature_rejection() {
    let mut data = read_fixture("copy.7z");
    data[0] = b'X';
    data[1] = b'X';
    let res = parse_7z_metadata(&data, None);
    assert!(res.is_err());
}

#[test]
fn test_corrupted_next_header_crc_rejection() {
    let mut data = read_fixture("copy.7z");
    // Corrupt next_header_crc at offset 28
    data[28] ^= 0xAA;
    let res = parse_7z_metadata(&data, None);
    assert!(res.is_err());
}

#[test]
fn test_truncated_archive_rejection() {
    let data = read_fixture("copy.7z");
    let truncated = &data[..20];
    let res = parse_7z_metadata(truncated, None);
    assert!(res.is_err());
}

// MARK: - 6. Synthetic Matrix & End-to-End Roundtrip Tests

#[test]
fn test_multi_level_nested_directory_roundtrip() {
    let items = vec![
        ZipInputItem {
            rel_path: "root/".to_string(),
            data: Vec::new(),
            mtime_epoch_secs: 1700000000,
            mode: 0o755,
            is_directory: true,
        },
        ZipInputItem {
            rel_path: "root/sub1/".to_string(),
            data: Vec::new(),
            mtime_epoch_secs: 1700000000,
            mode: 0o755,
            is_directory: true,
        },
        ZipInputItem {
            rel_path: "root/sub1/nested.txt".to_string(),
            data: b"Deeply nested file in 7z structure".to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "root/sub2/file2.bin".to_string(),
            data: vec![0xAB; 4096],
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
    ];

    let bytes = create_7z_solid_archive_bytes(&items, 5, 2).expect("create 7z");
    let reader = SevenZArchive::open_slice(&bytes).expect("open 7z");
    assert_eq!(reader.len(), 4);

    let d1 = reader.extract_entry_bytes_stream(2, None).expect("extract nested");
    assert_eq!(d1, b"Deeply nested file in 7z structure");

    let d2 = reader.extract_entry_bytes_stream(3, None).expect("extract bin");
    assert_eq!(d2, vec![0xAB; 4096]);
}

#[test]
fn test_100_small_files_solid_aggregation_roundtrip() {
    let count = 100;
    let mut items = Vec::with_capacity(count);
    for i in 0..count {
        items.push(ZipInputItem {
            rel_path: format!("files/file_{:03}.txt", i),
            data: format!("Content for file index #{} in solid 7z archive", i).into_bytes(),
            mtime_epoch_secs: 1700000000 + (i as u32),
            mode: 0o644,
            is_directory: false,
        });
    }

    let bytes = create_7z_solid_archive_bytes(&items, 3, 2).expect("create solid 100");
    let reader = SevenZArchive::open_slice(&bytes).expect("open solid 100");
    assert_eq!(reader.len(), count);

    for (i, item) in items.iter().enumerate() {
        let extracted = reader.extract_entry_bytes_stream(i, None).expect("extract item");
        assert_eq!(extracted, item.data, "Mismatch at file index {}", i);
    }
}

#[test]
fn test_large_file_streaming_roundtrip() {
    let payload_size = 2 * 1024 * 1024; // 2 MB
    let mut large_data = Vec::with_capacity(payload_size);
    for i in 0..payload_size {
        large_data.push((i % 251) as u8);
    }

    let items = vec![
        ZipInputItem {
            rel_path: "large_data.bin".to_string(),
            data: large_data.clone(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
    ];

    let bytes = create_7z_solid_archive_bytes(&items, 4, 4).expect("create large 7z");
    let reader = SevenZArchive::open_slice(&bytes).expect("open large 7z");
    assert_eq!(reader.len(), 1);

    let extracted = reader.extract_entry_bytes_stream(0, None).expect("extract large");
    assert_eq!(extracted.len(), payload_size);
    assert_eq!(crc32_fast(0, &extracted), crc32_fast(0, &large_data));
}

#[test]
fn test_zero_byte_file_in_solid_stream() {
    let items = vec![
        ZipInputItem {
            rel_path: "before.txt".to_string(),
            data: b"Data before zero-byte file".to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "empty.bin".to_string(),
            data: Vec::new(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "after.txt".to_string(),
            data: b"Data after zero-byte file".to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
    ];

    let bytes = create_7z_solid_archive_bytes(&items, 3, 1).expect("create zero-byte mix");
    let reader = SevenZArchive::open_slice(&bytes).expect("open zero-byte mix");
    assert_eq!(reader.len(), 3);

    let e0 = reader.extract_entry_bytes_stream(0, None).expect("extract before");
    let e1 = reader.extract_entry_bytes_stream(1, None).expect("extract empty");
    let e2 = reader.extract_entry_bytes_stream(2, None).expect("extract after");

    assert_eq!(e0, b"Data before zero-byte file");
    assert!(e1.is_empty());
    assert_eq!(e2, b"Data after zero-byte file");
}

#[test]
fn test_seek_index_path_lookup_and_offsets() {
    let items = vec![
        ZipInputItem {
            rel_path: "docs/readme.md".to_string(),
            data: b"# Readme".to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "src/main.rs".to_string(),
            data: b"fn main() {}".to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
    ];

    let bytes = create_7z_solid_archive_bytes(&items, 3, 1).expect("create");
    let reader = SevenZArchive::open_slice(&bytes).expect("open");
    let loc = reader.seek_index().get_by_path("src/main.rs").expect("find path");
    assert_eq!(loc.file_index, 1);
    assert_eq!(loc.uncompressed_size, 12);
    assert_eq!(loc.offset_in_folder, 8); // after 8 bytes of "# Readme"
}

#[test]
fn test_solid_early_exit_selective_extraction() {
    let items = vec![
        ZipInputItem {
            rel_path: "first.txt".to_string(),
            data: b"First file in solid stream".to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "second.txt".to_string(),
            data: vec![0x42u8; 100_000],
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "third.txt".to_string(),
            data: vec![0x99u8; 500_000],
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
    ];

    let bytes = create_7z_solid_archive_bytes(&items, 3, 1).expect("create");
    let archive = SevenZArchive::open_slice(&bytes).expect("open");

    // Extract first file only - early exit should not decode second and third
    let extractor = archive.solid_extractor();
    let (f0_out, stats) = extractor
        .extract_to_vec(0, None)
        .expect("extract file 0");
    assert_eq!(f0_out, b"First file in solid stream");
    assert_eq!(stats.extracted_target_bytes, 26);
    assert!(stats.early_exit_triggered);
}

#[test]
fn test_extract_all_to_disk_with_sanitizer() {
    let items = vec![
        ZipInputItem {
            rel_path: "folder/".to_string(),
            data: Vec::new(),
            mtime_epoch_secs: 1700000000,
            mode: 0o755,
            is_directory: true,
        },
        ZipInputItem {
            rel_path: "folder/sample.txt".to_string(),
            data: b"Sample disk extraction test".to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
    ];

    let bytes = create_7z_solid_archive_bytes(&items, 3, 1).expect("create");
    let archive = SevenZArchive::open_slice(&bytes).expect("open");

    let temp = tempfile::tempdir().expect("tempdir");
    let options = ttzip_engine::types::TTZipExtractOptions::default();

    let report = archive.extract_all(temp.path(), &options).expect("extract all");
    assert_eq!(report.processed_entries_count, 2);
    assert_eq!(report.total_uncompressed_bytes, 27);

    let sample_file = temp.path().join("folder/sample.txt");
    assert!(sample_file.exists());
    assert_eq!(fs::read(&sample_file).unwrap(), b"Sample disk extraction test");
}

#[test]
fn test_compression_levels_matrix_roundtrip() {
    let items = vec![ZipInputItem {
        rel_path: "data.txt".to_string(),
        data: b"Data repeated ".repeat(500),
        mtime_epoch_secs: 1700000000,
        mode: 0o644,
        is_directory: false,
    }];

    for level in [1, 3, 5, 7, 9] {
        let bytes = create_7z_solid_archive_bytes(&items, level, 1)
            .unwrap_or_else(|e| panic!("Level {} failed: {:?}", level, e));
        let archive = SevenZArchive::open_slice(&bytes).unwrap();
        let extracted = archive.extract_entry_bytes_stream(0, None).unwrap();
        assert_eq!(extracted.len(), items[0].data.len());
        assert_eq!(extracted, items[0].data);
    }
}
