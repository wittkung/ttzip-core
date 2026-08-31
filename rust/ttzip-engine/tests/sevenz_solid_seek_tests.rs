// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit and integration test suite for 7-Zip Solid Stream
//! O(1) Indexing, 4MB Micro-Buffer Sliding Discard, and Early-Exit Circuit Breaker.

use std::io::{Cursor, Read, Result as IoResult};

use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::sevenz::dag::SevenZError;
use ttzip_engine::sevenz::solid_seek::{
    SolidEarlyExitExtractor, SolidFolderIndex, SOLID_MICRO_BUFFER_SIZE,
};

/// Counting reader wrapper to track exact physical byte reads from the underlying stream.
struct CountingReader<R: Read> {
    inner: R,
    bytes_read: u64,
}

impl<R: Read> CountingReader<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            bytes_read: 0,
        }
    }

    pub fn bytes_read(&self) -> u64 {
        self.bytes_read
    }
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let n = self.inner.read(buf)?;
        self.bytes_read += n as u64;
        Ok(n)
    }
}

/// Helper to generate deterministic synthetic files for solid stream tests.
struct TestSubstream {
    data: Vec<u8>,
    crc: u32,
}

fn generate_synthetic_dataset(count: usize, base_seed: u32) -> Vec<TestSubstream> {
    let mut items = Vec::with_capacity(count);
    for i in 0..count {
        let size = 1024 + ((i * 313 + 17) % 8192);
        let mut data = Vec::with_capacity(size);

        let header = format!(
            "=== TTZip Solid Substream #{:04} | Seed=0x{:08x} ===\n",
            i,
            base_seed.wrapping_add(i as u32)
        );
        data.extend_from_slice(header.as_bytes());

        let pattern = (i as u32).wrapping_mul(0x9e3779b9);
        while data.len() < size {
            let chunk_idx = data.len();
            let line = format!(
                "Offset {:05}: Pattern=0x{:08x}\n",
                chunk_idx,
                pattern.wrapping_add(chunk_idx as u32)
            );
            data.extend_from_slice(line.as_bytes());
        }
        data.truncate(size);

        let crc = crc32_fast(0, &data);
        items.push(TestSubstream { data, crc });
    }
    items
}

#[test]
fn test_solid_folder_index_o1_jump_table_math() {
    let dataset = generate_synthetic_dataset(10, 0x12345678);
    let sizes: Vec<u64> = dataset.iter().map(|d| d.data.len() as u64).collect();
    let crcs: Vec<Option<u32>> = dataset.iter().map(|d| Some(d.crc)).collect();

    let index = SolidFolderIndex::from_sizes_and_crcs(&sizes, &crcs);

    assert_eq!(index.len(), 10);
    assert!(!index.is_empty());

    let mut expected_offset = 0u64;
    for (i, item) in dataset.iter().enumerate() {
        let meta = index.get(i).expect("substream must exist");
        assert_eq!(meta.sub_index, i);
        assert_eq!(meta.start_offset, expected_offset);
        assert_eq!(meta.size, item.data.len() as u64);
        assert_eq!(meta.crc, Some(item.crc));
        assert_eq!(meta.end_offset(), expected_offset + item.data.len() as u64);

        let range = index.substream_range(i).expect("range must exist");
        assert_eq!(range, (expected_offset, meta.end_offset()));

        expected_offset += item.data.len() as u64;
    }

    assert_eq!(index.total_uncompressed_size(), expected_offset);
    assert_eq!(index.prefix_sums().len(), 11);
    assert_eq!(index.prefix_sums()[10], expected_offset);
}

#[test]
fn test_solid_early_exit_10_files_extraction_indices_0_5_9() {
    let dataset = generate_synthetic_dataset(10, 0xcafe1234);
    let mut combined_stream = Vec::new();
    let mut sizes = Vec::new();
    let mut crcs = Vec::new();

    for item in &dataset {
        combined_stream.extend_from_slice(&item.data);
        sizes.push(item.data.len() as u64);
        crcs.push(Some(item.crc));
    }

    let index = SolidFolderIndex::from_sizes_and_crcs(&sizes, &crcs);
    let test_indices = [0usize, 5usize, 9usize];

    for &target_idx in &test_indices {
        let mut cursor = CountingReader::new(Cursor::new(&combined_stream));
        let mut extracted_output = Vec::new();

        let report = SolidEarlyExitExtractor::extract_substream_with_stats(
            &mut cursor,
            target_idx,
            &index,
            &mut extracted_output,
        )
        .expect("extract substream failed");

        // Verify data integrity
        let expected = &dataset[target_idx];
        assert_eq!(extracted_output.len(), expected.data.len());
        assert_eq!(extracted_output, expected.data);

        // Verify CRC32
        assert_eq!(report.computed_crc, expected.crc);
        assert_eq!(report.expected_crc, Some(expected.crc));
        assert!(report.crc_matched);

        // Verify spatial stats
        let meta = index.get(target_idx).unwrap();
        assert_eq!(report.start_offset, meta.start_offset);
        assert_eq!(report.size, meta.size);
        assert_eq!(report.skipped_preceding_bytes, meta.start_offset);
        assert_eq!(report.extracted_bytes, meta.size);

        // Verify physical Early-Exit behavior
        let expected_total_read = meta.end_offset();
        assert_eq!(cursor.bytes_read(), expected_total_read);

        if target_idx < 9 {
            assert!(
                report.early_exit_triggered,
                "Early exit must trigger for sub_index {}",
                target_idx
            );
            assert!(
                cursor.bytes_read() < combined_stream.len() as u64,
                "Substream {} read {} bytes, must be less than total solid block size {}",
                target_idx,
                cursor.bytes_read(),
                combined_stream.len()
            );
        } else {
            // Last file: early_exit_triggered is false because whole block was consumed
            assert!(!report.early_exit_triggered);
            assert_eq!(cursor.bytes_read(), combined_stream.len() as u64);
        }
    }
}

#[test]
fn test_solid_early_exit_100_files_random_substream_extraction() {
    let dataset = generate_synthetic_dataset(100, 0xdeadbeef);
    let mut combined_stream = Vec::new();
    let mut sizes = Vec::new();
    let mut crcs = Vec::new();

    for item in &dataset {
        combined_stream.extend_from_slice(&item.data);
        sizes.push(item.data.len() as u64);
        crcs.push(Some(item.crc));
    }

    let index = SolidFolderIndex::from_sizes_and_crcs(&sizes, &crcs);
    let target_indices = [0, 5, 9, 27, 50, 73, 99];

    for &idx in &target_indices {
        let mut cursor = CountingReader::new(Cursor::new(&combined_stream));
        let (extracted_data, crc) =
            SolidEarlyExitExtractor::extract_substream_to_vec(&mut cursor, idx, &index)
                .unwrap_or_else(|e| panic!("failed to extract substream {}: {:?}", idx, e));

        let expected = &dataset[idx];
        assert_eq!(extracted_data.len(), expected.data.len());
        assert_eq!(extracted_data, expected.data);
        assert_eq!(crc, expected.crc);

        // Verify physical read stopped immediately after target substream
        let meta = index.get(idx).unwrap();
        assert_eq!(cursor.bytes_read(), meta.end_offset());
    }
}

#[test]
fn test_solid_early_exit_physical_zero_decompression_verification() {
    // 50 files of 10KB each (total 500KB)
    let count = 50;
    let file_size = 10 * 1024;
    let mut combined_stream = Vec::with_capacity(count * file_size);
    let mut sizes = Vec::with_capacity(count);
    let mut crcs = Vec::with_capacity(count);

    for i in 0..count {
        let chunk = vec![(i & 0xFF) as u8; file_size];
        let crc = crc32_fast(0, &chunk);
        combined_stream.extend_from_slice(&chunk);
        sizes.push(file_size as u64);
        crcs.push(Some(crc));
    }

    let index = SolidFolderIndex::from_sizes_and_crcs(&sizes, &crcs);

    // Case 1: Extract entry 0 (first file)
    {
        let mut reader = CountingReader::new(Cursor::new(&combined_stream));
        let mut out = Vec::new();
        let crc = SolidEarlyExitExtractor::extract_substream(&mut reader, 0, &index, &mut out)
            .expect("extract 0");

        assert_eq!(crc, crcs[0].unwrap());
        assert_eq!(out.len(), file_size);
        // Physical zero read for files 1..49 (490KB never touched)
        assert_eq!(reader.bytes_read(), file_size as u64);
    }

    // Case 2: Extract entry 5 (6th file)
    {
        let mut reader = CountingReader::new(Cursor::new(&combined_stream));
        let mut out = Vec::new();
        let crc = SolidEarlyExitExtractor::extract_substream(&mut reader, 5, &index, &mut out)
            .expect("extract 5");

        assert_eq!(crc, crcs[5].unwrap());
        assert_eq!(out.len(), file_size);
        // Preceding 5 files (50KB) skipped, 6th file (10KB) read, files 6..49 (440KB) never touched
        assert_eq!(reader.bytes_read(), (6 * file_size) as u64);
    }

    // Case 3: Extract entry 9 (10th file)
    {
        let mut reader = CountingReader::new(Cursor::new(&combined_stream));
        let mut out = Vec::new();
        let crc = SolidEarlyExitExtractor::extract_substream(&mut reader, 9, &index, &mut out)
            .expect("extract 9");

        assert_eq!(crc, crcs[9].unwrap());
        assert_eq!(out.len(), file_size);
        // Preceding 9 files (90KB) skipped, 10th file (10KB) read, files 10..49 (400KB) never touched
        assert_eq!(reader.bytes_read(), (10 * file_size) as u64);
    }
}

/// Custom infinite stream generator to simulate multi-megabyte Solid Blocks without pre-allocating memory.
struct SyntheticSolidStream {
    total_bytes: u64,
    current_pos: u64,
    target_start: u64,
    target_payload: Vec<u8>,
}

impl SyntheticSolidStream {
    pub fn new(total_bytes: u64, target_start: u64, target_payload: Vec<u8>) -> Self {
        Self {
            total_bytes,
            current_pos: 0,
            target_start,
            target_payload,
        }
    }
}

impl Read for SyntheticSolidStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        if self.current_pos >= self.total_bytes {
            return Ok(0);
        }

        let available = (self.total_bytes - self.current_pos).min(buf.len() as u64) as usize;
        let mut written = 0;

        for i in 0..available {
            let pos = self.current_pos + i as u64;
            let target_end = self.target_start + self.target_payload.len() as u64;
            if pos >= self.target_start && pos < target_end {
                let offset_in_target = (pos - self.target_start) as usize;
                buf[i] = self.target_payload[offset_in_target];
            } else {
                buf[i] = (pos & 0xFF) as u8;
            }
            written += 1;
        }

        self.current_pos += written as u64;
        Ok(written)
    }
}

#[test]
fn test_solid_seek_zero_heap_memory_bounded_large_stream() {
    // 64MB preceding stream before target file (Tests 4MB micro-buffer chunked discard)
    let preceding_bytes = 64 * 1024 * 1024u64;
    let target_data = b"Target payload nestled at 64MB offset in massive 7z solid block!".to_vec();
    let target_size = target_data.len() as u64;
    let target_crc = crc32_fast(0, &target_data);
    let trailing_bytes = 32 * 1024 * 1024u64;
    let total_bytes = preceding_bytes + target_size + trailing_bytes;

    let sizes = vec![preceding_bytes, target_size, trailing_bytes];
    let crcs = vec![None, Some(target_crc), None];
    let index = SolidFolderIndex::from_sizes_and_crcs(&sizes, &crcs);

    let stream = SyntheticSolidStream::new(total_bytes, preceding_bytes, target_data.clone());
    let mut counting_stream = CountingReader::new(stream);
    let mut output = Vec::new();

    let report = SolidEarlyExitExtractor::extract_substream_with_stats(
        &mut counting_stream,
        1,
        &index,
        &mut output,
    )
    .expect("extract from large stream failed");

    assert_eq!(output, target_data);
    assert_eq!(report.computed_crc, target_crc);
    assert!(report.crc_matched);
    assert_eq!(report.skipped_preceding_bytes, preceding_bytes);
    assert_eq!(report.extracted_bytes, target_size);
    assert!(report.early_exit_triggered);

    // Verify exact bytes consumed: preceding (64MB) + target (63B), trailing 32MB untouched!
    assert_eq!(
        counting_stream.bytes_read(),
        preceding_bytes + target_size
    );
    const { assert!(SOLID_MICRO_BUFFER_SIZE == 4 * 1024 * 1024) };
}

#[test]
fn test_solid_seek_crc_mismatch_error() {
    let dummy_data = b"Some valid content".to_vec();
    let wrong_crc = 0x12345678;
    let sizes = vec![dummy_data.len() as u64];
    let crcs = vec![Some(wrong_crc)];
    let index = SolidFolderIndex::from_sizes_and_crcs(&sizes, &crcs);

    let mut cursor = Cursor::new(dummy_data);
    let mut out = Vec::new();
    let res = SolidEarlyExitExtractor::extract_substream(&mut cursor, 0, &index, &mut out);

    match res {
        Err(SevenZError::CrcMismatch { expected, computed }) => {
            assert_eq!(expected, wrong_crc);
            assert_ne!(computed, wrong_crc);
        }
        other => panic!("Expected CrcMismatch error, got {:?}", other),
    }
}

#[test]
fn test_solid_seek_out_of_bounds_substream_index() {
    let sizes = vec![100, 200];
    let index = SolidFolderIndex::from_sizes(&sizes);

    let mut cursor = Cursor::new(vec![0u8; 300]);
    let mut out = Vec::new();
    let res = SolidEarlyExitExtractor::extract_substream(&mut cursor, 99, &index, &mut out);

    match res {
        Err(SevenZError::InvalidSubstreamIndex { index, total }) => {
            assert_eq!(index, 99);
            assert_eq!(total, 2);
        }
        other => panic!("Expected InvalidSubstreamIndex, got {:?}", other),
    }
}

#[test]
fn test_solid_seek_unexpected_eof() {
    let sizes = vec![500];
    let index = SolidFolderIndex::from_sizes(&sizes);

    // Provide only 100 bytes when 500 are expected
    let mut cursor = Cursor::new(vec![0u8; 100]);
    let mut out = Vec::new();
    let res = SolidEarlyExitExtractor::extract_substream(&mut cursor, 0, &index, &mut out);

    match res {
        Err(SevenZError::UnexpectedEof { required, actual }) => {
            assert_eq!(required, 400);
            assert_eq!(actual, 100);
        }
        other => panic!("Expected UnexpectedEof, got {:?}", other),
    }
}

#[test]
fn test_solid_seek_empty_file_handling() {
    let sizes = vec![100, 0, 200];
    let crcs = vec![None, Some(0), None];
    let index = SolidFolderIndex::from_sizes_and_crcs(&sizes, &crcs);

    let stream_data = vec![0xAAu8; 300];
    let mut cursor = CountingReader::new(Cursor::new(&stream_data));
    let mut out = Vec::new();

    let report = SolidEarlyExitExtractor::extract_substream_with_stats(
        &mut cursor,
        1,
        &index,
        &mut out,
    )
    .expect("extract empty substream");

    assert!(out.is_empty());
    assert_eq!(report.extracted_bytes, 0);
    assert_eq!(report.computed_crc, 0);
    assert!(report.crc_matched);
    assert_eq!(cursor.bytes_read(), 100); // Only skipped the first file
}
