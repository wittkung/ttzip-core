// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive Test Suite for 7-Zip Multi-Threaded Offline Block Preparation & Append Pipeline.

use std::io::Cursor;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::sevenz::{
    prepare_block, PreparedBlock, SevenZArchiveWriter, SevenZEncoderMethod, SevenZEncoderOptions,
    SevenZReader,
};

#[derive(Debug, Clone)]
struct TestFileItem {
    name: String,
    data: Vec<u8>,
    crc: u32,
}

fn create_synthetic_data(seed: u32, size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    let mut state = seed;
    while data.len() < size {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        let bytes = state.to_le_bytes();
        let copy_len = (size - data.len()).min(4);
        data.extend_from_slice(&bytes[..copy_len]);
    }
    data
}

#[test]
fn test_single_prepared_block_lzma2_and_copy_roundtrip() {
    let raw_files = [
        TestFileItem {
            name: "docs/readme.txt".to_string(),
            data: b"TTZip High-performance 7z Multi-threaded Archiver Engine.\n".to_vec(),
            crc: crc32_fast(0, b"TTZip High-performance 7z Multi-threaded Archiver Engine.\n"),
        },
        TestFileItem {
            name: "src/main.rs".to_string(),
            data: b"fn main() { println!(\"Hello TTZip!\"); }\n".to_vec(),
            crc: crc32_fast(0, b"fn main() { println!(\"Hello TTZip!\"); }\n"),
        },
        TestFileItem {
            name: "assets/binary.dat".to_string(),
            data: create_synthetic_data(0x12345678, 65536),
            crc: crc32_fast(0, &create_synthetic_data(0x12345678, 65536)),
        },
    ];

    // 1. Test with LZMA2 compression
    let lzma2_entries: Vec<(String, u64, Cursor<Vec<u8>>)> = raw_files
        .iter()
        .map(|f| (f.name.clone(), f.data.len() as u64, Cursor::new(f.data.clone())))
        .collect();

    let lzma2_options = SevenZEncoderOptions {
        compression_level: 6,
        thread_budget: 2,
        dict_size: None,
    };

    let block_lzma2 = prepare_block(lzma2_entries, SevenZEncoderMethod::Lzma2, lzma2_options)
        .expect("prepare_block with LZMA2 failed");

    assert_eq!(block_lzma2.substream_sizes.len(), 3);
    assert_eq!(block_lzma2.substream_crcs.len(), 3);
    assert_eq!(block_lzma2.unpack_size, (raw_files[0].data.len() + raw_files[1].data.len() + raw_files[2].data.len()) as u64);
    assert!(!block_lzma2.compressed_data.is_empty());

    let mut cursor = Cursor::new(Vec::new());
    let mut writer = SevenZArchiveWriter::new(&mut cursor).expect("create writer failed");
    let names_lzma2: Vec<String> = raw_files.iter().map(|f| f.name.clone()).collect();
    writer
        .push_prepared_block(block_lzma2, names_lzma2)
        .expect("push block failed");
    let total_written = writer.finalize().expect("finalize archive failed");
    assert!(total_written > 32);

    let archive_bytes = cursor.into_inner();
    let reader = SevenZReader::open_slice(&archive_bytes).expect("open 7z archive slice failed");
    assert_eq!(reader.len(), 3);

    for (i, expected) in raw_files.iter().enumerate() {
        let extracted = reader
            .extract_entry_bytes_stream(i, None)
            .expect("extract entry failed");
        assert_eq!(extracted, expected.data);
        assert_eq!(crc32_fast(0, &extracted), expected.crc);
    }

    // 2. Test with Copy (Store) mode
    let copy_entries: Vec<(String, u64, Cursor<Vec<u8>>)> = raw_files
        .iter()
        .map(|f| (f.name.clone(), f.data.len() as u64, Cursor::new(f.data.clone())))
        .collect();

    let copy_options = SevenZEncoderOptions {
        compression_level: 0,
        thread_budget: 1,
        dict_size: None,
    };

    let block_copy = prepare_block(copy_entries, SevenZEncoderMethod::Copy, copy_options)
        .expect("prepare_block with Copy failed");

    let mut cursor_copy = Cursor::new(Vec::new());
    let mut writer_copy = SevenZArchiveWriter::new(&mut cursor_copy).expect("create copy writer failed");
    let names_copy: Vec<String> = raw_files.iter().map(|f| f.name.clone()).collect();
    writer_copy
        .push_prepared_block(block_copy, names_copy)
        .expect("push copy block failed");
    writer_copy.finalize().expect("finalize copy archive failed");

    let copy_archive_bytes = cursor_copy.into_inner();
    let copy_reader = SevenZReader::open_slice(&copy_archive_bytes).expect("open copy archive slice failed");
    assert_eq!(copy_reader.len(), 3);

    for (i, expected) in raw_files.iter().enumerate() {
        let extracted = copy_reader
            .extract_entry_bytes_stream(i, None)
            .expect("extract copy entry failed");
        assert_eq!(extracted, expected.data);
        assert_eq!(crc32_fast(0, &extracted), expected.crc);
    }
}

#[test]
fn test_multi_concurrent_prepared_blocks_roundtrip() {
    const NUM_BLOCKS: usize = 4;
    const FILES_PER_BLOCK: usize = 5;

    // Generate test data partitioned into blocks
    let mut all_expected_files: Vec<TestFileItem> = Vec::new();
    let mut block_file_groups: Vec<Vec<TestFileItem>> = Vec::new();

    for b in 0..NUM_BLOCKS {
        let mut group = Vec::new();
        for f in 0..FILES_PER_BLOCK {
            let name = format!("block_{:02}/sub_item_{:03}.bin", b, f);
            let size = 8192 + (b * 1337 + f * 997) % 16384;
            let data = create_synthetic_data(((b * 100 + f) as u32).wrapping_mul(0xdeadbeef), size);
            let crc = crc32_fast(0, &data);
            let item = TestFileItem { name, data, crc };
            group.push(item.clone());
            all_expected_files.push(item);
        }
        block_file_groups.push(group);
    }

    // Prepare blocks concurrently across independent threads
    let mut handles = Vec::new();
    for group in block_file_groups {
        let handle = thread::spawn(move || {
            let entries: Vec<(String, u64, Cursor<Vec<u8>>)> = group
                .iter()
                .map(|item| (item.name.clone(), item.data.len() as u64, Cursor::new(item.data.clone())))
                .collect();
            let names: Vec<String> = group.iter().map(|item| item.name.clone()).collect();
            let options = SevenZEncoderOptions {
                compression_level: 5,
                thread_budget: 1,
                dict_size: None,
            };
            let block = prepare_block(entries, SevenZEncoderMethod::Lzma2, options)
                .expect("concurrent prepare_block failed");
            (block, names)
        });
        handles.push(handle);
    }

    let mut prepared_results: Vec<(PreparedBlock, Vec<String>)> = Vec::new();
    for handle in handles {
        prepared_results.push(handle.join().expect("thread join failed"));
    }

    // Sequentially append all prepared blocks into the 7z archive writer
    let mut cursor = Cursor::new(Vec::new());
    let mut writer = SevenZArchiveWriter::new(&mut cursor).expect("writer creation failed");

    for (block, names) in prepared_results {
        writer
            .push_prepared_block(block, names)
            .expect("push prepared block failed");
    }

    let total_archive_size = writer.finalize().expect("writer finalize failed");
    assert!(total_archive_size > 0);

    let archive_bytes = cursor.into_inner();
    assert_eq!(archive_bytes.len() as u64, total_archive_size);

    // Verify generated multi-folder 7z archive using SevenZReader
    let reader = SevenZReader::open_slice(&archive_bytes).expect("SevenZReader open failed");
    assert_eq!(reader.len(), NUM_BLOCKS * FILES_PER_BLOCK);
    assert_eq!(reader.info().folders.len(), NUM_BLOCKS);

    for (idx, expected) in all_expected_files.iter().enumerate() {
        let file_meta = &reader.files()[idx];
        assert_eq!(file_meta.rel_path, expected.name);
        assert!(!file_meta.is_directory);

        let extracted = reader
            .extract_entry_bytes_stream(idx, None)
            .expect("extract entry failed");
        assert_eq!(extracted.len(), expected.data.len());
        assert_eq!(crc32_fast(0, &extracted), expected.crc);
        assert_eq!(extracted, expected.data);
    }
}

#[test]
fn test_mixed_directories_and_empty_files_in_prepared_blocks() {
    let mixed_entries = vec![
        ("assets/".to_string(), 0u64, Cursor::new(Vec::new())),
        ("assets/empty_placeholder.txt".to_string(), 0u64, Cursor::new(Vec::new())),
        ("assets/logo.svg".to_string(), 32u64, Cursor::new(b"<svg height='100'></svg>".to_vec())),
        ("configs/".to_string(), 0u64, Cursor::new(Vec::new())),
        ("configs/app.json".to_string(), 20u64, Cursor::new(b"{\"version\":\"1.0.0\"}".to_vec())),
    ];

    let names: Vec<String> = mixed_entries.iter().map(|(n, _, _)| n.clone()).collect();
    let options = SevenZEncoderOptions::default();

    let block = prepare_block(mixed_entries, SevenZEncoderMethod::Lzma2, options)
        .expect("prepare block with mixed entries failed");

    // Only 2 non-empty files should produce substreams in the folder
    assert_eq!(block.substream_sizes.len(), 2);
    assert_eq!(block.substream_crcs.len(), 2);
    let expected_unpack = b"<svg height='100'></svg>".len() as u64 + b"{\"version\":\"1.0.0\"}".len() as u64;
    assert_eq!(block.unpack_size, expected_unpack);

    let mut cursor = Cursor::new(Vec::new());
    let mut writer = SevenZArchiveWriter::new(&mut cursor).expect("create writer failed");
    writer
        .push_prepared_block(block, names)
        .expect("push mixed block failed");
    writer.finalize().expect("finalize mixed archive failed");

    let archive_bytes = cursor.into_inner();
    let reader = SevenZReader::open_slice(&archive_bytes).expect("open mixed archive failed");
    assert_eq!(reader.len(), 5);

    // Entry 0: assets/ (directory)
    let f0 = &reader.files()[0];
    assert_eq!(f0.rel_path, "assets/");
    assert!(f0.is_directory);
    assert!(reader.extract_entry_bytes_stream(0, None).unwrap().is_empty());

    // Entry 1: assets/empty_placeholder.txt (empty file)
    let f1 = &reader.files()[1];
    assert_eq!(f1.rel_path, "assets/empty_placeholder.txt");
    assert!(!f1.is_directory);
    assert!(reader.extract_entry_bytes_stream(1, None).unwrap().is_empty());

    // Entry 2: assets/logo.svg (content file)
    let f2 = reader.extract_entry_bytes_stream(2, None).unwrap();
    assert_eq!(f2, b"<svg height='100'></svg>");

    // Entry 3: configs/ (directory)
    let f3 = &reader.files()[3];
    assert_eq!(f3.rel_path, "configs/");
    assert!(f3.is_directory);

    // Entry 4: configs/app.json (content file)
    let f4 = reader.extract_entry_bytes_stream(4, None).unwrap();
    assert_eq!(f4, b"{\"version\":\"1.0.0\"}");
}

#[test]
fn test_multithreaded_pipeline_throughput_scaling() {
    const NUM_BLOCKS: usize = 4;
    const CHUNK_SIZE: usize = 256 * 1024; // 256 KB per block -> 1 MB total

    let block_payloads: Vec<Vec<u8>> = (0..NUM_BLOCKS)
        .map(|i| create_synthetic_data(0xabcdef01 + (i as u32), CHUNK_SIZE))
        .collect();

    let shared_payloads = Arc::new(block_payloads);

    // 1. Benchmark Sequential Preparation
    let start_seq = Instant::now();
    let mut seq_blocks = Vec::new();
    for i in 0..NUM_BLOCKS {
        let data = shared_payloads[i].clone();
        let entry = vec![(format!("seq_item_{}.bin", i), data.len() as u64, Cursor::new(data))];
        let options = SevenZEncoderOptions {
            compression_level: 6,
            thread_budget: 1,
            dict_size: None,
        };
        let block = prepare_block(entry, SevenZEncoderMethod::Lzma2, options).unwrap();
        seq_blocks.push(block);
    }
    let seq_duration = start_seq.elapsed();

    // 2. Benchmark Parallel Preparation across 4 worker threads
    let start_par = Instant::now();
    let mut handles = Vec::new();
    for i in 0..NUM_BLOCKS {
        let payloads_ref = Arc::clone(&shared_payloads);
        let handle = thread::spawn(move || {
            let data = payloads_ref[i].clone();
            let entry = vec![(format!("par_item_{}.bin", i), data.len() as u64, Cursor::new(data))];
            let options = SevenZEncoderOptions {
                compression_level: 6,
                thread_budget: 1,
                dict_size: None,
            };
            prepare_block(entry, SevenZEncoderMethod::Lzma2, options).unwrap()
        });
        handles.push(handle);
    }

    let mut par_blocks = Vec::new();
    for handle in handles {
        par_blocks.push(handle.join().unwrap());
    }
    let par_duration = start_par.elapsed();

    println!(
        "Throughput comparison (4x 256KB blocks): Sequential = {:?}, Parallel = {:?} (Speedup: {:.2}x)",
        seq_duration,
        par_duration,
        (seq_duration.as_secs_f64() / par_duration.as_secs_f64().max(0.0001))
    );

    // 3. Assemble parallel blocks and verify data integrity
    let mut cursor = Cursor::new(Vec::new());
    let mut writer = SevenZArchiveWriter::new(&mut cursor).expect("writer init failed");
    for (i, block) in par_blocks.into_iter().enumerate() {
        let name = format!("par_item_{}.bin", i);
        writer.push_prepared_block(block, vec![name]).unwrap();
    }
    writer.finalize().expect("writer finalize failed");

    let archive_bytes = cursor.into_inner();
    let reader = SevenZReader::open_slice(&archive_bytes).expect("open archive failed");
    assert_eq!(reader.len(), NUM_BLOCKS);

    for i in 0..NUM_BLOCKS {
        let extracted = reader.extract_entry_bytes_stream(i, None).unwrap();
        assert_eq!(extracted, shared_payloads[i]);
    }
}
