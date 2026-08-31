// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive test suite for 7-Zip Solid Stream O(1) Indexing and 4MB Micro-Buffer Early-Exit Decompression.

use sha2::{Digest, Sha256};
use std::io::Cursor;

use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::sevenz::{
    create_7z_solid_archive_bytes, format::*, SevenZArchive, SevenZSignatureHeader, SolidFolderIndex,
};
use ttzip_engine::types::TTZipStatus;
use ttzip_engine::zip::writer::ZipInputItem;

struct GroundTruthFile {
    rel_path: String,
    data: Vec<u8>,
    crc32: u32,
    sha256_hex: String,
}

fn generate_solid_dataset(count: usize) -> (Vec<ZipInputItem>, Vec<GroundTruthFile>) {
    let mut items = Vec::with_capacity(count);
    let mut ground_truth = Vec::with_capacity(count);

    for i in 0..count {
        let rel_path = format!("dataset/folder_a/sub_module_{:04}.bin", i);
        // Varying data sizes between 16KB and 48KB (averaging ~32KB)
        let size = 16384 + ((i * 1013) % 32768);
        let mut data = Vec::with_capacity(size);

        let header = format!(
            "=== TTZip Solid Stream Test Entry {:04} | Seed 0x{:08x} | Timestamp {} ===\n",
            i,
            (i as u32).wrapping_mul(0x45d9f3b),
            1700000000 + i * 7
        );
        data.extend_from_slice(header.as_bytes());

        let seed = (i as u32).wrapping_mul(0x27d4eb2d);
        while data.len() < size {
            let chunk_idx = data.len();
            let line = format!(
                "Offset {:06}: Block Seed=0x{:08x}, Pattern Index={}\n",
                chunk_idx,
                seed.wrapping_add(chunk_idx as u32),
                i
            );
            data.extend_from_slice(line.as_bytes());
        }
        data.truncate(size);

        let crc = crc32_fast(0, &data);
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let sha256_hex = hex::encode(hasher.finalize());

        ground_truth.push(GroundTruthFile {
            rel_path: rel_path.clone(),
            data: data.clone(),
            crc32: crc,
            sha256_hex,
        });

        items.push(ZipInputItem {
            rel_path,
            data,
            mtime_epoch_secs: (1700000000 + i * 7) as u32,
            mode: 0o644,
            is_directory: false,
        });
    }

    (items, ground_truth)
}

#[test]
fn test_solid_folder_index_o1_jump_table_math() {
    let (items, ground_truth) = generate_solid_dataset(100);
    let archive_bytes = create_7z_solid_archive_bytes(&items, 3, 2).expect("create 7z failed");
    let archive = SevenZArchive::open_slice(&archive_bytes).expect("open archive failed");

    let solid_index = SolidFolderIndex::build(archive.info());
    assert_eq!(solid_index.file_count(), 100);
    assert_eq!(solid_index.folder_count(), 1);

    let folder_table = solid_index.folder(0).expect("folder 0 must exist");
    assert_eq!(folder_table.substream_to_file_index.len(), 100);
    assert_eq!(folder_table.stream_prefix_sums.len(), 101);
    assert_eq!(folder_table.stream_prefix_sums[0], 0);

    let mut expected_offset = 0u64;
    for i in 0..100 {
        let expected_size = ground_truth[i].data.len() as u64;
        let expected_end = expected_offset + expected_size;

        // Verify prefix sum jump table
        assert_eq!(folder_table.stream_prefix_sums[i + 1], expected_end);

        // Verify O(1) folder stream range
        let range = solid_index.folder_stream_range(0, i).expect("range for stream");
        assert_eq!(range, (expected_offset, expected_end));

        // Verify O(1) file lookup
        let file_range = solid_index.lookup(i).expect("file range");
        assert_eq!(file_range.file_index, i);
        assert_eq!(file_range.rel_path, ground_truth[i].rel_path);
        assert_eq!(file_range.folder_index, Some(0));
        assert_eq!(file_range.substream_index, Some(i));
        assert_eq!(file_range.offset_start, expected_offset);
        assert_eq!(file_range.offset_end, expected_end);
        assert_eq!(file_range.uncompressed_size, expected_size);
        assert_eq!(file_range.crc, Some(ground_truth[i].crc32));

        // Verify lookup by path
        let by_path = solid_index.lookup_by_path(&ground_truth[i].rel_path).expect("find by path");
        assert_eq!(by_path, file_range);

        expected_offset = expected_end;
    }

    assert_eq!(solid_index.folder_total_size(0), Some(expected_offset));
}

#[test]
fn test_solid_stream_first_middle_last_extraction_and_early_exit() {
    let (items, ground_truth) = generate_solid_dataset(300);
    let total_uncompressed_bytes: u64 = ground_truth.iter().map(|g| g.data.len() as u64).sum();
    assert!(
        total_uncompressed_bytes > 4 * 1024 * 1024,
        "Total uncompressed size must exceed 4MB micro-buffer chunk"
    );

    let archive_bytes = create_7z_solid_archive_bytes(&items, 3, 2).expect("create 7z failed");
    let archive = SevenZArchive::open_slice(&archive_bytes).expect("open archive failed");
    let extractor = archive.solid_extractor();

    let test_indices = [
        0usize,       // First file (0)
        300 / 2,      // Middle file (N/2 = 150)
        299usize,     // Last file (N-1 = 299)
    ];

    for &idx in &test_indices {
        let expected = &ground_truth[idx];

        // 1. In-memory extraction with stats
        let (extracted_vec, stats) = extractor
            .extract_to_vec(idx, None)
            .unwrap_or_else(|e| panic!("failed to extract entry {}: {:?}", idx, e));

        // Verify data integrity: byte equality, CRC32, SHA-256
        assert_eq!(extracted_vec.len(), expected.data.len());
        assert_eq!(extracted_vec, expected.data);

        let computed_crc = crc32_fast(0, &extracted_vec);
        assert_eq!(computed_crc, expected.crc32);
        assert_eq!(stats.computed_crc, expected.crc32);
        assert!(stats.crc_matched);

        let mut hasher = Sha256::new();
        hasher.update(&extracted_vec);
        assert_eq!(hex::encode(hasher.finalize()), expected.sha256_hex);

        // Verify spatial stats
        let loc = archive.solid_index().lookup(idx).unwrap();
        assert_eq!(stats.target_offset_start, loc.offset_start);
        assert_eq!(stats.target_offset_end, loc.offset_end);
        assert_eq!(stats.skipped_preceding_bytes, loc.offset_start);
        assert_eq!(stats.extracted_target_bytes, loc.uncompressed_size);

        // Verify Early-Exit behavior
        if idx == 0 {
            // First file: 0 skipped preceding bytes, early exit must trigger
            assert_eq!(stats.skipped_preceding_bytes, 0);
            assert!(
                stats.early_exit_triggered,
                "Early Exit must trigger for entry 0"
            );
            assert!(
                stats.decompressed_bytes_total < total_uncompressed_bytes,
                "Entry 0 decompressed total {} must be < total archive size {}",
                stats.decompressed_bytes_total,
                total_uncompressed_bytes
            );
        } else if idx == 150 {
            // Middle file: skipped preceding bytes must equal offset_start, early exit must trigger
            assert_eq!(stats.skipped_preceding_bytes, loc.offset_start);
            assert!(
                stats.early_exit_triggered,
                "Early Exit must trigger for middle entry 150"
            );
            assert!(
                stats.decompressed_bytes_total < total_uncompressed_bytes,
                "Entry 150 decompressed total {} must be < total archive size {}",
                stats.decompressed_bytes_total,
                total_uncompressed_bytes
            );
            assert!(
                stats.decompressed_bytes_total >= loc.offset_end,
                "Decompressed bytes must at least cover target file end"
            );
        } else if idx == 299 {
            // Last file: skipped preceding bytes must equal offset_start
            assert_eq!(stats.skipped_preceding_bytes, loc.offset_start);
            assert_eq!(stats.decompressed_bytes_total, total_uncompressed_bytes);
        }

        // 2. Direct-to-Writer stream extraction
        let mut cursor = Cursor::new(Vec::new());
        let writer_stats = extractor
            .extract_to_writer(idx, None, &mut cursor)
            .expect("extract to writer failed");

        assert_eq!(cursor.into_inner(), expected.data);
        assert_eq!(writer_stats.computed_crc, expected.crc32);
        assert!(writer_stats.crc_matched);
    }
}

#[test]
fn test_solid_stream_extract_by_path() {
    let (items, ground_truth) = generate_solid_dataset(20);
    let archive_bytes = create_7z_solid_archive_bytes(&items, 3, 2).expect("create 7z failed");
    let archive = SevenZArchive::open_slice(&archive_bytes).expect("open archive failed");
    let extractor = archive.solid_extractor();

    for file in &ground_truth {
        let (data, stats) = extractor
            .extract_by_path_to_vec(&file.rel_path, None)
            .unwrap_or_else(|e| panic!("failed to extract by path {}: {:?}", file.rel_path, e));

        assert_eq!(data, file.data);
        assert_eq!(stats.computed_crc, file.crc32);
    }

    // Non-existent path returns ErrFileNotFound
    let bad_res = extractor.extract_by_path_to_vec("non/existent/file.txt", None);
    assert_eq!(bad_res.unwrap_err(), TTZipStatus::ErrFileNotFound);
}

#[test]
fn test_solid_stream_budget_limit_guard() {
    let (items, ground_truth) = generate_solid_dataset(20);
    let archive_bytes = create_7z_solid_archive_bytes(&items, 3, 2).expect("create 7z failed");
    let archive = SevenZArchive::open_slice(&archive_bytes).expect("open archive failed");

    let solid_index = archive.solid_index();
    let entry15 = solid_index.lookup(15).unwrap();
    assert!(entry15.offset_start > 0);

    // Set budget strictly smaller than entry 15's preceding offset
    let small_budget = entry15.offset_start - 1;
    let extractor_budgeted = archive.solid_extractor().with_preceding_budget(small_budget);

    let res = extractor_budgeted.extract_to_vec(15, None);
    assert_eq!(res.unwrap_err(), TTZipStatus::ErrSolidBudgetExceeded);

    // Entry 0 should still succeed since offset_start is 0 <= budget
    let (entry0_data, _) = extractor_budgeted
        .extract_to_vec(0, None)
        .expect("entry 0 within budget");
    assert_eq!(entry0_data, ground_truth[0].data);
}

#[test]
fn test_solid_stream_empty_files_and_directories() {
    let items = vec![
        ZipInputItem {
            rel_path: "empty_dir/".to_string(),
            data: Vec::new(),
            mtime_epoch_secs: 1700000000,
            mode: 0o755,
            is_directory: true,
        },
        ZipInputItem {
            rel_path: "empty_dir/zero_byte.txt".to_string(),
            data: Vec::new(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "real_file.txt".to_string(),
            data: b"Content of real file".to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        },
    ];

    let archive_bytes = create_7z_solid_archive_bytes(&items, 3, 2).expect("create 7z failed");
    let archive = SevenZArchive::open_slice(&archive_bytes).expect("open archive failed");
    let extractor = archive.solid_extractor();

    // Directory entry (index 0)
    let (dir_data, dir_stats) = extractor.extract_to_vec(0, None).expect("extract dir");
    assert!(dir_data.is_empty());
    assert_eq!(dir_stats.extracted_target_bytes, 0);
    assert!(dir_stats.crc_matched);

    // Zero-byte file (index 1)
    let (zero_data, zero_stats) = extractor.extract_to_vec(1, None).expect("extract zero byte");
    assert!(zero_data.is_empty());
    assert_eq!(zero_stats.extracted_target_bytes, 0);
    assert!(zero_stats.crc_matched);

    // Real file (index 2)
    let (real_data, real_stats) = extractor.extract_to_vec(2, None).expect("extract real file");
    assert_eq!(real_data, b"Content of real file");
    assert_eq!(real_stats.extracted_target_bytes, 20);
    assert!(real_stats.crc_matched);
}

#[test]
fn test_solid_stream_multi_folder_archive() {
    let f0_data = b"Stream 0 inside Folder 0!";
    let f1_data = b"Stream 1 inside Folder 0!";
    let mut folder0_uncomp = Vec::new();
    folder0_uncomp.extend_from_slice(f0_data);
    folder0_uncomp.extend_from_slice(f1_data);

    let f2_data = b"Stream 0 inside Folder 1!";
    let folder1_uncomp = f2_data.to_vec();

    let mut payload = Vec::new();
    payload.extend_from_slice(&folder0_uncomp);
    payload.extend_from_slice(&folder1_uncomp);

    let mut h = Vec::new();
    h.push(K_HEADER);
    h.push(K_MAIN_STREAMS_INFO);

    // PackInfo
    h.push(K_PACK_INFO);
    write_varint(0, &mut h);
    write_varint(2, &mut h); // 2 pack streams
    h.push(K_SIZE);
    write_varint(folder0_uncomp.len() as u64, &mut h);
    write_varint(folder1_uncomp.len() as u64, &mut h);
    h.push(K_END);

    // UnpackInfo
    h.push(K_UNPACK_INFO);
    h.push(K_FOLDER);
    write_varint(2, &mut h); // 2 folders
    h.push(0); // external = 0

    // Folder 0: 1 Coder (METHOD_COPY)
    write_varint(1, &mut h);
    h.push(0x01);
    h.push(0x00);

    // Folder 1: 1 Coder (METHOD_COPY)
    write_varint(1, &mut h);
    h.push(0x01);
    h.push(0x00);

    // CodersUnpackSize
    h.push(K_CODERS_UNPACK_SIZE);
    write_varint(folder0_uncomp.len() as u64, &mut h);
    write_varint(folder1_uncomp.len() as u64, &mut h);
    h.push(K_END);

    // SubStreamsInfo
    h.push(K_SUB_STREAMS_INFO);
    h.push(K_NUM_UNPACK_STREAM);
    write_varint(2, &mut h); // Folder 0: 2 streams
    write_varint(1, &mut h); // Folder 1: 1 stream

    h.push(K_SIZE);
    write_varint(f0_data.len() as u64, &mut h); // Folder 0 stream 0 size

    h.push(K_END); // end kSubStreamsInfo
    h.push(K_END); // end kMainStreamsInfo

    // FilesInfo
    h.push(K_FILES_INFO);
    write_varint(3, &mut h); // 3 files

    // Name
    h.push(K_NAME);
    let names = ["f0.txt", "f1.txt", "f2.txt"];
    let mut names_u16 = Vec::new();
    for name in &names {
        for u in name.encode_utf16() {
            names_u16.extend_from_slice(&u.to_le_bytes());
        }
        names_u16.extend_from_slice(&0u16.to_le_bytes());
    }
    write_varint((1 + names_u16.len()) as u64, &mut h);
    h.push(0); // external
    h.extend_from_slice(&names_u16);

    // WinAttributes
    h.push(K_WIN_ATTRIBUTES);
    write_varint((2 + 3 * 4) as u64, &mut h);
    h.push(1); // allDefined
    h.push(0); // external
    h.extend_from_slice(&0x20u32.to_le_bytes());
    h.extend_from_slice(&0x20u32.to_le_bytes());
    h.extend_from_slice(&0x20u32.to_le_bytes());

    h.push(K_END); // end kFilesInfo
    h.push(K_END); // end kHeader

    let sig = SevenZSignatureHeader {
        major_version: 0,
        minor_version: 4,
        start_header_crc: 0,
        next_header_offset: payload.len() as u64,
        next_header_size: h.len() as u64,
        next_header_crc: crc32_fast(0, &h),
    };

    let mut archive_bytes = Vec::new();
    archive_bytes.extend_from_slice(&sig.serialize());
    archive_bytes.extend_from_slice(&payload);
    archive_bytes.extend_from_slice(&h);

    let archive = SevenZArchive::open_slice(&archive_bytes).expect("open multi-folder archive");
    assert_eq!(archive.len(), 3);
    assert_eq!(archive.solid_index().folder_count(), 2);

    let extractor = archive.solid_extractor();

    // Extract f0 from Folder 0
    let (data0, stats0) = extractor.extract_to_vec(0, None).expect("extract f0");
    assert_eq!(data0, f0_data);
    assert_eq!(stats0.folder_index, 0);
    assert_eq!(stats0.target_offset_start, 0);

    // Extract f1 from Folder 0
    let (data1, stats1) = extractor.extract_to_vec(1, None).expect("extract f1");
    assert_eq!(data1, f1_data);
    assert_eq!(stats1.folder_index, 0);
    assert_eq!(stats1.target_offset_start, f0_data.len() as u64);

    // Extract f2 from Folder 1
    let (data2, stats2) = extractor.extract_to_vec(2, None).expect("extract f2");
    assert_eq!(data2, f2_data);
    assert_eq!(stats2.folder_index, 1);
    assert_eq!(stats2.target_offset_start, 0);
}
